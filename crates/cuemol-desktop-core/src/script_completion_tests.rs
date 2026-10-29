use super::*;

#[test]
fn read_only_script_preserves_undo_state() {
    let mut app = DesktopAppState::new();
    app.open_bytes(
        "probe.pdb",
        b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n",
    )
    .unwrap();
    let before = app.clone();
    app.run_script_js("print(1);").unwrap();
    assert_eq!(
        before.scene_manager().scenes(),
        app.scene_manager().scenes()
    );
    assert_eq!(before.command_manager.views(), app.command_manager.views());
    assert_eq!(before.script_styles, app.script_styles);
    assert_eq!(before.runtime, app.runtime);
    assert!(before.same_undo_state(&app));
}

#[test]
fn script_command_arguments_preserve_text_and_count_semantics() {
    let mut app = DesktopAppState::new();
    app.new_scene("arguments", true);
    let arguments = vec!["alpha".into(), "日本語 with spaces".into(), String::new()];
    let (log, _) = app.run_script_js_with_args(
        "print(getCmdArgs(-1)); print(cuemol.getCmdArgs(0)); print(getCmdArgs(1)); print('['+getCmdArgs(2)+']'); print(getCmdArgs(3)); var bad = 0; for (var x of ['0', 0.5, NaN]) { try { getCmdArgs(x); } catch (e) { bad++; } } print(bad);",
        &arguments,
    ).unwrap();
    assert_eq!(log, "3\nalpha\n日本語 with spaces\n[]\n3\n3\n");
    assert_eq!(app.run_script_js("print(getCmdArgs(0));").unwrap().0, "0\n");
}

