#![cfg(feature = "scripting")]

use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;

const PDB: &str = "HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00  0.00           C\nHETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00  0.00           O\nEND\n";

fn fixture() -> (DesktopAppState, Uid, Uid) {
    let mut state = DesktopAppState::new();
    let scene = state.new_scene("styles", true).result_scene;
    let object = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("Probe", cuemol_molstr::read_pdb_str(PDB).unwrap());
    let renderer = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("cpk", "atoms");
    (state, object, renderer)
}

fn script(state: &mut DesktopAppState, source: &str) -> String {
    state.run_script_js(source).unwrap().0
}

fn create_selection(state: &mut DesktopAppState, name: &str, scope: u64) -> u64 {
    script(state, &format!(
        "var styles=cuemol.getService('StyleManager'); var id=styles.createStyleSet('{name}',{scope}); \
         styles.setStrData('sel','focus','name C1',{scope},id); print(id);"
    )).trim().parse().unwrap()
}

fn selected_names(state: &DesktopAppState) -> Vec<String> {
    let snapshot = state.active_render_snapshot("").unwrap();
    let object = state
        .scene_manager()
        .scene(state.active_scene().unwrap())
        .unwrap()
        .objects()
        .first()
        .unwrap();
    snapshot
        .atoms
        .iter()
        .map(|atom| {
            object.mol().atoms()[atom.atom_index.unwrap()]
                .name
                .trim()
                .to_string()
        })
        .collect()
}

#[test]
fn script_selection_updates_drawing_picking_and_saved_scene() {
    let (mut state, object, renderer) = fixture();
    let scene = state.active_scene().unwrap().get();
    let id = create_selection(&mut state, "local", scene);
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    assert_eq!(selected_names(&state), ["C1"]);
    assert_eq!(state.active_pick_atoms("").unwrap().len(), 1);
    assert!(state.selection_definitions().0.contains(&"focus".into()));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.qsc");
    state.save_active_scene_qsc(&path).unwrap();
    let mut loaded = DesktopAppState::new();
    loaded.load_qsc_file(&path).unwrap();
    assert_eq!(selected_names(&loaded), ["C1"]);
    assert_eq!(script(&mut state, &format!("print(cuemol.getService('StyleManager').removeStrData('sel','focus',{scene},{id}));")), "true\n");
    assert!(selected_names(&state).is_empty());
    assert!(state.active_pick_atoms("").unwrap().is_empty());
    assert_eq!(
        state
            .scene_manager()
            .object(object)
            .unwrap()
            .renderer(renderer)
            .unwrap()
            .qsc_attr("sel"),
        Some("focus")
    );
}

#[test]
fn gui_edits_keep_script_style_identity_and_do_not_resurrect_deleted_sets() {
    let (mut state, object, renderer) = fixture();
    let scene = state.active_scene().unwrap();
    let index = state.create_scene_style(scene, "local").unwrap();
    state
        .update_scene_style(
            scene,
            index,
            "local",
            false,
            "<styles id=\"local\"><sel id=\"focus\" value=\"name C1\"/></styles>",
        )
        .unwrap();
    let id = script(
        &mut state,
        &format!(
            "var styles=cuemol.getService('StyleManager'); var id=styles.hasStyleSet('local',{0}); styles.setStrData('sel','keep','*',{0},id); print(id);",
            scene.get()
        ),
    )
    .trim()
    .parse::<u64>()
    .unwrap();
    state
        .update_scene_style(
            scene,
            index,
            "renamed",
            false,
            "<styles id=\"renamed\"><sel id=\"focus\" value=\"name O1\"/></styles>",
        )
        .unwrap();
    assert_eq!(script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); print(styles.hasStyleSet('renamed',{})); print(styles.getStrData('sel','focus',{}));", scene.get(), scene.get())), format!("{id}\nname O1\n"));
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    assert_eq!(selected_names(&state), ["O1"]);
    state.destroy_scene_style(scene, index).unwrap();
    assert_eq!(
        script(
            &mut state,
            &format!(
                "print(cuemol.getService('StyleManager').hasStyleSet('renamed',{}));",
                scene.get()
            )
        ),
        "0\n"
    );
    assert!(selected_names(&state).is_empty());
    state.create_scene_style(scene, "renamed").unwrap();
    let fresh = script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); print(styles.hasStyleSet('renamed',{})); print(styles.getStyleSet({id}) == null);", scene.get()));
    let rows = fresh.lines().collect::<Vec<_>>();
    assert_ne!(rows[0].parse::<u64>().unwrap(), id);
    assert_eq!(rows[1], "true");
    let replacement = rows[0].parse::<u64>().unwrap();
    state.destroy_scene_style(scene, 0).unwrap();
    state.create_scene_style(scene, "renamed").unwrap();
    assert_eq!(
        script(
            &mut state,
            &format!(
                "print(cuemol.getService('StyleManager').getStyleSet({replacement}) == null);"
            )
        ),
        "true\n"
    );
}

