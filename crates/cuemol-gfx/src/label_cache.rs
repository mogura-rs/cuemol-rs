use cuemol_core::Vector4;

use crate::{global_text_render_manager, PixelBuffer, TextRenderManager};

const SCALE_TOLERANCE: f64 = 1.0e-4;

pub trait PixelDrawTarget {
    fn pixel_scale(&self) -> f64;
    fn draw_pixels(&mut self, position: Vector4, buffer: &PixelBuffer);
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabelCacheEntry {
    id: i32,
    position: Vector4,
    buffer: Option<PixelBuffer>,
    text: String,
}

impl LabelCacheEntry {
    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn position(&self) -> Vector4 {
        self.position
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn buffer(&self) -> Option<&PixelBuffer> {
        self.buffer.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabelCacheImpl {
    entries: Vec<LabelCacheEntry>,
    font_size: f64,
    font_name: String,
    font_style: String,
    font_weight: String,
    next_id: i32,
    scale_factor: f64,
}

impl Default for LabelCacheImpl {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            font_size: 12.0,
            font_name: String::new(),
            font_style: String::new(),
            font_weight: String::new(),
            next_id: 0,
            scale_factor: -1.0,
        }
    }
}

impl LabelCacheImpl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_font(
        &mut self,
        font_size: f64,
        font_name: impl Into<String>,
        font_style: impl Into<String>,
        font_weight: impl Into<String>,
    ) {
        self.font_size = font_size;
        self.font_name = font_name.into();
        self.font_style = font_style.into();
        self.font_weight = font_weight.into();
    }

    pub fn font_size(&self) -> f64 {
        self.font_size
    }

    pub fn set_font_size(&mut self, value: f64) {
        self.font_size = value;
    }

    pub fn font_name(&self) -> &str {
        &self.font_name
    }

    pub fn set_font_name(&mut self, value: impl Into<String>) {
        self.font_name = value.into();
    }

    pub fn font_style(&self) -> &str {
        &self.font_style
    }

    pub fn set_font_style(&mut self, value: impl Into<String>) {
        self.font_style = value.into();
    }

    pub fn font_weight(&self) -> &str {
        &self.font_weight
    }

    pub fn set_font_weight(&mut self, value: impl Into<String>) {
        self.font_weight = value.into();
    }

