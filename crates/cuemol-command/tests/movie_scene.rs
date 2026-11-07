use cuemol_command::{call_tool, movie::MovieScene};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;

fn animated_scene() -> (DesktopAppState, Camera) {
    let mut state = DesktopAppState::new();
    state.new_scene("Movie", true);
    let mut camera = Camera::new();
    camera.set_zoom(73.);
    call_tool(&mut state, &mut camera, "add_anim", &json!({"type":"spin"})).unwrap();
    (state, camera)
}

#[test]
fn movie_frames_keep_live_edits_and_the_captured_camera_independent() {
    let (mut state, camera) = animated_scene();
    let before = state.clone();
    let mut movie = MovieScene::capture(&state, &camera, &json!({})).unwrap();
    movie.seek(movie.duration() / 4.).unwrap();
    assert!(state.same_undo_state(&before));
    assert_ne!(movie.camera().rotation(), camera.rotation());
    state.new_scene("Edited while rendering", true);
    let edited = state.clone();
    movie.seek(movie.duration() / 2.).unwrap();
    assert!(state.same_undo_state(&edited));
    assert_eq!(movie.camera().zoom(), 73.);
    drop(movie);
    assert!(state.same_undo_state(&edited));
}

#[test]
fn movie_target_uses_its_own_view_and_does_not_switch_the_live_scene() {
    let (mut state, camera) = animated_scene();
    let scene = state.active_scene().unwrap();
    let view = state.active_view_summary().unwrap().uid;
    state.set_active_view_camera(&camera).unwrap();
    state.new_scene("Active scene", true);
    let before = state.clone();
    let mut active_camera = Camera::new();
    active_camera.set_zoom(11.);
    let mut movie = MovieScene::capture(
        &state,
        &active_camera,
        &json!({"sceneId":scene.get(), "viewId":view.get()}),
    )
    .unwrap();
    movie.seek(0.).unwrap();
    assert_eq!(movie.camera().zoom(), 73.);
    assert_eq!(movie.state().active_scene(), Some(scene));
    assert!(state.same_undo_state(&before));
}

#[test]
fn missing_start_camera_uses_captured_view_without_changing_saved_cameras() {
    for name in ["", "deleted camera"] {
        let (mut state, mut camera) = animated_scene();
        let scene = state.active_scene().unwrap();
        let mut animation = state.scene_animation(scene).unwrap().unwrap();
        animation.set_startcam(name);
        state.set_scene_animation(scene, Some(&animation)).unwrap();
        camera.set_zoom(21.);
        let before = state.clone();
        let mut movie = MovieScene::capture(&state, &camera, &json!({})).unwrap();
        movie.seek(movie.duration() / 4.).unwrap();
        assert_eq!(movie.camera().zoom(), 21.);
        assert!(state.same_undo_state(&before));
        assert_eq!(
            state
                .scene_manager()
                .scene(scene)
                .unwrap()
                .camera_by_name("__current")
                .unwrap()
                .zoom(),
            73.
        );
    }
}

#[test]
fn movie_capture_does_not_stop_or_seek_live_playback() {
    let (mut state, mut camera) = animated_scene();
    call_tool(
        &mut state,
        &mut camera,
        "animate",
        &json!({"action":"seek","timeMs":200}),
    )
    .unwrap();
    call_tool(
        &mut state,
        &mut camera,
        "animate",
        &json!({"action":"play"}),
    )
    .unwrap();
    let scene = state.active_scene().unwrap();
    let before = state.clone();
    let mut movie = MovieScene::capture(&state, &camera, &json!({})).unwrap();
    movie.seek(0.).unwrap();
    assert!(state.is_scene_animation_playing(scene));
    assert!(state.same_undo_state(&before));
    assert!(!movie.state().is_scene_animation_playing(scene));
    assert!(movie.seek(f64::NAN).is_err());
    assert!(movie.seek(-1.).is_err());
}

#[test]
fn animated_renderer_properties_and_visibility_do_not_leak_to_live_objects() {
    use cuemol_anim::{AnimMgr, AnimMotion, RealPropAnim, ShowHideAnim};
    use cuemol_core::time_from_seconds;
    let (mut state, camera) = animated_scene();
    let scene = state.active_scene().unwrap();
    let object = state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("mol", cuemol_molstr::MolCoord::new());
    let renderer = state
        .new_renderer(scene, object, "simple", "animated")
        .unwrap()
        .result_renderer;
    let mut property = RealPropAnim::new();
    property.set_rend("mol/animated");
    property.set_prop("width");
    property.set_start_value(1.);
    property.set_end_value(3.);
    property.base_mut().set_rel_end(time_from_seconds(1.));
    let mut hide = ShowHideAnim::new();
    hide.set_rend("mol/animated");
    hide.set_hide(true);
    hide.base_mut().set_rel_end(time_from_seconds(1.));
    let mut animation = AnimMgr::new();
    animation.append_motion(AnimMotion::RealProperty(property));
    animation.append_motion(AnimMotion::ShowHide(hide));
    animation.set_length(time_from_seconds(2.));
    state.set_scene_animation(scene, Some(&animation)).unwrap();
    let before = state.clone();
    let mut movie = MovieScene::capture(&state, &camera, &json!({})).unwrap();
    movie.seek(0.5).unwrap();
    assert_eq!(
        movie
            .state()
            .scene_manager()
            .object(object)
            .unwrap()
            .renderer(renderer)
            .unwrap()
            .qsc_attr("width"),
        Some("2")
    );
    movie.seek(1.1).unwrap();
    assert!(!movie
        .state()
        .scene_manager()
        .object(object)
        .unwrap()
        .renderer(renderer)
        .unwrap()
        .is_visible());
    assert!(state
        .scene_manager()
        .object(object)
        .unwrap()
        .renderer(renderer)
        .unwrap()
        .is_visible());
    assert!(state.same_undo_state(&before));
}
