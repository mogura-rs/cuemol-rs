use super::*;

impl DesktopAppState {
    pub fn configure_loaded_bytes(
        &mut self,
        loaded: &GuiLoadedPath,
        setup: &GuiRendererSetup,
        map_center: Option<Vector4>,
    ) -> Result<()> {
        match *loaded {
            GuiLoadedPath::Object {
                scene_uid,
                object_uid,
            } => {
                self.set_object_common_properties(
                    object_uid,
                    setup.object_name.trim(),
                    true,
                    false,
                )?;
                self.configure_molecule_renderer(scene_uid, object_uid, setup)
            }
            GuiLoadedPath::Map { map_uid, .. } => {
                self.rename_density_map(map_uid, &setup.object_name)?;
                let renderers = self.density_map_summary(map_uid).unwrap().renderers;
                for renderer in renderers {
                    self.destroy_density_renderer(map_uid, renderer.uid)?;
                }
                let uid = self.add_density_renderer(map_uid, &setup.renderer_type)?;
                let map = self.density_maps.get_mut(&map_uid).unwrap();
                let center = map.map.fit_view_center_and_radius().0;
                let renderer = map.renderers.iter_mut().find(|r| r.uid == uid).unwrap();
                renderer.name = setup.renderer_name.clone();
                renderer.center = map_center.unwrap_or(center);
                Ok(())
            }
            GuiLoadedPath::Scene { .. } => Ok(()),
        }
    }

    /// Load a browser file or an in-memory download through the native readers.
    pub fn open_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<GuiLoadedPath> {
        if [".gz", ".bz2", ".xz"]
            .iter()
            .any(|suffix| name.to_ascii_lowercase().ends_with(suffix))
        {
            let base = name.rsplit_once('.').map(|(base, _)| base).unwrap_or(name);
            return self.open_bytes(base, &cuemol_core::compressed::decode_bytes(bytes)?);
        }
        let extension = name
            .rsplit('.')
            .next()
            .unwrap_or("pdb")
            .to_ascii_lowercase();
        if matches!(extension.as_str(), "qsc" | "qsl") {
            let previous = self.active_scene();
            let scene_uid = self.new_scene(name, true).result_scene;
            let result = (|| {
                self.scene_manager_mut()
                    .scene_mut(scene_uid)
                    .unwrap()
                    .load_qsc_bytes_with_molecule_reader(
                        bytes,
                        None,
                        &crate::trajectory_io::read_extra_qsc_molecule,
                    )?;
                self.restore_loaded_scene_data(scene_uid)?;
                let end = byte_index(bytes, QSC_XML_END_MARKER.as_bytes()).unwrap_or(bytes.len());
                let xml = std::str::from_utf8(&bytes[..end])
                    .map_err(|error| CueMolError::parse(error.to_string()))?;
                let tree = parse_ldom2_xml(xml)?;
                self.restore_named_qsc_camera_visibility(scene_uid, tree.top())?;
                if let Some(camera) = self.active_scene_saved_camera() {
                    self.apply_scene_visibility_settings(scene_uid, camera.visibility_settings())?;
                    self.set_active_view_camera(&camera)?;
                }
                Ok(GuiLoadedPath::Scene { scene_uid })
            })();
            if result.is_err() {
                self.destroy_scene(scene_uid);
                self.set_active_scene(previous);
            }
            return result;
        }
        let map_format = match extension.as_str() {
            "map" | "mrc" | "ccp4" => Some("ccp4map"),
            "dx" => Some("apbs"),
            "xplor" => Some("xplormap"),
            "brix" | "omap" => Some("brix"),
            "mtz" => Some("mtzmap"),
            _ => None,
        };
        if let Some(format) = map_format {
            let map = read_density_map_bytes(bytes, format)?;
            let scene_uid = self.ensure_active_scene();
            let map_uid = self.add_density_map(scene_uid, name, map);
            self.density_maps.get_mut(&map_uid).unwrap().format = format.to_string();
            self.add_density_renderer(map_uid, "contour")?;
            return Ok(GuiLoadedPath::Map { scene_uid, map_uid });
        }
        let text = || std::str::from_utf8(bytes).map_err(|e| CueMolError::parse(e.to_string()));
        let mol = match extension.as_str() {
            "pdb" | "ent" => read_pdb_str(text()?)?,
            "cif" | "mmcif" => read_mmcif_str(text()?)?,
            "sdf" | "mol" => read_sdf_str(text()?)?,
            "mol2" => read_mol2_str(text()?)?,
            "gro" => cuemol_mdtools::read_gro_str(text()?)?,
            "psf" => cuemol_mdtools::read_psf_str(text()?)?,
            "prmtop" | "parm7" | "top" => cuemol_mdtools::read_amber_prmtop_str(text()?)?,
            "qdf" | "qdfmol" => read_qdf_mol_bytes(bytes)?,
            _ => {
                return Err(CueMolError::invalid_argument(format!(
                    "unsupported file format: {extension}"
                )))
            }
        };
        let scene_uid = self.ensure_active_scene();
        let object_uid = self
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule(name, mol);
        self.create_default_molecule_renderers(scene_uid, object_uid)?;
        Ok(GuiLoadedPath::Object {
            scene_uid,
            object_uid,
        })
    }

