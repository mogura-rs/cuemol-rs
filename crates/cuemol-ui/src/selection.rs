use cuemol_command::selection::{self, Context, Draft, Evaluation, KEYWORDS};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use std::collections::BTreeMap;
use std::time::Duration;

const BINARY: &[(&str, &str)] = &[
    ("set", "Set"),
    ("add", "Add"),
    ("sub", "Sub"),
    ("intersect", "Intsec"),
];
const UNARY: &[(&str, &str)] = &[
    ("not", "Invert"),
    ("byres", "Byres"),
    ("sidechain", "Sidech"),
    ("mainchain", "Mainch"),
    ("around", "Arnd"),
    ("expand", "Expn"),
];

#[derive(Debug)]
pub struct Applied {
    pub expression: String,
    pub quick: bool,
}

#[derive(Debug, Default)]
pub struct Builder {
    pub rects: BTreeMap<String, [f32; 4]>,
    pub draft: Draft,
    tab: usize,
    context: Context,
    key: Option<(Option<Uid>, Option<Uid>, u64)>,
    candidates: BTreeMap<String, Vec<String>>,
    counts: BTreeMap<String, Evaluation>,
    count_request: Vec<String>,
    count_deadline: f64,
    counts_ready: bool,
    omit_applied: bool,
}

impl Builder {
    pub fn for_molecule() -> Self {
        Self {
            omit_applied: true,
            ..Self::default()
        }
    }

    pub fn opened(&mut self) {
        self.tab = 0;
        self.key = None;
    }

