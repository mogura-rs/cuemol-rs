use cuemol_command::prepare_current_view;
use cuemol_core::{Quat, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::{Camera, StereoMode};

#[test]
fn current_view_is_a_portable_camera_without_file_links_or_visibility() {
    let mut state = DesktopAppState::new();
    state.new_scene("View source", true);
    let mut camera = Camera::new();
    camera.set_name("named camera");
    camera.set_source("source.cam");
    camera.set_alt_source("alternate.cam");
    camera.set_center(Vector4::new3(1.123456789, -3.987654321, 7.));
    camera.set_zoom(31.125);
    camera.set_cam_dist(83.25);
    camera.set_slab_depth(16.375);
    camera.set_rotation(Quat::new(0., 0., 0.6, 0.8));
    camera.set_perspective(true);
    camera.set_stereo_mode(StereoMode::Cross);
    camera.set_stereo_distance(2.75);
    camera.vis_append(99, false, true);
    let before = state.clone();
    let prepared = prepare_current_view(&state, &camera).unwrap();
    assert!(state.same_undo_state(&before));
    assert!(!prepared.xml.contains("visibilities"));
    assert!(!prepared.xml.contains("src="));
    let imported = Camera::from_xml_string(&prepared.xml).unwrap();
    assert_eq!(imported.name(), "__current");
    assert_eq!(imported.center(), camera.center());
    assert_eq!(imported.zoom(), camera.zoom());
    assert_eq!(imported.cam_dist(), camera.cam_dist());
    assert_eq!(imported.slab_depth(), camera.slab_depth());
    assert_eq!(imported.rotation(), camera.rotation());
    assert_eq!(imported.stereo_mode(), camera.stereo_mode());
    assert_eq!(imported.stereo_distance(), camera.stereo_distance());
    assert!(imported.is_perspective());
    prepared.finish(&mut state).unwrap();
    let stored = state
        .scene_manager()
        .scene(state.active_scene().unwrap())
        .unwrap()
        .camera_by_name("__current")
        .unwrap();
    assert_eq!(stored.to_xml_string(), imported.to_xml_string());
    assert_eq!(camera.source(), "source.cam");
    assert_eq!(camera.visibility_settings().count(), 1);
}

#[test]
fn current_view_completion_keeps_the_captured_scene_and_rejects_a_deleted_target() {
    let mut state = DesktopAppState::new();
    state.new_scene("Source", true);
    let source = state.active_scene().unwrap();
    let mut camera = Camera::new();
    camera.set_zoom(29.);
    let prepared = prepare_current_view(&state, &camera).unwrap();
    state.new_scene("Other", true);
    let active = state.active_scene();
    let view = state.active_view_camera();
    prepared.finish(&mut state).unwrap();
    assert_eq!(state.active_scene(), active);
    assert_eq!(state.active_view_camera(), view);
    assert_eq!(
        state
            .scene_manager()
            .scene(source)
            .unwrap()
            .camera_by_name("__current")
            .unwrap()
            .zoom(),
        29.
    );
    let prepared = prepare_current_view(&state, &camera).unwrap();
    state.destroy_scene(active.unwrap());
    let before = state.clone();
    assert!(prepared.finish(&mut state).is_err());
    assert!(state.same_undo_state(&before));
}
