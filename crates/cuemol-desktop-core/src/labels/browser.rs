use super::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
let fontFamilies = '';
let fontGeneration = 0;
let fontsRequested = false;
let fontChange;
export function cuemolFontFamilies() {
  if (!fontsRequested) {
    fontsRequested = true;
    if (typeof globalThis.queryLocalFonts === 'function') {
      Promise.resolve().then(() => globalThis.queryLocalFonts()).then(fonts => {
        fontFamilies = [...new Set(fonts.map(font => font.family).filter(name => typeof name === 'string' && name.trim()))].join('\n');
        fontGeneration += 1;
        fontChange?.();
      }).catch(() => {});
    }
  }
  return fontFamilies;
}
export function cuemolFontCatalogGeneration() { return fontGeneration; }
export function cuemolFontCatalogListener(callback) { fontChange = callback; }
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = cuemolFontFamilies)]
    fn browser_font_families() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = cuemolFontCatalogGeneration)]
    pub(super) fn font_catalog_generation() -> u32;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = cuemolFontCatalogListener)]
    fn font_catalog_listener(callback: &wasm_bindgen::JsValue);
}

pub(super) fn on_font_catalog_change(callback: impl Fn() + 'static) {
    use wasm_bindgen::closure::Closure;
    thread_local! {
        static LISTENER: std::cell::RefCell<Option<Closure<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
    }
    let callback = Closure::wrap(Box::new(callback) as Box<dyn Fn()>);
    font_catalog_listener(callback.as_ref());
    LISTENER.with(|listener| *listener.borrow_mut() = Some(callback));
}

pub(super) fn font_families() -> Vec<String> {
    browser_font_families()
        .lines()
        .map(str::to_string)
        .collect()
}

fn text_lines(text: &str) -> impl Iterator<Item = &str> {
    let mut remaining = Some(text);
    std::iter::from_fn(move || {
        let text = remaining.take()?;
        let Some(end) = text.find(['\r', '\n']) else {
            return Some(text);
        };
        let bytes = text.as_bytes();
        let paired = bytes
            .get(end + 1)
            .is_some_and(|&next| matches!((bytes[end], next), (b'\r', b'\n') | (b'\n', b'\r')));
        remaining = Some(&text[end + if paired { 2 } else { 1 }..]);
        Some(&text[..end])
    })
}

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
    let logical_height = size.ceil() * text_lines(&label.text).count() as f64;
    for text in text_lines(&label.text) {
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
    for (line, text) in text_lines(&label.text).enumerate() {
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
