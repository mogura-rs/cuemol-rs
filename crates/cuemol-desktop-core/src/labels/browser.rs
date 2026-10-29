use super::*;
use wasm_bindgen::JsCast;

pub(super) fn rasterize(label: &GuiAtomLabel, scale: f32, limit: u32) -> Option<LabelImage> {
    let canvas = web_sys::OffscreenCanvas::new(1, 1).ok()?;
    let context = canvas
        .get_context("2d")
        .ok()??
        .dyn_into::<web_sys::OffscreenCanvasRenderingContext2d>()
        .ok()?;
    let size = f64::from(pixel_size(label, scale));
    let style = match label.font_style.to_ascii_lowercase().as_str() {
        "italic" => "italic",
        "oblique" => "oblique",
        _ => "normal",
    };
    let fallback = format!("{}px sans-serif", size.floor());
    let requested = format!(
        "{style} {} {}px {}",
        font_weight(label),
        size.floor(),
        label.font_name
    );
    context.set_font(&fallback);
    context.set_font(&requested);
    let font = context.font();
    let mut logical_width = 0_f64;
    let logical_height = size.ceil() * label.text.split('\n').count() as f64;
    for text in label.text.split('\n') {
        let metrics = context.measure_text(text).ok()?;
        logical_width = logical_width.max(metrics.width());
    }
    logical_width = (logical_width.ceil() / 4.).ceil() * 4.;
    let width = logical_width.clamp(1., f64::from(limit)) as u32;
    let height = logical_height.clamp(1., f64::from(limit)) as u32;
    canvas.set_width(width);
    canvas.set_height(height);
    context.set_font(&font);
    context.set_text_baseline("bottom");
    context.set_fill_style_str("white");
    for (line, text) in label.text.split('\n').enumerate() {
        context
            .fill_text(text, 0., (line + 1) as f64 * size.ceil())
            .ok()?;
    }
    let pixels = context
        .get_image_data(0., 0., f64::from(width), f64::from(height))
        .ok()?
        .data();
    Some(LabelImage {
        width,
        height,
        logical_size: [logical_width as f32, logical_height as f32],
        origin: [0., 0.],
        alpha: pixels.0.chunks_exact(4).map(|pixel| pixel[3]).collect(),
    })
}
