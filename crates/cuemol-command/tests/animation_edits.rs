use cuemol_command::call_tool;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};

fn call(state: &mut DesktopAppState, camera: &mut Camera, tool: &str, args: Value) -> Value {
    call_tool(state, camera, tool, &args).unwrap_or_else(|error| panic!("{tool} {args}: {error}"))
}

fn fixture() -> (DesktopAppState, Camera) {
    let mut state = DesktopAppState::new();
    state.new_scene("animation", true);
    let mut camera = Camera::new();
    for name in ["base", "child", "last"] {
        call(
            &mut state,
            &mut camera,
            "add_anim",
            json!({"type":"wait","name":name}),
        );
    }
    (state, camera)
}

#[test]
fn renaming_an_animation_keeps_its_dependents_and_identity() {
    let (mut state, mut camera) = fixture();
    let before = call(&mut state, &mut camera, "list_anims", json!({}));
    call(
        &mut state,
        &mut camera,
        "set_anim_prop",
        json!({"element":"base","prop":"name","value":" renamed "}),
    );
    let after = call(&mut state, &mut camera, "list_anims", json!({}));
    assert_eq!(after["elements"][0]["name"], "renamed");
    assert_eq!(after["elements"][1]["timeRefName"], "renamed");
    for i in 0..3 {
        for key in ["uid", "absStartMs", "absEndMs"] {
            assert_eq!(after["elements"][i][key], before["elements"][i][key]);
        }
    }
    for name in ["", "child"] {
        let anchor = state.clone();
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_anim_generic_prop",
            &json!({"element":"renamed","prop":"name","value":name})
        )
        .is_err());
        assert!(anchor.same_undo_state(&state));
    }
}

#[test]
fn deleting_an_animation_detaches_children_at_their_absolute_times() {
    let (mut state, mut camera) = fixture();
    let before = call(&mut state, &mut camera, "list_anims", json!({}));
    call(
        &mut state,
        &mut camera,
        "remove_anim",
        json!({"element":"base"}),
    );
    let after = call(&mut state, &mut camera, "list_anims", json!({}));
    assert_eq!(after["elements"][0]["timeRefName"], "");
    for i in 0..2 {
        for key in ["uid", "absStartMs", "absEndMs"] {
            assert_eq!(after["elements"][i][key], before["elements"][i + 1][key]);
        }
    }
    assert_eq!(after["elements"][0]["startMs"], 1000.);
    assert_eq!(after["mgr"]["lengthMs"], 3000.);
}

#[test]
fn changing_an_animation_reference_preserves_its_span_and_rejects_cycles() {
    let (mut state, mut camera) = fixture();
    call(
        &mut state,
        &mut camera,
        "set_anim_prop",
        json!({"element":"child","prop":"timeRefName","value":""}),
    );
    let detached = call(&mut state, &mut camera, "list_anims", json!({}));
    assert_eq!(detached["elements"][1]["startMs"], 1000.);
    assert_eq!(detached["elements"][1]["endMs"], 2000.);
    let anchor = state.clone();
    for name in ["child", "last", "missing"] {
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_anim_prop",
            &json!({"element":"child","prop":"timeRefName","value":name})
        )
        .is_err());
        assert!(anchor.same_undo_state(&state));
    }
    call(
        &mut state,
        &mut camera,
        "set_anim_time",
        json!({"element":"base","startMs":3000.,"endMs":4000.}),
    );
    call(
        &mut state,
        &mut camera,
        "set_anim_generic_prop",
        json!({"element":"child","prop":"timeRefName","value":"base"}),
    );
    let rebased = call(&mut state, &mut camera, "list_anims", json!({}));
    assert_eq!(rebased["elements"][1]["startMs"], 0.);
    assert_eq!(rebased["elements"][1]["endMs"], 1000.);
    assert_eq!(rebased["elements"][1]["absStartMs"], 4000.);
    assert_eq!(rebased["elements"][2]["absStartMs"], 5000.);
}

#[test]
fn a_broken_animation_chain_does_not_block_an_unrelated_edit() {
    let (mut state, mut camera) = fixture();
    let scene = state.active_scene().unwrap();
    let mut manager = state.scene_animation(scene).unwrap().unwrap();
    manager.motions_mut()[2]
        .base_mut()
        .set_time_ref_name("missing");
    state.set_scene_animation(scene, Some(&manager)).unwrap();
    call(
        &mut state,
        &mut camera,
        "set_anim_prop",
        json!({"element":"base","prop":"disabled","value":"true"}),
    );
    let after = call(&mut state, &mut camera, "list_anims", json!({}));
    assert_eq!(after["elements"][0]["disabled"], true);
    assert_eq!(after["elements"][2]["timeRefState"], "missing");
    assert!(after["resolveError"].as_str().unwrap().contains("missing"));
}

#[test]
fn animation_generic_timing_and_unknown_resets_are_rejected_atomically() {
    let (mut state, mut camera) = fixture();
    for (tool, args) in [
        (
            "set_anim_generic_prop",
            json!({"element":"child", "prop":"start", "value":"5000000"}),
        ),
        (
            "set_anim_generic_prop",
            json!({"element":"child", "prop":"end", "value":"5000000"}),
        ),
        (
            "reset_anim_prop",
            json!({"element":"child", "props":"disabled,timeRefName"}),
        ),
        (
            "reset_anim_prop",
            json!({"element":"child", "props":"does_not_exist"}),
        ),
    ] {
        let before = state.clone();
        assert!(call_tool(&mut state, &mut camera, tool, &args).is_err());
        assert!(before.same_undo_state(&state));
    }
}

