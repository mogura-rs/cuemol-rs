use super::*;
use cuemol_molstr::molecule::MoleculeData;
use cuemol_xtal::map_renderers::DensityMapData;
use std::collections::VecDeque;
use std::sync::Arc;

/// Parsed file contents with no application IDs or undo history.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PreparedFile {
    name: String,
    contents: PreparedContents,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum PreparedContents {
    Molecule(MoleculeData),
    Map {
        format: String,
        data: DensityMapData,
    },
    Scene {
        source: Arc<Vec<u8>>,
        molecules: Vec<MoleculeData>,
        maps: Vec<DensityMapData>,
    },
    Pymol(cuemol_importers::PickleValue),
    Surface {
        format: String,
        source: Vec<u8>,
    },
    Lightweight {
        source: Vec<u8>,
        renderer_ids: Vec<i32>,
    },
    Style {
        node: LDom2Node,
        data: String,
    },
}

impl PreparedFile {
    pub fn is_scene(&self) -> bool {
        matches!(
            self.contents,
            PreparedContents::Scene { .. } | PreparedContents::Pymol(_)
        )
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn scene_source(&self) -> Option<Arc<Vec<u8>>> {
        match &self.contents {
            PreparedContents::Scene { source, .. } => Some(source.clone()),
            _ => None,
        }
    }

    pub fn style_name(&self) -> Option<String> {
        match &self.contents {
            PreparedContents::Style { node, .. } => Some(node.str_attr("id").to_string()),
            _ => None,
        }
    }

    /// The transfer protocol is private to matching application builds.
    pub fn encode(&self) -> Result<Vec<u8>> {
        postcard::to_extend(self, b"CMPL\x05".to_vec())
            .map_err(|error| CueMolError::parse(error.to_string()))
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        let data = data
            .strip_prefix(b"CMPL\x05")
            .ok_or_else(|| CueMolError::parse("unsupported prepared-file version"))?;
        let (value, remaining) = postcard::take_from_bytes(data)
            .map_err(|error| CueMolError::parse(error.to_string()))?;
        if !remaining.is_empty() {
            return Err(CueMolError::parse("trailing prepared-file data"));
        }
        Ok(value)
    }

    pub fn parse_with_molecule_options(
        name: &str,
        bytes: &[u8],
        options: &GuiMoleculeReaderOptions,
    ) -> Result<Self> {
        let decoded = if [".gz", ".bz2", ".xz"]
            .iter()
            .any(|suffix| name.to_ascii_lowercase().ends_with(suffix))
        {
            std::borrow::Cow::Owned(cuemol_core::compressed::decode_bytes(bytes)?)
        } else {
            std::borrow::Cow::Borrowed(bytes)
        };
        let text =
            std::str::from_utf8(&decoded).map_err(|error| CueMolError::parse(error.to_string()))?;
        let molecule = match options.kind {
            GuiMoleculeReaderKind::Pdb => cuemol_molstr::read_pdb_str_with_options(
                text,
                cuemol_molstr::PdbReadOptions {
                    load_model: !options.ignore_multiple_models,
                    load_anisou: !options.ignore_anisou,
                    load_alt_conf: !options.ignore_alt_conf,
                    build_secondary: options.calculate_secondary_structure,
                    load_seg_id: options.load_segid_as_chain,
                    auto_topo_gen: options.auto_generate_topology,
                },
            )?,
            GuiMoleculeReaderKind::Mmcif => {
                let mut molecule = cuemol_molstr::read_mmcif_str_with_options(
                    text,
                    cuemol_molstr::MmcifReadOptions {
                        load_model: !options.ignore_multiple_models,
                        load_anisou: !options.ignore_anisou,
                        load_alt_conf: !options.ignore_alt_conf,
                        load_secstr: !options.calculate_secondary_structure,
                        auto_topo_gen: options.auto_generate_topology,
                    },
                )?;
                if options.calculate_secondary_structure {
                    molecule.calc_prot_2ndry(-500., false);
                }
                molecule
            }
        };
        Ok(Self {
            name: name.into(),
            contents: PreparedContents::Molecule(molecule.transfer_data()),
        })
    }

    pub fn parse_with_file_options(
        name: &str,
        bytes: &[u8],
        options: &GuiFileReaderOptions,
        companion: Option<&[u8]>,
    ) -> Result<Self> {
        use GuiFileReaderOptions as Reader;
        if matches!(options, Reader::Automatic) {
            return Self::parse(name, bytes);
        }
        if let Reader::Molecule(options) = options {
            return Self::parse_with_molecule_options(name, bytes, options);
        }
        let decode = |bytes: &[u8]| cuemol_core::compressed::decode_bytes(bytes);
        let decoded;
        let bytes = if bytes.starts_with(&[0x1f, 0x8b])
            || bytes.starts_with(b"BZh")
            || bytes.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0])
        {
            decoded = decode(bytes)?;
            decoded.as_slice()
        } else {
            bytes
        };
        let companion = companion.map(decode).transpose()?;
        fn text(bytes: &[u8]) -> Result<&str> {
            std::str::from_utf8(bytes).map_err(|error| CueMolError::parse(error.to_string()))
        }
        let contents = match options {
            Reader::Ccp4(options) => {
                if !options.min.is_finite()
                    || !options.max.is_finite()
                    || options.subsample == 0
                    || !options.max_voxels.is_finite()
                    || options.max_voxels < 0.
                {
                    return Err(CueMolError::invalid_argument("invalid CCP4 reader options"));
                }
                let mut reader = Ccp4MapReader::new();
                reader.set_normalize(options.normalize);
                reader.set_truncate_min(options.truncate_min);
                reader.set_min(options.min);
                reader.set_truncate_max(options.truncate_max);
                reader.set_max(options.max);
                reader.set_subsample(options.subsample);
                reader.set_max_voxels(options.max_voxels);
                reader.set_use_header_stats(options.use_header_stats);
                let mut map = reader.read_density_map_bytes(bytes)?;
                map.set_map_type(options.map_type.as_str())?;
                PreparedContents::Map {
                    format: "ccp4map".into(),
                    data: map.transfer_data(),
                }
            }
            Reader::Mtz(options) => {
                validate_mtz_map_reader_options(options)?;
                let mut reader = Mtz2MapReader::new();
                reader.set_column_f(&options.column_f);
                reader.set_column_phi(if options.use_phase {
                    &options.column_phi
                } else {
                    ""
                });
                reader.set_column_weight(if options.use_weight {
                    &options.column_weight
                } else {
                    ""
                });
                reader.set_resolution(options.resolution);
                reader.set_grid_size(options.grid_size);
                reader.read_bytes(bytes)?;
                PreparedContents::Map {
                    format: "mtzmap".into(),
                    data: reader.build_density_map()?.transfer_data(),
                }
            }
            Reader::Amber => {
                let topology = text(bytes)?;
                let coordinates = companion.as_deref().map(text).transpose()?;
                let loaded = cuemol_mdtools::read_amber_prmtop_with_restart_str(
                    &topology,
                    coordinates.as_deref(),
                )?;
                PreparedContents::Molecule(loaded.molecule.transfer_data())
            }
            Reader::Namd => {
                let topology = text(companion.as_deref().ok_or_else(|| {
                    CueMolError::invalid_argument("a PSF topology file is required")
                })?)?;
                let loaded = cuemol_mdtools::read_namd_coor_with_psf_bytes(bytes, &topology)?;
                PreparedContents::Molecule(loaded.molecule.transfer_data())
            }
            Reader::Msms { primary_vertices } => {
                let primary = text(bytes)?;
                let companion = text(companion.as_deref().ok_or_else(|| {
                    CueMolError::invalid_argument("both MSMS vertex and face files are required")
                })?)?;
                let (vertices, faces) = if *primary_vertices {
                    (&primary, &companion)
                } else {
                    (&companion, &primary)
                };
                let mut surface = cuemol_surface::MolSurfObj::new();
                *surface.mesh_mut() = cuemol_surface::read_msms_surface_str(vertices, faces)?;
                PreparedContents::Surface {
                    format: "qdfsurf".into(),
                    source: cuemol_surface::qdf::write_qdf_surface_bytes(&surface),
                }
            }
            Reader::Automatic | Reader::Molecule(_) => unreachable!(),
        };
        Ok(Self {
            name: name.into(),
            contents,
        })
    }

    pub fn parse_pdb_download(name: &str, bytes: &[u8], kind: GuiPdbFileKind) -> Result<Self> {
        let lower = name.to_ascii_lowercase();
        let plain_name = lower.strip_suffix(".gz").unwrap_or(&lower);
        if kind == GuiPdbFileKind::Coordinates {
            let format = if plain_name.ends_with(".pdb") {
                "pdb"
            } else {
                "mmcif"
            };
            let options = molecule_reader_options_for_format(format).unwrap();
            return Self::parse_with_molecule_options(name, bytes, &options);
        }
        if kind == GuiPdbFileKind::Emdb {
            return Self::parse(name, bytes);
        }
        let (map, format) = if plain_name.ends_with(".mtz") {
            let mut reader = Mtz2MapReader::new();
            reader.set_grid_size(0.25);
            reader.read_bytes(bytes)?;
            let (amplitude, phase) = if kind == GuiPdbFileKind::FoFc {
                ("DELFWT", "PHDELWT")
            } else {
                ("FWT", "PHWT")
            };
            reader.set_column_f(amplitude);
            reader.set_column_phi(phase);
            reader.select_fft_columns()?;
            (reader.build_density_map()?, "mtzmap")
        } else {
            let decoded = cuemol_core::compressed::decode_bytes(bytes)?;
            let text =
                std::str::from_utf8(&decoded).map_err(|e| CueMolError::parse(e.to_string()))?;
            (density_map_from_cif(text, -1.0, 0.25)?, "mmcifmap")
        };
        Ok(Self {
            name: name.into(),
            contents: PreparedContents::Map {
                format: format.into(),
                data: map.transfer_data(),
            },
        })
    }

    pub fn parse(name: &str, bytes: &[u8]) -> Result<Self> {
        let lower = name.to_ascii_lowercase();
        if [".gz", ".bz2", ".xz"]
            .iter()
            .any(|suffix| lower.ends_with(suffix))
        {
            return Self::parse(
                name.rsplit_once('.').unwrap().0,
                &cuemol_core::compressed::decode_bytes(bytes)?,
            );
        }
        let extension = lower.rsplit('.').next().unwrap_or("pdb");
        let contents = if matches!(extension, "qsc" | "qsl") {
            let mut manager = MolSceneManager::new();
            let uid = manager.new_scene(name);
            let scene = manager.scene_mut(uid).unwrap();
            scene.load_qsc_bytes_with_reader_options(
                bytes,
                None,
                &crate::trajectory_io::read_extra_qsc_molecule_with_options,
            )?;
            let molecules = scene
                .objects()
                .iter()
                .map(|object| object.mol().transfer_data())
                .collect();
            let maps = scene
                .loaded_density_maps()
                .iter()
                .map(|loaded| {
                    let options = loaded.qsc_node.as_ref().and_then(|node| {
                        node.children()
                            .iter()
                            .find(|node| !node.is_attr() && node.tag_name() == "ropts")
                    });
                    Ok(read_density_map_bytes_with_options(
                        &loaded.raw_data,
                        &loaded.srctype,
                        options,
                    )?
                    .transfer_data())
                })
                .collect::<Result<_>>()?;
            PreparedContents::Scene {
                source: Arc::new(bytes.to_vec()),
                molecules,
                maps,
            }
        } else {
            Self::parse_object(extension, bytes)?
        };
        Ok(Self {
            name: name.to_string(),
            contents,
        })
    }

    fn parse_object(extension: &str, bytes: &[u8]) -> Result<PreparedContents> {
        use cuemol_molstr::{ContentVerdict, HandlerSniffer};
        if extension == "xml" {
            let text = std::str::from_utf8(bytes)
                .map_err(|error| CueMolError::parse(error.to_string()))?;
            let (node, data) = style_edit::parse_import(text)?;
            return Ok(PreparedContents::Style { node, data });
        }
        let sniff = |kind| cuemol_molstr::stream::sniff_bytes(bytes, kind) == ContentVerdict::Yes;
        let format = match extension {
            "qdf" => [
                (HandlerSniffer::QdfMol, "qdfmol"),
                (HandlerSniffer::QdfMap, "qdfmap"),
                (HandlerSniffer::QdfSurf, "qdfsurf"),
                (HandlerSniffer::QdfPot, "qdfpot"),
                (HandlerSniffer::QdfLwObj, "qdflwobj"),
            ]
            .into_iter()
            .find_map(|(kind, name)| sniff(kind).then_some(name))
            .ok_or_else(|| CueMolError::parse("unknown or invalid QDF object type"))?,
            "cif" | "mmcif"
                if [
                    "_refln.pdbx_FWT",
                    "_refln.pdbx_DELFWT",
                    "_refln.pdbx_PHWT",
                    "_refln.pdbx_DELPHWT",
                ]
                .iter()
                .any(|tag| byte_index(bytes, tag.as_bytes()).is_some()) =>
            {
                "mmcifmap"
            }
            "map" if sniff(HandlerSniffer::XplorMap) => "xplor",
            other => other,
        };
        if format == "pse" {
            return Ok(PreparedContents::Pymol(cuemol_importers::parse_pickle(
                &cuemol_core::compressed::decode_bytes(bytes)?,
            )?));
        }
        if matches!(format, "obj" | "ply" | "qdfsurf") {
            match format {
                "obj" => {
                    cuemol_surface::read_obj_str(
                        std::str::from_utf8(bytes)
                            .map_err(|error| CueMolError::parse(error.to_string()))?,
                    )?;
                }
                "ply" => {
                    cuemol_surface::read_ply_bytes(bytes)?;
                }
                _ => {
                    cuemol_surface::qdf::read_qdf_surface_bytes(bytes)?;
                }
            }
            return Ok(PreparedContents::Surface {
                format: format.into(),
                source: bytes.to_vec(),
            });
        }
        if format == "qdflwobj" {
            let object = cuemol_lwview::read_qdf_lwobj_bytes(bytes)?;
            let renderer_ids = object
                .draw_elements()
                .iter()
                .map(cuemol_lwview::LWDrawElement::data_id)
                .filter(|id| *id != 0)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            return Ok(PreparedContents::Lightweight {
                source: bytes.to_vec(),
                renderer_ids,
            });
        }
        let map_format = match format {
            "map" | "mrc" | "ccp4" => Some("ccp4map"),
            "dx" => Some("apbs"),
            "xplor" | "cns" => Some("xplormap"),
            "brix" | "omap" => Some("brix"),
            "mtz" => Some("mtzmap"),
            "mmcifmap" | "qdfmap" | "qdfpot" => Some(format),
            _ => None,
        };
        if let Some(format) = map_format {
            return Ok(PreparedContents::Map {
                format: format.into(),
                data: read_density_map_bytes(bytes, format)?.transfer_data(),
            });
        }
        let text =
            || std::str::from_utf8(bytes).map_err(|error| CueMolError::parse(error.to_string()));
        let molecule = match format {
            "pdb" | "ent" => read_pdb_str(text()?)?,
            "cif" | "mmcif" => read_mmcif_str(text()?)?,
            "sdf" | "mol" => read_sdf_str(text()?)?,
            "mol2" => read_mol2_str(text()?)?,
            "gro" => cuemol_mdtools::read_gro_str(text()?)?,
            "psf" => cuemol_mdtools::read_psf_str(text()?)?,
            "prmtop" | "parm7" | "top" => cuemol_mdtools::read_amber_prmtop_str(text()?)?,
            "qdfmol" => read_qdf_mol_bytes(bytes)?,
            "qdfpdb" => cuemol_molstr::read_qdfpdb_str(text()?)?,
            _ => {
                return Err(CueMolError::invalid_argument(format!(
                    "unsupported file format: {extension}"
                )))
            }
        };
        Ok(PreparedContents::Molecule(molecule.transfer_data()))
    }
}

