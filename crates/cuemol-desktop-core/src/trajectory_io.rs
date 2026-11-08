use super::*;

enum FramePositions<'a> {
    Vectors(&'a [Vector4]),
    Packed(&'a [f32]),
}

struct FrameView<'a> {
    positions: FramePositions<'a>,
    indices: Option<&'a [usize]>,
    count: usize,
    cell: Option<[f64; 6]>,
}

impl<'a> FrameView<'a> {
    fn new(
        positions: FramePositions<'a>,
        indices: Option<&'a [usize]>,
        cell: Option<[f64; 6]>,
    ) -> Result<Self> {
        let source_count = match &positions {
            FramePositions::Vectors(values) => values.len(),
            FramePositions::Packed(values) => values.len() / 3,
        };
        if indices.is_some_and(|indices| indices.iter().any(|index| *index >= source_count)) {
            return Err(CueMolError::invalid_argument(
                "trajectory selection index is out of range",
            ));
        }
        Ok(Self {
            positions,
            indices,
            count: indices.map_or(source_count, <[usize]>::len),
            cell,
        })
    }

    fn position(&self, index: usize) -> Vector4 {
        let index = self.indices.map_or(index, |indices| indices[index]);
        match &self.positions {
            FramePositions::Vectors(values) => values[index],
            FramePositions::Packed(values) => Vector4::new3(
                values[index * 3] as f64,
                values[index * 3 + 1] as f64,
                values[index * 3 + 2] as f64,
            ),
        }
    }

    fn accumulate(&self, sums: &mut [Vector4]) {
        fn add(sums: &mut [Vector4], values: impl Iterator<Item = Vector4>) {
            for (sum, value) in sums.iter_mut().zip(values) {
                *sum = *sum + value;
            }
        }
        let unpack =
            |value: &[f32]| Vector4::new3(value[0] as f64, value[1] as f64, value[2] as f64);
        match (&self.positions, self.indices) {
            (FramePositions::Vectors(values), None) => add(sums, values.iter().copied()),
            (FramePositions::Vectors(values), Some(indices)) => {
                add(sums, indices.iter().map(|&index| values[index]))
            }
            (FramePositions::Packed(values), None) => add(sums, values.chunks_exact(3).map(unpack)),
            (FramePositions::Packed(values), Some(indices)) => add(
                sums,
                indices
                    .iter()
                    .map(|&index| unpack(&values[index * 3..index * 3 + 3])),
            ),
        }
    }
}

fn apply_coordinates(
    molecule: &mut MolCoord,
    count: usize,
    indices: Option<&[usize]>,
    cell: Option<[f64; 6]>,
    position: impl Fn(usize) -> Vector4,
) -> Result<()> {
    if indices.is_some_and(|indices| indices.iter().any(|index| *index >= count)) {
        return Err(CueMolError::invalid_argument(
            "trajectory selection index is out of range",
        ));
    }
    if let Some(indices) = indices {
        molecule.set_positions(indices.iter().map(|index| position(*index)))?;
    } else {
        molecule.set_positions((0..count).map(position))?;
    }
    if let Some([a, b, c, alpha, beta, gamma]) = cell {
        molecule.set_cell_params(cuemol_core::CellParams::new(a, b, c, alpha, beta, gamma));
    }
    molecule.fire_atoms_moved();
    Ok(())
}