#[test]
fn animation_opacity_alias_changes_the_persistent_native_property() {
    let (mut state, mut camera) = fixture();
    call(
        &mut state,
        &mut camera,
        "add_anim",
        json!({"type":"show","name":"visible"}),
    );
    call(
        &mut state,
        &mut camera,
        "set_anim_prop",
        json!({"element":"visible","prop":"tgtAlpha","value":"0.35"}),
    );
    let manager = state
        .scene_animation(state.active_scene().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        manager.motions()[3]
            .to_ldom2_node()
            .str_attr("tgt_alpha")
            .as_str(),
        "0.35"
    );
    call(
        &mut state,
        &mut camera,
        "reset_anim_prop",
        json!({"element":"visible","props":"tgt_alpha"}),
    );
    let manager = state
        .scene_animation(state.active_scene().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        manager.motions()[3]
            .to_ldom2_node()
            .str_attr("tgt_alpha")
            .as_str(),
        "1"
    );
}

#[test]
fn an_unresolved_loaded_animation_can_be_inspected_and_repaired() {
    let (mut state, mut camera) = fixture();
    let scene = state.active_scene().unwrap();
    let mut manager = state.scene_animation(scene).unwrap().unwrap();
    manager.motions_mut()[1]
        .base_mut()
        .set_time_ref_name("missing");
    state.set_scene_animation(scene, Some(&manager)).unwrap();
    let bytes = state.active_scene_bytes().unwrap();
    let mut loaded = DesktopAppState::new();
    loaded.open_bytes("unresolved.qsc", &bytes).unwrap();
    let timeline = call(&mut loaded, &mut camera, "list_anims", json!({}));
    assert_eq!(timeline["elements"][1]["timeRefState"], "missing");
    assert_eq!(timeline["elements"][2]["timeRefState"], "upstream");
    let before = loaded.clone();
    assert!(call_tool(
        &mut loaded,
        &mut camera,
        "animate",
        &json!({"action":"seek", "timeMs":500.})
    )
    .is_err());
    assert!(before.same_undo_state(&loaded));
    call(
        &mut loaded,
        &mut camera,
        "set_anim_prop",
        json!({"element":"child", "prop":"timeRefName", "value":"base"}),
    );
    let fixed = call(&mut loaded, &mut camera, "list_anims", json!({}));
    assert!(fixed.get("resolveError").is_none());
    assert_eq!(fixed["elements"][2]["absStartMs"], 2000.);
}

#[test]
fn animation_edits_stop_only_after_validation_succeeds() {
    let (mut state, mut camera) = fixture();
    let scene = state.active_scene().unwrap();
    call(&mut state, &mut camera, "animate", json!({"action":"play"}));
    assert!(state.is_scene_animation_playing(scene));
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_anim_prop",
        &json!({"element":"base","prop":"timeRefName","value":"last"})
    )
    .is_err());
    assert!(state.is_scene_animation_playing(scene));
    call(
        &mut state,
        &mut camera,
        "set_anim_prop",
        json!({"element":"base","prop":"disabled","value":"true"}),
    );
    assert!(!state.is_scene_animation_playing(scene));
    assert!(!state.has_scene_animation_player(scene));
}

#[test]
fn animation_timing_writes_round_like_the_reference_editor() {
    let (mut state, mut camera) = fixture();
    call(
        &mut state,
        &mut camera,
        "set_anim_time",
        json!({"element":"base", "startMs":1000.5, "endMs":2000.49}),
    );
    let timeline = call(&mut state, &mut camera, "list_anims", json!({}));
    assert_eq!(timeline["elements"][0]["startMs"], 1001.);
    assert_eq!(timeline["elements"][0]["endMs"], 2000.);
    assert_eq!(timeline["elements"][1]["absStartMs"], 2000.);
}

#[test]
fn signed_animation_offsets_and_typed_spin_axes_match_reference_commands() {
    let (mut state, mut camera) = fixture();
    call(
        &mut state,
        &mut camera,
        "set_anim_time",
        json!({"element":"child","startMs":-250,"endMs":750}),
    );
    let timeline = call(&mut state, &mut camera, "list_anims", json!({}));
    assert_eq!(timeline["elements"][1]["absStartMs"], 750.);
    call(
        &mut state,
        &mut camera,
        "add_anim",
        json!({"type":"spin","name":"spin"}),
    );
    call(
        &mut state,
        &mut camera,
        "set_anim_prop",
        json!({"element":"spin","prop":"axis","value":"1 0 0"}),
    );
    let manager = state
        .scene_animation(state.active_scene().unwrap())
        .unwrap()
        .unwrap();
    let spin = manager
        .motions()
        .iter()
        .find(|motion| motion.base().name() == "spin")
        .unwrap();
    let detail = cuemol_command::animation_element_detail(&state, spin.base().uid().get()).unwrap();
    assert_eq!(detail["properties"]["axis"], "(1,0,0)");
    let before = state.clone();
    assert!(call_tool(
        &mut state,
        &mut camera,
        "set_anim_prop",
        &json!({"element":"spin","prop":"axis","value":"1 NaN 0"})
    )
    .is_err());
    assert!(before.same_undo_state(&state));
}