#[test]
fn global_and_local_styles_keep_scope_and_precedence() {
    let (mut state, object, renderer) = fixture();
    let scene = state.active_scene().unwrap();
    let global = create_selection(&mut state, "global-test", 0);
    assert!(state
        .style_summaries(scene)
        .iter()
        .any(|set| set.name == "global-test" && set.modified));
    assert_eq!(
        script(
            &mut state,
            "print(cuemol.getService('StyleManager').createStyleSet('global-test',0));"
        ),
        "0\n"
    );
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    assert_eq!(selected_names(&state), ["C1"]);
    let local = create_selection(&mut state, "local", scene.get());
    script(
        &mut state,
        &format!(
            "cuemol.getService('StyleManager').setStrData('sel','focus','name O1',{},{local});",
            scene.get()
        ),
    );
    assert_eq!(selected_names(&state), ["O1"]);
    let (locals, globals) = state.selection_definitions();
    assert!(locals.contains(&"focus".into()));
    assert!(globals.contains(&"focus".into()));
    script(
        &mut state,
        &format!(
            "cuemol.getService('StyleManager').destroyStyleSet({},{local});",
            scene.get()
        ),
    );
    assert_eq!(selected_names(&state), ["C1"]);
    script(
        &mut state,
        &format!("cuemol.getService('StyleManager').destroyStyleSet(0,{global});"),
    );
    assert!(selected_names(&state).is_empty());
}

#[test]
fn equal_style_set_names_in_distinct_scopes_keep_local_precedence_and_global_fallback() {
    let (mut state, object, renderer) = fixture();
    let scope = state.active_scene().unwrap().get();
    let global = create_selection(&mut state, "shared", 0);
    script(&mut state, &format!("cuemol.getService('StyleManager').setColor('sky',cuemol.makeColor('#123456'),0,{global});"));
    let local = create_selection(&mut state, "shared", scope);
    script(&mut state, &format!("cuemol.getService('StyleManager').setStrData('sel','focus','name O1',{scope},{local});"));
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    assert_eq!(selected_names(&state), ["O1"]);
    state
        .scene_manager_mut()
        .scene_mut(Uid::new(scope))
        .unwrap()
        .set_scene_attribute("bgcolor", "sky");
    let background = state.active_background_color().unwrap();
    for (value, byte) in background[..3].iter().zip([18., 52., 86.]) {
        assert!((*value - byte / 255.).abs() < 1e-6);
    }
}

#[test]
fn script_style_creation_in_a_new_scene_survives_the_next_submission() {
    let mut state = DesktopAppState::new();
    state.ensure_active_scene();
    let log = script(&mut state, "var scene=cuemol.getService('SceneManager').createScene(); scene.name='new'; var styles=cuemol.getService('StyleManager'); var id=styles.createStyleSet('new-local',scene.uid); styles.setStrData('sel','fresh','name C1',scene.uid,id); print(scene.uid); print(id);");
    let ids = log
        .lines()
        .map(|line| line.parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        state
            .scene_manager()
            .scene(Uid::new(ids[0]))
            .unwrap()
            .loaded_styles()
            .len(),
        1
    );
    assert_eq!(script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); print(styles.hasStyleSet('new-local',{})); print(styles.getStrData('sel','fresh',{}));", ids[0], ids[0])), format!("{}\nname C1\n", ids[1]));
}

