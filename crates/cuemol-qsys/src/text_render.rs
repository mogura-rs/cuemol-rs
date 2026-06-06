use cuemol_gfx::PixelBuffer;

#[derive(Debug, Clone, PartialEq)]
pub struct Canvas2DTextRender2 {
    font_size: f64,
    font_name: String,
    css_font: String,
    mouse_cursor: i32,
}

impl Default for Canvas2DTextRender2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas2DTextRender2 {
    pub fn new() -> Self {
        Self {
            font_size: 0.0,
            font_name: String::new(),
            css_font: String::new(),
            mouse_cursor: 0,
        }
    }

    pub fn setup_font(
        &mut self,
        font_size: f64,
        font_name: impl AsRef<str>,
        font_style: impl AsRef<str>,
        font_weight: impl AsRef<str>,
    ) -> bool {
        let font_name = font_name.as_ref();
        let style = font_style.as_ref().to_ascii_lowercase();
        let weight = font_weight.as_ref().to_ascii_lowercase();

        let mut css = String::new();
        if style == "italic" || style == "oblique" {
            css.push_str(&style);
            css.push(' ');
        }
        if weight == "bold" {
            css.push_str("bold ");
        }
        css.push_str(&format!("{}px {}", font_size as i32, font_name));

        self.font_size = font_size;
        self.font_name = font_name.to_string();
        self.css_font = css;
        true
    }

    pub fn css_font(&self) -> &str {
        &self.css_font
    }

    pub fn font_name(&self) -> &str {
        &self.font_name
    }

    pub fn font_size(&self) -> f64 {
        self.font_size
    }

    pub fn render_text(&self, text: impl AsRef<str>, buffer: &mut PixelBuffer) -> bool {
        let height = self.font_size.ceil().max(1.0) as usize;
        let width = estimate_text_width(text.as_ref(), height);
        let size = width.saturating_mul(height);
        if buffer.size() < size {
            buffer.resize(size);
        }
        buffer.set_depth(8);
        buffer.set_width(width);
        buffer.set_height(height);
        true
    }

    pub fn mouse_cursor(&self) -> i32 {
        self.mouse_cursor
    }

    pub fn set_mouse_cursor(&mut self, cursor: i32) {
        self.mouse_cursor = cursor;
    }
}

pub fn create_text_render() -> Box<Canvas2DTextRender2> {
    let mut renderer = Canvas2DTextRender2::new();
    renderer.setup_font(12.0, "sans-serif", "normal", "normal");
    Box::new(renderer)
}

pub fn destroy_text_render(_renderer: Canvas2DTextRender2) {}

fn estimate_text_width(text: &str, height: usize) -> usize {
    let chars = text.chars().count().max(1);
    chars.saturating_mul((height / 2).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas2d_text_render_css_font_matches_reference_cases() {
        let mut render = Canvas2DTextRender2::new();
        assert!(render.setup_font(12.0, "sans-serif", "normal", "normal"));
        assert_eq!(render.css_font(), "12px sans-serif");
        render.setup_font(16.0, "serif", "italic", "normal");
        assert_eq!(render.css_font(), "italic 16px serif");
        render.setup_font(14.0, "monospace", "oblique", "normal");
        assert_eq!(render.css_font(), "oblique 14px monospace");
        render.setup_font(10.0, "Arial", "normal", "bold");
        assert_eq!(render.css_font(), "bold 10px Arial");
        render.setup_font(20.0, "Times New Roman", "italic", "bold");
        assert_eq!(render.css_font(), "italic bold 20px Times New Roman");
        render.setup_font(12.0, "sans-serif", "ITALIC", "BOLD");
        assert_eq!(render.css_font(), "italic bold 12px sans-serif");
        render.setup_font(15.9, "sans-serif", "normal", "normal");
        assert_eq!(render.css_font(), "15px sans-serif");
        render.set_mouse_cursor(3);
        assert_eq!(render.mouse_cursor(), 3);
    }

    #[test]
    fn canvas2d_text_render_factory_and_render_text_match_reference_shape() {
        let render = create_text_render();
        assert_eq!(render.css_font(), "12px sans-serif");
        let mut buffer = PixelBuffer::new();
        assert!(render.render_text("hello", &mut buffer));
        assert_eq!(buffer.depth(), 8);
        assert!(buffer.width() > 0);
        assert!(buffer.height() > 0);
    }
}