impl GuiTrajectoryFrames {
    fn with_frame<T>(
        &self,
        index: usize,
        indices: Option<&[usize]>,
        read: impl FnOnce(FrameView<'_>) -> Result<T>,
    ) -> Result<T> {
        match self {
            Self::Memory(frames) => {
                let frame = frames
                    .get(index)
                    .ok_or_else(|| CueMolError::not_found("trajectory frame"))?;
                read(FrameView::new(
                    FramePositions::Vectors(frame),
                    indices,
                    None,
                )?)
            }
            Self::Owned(block) => {
                let frame = block
                    .frame(index)
                    .ok_or_else(|| CueMolError::not_found("trajectory frame"))?;
                read(FrameView::new(
                    FramePositions::Packed(frame),
                    indices,
                    block.cell(index),
                )?)
            }
            Self::File(file) => {
                let frame = file.read_frame(index)?;
                read(FrameView::new(
                    FramePositions::Packed(&frame.coordinates),
                    indices,
                    frame.cell,
                )?)
            }
            Self::Selected {
                source,
                indices: selected,
            } => {
                if let Some(indices) = indices {
                    let remapped = indices
                        .iter()
                        .map(|index| {
                            selected.get(*index).copied().ok_or_else(|| {
                                CueMolError::invalid_argument(
                                    "trajectory selection index is out of range",
                                )
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    source.with_frame(index, Some(&remapped), read)
                } else {
                    source.with_frame(index, Some(selected), read)
                }
            }
        }
    }

    pub(super) fn frame_with_cell(&self, index: usize) -> Result<(Vec<Vector4>, Option<[f64; 6]>)> {
        self.with_frame(index, None, |frame| {
            Ok((
                (0..frame.count)
                    .map(|index| frame.position(index))
                    .collect(),
                frame.cell,
            ))
        })
    }

    pub(super) fn accumulate_frame(
        &self,
        index: usize,
        positions: &mut Vec<Vector4>,
        initialize: bool,
    ) -> Result<Option<[f64; 6]>> {
        self.with_frame(index, None, |frame| {
            if initialize {
                positions.clear();
                positions.resize(frame.count, Vector4::ZERO);
            } else if positions.len() != frame.count {
                return Err(CueMolError::invalid_argument(
                    "trajectory atom counts differ",
                ));
            }
            frame.accumulate(positions);
            Ok(frame.cell)
        })
    }

    pub(super) fn frame_atom_count(&self, index: usize) -> Result<usize> {
        match self {
            Self::Memory(frames) => frames
                .get(index)
                .map(Vec::len)
                .ok_or_else(|| CueMolError::not_found("trajectory frame")),
            Self::Owned(block) => block
                .frame(index)
                .map(|frame| frame.len() / 3)
                .ok_or_else(|| CueMolError::not_found("trajectory frame")),
            Self::File(file) => Ok(file.read_frame(index)?.coordinates.len() / 3),
            Self::Selected { source, indices } => {
                let count = source.frame_atom_count(index)?;
                if indices.iter().any(|index| *index >= count) {
                    return Err(CueMolError::invalid_argument(
                        "trajectory selection index is out of range",
                    ));
                }
                Ok(indices.len())
            }
        }
    }

    pub(super) fn apply_frame(
        &self,
        molecule: &mut MolCoord,
        index: usize,
        indices: Option<&[usize]>,
    ) -> Result<()> {
        match self {
            Self::Memory(frames) => {
                let frame = frames
                    .get(index)
                    .ok_or_else(|| CueMolError::not_found("trajectory frame"))?;
                apply_coordinates(molecule, frame.len(), indices, None, |index| frame[index])
            }
            Self::Owned(block) => {
                let frame = block
                    .frame(index)
                    .ok_or_else(|| CueMolError::not_found("trajectory frame"))?;
                apply_coordinates(molecule, frame.len() / 3, indices, block.cell(index), |i| {
                    Vector4::new3(
                        frame[i * 3] as f64,
                        frame[i * 3 + 1] as f64,
                        frame[i * 3 + 2] as f64,
                    )
                })
            }
            Self::File(file) => {
                let frame = file.read_frame(index)?;
                apply_coordinates(
                    molecule,
                    frame.coordinates.len() / 3,
                    indices,
                    frame.cell,
                    |i| {
                        Vector4::new3(
                            frame.coordinates[i * 3] as f64,
                            frame.coordinates[i * 3 + 1] as f64,
                            frame.coordinates[i * 3 + 2] as f64,
                        )
                    },
                )
            }
            Self::Selected {
                source,
                indices: selected,
            } => {
                if let Some(indices) = indices {
                    let remapped = indices
                        .iter()
                        .map(|index| {
                            selected.get(*index).copied().ok_or_else(|| {
                                CueMolError::invalid_argument(
                                    "trajectory selection index is out of range",
                                )
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    source.apply_frame(molecule, index, Some(&remapped))
                } else {
                    source.apply_frame(molecule, index, Some(selected))
                }
            }
        }
    }

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
            let coordinate = options
                .map(|node| node.str_attr("subPath_coord").to_string())
                .filter(|path| !path.trim().is_empty());
            let restart = coordinate
                .map(|coordinate| {
                    let path = Path::new(&coordinate);
                    let path = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        base_path.unwrap_or_else(|| Path::new(".")).join(path)
                    };
                    cuemol_molstr::read_text_maybe_compressed(path)
                })
                .transpose()?;
            cuemol_mdtools::read_amber_prmtop_with_restart_str_and_bond_mode(
                text,
                restart.as_deref(),
                mode,
            )
            .map(|loaded| loaded.molecule)
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

#[cfg(test)]
mod playback_tests {
    use super::*;
    use std::sync::Arc;

    fn molecule(count: usize) -> MolCoord {
        let template = cuemol_molstr::read_pdb_str(
            "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n",
        )
        .unwrap()
        .atoms()[0]
            .clone();
        let mut molecule = MolCoord::new();
        for index in 0..count {
            let mut atom = template.clone();
            atom.serial = index as i32 + 1;
            molecule.push_atom(atom);
        }
        molecule
    }

    #[test]
    fn direct_playback_preserves_all_storage_kinds_selection_order_and_cells() {
        let cell = Some([10., 11., 12., 90., 80., 75.]);
        let mut block = cuemol_mdtools::TrajBlock::new(3);
        block
            .append_frame(vec![-0., 2., 3., 4., 5., 6., 7., 8., 9.], cell)
            .unwrap();
        block
            .append_frame(vec![1., 3., 5., 7., 9., 11., 13., 15., 17.], cell)
            .unwrap();
        let owned = GuiTrajectoryFrames::Owned(Arc::new(block));
        let vectors = GuiTrajectoryFrames::Memory(Arc::new(
            (0..2)
                .map(|index| owned.frame_with_cell(index).unwrap().0)
                .collect(),
        ));
        let bytes = owned.dcd_bytes(3).unwrap();
        let file = GuiTrajectoryFrames::File(Arc::new(
            cuemol_mdtools::IndexedTrajectory::from_bytes(&bytes, 1, Some(3)).unwrap(),
        ));
        for source in [owned, vectors, file] {
            let selected = GuiTrajectoryFrames::Selected {
                source: Arc::new(source.clone()),
                indices: Arc::new(vec![2, 0]),
            };
            let nested = GuiTrajectoryFrames::Selected {
                source: Arc::new(selected.clone()),
                indices: Arc::new(vec![1, 0, 1]),
            };
            for frames in [source, selected, nested] {
                for index in 0..2 {
                    let (expected, cell) = frames.frame_with_cell(index).unwrap();
                    let mut molecule = molecule(expected.len());
                    let generation = molecule.atoms_moved_generation();
                    frames.apply_frame(&mut molecule, index, None).unwrap();
                    for (actual, expected) in molecule.atoms().iter().zip(expected) {
                        let actual = actual.position;
                        assert_eq!(
                            [actual.x, actual.y, actual.z, actual.w].map(f64::to_bits),
                            [expected.x, expected.y, expected.z, expected.w].map(f64::to_bits),
                        );
                    }
                    assert_eq!(molecule.atoms_moved_generation(), generation + 1);
                    assert_eq!(molecule.cell_params().is_some(), cell.is_some());
                    if let Some([a, b, c, alpha, beta, gamma]) = cell {
                        assert_eq!(
                            molecule.cell_params(),
                            Some(&cuemol_core::CellParams::new(a, b, c, alpha, beta, gamma))
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn averaged_trajectory_preserves_order_cells_and_nested_selection_across_blocks() {
        let raw = (0..7)
            .map(|frame| {
                (0..4)
                    .map(|atom| {
                        Vector4::new3(
                            ((frame * 4 + atom) as f32 * 0.1) as f64,
                            if frame % 2 == 0 {
                                1.0e12_f32 as f64
                            } else {
                                -1.0e12_f32 as f64
                            },
                            -0.,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let cells = (0..7)
            .map(|frame| (frame < 5).then_some([20. + frame as f64, 25., 30., 90., 80., 75.]))
            .collect::<Vec<_>>();
        let mut blocks = Vec::new();
        for (block_index, (start, end)) in [(0, 3), (3, 5), (5, 7)].into_iter().enumerate() {
            let mut packed = cuemol_mdtools::TrajBlock::new(4);
            for frame in start..end {
                packed
                    .append_frame(
                        raw[frame]
                            .iter()
                            .flat_map(|p| [p.x as f32, p.y as f32, p.z as f32])
                            .collect(),
                        cells[frame],
                    )
                    .unwrap();
            }
            let source = GuiTrajectoryFrames::Owned(Arc::new(packed));
            let source = match block_index {
                0 => source,
                1 => GuiTrajectoryFrames::File(Arc::new(
                    cuemol_mdtools::IndexedTrajectory::from_bytes(
                        &source.dcd_bytes(4).unwrap(),
                        1,
                        Some(4),
                    )
                    .unwrap(),
                )),
                _ => GuiTrajectoryFrames::Memory(Arc::new(raw[start..end].to_vec())),
            };
            let selected = GuiTrajectoryFrames::Selected {
                source: Arc::new(source),
                indices: Arc::new(vec![3, 1, 0]),
            };
            let nested = GuiTrajectoryFrames::Selected {
                source: Arc::new(selected),
                indices: Arc::new(vec![2, 0, 2]),
            };
            blocks.push(GuiTrajectoryBlockData {
                stride: 1,
                uid: block_index as u64 + 1,
                source: String::new(),
                format: "TEST".into(),
                frames: nested,
            });
        }
        for radius in [0, 1, 3, usize::MAX] {
            let mut trajectory = GuiTrajectoryData {
                blocks: blocks.clone(),
                frame_average_size: radius,
                ..Default::default()
            };
            for index in 0..raw.len() {
                let expected = [0, 3, 0].map(|atom| {
                    if radius == 0 {
                        return raw[index][atom];
                    }
                    let start = index.saturating_sub(radius);
                    let end = index.saturating_add(radius).min(raw.len() - 1);
                    let sum =
                        (start..=end).fold(Vector4::ZERO, |sum, frame| sum + raw[frame][atom]);
                    sum.scale(1. / (end - start + 1) as f64)
                });
                let mut molecule = molecule(3);
                let old_cell = cuemol_core::CellParams::new(10., 11., 12., 90., 90., 90.);
                molecule.set_cell_params(old_cell.clone());
                let before = molecule.clone();
                trajectory.apply_frame(&mut molecule, index).unwrap();
                assert_eq!(trajectory.current_frame, index);
                for (atom, expected) in molecule.atoms().iter().zip(expected) {
                    let p = atom.position;
                    assert_eq!(
                        [p.x, p.y, p.z, p.w].map(f64::to_bits),
                        [expected.x, expected.y, expected.z, expected.w].map(f64::to_bits),
                        "radius={radius} index={index}"
                    );
                }
                let expected_cell = cells[index]
                    .map(|[a, b, c, alpha, beta, gamma]| {
                        cuemol_core::CellParams::new(a, b, c, alpha, beta, gamma)
                    })
                    .unwrap_or(old_cell);
                assert_eq!(molecule.cell_params(), Some(&expected_cell));
                assert_eq!(before.atoms()[0].position, Vector4::ZERO);
            }
        }
    }

    #[test]
    fn averaged_trajectory_failure_preserves_the_visible_frame_and_cell() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("changing.dcd");
        let mut block = cuemol_mdtools::TrajBlock::new(1);
        block
            .append_frame(vec![1., 2., 3.], Some([10., 11., 12., 90., 90., 90.]))
            .unwrap();
        block
            .append_frame(vec![4., 5., 6.], Some([20., 21., 22., 90., 90., 90.]))
            .unwrap();
        let bytes = GuiTrajectoryFrames::Owned(Arc::new(block))
            .dcd_bytes(1)
            .unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let frames = GuiTrajectoryFrames::File(Arc::new(
            cuemol_mdtools::IndexedTrajectory::open(&path, 1, Some(1)).unwrap(),
        ));
        let mut trajectory = GuiTrajectoryData {
            blocks: vec![GuiTrajectoryBlockData {
                stride: 1,
                uid: 1,
                source: String::new(),
                format: "DCD".into(),
                frames,
            }],
            ..Default::default()
        };
        let mut molecule = molecule(1);
        trajectory.apply_frame(&mut molecule, 0).unwrap();
        let before = molecule.clone();
        trajectory.frame_average_size = 1;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(bytes.len() as u64 - 4)
            .unwrap();
        for index in [0, 1, 2, usize::MAX] {
            assert!(trajectory.apply_frame(&mut molecule, index).is_err());
            assert_eq!(trajectory.current_frame, 0);
            assert_eq!(molecule, before);
            assert_eq!(
                molecule.atoms_moved_generation(),
                before.atoms_moved_generation()
            );
        }
    }

    #[test]
    fn invalid_direct_frame_does_not_change_coordinates_cell_or_frame_index() {
        let source = GuiTrajectoryFrames::Memory(Arc::new(vec![
            vec![Vector4::new3(1., 2., 3.)],
            vec![Vector4::ZERO, Vector4::ZERO],
        ]));
        let mut trajectory = GuiTrajectoryData {
            blocks: vec![GuiTrajectoryBlockData {
                stride: 1,
                uid: 1,
                source: String::new(),
                format: "SYNTHETIC".into(),
                frames: source.clone(),
            }],
            ..Default::default()
        };
        let mut molecule = molecule(1);
        molecule.set_cell_params(cuemol_core::CellParams::new(10., 11., 12., 90., 90., 90.));
        trajectory.apply_frame(&mut molecule, 0).unwrap();
        let before = molecule.clone();
        for index in [1, 2, usize::MAX] {
            assert!(trajectory.apply_frame(&mut molecule, index).is_err());
            assert_eq!(trajectory.current_frame, 0);
            assert_eq!(molecule.atoms(), before.atoms());
            assert_eq!(molecule.cell_params(), before.cell_params());
            assert_eq!(
                molecule.atoms_moved_generation(),
                before.atoms_moved_generation()
            );
        }
        for selected in [vec![1], vec![0, usize::MAX]] {
            let frames = GuiTrajectoryFrames::Selected {
                source: Arc::new(source.clone()),
                indices: Arc::new(selected),
            };
            assert!(frames.frame_with_cell(0).is_err());
            assert!(frames.apply_frame(&mut molecule, 0, None).is_err());
            assert_eq!(molecule.atoms(), before.atoms());
            assert_eq!(
                molecule.atoms_moved_generation(),
                before.atoms_moved_generation()
            );
        }
    }
}