#[test]
fn reading_loaded_styles_does_not_create_an_undo_change() {
    let (mut state, _, _) = fixture();
    let scene = state.active_scene().unwrap();
    state.create_scene_style(scene, "local").unwrap();
    let before = state.clone();
    assert_eq!(
        script(
            &mut state,
            "print(cuemol.getService('StyleManager').getStrData('sel','missing',scene.uid));"
        ),
        "\n"
    );
    assert!(state.same_undo_state(&before));
}

#[test]
fn script_palette_and_renderer_presets_reach_the_viewport() {
    let (mut state, object, previous_renderer) = fixture();
    let scope = state.active_scene().unwrap().get();
    let id = create_selection(&mut state, "appearance", scope);
    let object = state.scene_manager_mut().object_mut(object).unwrap();
    object.destroy_renderer(previous_renderer);
    let renderer = object.add_renderer("ballstick", "styled");
    object
        .renderer_mut(renderer)
        .unwrap()
        .set_qsc_attr("style", "Custom");
    script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); styles.setColor('accent',cuemol.makeColor('#123456'),{scope},{id}); styles.setStyleValue({scope},'appearance','Custom.defaultcolor','accent'); styles.setStyleValue({scope},'appearance','Custom.sphr','0.9');"));
    let snapshot = state.active_render_snapshot("").unwrap();
    let color = snapshot.atoms[0].color.as_ref().unwrap().rgba8().unwrap();
    assert_eq!((color.r, color.g, color.b), (18, 52, 86));
    assert!((snapshot.atoms[0].radius - 0.9).abs() < 1e-6);
    script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); styles.setColor('accent',cuemol.makeColor('#22cc44'),{scope},{id}); styles.setStyleValue({scope},'appearance','Custom.sphr','0.4');"));
    let snapshot = state.active_render_snapshot("").unwrap();
    let color = snapshot.atoms[0].color.as_ref().unwrap().rgba8().unwrap();
    assert_eq!((color.r, color.g, color.b), (34, 204, 68));
    assert!((snapshot.atoms[0].radius - 0.4).abs() < 1e-6);
    let global = create_selection(&mut state, "global-appearance", 0);
    script(&mut state, &format!("cuemol.getService('StyleManager').setColor('sky',cuemol.makeColor('#123456'),0,{global});"));
    state
        .scene_manager_mut()
        .scene_mut(Uid::new(scope))
        .unwrap()
        .set_scene_attribute("bgcolor", "sky");
    let background = state.active_background_color().unwrap();
    for (value, byte) in background[..3].iter().zip([18., 52., 86.]) {
        assert!((*value - byte / 255.).abs() < 1e-6);
    }
}

#[test]
fn script_style_undo_and_redo_restore_drawing_and_identity() {
    let (mut state, object, renderer) = fixture();
    let scope = state.active_scene().unwrap().get();
    let id = create_selection(&mut state, "local", scope);
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    script(&mut state, &format!("scene.startUndoTxn('selection'); cuemol.getService('StyleManager').setStrData('sel','focus','name O1',{scope},{id}); scene.commitUndoTxn();"));
    assert_eq!(selected_names(&state), ["O1"]);
    script(&mut state, "scene.undo(0);");
    assert_eq!(selected_names(&state), ["C1"]);
    script(&mut state, "scene.redo(0);");
    assert_eq!(selected_names(&state), ["O1"]);
    assert_eq!(
        script(
            &mut state,
            &format!("print(cuemol.getService('StyleManager').hasStyleSet('local',{scope}));")
        ),
        format!("{id}\n")
    );
}

#[test]
fn newly_created_style_sets_keep_the_first_definition_in_precedence() {
    let (mut state, object, renderer) = fixture();
    let scope = state.active_scene().unwrap();
    let first = state.create_scene_style(scope, "first").unwrap();
    state
        .update_scene_style(
            scope,
            first,
            "first",
            false,
            "<styles id=\"first\"><sel id=\"focus\" value=\"name O1\"/></styles>",
        )
        .unwrap();
    let second = state.create_scene_style(scope, "second").unwrap();
    state
        .update_scene_style(
            scope,
            second,
            "second",
            false,
            "<styles id=\"second\"><sel id=\"focus\" value=\"name C1\"/></styles>",
        )
        .unwrap();
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    assert_eq!(selected_names(&state), ["O1"]);
    assert_eq!(
        script(
            &mut state,
            &format!(
                "print(cuemol.getService('StyleManager').getStrData('sel','focus',{}));",
                scope.get()
            )
        ),
        "name O1\n"
    );
}

