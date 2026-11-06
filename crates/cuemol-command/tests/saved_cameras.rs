use cuemol_command::call_tool;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;

fn fixture() -> (DesktopAppState, Camera) {
    let mut state = DesktopAppState::new();
    state.new_scene("cameras", true);
    (state, Camera::new())
}

fn names(state: &DesktopAppState) -> Vec<String> {
    state
        .active_camera_summaries()
        .into_iter()
        .map(|camera| camera.name)
        .collect()
}

#[test]
fn saved_camera_overwrite_clears_visibility_and_preserves_its_source_and_slot() {
    let (mut state, mut view) = fixture();
    let scene = state.active_scene().unwrap();
    let object = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("mol", Default::default());
    call_tool(
        &mut state,
        &mut view,
        "save_camera",
        &json!({"name":"front","withVisibility":true}),
    )
    .unwrap();
    call_tool(
        &mut state,
        &mut view,
        "save_camera",
        &json!({"name":"back"}),
    )
    .unwrap();
    let mut saved = state
        .scene_manager()
        .scene(scene)
        .unwrap()
        .camera_by_name("front")
        .unwrap()
        .clone();
    saved.set_source("front.cam");
    state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .set_camera(saved);
    call_tool(
        &mut state,
        &mut view,
        "apply_camera",
        &json!({"name":"back"}),
    )
    .unwrap();
    view.vis_append(object.get(), false, true);
    call_tool(
        &mut state,
        &mut view,
        "save_camera",
        &json!({"name":"front"}),
    )
    .unwrap();
    let saved = state
        .scene_manager()
        .scene(scene)
        .unwrap()
        .camera_by_name("front")
        .unwrap();
    assert_eq!(saved.visibility_size(), 0);
    assert_eq!(saved.source(), "front.cam");
    assert_eq!(names(&state), ["front", "back"]);
    call_tool(
        &mut state,
        &mut view,
        "save_camera",
        &json!({"name":"front","withVisibility":true}),
    )
    .unwrap();
    assert_eq!(state.active_camera_summaries()[0].visibility_count, 1);
    call_tool(
        &mut state,
        &mut view,
        "apply_camera",
        &json!({"name":"front","withVisibility":true}),
    )
    .unwrap();
    assert_eq!(view.visibility_size(), 0);
    assert_eq!(view.source(), "");
    assert_eq!(state.active_camera_summaries()[0].visibility_count, 1);
}

#[test]
fn camera_moves_pin_current_and_survive_subsequent_edits() {
    let (mut state, mut view) = fixture();
    for name in ["first", "__current", "second", "third"] {
        call_tool(&mut state, &mut view, "save_camera", &json!({"name":name})).unwrap();
    }
    assert_eq!(names(&state), ["__current", "first", "second", "third"]);
    call_tool(
        &mut state,
        &mut view,
        "move_camera",
        &json!({"name":"third","end":"top"}),
    )
    .unwrap();
    assert_eq!(names(&state), ["__current", "third", "first", "second"]);
    call_tool(
        &mut state,
        &mut view,
        "rename_camera",
        &json!({"name":"first","newName":" renamed "}),
    )
    .unwrap();
    call_tool(
        &mut state,
        &mut view,
        "save_camera",
        &json!({"name":"third"}),
    )
    .unwrap();
    assert_eq!(names(&state), ["__current", "third", "renamed", "second"]);
    let before = state.clone();
    for args in [
        json!({"name":"third","end":"top"}),
        json!({"name":"__current","end":"bottom"}),
    ] {
        call_tool(&mut state, &mut view, "move_camera", &args).unwrap();
        assert!(before.same_undo_state(&state));
    }
    call_tool(
        &mut state,
        &mut view,
        "move_camera",
        &json!({"name":"third","to":4}),
    )
    .unwrap();
    assert_eq!(names(&state), ["__current", "renamed", "second", "third"]);
}

#[test]
fn camera_xml_and_property_replacement_keep_metadata_and_reject_invalid_targets() {
    let (mut state, mut view) = fixture();
    let scene = state.active_scene().unwrap();
    let object = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("mol", Default::default());
    call_tool(
        &mut state,
        &mut view,
        "save_camera",
        &json!({"name":"front","withVisibility":true}),
    )
    .unwrap();
    call_tool(
        &mut state,
        &mut view,
        "save_camera",
        &json!({"name":"back"}),
    )
    .unwrap();
    let mut camera = state
        .scene_manager()
        .scene(scene)
        .unwrap()
        .camera_by_name("front")
        .unwrap()
        .clone();
    camera.set_source("saved camera.cam");
    camera.set_name("renamed");
    camera.set_zoom(77.);
    state.replace_scene_camera(scene, "front", camera).unwrap();
    let xml = state.scene_camera_xml(scene, "renamed").unwrap();
    assert!(xml.contains("mol"));
    let name = state
        .import_scene_camera_xml(scene, "new camera.cam", &xml, None)
        .unwrap();
    assert_eq!(name, "copy1_renamed");
    let imported = state
        .scene_manager()
        .scene(scene)
        .unwrap()
        .camera_by_name(&name)
        .unwrap();
    assert_eq!(imported.zoom(), 77.);
    assert_eq!(imported.source(), "new camera.cam");
    assert_eq!(
        imported.visibility_settings().collect::<Vec<_>>(),
        [(object.get(), true, true)]
    );
    let before = state.clone();
    assert!(state
        .import_scene_camera_xml(scene, "bad.cam", "<invalid", None)
        .is_err());
    assert!(before.same_undo_state(&state));
    assert!(state
        .import_scene_camera_xml(scene, "new.cam", &xml, Some("missing"))
        .is_err());
    assert!(before.same_undo_state(&state));
    state
        .import_scene_camera_xml(scene, "new.cam", &xml, Some("back"))
        .unwrap();
    assert_eq!(names(&state), ["renamed", "back", "copy1_renamed"]);
    let mut collision = state
        .scene_manager()
        .scene(scene)
        .unwrap()
        .camera_by_name("back")
        .unwrap()
        .clone();
    collision.set_name("renamed");
    let before = state.clone();
    assert!(state
        .replace_scene_camera(scene, "back", collision)
        .is_err());
    assert!(before.same_undo_state(&state));
}
