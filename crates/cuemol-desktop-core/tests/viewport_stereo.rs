#![cfg(feature = "offscreen")]

use cuemol_core::{Matrix4, Quat, Vector4};
use cuemol_desktop_core::{
    effects::{Antialiasing, ViewportOptions},
    offscreen::SceneRenderer,
};
use cuemol_qsys::{Camera, StereoMode};
use cuemol_render::{RenderAtom, RenderSceneSnapshot, RenderVolume};
use std::sync::Arc;

#[test]
fn stereo_eyes_match_independent_camera_renders_with_shared_geometry_and_volumes() {
    let renderer = SceneRenderer::new().expect("a GPU adapter is required");
    let mut scene = RenderSceneSnapshot::empty();
    scene.atoms.push(RenderAtom::new(
        Vector4::new3(-1.1, 0.5, 2.3),
        0.8,
        Some(cuemol_gfx::parse_color("#ff00ff").unwrap()),
    ));
    scene.atoms.push(RenderAtom::new(
        Vector4::new3(1.4, -0.5, -1.6),
        1.1,
        Some(cuemol_gfx::parse_color("#00ffff").unwrap()),
    ));
    scene.volumes.push(
        RenderVolume::new(
            [3; 3],
            Arc::new(vec![180; 27]),
            Matrix4 {
                values: [
                    [2., 0., 0., -1.],
                    [0., 2., 0., -1.],
                    [0., 0., 2., -2.],
                    [0., 0., 0., 1.],
                ],
            },
            [[255, 180, 0, 90]; 256],
        )
        .unwrap(),
    );
    scene.recompute_bounds();
    let mut camera = Camera::new();
    camera.set_zoom(9.);
    camera.set_cam_dist(20.);
    camera.set_slab_depth(25.);
    camera.set_stereo_distance(16.);
    camera.set_rotation(Quat::from_axis_angle(Vector4::new3(1., 0., 0.), 0.2).unwrap());
    for perspective in [false, true] {
        camera.set_perspective(perspective);
        for (mode, swap, width) in [
            (StereoMode::Parallel, false, 320),
            (StereoMode::Cross, false, 320),
            (StereoMode::Parallel, true, 321),
        ] {
            camera.set_stereo_mode(mode);
            let options = ViewportOptions {
                antialiasing: Antialiasing::None,
                swap_stereo_eyes: swap,
                ..Default::default()
            };
            let image = renderer
                .render_viewport(&scene, &camera, width, 160, [0., 0., 0., 1.], options)
                .unwrap();
            let eyes = cuemol_qsys::stereo::viewports(&camera, [width as f64, 160.], swap);
            let split = width / 2;
            let mut colored = [0; 2];
            for (index, eye) in eyes.iter().enumerate() {
                let size = if index == 0 { split } else { width - split };
                let mono = renderer
                    .render_viewport(&scene, &eye.camera, size, 160, [0., 0., 0., 1.], options)
                    .unwrap();
                for y in 0..160usize {
                    let row = &image[(y * width as usize + index * split as usize) * 4..]
                        [..size as usize * 4];
                    assert_eq!(
                        row,
                        &mono[y * size as usize * 4..(y + 1) * size as usize * 4],
                        "{mode:?} swap={swap} perspective={perspective} eye={index} row={y}"
                    );
                    colored[index] += row
                        .chunks_exact(4)
                        .filter(|p| p[0].max(p[1]).max(p[2]) > 30)
                        .count();
                }
            }
            assert!(colored.iter().all(|count| *count > 500));
        }
    }
}

