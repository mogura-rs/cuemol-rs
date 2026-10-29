use crate::{picking, projection, DesktopAppState, GuiAtomLabel};
use cuemol_core::Vector4;
use cuemol_qsys::Camera;
use cuemol_render::RenderSceneSnapshot;

pub fn swap_eyes(state: &DesktopAppState) -> bool {
    state
        .active_view_summary()
        .and_then(|view| state.command_manager().view(view.uid))
        .is_some_and(|view| view.swap_stereo_eyes())
}

pub fn labels(
    state: &DesktopAppState,
    camera: &Camera,
    size: [f32; 2],
    swap: bool,
    max: usize,
) -> Vec<GuiAtomLabel> {
    let mut labels = Vec::new();
    for eye in cuemol_qsys::stereo::viewports(camera, size.map(f64::from), swap) {
        let mvp = projection::camera_mvp(&eye.camera, eye.aspect());
        let [x, y, width, height] = eye.rect.map(|v| v as f32);
        labels.extend(
            state
                .active_name_labels(mvp, width, height, max)
                .into_iter()
                .chain(state.active_atom_intr_labels(mvp, width, height, max))
                .map(|mut label| {
                    label.x += x;
                    label.y += y;
                    label.clip = Some([x, y, width, height]);
                    label
                }),
        );
    }
    labels
}

#[allow(clippy::too_many_arguments)]
pub fn cpu_hit(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    swap: bool,
    point: [f64; 2],
    size: [f64; 2],
    precision: f64,
    cpu_only: bool,
) -> Option<picking::PickRecord> {
    cuemol_qsys::stereo::viewports(camera, size, swap)
        .into_iter()
        .find_map(|eye| {
            let point = eye.local_point(point)?;
            let mvp = if cpu_only {
                projection::camera_pick_mvp
            } else {
                projection::camera_mvp
            };
            picking::cpu_hit(
                snapshot,
                mvp(&eye.camera, eye.aspect()),
                point,
                [eye.rect[2], eye.rect[3]],
                precision,
                cpu_only,
            )
        })
}

/// Project a world point into each visible eye, retaining display depth.
pub fn project(camera: &Camera, size: [f64; 2], swap: bool, position: Vector4) -> Vec<[f64; 3]> {
    cuemol_qsys::stereo::viewports(camera, size, swap)
        .into_iter()
        .filter_map(|eye| {
            let mvp = projection::camera_mvp(&eye.camera, eye.aspect());
            let p = [position.x as f32, position.y as f32, position.z as f32];
            let (x, y, depth) =
                crate::project_label(mvp, p, eye.rect[2] as f32, eye.rect[3] as f32)?;
            Some([
                f64::from(x) + eye.rect[0],
                f64::from(y) + eye.rect[1],
                f64::from(depth),
            ])
        })
        .collect()
}

pub fn center_mark_lines(
    camera: &Camera,
    size: [f32; 2],
    swap: bool,
    background: [f32; 4],
) -> Vec<crate::GuiScreenLine> {
    let color = if background[0] + background[1] + background[2] > 1.5 {
        "#202020"
    } else {
        "#e0e0e0"
    };
    let mut lines = Vec::new();
    for eye in cuemol_qsys::stereo::viewports(camera, size.map(f64::from), swap) {
        let [x, y, w, h] = eye.rect.map(|v| v as f32);
        let center = [x + w * 0.5, y + h * 0.5];
        let directions = match camera.center_mark() {
            cuemol_qsys::CenterMark::None => Vec::new(),
            cuemol_qsys::CenterMark::Cross => {
                vec![([-10., 0.], [10., 0.]), ([0., -10.], [0., 10.])]
            }
            cuemol_qsys::CenterMark::Axis => [
                Vector4::new3(20., 0., 0.),
                Vector4::new3(0., 20., 0.),
                Vector4::new3(0., 0., 20.),
            ]
            .into_iter()
            .map(|axis| {
                let p = eye.camera.rotation().to_matrix().mul_vec(axis);
                ([0., 0.], [p.x as f32, -p.y as f32])
            })
            .collect(),
        };
        for (from, to) in directions {
            lines.push(crate::GuiScreenLine {
                x1: center[0] + from[0],
                y1: center[1] + from[1],
                x2: center[0] + to[0],
                y2: center[1] + to[1],
                width: 1.,
                color: color.into(),
                stipple: false,
            });
        }
    }
    lines
}

pub fn annotation_lines(
    state: &DesktopAppState,
    camera: &Camera,
    size: [f32; 2],
    swap: bool,
) -> Vec<crate::GuiScreenLine> {
    let mut lines = Vec::new();
    for eye in cuemol_qsys::stereo::viewports(camera, size.map(f64::from), swap) {
        let [x, y, w, h] = eye.rect.map(|v| v as f32);
        let mvp = projection::camera_mvp(&eye.camera, eye.aspect());
        for mut line in state.active_atom_intr_lines(mvp, w, h, 10000) {
            let dx = line.x2 - line.x1;
            let dy = line.y2 - line.y1;
            let mut lo = 0_f32;
            let mut hi = 1_f32;
            let mut visible = true;
            for (p, q) in [
                (-dx, line.x1),
                (dx, w - line.x1),
                (-dy, line.y1),
                (dy, h - line.y1),
            ] {
                if p.abs() < f32::EPSILON {
                    if q < 0. {
                        visible = false;
                        break;
                    }
                } else if p < 0. {
                    lo = lo.max(q / p);
                } else {
                    hi = hi.min(q / p);
                }
            }
            if !visible || lo > hi {
                continue;
            }
            line.x2 = line.x1 + dx * hi + x;
            line.y2 = line.y1 + dy * hi + y;
            line.x1 += dx * lo + x;
            line.y1 += dy * lo + y;
            lines.push(line);
        }
    }
    lines
}
