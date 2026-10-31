use cuemol_desktop_core::{DesktopAppState, PreparedFile};

#[test]
fn invalid_qsc_palette_preserves_existing_scene_state() {
    let directory =
        std::env::temp_dir().join(format!("cuemol-invalid-palette-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("invalid.qsc");
    let data = br##"<scene name="invalid"><styles id="invalid"><color id="copy" value="unknown-accent"/></styles></scene>"##;
    std::fs::write(&path, data).unwrap();
    let mut observations = Vec::new();
    for prepared in [false, true] {
        let mut app = DesktopAppState::new();
        app.new_scene("existing", true);
        let before = app.clone();
        let failed = if prepared {
            app.open_prepared(PreparedFile::parse("invalid.qsc", data).unwrap())
                .is_err()
        } else {
            app.load_qsc_file(&path).is_err()
        };
        observations.push((
            prepared,
            failed,
            app.same_undo_state(&before),
            app.scene_manager().scene_uids().len(),
        ));
    }
    std::fs::remove_dir_all(directory).unwrap();
    for (prepared, failed, unchanged, scenes) in observations {
        assert!(failed, "prepared={prepared}");
        assert!(
            unchanged,
            "prepared={prepared}: failed palette left {scenes} scenes"
        );
    }
}