    pub fn add_string(&mut self, position: Vector4, text: impl Into<String>) -> i32 {
        let id = self.next_id;
        self.entries.push(LabelCacheEntry {
            id,
            position,
            buffer: None,
            text: text.into(),
        });
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    pub fn remove(&mut self, id: i32) -> bool {
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return false;
        };
        self.entries.remove(index);
        true
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> &[LabelCacheEntry] {
        &self.entries
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }

    pub fn draw<T: PixelDrawTarget>(&mut self, target: &mut T) -> bool {
        let Ok(mut manager) = global_text_render_manager().lock() else {
            return false;
        };
        self.draw_with(target, &mut manager)
    }

    pub fn draw_with<T: PixelDrawTarget>(
        &mut self,
        target: &mut T,
        manager: &mut TextRenderManager,
    ) -> bool {
        let scale = target.pixel_scale();
        if !manager.setup_font(
            self.font_size * scale,
            &self.font_name,
            &self.font_style,
            &self.font_weight,
        ) {
            return false;
        }
        let use_cache = if self.scale_factor < 0.0 {
            self.scale_factor = scale;
            true
        } else {
            (scale - self.scale_factor).abs() < SCALE_TOLERANCE
        };

        for entry in &mut self.entries {
            if use_cache {
                if entry.buffer.is_none() {
                    let mut buffer = PixelBuffer::new();
                    if !manager.render_text(&entry.text, &mut buffer) {
                        return false;
                    }
                    entry.buffer = Some(buffer);
                }
                target.draw_pixels(
                    entry.position,
                    entry.buffer.as_ref().expect("label cache is missing"),
                );
            } else {
                let mut buffer = PixelBuffer::new();
                if !manager.render_text(&entry.text, &mut buffer) {
                    return false;
                }
                target.draw_pixels(entry.position, &buffer);
            }
        }
        true
    }

    pub fn render(&mut self, scale: f64) -> bool {
        let Ok(mut manager) = global_text_render_manager().lock() else {
            return false;
        };
        self.render_with(scale, &mut manager)
    }

    pub fn render_with(&mut self, scale: f64, manager: &mut TextRenderManager) -> bool {
        if !manager.setup_font(
            self.font_size * scale,
            &self.font_name,
            &self.font_style,
            &self.font_weight,
        ) {
            return false;
        }
        for entry in &mut self.entries {
            if entry.buffer.is_some() {
                continue;
            }
            let mut buffer = PixelBuffer::new();
            if !manager.render_text(&entry.text, &mut buffer) {
                return false;
            }
            entry.buffer = Some(buffer);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::TextRenderImpl;

    #[derive(Debug, Default)]
    struct RenderState {
        setup_sizes: Vec<f64>,
        texts: Vec<String>,
    }

    struct TestRenderer {
        state: Arc<Mutex<RenderState>>,
    }

    impl TextRenderImpl for TestRenderer {
        fn render_text(&mut self, text: &str, buffer: &mut PixelBuffer) -> bool {
            self.state.lock().unwrap().texts.push(text.to_string());
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
            self.state.lock().unwrap().setup_sizes.push(font_size);
            true
        }
    }

    #[derive(Default)]
    struct TestTarget {
        scale: f64,
        draws: Vec<(Vector4, usize)>,
    }

    impl PixelDrawTarget for TestTarget {
        fn pixel_scale(&self) -> f64 {
            self.scale
        }

        fn draw_pixels(&mut self, position: Vector4, buffer: &PixelBuffer) {
            self.draws.push((position, buffer.width()));
        }
    }

    fn test_manager() -> (TextRenderManager, Arc<Mutex<RenderState>>) {
        let state = Arc::new(Mutex::new(RenderState::default()));
        let mut manager = TextRenderManager::new();
        manager.set_impl(TestRenderer {
            state: state.clone(),
        });
        (manager, state)
    }

    #[test]
    fn cache_reuses_images_only_at_the_recorded_pixel_scale() {
        let (mut manager, state) = test_manager();
        let mut cache = LabelCacheImpl::new();
        cache.set_font(12.0, "sans", "normal", "bold");
        cache.add_string(Vector4::new3(1.0, 2.0, 3.0), "abc");
        cache.add_string(Vector4::ZERO, "de");
        let mut target = TestTarget {
            scale: 2.0,
            ..TestTarget::default()
        };

        assert!(cache.draw_with(&mut target, &mut manager));
        assert!(cache.draw_with(&mut target, &mut manager));
        assert_eq!(state.lock().unwrap().texts.len(), 2);
        assert_eq!(cache.scale_factor(), 2.0);
        target.scale = 1.0;
        assert!(cache.draw_with(&mut target, &mut manager));
        let state = state.lock().unwrap();
        assert_eq!(state.texts.len(), 4);
        assert_eq!(state.setup_sizes, vec![24.0, 24.0, 12.0]);
        assert_eq!(target.draws.len(), 6);
    }

    #[test]
    fn render_prefills_entries_and_ids_remain_monotonic() {
        let (mut manager, state) = test_manager();
        let mut cache = LabelCacheImpl::new();
        let first = cache.add_string(Vector4::ZERO, "one");
        let second = cache.add_string(Vector4::ZERO, "two");
        assert_eq!((first, second), (0, 1));
        assert!(cache.remove(first));
        assert!(!cache.remove(99));
        assert!(cache.render_with(1.5, &mut manager));
        assert!(cache.entries()[0].buffer().is_some());
        assert_eq!(state.lock().unwrap().texts, vec!["two"]);
        cache.invalidate_all();
        assert_eq!(cache.add_string(Vector4::ZERO, "three"), 2);
    }
}