#[test]
fn anonymous_style_handles_survive_insertion_and_removal_of_other_sets() {
    let (mut state, object, renderer) = fixture();
    let scope = state.active_scene().unwrap();
    let data = "<styles><sel id=\"focus\" value=\"name C1\"/></styles>".to_string();
    let node = cuemol_core::parse_ldom2_xml(&data).unwrap().top().clone();
    state
        .scene_manager_mut()
        .scene_mut(scope)
        .unwrap()
        .add_loaded_style(cuemol_molstr::LoadedStyle::new(node, data));
    let id = script(
        &mut state,
        &format!(
            "print(cuemol.getService('StyleManager').hasStyleSet('scene-style-0',{}));",
            scope.get()
        ),
    )
    .trim()
    .parse::<u64>()
    .unwrap();
    let index = state.create_scene_style(scope, "other").unwrap();
    assert_eq!(script(&mut state, &format!("print(cuemol.getService('StyleManager').getStyleSet({id}).getStrData('sel','focus'));")), "name C1\n");
    state.destroy_scene_style(scope, index).unwrap();
    script(&mut state, &format!("cuemol.getService('StyleManager').getStyleSet({id}).setStrData('sel','focus','name O1');"));
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    assert_eq!(selected_names(&state), ["O1"]);
    assert!(
        state.scene_manager().scene(scope).unwrap().loaded_styles()[0]
            .node
            .str_attr("id")
            .is_empty()
    );
}

#[test]
fn retained_python_style_handle_observes_gui_edits() {
    let (mut state, object, renderer) = fixture();
    let scope = state.active_scene().unwrap();
    let id = create_selection(&mut state, "python-local", scope.get());
    let run = |state: &mut DesktopAppState, source: &str| {
        script(
            state,
            &format!(
                "cuemol.getService('PythonBridge').runString({});",
                serde_json::to_string(source).unwrap()
            ),
        )
    };
    run(&mut state, &format!("import cuemol\nsaved_style = cuemol.getService('StyleManager').getStyleSet({id})\nassert saved_style.name == 'python-local'\n"));
    state
        .update_scene_style(
            scope,
            0,
            "renamed",
            false,
            "<styles id=\"renamed\"><sel id=\"focus\" value=\"name O1\"/></styles>",
        )
        .unwrap();
    run(&mut state, "assert saved_style.name == 'renamed'\nassert saved_style.getStrData('sel','focus') == 'name O1'\nsaved_style.setStrData('sel','focus','name C1')\n");
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    assert_eq!(selected_names(&state), ["C1"]);
}

#[test]
fn script_edits_preserve_linked_style_metadata_and_save_the_updated_data() {
    let (mut state, object, renderer) = fixture();
    let scope = state.active_scene().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("linked.xml");
    std::fs::write(
        &source,
        "<styles id=\"linked\"><sel id=\"focus\" value=\"name C1\"/></styles>",
    )
    .unwrap();
    let index = state.load_scene_style_file(scope, &source, false).unwrap();
    state
        .scene_manager_mut()
        .scene_mut(scope)
        .unwrap()
        .loaded_styles_mut()[index]
        .node
        .set_str_attr("alt_src", "fallback.xml");
    let id = script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); var id=styles.hasStyleSet('linked',{}); styles.setStrData('sel','focus','name O1',{},id); print(id);", scope.get(), scope.get()));
    let loaded = &state.scene_manager().scene(scope).unwrap().loaded_styles()[index];
    assert_eq!(
        loaded.node.str_attr("src").as_str(),
        source.to_str().unwrap()
    );
    assert_eq!(loaded.node.str_attr("alt_src").as_str(), "fallback.xml");
    assert_eq!(loaded.node.str_attr("modified").as_str(), "true");
    assert!(loaded.node.children().iter().all(|child| child.is_attr()));
    assert!(loaded.data.contains("name O1"));
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    assert_eq!(selected_names(&state), ["O1"]);
    let saved = dir.path().join("saved.xml");
    state.save_scene_style_file(scope, index, &saved).unwrap();
    assert!(std::fs::read_to_string(&saved).unwrap().contains("name O1"));
    assert_eq!(
        script(
            &mut state,
            &format!(
                "print(cuemol.getService('StyleManager').hasStyleSet('linked',{}));",
                scope.get()
            )
        ),
        id
    );
    std::fs::write(
        &saved,
        "<styles id=\"renamed\"><sel id=\"focus\" value=\"name C1\"/></styles>",
    )
    .unwrap();
    state.reload_scene_style_file(scope, index).unwrap();
    assert_eq!(
        script(
            &mut state,
            &format!(
                "print(cuemol.getService('StyleManager').hasStyleSet('renamed',{}));",
                scope.get()
            )
        ),
        id
    );
    assert_eq!(selected_names(&state), ["C1"]);
}