impl DesktopAppState {
    pub fn insert_morph_frame_from_prepared(
        &mut self,
        target: Uid,
        file: PreparedFile,
        insert_before: Option<usize>,
    ) -> Result<GuiMorphSummary> {
        let PreparedFile { name, contents } = file;
        if guess_path_format(Path::new(&name), &self.stream_manager)? != "pdb" {
            return Err(CueMolError::invalid_argument(
                "A morph frame requires a PDB file.",
            ));
        }
        let PreparedContents::Molecule(data) = contents else {
            return Err(CueMolError::invalid_argument(
                "A morph frame requires a molecule.",
            ));
        };
        let molecule = data.into_molecule()?;
        self.insert_morph_frame(
            target,
            molecule,
            name.clone(),
            name,
            "pdb".to_string(),
            insert_before,
        )
    }

    pub fn open_prepared(&mut self, file: PreparedFile) -> Result<GuiLoadedPath> {
        let reusable =
            matches!(file.contents, PreparedContents::Pymol(_)).then(|| self.ensure_active_scene());
        self.open_prepared_in_scene(file, reusable)
    }

    pub(crate) fn open_prepared_in_scene(
        &mut self,
        file: PreparedFile,
        reusable: Option<Uid>,
    ) -> Result<GuiLoadedPath> {
        let PreparedFile { name, contents } = file;
        match contents {
            PreparedContents::Style { node, data } => {
                let scene_uid = self.ensure_active_scene();
                self.insert_imported_style(scene_uid, node, data, true)?;
                Ok(GuiLoadedPath::Scene { scene_uid })
            }
            PreparedContents::Molecule(data) => {
                let molecule = data.into_molecule()?;
                let scene_uid = self.ensure_active_scene();
                let object_uid = self
                    .scene_manager_mut()
                    .scene_mut(scene_uid)
                    .unwrap()
                    .add_molecule(name, molecule);
                self.create_default_molecule_renderers(scene_uid, object_uid)?;
                Ok(GuiLoadedPath::Object {
                    scene_uid,
                    object_uid,
                })
            }
            PreparedContents::Map { format, data } => {
                let map = data.into_map()?;
                let scene_uid = self.ensure_active_scene();
                let map_uid = self.add_density_map(scene_uid, &name, map);
                self.density_maps.get_mut(&map_uid).unwrap().format = format;
                self.add_density_renderer(map_uid, "contour")?;
                Ok(GuiLoadedPath::Map { scene_uid, map_uid })
            }
            PreparedContents::Scene {
                source,
                molecules,
                maps,
            } => {
                let molecules = std::cell::RefCell::new(
                    molecules
                        .into_iter()
                        .map(MoleculeData::into_molecule)
                        .collect::<Result<VecDeque<_>>>()?,
                );
                let maps = maps
                    .into_iter()
                    .map(DensityMapData::into_map)
                    .collect::<Result<VecDeque<_>>>()?;
                let previous = self.active_scene();
                let scene_uid = reusable.unwrap_or_else(|| self.new_scene(name, true).result_scene);
                let result = (|| {
                    self.scene_manager_mut()
                        .scene_mut(scene_uid)
                        .unwrap()
                        .load_qsc_bytes_with_molecule_decoder(&source, None, &|_, _, _, _| {
                            molecules.borrow_mut().pop_front().ok_or_else(|| {
                                CueMolError::parse("missing prepared scene molecule")
                            })
                        })?;
                    if !molecules.borrow().is_empty() {
                        return Err(CueMolError::parse("unexpected prepared scene molecule"));
                    }
                    self.restore_loaded_scene_data_with_maps(scene_uid, Some(maps))?;
                    let end =
                        byte_index(&source, QSC_XML_END_MARKER.as_bytes()).unwrap_or(source.len());
                    let xml = std::str::from_utf8(&source[..end])
                        .map_err(|error| CueMolError::parse(error.to_string()))?;
                    let tree = parse_ldom2_xml(xml)?;
                    self.restore_named_qsc_camera_visibility(scene_uid, tree.top())?;
                    if let Some(camera) = self.active_scene_saved_camera() {
                        self.apply_scene_visibility_settings(
                            scene_uid,
                            camera.visibility_settings(),
                        )?;
                        self.set_active_view_camera(&camera)?;
                    }
                    Ok(GuiLoadedPath::Scene { scene_uid })
                })();
                if result.is_err() {
                    self.destroy_scene(scene_uid);
                    self.set_active_scene(previous);
                }
                result
            }
            PreparedContents::Pymol(tree) => {
                let scene_uid =
                    reusable.unwrap_or_else(|| self.new_scene(&name, true).result_scene);
                self.load_pse_tree(scene_uid, &tree, &name)?;
                if let Some(camera) = self.active_scene_saved_camera() {
                    self.set_active_view_camera(&camera)?;
                }
                Ok(GuiLoadedPath::Scene { scene_uid })
            }
            PreparedContents::Surface { format, source } => {
                let scene_uid = self.ensure_active_scene();
                let (data, raw_data) = LoadedSurface::source_payload(&format, source);
                let mut surface = LoadedSurface {
                    uid: Uid::new(0),
                    name,
                    object_type: "MolSurfObj".into(),
                    data,
                    srctype: format,
                    raw_data,
                    qsc_node: None,
                };
                ensure_surface_qsc_node(&mut surface);
                self.scene_manager_mut()
                    .scene_mut(scene_uid)
                    .unwrap()
                    .add_loaded_surface(surface);
                Ok(GuiLoadedPath::Scene { scene_uid })
            }
            PreparedContents::Lightweight {
                source,
                renderer_ids,
            } => {
                let scene_uid = self.ensure_active_scene();
                let mut object = LoadedLWObject {
                    uid: Uid::new(0),
                    name,
                    srctype: "qdflwobj".into(),
                    raw_data: source.into(),
                    visible: true,
                    xform: Matrix4::IDENTITY,
                    qsc_node: None,
                    renderers: renderer_ids
                        .into_iter()
                        .map(|data_id| LoadedLWRenderer {
                            name: format!("lwrend_{data_id}"),
                            data_id,
                            visible: true,
                            xform: Matrix4::IDENTITY,
                        })
                        .collect(),
                };
                ensure_lw_qsc_node(&mut object);
                self.scene_manager_mut()
                    .scene_mut(scene_uid)
                    .unwrap()
                    .add_loaded_lw_object(object);
                Ok(GuiLoadedPath::Scene { scene_uid })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PDB: &[u8] = b"ATOM     17  CA  GLY A   1       1.000   2.000   3.000  0.75 12.00           C  \nATOM     93  C   GLY A   1       2.400   2.000   3.000  1.00 10.00           C  \nEND\n";

    #[test]
    fn prepared_morph_preserves_base_ids_and_renderers_without_loading_objects() {
        let mut app = DesktopAppState::new();
        app.open_bytes("target.pdb", PDB).unwrap();
        let target = app.active_object_summaries()[0].uid;
        let object = app.scene_manager().object(target).unwrap();
        let renderers = object
            .renderers()
            .iter()
            .map(|r| r.uid())
            .collect::<Vec<_>>();
        let atoms = object
            .mol()
            .atoms()
            .iter()
            .map(|a| (a.serial, a.position))
            .collect::<Vec<_>>();
        let base = app.clone();
        let moved = std::str::from_utf8(PDB)
            .unwrap()
            .replace("1.000", "9.000")
            .replace("2.400", "8.400");
        let prepared = PreparedFile::parse("other.pdb", moved.as_bytes()).unwrap();
        let prepared = PreparedFile::decode(&prepared.encode().unwrap()).unwrap();
        let result = app
            .insert_morph_frame_from_prepared(target, prepared, None)
            .unwrap();
        assert_eq!(result.frame_count, 2);
        assert_eq!(app.active_object_summaries().len(), 1);
        assert_eq!(
            app.scene_manager().object(target).unwrap().object_type(),
            "MorphMol"
        );
        assert_eq!(
            app.scene_manager()
                .object(target)
                .unwrap()
                .renderers()
                .iter()
                .map(|r| r.uid())
                .collect::<Vec<_>>(),
            renderers
        );
        assert_eq!(
            app.scene_manager()
                .object(target)
                .unwrap()
                .mol()
                .atoms()
                .iter()
                .map(|a| (a.serial, a.position))
                .collect::<Vec<_>>(),
            atoms
        );
        let frames = app.morph_frame_summaries(target);
        assert_eq!(frames[0].name, "(this)");
        assert_eq!(frames[0].source_type, "<this>");
        assert_eq!(frames[1].source, "other.pdb");
        assert!(!app.initialize_morph_object(target).unwrap().1);
        assert!(app.remove_morph_frame(target, 0).is_err());
        app.apply_morph_frame(target, 1.).unwrap();
        assert_ne!(
            app.scene_manager().object(target).unwrap().mol().atoms()[0].position,
            atoms[0].1
        );
        app.apply_morph_frame(target, 0.).unwrap();
        assert_eq!(
            app.scene_manager()
                .object(target)
                .unwrap()
                .mol()
                .atoms()
                .iter()
                .map(|a| (a.serial, a.position))
                .collect::<Vec<_>>(),
            atoms
        );
        app.restore_snapshot(base.clone());
        assert!(base.same_undo_state(&app));
        assert!(app.morph_frame_value(target).is_none());
        assert_eq!(
            app.scene_manager().object(target).unwrap().object_type(),
            "MolCoord"
        );

        let invalid = PreparedFile {
            name: "other.cif".into(),
            contents: PreparedContents::Molecule(
                read_pdb_str(std::str::from_utf8(PDB).unwrap())
                    .unwrap()
                    .transfer_data(),
            ),
        };
        assert!(app
            .insert_morph_frame_from_prepared(target, invalid, None)
            .is_err());
        assert!(base.same_undo_state(&app));
        app.open_bytes("source.pdb", moved.as_bytes()).unwrap();
        let source = app.active_object_summaries()[1].uid;
        app.insert_morph_frame_from_object(target, source, "", None)
            .unwrap();
        assert_eq!(
            app.morph_frame_summaries(target)[1].name,
            app.scene_manager().object(source).unwrap().name()
        );
        assert_eq!(app.morph_frame_summaries(target)[0].name, "(this)");
    }

    #[test]
    fn transfer_keeps_applied_custom_topology_pivots_and_links() {
        let mut source = std::str::from_utf8(PDB).unwrap().replace("END\n", "");
        source.push_str("ATOM     94  N   GLY A   2       3.700   2.000   3.000  1.00 10.00           N\nATOM     95  CA  GLY A   2       5.100   2.000   3.000  1.00 10.00           C\nEND\n");
        let mut molecule = read_pdb_str(&source).unwrap();
        let mut topology = cuemol_molstr::TopoDB::global().clone();
        let mut residue = topology.residue("GLY").unwrap().clone();
        residue.pivot_atom = "C".into();
        topology.put(residue);
        molecule.apply_topology_with_db(&topology);
        let prepared = PreparedFile {
            name: "custom.pdb".into(),
            contents: PreparedContents::Molecule(molecule.transfer_data()),
        };
        let encoded = prepared.encode().unwrap();
        let restored = PreparedFile::decode(&encoded).unwrap();
        let PreparedContents::Molecule(data) = restored.contents else {
            panic!("molecule expected")
        };
        let molecule = data.into_molecule().unwrap();
        assert_eq!(
            molecule.residue_pivot_pos("A", "1").unwrap().unwrap().x,
            2.4
        );
        assert_eq!(
            molecule.topology_linked(&molecule.atoms()[0], &molecule.atoms()[2]),
            Some(true)
        );
        let mut incompatible = encoded;
        incompatible[4] = 2;
        assert!(PreparedFile::decode(&incompatible).is_err());
    }

    #[test]
    fn transfer_preserves_molecular_precision_ids_properties_and_bonds() {
        let mut molecule = read_pdb_str(std::str::from_utf8(PDB).unwrap()).unwrap();
        molecule.atoms_mut()[0].position = Vector4::new4(1.2345678901234567, -0.0, 1e-125, 3.5);
        molecule.atoms_mut()[0].set_int_prop("isotope", 13);
        molecule.atoms_mut()[0].set_real_prop("charge", -0.12345678901234567);
        molecule.atoms_mut()[0].formal_charge = Some(-1);
        molecule.atoms_mut()[0].alt_loc = Some('B');
        molecule.atoms_mut()[0].ins_code = Some('A');
        molecule.set_all_residue_props(BTreeMap::from([(
            cuemol_molstr::ResidueKey::new("A", 1, Some('A')),
            BTreeMap::from([("secondary".into(), "H".into())]),
        )]));
        molecule.remove_bond_between(0, 1);
        molecule.add_bond_stereo(0, 1, cuemol_molstr::BondOrder::Aromatic, 6);
        molecule.mark_bond_persistent(0, 1);
        molecule.set_cell_params(cuemol_core::CellParams {
            space_group_name: "P 21".into(),
            space_group_number: Some(4),
            ..cuemol_core::CellParams::new(10., 20., 30., 90., 110., 90.)
        });
        molecule.set_selection_text("name CA").unwrap();
        let mut color = cuemol_gfx::SolidColor::create_hsb(0.2, 0.6, 0.8, 0.7);
        color.set_material("test");
        molecule.set_default_color(color);
        molecule.fire_atoms_moved();
        let prepared = PreparedFile {
            name: "precise.mmcif".into(),
            contents: PreparedContents::Molecule(molecule.transfer_data()),
        };
        let restored = PreparedFile::decode(&prepared.encode().unwrap()).unwrap();
        let PreparedContents::Molecule(data) = restored.contents else {
            panic!("molecule expected")
        };
        let restored = data.into_molecule().unwrap();
        assert_eq!(restored, molecule);
        assert_eq!(
            restored.atoms()[0].position.y.to_bits(),
            (-0.0_f64).to_bits()
        );
    }

    #[test]
    fn binary_transfer_preserves_nonfinite_property_bits() {
        let mut molecule = read_pdb_str(std::str::from_utf8(PDB).unwrap()).unwrap();
        let bits = 0x7ff8_0000_0000_0042;
        molecule.atoms_mut()[0].set_real_prop("payload", f64::from_bits(bits));
        let prepared = PreparedFile {
            name: "properties.pdb".into(),
            contents: PreparedContents::Molecule(molecule.transfer_data()),
        };
        let PreparedContents::Molecule(data) = PreparedFile::decode(&prepared.encode().unwrap())
            .unwrap()
            .contents
        else {
            panic!("molecule expected")
        };
        assert_eq!(
            data.into_molecule().unwrap().atoms()[0]
                .real_prop("payload")
                .unwrap()
                .to_bits(),
            bits
        );
    }

    #[test]
    fn transfer_preserves_continuous_map_storage_and_statistics() {
        let mut map = DensityMap::from_continuous_grid(cuemol_molstr::GridMap {
            counts: [2, 2, 2],
            origin: Vector4::new3(1.123456789, 2., -3.),
            delta: [
                Vector4::new3(1., 0., 0.),
                Vector4::new3(0., 2., 0.),
                Vector4::new3(0., 0., 3.),
            ],
            values: vec![-1e-12, 1e6, 0.125, 2.5, 8., 16., -0.75, 0.],
        });
        map.set_map_type("em").unwrap();
        let prepared = PreparedFile {
            name: "potential.dx".into(),
            contents: PreparedContents::Map {
                format: "apbs".into(),
                data: map.transfer_data(),
            },
        };
        let PreparedContents::Map { data, .. } = PreparedFile::decode(&prepared.encode().unwrap())
            .unwrap()
            .contents
        else {
            panic!("map expected")
        };
        assert_eq!(data.into_map().unwrap(), map);
    }

    #[test]
    fn prepared_scene_keeps_reader_options_and_uses_current_application_ids() {
        let ligand = std::str::from_utf8(PDB).unwrap().replace("GLY", "LIG");
        assert_eq!(read_pdb_str(&ligand).unwrap().bond_count(), 1);
        let source = format!("<scene bgcolor=\"#3366b3\"><object name=\"molecule\" type=\"MolCoord\" src=\"datachunk:00000\" srctype=\"pdb\"><ropts autoTopoGen=\"false\"/></object><camera name=\"__current\" zoom=\"23\"/></scene>\n\n{QSC_XML_END_MARKER}\n==========START datachunk:00000==========\n{ligand}\n==========END datachunk:00000==========\n");
        let prepared = PreparedFile::decode(
            &PreparedFile::parse("scene.qsc", source.as_bytes())
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap();
        let mut state = DesktopAppState::new();
        state.open_bytes("first.pdb", PDB).unwrap();
        let first = state.active_scene().unwrap();
        let first_object = state.active_object_summaries()[0].uid;
        let GuiLoadedPath::Scene { scene_uid } = state.open_prepared(prepared).unwrap() else {
            panic!("scene expected")
        };
        assert_ne!(first, scene_uid);
        let object = &state.scene_manager().scene(scene_uid).unwrap().objects()[0];
        assert_ne!(object.uid(), first_object);
        assert_eq!(object.mol().atoms()[0].serial, 17);
        assert_eq!(object.mol().atoms()[1].serial, 93);
        assert_eq!(object.mol().bond_count(), 0);
        assert_eq!(state.active_view_camera().unwrap().zoom(), 23.);
    }

    #[test]
    fn transfer_keeps_generated_ligand_topology_for_reapplication() {
        let mut source = std::str::from_utf8(PDB)
            .unwrap()
            .replace("GLY", "XYZ")
            .replace("END\n", "");
        source.push_str("HETATM   94  CA  XYZ A   2      10.000   2.000   3.000  1.00 10.00           C\nHETATM   95  C   XYZ A   2      14.000   2.000   3.000  1.00 10.00           C\nEND\n");
        let prepared = PreparedFile::parse("ligand.pdb", source.as_bytes()).unwrap();
        let PreparedContents::Molecule(data) = PreparedFile::decode(&prepared.encode().unwrap())
            .unwrap()
            .contents
        else {
            panic!("molecule expected")
        };
        let mut molecule = data.into_molecule().unwrap();
        assert_eq!(molecule.bond_count(), 2);
        molecule.atoms_mut()[1].position.x = 6.;
        molecule.apply_topology();
        assert_eq!(molecule.bond_count(), 2);
    }

    #[test]
    fn prepared_qdfpdb_keeps_deposited_secondary_structure_unless_overridden() {
        let pdb = "HELIX    1  AA SER A    1  GLY A    2  1                                   2    \nATOM      1  CA  SER A   1       0.000   0.000   0.000  1.00 10.00           C\nATOM      2  C   SER A   1       1.500   0.000   0.000  1.00 10.00           C\nATOM      3  N   GLY A   2       2.800   0.000   0.000  1.00 10.00           N\nATOM      4  CA  GLY A   2       3.800   0.000   0.000  1.00 10.00           C\nEND\n";
        for (format, options, expected) in [
            ("qdfpdb", "", Some("helix")),
            ("qdfpdb", "<ropts build2ndry=\"true\"/>", None),
            ("pdb", "", None),
            ("pdb", "<ropts build2ndry=\"false\"/>", Some("helix")),
        ] {
            let source = format!("<scene><object type=\"MolCoord\" name=\"molecule\" src=\"datachunk:00000\" srctype=\"{format}\">{options}</object></scene>\n========== End of XML ==========\n\n==========START datachunk:00000==========\n{pdb}\n==========END datachunk:00000==========\n");
            let prepared = PreparedFile::parse("secondary.qsc", source.as_bytes()).unwrap();
            let mut state = DesktopAppState::new();
            let prepared = PreparedFile::decode(&prepared.encode().unwrap()).unwrap();
            state.open_prepared(prepared).unwrap();
            let object = state
                .scene_manager()
                .scene(state.active_scene().unwrap())
                .unwrap()
                .objects()
                .first()
                .unwrap();
            assert_eq!(
                object
                    .mol()
                    .get_residue_prop_str("A", "1", "secondary")
                    .unwrap()
                    .as_deref(),
                expected,
                "{format} {options}"
            );
        }
    }

    #[test]
    fn invalid_transfers_are_rejected_before_application_changes() {
        let prepared = PreparedFile::parse("valid.pdb", PDB)
            .unwrap()
            .encode()
            .unwrap();
        for end in [0, 4, prepared.len() / 2, prepared.len() - 1] {
            assert!(PreparedFile::decode(&prepared[..end]).is_err());
        }
        let mut trailing = prepared;
        trailing.push(0);
        assert!(PreparedFile::decode(&trailing).is_err());
        let mut state = DesktopAppState::new();
        state.open_bytes("first.pdb", PDB).unwrap();
        let before = state.clone();
        assert!(state.open_bytes("broken.pdb", b"invalid").is_err());
        assert_eq!(
            state.scene_manager().scenes(),
            before.scene_manager().scenes()
        );
        assert!(state.same_undo_state(&before));
    }

    #[test]
    fn malformed_maps_preserve_existing_scenes_and_undo_state() {
        let dx = b"object 1 class gridpositions counts 2 2 2\norigin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 3 class array type double rank 0 items 7 data follows\n0 1 2 3 4 5 6\n".to_vec();
        let xplor = format!(
            "0\n1 {} {} 1 0 0 1 0 0\n1 1 1 90 90 90\nZYX\n",
            i32::MIN,
            i32::MAX
        )
        .into_bytes();
        let mut brix = format!(
            ":-) origin {} 0 0 extent 1 1 1 grid 1 1 1 cell 1 1 1 90 90 90 prod 1 plus 0 sigma 1",
            i32::MAX
        )
        .into_bytes();
        brix.resize(512, b' ');
        let mut state = DesktopAppState::new();
        state.open_bytes("existing.pdb", PDB).unwrap();
        let before = state.clone();
        for (name, bytes) in [
            ("broken.dx", dx),
            ("broken.xplor", xplor),
            ("broken.brix", brix),
        ] {
            for _ in 0..3 {
                assert!(PreparedFile::parse(name, &bytes).is_err(), "{name}");
                assert!(state.open_bytes(name, &bytes).is_err(), "{name}");
                assert_eq!(state.active_scene(), before.active_scene());
                assert_eq!(
                    state.scene_manager().scenes(),
                    before.scene_manager().scenes()
                );
                assert!(state.same_undo_state(&before));
            }
        }
    }
}
