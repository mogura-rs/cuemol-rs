use cuemol_core::Vector4;
use cuemol_qsys::{Camera, StereoMode};
use cuemol_render::{
    render_umbreon_npr_snapshot, render_umbreon_snapshot, render_umbreon_snapshot_with_cancel,
    HatchRenderParams, RenderAtom, RenderSceneSnapshot, UmbreonRenderParams,
};
use std::sync::atomic::AtomicBool;

#[test]
fn stereo_cpu_images_match_independent_eyes_after_filtering_and_npr() {
    let mut scene = RenderSceneSnapshot::empty();
    scene.atoms.push(RenderAtom::new(
        Vector4::new3(0., 0., 2.),
        1.,
        Some(cuemol_gfx::parse_color("#ff00ff").unwrap()),
    ));
    let mut camera = Camera::new();
    camera.set_zoom(8.);
    camera.set_cam_dist(20.);
    camera.set_slab_depth(30.);
    camera.set_stereo_mode(StereoMode::Cross);
    camera.set_stereo_distance(18.);
    for (swap, denoiser, npr) in [
        (false, 0, false),
        (true, 1, false),
        (false, 2, false),
        (true, 0, true),
    ] {
        let settings = UmbreonRenderParams {
            width: 129,
            height: 64,
            supersample: 1,
            perspective: false,
            swap_stereo_eyes: swap,
            denoiser,
            gi_denoise: false,
            transparent_background: true,
            ..Default::default()
        };
        let hatch = HatchRenderParams::default();
        let render = |camera: &Camera, settings: &UmbreonRenderParams| {
            if npr {
                render_umbreon_npr_snapshot(&scene, camera, [0., 0., 0., 0.], settings, &hatch)
                    .unwrap()
            } else {
                render_umbreon_snapshot(&scene, camera, [0., 0., 0., 0.], settings).unwrap()
            }
        };
        let pair = render(&camera, &settings);
        assert_eq!((pair.width, pair.height, pair.components), (129, 64, 4));
        if denoiser == 2 {
            let prepared =
                cuemol_render::prepare_oidn_render(&scene, &camera, [0.; 4], &settings, None)
                    .unwrap()
                    .run()
                    .unwrap();
            assert_eq!(prepared.pixels, pair.pixels);
        }
        for eye in cuemol_qsys::stereo::viewports(&camera, [129., 64.], swap) {
            let mut mono_settings = settings.clone();
            mono_settings.width = eye.rect[2] as i32;
            let mono = render(&eye.camera, &mono_settings);
            for y in 0..64usize {
                let offset = (y * 129 + eye.rect[0] as usize) * 4;
                assert_eq!(
                    &pair.pixels[offset..][..mono.width as usize * 4],
                    &mono.pixels[y * mono.width as usize * 4..(y + 1) * mono.width as usize * 4]
                );
            }
        }
        let cancelled = AtomicBool::new(true);
        assert!(render_umbreon_snapshot_with_cancel(
            &scene, &camera, [0.; 4], &settings, None, &cancelled
        )
        .is_err());
    }
}
