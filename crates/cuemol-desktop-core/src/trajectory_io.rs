use super::*;

impl GuiTrajectoryFrames {
    pub(super) fn is_lazy(&self) -> bool {
        match self {
            Self::File(_) => true,
            Self::Selected { source, .. } => source.is_lazy(),
            _ => false,
        }
    }

    pub(super) fn dcd_bytes(&self, atom_count: usize) -> Result<Vec<u8>> {
        fn record(output: &mut Vec<u8>, data: &[u8]) -> Result<()> {
            let size = i32::try_from(data.len()).map_err(|_| {
                CueMolError::invalid_argument("DCD record exceeds the format limit")
            })?;
            output.extend(size.to_le_bytes());
            output.extend_from_slice(data);
            output.extend(size.to_le_bytes());
            Ok(())
        }
        let atoms = i32::try_from(atom_count)
            .map_err(|_| CueMolError::invalid_argument("too many DCD atoms"))?;
        let frames = i32::try_from(self.len())
            .map_err(|_| CueMolError::invalid_argument("too many DCD frames"))?;
        let has_cell = self.len() > 0 && self.frame_with_cell(0)?.1.is_some();
        let mut header = [0_u8; 84];
        header[..4].copy_from_slice(b"CORD");
        header[4..8].copy_from_slice(&frames.to_le_bytes());
        header[12..16].copy_from_slice(&1_i32.to_le_bytes());
        header[40..44].copy_from_slice(&1_f32.to_le_bytes());
        header[44..48].copy_from_slice(&i32::from(has_cell).to_le_bytes());
        header[80..84].copy_from_slice(&24_i32.to_le_bytes());
        let mut output = Vec::new();
        record(&mut output, &header)?;
        record(&mut output, &0_i32.to_le_bytes())?;
        record(&mut output, &atoms.to_le_bytes())?;
        let mut axis = Vec::with_capacity(atom_count.saturating_mul(4));
        for index in 0..self.len() {
            let (positions, cell) = self.frame_with_cell(index)?;
            if positions.len() != atom_count {
                return Err(CueMolError::invalid_argument(
                    "trajectory atom counts differ",
                ));
            }
            if has_cell {
                let [a, b, c, alpha, beta, gamma] = cell.ok_or_else(|| {
                    CueMolError::invalid_argument("trajectory cell is missing in a frame")
                })?;
                let data = [a, gamma, b, beta, alpha, c]
                    .into_iter()
                    .flat_map(f64::to_le_bytes)
                    .collect::<Vec<_>>();
                record(&mut output, &data)?;
            }
            for component in 0..3 {
                axis.clear();
                for p in &positions {
                    let value = [p.x, p.y, p.z][component] as f32;
                    axis.extend(value.to_le_bytes());
                }
                record(&mut output, &axis)?;
            }
        }
        Ok(output)
    }
}

fn reference_path(path: &Path, directory: bool) -> Option<PathBuf> {
    let absolute = crate::absolute_scene_path(path).ok()?;
    if directory {
        absolute.canonicalize().ok()
    } else {
        absolute
            .parent()
            .zip(absolute.file_name())
            .and_then(|(parent, name)| parent.canonicalize().ok().map(|parent| parent.join(name)))
    }
}

pub(crate) fn relative_source(source: &str, base: Option<&Path>) -> String {
    let Some(base) = base else {
        return source.to_string();
    };
    let Some((absolute, base)) =
        reference_path(Path::new(source), false).zip(reference_path(base, true))
    else {
        return source.to_string();
    };
    let target = absolute.components().collect::<Vec<_>>();
    let origin = base.components().collect::<Vec<_>>();
    let common = origin
        .iter()
        .zip(&target)
        .take_while(|(a, b)| a == b)
        .count();
    if common == 0 {
        return crate::absolute_scene_path(Path::new(source))
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| source.to_string());
    }
    let mut result = vec!["..".to_string(); origin.len() - common];
    for component in &target[common..] {
        result.push(component.as_os_str().to_string_lossy().into_owned());
    }
    result.join("/")
}

fn reader_type(format: &str) -> &'static str {
    match format.to_ascii_lowercase().as_str() {
        "xtc" => "xtctraj",
        "trr" => "trrtraj",
        "nc" | "netcdf" => "ambnetcdftraj",
        "mdcrd" | "crd" => "mdcrd",
        _ => "dcdtraj",
    }
}

fn materialize(indexed: cuemol_mdtools::IndexedTrajectory) -> Result<GuiTrajectoryFrames> {
    let mut block = cuemol_mdtools::TrajBlock::new(indexed.atom_count());
    for index in 0..indexed.frame_count() {
        let frame = indexed.read_frame(index)?;
        block.append_frame(frame.coordinates, frame.cell)?;
    }
    Ok(GuiTrajectoryFrames::Owned(std::sync::Arc::new(block)))
}

