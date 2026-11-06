use cuemol_core::{Uid, Vector4};
use cuemol_gfx::PixelBuffer;
use cuemol_qsys::Camera;
use cuemol_render::PovPixelImage;

use crate::DesktopAppState;

pub(crate) fn collect_images(
    app: &DesktopAppState,
    scene: Uid,
    camera: &Camera,
    width: u32,
    height: u32,
) -> Vec<PovPixelImage> {
    let mvp = crate::offscreen::camera_mvp(camera, f64::from(width) / f64::from(height));
    let inverse = glam::Mat4::from_cols_array_2d(&mvp).inverse();
    if !inverse.is_finite() {
        return Vec::new();
    }
    const MAX_LABELS: usize = 10_000;
    let mut labels = app.name_labels(scene, mvp, width as f32, height as f32, MAX_LABELS);
    labels.extend(app.atom_intr_labels(
        scene,
        mvp,
        width as f32,
        height as f32,
        MAX_LABELS - labels.len(),
    ));
    labels
        .into_iter()
        .filter_map(|label| {
            let image = crate::labels::rasterize_label(&label, 2.);
            let color = cuemol_gfx::parse_color(&label.color).ok()?.rgba8()?;
            let left = label.x - label.anchor[0] * image.logical_size[0] - 2.;
            let top = label.y - label.anchor[1] * image.logical_size[1] - 2.;
            let right = left + image.width as f32 / 2.;
            let bottom = top + image.height as f32 / 2.;
            let depth = if label.on_top { 0.0001 } else { label.depth };
            let corners =
                [[left, bottom], [right, bottom], [right, top], [left, top]].map(|[x, y]| {
                    let point = inverse.project_point3(glam::Vec3::new(
                        2. * x / width as f32 - 1.,
                        1. - 2. * y / height as f32,
                        depth,
                    ));
                    Vector4::new3(point.x as f64, point.y as f64, point.z as f64)
                });
            let mut pixels = PixelBuffer::new();
            pixels.set_width(image.width as usize);
            pixels.set_height(image.height as usize);
            pixels.resize(image.alpha.len());
            for (target, source) in pixels
                .data_mut()
                .chunks_exact_mut(image.width as usize)
                .zip(image.alpha.rchunks_exact(image.width as usize))
            {
                target.copy_from_slice(source);
            }
            Some(
                PovPixelImage::new(corners[0], [color.r, color.g, color.b, color.a], pixels)
                    .with_corners(corners),
            )
        })
        .collect()
}
