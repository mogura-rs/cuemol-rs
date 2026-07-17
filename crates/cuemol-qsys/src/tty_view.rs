use crate::{View, ViewCap};
use cuemol_core::{Matrix4, Uid, Vector4};
use cuemol_gfx::{AbstractColor, DisplayContext};
use std::fmt;

/// Display context that accepts and discards primitive drawing operations.
#[derive(Debug, Default)]
pub struct TtyDisplayContext {
    context: DisplayContext,
}

impl TtyDisplayContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_current(&mut self) -> bool {
        true
    }

    pub fn is_current(&self) -> bool {
        true
    }

    pub fn is_file(&self) -> bool {
        true
    }

    pub fn context(&self) -> &DisplayContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut DisplayContext {
        &mut self.context
    }

    pub fn vertex(&mut self, _value: Vector4) {}

    pub fn normal(&mut self, _value: Vector4) {}

    pub fn color(&mut self, _value: &dyn AbstractColor) {}

    pub fn push_matrix(&mut self) {}

    pub fn pop_matrix(&mut self) {}

    pub fn mult_matrix(&mut self, _value: Matrix4) {}

    pub fn load_matrix(&mut self, _value: Matrix4) {}

    pub fn set_polygon_mode(&mut self, _value: i32) {}

    pub fn start_points(&mut self) {}

    pub fn start_polygon(&mut self) {}

    pub fn start_lines(&mut self) {}

    pub fn start_line_strip(&mut self) {}

    pub fn start_triangles(&mut self) {}

    pub fn start_triangle_strip(&mut self) {}

    pub fn start_triangle_fan(&mut self) {}

    pub fn start_quad_strip(&mut self) {}

    pub fn start_quads(&mut self) {}

    pub fn end(&mut self) {}
}

/// Headless view whose display context discards all geometry.
#[derive(Debug)]
pub struct TtyView {
    view: View,
    context: TtyDisplayContext,
}

impl TtyView {
    pub fn new(uid: Uid, scene_uid: Uid) -> Self {
        Self {
            view: View::new(uid, scene_uid),
            context: TtyDisplayContext::new(),
        }
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    pub fn capabilities(&self) -> ViewCap {
        ViewCap::default()
    }

    pub fn display_context(&self) -> &TtyDisplayContext {
        &self.context
    }

    pub fn display_context_mut(&mut self) -> &mut TtyDisplayContext {
        &mut self.context
    }

    pub fn set_up_model_matrix(&mut self, _stereo_eye: i32) {}

    pub fn set_up_projection_matrix(&mut self, _width: u32, _height: u32) {}

    /// Run a scene display callback against the no-op display context.
    pub fn draw_scene<F>(&mut self, display: F)
    where
        F: FnOnce(&mut TtyDisplayContext),
    {
        self.context
            .context_mut()
            .set_view_id(self.view.uid().get());
        self.context
            .context_mut()
            .set_scene_id(self.view.scene_uid().get());
        self.context.set_current();
        display(&mut self.context);
    }
}

impl fmt::Display for TtyView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TTYView")
    }
}

pub type TTYView = TtyView;
pub type TTYDisplayContext = TtyDisplayContext;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tty_view_runs_display_callback_with_target_ids() {
        let mut view = TtyView::new(Uid::new(17), Uid::new(23));
        let mut called = false;
        view.draw_scene(|context| {
            called = true;
            assert!(context.is_current());
            assert!(context.is_file());
            assert_eq!(context.context().view_id(), 17);
            assert_eq!(context.context().scene_id(), 23);
            context.vertex(Vector4::new3(1.0, 2.0, 3.0));
        });
        assert!(called);
        assert_eq!(view.to_string(), "TTYView");
        assert_eq!(view.capabilities(), ViewCap::default());
    }
}
