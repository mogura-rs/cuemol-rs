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
