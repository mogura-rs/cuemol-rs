use crate::agent_runtime::{AgentCall, AgentSession, Wake};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use egui::{Context, TextureHandle};
use zeroize::Zeroize;

#[derive(Default)]
pub struct AgentPanel {
    session: AgentSession,
    pub visible: bool,
    pub docked: bool,
    draft: String,
    prompts: Vec<String>,
    prompt_history_dirty: bool,
    recall_index: Option<usize>,
    recall_draft: String,
    composing: bool,
    input_id: Option<egui::Id>,
    input_rect: Option<egui::Rect>,
    pub rects: std::collections::BTreeMap<String, [f32; 4]>,
    pending_events: Vec<egui::Event>,
    textures: std::collections::BTreeMap<usize, TextureHandle>,
}
impl std::fmt::Debug for AgentPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPanel")
            .field("visible", &self.visible)
            .field("running", &self.running())
            .finish_non_exhaustive()
    }
}
impl std::ops::Deref for AgentPanel {
    type Target = AgentSession;
    fn deref(&self) -> &Self::Target {
        &self.session
    }
}
impl std::ops::DerefMut for AgentPanel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session
    }
}
fn wake(ctx: &Context) -> Wake {
    let ctx = ctx.clone();
    std::sync::Arc::new(move || ctx.request_repaint())
}
impl AgentPanel {
    pub fn load_prompt_history(&mut self, source: &str) {
        if let Ok(values) = serde_json::from_str::<Vec<String>>(source) {
            self.prompts.clear();
            for value in values {
                let value = value.trim();
                if !value.is_empty() && !self.prompts.iter().any(|stored| stored == value) {
                    self.prompts.push(value.into());
                    if self.prompts.len() == 50 {
                        break;
                    }
                }
            }
        }
    }
    pub fn take_prompt_history_changed(&mut self) -> Option<String> {
        std::mem::take(&mut self.prompt_history_dirty)
            .then(|| serde_json::to_string(&self.prompts).unwrap())
    }
    fn remember_prompt(&mut self, prompt: &str) {
        self.prompts.retain(|value| value != prompt);
        self.prompts.insert(0, prompt.into());
        self.prompts.truncate(50);
        self.recall_index = None;
        self.prompt_history_dirty = true;
    }
    pub fn prepare_input(&mut self, context: &Context, input: &mut egui::RawInput) {
        self.pending_events.append(&mut input.events);
        input.events = std::mem::take(&mut self.pending_events);
        let focused = self
            .input_id
            .is_some_and(|id| context.memory(|memory| memory.has_focus(id)));
        let clicked = self.input_rect.is_some_and(|rect|input.events.iter().any(|event|matches!(event,egui::Event::PointerButton{pos,pressed:true,..}if rect.contains(*pos))));
        if !focused && !clicked {
            return;
        }
        let mut composing = self.composing;
        let mut committed = false;
        let boundary = input.events.iter().position(|event| {
            if let egui::Event::Ime(event) = event {
                committed |= matches!(event, egui::ImeEvent::Commit(_));
                composing = matches!(event, egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_));
            }
            !composing
                && !committed
                && matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Enter | egui::Key::ArrowUp | egui::Key::ArrowDown,
                        pressed: true,
                        ..
                    }
                )
        });
        if let Some(index) = boundary {
            self.pending_events = input.events.split_off(if index == 0 { 1 } else { index });
        }
        if !self.pending_events.is_empty() {
            context.request_repaint();
        }
    }

    pub fn start(
        &mut self,
        state: &mut DesktopAppState,
        camera: &mut Camera,
        prompt: String,
        ctx: &Context,
        revision: u64,
    ) {
        self.session
            .start(state, camera, prompt, wake(ctx), revision);
    }
    pub fn poll(&mut self, ctx: &Context) -> Option<AgentCall> {
        self.session.poll(wake(ctx))
    }
    pub fn show(&mut self, ctx: &Context) -> Option<String> {
        if !self.visible || self.docked {
            return None;
        }
        let mut open = self.visible;
        let mut prompt = None;
        egui::Window::new("AI Agent")
            .open(&mut open)
            .default_size([440., 560.])
            .show(ctx, |ui| prompt = self.show_docked(ui));
        self.visible = open;
        prompt
    }
    pub fn settings_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.session.poll_models();
        let before = self.settings_json();
        let previous_provider = self
            .settings
            .model_spec()
            .ok()
            .map(|(provider, _)| provider);
        ui.label("Messages and scene data are sent to the selected provider when you press Send.");
        ui.add_enabled_ui(!self.running(), |ui| {
            ui.label("Model (provider:model)");
            ui.text_edit_singleline(&mut self.settings.model);
            if ui
                .add_enabled(
                    self.model_job.is_none(),
                    egui::Button::new("Refresh available models"),
                )
                .clicked()
            {
                if let Err(error) = self.session.refresh_models(wake(&ctx)) {
                    self.session.status = error;
                }
            }
            if !self.models.is_empty() {
                egui::ComboBox::from_label("Available models")
                    .selected_text(&self.settings.model)
                    .show_ui(ui, |ui| {
                        for model in &self.session.models {
                            ui.selectable_value(
                                &mut self.session.settings.model,
                                model.clone(),
                                model,
                            );
                        }
                    });
            }
            egui::ComboBox::from_label("Reasoning")
                .selected_text(&self.settings.reasoning)
                .show_ui(ui, |ui| {
                    for value in ["default", "low", "medium", "high"] {
                        ui.selectable_value(&mut self.settings.reasoning, value.into(), value);
                    }
                });
            ui.checkbox(
                &mut self.settings.enter_sends,
                "Enter sends; Shift+Enter adds a line",
            );
            ui.label("API key");
            ui.add(egui::TextEdit::singleline(&mut *self.key).password(true));
            #[cfg(not(target_arch = "wasm32"))]
            ui.horizontal(|ui| {
                if ui.button("Save in OS keychain").clicked() {
                    if let Err(error) = self.session.save_key() {
                        self.session.status = error;
                    }
                }
                if ui.button("Delete stored key").clicked() {
                    if let Err(error) = self.session.delete_key() {
                        self.session.status = error;
                    }
                }
            });
            #[cfg(target_arch = "wasm32")]
            ui.label("The key is kept only in this tab's memory.");
            ui.collapsing("Custom API endpoint", |ui| {
                ui.text_edit_singleline(&mut self.settings.endpoint);
            });
        });
        if previous_provider
            != self
                .settings
                .model_spec()
                .ok()
                .map(|(provider, _)| provider)
        {
            self.key.zeroize();
            self.models.clear();
            self.model_job = None;
        }
        self.settings_dirty |= before != self.settings_json();
    }

    pub fn show_docked(&mut self, ui: &mut egui::Ui) -> Option<String> {
        self.rects.clear();
        let ctx = ui.ctx().clone();
        ui.collapsing("Agent settings", |ui| self.settings_ui(ui));
        ui.separator();
        let footer_height = 5. * ui.text_style_height(&egui::TextStyle::Body)
            + 2. * ui.spacing().interact_size.y
            + 7. * ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .min_scrolled_height(0.)
            .max_height((ui.available_height() - footer_height).max(60.))
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.session.messages.is_empty() {
                    ui.weak("Describe what you want to see. For example: load 1CRN and show it as a cartoon.");
                }
                for (index, message) in self.session.transcript() {
                    if let Some(tool) = message.tool {
                        let status = tool.result.map_or("running...",|(_,ok)|if ok {"ok"} else {"failed"});
                        egui::CollapsingHeader::new(format!("{} · {status}",tool.name)).id_salt(("agent-tool",index)).show(ui,|ui| {
                            ui.strong("Input");
                            ui.add(egui::Label::new(tool.input).wrap().selectable(true));
                            if let Some((result,_)) = tool.result {
                                ui.strong("Result");
                                ui.add(egui::Label::new(result).wrap().selectable(true));
                            }
                        });
                    } else {
                        ui.strong(message.role);
                        ui.add(egui::Label::new(message.text).wrap().selectable(true));
                    }
                    if let Some(data) = message.image {
                        if let std::collections::btree_map::Entry::Vacant(entry) =
                            self.textures.entry(index)
                        {
                            use base64::Engine;
                            if let Some(rgba) = base64::engine::general_purpose::STANDARD
                                .decode(data)
                                .ok()
                                .and_then(|bytes| image::load_from_memory(&bytes).ok())
                                .map(|image| image.into_rgba8())
                            {
                                entry.insert(ctx.load_texture(
                                    format!("agent-image-{index}"),
                                    egui::ColorImage::from_rgba_unmultiplied(
                                        [rgba.width() as usize, rgba.height() as usize],
                                        &rgba,
                                    ),
                                    Default::default(),
                                ));
                            }
                        }
                        if let Some(image) = self.textures.get(&index) {
                            ui.add(
                                egui::Image::new((image.id(), image.size_vec2()))
                                    .max_size(egui::vec2(360., 270.)),
                            );
                        }
                    }
                }
            });
        let composer_id = ui.make_persistent_id("agent-composer");
        self.input_id = Some(composer_id);
        let focused = ui.memory(|memory| memory.has_focus(composer_id));
        let mut committed = false;
        ui.input(|input| {
            for event in &input.events {
                if let egui::Event::Ime(event) = event {
                    committed |= matches!(event, egui::ImeEvent::Commit(_));
                    self.composing =
                        matches!(event, egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_));
                }
            }
        });
        let mut send = false;
        let mut recalled = false;
        if focused && !self.running() && !self.composing && !committed {
            let range = egui::TextEdit::load_state(ui.ctx(), composer_id)
                .and_then(|state| state.cursor.char_range());
            let cursor = range.map_or(self.draft.chars().count(), |range| range.primary.index);
            let before: String = self.draft.chars().take(cursor).collect();
            let after: String = self.draft.chars().skip(cursor).collect();
            if range.is_none_or(|range| range.is_empty()) {
                for (key, backward, edge) in [
                    (egui::Key::ArrowUp, true, !before.contains('\n')),
                    (egui::Key::ArrowDown, false, !after.contains('\n')),
                ] {
                    let available = if backward {
                        self.recall_index.map_or(!self.prompts.is_empty(), |index| {
                            index + 1 < self.prompts.len()
                        })
                    } else {
                        self.recall_index.is_some()
                    };
                    if edge
                        && available
                        && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key))
                    {
                        if backward {
                            if self.recall_index.is_none() {
                                self.recall_draft = self.draft.clone();
                            }
                            self.recall_index =
                                Some(self.recall_index.map_or(0, |index| index + 1));
                        } else {
                            self.recall_index =
                                self.recall_index.and_then(|index| index.checked_sub(1));
                        }
                        self.draft = self.recall_index.map_or_else(
                            || self.recall_draft.clone(),
                            |index| self.prompts[index].clone(),
                        );
                        recalled = true;
                    }
                }
            }
            if recalled {
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), composer_id) {
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(self.draft.chars().count()),
                        )));
                    state.store(ui.ctx(), composer_id);
                }
            }
            let enter_sends = self.settings.enter_sends;
            send = ui.input_mut(|input| {
                let before = input.events.len();
                input.events.retain(|event| !matches!(event,egui::Event::Key{key:egui::Key::Enter,pressed:true,modifiers,..}if !modifiers.shift && !modifiers.alt && if enter_sends { *modifiers == egui::Modifiers::NONE } else {modifiers.command || modifiers.ctrl}));
                input.events.len() != before
            });
        }
        let draft_before_input = self.draft.clone();
        let response = ui.add_enabled(
            !self.running(),
            egui::TextEdit::multiline(&mut self.draft)
                .id(composer_id)
                .lock_focus(true)
                .hint_text("Ask for what you want to see. Up arrow for earlier prompts")
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        self.input_rect = Some(response.rect);
        let rect = response.interact_rect;
        self.rects.insert(
            "Agent prompt".into(),
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
        );
        if self.draft != draft_before_input {
            self.recall_index = None;
        }
        ui.small(if self.settings.enter_sends {
            "Enter to send; Shift+Enter for a new line"
        } else if ui.ctx().os() == egui::os::OperatingSystem::Mac {
            "Cmd+Enter to send"
        } else {
            "Ctrl+Enter to send"
        });
        ui.horizontal(|ui| {
            send |= ui
                .add_enabled(
                    !self.running() && self.settings.enabled && !self.draft.trim().is_empty(),
                    egui::Button::new("Send"),
                )
                .clicked();
            if ui
                .add_enabled(self.running(), egui::Button::new("Stop"))
                .clicked()
            {
                self.cancel("Cancelled; completed edits are kept.");
            }
            if ui
                .add_enabled(!self.running(), egui::Button::new("Clear chat"))
                .clicked()
            {
                let _ = self.session.new_conversation();
                self.textures.clear();
            }
        });
        ui.label(&self.status);
        if !self.settings.enabled && self.running() {
            self.cancel("AI Agent disabled.");
        }
        if send && !self.running() && self.settings.enabled && !self.draft.trim().is_empty() {
            let prompt = std::mem::take(&mut self.draft).trim().to_string();
            self.remember_prompt(&prompt);
            Some(prompt)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(panel: &mut AgentPanel, ctx: &Context, events: Vec<egui::Event>) -> Option<String> {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(480., 800.),
            )),
            events,
            ..Default::default()
        };
        panel.prepare_input(ctx, &mut input);
        let mut prompt = None;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                prompt = panel.show_docked(ui);
            });
        });
        prompt
    }
    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    #[test]
    fn composer_recalls_prompts_and_preserves_drafts_and_multiline_caret() {
        let mut panel = AgentPanel::default();
        panel.settings.enabled = true;
        panel.load_prompt_history(r#"["recent","older","recent","  "]"#);
        assert_eq!(panel.prompts, ["recent", "older"]);
        panel.draft = "draft".into();
        let ctx = Context::default();
        frame(&mut panel, &ctx, vec![]);
        ctx.memory_mut(|memory| memory.request_focus(panel.input_id.unwrap()));
        frame(&mut panel, &ctx, vec![]);
        for (key_, expected) in [
            (egui::Key::ArrowUp, "recent"),
            (egui::Key::ArrowUp, "older"),
            (egui::Key::ArrowDown, "recent"),
            (egui::Key::ArrowDown, "draft"),
        ] {
            frame(&mut panel, &ctx, vec![key(key_, egui::Modifiers::NONE)]);
            assert_eq!(
                panel.draft,
                expected,
                "index={:?} focus={} cursor={:?}",
                panel.recall_index,
                ctx.memory(|memory| memory.has_focus(panel.input_id.unwrap())),
                egui::TextEdit::load_state(&ctx, panel.input_id.unwrap())
                    .unwrap()
                    .cursor
                    .char_range()
            );
        }
        panel.draft = "first\nsecond".into();
        let id = panel.input_id.unwrap();
        let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(panel.draft.chars().count()),
            )));
        state.store(&ctx, id);
        frame(
            &mut panel,
            &ctx,
            vec![key(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        );
        assert_eq!(panel.draft, "first\nsecond");
    }
    #[test]
    fn composer_queues_input_before_send_and_ignores_ime_confirmation() {
        let mut panel = AgentPanel::default();
        panel.settings.enabled = true;
        let ctx = Context::default();
        frame(&mut panel, &ctx, vec![]);
        ctx.memory_mut(|memory| memory.request_focus(panel.input_id.unwrap()));
        frame(&mut panel, &ctx, vec![]);
        assert!(frame(
            &mut panel,
            &ctx,
            vec![
                egui::Event::Text("first prompt".into()),
                key(egui::Key::Enter, egui::Modifiers::CTRL)
            ]
        )
        .is_none());
        assert_eq!(
            frame(&mut panel, &ctx, vec![]).as_deref(),
            Some("first prompt")
        );
        panel.settings.enter_sends = true;
        assert!(frame(
            &mut panel,
            &ctx,
            vec![
                egui::Event::Ime(egui::ImeEvent::Enabled),
                egui::Event::Ime(egui::ImeEvent::Preedit("input".into())),
                key(egui::Key::Enter, egui::Modifiers::NONE)
            ]
        )
        .is_none());
        assert!(frame(
            &mut panel,
            &ctx,
            vec![
                egui::Event::Ime(egui::ImeEvent::Commit("converted".into())),
                key(egui::Key::Enter, egui::Modifiers::NONE)
            ]
        )
        .is_none());
        assert_eq!(panel.prompts, ["first prompt"]);
        for index in 0..60 {
            panel.remember_prompt(&format!("prompt {index}"));
        }
        panel.remember_prompt("prompt 59");
        assert_eq!(panel.prompts.len(), 50);
        assert_eq!(panel.prompts[0], "prompt 59");
        assert!(panel.take_prompt_history_changed().is_some());
        assert!(panel.take_prompt_history_changed().is_none());
    }
}
