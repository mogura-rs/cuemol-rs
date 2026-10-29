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
}

impl PreparedFile {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn scene_source(&self) -> Option<Arc<Vec<u8>>> {
        match &self.contents {
            PreparedContents::Scene { source, .. } => Some(source.clone()),
            _ => None,
        }
    }

    /// The transfer protocol is private to matching application builds.
    pub fn encode(&self) -> Result<Vec<u8>> {
        postcard::to_extend(self, b"CMPL\x01".to_vec())
            .map_err(|error| CueMolError::parse(error.to_string()))
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        let data = data
            .strip_prefix(b"CMPL\x01")
            .ok_or_else(|| CueMolError::parse("unsupported prepared-file version"))?;
        let (value, remaining) = postcard::take_from_bytes(data)
            .map_err(|error| CueMolError::parse(error.to_string()))?;
        if !remaining.is_empty() {
            return Err(CueMolError::parse("trailing prepared-file data"));
        }
        Ok(value)
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
    pub fn open_prepared(&mut self, file: PreparedFile) -> Result<GuiLoadedPath> {
        let PreparedFile { name, contents } = file;
        match contents {
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
                let scene_uid = self.new_scene(name, true).result_scene;
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
                let scene_uid = self.ensure_active_scene();
                self.load_pse_tree(scene_uid, &tree, &name)?;
                if let Some(camera) = self.active_scene_saved_camera() {
                    self.set_active_view_camera(&camera)?;
                }
                Ok(GuiLoadedPath::Scene { scene_uid })
            }
            PreparedContents::Surface { format, source } => {
                let scene_uid = self.ensure_active_scene();
                let mut surface = LoadedSurface {
                    uid: Uid::new(0),
                    name,
                    object_type: "MolSurfObj".into(),
                    data: if format == "obj" {
                        String::from_utf8_lossy(&source).into_owned()
                    } else {
                        String::new()
                    },
                    srctype: format,
                    raw_data: source,
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
                    raw_data: source,
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
}
