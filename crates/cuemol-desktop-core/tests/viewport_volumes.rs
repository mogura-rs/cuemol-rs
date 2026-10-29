#![cfg(feature = "offscreen")]

use cuemol_core::{Matrix4, Quat, Vector4};
use cuemol_desktop_core::{
    effects::{Antialiasing, ViewportOptions},
    offscreen::SceneRenderer,
};
use cuemol_qsys::Camera;
use cuemol_render::{
    render_umbreon_snapshot, MeshVertex, RenderMesh, RenderSceneSnapshot, RenderVolume,
    UmbreonRenderParams,
};
use std::sync::Arc;

fn volume(color: [u8; 4]) -> RenderVolume {
    RenderVolume::new(
        [4; 3],
        Arc::new(vec![128; 64]),
        Matrix4 {
            values: [
                [4., 0., 0., -2.],
                [0., 4., 0., -2.],
                [0., 0., 4., -2.],
                [0., 0., 0., 1.],
            ],
        },
        [color; 256],
    )
    .unwrap()
}

fn camera() -> Camera {
    let mut camera = Camera::new();
    camera.set_center_mark(cuemol_qsys::CenterMark::None);
    camera.set_zoom(8.);
    camera.set_cam_dist(20.);
    camera.set_slab_depth(30.);
    camera
}

fn cpu(scene: &RenderSceneSnapshot, camera: &Camera) -> Vec<u8> {
    render_umbreon_snapshot(
        scene,
        camera,
        [0., 0., 0., 1.],
        &UmbreonRenderParams {
            width: 64,
            height: 64,
            supersample: 1,
            perspective: camera.is_perspective(),
            use_clip_z: true,
            ao_samples: 0,
            shadows: false,
            use_gi: false,
            denoiser: 0,
            show_edge_lines: false,
            ..Default::default()
        },
    )
    .unwrap()
    .pixels
}

fn compare_centers(gpu: &[u8], cpu: &[u8]) {
    for (x, y) in [(26, 27), (31, 32), (37, 35)] {
        let a = &gpu[(y * 64 + x) * 4..][..3];
        let b = &cpu[(y * 64 + x) * 3..][..3];
        assert!(
            a.iter().zip(b).all(|(a, b)| a.abs_diff(*b) <= 5),
            "GPU {a:?}, CPU {b:?} at {x},{y}"
        );
    }
}

#[test]
fn volume_opacity_rotation_and_overlapping_grids_match_cpu_compositing() {
    let Some(renderer) = SceneRenderer::new() else {
        eprintln!("Skipping volume rendering: no GPU adapter");
        return;
    };
    let mut scene = RenderSceneSnapshot::empty();
    scene.volumes.push(volume([255, 0, 0, 128]));
    scene.recompute_bounds();
    let mut camera = camera();
    let render = |scene: &RenderSceneSnapshot, camera: &Camera| {
        renderer
            .render_camera(scene, camera, 64, 64, [0., 0., 0., 1.], 0)
            .unwrap()
    };
    let rgba = render(&scene, &camera);
    let expected = ((1. - (1. - 128. / 255. * 0.5_f64).powi(8)) * 255.).round() as u8;
    assert!(rgba[(32 * 64 + 32) * 4].abs_diff(expected) <= 3);
    compare_centers(&rgba, &cpu(&scene, &camera));
    let matrix = cuemol_desktop_core::offscreen::camera_mvp(&camera, 1.);
    let matrix_image = renderer
        .render_snapshot(&scene, matrix, 64, 64, [0., 0., 0., 1.])
        .unwrap();
    compare_centers(&matrix_image, &cpu(&scene, &camera));
    scene.volumes[0].alpha = 0.5;
    let translucent = render(&scene, &camera);
    assert!(translucent[(32 * 64 + 32) * 4] < rgba[(32 * 64 + 32) * 4] - 35);
    compare_centers(&translucent, &cpu(&scene, &camera));
    scene.volumes.push(volume([0, 255, 0, 64]));
    compare_centers(&render(&scene, &camera), &cpu(&scene, &camera));
    camera.set_rotation(Quat::new(0., 0.22_f64.sin(), 0., 0.22_f64.cos()));
    camera.set_perspective(true);
    compare_centers(&render(&scene, &camera), &cpu(&scene, &camera));
    let matrix = cuemol_desktop_core::offscreen::camera_mvp(&camera, 1.);
    compare_centers(
        &renderer
            .render_snapshot(&scene, matrix, 64, 64, [0., 0., 0., 1.])
            .unwrap(),
        &cpu(&scene, &camera),
    );

    let options = ViewportOptions {
        ao_enabled: true,
        antialiasing: Antialiasing::None,
        jitter_level: 0,
        ..Default::default()
    };
    let postprocessed = renderer
        .render_viewport(&scene, &camera, 64, 64, [0., 0., 0., 1.], options)
        .unwrap();
    compare_centers(&postprocessed, &cpu(&scene, &camera));
}

#[test]
fn volume_texture_interpolation_respects_opaque_geometry_and_slab() {
    let Some(renderer) = SceneRenderer::new() else {
        eprintln!("Skipping volume rendering: no GPU adapter");
        return;
    };
    let mut scene = RenderSceneSnapshot::empty();
    let mut field = volume([255, 0, 0, 64]);
    field.values = Arc::new((0..64).map(|i| ((i % 4) * 85) as u8).collect());
    field.transfer = std::array::from_fn(|i| [i as f32 / 255., 0., 0., 64. / 255.]);
    scene.volumes.push(field);
    let mut camera = camera();
    let render = |scene: &RenderSceneSnapshot, camera: &Camera| {
        renderer
            .render_camera(scene, camera, 64, 64, [0., 0., 0., 1.], 0)
            .unwrap()
    };
    compare_centers(&render(&scene, &camera), &cpu(&scene, &camera));
    scene.meshes.push(RenderMesh {
        vertices: [(-4., -4.), (0., -4.), (0., 4.), (-4., 4.)]
            .into_iter()
            .map(|(x, y)| MeshVertex {
                pick_atom_index: None,
                position: Vector4::new3(x, y, 3.),
                normal: Vector4::new3(0., 0., 1.),
                color: [0., 0., 1., 1.],
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
    });
    let visible = render(&scene, &camera);
    assert_eq!(visible[(32 * 64 + 26) * 4], 0);
    assert!(visible[(32 * 64 + 37) * 4] > 80);
    assert_eq!(cpu(&scene, &camera)[(32 * 64 + 26) * 3], 0);
    scene.meshes.clear();
    camera.set_slab_depth(2.);
    let clipped = render(&scene, &camera);
    assert!(clipped[(32 * 64 + 37) * 4] < visible[(32 * 64 + 37) * 4] - 10);
    compare_centers(&clipped, &cpu(&scene, &camera));
}
