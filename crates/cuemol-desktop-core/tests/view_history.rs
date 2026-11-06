use cuemol_desktop_core::DesktopAppState;

#[test]
fn scene_history_keeps_live_views_in_all_remaining_scenes() {
    let mut current = DesktopAppState::new();
    let first = current.new_scene("first", true);
    let second = current.new_scene("second", true);
    let mut snapshot = current.clone();
    let first_view = first.result_view.unwrap();
    current
        .command_manager_mut()
        .view_mut(first_view)
        .unwrap()
        .set_zoom(37.);
    let extra = current
        .command_manager_mut()
        .new_view_command(second.result_scene)
        .unwrap();
    let mut camera = current.active_view_camera().unwrap();
    camera.set_zoom(81.);
    current.set_active_view_camera(&camera).unwrap();
    let restored = snapshot.preserve_view_state_from(&current, &camera);
    assert_eq!(restored.zoom(), 81.);
    assert_eq!(snapshot.view_camera(first_view).unwrap().zoom(), 37.);
    assert_eq!(snapshot.view_camera(extra).unwrap().zoom(), 81.);
    assert_eq!(
        snapshot
            .scene_manager()
            .scene(second.result_scene)
            .unwrap()
            .active_view_uid(),
        Some(extra)
    );
    assert_eq!(snapshot.command_manager().views().len(), 3);
}

#[cfg(feature = "scripting")]
#[test]
fn script_scene_history_excludes_camera_changes_inside_transactions() {
    let mut state = DesktopAppState::new();
    let created = state.new_scene("history", true);
    let (actual, _) = state.run_script_js(&format!(r#"
        var view = cuemol.getView({});
        var molecule = cuemol.MolCoord(); molecule.name = 'probe'; scene.addObject(molecule);
        scene.startUndoTxn('camera'); view.zoom = 71; scene.commitUndoTxn();
        print('camera-only=' + scene.getUndoSize() + ':' + view.zoom);
        scene.startUndoTxn('edit'); molecule.name = 'changed'; view.zoom = 75; scene.commitUndoTxn();
        view.slab = 33; scene.undo(0);
        print('undo=' + molecule.name + ':' + view.zoom + ':' + view.slab);
        view.zoom = 91; scene.redo(0);
        print('redo=' + molecule.name + ':' + view.zoom);
        scene.startUndoTxn('rollback'); molecule.name = 'cancelled'; view.zoom = 101; scene.rollbackUndoTxn();
        print('rollback=' + molecule.name + ':' + view.zoom);
    "#, created.result_view.unwrap().get())).unwrap();
    assert_eq!(
        actual,
        "camera-only=0:71\nundo=probe:75:33\nredo=changed:91\nrollback=changed:101\n"
    );
    let Some(oracle) = std::env::var_os("CUEMOL_VIEW_HISTORY_ORACLE") else {
        eprintln!("C++ view history oracle not configured; Rust regression ran");
        return;
    };
    let config = std::env::var_os("CUEMOL_CAMERA_FIT_CONFIG").expect("oracle configuration");
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("view-history.txt");
    assert!(std::process::Command::new(oracle)
        .arg(config)
        .arg(&output)
        .status()
        .unwrap()
        .success());
    assert_eq!(actual, std::fs::read_to_string(output).unwrap());
}
