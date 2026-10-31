use cuemol_command::console::{Completion, Dialect, Session};
use cuemol_desktop_core::DesktopAppState;
use std::collections::BTreeMap;

fn consume_plain_key(ui: &egui::Ui, wanted: egui::Key) -> bool {
    ui.input_mut(|input| {
        let before = input.events.len();
        input.events.retain(|event| !matches!(event, egui::Event::Key { key, pressed: true, modifiers, .. } if *key == wanted && *modifiers == egui::Modifiers::NONE));
        input.events.len() != before
    })
}

#[derive(Clone, Copy)]
pub enum ConsoleIntent {
    Run,
    Clear,
    Help,
    Stop,
}

#[derive(Debug, Default)]
pub struct ConsoleEditor {
    pub source: String,
    pub dialect: Dialect,
    pub history: Vec<String>,
    pub history_position: usize,
    draft: String,
    completions: Vec<Completion>,
    completion_index: usize,
    composing: bool,
    input_id: Option<egui::Id>,
    pending_events: Vec<egui::Event>,
    pub rects: BTreeMap<String, [f32; 4]>,
}

impl ConsoleEditor {
    /// Call before egui processes raw input so console actions keep their event order.
    pub fn prepare_input(&mut self, context: &egui::Context, input: &mut egui::RawInput) {
        self.pending_events.append(&mut input.events);
        input.events = std::mem::take(&mut self.pending_events);
        let focused = self
            .input_id
            .is_some_and(|id| context.memory(|memory| memory.has_focus(id)));
        let clicked = self.rects.get("Console input").is_some_and(|rect| {
            let rect = egui::Rect::from_min_size(
                egui::pos2(rect[0], rect[1]),
                egui::vec2(rect[2], rect[3]),
            );
            input.events.iter().any(|event| {
                matches!(event, egui::Event::PointerButton { pos, pressed: true, .. } if rect.contains(*pos))
            })
        });
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
                && matches!(event, egui::Event::Key {
                key: egui::Key::Enter | egui::Key::Tab | egui::Key::Escape
                    | egui::Key::ArrowUp | egui::Key::ArrowDown,
                pressed: true,
                modifiers,
                ..
            } if *modifiers == egui::Modifiers::NONE)
        });
        if let Some(index) = boundary {
            // TextEdit must finish preceding edits before a console action runs.
            self.pending_events = input.events.split_off(if index == 0 { 1 } else { index });
        }
        if !self.pending_events.is_empty() {
            context.request_repaint();
        }
    }

    pub fn remember(&mut self, source: &str) {
        self.history.push(source.to_string());
        if self.history.len() > 100 {
            self.history.remove(0);
        }
        self.history_position = self.history.len();
        self.draft.clear();
        self.completions.clear();
    }

    pub fn recall(&mut self, offset: isize) {
        if self.history_position == self.history.len() && offset < 0 {
            self.draft = self.source.clone();
        }
        self.history_position = self
            .history_position
            .saturating_add_signed(offset)
            .min(self.history.len());
        self.source = self
            .history
            .get(self.history_position)
            .cloned()
            .unwrap_or_else(|| self.draft.clone());
        self.completions.clear();
    }

    fn record(&mut self, name: &str, response: &egui::Response) {
        let rect = response.interact_rect;
        self.rects.insert(
            name.into(),
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
        );
    }

    fn finish_completion(&mut self) {
        if let Some(completion) = self.completions.get(self.completion_index) {
            self.source = completion.replacement.clone();
        }
        self.completions.clear();
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        session: &Session,
        logs: &[String],
        running: bool,
    ) -> Option<ConsoleIntent> {
        self.rects.clear();
        let mut intent = None;
        ui.horizontal(|ui| {
            for dialect in [Dialect::CueMol, Dialect::PyMol, Dialect::JavaScript] {
                let response = ui.selectable_value(&mut self.dialect, dialect, dialect.label());
                self.record(&format!("Console {}", dialect.label()), &response);
                if response.changed() {
                    self.completions.clear();
                }
            }
            for (label, action) in [
                ("Clear", ConsoleIntent::Clear),
                ("Help", ConsoleIntent::Help),
            ] {
                let response = ui.small_button(label);
                self.record(&format!("Console {label}"), &response);
                if response.clicked() {
                    intent = Some(action);
                }
            }
            if running {
                let stop = ui.small_button("Stop");
                self.record("Console Stop", &stop);
                if stop.clicked() {
                    intent = Some(ConsoleIntent::Stop);
                }
            }
        });
        let rows = self.source.lines().count().max(1).min(6);
        let input_height = (rows as f32 * ui.text_style_height(&egui::TextStyle::Monospace) + 6.)
            .max(ui.spacing().interact_size.y)
            .ceil();
        egui::ScrollArea::vertical()
            .id_salt("console_output")
            .auto_shrink([false, false])
            .min_scrolled_height(0.)
            .max_height(
                (ui.available_height() - input_height - ui.spacing().item_spacing.y).max(8.),
            )
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in logs.iter().skip(logs.len().saturating_sub(1000)) {
                    ui.monospace(line);
                }
            });
        let id = ui.make_persistent_id("console_command");
        self.input_id = Some(id);
        if !self.pending_events.is_empty() {
            ui.ctx().request_repaint();
        }
        let focused = ui.memory(|memory| memory.has_focus(id));
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
        let mut reset_cursor = false;
        if focused && !self.composing && !committed {
            if consume_plain_key(ui, egui::Key::Tab) {
                if self.completions.is_empty() {
                    self.completions = session.complete_dialect(state, self.dialect, &self.source);
                    self.completion_index = 0;
                    if self.completions.len() == 1 {
                        self.finish_completion();
                        reset_cursor = true;
                    }
                } else {
                    self.completion_index = (self.completion_index + 1) % self.completions.len();
                }
            }
            if consume_plain_key(ui, egui::Key::Escape) {
                self.completions.clear();
            }
            let cursor = egui::TextEdit::load_state(ui.ctx(), id)
                .and_then(|state| state.cursor.char_range())
                .map(|range| range.primary.index)
                .unwrap_or(self.source.chars().count());
            let before: String = self.source.chars().take(cursor).collect();
            let after: String = self.source.chars().skip(cursor).collect();
            for (key, direction, edge) in [
                (egui::Key::ArrowUp, -1, !before.contains('\n')),
                (egui::Key::ArrowDown, 1, !after.contains('\n')),
            ] {
                if (!self.completions.is_empty() || edge) && consume_plain_key(ui, key) {
                    if self.completions.is_empty() {
                        self.recall(direction);
                        reset_cursor = true;
                    } else {
                        self.completion_index = self
                            .completion_index
                            .saturating_add_signed(direction)
                            .min(self.completions.len() - 1);
                    }
                }
            }
            if consume_plain_key(ui, egui::Key::Enter) {
                if self.completions.is_empty() {
                    if !running {
                        intent = Some(ConsoleIntent::Run);
                    }
                } else {
                    self.finish_completion();
                    reset_cursor = true;
                }
            }
        }
        if reset_cursor {
            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(self.source.chars().count()),
                    )));
                state.store(ui.ctx(), id);
            }
        }
        let input = ui
            .horizontal(|ui| {
                ui.monospace(self.dialect.prompt());
                let input_width = (ui.available_width() - 48.).max(40.);
                let response = egui::ScrollArea::vertical()
                    .id_salt(id.with("input_scroll"))
                    .auto_shrink([false, false])
                    .min_scrolled_height(input_height)
                    .max_height(input_height)
                    .max_width(input_width)
                    .show(ui, |ui| {
                        ui.add_sized(
                            [input_width, input_height],
                            egui::TextEdit::multiline(&mut self.source)
                                .id(id)
                                .font(egui::TextStyle::Monospace)
                                .desired_rows(rows)
                                .lock_focus(true)
                                .return_key(egui::KeyboardShortcut::new(
                                    egui::Modifiers::SHIFT,
                                    egui::Key::Enter,
                                )),
                        )
                    })
                    .inner;
                self.record("Console input", &response);
                if response.changed() {
                    self.completions.clear();
                }
                let run = ui.add_enabled(
                    !running && !self.source.trim().is_empty(),
                    egui::Button::new("Run"),
                );
                self.record("Console Run", &run);
                if run.clicked() {
                    intent = Some(ConsoleIntent::Run);
                    response.request_focus();
                }
                response
            })
            .inner;
        if !self.completions.is_empty() {
            let height =
                (self.completions.len().min(8) as f32 * 20. + 12.).min(input.interact_rect.top());
            egui::Area::new(id.with("completions"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(
                    input.interact_rect.left(),
                    (input.interact_rect.top() - height).max(0.),
                ))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(height - 12.)
                            .show(ui, |ui| {
                                let mut chosen = None;
                                for (index, completion) in self.completions.iter().enumerate() {
                                    let response = ui.selectable_label(
                                        index == self.completion_index,
                                        &completion.display,
                                    );
                                    if index == self.completion_index {
                                        response.scroll_to_me(None);
                                    }
                                    if response.clicked() {
                                        chosen = Some(index);
                                    }
                                }
                                if let Some(index) = chosen {
                                    self.completion_index = index;
                                    self.finish_completion();
                                    input.request_focus();
                                }
                            });
                    });
                });
        }
        intent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Console {
        context: egui::Context,
        state: DesktopAppState,
        session: Session,
        editor: ConsoleEditor,
    }

    impl Console {
        fn new(source: &str) -> Self {
            let mut console = Self {
                context: egui::Context::default(),
                state: DesktopAppState::new(),
                session: Session::default(),
                editor: ConsoleEditor {
                    source: source.into(),
                    ..Default::default()
                },
            };
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                ..Default::default()
            };
            let _ = console.context.run(input, |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    let id = ui.make_persistent_id("console_command");
                    ui.memory_mut(|memory| memory.request_focus(id));
                    console
                        .editor
                        .show(ui, &console.state, &console.session, &[], false);
                });
            });
            console
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> Option<ConsoleIntent> {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events,
                ..Default::default()
            };
            self.editor.prepare_input(&self.context, &mut input);
            let mut intent = None;
            let _ = self.context.run(input, |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    intent = self.editor.show(ui, &self.state, &self.session, &[], false);
                });
            });
            intent
        }

        fn drain(&mut self, events: Vec<egui::Event>) -> Vec<String> {
            let mut submitted = Vec::new();
            let mut events = events;
            for _ in 0..30 {
                if matches!(
                    self.frame(std::mem::take(&mut events)),
                    Some(ConsoleIntent::Run)
                ) {
                    let source = std::mem::take(&mut self.editor.source);
                    self.editor.remember(&source);
                    submitted.push(source);
                }
                if self.editor.pending_events.is_empty() {
                    return submitted;
                }
            }
            panic!("Console input did not drain");
        }
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn enter_submits_before_later_text_in_the_same_frame() {
        let mut console = Console::new("close_log");
        assert!(matches!(
            console.frame(vec![key(egui::Key::Enter), egui::Event::Text("u".into())]),
            Some(ConsoleIntent::Run)
        ));
        assert_eq!(std::mem::take(&mut console.editor.source), "close_log");
        assert!(console
            .frame(vec![egui::Event::Text("ndo".into())])
            .is_none());
        assert_eq!(console.editor.source, "undo");
    }

    #[test]
    fn batched_submissions_preserve_commands_and_trailing_draft() {
        let mut console = Console::new("");
        let submitted = console.drain(vec![
            egui::Event::Text("close_log".into()),
            key(egui::Key::Enter),
            egui::Event::Text("undo".into()),
            key(egui::Key::Enter),
            egui::Event::Text("redo".into()),
        ]);
        assert_eq!(submitted, ["close_log", "undo"]);
        assert_eq!(console.editor.source, "redo");
    }

    #[test]
    fn history_recall_keeps_later_edits_and_original_draft() {
        let mut console = Console::new("");
        console.editor.remember("help");
        assert!(console
            .drain(vec![
                egui::Event::Text("draft".into()),
                key(egui::Key::ArrowUp),
                egui::Event::Text(" undo".into()),
            ])
            .is_empty());
        assert_eq!(console.editor.source, "help undo");
        console.drain(vec![key(egui::Key::ArrowDown)]);
        assert_eq!(console.editor.source, "draft");
    }

    #[test]
    fn shift_enter_inserts_a_newline_without_submitting() {
        let mut console = Console::new("help");
        let mut enter = key(egui::Key::Enter);
        if let egui::Event::Key { modifiers, .. } = &mut enter {
            *modifiers = egui::Modifiers::SHIFT;
        }
        assert!(console
            .drain(vec![enter, egui::Event::Text("undo".into())])
            .is_empty());
        assert_eq!(console.editor.source, "help\nundo");
    }

    #[test]
    fn ime_confirmation_does_not_submit() {
        let mut console = Console::new("");
        console.frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
        console.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit(
            "日本".into(),
        ))]);
        assert!(console
            .drain(vec![
                egui::Event::Ime(egui::ImeEvent::Commit("日本".into())),
                key(egui::Key::Enter),
            ])
            .is_empty());
        assert_eq!(console.editor.source, "日本");
    }

    #[test]
    fn submission_precedes_composition_for_the_next_command() {
        let mut console = Console::new("help");
        let submitted = console.drain(vec![
            key(egui::Key::Enter),
            egui::Event::Ime(egui::ImeEvent::Enabled),
            egui::Event::Ime(egui::ImeEvent::Preedit("日本".into())),
            egui::Event::Ime(egui::ImeEvent::Commit("日本".into())),
        ]);
        assert_eq!(submitted, ["help"]);
        assert_eq!(console.editor.source, "日本");
    }

    #[test]
    fn clicking_into_the_console_preserves_batched_submissions() {
        let mut console = Console::new("");
        console.context.memory_mut(|memory| {
            memory.surrender_focus(console.editor.input_id.unwrap());
        });
        let rect = console.editor.rects["Console input"];
        let pos = egui::pos2(rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.);
        let mut events = vec![egui::Event::PointerMoved(pos)];
        for pressed in [true, false] {
            events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        events.extend([
            egui::Event::Text("help".into()),
            key(egui::Key::Enter),
            egui::Event::Text("undo".into()),
        ]);
        assert_eq!(console.drain(events), ["help"]);
        assert_eq!(console.editor.source, "undo");
    }

    #[test]
    fn another_focused_widget_keeps_its_input_batch() {
        let mut console = Console::new("");
        console
            .context
            .memory_mut(|memory| memory.request_focus(egui::Id::new("other")));
        let events = vec![egui::Event::Text("draft".into()), key(egui::Key::Enter)];
        let mut input = egui::RawInput {
            events: events.clone(),
            ..Default::default()
        };
        console.editor.prepare_input(&console.context, &mut input);
        assert_eq!(input.events, events);
        assert!(console.editor.pending_events.is_empty());
    }
}