    /// Embed all objects so downloaded scenes do not depend on local paths.
    pub fn active_scene_bytes(&mut self) -> Result<Vec<u8>> {
        let scene_uid = self
            .active_scene()
            .ok_or_else(|| CueMolError::not_found("scene"))?;
        self.sync_trajectory_qsc_nodes(scene_uid, true, None)?;
        self.sync_morph_qsc_nodes(scene_uid)?;
        self.sync_density_map_qsc_nodes(scene_uid)?;
        let (document, _) = self.scene_qsc_with_save_options(
            scene_uid,
            GuiSceneSaveOptions {
                embed_all: true,
                ..Default::default()
            },
        )?;
        Ok(document)
    }

    /// Split an embedded scene around an external trajectory payload for Blob-based export.
    pub fn active_scene_bytes_with_external_trajectory(
        &mut self,
        object_uid: Uid,
        source_type: &str,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let mut parts = self
            .active_scene_parts_with_external_trajectories(&[(object_uid, source_type)])?
            .into_iter();
        Ok((parts.next().unwrap(), parts.next().unwrap()))
    }

    /// Interleave these fragments with the supplied payloads to export without copying their bytes.
    /// Existing embedded blocks precede the additional payloads for each molecule.
    pub fn active_scene_parts_with_external_trajectories(
        &mut self,
        sources: &[(Uid, &str)],
    ) -> Result<Vec<Vec<u8>>> {
        let scene = self
            .active_scene()
            .and_then(|sid| self.scene_manager().scene(sid))
            .ok_or_else(|| CueMolError::not_found("scene"))?;
        let indices = sources
            .iter()
            .map(|(uid, source)| {
                if !matches!(
                    *source,
                    "dcdtraj" | "xtctraj" | "trrtraj" | "ambnetcdftraj" | "mdcrd"
                ) {
                    return Err(CueMolError::invalid_argument(
                        "unsupported trajectory source",
                    ));
                }
                scene
                    .objects()
                    .iter()
                    .position(|obj| obj.uid() == *uid)
                    .ok_or_else(|| CueMolError::not_found("trajectory topology"))
            })
            .collect::<Result<Vec<_>>>()?;
        let bytes = self.active_scene_bytes()?;
        if sources.is_empty() {
            return Ok(vec![bytes]);
        }
        let (xml, suffix) = split_qsc_document(&bytes)?;
        let mut tree = parse_ldom2_xml(xml)?;
        let mut available = (0..100_000)
            .rev()
            .map(|index| format!("{index:05}"))
            .filter(|id| {
                byte_index(
                    suffix,
                    format!("{QSC_DATA_CHUNK_START}{id}==========").as_bytes(),
                )
                .is_none()
            });
        let mut chunks = Vec::with_capacity(sources.len());
        for (index, (_, source)) in indices.into_iter().zip(sources) {
            let chunk = available.next().ok_or_else(|| {
                CueMolError::invalid_argument("too many embedded scene data chunks")
            })?;
            let node = tree
                .top_mut()
                .children_mut()
                .iter_mut()
                .filter(|node| !node.is_attr() && node.tag_name() == "object")
                .nth(index)
                .ok_or_else(|| CueMolError::not_found("trajectory topology node"))?;
            node.set_type_name("Trajectory");
            if node.find_child("trajfiles").is_none() {
                node.append_child("trajfiles");
            }
            let file = node
                .find_child_mut("trajfiles")
                .unwrap()
                .append_child("trajfile");
            file.set_type_name("TrajBlock");
            file.set_str_attr("src", format!("datachunk:{chunk}"));
            file.set_str_attr("srctype", *source);
            chunks.push(chunk);
        }
        let mut parts = Vec::with_capacity(chunks.len() + 1);
        let mut fragment =
            format!("{}\n\n{}", write_ldom2_xml(&tree), QSC_XML_END_MARKER).into_bytes();
        fragment.extend_from_slice(suffix);
        for chunk in chunks {
            fragment.extend_from_slice(
                format!("==========START datachunk:{chunk}==========\n").as_bytes(),
            );
            parts.push(fragment);
            fragment = format!("\n==========END datachunk:{chunk}==========\n").into_bytes();
        }
        parts.push(fragment);
        Ok(parts)
    }

