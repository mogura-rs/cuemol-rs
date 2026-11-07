use crate::GuiAtomLabel;
use ab_glyph::{point, Font, FontRef, ScaleFont};

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;

pub(crate) fn font_families() -> Vec<String> {
    #[cfg(target_arch = "wasm32")]
    {
        browser::font_families()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::font_families()
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn font_catalog_generation() -> u32 {
    browser::font_catalog_generation()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn on_font_catalog_change(callback: impl Fn() + 'static) {
    browser::on_font_catalog_change(callback);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn request_font_catalog() {
    browser::request_font_catalog();
}

pub struct LabelImage {
    pub width: u32,
    pub height: u32,
    pub logical_size: [f32; 2],
    pub origin: [f32; 2],
    pub alpha: Vec<u8>,
}

pub fn rasterize_label(label: &GuiAtomLabel, scale: f32) -> LabelImage {
    rasterize_label_limited(label, scale, 4096)
}

fn rasterize_label_limited(label: &GuiAtomLabel, scale: f32, limit: u32) -> LabelImage {
    #[cfg(target_arch = "wasm32")]
    let image = browser::rasterize(label, scale, limit);
    #[cfg(not(target_arch = "wasm32"))]
    let image = native::rasterize(label, scale, limit);
    image.unwrap_or_else(|| rasterize_embedded(label, scale, limit))
}

fn pixel_size(label: &GuiAtomLabel, scale: f32) -> f32 {
    let font_size = if label.font_size.is_finite() {
        label.font_size
    } else {
        12.
    };
    let scale = if scale.is_finite() { scale } else { 1. };
    (font_size.clamp(1., 512.) as f32 * scale).clamp(1., 1024.)
}

fn font_weight(label: &GuiAtomLabel) -> u16 {
    match label.font_weight.to_ascii_lowercase().as_str() {
        "bold" | "bolder" => 700,
        "lighter" => 300,
        value => value.parse::<u16>().unwrap_or(400).clamp(1, 1000),
    }
}

fn rasterize_embedded(label: &GuiAtomLabel, scale: f32, limit: u32) -> LabelImage {
    let font = FontRef::try_from_slice(if label.font_name.to_ascii_lowercase().contains("mono") {
        epaint_default_fonts::HACK_REGULAR
    } else {
        epaint_default_fonts::UBUNTU_LIGHT
    })
    .expect("embedded label font is valid");
    let size = pixel_size(label, scale);
    let scaled = font.as_scaled(size);
    let line_height = (scaled.height() + scaled.line_gap()).max(1.);
    let line_width = |line: &str| {
        let mut previous = None;
        let mut width = 0.;
        for ch in line.chars() {
            let id = font.glyph_id(ch);
            if let Some(previous) = previous {
                width += scaled.kern(previous, id);
            }
            width += scaled.h_advance(id);
            previous = Some(id);
        }
        width
    };
    let style = label.font_style.to_ascii_lowercase();
    let italic = style == "italic" || style == "oblique";
    let bold = font_weight(label) >= 600;
    let logical_width = label
        .text
        .split('\n')
        .map(line_width)
        .fold(0., f32::max)
        .ceil();
    let logical_height = (line_height * label.text.split('\n').count() as f32).ceil();
    let width = (logical_width + 4. + if italic { size * 0.2 } else { 0. })
        .ceil()
        .clamp(1., limit as f32) as u32;
    let height = (logical_height + 4.).clamp(1., limit as f32) as u32;
    let mut alpha = vec![0_u8; width as usize * height as usize];
    for (line, text) in label.text.split('\n').enumerate() {
        let mut x = 2.;
        let baseline = 2. + scaled.ascent() + line as f32 * line_height;
        let mut previous = None;
        for ch in text.chars() {
            let id = font.glyph_id(ch);
            if let Some(previous) = previous {
                x += scaled.kern(previous, id);
            }
            let glyph = id.with_scale_and_position(size, point(x, baseline));
            if let Some(outline) = font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|dx, dy, coverage| {
                    let shear = if italic {
                        ((baseline - (bounds.min.y + dy as f32)) * 0.18).round() as i32
                    } else {
                        0
                    };
                    let x = bounds.min.x as i32 + dx as i32 + shear;
                    let y = bounds.min.y as i32 + dy as i32;
                    for x in x..=x + i32::from(bold) {
                        if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                            let pixel = &mut alpha[y as usize * width as usize + x as usize];
                            *pixel = (255. * coverage + f32::from(*pixel) * (1. - coverage)).round()
                                as u8;
                        }
                    }
                });
            }
            x += scaled.h_advance(id);
            previous = Some(id);
        }
    }
    LabelImage {
        width,
        height,
        logical_size: [logical_width, logical_height],
        origin: [2., 2.],
        alpha,
    }
}

#[cfg(feature = "offscreen")]
mod gpu;
#[cfg(feature = "offscreen")]
pub use gpu::LabelRenderer;

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn label(text: &str) -> GuiAtomLabel {
        GuiAtomLabel {
            x: 0.,
            y: 0.,
            text: text.into(),
            color: "white".into(),
            font_size: 32.,
            font_name: "Hack".into(),
            font_style: "normal".into(),
            font_weight: "normal".into(),
            anchor: [0., 0.],
            depth: 0.5,
            clip: None,
            on_top: true,
        }
    }

    #[test]
    fn named_font_family_controls_glyphs_and_monospaced_advances() {
        let narrow = rasterize_label(&label("iiii"), 1.);
        let wide = rasterize_label(&label("WWWW"), 1.);
        assert!((narrow.logical_size[0] - wide.logical_size[0]).abs() <= 1.);
        let mut proportional = label("iiii");
        proportional.font_name = "Ubuntu".into();
        let proportional = rasterize_label(&proportional, 1.);
        assert!(narrow.logical_size[0] > proportional.logical_size[0] * 1.5);
        assert_ne!(narrow.alpha, proportional.alpha);
    }

    #[test]
    fn font_style_and_weight_match_without_case_sensitivity() {
        let mut text = label("Glyphs");
        text.font_style = "italic".into();
        text.font_weight = "bold".into();
        let lower = rasterize_label(&text, 1.);
        text.font_style = "ITALIC".into();
        text.font_weight = "BOLD".into();
        let upper = rasterize_label(&text, 1.);
        assert_eq!(upper.logical_size, lower.logical_size);
        assert!(upper.alpha == lower.alpha);
        let regular = rasterize_label(&label("Glyphs"), 1.);
        assert!(regular.alpha != lower.alpha);
    }

    #[test]
    fn missing_bold_face_still_produces_bold_glyphs() {
        let mut text = label("Glyphs");
        let normal = rasterize_label(&text, 1.);
        text.font_weight = "bold".into();
        let bold = rasterize_label(&text, 1.);
        assert!(
            bold.alpha.iter().map(|&v| u64::from(v)).sum::<u64>()
                > normal.alpha.iter().map(|&v| u64::from(v)).sum::<u64>()
        );
    }
}
