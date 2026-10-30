use cuemol_command::{
    call_tool,
    selection::{self, Draft},
};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;

fn fixture() -> (DesktopAppState, Uid) {
    let mut state = DesktopAppState::new();
    state.new_scene("selection", true);
    state.open_bytes("selection.pdb", b"ATOM      1  CA AGLY A  -2       0.000   0.000   0.000  1.00 10.00           C\nATOM      2  N   GLY A  -2       1.000   0.000   0.000  1.00 20.00           N\nATOM      3  O   ALA a  20A      4.000   0.000   0.000  1.00 30.00           O\nEND\n").unwrap();
    let id = state.active_object_summaries()[0].uid;
    (state, id)
}

#[test]
fn named_selection_catalog_follows_active_scene_and_includes_global_definitions() {
    let (mut state, _) = fixture();
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"name":"catalog_probe","selection":"name O"}),
    )
    .unwrap();
    let before = state.clone();
    let names = call_tool(&mut state, &mut camera, "get_scene_state", &json!({})).unwrap();
    assert!(names["namedSelections"]
        .as_array()
        .unwrap()
        .contains(&json!("catalog_probe")));
    assert!(names["namedSelections"]
        .as_array()
        .unwrap()
        .contains(&json!("protein")));
    assert!(state.same_undo_state(&before));
    state.new_scene("other", true);
    let names = call_tool(&mut state, &mut camera, "get_scene_state", &json!({})).unwrap();
    assert!(!names["namedSelections"]
        .as_array()
        .unwrap()
        .contains(&json!("catalog_probe")));
}

#[test]
fn named_selection_edits_and_analysis_use_the_target_molecules_scene() {
    let (mut state, id) = fixture();
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"name":"picked","selection":"name O"}),
    )
    .unwrap();
    state.new_scene("other", true);
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"name":"picked","selection":"name CA"}),
    )
    .unwrap();
    let mut deleted = state.clone();
    assert_eq!(deleted.delete_atoms_in_object(id, "picked").unwrap(), 1);
    assert!(deleted
        .scene_manager()
        .object(id)
        .unwrap()
        .mol()
        .atoms()
        .iter()
        .all(|atom| atom.name.trim() != "O"));
    let mut renamed = state.clone();
    assert_eq!(
        renamed.change_chain_in_object(id, "picked", "Z").unwrap(),
        1
    );
    assert_eq!(
        renamed.scene_manager().object(id).unwrap().mol().atoms()[2].chain_id,
        "Z"
    );
    let mut renumbered = state.clone();
    assert_eq!(
        renumbered
            .renumber_residues_in_object(id, "picked", 77)
            .unwrap(),
        1
    );
    assert_eq!(
        renumbered.scene_manager().object(id).unwrap().mol().atoms()[2].res_id,
        77
    );
    let direct = state
        .calc_atom_contacts_in_object(id, "name O", 0., 5., false, 100)
        .unwrap();
    assert_eq!(
        state
            .calc_atom_contacts_in_object(id, "picked", 0., 5., false, 100)
            .unwrap(),
        direct
    );
    let mesh = state.build_surface_mesh(id, "picked", 1., 1.4).unwrap();
    let direct = state.build_surface_mesh(id, "name O", 1., 1.4).unwrap();
    assert!(!mesh.vertices.is_empty());
    assert_eq!(mesh.vertices, direct.vertices);
    assert_eq!(mesh.indices, direct.indices);
}

#[cfg(feature = "scripting")]
#[test]
fn named_selection_script_measure_uses_scene_definitions() {
    let (mut state, _) = fixture();
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"name":"pair","selection":"name CA,N"}),
    )
    .unwrap();
    let direct = state
        .run_script_js("cuemol.print(cuemol.measure('name CA,N'));")
        .unwrap();
    let named = state
        .run_script_js("cuemol.print(cuemol.measure('pair'));")
        .unwrap();
    assert_eq!(named.0, direct.0);
    assert!(!named.0.contains("null"));
}

