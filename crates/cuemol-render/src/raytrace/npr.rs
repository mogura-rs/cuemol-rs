use super::{render_umbreon_snapshot, UmbreonFrame, UmbreonRenderParams};
use crate::RenderSceneSnapshot;
use cuemol_core::Result;
use cuemol_qsys::Camera;

#[derive(Debug, Clone)]
pub struct HatchRenderParams {
    pub density: f64,
    pub width_scale: f64,
    pub ink: [u8; 3],
    pub paper: [u8; 3],
    pub colored: bool,
}
impl Default for HatchRenderParams {
    fn default() -> Self {
        Self {
            density: 1.,
            width_scale: 1.,
            ink: [0; 3],
            paper: [255; 3],
            colored: false,
        }
    }
}

pub fn render_umbreon_npr_snapshot(
    snapshot: &RenderSceneSnapshot,
    camera: &Camera,
    background: [f32; 4],
    params: &UmbreonRenderParams,
    hatch: &HatchRenderParams,
) -> Result<UmbreonFrame> {
    let mut shaded = params.clone();
    shaded.transparent_background = true;
    shaded.use_gi = false;
    let mut frame = render_umbreon_snapshot(snapshot, camera, background, &shaded)?;
    let source = std::mem::take(&mut frame.pixels);
    let width = frame.width as usize;
    let height = frame.height as usize;
    let components = if params.transparent_background { 4 } else { 3 };
    frame.components = components as i32;
    frame.pixels.resize(width * height * components, 0);
    let spacing = 7. / hatch.density.clamp(0.1, 10.);
    let line_width = hatch.width_scale.clamp(0.1, 4.) * 0.8;
    for y in 0..height {
        for x in 0..width {
            let offset = (y * width + x) * 4;
            let color = &source[offset..offset + 4];
            let alpha = color[3] as f64 / 255.;
            let tone = 1. - *color[..3].iter().max().unwrap() as f64 / 255.;
            let diagonal = (x as f64 + y as f64 * 0.65).rem_euclid(spacing);
            let crossing = (x as f64 - y as f64 * 0.85).rem_euclid(spacing);
            let hatch_line = (tone > 0.13 && diagonal < line_width)
                || (tone > 0.48 && crossing < line_width)
                || (tone > 0.78 && diagonal < line_width * 2.);
            let boundary = [
                (x.saturating_sub(1), y),
                ((x + 1).min(width - 1), y),
                (x, y.saturating_sub(1)),
                (x, (y + 1).min(height - 1)),
            ]
            .iter()
            .any(|&(nx, ny)| source[(ny * width + nx) * 4 + 3] < 64);
            let ink = hatch_line || boundary;
            for channel in 0..3 {
                let paper = if hatch.colored {
                    0.45 * color[channel] as f64 + 0.55 * hatch.paper[channel] as f64
                } else {
                    hatch.paper[channel] as f64
                };
                let foreground = if ink {
                    hatch.ink[channel] as f64
                } else {
                    paper
                };
                let value = if params.transparent_background {
                    foreground
                } else {
                    foreground * alpha + background[channel] as f64 * 255. * (1. - alpha)
                };
                frame.pixels[(y * width + x) * components + channel] =
                    value.round().clamp(0., 255.) as u8;
            }
            if components == 4 {
                frame.pixels[(y * width + x) * 4 + 3] = color[3];
            }
        }
    }
    Ok(frame)
}
