use cuemol_anim::{AnimMgr, AnimMotion, CamMotion, RendXformAnim, SimpleSpin};
use cuemol_core::{Uid, Vector4};
use cuemol_qsys::{Camera, Scene};

#[test]
fn overlapping_camera_motions_start_from_the_camera_at_their_start_time() {
    let mut scene = Scene::<()>::new(Uid::new(1), "scene");
    let view = scene.create_view();
    scene.set_camera("start", Camera::new());
    for (name, x) in [("first", 10.0), ("second", 30.0)] {
        let mut camera = Camera::new();
        camera.set_center(Vector4::new3(x, 0.0, 0.0));
        scene.set_camera(name, camera);
    }
    let mut manager = AnimMgr::new();
    manager.set_startcam("start");
    for (name, start, end) in [("first", 0, 100), ("second", 50, 150)] {
        let mut motion = CamMotion::new();
        motion.set_endcam(name);
        motion.base_mut().set_rel_start(start);
        motion.base_mut().set_rel_end(end);
        manager.append_motion(AnimMotion::Camera(motion));
    }
    for (time, x) in [(100, 17.5), (75, 11.25), (150, 30.0), (25, 2.5)] {
        manager.go_time_in_scene(time, &mut scene, view).unwrap();
        assert!((scene.view(view).unwrap().center().x - x).abs() < 1e-12);
    }
}

#[test]
fn renderer_translation_keeps_the_camera_orientation_at_its_start() {
    let mut scene = Scene::new(Uid::new(1), "scene");
    let view = scene.create_view();
    scene.set_camera("start", Camera::new());
    let object = scene.add_object("molecule", ());
    let renderer = scene.object_mut(object).unwrap().create_renderer("simple");
    scene
        .object_mut(object)
        .unwrap()
        .renderer_mut(renderer)
        .unwrap()
        .set_name("main");
    let mut spin = SimpleSpin::new();
    spin.set_angle(90.0);
    spin.base_mut().set_rel_end(100);
    let mut translation = RendXformAnim::new();
    translation.set_rend("main");
    translation.set_end_tran(Vector4::new3(10.0, 0.0, 0.0));
    translation.base_mut().set_rel_start(50);
    translation.base_mut().set_rel_end(100);
    let mut manager = AnimMgr::new();
    manager.set_startcam("start");
    manager.append_motion(AnimMotion::Spin(spin));
    manager.append_motion(AnimMotion::RendererTransform(translation));
    manager.go_time_in_scene(100, &mut scene, view).unwrap();
    let offset = scene
        .object(object)
        .unwrap()
        .renderer(renderer)
        .unwrap()
        .xform_matrix()
        .get_trans_part();
    let diagonal = 10.0 / 2.0_f64.sqrt();
    assert!((offset.x - diagonal).abs() < 1e-10, "{offset:?}");
    assert!(offset.y.abs() < 1e-10);
    assert!((offset.z + diagonal).abs() < 1e-10, "{offset:?}");
}

#[test]
fn rendered_frames_reuse_the_initial_view_without_a_named_camera() {
    let mut scene = Scene::<()>::new(Uid::new(1), "scene");
    let view = scene.create_view();
    let mut destination = Camera::new();
    destination.set_center(Vector4::new3(12.0, 0.0, 0.0));
    scene.set_camera("end", destination);
    let mut motion = CamMotion::new();
    motion.set_endcam("end");
    motion.base_mut().set_rel_end(cuemol_core::NANOS_PER_SECOND);
    let mut manager = AnimMgr::new();
    manager.append_motion(AnimMotion::Camera(motion));
    assert_eq!(
        manager.setup_render(0, cuemol_core::NANOS_PER_SECOND, 4.0),
        5
    );
    let mut writer = cuemol_qsys::SceneExporter::default();
    for x in [0.0, 3.0, 6.0, 9.0, 12.0] {
        assert!(manager
            .begin_frame_in_scene(&mut writer, &mut scene, view)
            .unwrap());
        assert_eq!(writer.camera_object().unwrap().center().x, x);
        assert_eq!(writer.client_scene(), Some(scene.uid()));
        manager.end_frame(&mut writer);
        assert_eq!(writer.client_scene(), None);
    }
    assert!(!manager
        .begin_frame_in_scene(&mut writer, &mut scene, view)
        .unwrap());
    for rate in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e30] {
        assert_eq!(
            manager.setup_render(0, cuemol_core::NANOS_PER_SECOND, rate),
            0
        );
        assert!(!manager.begin_frame(&mut writer));
    }
}