impl DesktopAppState {
    pub(super) fn sync_trajectory_qsc_nodes(
        &mut self,
        scene_uid: Uid,
        embed_all: bool,
        base: Option<&Path>,
    ) -> Result<()> {
        let states = self
            .trajectories
            .iter()
            .filter(|(uid, _)| self.scene_manager().scene_uid_for_object(**uid) == Some(scene_uid))
            .map(|(uid, data)| (*uid, data.clone()))
            .collect::<Vec<_>>();
        for (uid, data) in states {
            let object = self
                .scene_manager_mut()
                .object_mut(uid)
                .ok_or_else(|| CueMolError::not_found("trajectory object"))?;
            let mut extensions = object
                .qsc_extension_nodes()
                .iter()
                .filter(|node| node.tag_name() != "trajfiles")
                .cloned()
                .collect::<Vec<_>>();
            let mut extra = object.qsc_extra_data_map().clone();
            for old in object
                .qsc_extension_nodes()
                .iter()
                .filter(|node| node.tag_name() == "trajfiles")
            {
                for block in old.children() {
                    extra.remove(block.str_attr("src").as_str());
                }
            }
            let mut files = LDom2Node::new();
            files.set_tag_name("trajfiles");
            for (index, block) in data.blocks.iter().enumerate() {
                let node = files.append_child("trajfile");
                node.set_type_name("TrajBlock");
                if embed_all || block.source.is_empty() || block.source.starts_with("datachunk:") {
                    let source = format!("datachunk:trajectory-{}-{index}", uid.get());
                    node.set_str_attr("src", &source);
                    node.set_str_attr("srctype", "dcdtraj");
                    extra.insert(
                        source,
                        block.frames.dcd_bytes(object.mol().atom_count())?.into(),
                    );
                } else {
                    node.set_str_attr("src", relative_source(&block.source, base));
                    node.set_str_attr("srctype", reader_type(&block.format));
                    let options = node.append_child("ropts");
                    options.set_str_attr("nevery", block.stride.to_string());
                    options.set_str_attr("lazy_load", block.frames.is_lazy().to_string());
                }
            }
            extensions.push(files);
            object.set_object_type("Trajectory");
            object.set_qsc_extra_attr("frame_aver_size", data.frame_average_size.to_string());
            if let Some(selection) = &data.load_selection {
                object.set_qsc_extra_attr("loadsel", selection);
            } else {
                object.remove_qsc_extra_attr("loadsel");
            }
            if let (Some(count), Some(indices)) =
                (data.source_atom_count, data.atom_indices.as_ref())
            {
                object.set_qsc_extra_attr("load_atom_count", count.to_string());
                object.set_qsc_extra_attr(
                    "load_atom_indices",
                    indices
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            } else {
                object.remove_qsc_extra_attr("load_atom_count");
                object.remove_qsc_extra_attr("load_atom_indices");
            }
            object.replace_shared_qsc_extensions(extensions, extra);
        }
        Ok(())
    }

    pub(super) fn restore_scene_trajectories(&mut self, scene_uid: Uid) -> Result<()> {
        let Some(scene) = self.scene_manager().scene(scene_uid) else {
            return Ok(());
        };
        let base = PathBuf::from(scene.base_path());
        let objects = scene
            .objects()
            .iter()
            .filter(|object| object.object_type() == "Trajectory")
            .cloned()
            .collect::<Vec<_>>();
        for object in objects {
            let mut molecule = object.mol().clone();
            let attrs = object.qsc_extra_attrs();
            let mut data = GuiTrajectoryData::default();
            data.frame_average_size = attrs
                .get("frame_aver_size")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            if let (Some(count), Some(indices)) =
                (attrs.get("load_atom_count"), attrs.get("load_atom_indices"))
            {
                let count = count
                    .parse::<usize>()
                    .map_err(|_| CueMolError::parse("invalid trajectory atom count"))?;
                let indices = if indices.is_empty() {
                    Vec::new()
                } else {
                    indices
                        .split(',')
                        .map(|v| {
                            v.parse::<usize>()
                                .map_err(|_| CueMolError::parse("invalid trajectory atom index"))
                        })
                        .collect::<Result<Vec<_>>>()?
                };
                if indices.len() != molecule.atom_count()
                    || indices.iter().any(|index| *index >= count)
                {
                    return Err(CueMolError::parse(
                        "trajectory load selection does not match topology",
                    ));
                }
                data.source_atom_count = Some(count);
                data.atom_indices = Some(std::sync::Arc::new(indices));
                data.load_selection = attrs.get("loadsel").cloned();
            } else {
                let selection = attrs
                    .get("loadsel")
                    .map(|s| parse_selector(s))
                    .transpose()?;
                data.apply_load_selection(&mut molecule, selection.as_ref())?;
            }
            if let Some(files) = object
                .qsc_extension_nodes()
                .iter()
                .find(|n| n.tag_name() == "trajfiles")
            {
                for node in files
                    .children()
                    .iter()
                    .filter(|n| n.tag_name() == "trajfile" && !n.is_attr())
                {
                    let source = node.str_attr("src").to_string();
                    let source_type = node.str_attr("srctype").to_string();
                    let options = node.find_child("ropts");
                    let stride = options
                        .and_then(|n| n.str_attr("nevery").as_str().parse::<usize>().ok())
                        .unwrap_or(1);
                    let lazy = options.is_none_or(|n| n.str_attr("lazy_load").as_str() != "false");
                    let (frames, source, format) = if source.starts_with("datachunk:") {
                        let bytes = object
                            .qsc_extra_data_shared(&source)
                            .ok_or_else(|| CueMolError::not_found("trajectory data chunk"))?;
                        let hint = if source_type == "mdcrd" {
                            "trajectory.mdcrd"
                        } else {
                            "trajectory"
                        };
                        let indexed =
                            cuemol_mdtools::IndexedTrajectory::from_shared_bytes_with_name(
                                bytes,
                                hint,
                                stride,
                                if source_type == "mdcrd" {
                                    Some(molecule.atom_count())
                                } else {
                                    None
                                },
                            )?;
                        let format = indexed.format_name().to_string();
                        let frames = if lazy {
                            GuiTrajectoryFrames::File(std::sync::Arc::new(indexed))
                        } else {
                            materialize(indexed)?
                        };
                        (frames, source, format)
                    } else {
                        let path = Path::new(&source);
                        let path = if path.is_absolute() {
                            path.to_path_buf()
                        } else {
                            base.join(path)
                        };
                        let indexed = cuemol_mdtools::IndexedTrajectory::open(
                            &path,
                            stride,
                            data.source_atom_count,
                        )?;
                        let format = indexed.format_name().to_string();
                        let frames = if lazy {
                            GuiTrajectoryFrames::File(std::sync::Arc::new(indexed))
                        } else {
                            materialize(indexed)?
                        };
                        (frames, path.to_string_lossy().into_owned(), format)
                    };
                    let block = GuiTrajectoryBlockData {
                        uid: self.allocate_trajectory_block_uid(),
                        source,
                        format,
                        stride,
                        frames,
                    };
                    data.blocks
                        .push(data.prepare_block(block, molecule.atom_count())?);
                }
            }
            data.initialized = !data.blocks.is_empty();
            if data.frame_count() > 0 {
                data.apply_frame(&mut molecule, 0)?;
            }
            self.scene_manager_mut()
                .object_mut(object.uid())
                .unwrap()
                .replace_mol(molecule);
            self.trajectories.insert(object.uid(), data);
        }
        Ok(())
    }
}

pub(super) fn read_extra_qsc_molecule_with_options(
    format: &str,
    bytes: &[u8],
    options: Option<&LDom2Node>,
    base_path: Option<&Path>,
) -> Result<MolCoord> {
    if format == "namdcoor" {
        let topology = options
            .map(|node| node.str_attr("subPath_topo").to_string())
            .filter(|path| !path.is_empty())
            .ok_or_else(|| {
                CueMolError::invalid_argument("NAMD coordinate source requires a topology path")
            })?;
        let path = Path::new(&topology);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base_path.unwrap_or_else(|| Path::new(".")).join(path)
        };
        let psf = cuemol_molstr::read_text_maybe_compressed(&path)?;
        return cuemol_mdtools::read_namd_coor_with_psf_bytes(bytes, &psf)
            .map(|load| load.molecule);
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|error| CueMolError::parse(format!("invalid trajectory topology: {error}")))?;
    match format {
        "gro" => cuemol_mdtools::read_gro_str(text),
        "psf" => cuemol_mdtools::read_psf_str(text),
        "amberprm" | "amberprmtop" => {
            let mode = amber_bond_mode(
                options
                    .map(|node| node.str_attr("bondmode").to_string())
                    .as_deref()
                    .unwrap_or("file"),
            )?;
            cuemol_mdtools::read_amber_prmtop_str_with_bond_mode(text, mode)
        }
        _ => Err(CueMolError::invalid_argument(format!(
            "unsupported QSC molecule source type {format}"
        ))),
    }
}

pub(super) fn amber_bond_mode(value: &str) -> Result<i32> {
    use cuemol_mdtools::AmberPrmtopReader;
    match value {
        "" | "file" | "0" => Ok(AmberPrmtopReader::BONDMODE_FILE),
        "hybrid" | "1" => Ok(AmberPrmtopReader::BONDMODE_HYBRID),
        "autogen" | "2" => Ok(AmberPrmtopReader::BONDMODE_AUTOGEN),
        _ => Err(CueMolError::invalid_argument(format!(
            "invalid AMBER bond mode: {value}"
        ))),
    }
}
