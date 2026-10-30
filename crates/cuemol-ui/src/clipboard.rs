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
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            if egui::TextEdit::load_state(ctx, id).is_some() {
                self.text = Some(id);
            }
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
