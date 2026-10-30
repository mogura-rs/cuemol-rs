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
    pub rects: BTreeMap<String, [f32; 4]>,
}

impl ConsoleEditor {
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
