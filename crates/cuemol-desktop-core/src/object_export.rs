use super::*;

impl DesktopAppState {
    pub fn workspace_object_export_bytes(
        &self,
        kind: GuiWorkspaceObjectKind,
        object_uid: Uid,
        format: &str,
        selection: Option<&str>,
    ) -> Result<Vec<u8>> {
        let selection = selection.filter(|value| !value.trim().is_empty() && value.trim() != "*");
        if kind != GuiWorkspaceObjectKind::Molecule && selection.is_some() {
            return Err(CueMolError::invalid_argument(
                "only molecules support atom selections",
            ));
        }
        match kind {
            GuiWorkspaceObjectKind::Molecule => {
                let object = self
                    .scene_manager()
                    .object(object_uid)
                    .ok_or_else(|| CueMolError::not_found("molecule"))?;
                let selector = selection
                    .map(|text| self.parse_object_selector(object_uid, text))
                    .transpose()?;
                let molecule = object.mol();
                Ok(match format {
                    "pdb" => {
                        write_pdb_string_with_selection(molecule, selector.as_ref()).into_bytes()
                    }
                    "sdf" => match &selector {
                        Some(selection) => {
                            cuemol_molstr::write_sdf_string_with_selection(molecule, selection)
                        }
                        None => write_sdf_string(molecule),
                    }
                    .into_bytes(),
                    "pqr" => {
                        let mut writer = PqrFileWriter::new();
                        writer.set_selection(selector);
                        writer.write_pqr_string(molecule).into_bytes()
                    }
                    "xyzr" => {
                        let mut writer = XyzrFileWriter::new();
                        writer.set_selection(selector);
                        writer.write_xyzr_string(molecule).into_bytes()
                    }
                    "qdfmol" => {
                        if let Some(selector) = selector {
                            let mut selected = cuemol_molstr::MolCoord::new();
                            selected.copy_selected_atoms_from(molecule, &selector)?;
                            if let Some(cell) = molecule.cell_params() {
                                selected.set_cell_params(cell.clone());
                            }
                            QdfMolFileWriter::new().write_bytes(&selected)
                        } else {
                            QdfMolFileWriter::new().write_bytes(molecule)
                        }
                    }
                    _ => {
                        return Err(CueMolError::invalid_argument(format!(
                            "unsupported molecule writer: {format}"
                        )))
                    }
                })
            }
            GuiWorkspaceObjectKind::DensityMap => {
                let object = self
                    .density_maps
                    .get(&object_uid)
                    .ok_or_else(|| CueMolError::not_found("density map"))?;
                match (format, object.class_name.as_str()) {
                    ("qdfpot", "ElePotMap") => Ok(cuemol_surface::QdfPotWriter::new()
                        .write_bytes(&electrostatic_map_from_density_map(&object.map))),
                    ("qdfmap", class) if class != "ElePotMap" => {
                        Ok(cuemol_xtal::write_qdf_denmap_bytes(&object.map))
                    }
                    _ => Err(CueMolError::invalid_argument(format!(
                        "unsupported map writer: {format}"
                    ))),
                }
            }
            GuiWorkspaceObjectKind::Surface if format == "qdfsurf" => {
                let scene_uid = self
                    .scene_manager()
                    .scene_uid_for_surface(object_uid)
                    .ok_or_else(|| CueMolError::not_found("surface"))?;
                let surface = self
                    .scene_manager()
                    .scene(scene_uid)
                    .and_then(|scene| scene.loaded_surface(object_uid))
                    .and_then(loaded_surface_object)
                    .ok_or_else(|| {
                        CueMolError::parse("surface data could not be converted to qdf")
                    })?;
                Ok(cuemol_surface::qdf::write_qdf_surface_bytes(&surface))
            }
            GuiWorkspaceObjectKind::Lightweight if format == "qdflwobj" => {
                let scene_uid = self
                    .scene_manager()
                    .scene_uid_for_lw_object(object_uid)
                    .ok_or_else(|| CueMolError::not_found("lightweight object"))?;
                let object = self
                    .scene_manager()
                    .scene(scene_uid)
                    .and_then(|scene| scene.loaded_lw_object(object_uid))
                    .ok_or_else(|| CueMolError::not_found("lightweight object"))?;
                cuemol_lwview::read_qdf_lwobj_bytes(&object.raw_data)?;
                Ok(object.raw_data.clone())
            }
            _ => Err(CueMolError::invalid_argument(format!(
                "unsupported object writer: {format}"
            ))),
        }
    }

    pub fn link_workspace_object_export(
        &mut self,
        kind: GuiWorkspaceObjectKind,
        object_uid: Uid,
        source: &str,
        format: &str,
        bytes: Vec<u8>,
    ) -> Result<()> {
        match kind {
            GuiWorkspaceObjectKind::Molecule => {
                let object = self
                    .scene_manager_mut()
                    .object_mut(object_uid)
                    .ok_or_else(|| CueMolError::not_found("molecule"))?;
                object.set_source_type(format);
                object.set_source(source);
                object.set_modified(false);
            }
            GuiWorkspaceObjectKind::DensityMap => {
                let object = self
                    .density_maps
                    .get_mut(&object_uid)
                    .ok_or_else(|| CueMolError::not_found("density map"))?;
                object.format = format.into();
                object.source = source.into();
                object.modified = false;
                let node = density_map_qsc_node(object, format, source);
                let (scene_uid, index) = (object.scene_uid, object.storage_index);
                let loaded = self
                    .scene_manager_mut()
                    .scene_mut(scene_uid)
                    .and_then(|scene| scene.loaded_density_maps_mut().get_mut(index))
                    .ok_or_else(|| CueMolError::not_found("density map storage"))?;
                loaded.srctype = format.into();
                loaded.data.clear();
                loaded.raw_data = bytes.into();
                loaded.qsc_node = Some(node);
            }
            GuiWorkspaceObjectKind::Surface => {
                let scene_uid = self
                    .scene_manager()
                    .scene_uid_for_surface(object_uid)
                    .ok_or_else(|| CueMolError::not_found("surface"))?;
                let object = self
                    .scene_manager_mut()
                    .scene_mut(scene_uid)
                    .and_then(|scene| scene.loaded_surface_mut(object_uid))
                    .ok_or_else(|| CueMolError::not_found("surface"))?;
                object.srctype = format.into();
                object.data.clear();
                object.raw_data = bytes;
                let node = ensure_surface_qsc_node(object);
                node.set_str_attr("src", source);
                node.set_str_attr("srctype", format);
                node.remove_child("modified");
            }
            GuiWorkspaceObjectKind::Lightweight => {
                let scene_uid = self
                    .scene_manager()
                    .scene_uid_for_lw_object(object_uid)
                    .ok_or_else(|| CueMolError::not_found("lightweight object"))?;
                let object = self
                    .scene_manager_mut()
                    .scene_mut(scene_uid)
                    .and_then(|scene| scene.loaded_lw_object_mut(object_uid))
                    .ok_or_else(|| CueMolError::not_found("lightweight object"))?;
                object.srctype = format.into();
                object.raw_data = bytes;
                let node = ensure_lw_qsc_node(object);
                node.set_str_attr("src", source);
                node.set_str_attr("srctype", format);
            }
        }
        Ok(())
    }
}
