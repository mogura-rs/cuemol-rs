use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiMapCenterPolicy {
    #[default]
    Auto,
    SetMapCenter,
    MoveViewCenter,
}

impl GuiMapCenterPolicy {
    pub fn fits_map(self, is_em: bool) -> bool {
        match self {
            Self::Auto => is_em,
            Self::SetMapCenter => false,
            Self::MoveViewCenter => true,
        }
    }
}

impl DesktopAppState {
    pub fn apply_density_map_center_policy(
        &mut self,
        map_uid: Uid,
        renderer_uids: &[Uid],
        policy: GuiMapCenterPolicy,
        view: &mut View,
    ) -> Result<()> {
        let Some((_, kind)) = self.density_map_kind(map_uid) else {
            return Ok(());
        };
        let fit = policy.fits_map(kind == "em");
        let renderers = self
            .density_map_summary(map_uid)
            .ok_or_else(|| CueMolError::not_found("density map"))?
            .renderers
            .into_iter()
            .filter(|renderer| {
                !matches!(renderer.renderer_type.as_str(), "*unitcell" | "unitcell")
                    && (renderer_uids.is_empty() || renderer_uids.contains(&renderer.uid))
            })
            .map(|renderer| renderer.uid)
            .collect::<Vec<_>>();
        if renderers.is_empty() {
            return Ok(());
        }
        if fit {
            self.fit_density_map_view(map_uid, view)?;
        } else {
            for renderer in renderers {
                self.set_density_renderer_center(map_uid, renderer, view.camera().center())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuiMapType {
    #[default]
    Auto,
    Xtal,
    Em,
}

impl GuiMapType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Xtal => "xtal",
            Self::Em => "em",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiAmberPrmtopReaderOptions {
    pub coordinate_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMapHeader {
    pub counts: [usize; 3],
    pub mode: i32,
    pub space_group: i32,
    pub voxels: u64,
}

impl GuiMapHeader {
    pub const LARGE_MAP_VOXELS: u64 = 256 * 1024 * 1024;

    pub fn from_path(path: &Path) -> Result<Self> {
        Self::from_bytes(&cuemol_core::compressed::read(path, 1024)?)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let header = cuemol_xtal::Ccp4Header::parse(bytes)?;
        Ok(Self {
            counts: header.counts,
            mode: header.mode,
            space_group: header.space_group,
            voxels: header.voxel_count()? as u64,
        })
    }

    pub fn stored_voxels(&self, stride: usize) -> u64 {
        self.counts.iter().fold(1u64, |total, count| {
            total.saturating_mul(count.div_ceil(stride.max(1)) as u64)
        })
    }

    pub fn suggested_subsample(&self) -> usize {
        let mut stride = 1;
        while self.stored_voxels(stride) > Self::LARGE_MAP_VOXELS && stride < 8 {
            stride *= 2;
        }
        stride
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(crate) fn topology() -> String {
        let mut pointers = [0; 31];
        pointers[0] = 1;
        pointers[11] = 1;
        let mut result =
            "%VERSION VERSION_STAMP = V0001.000\n%FLAG POINTERS\n%FORMAT(10I8)\n".to_string();
        for chunk in pointers.chunks(10) {
            for value in chunk {
                result.push_str(&format!("{value:8}"));
            }
            result.push('\n');
        }
        result.push_str("%FLAG ATOM_NAME\n%FORMAT(20a4)\nC1  \n%FLAG ATOMIC_NUMBER\n%FORMAT(10I8)\n       6\n%FLAG RESIDUE_LABEL\n%FORMAT(20a4)\nLIG \n%FLAG RESIDUE_POINTER\n%FORMAT(10I8)\n       1\n");
        result
    }

    #[test]
    fn amber_coordinates_survive_scene_roundtrips_and_failed_loads_are_atomic() {
        let dir = tempfile::tempdir_in(".").unwrap();
        let path = dir.path().join("sample.prmtop.gz");
        let coordinates = dir.path().join("sample.rst7");
        let mut writer = cuemol_qsys::InOutHandler::default();
        writer.set_path(path.to_string_lossy());
        writer.set_compress(1);
        writer.write_bytes(topology().as_bytes()).unwrap();
        std::fs::write(
            &coordinates,
            format!("Coordinates\n1\n{:12.7}{:12.7}{:12.7}\n", 1.25, -2.5, 3.75),
        )
        .unwrap();
        let mut app = DesktopAppState::new();
        let mut options = app.molecule_file_setup_for_path(&path).unwrap().unwrap();
        assert_eq!(
            options
                .amber_prmtop_reader_options
                .as_ref()
                .unwrap()
                .coordinate_path,
            ""
        );
        options
            .amber_prmtop_reader_options
            .as_mut()
            .unwrap()
            .coordinate_path = coordinates.to_string_lossy().into_owned();
        let setup = GuiRendererSetup {
            object_name: "loaded".into(),
            renderer_type: "ballstick".into(),
            renderer_name: "atoms".into(),
            selection: None,
            recenter: true,
            map_center_policy: None,
        };
        let GuiLoadedPath::Object { object_uid, .. } = app
            .open_path_with_molecule_reader_setup(&path, &options, &setup)
            .unwrap()
        else {
            panic!("molecule")
        };
        let expected = Vector4::new3(1.25, -2.5, 3.75);
        assert_eq!(
            app.scene_manager()
                .object(object_uid)
                .unwrap()
                .mol()
                .atoms()[0]
                .position,
            expected
        );
        for embed_all in [false, true] {
            app.scene_manager_mut()
                .object_mut(object_uid)
                .unwrap()
                .set_modified(false);
            let scene = dir.path().join(if embed_all {
                "embedded.qsc"
            } else {
                "linked.qsc"
            });
            app.save_active_scene_qsc_with_options(
                &scene,
                GuiSceneSaveOptions {
                    embed_all,
                    ..Default::default()
                },
            )
            .unwrap();
            let mut restored = DesktopAppState::new();
            restored.open_path(&scene).unwrap();
            let molecule = &restored
                .scene_manager()
                .scene(restored.active_scene().unwrap())
                .unwrap()
                .objects()[0];
            assert_eq!(
                molecule.mol().atoms()[0].position,
                expected,
                "embed={embed_all}"
            );
        }
        let before = app.config_text();
        let count = app
            .scene_manager()
            .scene(app.active_scene().unwrap())
            .unwrap()
            .objects()
            .len();
        options
            .amber_prmtop_reader_options
            .as_mut()
            .unwrap()
            .coordinate_path = dir
            .path()
            .join("missing.rst7")
            .to_string_lossy()
            .into_owned();
        assert!(app
            .open_path_with_molecule_reader_setup(&path, &options, &setup)
            .is_err());
        assert_eq!(app.config_text(), before);
        assert_eq!(
            app.scene_manager()
                .scene(app.active_scene().unwrap())
                .unwrap()
                .objects()
                .len(),
            count
        );
    }

    #[test]
    fn map_header_uses_rounded_grid_counts_and_bounded_stride_suggestions() {
        let header = GuiMapHeader {
            counts: [1025, 1025, 257],
            mode: 2,
            space_group: 0,
            voxels: 1025 * 1025 * 257,
        };
        assert_eq!(header.stored_voxels(2), 513 * 513 * 129);
        assert_eq!(header.stored_voxels(0), header.voxels);
        assert_eq!(header.suggested_subsample(), 2);
        assert!(serde_json::from_str::<GuiMapType>("\"invalid\"").is_err());
        assert_eq!(serde_json::to_string(&GuiMapType::Em).unwrap(), "\"em\"");
    }

    pub(crate) fn test_map_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 1024];
        for (word, value) in [
            (0, 2i32),
            (1, 2),
            (2, 2),
            (3, 2),
            (7, 2),
            (8, 2),
            (9, 2),
            (16, 1),
            (17, 2),
            (18, 3),
            (22, 1),
        ] {
            bytes[word * 4..word * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (word, value) in [
            (10, 2f32),
            (11, 2.),
            (12, 2.),
            (13, 90.),
            (14, 90.),
            (15, 90.),
            (20, 7.),
            (21, 3.5),
            (54, 2.),
        ] {
            bytes[word * 4..word * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[208..212].copy_from_slice(b"MAP ");
        for value in 0..8 {
            bytes.extend_from_slice(&(value as f32).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn file_map_kind_override_precedes_renderer_creation_and_survives_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("density.mrc");
        let bytes = test_map_bytes();
        std::fs::write(&path, bytes).unwrap();
        let mut app = DesktopAppState::new();
        let defaults = app.file_renderer_setup_for_path(&path).unwrap().unwrap();
        assert_eq!(defaults.map_header.unwrap().counts, [2, 2, 2]);
        let options = GuiDensityMapReaderOptions::Ccp4(GuiCcp4MapReaderOptions {
            map_type: GuiMapType::Em,
            ..Default::default()
        });
        let setup = GuiRendererSetup {
            object_name: "map".into(),
            renderer_type: "isosurf".into(),
            renderer_name: "surface".into(),
            selection: None,
            recenter: true,
            map_center_policy: None,
        };
        let GuiLoadedPath::Map { map_uid, .. } = app
            .open_density_map_path_with_renderer_setup(&path, Some(&options), None, &setup)
            .unwrap()
        else {
            panic!("map")
        };
        assert_eq!(app.density_map_kind(map_uid), Some(("em", "em")));
        assert_eq!(
            app.density_map_summary(map_uid).unwrap().renderers[0]
                .specific
                .get("region_mode_resolved")
                .map(String::as_str),
            Some("full")
        );
        let scene = app.active_scene_bytes().unwrap();
        let mut restored = DesktopAppState::new();
        restored.open_bytes("map.qsc", &scene).unwrap();
        let map_uid = restored.density_map_summaries(restored.active_scene().unwrap())[0].uid;
        assert_eq!(restored.density_map_kind(map_uid), Some(("em", "em")));
    }
    #[test]
    fn map_center_policy_resolves_after_reader_override_and_fits_only_the_loaded_map() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("density.mrc");
        std::fs::write(&path, test_map_bytes()).unwrap();
        for kind in [GuiMapType::Xtal, GuiMapType::Em] {
            for policy in [
                GuiMapCenterPolicy::Auto,
                GuiMapCenterPolicy::SetMapCenter,
                GuiMapCenterPolicy::MoveViewCenter,
            ] {
                let mut app = DesktopAppState::new();
                let current_center = Vector4::new3(70., -25., 41.);
                let setup = GuiRendererSetup {
                    object_name: "loaded".into(),
                    renderer_type: "contour".into(),
                    renderer_name: "density".into(),
                    selection: None,
                    recenter: false,
                    map_center_policy: Some(policy),
                };
                let options = GuiDensityMapReaderOptions::Ccp4(GuiCcp4MapReaderOptions {
                    map_type: kind,
                    ..Default::default()
                });
                let GuiLoadedPath::Map { scene_uid, map_uid } = app
                    .open_density_map_path_with_renderer_setup(
                        &path,
                        Some(&options),
                        Some(current_center),
                        &setup,
                    )
                    .unwrap()
                else {
                    panic!("map")
                };
                let mut view = View::new(Uid::new(1), scene_uid);
                let mut camera = view.camera();
                camera.set_center(current_center);
                camera.set_zoom(150.);
                view.set_camera(&camera);
                view.set_view_size(400, 800);
                let mut expected = view.clone();
                let should_fit = policy == GuiMapCenterPolicy::MoveViewCenter
                    || (policy == GuiMapCenterPolicy::Auto && kind == GuiMapType::Em);
                if should_fit {
                    app.fit_density_map_view(map_uid, &mut expected).unwrap();
                }
                let renderer_uid = app.density_map_summary(map_uid).unwrap().renderers[0].uid;
                let before_center = app.density_map_summary(map_uid).unwrap().renderers[0].specific
                    ["center"]
                    .clone();
                if should_fit {
                    assert_ne!(before_center, current_center.to_string());
                }
                app.apply_density_map_center_policy(map_uid, &[], policy, &mut view)
                    .unwrap();
                assert_eq!(
                    view.camera().center(),
                    expected.camera().center(),
                    "{kind:?} {policy:?}"
                );
                assert_eq!(
                    view.camera().zoom(),
                    expected.camera().zoom(),
                    "{kind:?} {policy:?}"
                );
                let summary = app.density_map_summary(map_uid).unwrap();
                let renderer = summary
                    .renderers
                    .iter()
                    .find(|r| r.uid == renderer_uid)
                    .unwrap();
                assert_eq!(renderer.specific["center"], before_center);
                let unitcell = app.add_density_renderer(map_uid, "*unitcell").unwrap();
                let before = view.camera();
                app.apply_density_map_center_policy(
                    map_uid,
                    &[unitcell],
                    GuiMapCenterPolicy::MoveViewCenter,
                    &mut view,
                )
                .unwrap();
                assert_eq!(view.camera(), before);
            }
        }
    }
}
