use std::sync::{Mutex, OnceLock};

use crate::PixelBuffer;

pub trait TextRenderImpl: Send {
    fn render_text(&mut self, text: &str, buffer: &mut PixelBuffer) -> bool;

    fn setup_font(
        &mut self,
        font_size: f64,
        font_name: &str,
        font_style: &str,
        font_weight: &str,
    ) -> bool;
}

#[derive(Default)]
pub struct TextRenderManager {
    implementation: Option<Box<dyn TextRenderImpl>>,
}

impl std::fmt::Debug for TextRenderManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TextRenderManager")
            .field("has_implementation", &self.implementation.is_some())
            .finish()
    }
}

impl TextRenderManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_impl<T>(&mut self, implementation: T)
    where
        T: TextRenderImpl + 'static,
    {
        self.implementation = Some(Box::new(implementation));
    }

    pub fn set_boxed_impl(&mut self, implementation: Box<dyn TextRenderImpl>) {
        self.implementation = Some(implementation);
    }

    pub fn clear_impl(&mut self) {
        self.implementation = None;
    }

    pub fn has_impl(&self) -> bool {
        self.implementation.is_some()
    }

    pub fn setup_font(
        &mut self,
        font_size: f64,
        font_name: &str,
        font_style: &str,
        font_weight: &str,
    ) -> bool {
        self.implementation.as_mut().is_some_and(|implementation| {
            implementation.setup_font(font_size, font_name, font_style, font_weight)
        })
    }

    pub fn render_text(&mut self, text: &str, buffer: &mut PixelBuffer) -> bool {
        self.implementation
            .as_mut()
            .is_some_and(|implementation| implementation.render_text(text, buffer))
    }
}

pub fn global_text_render_manager() -> &'static Mutex<TextRenderManager> {
    static MANAGER: OnceLock<Mutex<TextRenderManager>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(TextRenderManager::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestRenderer {
        font_size: f64,
        render_count: usize,
    }

    impl TextRenderImpl for TestRenderer {
        fn render_text(&mut self, text: &str, buffer: &mut PixelBuffer) -> bool {
            self.render_count += 1;
            buffer.set_width(text.len());
            buffer.set_height(1);
            buffer.resize(text.len());
            true
        }

        fn setup_font(
            &mut self,
            font_size: f64,
            _font_name: &str,
            _font_style: &str,
            _font_weight: &str,
        ) -> bool {
            self.font_size = font_size;
            true
        }
    }

    #[test]
    fn manager_delegates_or_returns_false_without_an_implementation() {
        let mut manager = TextRenderManager::new();
        let mut buffer = PixelBuffer::new();
        assert!(!manager.setup_font(12.0, "sans", "normal", "normal"));
        assert!(!manager.render_text("abc", &mut buffer));
        manager.set_impl(TestRenderer::default());
        assert!(manager.setup_font(12.0, "sans", "normal", "normal"));
        assert!(manager.render_text("abc", &mut buffer));
        assert_eq!(buffer.width(), 3);
    }
}