#[test]
fn superposition_resolves_each_molecules_named_selection_in_its_own_scene() {
    let (mut state, reference) = fixture();
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"name":"fit_atoms","selection":"*"}),
    )
    .unwrap();
    let molecule = state
        .scene_manager()
        .object(reference)
        .unwrap()
        .mol()
        .clone();
    let scene = state.new_scene("moving", true).result_scene;
    let moving = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("moving", molecule);
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"name":"fit_atoms","selection":"none"}),
    )
    .unwrap();
    let fit = state
        .prepare_superposition(reference, "fit_atoms", moving, "*", "lsq", "CA")
        .unwrap();
    assert_eq!(fit.aligned_count, Some(3));
    assert!(fit.rmsd.unwrap() < 1e-9);
    let report = tempfile::tempdir().unwrap();
    let rmsd = state
        .write_superposition_rmsd_report(
            reference,
            "fit_atoms",
            moving,
            "*",
            report.path().join("rmsd.txt"),
        )
        .unwrap();
    assert!(rmsd < 1e-9);
}

#[test]
fn applied_named_selections_follow_definitions_in_sequences_and_overlays() {
    let (mut state, id) = fixture();
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"name":"picked","selection":"name O"}),
    )
    .unwrap();
    state.set_molecule_selection_text(id, "picked").unwrap();
    assert!(state.active_render_snapshot("picked").is_ok());
    let selected = |state: &DesktopAppState| {
        state
            .active_sequence_view("")
            .into_iter()
            .flat_map(|chain| chain.residues)
            .filter(|residue| residue.selected)
            .map(|residue| residue.residue_index)
            .collect::<Vec<_>>()
    };
    assert_eq!(selected(&state), ["20A"]);
    let overlay = |state: &DesktopAppState| {
        state
            .active_render_snapshot("")
            .unwrap()
            .bonds
            .into_iter()
            .filter(|bond| bond.screen_offset != [0., 0.])
            .count()
    };
    assert!(overlay(&state) > 0);
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"name":"picked","selection":"name CA"}),
    )
    .unwrap();
    assert_eq!(selected(&state), ["-2"]);
    assert_eq!(state.molecule_selection_text(id).unwrap(), "picked");
    assert!(overlay(&state) > 0);
}

#[test]
fn pane_definitions_use_writable_scene_styles_and_preserve_locked_sets() {
    let (mut state, id) = fixture();
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "create_style_set",
        &json!({"name":"default"}),
    )
    .unwrap();
    call_tool(
        &mut state,
        &mut camera,
        "set_style_readonly",
        &json!({"styleSet":"default","readonly":true}),
    )
    .unwrap();
    let mut args = json!({"name":"carbon","selection":"name CA","styleSet":selection::definition_style_set(&state)});
    assert_eq!(args["styleSet"], "default_0");
    call_tool(&mut state, &mut camera, "define_selection", &args).unwrap();
    assert_eq!(
        selection::evaluate(&state, Some(id), "carbon").count,
        Some(1)
    );
    call_tool(
        &mut state,
        &mut camera,
        "set_style_readonly",
        &json!({"styleSet":"default_0","readonly":true}),
    )
    .unwrap();
    call_tool(
        &mut state,
        &mut camera,
        "create_style_set",
        &json!({"name":"custom"}),
    )
    .unwrap();
    args["styleSet"] = json!(selection::definition_style_set(&state));
    args["name"] = json!("oxygen");
    args["selection"] = json!("name O");
    assert_eq!(args["styleSet"], "custom");
    call_tool(&mut state, &mut camera, "define_selection", &args).unwrap();
    let styles = state.style_summaries(state.active_scene().unwrap());
    assert!(styles
        .iter()
        .filter(|style| style.name.starts_with("default"))
        .all(|style| style.read_only));
    assert_eq!(
        selection::evaluate(&state, Some(id), "oxygen").count,
        Some(1)
    );
}

