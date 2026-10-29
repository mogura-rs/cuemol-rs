#![cfg(feature = "offscreen")]
use cuemol_core::Vector4;
use cuemol_desktop_core::{
    effects::{Antialiasing, ViewportOptions},
    offscreen::SceneRenderer,
    GuiAtomLabel,
};
use cuemol_qsys::Camera;
use cuemol_render::{MeshVertex, RenderMesh, RenderSceneSnapshot};

#[test]
fn labels_depth_test_each_pixel_and_keep_explicit_overlay_order() {
    let Some(renderer) = SceneRenderer::new() else {
        eprintln!("skipping label rendering: no GPU adapter");
        return;
    };
    let mut scene = RenderSceneSnapshot::empty();
    scene.meshes.push(RenderMesh {
        vertices: [(-4., -4.), (0., -4.), (0., 4.), (-4., 4.)]
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
    let mvp = cuemol_desktop_core::offscreen::camera_mvp(&camera, 128. / 96.);
    let depth = (-mvp[2][2] + mvp[3][2]) / (-mvp[2][3] + mvp[3][3]);
    let mut label = GuiAtomLabel {
        x: 64.,
        y: 48.,
        text: "MMMM".into(),
        color: "#ff0000".into(),
        font_size: 24.,
        font_name: "Hack".into(),
        font_style: "normal".into(),
        font_weight: "normal".into(),
        anchor: [0.5, 0.5],
        depth,
        clip: None,
        on_top: false,
    };
    let options = ViewportOptions {
        antialiasing: Antialiasing::None,
        jitter_level: 0,
        ao_enabled: false,
        ..Default::default()
    };
    let render = |label: &GuiAtomLabel| {
        renderer
            .render_viewport_with_labels(
                &scene,
                &camera,
                128,
                96,
                [0., 0., 0., 1.],
                options,
                std::slice::from_ref(label),
            )
            .unwrap()
    };
    let count = |pixels: &[u8], left: bool| {
        pixels
            .chunks_exact(4)
            .enumerate()
            .filter(|(i, p)| (i % 128 < 64) == left && p[0] > 220 && p[1] < 25 && p[2] < 25)
            .count()
    };
    let hidden = render(&label);
    assert_eq!(count(&hidden, true), 0);
    assert!(count(&hidden, false) > 40);
    label.on_top = true;
    let overlaid = render(&label);
    assert!(count(&overlaid, true) > 40);
    assert_eq!(count(&hidden, false), count(&overlaid, false));
    label.anchor = [1., 1.];
    let aligned = render(&label);
    assert!(count(&aligned, true) > 40);
    assert_eq!(count(&aligned, false), 0);
}

#[test]
fn live_label_dimensions_follow_logical_pixels_at_hidpi() {
    let renderer = SceneRenderer::new().expect("a GPU adapter is required");
    let mut camera = Camera::new();
    camera.set_center_mark(cuemol_qsys::CenterMark::None);
    let label = GuiAtomLabel {
        x: 64.,
        y: 48.,
        text: "MW".into(),
        color: "#ff0000".into(),
        font_size: 24.,
        font_name: "Hack".into(),
        font_style: "normal".into(),
        font_weight: "normal".into(),
        anchor: [0.5, 0.5],
        depth: 0.,
        clip: None,
        on_top: true,
    };
    let mut dimensions = Vec::new();
    for ratio in [1_u32, 2] {
        let width = 128 * ratio;
        let pixels = renderer
            .render_viewport_with_labels(
                &RenderSceneSnapshot::empty(),
                &camera,
                width,
                96 * ratio,
                [0., 0., 0., 1.],
                ViewportOptions {
                    antialiasing: Antialiasing::None,
                    pixels_per_point: ratio as f32,
                    ..Default::default()
                },
                std::slice::from_ref(&label),
            )
            .unwrap();
        let mut bounds = [u32::MAX, u32::MAX, 0, 0];
        for (index, pixel) in pixels.chunks_exact(4).enumerate() {
            if pixel[0] > 128 && pixel[1] < 10 && pixel[2] < 10 {
                let x = index as u32 % width;
                let y = index as u32 / width;
                bounds[0] = bounds[0].min(x);
                bounds[1] = bounds[1].min(y);
                bounds[2] = bounds[2].max(x);
                bounds[3] = bounds[3].max(y);
            }
        }
        assert!(bounds[0] < bounds[2] && bounds[1] < bounds[3]);
        dimensions.push([bounds[2] - bounds[0] + 1, bounds[3] - bounds[1] + 1]);
    }
    for axis in 0..2 {
        assert!(
            dimensions[1][axis].abs_diff(dimensions[0][axis] * 2) <= 2,
            "{dimensions:?}"
        );
    }
}
