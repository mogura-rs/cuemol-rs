use cuemol_desktop_core::DesktopAppState;

#[test]
fn failed_scene_save_keeps_linked_style_files_and_scene_state() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("original", true).result_scene;
    let style = state
        .import_scene_style(
            scene,
            "<styles id=\"custom\"><color id=\"accent\" value=\"red\"/></styles>",
            false,
        )
        .unwrap();
    let path = directory.path().join("custom.xml");
    state.save_scene_style_file(scene, style, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    state
        .update_scene_style(
            scene,
            style,
            "custom",
            false,
            "<styles id=\"custom\"><color id=\"accent\" value=\"blue\"/></styles>",
        )
        .unwrap();
    let snapshot = state.clone();
    assert!(state.save_active_scene_qsc(directory.path()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(state.same_undo_state(&snapshot));
}

#[test]
fn invalid_linked_style_keeps_other_styles_and_existing_scene_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("original", true).result_scene;
    let mut paths = Vec::new();
    for name in ["first", "second"] {
        let xml = format!("<styles id=\"{name}\"><color id=\"accent\" value=\"red\"/></styles>");
        let style = state.import_scene_style(scene, &xml, false).unwrap();
        let path = directory.path().join(format!("{name}.xml"));
        state.save_scene_style_file(scene, style, &path).unwrap();
        state
            .update_scene_style(scene, style, name, false, &xml.replace("red", "blue"))
            .unwrap();
        paths.push(path);
    }
    let before = std::fs::read(&paths[0]).unwrap();
    std::fs::remove_file(&paths[1]).unwrap();
    std::fs::create_dir(&paths[1]).unwrap();
    let scene_path = directory.path().join("scene.qsc");
    std::fs::write(&scene_path, b"original scene").unwrap();
    let snapshot = state.clone();
    assert!(state.save_active_scene_qsc(&scene_path).is_err());
    assert_eq!(std::fs::read(&paths[0]).unwrap(), before);
    assert_eq!(std::fs::read(&scene_path).unwrap(), b"original scene");
    assert!(state.same_undo_state(&snapshot));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 3);
}

#[test]
fn successful_saves_update_files_and_keep_reference_same_source_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("original", true).result_scene;
    let xml = "<styles id=\"custom\"><color id=\"accent\" value=\"red\"/></styles>";
    let style = state.import_scene_style(scene, xml, false).unwrap();
    let path = directory.path().join("custom.xml");
    state.save_scene_style_file(scene, style, &path).unwrap();
    state
        .update_scene_style(scene, style, "custom", false, &xml.replace("red", "blue"))
        .unwrap();
    assert!(
        state
            .style_summaries(scene)
            .iter()
            .find(|s| s.local_index == Some(style))
            .unwrap()
            .modified
    );
    state.save_scene_style_file(scene, style, &path).unwrap();
    assert!(
        state
            .style_summaries(scene)
            .iter()
            .find(|s| s.local_index == Some(style))
            .unwrap()
            .modified
    );
    state
        .update_scene_style(scene, style, "custom", false, &xml.replace("red", "green"))
        .unwrap();
    let scene_path = directory.path().join("scene.qsc");
    state.save_active_scene_qsc(&scene_path).unwrap();
    assert!(
        state
            .style_summaries(scene)
            .iter()
            .find(|s| s.local_index == Some(style))
            .unwrap()
            .modified
    );
    assert!(std::fs::read_to_string(&path).unwrap().contains("#008000"));
    let mut reloaded = DesktopAppState::new();
    let scene = reloaded.load_qsc_file(&scene_path).unwrap().unwrap();
    assert!(reloaded
        .scene_manager()
        .scene(scene)
        .unwrap()
        .loaded_styles()[0]
        .data
        .contains("#008000"));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn scene_saves_follow_symlinks_and_preserve_existing_file_permissions() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("original.qsc");
    let link = directory.path().join("linked.qsc");
    std::fs::write(&original, b"original").unwrap();
    std::fs::set_permissions(&original, std::fs::Permissions::from_mode(0o640)).unwrap();
    symlink("original.qsc", &link).unwrap();
    let mut state = DesktopAppState::new();
    state.new_scene("test", true);
    state.save_active_scene_qsc(&link).unwrap();
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(std::fs::read_to_string(&original)
        .unwrap()
        .contains("<scene"));
    assert_eq!(
        std::fs::metadata(&original).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[cfg(feature = "scripting")]
#[test]
fn failed_script_writer_keeps_external_styles_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("script", true).result_scene;
    let xml = "<styles id=\"custom\"><color id=\"accent\" value=\"red\"/></styles>";
    let style = state.import_scene_style(scene, xml, false).unwrap();
    let path = directory.path().join("custom.xml");
    state.save_scene_style_file(scene, style, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    state
        .update_scene_style(scene, style, "custom", false, &xml.replace("red", "blue"))
        .unwrap();
    let result = state.run_script_js(&format!(
        "var writer=cuemol.SceneXMLWriter();writer.attach(scene);writer.path={};writer.write();",
        serde_json::json!(directory.path())
    ));
    assert!(result.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn scene_and_style_paths_cannot_overwrite_each_other() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("collision", true).result_scene;
    let style = state
        .import_scene_style(scene, "<styles id=\"custom\"/>", false)
        .unwrap();
    let path = directory.path().join("custom.xml");
    state.save_scene_style_file(scene, style, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(state.save_active_scene_qsc(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