#[test]
fn script_style_save_checks_scope_and_updates_the_file_link() {
    let (mut state, _, _) = fixture();
    let scope = state.active_scene().unwrap().get();
    let id = create_selection(&mut state, "saved", scope);
    let dir = tempfile::tempdir().unwrap();
    let saved = dir.path().join("saved.xml");
    let path = serde_json::to_string(&saved).unwrap();
    assert_eq!(
        script(
            &mut state,
            &format!(
                "print(cuemol.getService('StyleManager').saveStyleSetToFile({}, {id}, {path}));",
                scope + 10000
            )
        ),
        "false\n"
    );
    assert!(!saved.exists());
    assert_eq!(script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); print(styles.saveStyleSetToFile({scope},{id},{path})); print(styles.getStyleSetSource({id})); print(styles.getStyleSet({id}).modified);")), format!("true\n{}\nfalse\n",saved.display()));
    let loaded = &state
        .scene_manager()
        .scene(Uid::new(scope))
        .unwrap()
        .loaded_styles()[0];
    assert_eq!(
        loaded.node.str_attr("src").as_str(),
        saved.to_str().unwrap()
    );
    assert_ne!(loaded.node.str_attr("modified").as_str(), "true");
    assert_eq!(script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); styles.setStrData('sel','focus','name O1',{scope},{id}); print(styles.saveStyleSetToFile({scope},{id},{path})); print(styles.getStyleSet({id}).modified);")), "true\ntrue\n");
    assert!(std::fs::read_to_string(&saved).unwrap().contains("name O1"));
}

#[test]
fn script_style_files_take_priority_and_reject_invalid_or_duplicate_ids() {
    let (mut state, object, renderer) = fixture();
    let scope = state.active_scene().unwrap().get();
    create_selection(&mut state, "first", scope);
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("different-filename.xml");
    std::fs::write(
        &source,
        "<styles id=\"loaded\"><sel id=\"focus\" value=\"name O1\"/></styles>",
    )
    .unwrap();
    let path = serde_json::to_string(&source).unwrap();
    let id = script(
        &mut state,
        &format!(
            "print(cuemol.getService('StyleManager').loadStyleSetFromFile({scope},{path},true));"
        ),
    )
    .trim()
    .parse::<u64>()
    .unwrap();
    state
        .set_renderer_selection_text(object, renderer, "focus")
        .unwrap();
    assert_eq!(selected_names(&state), ["O1"]);
    assert_eq!(script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); print(styles.getStyleSet({id}).name); print(styles.getStyleSet({id}).readonly); try {{ styles.loadStyleSetFromFile({scope},{path},false); print('unexpected success'); }} catch(error) {{ print('duplicate rejected'); }} print(JSON.parse(styles.getStyleSetsJSON({scope})).length);")), "loaded\ntrue\nduplicate rejected\n2\n");
    for contents in [
        "<styles><sel id=\"focus\" value=\"name C1\"/></styles>",
        "<invalid id=\"invalid\"/>",
        "<styles id=\"broken\">",
    ] {
        std::fs::write(&source, contents).unwrap();
        assert_eq!(script(&mut state, &format!("var styles=cuemol.getService('StyleManager'); try {{ styles.loadStyleSetFromFile({scope},{path},false); print('unexpected success'); }} catch(error) {{ print('invalid rejected'); }} print(JSON.parse(styles.getStyleSetsJSON({scope})).length);")), "invalid rejected\n2\n");
    }
    assert_eq!(selected_names(&state), ["O1"]);
}
