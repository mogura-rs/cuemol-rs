use std::path::Path;

use cuemol_core::Uid;
use cuemol_gfx::Rgba8;

use crate::{DesktopAppState, GuiAtomLabel};

#[cfg(feature = "egui-gui")]
pub(crate) fn collect_pov_labels(
    state: &DesktopAppState,
    camera: &cuemol_qsys::Camera,
    width: u32,
    height: u32,
    enabled: bool,
) -> Vec<GuiAtomLabel> {
    let Some(scene_uid) = state.active_scene() else {
        return Vec::new();
    };
    collect_scene_pov_labels(state, scene_uid, camera, width, height, enabled)
}

pub(crate) fn collect_scene_pov_labels(
    state: &DesktopAppState,
    scene_uid: Uid,
    camera: &cuemol_qsys::Camera,
    width: u32,
    height: u32,
    enabled: bool,
) -> Vec<GuiAtomLabel> {
    if !enabled || width == 0 || height == 0 {
        return Vec::new();
    }
    const MAX_LABELS: usize = 10_000;
    let aspect = f64::from(width) / f64::from(height);
    let mvp = crate::offscreen::camera_mvp(camera, aspect);
    let mut labels = state.name_labels(scene_uid, mvp, width as f32, height as f32, MAX_LABELS);
    if labels.len() < MAX_LABELS {
        labels.extend(state.atom_intr_labels(
            scene_uid,
            mvp,
            width as f32,
            height as f32,
            MAX_LABELS - labels.len(),
        ));
    }
    labels
}

pub(crate) fn overlay_pov_labels(path: &Path, labels: &[GuiAtomLabel]) -> Result<(), String> {
    let mut frame = image::open(path)
        .map_err(|error| format!("failed to read label target {}: {error}", path.display()))?
        .to_rgba8();
    for label in labels {
        draw_label(&mut frame, label);
    }
    frame
        .save(path)
        .map_err(|error| format!("failed to write labelled frame {}: {error}", path.display()))
}

fn draw_label(frame: &mut image::RgbaImage, label: &GuiAtomLabel) {
    let image = cuemol_desktop_core::labels::rasterize_label(label, 1.);
    let left = (label.x - label.anchor[0] * image.logical_size[0] - 2.).round() as i32;
    let top = (label.y - label.anchor[1] * image.logical_size[1] - 2.).round() as i32;
    let color = cuemol_gfx::parse_color(&label.color)
        .ok()
        .and_then(|color| color.rgba8())
        .unwrap_or(Rgba8 {
            r: 255,
            g: 255,
            b: 0,
            a: 255,
        });
    for (i, coverage) in image.alpha.iter().enumerate() {
        if *coverage == 0 {
            continue;
        }
        blend_label_pixel(
            frame,
            left + (i % image.width as usize) as i32,
            top + (i / image.width as usize) as i32,
            color,
            f32::from(*coverage) / 255.,
        );
    }
}

fn blend_label_pixel(frame: &mut image::RgbaImage, x: i32, y: i32, color: Rgba8, coverage: f32) {
    if x < 0 || y < 0 || x >= frame.width() as i32 || y >= frame.height() as i32 {
        return;
    }
    let pixel = frame.get_pixel_mut(x as u32, y as u32);
    let source_alpha = coverage.clamp(0.0, 1.0) * f32::from(color.a) / 255.0;
    let target_alpha = f32::from(pixel[3]) / 255.0;
    let output_alpha = source_alpha + target_alpha * (1.0 - source_alpha);
    if output_alpha <= f32::EPSILON {
        return;
    }
    for (target, source) in pixel.0[..3].iter_mut().zip([color.r, color.g, color.b]) {
        let target_value = f32::from(*target) / 255.0;
        let source_value = f32::from(source) / 255.0;
        let output = (source_value * source_alpha
            + target_value * target_alpha * (1.0 - source_alpha))
            / output_alpha;
        *target = (output * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = (output_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlays_projected_labels_on_pov_png() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-pov-label-overlay-{}.png",
            std::process::id()
        ));
        image::RgbaImage::from_pixel(96, 48, image::Rgba([255, 255, 255, 255]))
            .save(&path)
            .unwrap();
        overlay_pov_labels(
            &path,
            &[GuiAtomLabel {
                anchor: [0., 1.],
                depth: 0.,
                on_top: true,
                x: 8.0,
                y: 24.0,
                text: "CA".to_string(),
                color: "#ff0000".to_string(),
                font_size: 18.0,
                font_name: "sans-serif".to_string(),
                font_style: "normal".to_string(),
                font_weight: "bold".to_string(),
            }],
        )
        .unwrap();
        let frame = image::open(&path).unwrap().to_rgba8();
        assert!(frame.pixels().any(|pixel| pixel.0 != [255, 255, 255, 255]));
        std::fs::remove_file(path).unwrap();
    }
}