#[test]
fn composed_terms_preserve_case_ranges_and_operator_grouping() {
    let (state, id) = fixture();
    let mut draft = Draft::default();
    draft.fields.insert("chain".into(), " A , ".into());
    draft.fields.insert("resid".into(), " -2 : -1 ".into());
    assert_eq!(draft.term().as_deref(), Some("'A'.-2:-1.*"));
    let term = draft.apply("", "set").unwrap();
    assert_eq!(selection::evaluate(&state, Some(id), &term).count, Some(2));
    draft.set_keyword("chain");
    draft.fields.insert("value".into(), "a".into());
    assert_eq!(
        selection::evaluate(&state, Some(id), &draft.term().unwrap()).count,
        Some(1)
    );
    for (op, count) in [("set", 1), ("add", 3), ("sub", 2), ("intersect", 0)] {
        let expression = draft.apply(&term, op).unwrap();
        assert_eq!(
            selection::evaluate(&state, Some(id), &expression).count,
            Some(count),
            "{expression}"
        );
    }
    assert!(draft.apply("", "add").is_none());
    assert_eq!(
        draft.apply(&term, "not").as_deref(),
        Some("not ('A'.-2:-1.*)")
    );
    assert_eq!(
        selection::evaluate(&state, Some(id), &draft.apply(&term, "not").unwrap()).count,
        Some(1)
    );
    draft.distance = "NaN".into();
    assert!(draft.apply(&term, "around").is_none());
    draft.distance = "-1".into();
    assert!(draft.apply(&term, "expand").is_none());
    draft.distance = "5".into();
    assert_eq!(
        draft.apply(&term, "expand").as_deref(),
        Some("('A'.-2:-1.*) expand 5")
    );
    draft.set_keyword("hierarchical");
    draft.fields.insert("chain".into(), "*,1,/a/,'B'".into());
    assert_eq!(draft.term().as_deref(), Some("*,1,/a/,'B'.*.*"));
}

#[test]
fn context_and_counts_are_read_only_and_follow_the_target_scene() {
    let (mut state, id) = fixture();
    state.set_molecule_selection_text(id, "name CA").unwrap();
    let mut camera = Camera::new();
    call_tool(
        &mut state,
        &mut camera,
        "define_selection",
        &json!({"name":"chosen","selection":"chain 'a'"}),
    )
    .unwrap();
    let before = state.clone();
    let context = selection::context(&state, Some(id));
    assert_eq!(context.current, "name CA");
    assert!(context.scene.iter().any(|v| v == "chosen"));
    assert!(!context.global.iter().any(|v| v == "chosen"));
    assert!(context.global.iter().any(|v| v == "protein"));
    assert_eq!(selection::values(&state, Some(id), "chain"), ["A", "a"]);
    assert_eq!(
        selection::values(&state, Some(id), "aname"),
        ["CA", "N", "O"]
    );
    assert_eq!(
        selection::evaluate(&state, Some(id), "chosen").count,
        Some(1)
    );
    assert!(!selection::evaluate(&state, Some(id), "chain (").valid);
    assert!(selection::evaluate(&state, None, "protein").count.is_none());
    assert!(before.same_undo_state(&state));
    state.new_scene("other", true);
    assert!(
        selection::request(&state, &json!({"action":"context", "moleculeId":id.get()})).is_err()
    );
    assert!(selection::context(&state, None).scene.is_empty());
}

#[test]
fn selection_history_is_shared_bounded_and_persists_without_changing_the_scene() {
    let (mut state, _) = fixture();
    let before = state.clone();
    for n in 0..102 {
        state.remember_selection(&format!("resid {n}"));
    }
    for value in ["", "*", "none"] {
        state.remember_selection(value);
    }
    state.remember_selection(" resid 5 ");
    let history = state.selection_history();
    assert_eq!(history.len(), 100);
    assert_eq!(history[..2], ["resid 5", "resid 101"]);
    assert!(!history.iter().any(|value| value == "resid 1"));
    assert_eq!(
        before.scene_manager().scenes(),
        state.scene_manager().scenes()
    );
    let mut restored = DesktopAppState::new();
    restored.load_config_text(&state.config_text()).unwrap();
    assert_eq!(restored.selection_history(), history);
}

#[test]
fn selection_metadata_matches_renderer_editability() {
    let (mut state, object) = fixture();
    let mut camera = Camera::new();
    for (kind, readonly) in [
        ("tube", false),
        ("*namelabel", true),
        ("*selection", true),
        ("*group", true),
    ] {
        let renderer = state
            .scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .add_renderer(kind, kind);
        let result = call_tool(
            &mut state,
            &mut camera,
            "list_node_props",
            &json!({"nodeType":"renderer","nodeId":renderer.get()}),
        )
        .unwrap();
        let row = result["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"] == "sel")
            .unwrap();
        assert_eq!(row["readonly"], readonly, "{kind}");
        assert_eq!(
            state
                .set_renderer_selection_text(object, renderer, "name CA")
                .is_err(),
            readonly,
            "{kind}"
        );
    }
}
