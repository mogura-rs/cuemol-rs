use cuemol_command::{call_tool, Protocol};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};

fn fixture() -> (DesktopAppState, Camera) {
    let mut state = DesktopAppState::new();
    state.new_scene("animation", true);
    let mut camera = Camera::new();
    camera.set_zoom(87.);
    (state, camera)
}

fn call(
    protocol: &mut Protocol,
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: Value,
) -> Value {
    let result = protocol.handle(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}),state,camera).unwrap();
    assert_eq!(result["result"]["isError"], false, "{result}");
    result
}

#[test]
fn adding_animation_anchors_the_authoring_view_before_playback() {
    let (mut state, mut camera) = fixture();
    let scene = state.active_scene().unwrap();
    call_tool(&mut state, &mut camera, "add_anim", &json!({"type":"spin"})).unwrap();
    assert_eq!(
        state.scene_animation(scene).unwrap().unwrap().startcam(),
        "__current"
    );
    assert_eq!(
        state
            .scene_manager()
            .scene(scene)
            .unwrap()
            .camera_by_name("__current")
            .unwrap()
            .zoom(),
        87.
    );
    camera.set_zoom(23.);
    call_tool(
        &mut state,
        &mut camera,
        "animate",
        &json!({"action":"seek","timeMs":0}),
    )
    .unwrap();
    assert_eq!(camera.zoom(), 87.);
}

#[test]
fn animation_start_camera_is_retained_when_undoing_the_first_add() {
    let (mut state, mut camera) = fixture();
    let scene = state.active_scene().unwrap();
    let mut protocol = Protocol::new();
    protocol
        .handle(
            json!({"jsonrpc":"2.0","id":0,"method":"initialize"}),
            &mut state,
            &mut camera,
        )
        .unwrap();
    call(
        &mut protocol,
        &mut state,
        &mut camera,
        "add_anim",
        json!({"type":"wait"}),
    );
    call(&mut protocol, &mut state, &mut camera, "undo", json!({}));
    let animation = state.scene_animation(scene).unwrap().unwrap();
    assert_eq!(animation.size(), 0);
    assert_eq!(animation.startcam(), "__current");
    assert_eq!(
        state
            .scene_manager()
            .scene(scene)
            .unwrap()
            .camera_by_name("__current")
            .unwrap()
            .zoom(),
        87.
    );
    camera.set_zoom(12.);
    call(&mut protocol, &mut state, &mut camera, "redo", json!({}));
    assert_eq!(state.scene_animation(scene).unwrap().unwrap().size(), 1);
    assert_eq!(camera.zoom(), 12.);
    assert_eq!(
        state
            .scene_manager()
            .scene(scene)
            .unwrap()
            .camera_by_name("__current")
            .unwrap()
            .zoom(),
        87.
    );
}

#[test]
fn animation_add_preserves_existing_camera_and_explicit_start_choice() {
    let (mut state, mut camera) = fixture();
    let scene = state.active_scene().unwrap();
    call_tool(
        &mut state,
        &mut camera,
        "save_camera",
        &json!({"name":"chosen"}),
    )
    .unwrap();
    call_tool(
        &mut state,
        &mut camera,
        "set_anim_options",
        &json!({"startCamera":"chosen"}),
    )
    .unwrap();
    camera.set_zoom(23.);
    call_tool(&mut state, &mut camera, "add_anim", &json!({"type":"wait"})).unwrap();
    assert_eq!(
        state.scene_animation(scene).unwrap().unwrap().startcam(),
        "chosen"
    );
    assert_eq!(
        state
            .scene_manager()
            .scene(scene)
            .unwrap()
            .camera_by_name("__current")
            .unwrap()
            .zoom(),
        23.
    );
    camera.set_zoom(16.);
    call_tool(&mut state, &mut camera, "add_anim", &json!({"type":"wait"})).unwrap();
    assert_eq!(
        state
            .scene_manager()
            .scene(scene)
            .unwrap()
            .camera_by_name("__current")
            .unwrap()
            .zoom(),
        23.
    );
}

#[test]
fn animation_add_records_explicit_show_defaults_and_rejects_empty_resets() {
    let (mut state, mut camera) = fixture();
    let scene = state.active_scene().unwrap();
    for kind in ["show", "slidein"] {
        call_tool(&mut state, &mut camera, "add_anim", &json!({"type":kind})).unwrap();
        let animation = state.scene_animation(scene).unwrap().unwrap();
        assert!(!animation
            .motions()
            .last()
            .unwrap()
            .is_property_default("hide"));
    }
    for props in ["", ", ,", "hide fade"] {
        let before = state.clone();
        assert!(
            call_tool(
                &mut state,
                &mut camera,
                "reset_anim_prop",
                &json!({"element":"1","props":props})
            )
            .is_err(),
            "{props:?}"
        );
        assert!(before.same_undo_state(&state));
    }
}

#[test]
fn rejected_animation_add_does_not_create_a_start_camera() {
    let (mut state, mut camera) = fixture();
    for args in [
        json!({"type":"wait","name":" "}),
        json!({"type":"wait","before":"missing"}),
        json!({"type":"invalid"}),
    ] {
        let before = state.clone();
        assert!(call_tool(&mut state, &mut camera, "add_anim", &args).is_err());
        assert!(before.same_undo_state(&state));
    }
}
