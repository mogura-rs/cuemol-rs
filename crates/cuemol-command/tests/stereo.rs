use cuemol_command::{call_tool, navigation::polygon_hits_camera};
use cuemol_desktop_core::{stereo, DesktopAppState, GuiViewOptions};
use cuemol_qsys::{Camera, StereoMode};
use serde_json::json;

fn fixture() -> (DesktopAppState, Camera) {
    let mut state = DesktopAppState::new();
    state.open_bytes("stereo.pdb", b"HETATM    1  C1  LIG A   1      -1.000   0.500   2.000  1.00 10.00           C\nHETATM    2  C2  LIG A   2       1.000  -0.500  -2.000  1.00 10.00           C\nEND\n").unwrap();
    let mut camera = Camera::new();
    camera.set_zoom(10.);
    camera.set_cam_dist(30.);
    camera.set_slab_depth(50.);
    camera.set_stereo_distance(12.);
    (state, camera)
}

#[test]
fn both_eyes_pick_the_displayed_atom_and_region_selection_deduplicates() {
    let (mut state, mut camera) = fixture();
    let object = state.active_object_summaries()[0].uid;
    call_tool(
        &mut state,
        &mut camera,
        "set_atom_labels",
        &json!({"objId":object.get(),"selection":"aid 1","format":"{name}"}),
    )
    .unwrap();
    let snapshot = state.active_render_snapshot("").unwrap();
    let position = state.scene_manager().object(object).unwrap().mol().atoms()[0].position;
    for mode in [StereoMode::Parallel, StereoMode::Cross] {
        camera.set_stereo_mode(mode);
        for swap in [false, true] {
            let points = stereo::project(&camera, [601., 400.], swap, position);
            assert_eq!(points.len(), 2);
            let labels = stereo::labels(&state, &camera, [601., 400.], swap, 100);
            assert_eq!(labels.len(), 2);
            for (index, point) in points.iter().enumerate() {
                let hit = stereo::cpu_hit(
                    &snapshot,
                    &camera,
                    swap,
                    [point[0], point[1]],
                    [601., 400.],
                    5.,
                    false,
                )
                .unwrap();
                assert_eq!((hit.object_id, hit.atom_index), (object.get(), 0));
                let region = [
                    [point[0] - 2., point[1] - 2.],
                    [point[0] + 2., point[1] - 2.],
                    [point[0] + 2., point[1] + 2.],
                    [point[0] - 2., point[1] + 2.],
                ];
                let hits = polygon_hits_camera(&state, &camera, swap, [601., 400.], &region);
                assert_eq!(hits.len(), 1);
                assert_eq!(hits[0].index, 0);
                assert!((labels[index].x as f64 - point[0]).abs() < 1e-4);
                assert!((labels[index].y as f64 - point[1]).abs() < 1e-4);
                assert_eq!(
                    labels[index].clip.unwrap()[0],
                    if index == 0 { 0. } else { 300. }
                );
            }
            let all = polygon_hits_camera(
                &state,
                &camera,
                swap,
                [601., 400.],
                &[[0., 0.], [601., 0.], [601., 400.], [0., 400.]],
            );
            assert_eq!(all.len(), 2);
        }
    }
}

#[test]
fn view_options_follow_inheritance_and_activation_without_changing_cameras() {
    let (mut state, camera) = fixture();
    let first = state.active_view_summary().unwrap().uid;
    let scene = state.active_scene().unwrap();
    let options = GuiViewOptions {
        trans_mms: true,
        rot_mms: true,
        swap_stereo_eyes: true,
    };
    state.set_active_view_options(options);
    let inherited = state.new_view(scene, "inherited", Some(&camera)).unwrap();
    assert_eq!(state.active_view_options(), options);
    let fresh = state.new_view(scene, "fresh", None).unwrap();
    assert_eq!(state.active_view_options(), GuiViewOptions::default());
    for id in [first, inherited] {
        state.activate_view(id).unwrap();
        assert_eq!(state.active_view_options(), options);
    }
    state.activate_view(fresh).unwrap();
    assert_eq!(state.active_view_options(), GuiViewOptions::default());
}

#[test]
fn configured_umbreon_export_uses_scene_background_and_display_eye_order() {
    let (mut state, mut camera) = fixture();
    let scene = state.active_scene().unwrap();
    state
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .set_scene_attribute("bgcolor", "#104060");
    camera.set_name("__current");
    camera.set_stereo_mode(StereoMode::Parallel);
    state.set_active_scene_camera(camera.clone()).unwrap();
    state.set_active_view_options(GuiViewOptions {
        swap_stereo_eyes: true,
        ..Default::default()
    });
    let target = tempfile::NamedTempFile::new().unwrap();
    let mut exporter = cuemol_render::UmbreonSceneExporter::new();
    exporter.scene_mut().attach(scene);
    exporter
        .handler_mut()
        .set_path(target.path().to_string_lossy());
    exporter.set_width(129);
    exporter.set_height(64);
    exporter.set_use_gi(false);
    exporter.set_ao_samples(0);
    exporter.set_denoise_mode(cuemol_render::UmbreonDenoiseMode::None);
    exporter.set_supersample(1);
    let frame = state.write_umbreon_exporter(&mut exporter, "").unwrap();
    assert_eq!(&frame.pixels[..3], &[16, 64, 96]);
    let mut params = exporter.params().clone();
    params.width = 129;
    params.height = 64;
    params.swap_stereo_eyes = true;
    let expected = cuemol_render::render_umbreon_snapshot(
        &state.active_render_snapshot("").unwrap(),
        &camera,
        state.active_background_color().unwrap(),
        &params,
    )
    .unwrap();
    assert_eq!(frame.pixels, expected.pixels);
}

#[cfg(feature = "scripting")]
#[test]
fn script_hit_testing_uses_each_stereo_eye_and_eye_order() {
    let (mut state, mut camera) = fixture();
    let object = state.active_object_summaries()[0].uid;
    let position = state.scene_manager().object(object).unwrap().mol().atoms()[0].position;
    let view = state.active_view_summary().unwrap().uid;
    camera.set_stereo_mode(StereoMode::Cross);
    state.set_active_view_camera(&camera).unwrap();
    state
        .command_manager_mut()
        .view_mut(view)
        .unwrap()
        .set_view_size(600, 400);
    for swap in [false, true] {
        state.set_active_view_options(GuiViewOptions {
            swap_stereo_eyes: swap,
            ..Default::default()
        });
        for point in stereo::project(&camera, [600., 400.], swap, position) {
            let source = format!("var v=cuemol.getView({});var h=JSON.parse(v.hitTest({},{}));print(h.obj_id+':'+h.data[0]);",view.get(),point[0].round(),point[1].round());
            let (log, _) = state.run_script_js(&source).unwrap();
            assert_eq!(log, format!("{}:1\n", object.get()));
        }
    }
}