    fn refresh(&mut self, state: &DesktopAppState, molecule: Option<Uid>, revision: u64) {
        let key = (state.active_scene(), molecule, revision);
        if self.key == Some(key) {
            return;
        }
        if self
            .key
            .is_some_and(|previous| (previous.0, previous.1) != (key.0, key.1))
        {
            self.draft = Draft::default();
        }
        self.key = Some(key);
        self.context = selection::context(state, molecule);
        self.candidates.clear();
        self.counts.clear();
        self.count_request.clear();
        self.counts_ready = false;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        molecule: Option<Uid>,
        revision: u64,
        current: &str,
    ) -> Option<Applied> {
        self.rects.clear();
        self.refresh(state, molecule, revision);
        let mut applied = None;
        ui.horizontal(|ui| {
            for (index, name) in ["Named", "History", "Term", "Mod"].iter().enumerate() {
                let response = ui.selectable_value(&mut self.tab, index, *name);
                record(
                    ui,
                    &mut self.rects,
                    &format!("Selection tab {name}"),
                    &response,
                );
            }
        });
        ui.separator();
        let mut enter = false;
        match self.tab {
            0 => {
                egui::ScrollArea::vertical()
                    .id_salt("selection-named")
                    .max_height(230.)
                    .show(ui, |ui| {
                        if choice(ui, &mut self.rects, "all (*)", "*", current) {
                            applied = Some(Applied {
                                expression: "*".into(),
                                quick: true,
                            });
                        }
                        for (name, values) in [
                            (
                                "Selected",
                                if self.omit_applied || self.context.current.is_empty() {
                                    &[][..]
                                } else {
                                    std::slice::from_ref(&self.context.current)
                                },
                            ),
                            ("Scene", self.context.scene.as_slice()),
                            ("Global", self.context.global.as_slice()),
                        ] {
                            if values.is_empty() {
                                continue;
                            }
                            ui.weak(name);
                            for value in values {
                                if choice(ui, &mut self.rects, value, value, current) {
                                    applied = Some(Applied {
                                        expression: value.clone(),
                                        quick: true,
                                    });
                                }
                            }
                        }
                    });
            }
            1 => {
                if self.context.history.is_empty() {
                    ui.weak("No history");
                }
                egui::ScrollArea::vertical()
                    .id_salt("selection-history")
                    .max_height(230.)
                    .show_rows(ui, 24., self.context.history.len(), |ui, range| {
                        for index in range {
                            let value = &self.context.history[index];
                            if choice(ui, &mut self.rects, value, value, current) {
                                applied = Some(Applied {
                                    expression: value.clone(),
                                    quick: true,
                                });
                            }
                        }
                    });
            }
            2 => {
                let previous = self.draft.keyword.clone();
                let mut keyword = previous.clone();
                let response = egui::ComboBox::from_id_salt("selection-keyword")
                    .selected_text(self.draft.definition().map_or("", |value| value.label))
                    .show_ui(ui, |ui| {
                        for definition in KEYWORDS {
                            let response = ui.selectable_value(
                                &mut keyword,
                                definition.key.into(),
                                definition.label,
                            );
                            record(
                                ui,
                                &mut self.rects,
                                &format!("Selection keyword {}", definition.key),
                                &response,
                            );
                        }
                    })
                    .response;
                record(ui, &mut self.rects, "Selection keyword", &response);
                if keyword != previous {
                    self.draft.set_keyword(&keyword);
                }
                if let Some(definition) = self.draft.definition() {
                    if let Some(kind) = definition.candidates {
                        self.candidates
                            .entry(kind.into())
                            .or_insert_with(|| selection::values(state, molecule, kind));
                    }
                    match definition.kind {
                        "hierarchical" => {
                            ui.horizontal(|ui| {
                                let width =
                                    (ui.available_width() - 2. * ui.spacing().item_spacing.x) / 3.;
                                for (key, label) in
                                    [("chain", "chain"), ("resid", "resid"), ("aname", "atom")]
                                {
                                    let value = self.draft.fields.entry(key.into()).or_default();
                                    enter |= text(ui, &mut self.rects, key, value, label, width);
                                }
                            })
                            .response
                        }
                        "named" | "history" => {
                            let mut values = if definition.kind == "named" {
                                self.context
                                    .scene
                                    .iter()
                                    .chain(&self.context.global)
                                    .cloned()
                                    .collect::<Vec<_>>()
                            } else {
                                self.context.history.clone()
                            };
                            values.dedup();
                            let response = egui::ComboBox::from_id_salt("selection-picked")
                                .selected_text(if self.draft.picked.is_empty() {
                                    "Select..."
                                } else {
                                    &self.draft.picked
                                })
                                .width(ui.available_width())
                                .show_ui(ui, |ui| {
                                    for value in values {
                                        let response = ui.selectable_value(
                                            &mut self.draft.picked,
                                            value.clone(),
                                            &value,
                                        );
                                        record(
                                            ui,
                                            &mut self.rects,
                                            &format!("Selection candidate {value}"),
                                            &response,
                                        );
                                    }
                                })
                                .response;
                            record(ui, &mut self.rects, "Selection candidates", &response);
                            response
                        }
                        "none" => ui.label(""),
                        _ => {
                            ui.horizontal(|ui| {
                                if definition.kind == "compare" {
                                    let op = self
                                        .draft
                                        .fields
                                        .entry("op".into())
                                        .or_insert_with(|| "<".into());
                                    let response =
                                        egui::ComboBox::from_id_salt("selection-comparison")
                                            .selected_text(op.as_str())
                                            .width(50.)
                                            .show_ui(ui, |ui| {
                                                for value in ["<", ">", "<=", ">=", "="] {
                                                    ui.selectable_value(op, value.into(), value);
                                                }
                                            })
                                            .response;
                                    record(ui, &mut self.rects, "Selection comparison", &response);
                                } else if definition.kind == "nameValue" {
                                    let width = (ui.available_width() - 18.) / 2.;
                                    enter |= text(
                                        ui,
                                        &mut self.rects,
                                        "property",
                                        self.draft.fields.entry("name".into()).or_default(),
                                        "property",
                                        width,
                                    );
                                    ui.label("=");
                                }
                                let values = definition
                                    .candidates
                                    .and_then(|kind| self.candidates.get(kind));
                                let width = (ui.available_width()
                                    - if values.is_some_and(|v| !v.is_empty()) {
                                        34.
                                    } else {
                                        0.
                                    })
                                .max(24.);
                                enter |= text(
                                    ui,
                                    &mut self.rects,
                                    "value",
                                    self.draft.fields.entry("value".into()).or_default(),
                                    "value",
                                    width,
                                );
                                if let Some(values) = values.filter(|values| !values.is_empty()) {
                                    let response =
                                        egui::ComboBox::from_id_salt("selection-candidates")
                                            .selected_text("")
                                            .width(24.)
                                            .show_ui(ui, |ui| {
                                                for value in values {
                                                    let response = ui.button(value);
                                                    record(
                                                        ui,
                                                        &mut self.rects,
                                                        &format!("Selection candidate {value}"),
                                                        &response,
                                                    );
                                                    if response.clicked() {
                                                        self.draft
                                                            .fields
                                                            .insert("value".into(), value.clone());
                                                    }
                                                }
                                            })
                                            .response;
                                    record(ui, &mut self.rects, "Selection candidates", &response);
                                }
                            })
                            .response
                        }
                    };
                }
            }
            _ => {
                ui.horizontal(|ui| {
                    ui.label("Distance (Å)");
                    let response = egui::ComboBox::from_id_salt("selection-distance")
                        .selected_text(&self.draft.distance)
                        .show_ui(ui, |ui| {
                            for value in ["3", "4", "5", "6", "8", "10"] {
                                ui.selectable_value(&mut self.draft.distance, value.into(), value);
                            }
                        })
                        .response;
                    record(ui, &mut self.rects, "Selection distance", &response);
                });
            }
        }
        let operations = if self.tab == 2 {
            BINARY
        } else if self.tab == 3 {
            UNARY
        } else {
            &[]
        };
        let expressions: Vec<_> = operations
            .iter()
            .map(|(operation, _)| self.draft.apply(current, operation))
            .collect();
        let mut requested = if self.omit_applied {
            Vec::new()
        } else {
            vec![current.to_string()]
        };
        requested.extend(expressions.iter().flatten().cloned());
        requested.retain(|v| !v.trim().is_empty());
        requested.sort();
        requested.dedup();
        let now = ui.input(|input| input.time);
        if requested != self.count_request {
            self.count_request = requested;
            self.count_deadline = now + 0.25;
            self.counts_ready = false;
        }
        if !self.counts_ready {
            if now >= self.count_deadline {
                for expression in &self.count_request {
                    self.counts
                        .entry(expression.clone())
                        .or_insert_with(|| selection::evaluate(state, molecule, expression));
                }
                self.counts
                    .retain(|expression, _| self.count_request.contains(expression));
                self.counts_ready = true;
            } else {
                ui.ctx()
                    .request_repaint_after(Duration::from_secs_f64(self.count_deadline - now));
            }
        }
        if let Some(evaluation) = self.counts.get(current).filter(|_| !self.omit_applied) {
            if let Some(error) = &evaluation.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            } else if let Some(count) = evaluation.count {
                ui.weak(format!("{count} atoms"));
            }
        }
        for (row, pair) in operations.chunks(2).enumerate() {
            ui.horizontal(|ui| {
                let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
                for (column, (_, label)) in pair.iter().enumerate() {
                    let expression = &expressions[row * 2 + column];
                    let count = expression
                        .as_ref()
                        .and_then(|value| self.counts.get(value))
                        .and_then(|v| v.count);
                    let title = count
                        .map_or_else(|| label.to_string(), |count| format!("{label}  {count}"));
                    let response = ui.add_enabled(
                        expression.is_some(),
                        egui::Button::new(title).min_size(egui::vec2(width, 25.)),
                    );
                    record(
                        ui,
                        &mut self.rects,
                        &format!("Selection {label}"),
                        &response,
                    );
                    if response.clicked() {
                        applied = expression.clone().map(|expression| Applied {
                            expression,
                            quick: false,
                        });
                    }
                }
            });
        }
        if enter && current.trim().is_empty() {
            if let Some(expression) = self.draft.term() {
                applied = Some(Applied {
                    expression,
                    quick: true,
                });
            }
        }
        applied
    }
}

fn text(
    ui: &mut egui::Ui,
    rects: &mut BTreeMap<String, [f32; 4]>,
    key: &str,
    value: &mut String,
    hint: &str,
    width: f32,
) -> bool {
    let response = ui.add(
        egui::TextEdit::singleline(value)
            .id_salt(("selection-term", key))
            .hint_text(hint)
            .desired_width(width),
    );
    record(ui, rects, &format!("Selection {key}"), &response);
    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
}

fn choice(
    ui: &mut egui::Ui,
    rects: &mut BTreeMap<String, [f32; 4]>,
    label: &str,
    value: &str,
    current: &str,
) -> bool {
    let response = ui
        .selectable_label(value == current.trim(), label)
        .on_hover_text(value);
    record(ui, rects, &format!("Selection choice {value}"), &response);
    response.clicked()
}

fn record(
    ui: &egui::Ui,
    rects: &mut BTreeMap<String, [f32; 4]>,
    name: &str,
    response: &egui::Response,
) {
    if ui.is_rect_visible(response.rect) {
        let rect = response.rect;
        rects.insert(
            name.into(),
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
        );
    }
}