    pub fn apply_coordinate_frame(
        &mut self,
        object_uid: Uid,
        frame: &cuemol_mdtools::TrajectoryFrame,
    ) -> Result<()> {
        let object = self
            .scene_manager_mut()
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::not_found("molecule"))?;
        if frame.coordinates.len() != object.mol().atom_count() * 3 {
            return Err(CueMolError::parse(
                "trajectory atom count does not match topology",
            ));
        }
        for (atom, p) in object
            .mol_mut()
            .atoms_mut()
            .iter_mut()
            .zip(frame.coordinates.chunks_exact(3))
        {
            atom.position = Vector4::new3(p[0] as f64, p[1] as f64, p[2] as f64);
        }
        if let Some([a, b, c, alpha, beta, gamma]) = frame.cell {
            object
                .mol_mut()
                .set_cell_params(cuemol_core::CellParams::new(a, b, c, alpha, beta, gamma));
        }
        object.mol_mut().fire_atoms_moved();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PDB: &[u8] = b"HETATM    1  C1  LIG A   1       1.000   2.000   3.000  1.00 10.00           C  \nHETATM    2  O1  LIG A   1       2.300   2.000   3.000  1.00 10.00           O  \nEND\n";
    #[test]
    fn browser_export_preserves_existing_and_multiple_external_trajectory_blocks() {
        let mut state = DesktopAppState::new();
        state.open_bytes("first.pdb", PDB).unwrap();
        state.open_bytes("second.pdb", PDB).unwrap();
        let objects = state.active_object_summaries();
        let (first, second) = (objects[0].uid, objects[1].uid);
        let frame = |x| vec![Vector4::new3(x, 0., 0.), Vector4::new3(x + 1., 0., 0.)];
        state.set_object_trajectory(first, vec![frame(1.)]);
        let payload = |values: &[f64]| {
            GuiTrajectoryFrames::Memory(std::sync::Arc::new(
                values.iter().map(|x| frame(*x)).collect(),
            ))
            .dcd_bytes(2)
            .unwrap()
        };
        let fragments = state
            .active_scene_parts_with_external_trajectories(&[
                (first, "dcdtraj"),
                (second, "dcdtraj"),
                (first, "dcdtraj"),
            ])
            .unwrap();
        assert_eq!(fragments.len(), 4);
        let mut bytes = Vec::new();
        for (fragment, payload) in
            fragments[..3]
                .iter()
                .zip([payload(&[2., 3.]), payload(&[10., 11.]), payload(&[4.])])
        {
            bytes.extend(fragment);
            bytes.extend(payload);
        }
        bytes.extend(&fragments[3]);
        let mut restored = DesktopAppState::new();
        restored.open_bytes("scene.qsc", &bytes).unwrap();
        let objects = restored.active_object_summaries();
        assert_eq!(restored.trajectory_len(objects[0].uid), 4);
        assert_eq!(restored.trajectory_len(objects[1].uid), 2);
        for (index, expected) in [1., 2., 3., 4.].into_iter().enumerate() {
            restored
                .apply_trajectory_frame(objects[0].uid, index)
                .unwrap();
            assert_eq!(
                restored
                    .scene_manager()
                    .object(objects[0].uid)
                    .unwrap()
                    .mol()
                    .atoms()[0]
                    .position
                    .x,
                expected
            );
        }
        restored.apply_trajectory_frame(objects[1].uid, 1).unwrap();
        assert_eq!(
            restored
                .scene_manager()
                .object(objects[1].uid)
                .unwrap()
                .mol()
                .atoms()[0]
                .position
                .x,
            11.
        );
        assert!(state
            .active_scene_parts_with_external_trajectories(&[(first, "invalid")])
            .is_err());
    }

    #[test]
    fn browser_memory_scene_round_trip_has_embedded_coordinates_and_renderers() {
        let mut state = DesktopAppState::new();
        let GuiLoadedPath::Object { object_uid, .. } = state.open_bytes("ligand.pdb", PDB).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            state.renderer_summaries(object_uid)[0].renderer_type,
            "ballstick"
        );
        let scene = state.active_scene_bytes().unwrap();
        let mut loaded = DesktopAppState::new();
        loaded.open_bytes("embedded.qsc", &scene).unwrap();
        let object = &loaded.active_object_summaries()[0];
        assert_eq!(object.atom_count, 2);
        let positions: Vec<_> = loaded
            .scene_manager()
            .object(object.uid)
            .unwrap()
            .mol()
            .atoms()
            .iter()
            .map(|a| a.position)
            .collect();
        assert_eq!(positions[0], Vector4::new3(1.0, 2.0, 3.0));
        assert!(loaded
            .active_render_snapshot("")
            .unwrap()
            .atoms
            .iter()
            .any(|a| a.hit.is_none()));
    }
    #[test]
    fn coordinate_updates_validate_counts_and_update_the_cell() {
        let mut state = DesktopAppState::new();
        let GuiLoadedPath::Object { object_uid, .. } = state.open_bytes("ligand.pdb", PDB).unwrap()
        else {
            panic!()
        };
        let bad = cuemol_mdtools::TrajectoryFrame {
            coordinates: vec![1.0; 3],
            cell: None,
        };
        assert!(state.apply_coordinate_frame(object_uid, &bad).is_err());
        let frame = cuemol_mdtools::TrajectoryFrame {
            coordinates: vec![4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            cell: Some([30.0, 40.0, 50.0, 90.0, 100.0, 90.0]),
        };
        state.apply_coordinate_frame(object_uid, &frame).unwrap();
        let molecule = state.scene_manager().object(object_uid).unwrap().mol();
        assert_eq!(molecule.atoms()[1].position, Vector4::new3(7.0, 8.0, 9.0));
        assert_eq!(molecule.cell_params().unwrap().b, 40.0);
        assert_eq!(molecule.cell_params().unwrap().beta, 100.0);
    }
}
