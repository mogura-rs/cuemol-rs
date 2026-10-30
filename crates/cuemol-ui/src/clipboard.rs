use egui::{Context, Id, Rect, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Tree,
    Paint,
    Actions,
    Cameras,
}

#[derive(Debug, Default)]
pub struct Focus {
    pub scope: Option<Scope>,
    pub text: Option<Id>,
    regions: Vec<(Scope, Rect)>,
    menus: Vec<Rect>,
    pending: Vec<(Id, egui::Event)>,
    waiting: Option<Id>,
}

impl Focus {
    pub fn begin_frame(&mut self, ctx: &Context) {
        if let Some(position) = ctx.input(|input| {
            input
                .pointer
                .any_pressed()
                .then(|| input.pointer.interact_pos())
                .flatten()
        }) {
            let over_menu = self.menus.iter().any(|rect| rect.contains(position))
                || (egui::Popup::is_any_open(ctx)
                    && ctx
                        .layer_id_at(position)
                        .is_some_and(|layer| layer.order == egui::Order::Foreground));
            if !over_menu {
                self.scope = self
                    .regions
                    .iter()
                    .rev()
                    .find(|(_, rect)| rect.contains(position))
                    .map(|(scope, _)| *scope);
                self.text = None;
                self.waiting = None;
            }
        }
        if !ctx.input(|input| input.pointer.any_pressed()) {
            self.end_frame(ctx);
        }
        self.regions.clear();
        self.menus.clear();
        for (id, event) in self.pending.drain(..) {
            ctx.memory_mut(|memory| memory.request_focus(id));
            ctx.input_mut(|input| input.events.push(event));
        }
    }

    pub fn end_frame(&mut self, ctx: &Context) {
        let focused = ctx
            .memory(|memory| memory.focused())
            .filter(|id| egui::TextEdit::load_state(ctx, *id).is_some());
        let waiting = self.waiting.is_some()
            && !ctx.input(|input| {
                input.key_pressed(egui::Key::Escape) || input.key_pressed(egui::Key::Tab)
            });
        if focused.is_some()
            || (!egui::Popup::is_any_open(ctx) && self.pending.is_empty() && !waiting)
        {
            self.text = focused;
            if self.waiting != focused {
                self.waiting = None;
            }
        }
    }

    pub fn begin_text_read(&mut self) -> Option<Id> {
        self.waiting = self.text;
        self.text
    }

    pub fn finish_text_read(&mut self, id: Id) {
        if self.waiting == Some(id) {
            self.waiting = None;
        }
    }

    pub fn region(&mut self, scope: Scope, ui: &Ui) {
        let rect = ui.min_rect().intersect(ui.clip_rect());
        self.regions.push((scope, rect));
    }

    pub fn menu(&mut self, rect: Rect) {
        self.menus.push(rect);
    }

    pub fn text_event(&mut self, ctx: &Context, event: egui::Event) -> bool {
        let Some(id) = self.text else {
            return false;
        };
        self.pending.push((id, event));
        ctx.request_repaint();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_focus_clears_after_keyboard_navigation_but_survives_menu_dispatch() {
        let ctx = Context::default();
        let mut focus = Focus::default();
        let id = Id::new("clipboard-text");
        egui::text_edit::TextEditState::default().store(&ctx, id);
        ctx.memory_mut(|memory| memory.request_focus(id));
        focus.end_frame(&ctx);
        assert_eq!(focus.text, Some(id));
        ctx.memory_mut(|memory| memory.surrender_focus(id));
        focus.end_frame(&ctx);
        assert_eq!(focus.text, None);

        focus.text = Some(id);
        assert!(focus.text_event(&ctx, egui::Event::Cut));
        focus.end_frame(&ctx);
        assert_eq!(focus.text, Some(id));
        focus.begin_frame(&ctx);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
        assert!(ctx.input(|input| input.events.contains(&egui::Event::Cut)));
        ctx.memory_mut(|memory| memory.request_focus(Id::new("clipboard-button")));
        focus.end_frame(&ctx);
        assert_eq!(focus.text, None);

        focus.text = Some(id);
        ctx.memory_mut(|memory| memory.surrender_focus(Id::new("clipboard-button")));
        assert_eq!(focus.begin_text_read(), Some(id));
        focus.end_frame(&ctx);
        assert_eq!(focus.text, Some(id));
        focus.finish_text_read(id);
        focus.end_frame(&ctx);
        assert_eq!(focus.text, None);
    }
}
