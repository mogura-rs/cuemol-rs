#![cfg(feature = "offscreen")]

use cuemol_core::{Quat, Vector4};
use cuemol_desktop_core::{
    effects::{Antialiasing, ViewportOptions},
    offscreen::SceneRenderer,
};
use cuemol_qsys::{Camera, CenterMark, StereoMode};
use cuemol_render::{RenderAtom, RenderSceneSnapshot};

#[test]
fn center_mark_inverts_presented_rgb_and_preserves_alpha_in_each_eye() {
    let Some(renderer) = SceneRenderer::new() else {
        eprintln!("skipping center-mark pixels: no GPU adapter");
        return;
    };
    let mut camera = Camera::new();
    camera.set_zoom(8.);
    camera.set_cam_dist(20.);
    camera.set_slab_depth(30.);
    let options = ViewportOptions {
        antialiasing: Antialiasing::None,
        ..Default::default()
    };
    let mut scene = RenderSceneSnapshot::empty();
    for with_atom in [false, true] {
        if with_atom {
            scene.atoms.push(RenderAtom::new(
                Vector4::new3(0., 0., 0.),
                0.35,
                Some(cuemol_gfx::parse_color("#cc1940").unwrap()),
            ));
        }
        for stereo in [StereoMode::None, StereoMode::Parallel] {
            camera.set_stereo_mode(stereo);
            for background in [[0.2, 0.4, 0.7, 1.], [0.9, 0.1, 0.3, 0.6]] {
                let render = |camera: &Camera| {
                    renderer
                        .render_viewport_with_labels(
                            &scene,
                            camera,
                            128,
                            96,
                            background,
                            options,
                            &[],
                        )
                        .unwrap()
                };
                camera.set_center_mark(CenterMark::None);
                let plain = render(&camera);
                camera.set_center_mark(CenterMark::Cross);
                let marked = render(&camera);
                let mut changed = [0; 2];
                for (index, (before, after)) in plain
                    .chunks_exact(4)
                    .zip(marked.chunks_exact(4))
                    .enumerate()
                {
                    assert_eq!(before[3], after[3], "alpha at pixel {index}");
                    if before == after {
                        continue;
                    }
                    for channel in 0..3 {
                        assert!(
                            (i16::from(before[channel]) + i16::from(after[channel]) - 255).abs()
                                <= 1,
                            "RGB at pixel {index}: {before:?} -> {after:?}"
                        );
                    }
                    let x = index % 128;
                    let y = index / 128;
                    let center_x = if stereo == StereoMode::None {
                        64
                    } else if x < 64 {
                        32
                    } else {
                        96
                    };
                    assert!(x.abs_diff(center_x) <= 11 && y.abs_diff(48) <= 11);
                    changed[usize::from(x >= 64)] += 1;
                }
                assert!(changed.iter().sum::<usize>() >= 30, "{changed:?}");
                if stereo != StereoMode::None {
                    assert!(changed.into_iter().all(|count| count >= 30));
                }
            }
        }
    }
    camera.set_stereo_mode(StereoMode::None);
    camera.set_center_mark(CenterMark::Axis);
    let render_axis = |camera: &Camera| {
        renderer
            .render_viewport_with_labels(&scene, camera, 128, 96, [0.2, 0.4, 0.7, 1.], options, &[])
            .unwrap()
    };
    let initial = render_axis(&camera);
    camera.set_rotation(Quat::from_axis_angle(Vector4::new3(0., 0., 1.), 0.7).unwrap());
    let rotated = render_axis(&camera);
    assert!(
        initial
            .iter()
            .zip(rotated)
            .filter(|(a, b)| **a != *b)
            .count()
            > 30
    );
}