#[test]
fn stereo_labels_clip_at_the_eye_boundary_and_use_each_eyes_depth() {
    use cuemol_desktop_core::GuiAtomLabel;
    use cuemol_render::{MeshVertex, RenderMesh};
    let renderer = SceneRenderer::new().expect("a GPU adapter is required");
    let mut scene = RenderSceneSnapshot::empty();
    scene.meshes.push(RenderMesh {
        vertices: [(-3., -4.), (0., -4.), (0., 4.), (-3., 4.)]
            .into_iter()
            .map(|(x, y)| MeshVertex {
                pick_atom_index: None,
                position: Vector4::new3(x, y, 0.),
                normal: Vector4::new4(0., 0., 1., 2.),
                color: [1.; 4],
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
    });
    let mut camera = Camera::new();
    camera.set_zoom(8.);
    camera.set_cam_dist(20.);
    camera.set_slab_depth(30.);
    camera.set_stereo_mode(StereoMode::Parallel);
    camera.set_stereo_distance(12.);
    let options = ViewportOptions {
        antialiasing: Antialiasing::None,
        ..Default::default()
    };
    let eyes = cuemol_qsys::stereo::viewports(&camera, [256., 96.], false);
    let labels = eyes
        .iter()
        .enumerate()
        .map(|(i, eye)| {
            let mvp = cuemol_desktop_core::projection::camera_mvp(&eye.camera, 128. / 96.);
            let depth = (-mvp[2][2] + mvp[3][2]) / (-mvp[2][3] + mvp[3][3]);
            GuiAtomLabel {
                x: i as f32 * 128. + 64.,
                y: 48.,
                text: "MMMMMMMM".into(),
                color: "#ff0000".into(),
                font_size: 24.,
                font_name: "monospace".into(),
                font_style: "normal".into(),
                font_weight: "normal".into(),
                anchor: [0.5, 0.5],
                depth,
                on_top: false,
                clip: Some([i as f32 * 128., 0., 128., 96.]),
            }
        })
        .collect::<Vec<_>>();
    let pair = renderer
        .render_viewport_with_labels(&scene, &camera, 256, 96, [0., 0., 0., 1.], options, &labels)
        .unwrap();
    for (i, eye) in eyes.iter().enumerate() {
        let mut label = labels[i].clone();
        label.x -= i as f32 * 128.;
        label.clip = None;
        let mono = renderer
            .render_viewport_with_labels(
                &scene,
                &eye.camera,
                128,
                96,
                [0., 0., 0., 1.],
                options,
                &[label],
            )
            .unwrap();
        let red = mono
            .chunks_exact(4)
            .filter(|p| p[0] > 200 && p[1] < 20 && p[2] < 20)
            .count();
        assert!(red > 50);
        for y in 0..96usize {
            assert_eq!(
                &pair[(y * 256 + i * 128) * 4..][..128 * 4],
                &mono[y * 128 * 4..(y + 1) * 128 * 4]
            );
        }
    }
}

#[test]
fn stereo_export_keeps_eye_order_and_supersampling() {
    let renderer = SceneRenderer::new().expect("a GPU adapter is required");
    let mut scene = RenderSceneSnapshot::empty();
    scene.atoms.push(RenderAtom::new(
        Vector4::new3(0., 0., 2.),
        0.8,
        Some(cuemol_gfx::parse_color("#ff00ff").unwrap()),
    ));
    let mut camera = Camera::new();
    camera.set_zoom(8.);
    camera.set_cam_dist(20.);
    camera.set_slab_depth(30.);
    camera.set_stereo_mode(StereoMode::Cross);
    camera.set_stereo_distance(16.);
    let options = ViewportOptions {
        swap_stereo_eyes: true,
        ..Default::default()
    };
    let pair = renderer
        .render_camera_with_effects(&scene, &camera, 257, 96, [0., 0., 0., 1.], 1, options)
        .unwrap();
    for eye in cuemol_qsys::stereo::viewports(&camera, [257., 96.], true) {
        let w = eye.rect[2] as u32;
        let mono = renderer
            .render_camera_with_effects(&scene, &eye.camera, w, 96, [0., 0., 0., 1.], 1, options)
            .unwrap();
        for y in 0..96usize {
            assert_eq!(
                &pair[(y * 257 + eye.rect[0] as usize) * 4..][..w as usize * 4],
                &mono[y * w as usize * 4..(y + 1) * w as usize * 4]
            );
        }
    }
}

#[test]
fn stereo_depth_and_offscreen_view_preserve_eye_order() {
    use cuemol_desktop_core::offscreen::OffScreenView;
    let renderer = SceneRenderer::new().expect("a GPU adapter is required");
    let mut scene = RenderSceneSnapshot::empty();
    scene
        .atoms
        .push(RenderAtom::new(Vector4::new3(0., 0., 2.), 0.8, None));
    let mut camera = Camera::new();
    camera.set_zoom(8.);
    camera.set_cam_dist(20.);
    camera.set_slab_depth(30.);
    camera.set_stereo_mode(StereoMode::Parallel);
    camera.set_stereo_distance(18.);
    let swapped = cuemol_qsys::stereo::with_eye_order(&camera, true);
    let pair = renderer
        .render_camera_depth(&scene, &swapped, 257, 96)
        .unwrap();
    for eye in cuemol_qsys::stereo::viewports(&camera, [257., 96.], true) {
        let w = eye.rect[2] as u32;
        let mono = renderer
            .render_camera_depth(&scene, &eye.camera, w, 96)
            .unwrap();
        assert!(mono.chunks_exact(4).any(|p| p[0] > 0));
        for y in 0..96usize {
            assert_eq!(
                &pair[(y * 257 + eye.rect[0] as usize) * 4..][..w as usize * 4],
                &mono[y * w as usize * 4..(y + 1) * w as usize * 4]
            );
        }
    }
    let mut view = OffScreenView::new(cuemol_core::Uid::new(1), cuemol_core::Uid::new(2), 257, 96);
    view.set_camera(&camera);
    view.view_mut().set_swap_stereo_eyes(true);
    view.set_depth_mode(true);
    assert!(view.draw_scene(&scene, [0., 0., 0., 1.]));
    assert_eq!(view.pixels().unwrap(), pair);
    view.set_depth_mode(false);
    view.set_super_sample_level(0);
    assert!(view.draw_scene(&scene, [0., 0., 0., 1.]));
    assert_eq!(
        view.pixels().unwrap(),
        renderer
            .render_camera(&scene, &swapped, 257, 96, [0., 0., 0., 1.], 0)
            .unwrap()
    );
}