#[test]
fn script_style_capture_keeps_live_and_detached_renderer_properties() {
    let mut app = DesktopAppState::new();
    app.new_scene("styles", true);
    let (log, _) = app.run_script_js(r#"
        var styles = getService('StyleManager');
        var set = styles.createStyleSet('capture', scene.uid);
        var mol = cuemol.MolCoord(); scene.addObject(mol);
        var live = mol.createRenderer('simple');
        live.alpha = 0.4; live.width = 3.25; live.sel = cuemol.makeSel('name CA', 0);
        var color = cuemol.SolidColoring(); color.color = cuemol.makeColor('#ff8800'); live.coloring = color;
        styles.createStyleFromObj(scene.uid, set, 'Live', live);
        print(styles.getStyleValue(scene.uid, 'capture', 'Live.alpha'));
        print(styles.getStyleValue(scene.uid, 'capture', 'Live.width'));
        print(styles.getStyleValue(scene.uid, 'capture', 'Live.sel'));
        var detached = cuemol.SimpleRenderer(); detached.width = 7.5; detached.alpha = 0.6;
        styles.createStyleFromObj(scene.uid, set, 'Detached', detached);
        print(styles.getStyleValue(scene.uid, 'capture', 'Detached.width'));
        print(styles.getStyleValue(scene.uid, 'capture', 'Detached.alpha'));
    "#).unwrap();
    assert_eq!(log, "0.4\n3.25\nname CA\n7.5\n0.6\n");
    let set = app
        .script_styles
        .style_sets(app.active_scene().unwrap().get());
    let set = set.iter().find(|set| set.name() == "capture").unwrap();
    let node = set.get_style("Live").unwrap();
    assert!(node
        .children()
        .iter()
        .any(|child| child.tag_name() == "coloring"));
}

#[test]
fn style_capture_resolves_inherited_nested_properties() {
    let mut app = DesktopAppState::new();
    app.new_scene("styles", true);
    let (log, _) = app
        .run_script_js(
            r#"
        var styles = getService('StyleManager');
        var set = styles.createStyleSet('nested', scene.uid);
        var mol = cuemol.MolCoord(); scene.addObject(mol);
        var source = mol.createRenderer('ribbon'); source.helix.width = 5.5; source.alpha = 0.7;
        styles.createStyleFromObj(scene.uid, set, 'Source', source);
        var inherited = mol.createRenderer('ribbon'); inherited.applyStyles('Source');
        print(inherited.alpha+':'+inherited.helix.width);
        styles.createStyleFromObj(scene.uid, set, 'Copy', inherited);
        print(styles.getStyleValue(scene.uid, 'nested', 'Copy.helix.width'));
        inherited.helix.width = 2.25;
        print(inherited.helix.width);
    "#,
        )
        .unwrap();
    assert_eq!(log, "0.7:5.5\n5.5\n2.25\n");
}

#[test]
fn style_capture_keeps_inherited_coloring_after_one_property_is_edited() {
    let mut app = DesktopAppState::new();
    app.new_scene("color-styles", true);
    let (log, _) = app.run_script_js(r#"
        var styles = getService('StyleManager');
        var set = styles.createStyleSet('colors', scene.uid);
        var mol = cuemol.MolCoord(); scene.addObject(mol);
        var source = mol.createRenderer('simple');
        var colors = cuemol.RainbowColoring(); colors.start_hue = 100; colors.end_hue = 50; colors.sat = 0.2;
        source.coloring = colors; styles.createStyleFromObj(scene.uid, set, 'Rainbow', source);
        var inherited = mol.createRenderer('simple'); inherited.applyStyles('Rainbow');
        print(inherited.coloring.end_hue);
        inherited.coloring.start_hue = 200;
        print(inherited.coloring.end_hue+':'+inherited.coloring.sat);
        styles.createStyleFromObj(scene.uid, set, 'Edited', inherited);
        print(styles.getStyleValue(scene.uid, 'colors', 'Edited.coloring.end_hue'));
        print(styles.getStyleValue(scene.uid, 'colors', 'Edited.coloring.start_hue'));
    "#).unwrap();
    assert_eq!(log, "50\n50:0.2\n50\n200\n");
}

#[test]
fn python_bridge_globals_survive_separate_script_calls() {
    let mut app = DesktopAppState::new();
    app.new_scene("python", true);
    app.run_script_js("getService('PythonBridge').runString('completion_probe = [42]');")
        .unwrap();
    app.run_script_js("getService('PythonBridge').runString('completion_probe.append(7)');")
        .unwrap();
    app.run_script_js(
        "getService('PythonBridge').runString('assert completion_probe == [42, 7]');",
    )
    .unwrap();
    let mut other = DesktopAppState::new();
    other.new_scene("isolated", true);
    other.run_script_js(r#"getService('PythonBridge').runString('assert "completion_probe" not in globals()');"#).unwrap();
}

fn run_python(app: &mut DesktopAppState, source: &str) -> Result<(String, Option<String>)> {
    app.run_script_js(&format!(
        "getService('PythonBridge').runString({});",
        serde_json::to_string(source).unwrap()
    ))
}

#[test]
fn python_bindings_edit_live_objects_and_keep_detached_reader_targets() {
    let mut app = DesktopAppState::new();
    app.new_scene("python", true);
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        "ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C  \nEND\n",
    )
    .unwrap();
    run_python(&mut app, "import cuemol\nreader = cuemol.strMgr().createHandler('pdb', 0)\nreader.attach(reader.createDefaultObj())").unwrap();
    run_python(
        &mut app,
        &format!(
            "reader.setPath({})\nreader.read()",
            serde_json::to_string(&file.path().to_string_lossy()).unwrap()
        ),
    )
    .unwrap();
    run_python(&mut app, "molecule = reader.detach()\nassert molecule.getAtomSize() == 1\nmolecule.name = 'from-python'\ncuemol.scene().addObject(molecule)\nrenderer = molecule.createRenderer('simple')\nrenderer.width = 3.25\nmolecule.getAtomByID(1).pos = cuemol.vec(9, 8, 7)\nassert cuemol.obj('from-python').uid == molecule.uid").unwrap();
    let object = app
        .scene_manager()
        .scene(app.active_scene().unwrap())
        .unwrap()
        .object_by_name("from-python")
        .unwrap();
    assert_eq!(object.mol().atoms()[0].position.x, 9.0);
    assert_eq!(object.renderers()[0].qsc_attr("width"), Some("3.25"));
    app.run_script_js("scene.getObjectByName('from-python').name = 'from-js';")
        .unwrap();
    assert_eq!(
        run_python(
            &mut app,
            "assert molecule.name == 'from-js'\nprint(renderer.width)"
        )
        .unwrap()
        .0,
        "3.25\n"
    );
    let before = app.clone();
    run_python(
        &mut app,
        "assert cuemol.scene().getObjectByName('from-js').uid == molecule.uid",
    )
    .unwrap();
    assert!(before.same_undo_state(&app));
    assert!(run_python(&mut app, "renderer.no_such_property = 42").is_err());
    run_python(&mut app, "assert renderer.width == 3.25").unwrap();
}

#[test]
fn python_binding_releases_unreferenced_detached_payloads() {
    let mut app = DesktopAppState::new();
    app.new_scene("python", true);
    run_python(
        &mut app,
        "import cuemol\nvalues = [cuemol.createObj('MolCoord') for _ in range(30)]",
    )
    .unwrap();
    assert_eq!(
        app.script_services
            .resolve()
            .borrow()
            .python_objects
            .retained_counts(),
        (30, 30)
    );
    run_python(&mut app, "del values\nimport gc\ngc.collect()").unwrap();
    assert_eq!(
        app.script_services
            .resolve()
            .borrow()
            .python_objects
            .retained_counts(),
        (0, 0)
    );
    run_python(
        &mut app,
        "atom = cuemol.createObj('MolAtom')\natom.pos = cuemol.vec(1, 2, 3)",
    )
    .unwrap();
    run_python(&mut app, "assert atom.pos.y == 2\ndel atom").unwrap();
    assert_eq!(
        app.script_services
            .resolve()
            .borrow()
            .python_objects
            .retained_counts(),
        (0, 0)
    );
}

#[test]
fn python_callbacks_handle_nested_calls_and_survive_between_scripts() {
    let mut app = DesktopAppState::new();
    app.new_scene("callbacks", true);
    run_python(&mut app, "import cuemol\nevents = []\ndef changed(*args):\n    assert cuemol.scene().name == 'callbacks'\n    events.append(args[1])\nlistener = cuemol.scene().addListener(changed)\nmol = cuemol.createObj('MolCoord')\ncuemol.scene().addObject(mol)\nassert len(events) == 1").unwrap();
    app.run_script_js("scene.addObject(cuemol.MolCoord());")
        .unwrap();
    run_python(
        &mut app,
        "assert len(events) == 2\nassert cuemol.scene().removeListener(listener)",
    )
    .unwrap();
    assert_eq!(
        app.script_services
            .resolve()
            .borrow()
            .python_objects
            .callback_count(),
        0
    );
    app.run_script_js("scene.addObject(cuemol.MolCoord());")
        .unwrap();
    run_python(&mut app, "assert len(events) == 2").unwrap();
}

#[test]
fn python_callback_failure_rolls_back_scene_and_recovers_interpreter() {
    let mut app = DesktopAppState::new();
    app.new_scene("callbacks", true);
    run_python(&mut app, "import cuemol\ndef fail(*args):\n    raise ValueError('callback probe')\nlistener = cuemol.scene().addListener(fail)").unwrap();
    let before = app.clone();
    assert!(app
        .run_script_js("scene.addObject(cuemol.MolCoord());")
        .unwrap_err()
        .to_string()
        .contains("callback probe"));
    assert!(before.same_undo_state(&app));
    run_python(
        &mut app,
        "assert cuemol.scene().removeListener(listener)\nassert cuemol.scene().name == 'callbacks'",
    )
    .unwrap();
    assert_eq!(
        app.script_services
            .resolve()
            .borrow()
            .python_objects
            .callback_count(),
        0
    );
}

#[test]
fn service_settings_logs_and_listeners_survive_script_calls() {
    let mut app = DesktopAppState::new();
    app.new_scene("services", true);
    app.run_script_js("getService('ProcessManager').setSlotSize(3); getService('MsgLog').write('retained'); scene.addListener(function(method, type, source, target, description) { print('event:'+description); });").unwrap();
    let (log, _) = app.run_script_js("print(getService('ProcessManager').getSlotSize()); print(getService('MsgLog').getAccumMsg()); var mol = cuemol.MolCoord(); mol.name = 'created'; scene.addObject(mol);").unwrap();
    assert!(log.starts_with("3\nretained\n"), "{log}");
    assert!(log.contains("event:"), "{log}");
    let (log, _) = app.run_script_js("print(scene.removeListener(0)); getService('MsgLog').removeAccumMsg(); print(getService('MsgLog').getAccumMsg());").unwrap();
    assert_eq!(log, "true\n\n");
    let mut other = DesktopAppState::new();
    other.new_scene("isolated", true);
    assert_eq!(
        other
            .run_script_js("print(getService('ProcessManager').getSlotSize());")
            .unwrap()
            .0,
        "2\n"
    );
}

#[test]
fn process_service_retains_task_ids_between_scripts() {
    let mut app = DesktopAppState::new();
    app.new_scene("process", true);
    let cache = tempfile::tempdir().unwrap();
    let arguments = format!(
        "run --no-project --cache-dir {} python -c \"print(42)\"",
        serde_json::to_string(&cache.path().to_string_lossy()).unwrap()
    );
    let id = app
        .run_script_js(&format!(
            "print(getService('ProcessManager').queueTask('uv', {}, ''));",
            serde_json::to_string(&arguments).unwrap()
        ))
        .unwrap()
        .0;
    let id = id.trim().parse::<u64>().unwrap();
    let (log, _) = app.run_script_js(&format!("var manager = getService('ProcessManager'); manager.waitForExit({id}); print(manager.getResultOutput({id}));")).unwrap();
    assert_eq!(log, "42\n\n");
}

#[test]
fn script_services_are_released_with_the_last_application_snapshot() {
    let mut app = DesktopAppState::new();
    app.new_scene("services", true);
    run_python(&mut app, "value = 42").unwrap();
    let services = std::rc::Rc::downgrade(&app.script_services.resolve());
    let snapshot = app.clone();
    drop(app);
    assert!(services.upgrade().is_some());
    drop(snapshot);
    assert!(services.upgrade().is_none());
}

#[test]
fn surface_without_renderers_stays_hidden_after_scripts_and_renderer_recreation() {
    let mut app = DesktopAppState::new();
    let scene = app.new_scene("surfaces", true).result_scene;
    app.scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_loaded_surface(LoadedSurface {
            uid: Uid::new(71),
            name: "triangle".into(),
            object_type: "MolSurfObj".into(),
            srctype: "obj".into(),
            data: "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n".into(),
            raw_data: Vec::new(),
            qsc_node: None,
        });
    assert_eq!(app.active_scene_surface_meshes().len(), 1);
    let renderer = app.surface_summaries(scene)[0].renderers[0].uid;
    app.destroy_surface_renderer(Uid::new(71), renderer)
        .unwrap();
    assert!(app.active_scene_surface_meshes().is_empty());
    app.run_script_js("print(cuemol.getObject('triangle').name);")
        .unwrap();
    assert!(app.surface_summaries(scene)[0].renderers.is_empty());
    assert!(app.active_scene_surface_meshes().is_empty());
    app.add_surface_renderer(Uid::new(71)).unwrap();
    assert_eq!(app.surface_summaries(scene)[0].renderers.len(), 1);
    assert_eq!(app.active_scene_surface_meshes().len(), 1);
}
