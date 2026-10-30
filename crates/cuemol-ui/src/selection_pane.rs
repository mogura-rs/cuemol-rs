use crate::{
    selection::Builder,
    shell::{self, Icon},
};
use cuemol_command::selection::{self, Evaluation};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Duration};

#[derive(Debug)]
pub enum Action {
    Apply {
        object: Uid,
        expression: String,
        remember: bool,
    },
    Center {
        object: Uid,
        expression: String,
    },
    Define {
        name: String,
        expression: String,
    },
}

impl Action {
    pub fn command(&self, state: &DesktopAppState) -> (&'static str, Value) {
        match self {
            Self::Apply {
                object, expression, ..
            } => (
                "set_mol_selection",
                json!({"objId":object.get(),"selection":expression}),
            ),
            Self::Center { object, expression } => (
                "center_view",
                json!({"objId":object.get(),"selection":expression,"zoom":false}),
            ),
            Self::Define { name, expression } => (
                "define_selection",
                json!({"name":name,"selection":expression,"styleSet":selection::definition_style_set(state)}),
            ),
        }
    }

    pub fn history_expression(&self) -> Option<&str> {
        match self {
            Self::Apply {
                expression,
                remember: true,
                ..
            }
            | Self::Define { expression, .. } => Some(expression),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct SelectionPane {
    pub rects: BTreeMap<String, [f32; 4]>,
    builder: Builder,
    scene: Option<Uid>,
    target: Option<Uid>,
    revision: Option<u64>,
    objects: Vec<(Uid, String)>,
    current: String,
    text: String,
    defining: bool,
    name: String,
    error: Option<String>,
    evaluation: Option<Evaluation>,
    evaluated_text: String,
    deadline: f64,
}

impl Default for SelectionPane {
    fn default() -> Self {
        Self {
            rects: BTreeMap::new(),
            builder: Builder::for_molecule(),
            scene: None,
            target: None,
            revision: None,
            objects: Vec::new(),
            current: String::new(),
            text: String::new(),
            defining: false,
            name: String::new(),
            error: None,
            evaluation: None,
            evaluated_text: String::new(),
            deadline: 0.,
        }
    }
}

impl SelectionPane {
    fn refresh(&mut self, state: &DesktopAppState, revision: u64) {
        if self.scene != state.active_scene() {
            *self = Self {
                scene: state.active_scene(),
                ..Self::default()
            };
        }
        if self.revision != Some(revision) {
            self.revision = Some(revision);
            self.objects = state
                .active_object_summaries()
                .into_iter()
                .map(|object| (object.uid, object.name))
                .collect();
            if !self.objects.iter().any(|(id, _)| Some(*id) == self.target) {
                self.set_target(self.objects.first().map(|(id, _)| *id));
            }
            self.evaluation = None;
        }
        let current = self
            .target
            .and_then(|id| state.molecule_selection_text(id).ok())
            .unwrap_or("");
        if self.current != current {
            self.current = current.into();
            self.text = current.into();
            self.error = None;
            self.evaluation = None;
        }
    }

    fn set_target(&mut self, target: Option<Uid>) {
        self.target = target;
        self.current.clear();
        self.text.clear();
        self.builder = Builder::for_molecule();
        self.defining = false;
        self.name.clear();
        self.error = None;
        self.evaluation = None;
    }

    pub fn completed(&mut self, action: &Action, error: Option<String>) {
        if error.is_none() && matches!(action, Action::Define { .. }) {
            self.defining = false;
            self.name.clear();
        }
        self.error = error;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        revision: u64,
    ) -> Option<Action> {
        self.rects.clear();
        self.refresh(state, revision);
        let mut action = None;
        ui.label("Molecule");
        let before = self.target;
        let selected = self
            .objects
            .iter()
            .find(|(id, _)| Some(*id) == self.target)
            .map(|(_, name)| name.as_str())
            .unwrap_or("(no molecules)");
        let response = egui::ComboBox::from_id_salt("selection-pane-molecule")
            .selected_text(selected)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (id, name) in &self.objects {
                    let response = ui.selectable_value(&mut self.target, Some(*id), name);
                    record(
                        ui,
                        &mut self.rects,
                        &format!("Selection molecule {name}"),
                        &response,
                    );
                }
            })
            .response;
        record(ui, &mut self.rects, "Selection molecule", &response);
        if self.target != before {
            self.set_target(self.target);
            self.refresh(state, revision);
        }
        ui.add_space(6.);
        ui.label("Selection");
        ui.add_enabled_ui(self.target.is_some(), |ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.text)
                    .id_salt("selection-pane-expression")
                    .desired_width(ui.available_width())
                    .font(egui::TextStyle::Monospace)
                    .hint_text("Input selection command"),
            );
            record(ui, &mut self.rects, "Selection expression", &response);
            if response.changed() {
                self.error = None;
            }
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let changed = self.text != self.current;
            let mut apply = enter && changed;
            ui.horizontal(|ui| {
                apply |= control(
                    ui,
                    &mut self.rects,
                    Icon::Selection,
                    "Apply selection",
                    changed,
                )
                .clicked();
                if control(
                    ui,
                    &mut self.rects,
                    Icon::Fit,
                    "Center view on selection",
                    !self.current.is_empty(),
                )
                .clicked()
                {
                    action = self.target.map(|object| Action::Center {
                        object,
                        expression: self.current.clone(),
                    });
                }
                if control(
                    ui,
                    &mut self.rects,
                    Icon::Clear,
                    "Clear selection",
                    !self.text.is_empty() || !self.current.is_empty(),
                )
                .clicked()
                {
                    self.text.clear();
                    action = self.target.map(|object| Action::Apply {
                        object,
                        expression: String::new(),
                        remember: false,
                    });
                }
                let define = control(
                    ui,
                    &mut self.rects,
                    Icon::Tag,
                    "Define name",
                    !self.text.trim().is_empty(),
                );
                if define.clicked() {
                    self.defining = !self.defining;
                    self.name.clear();
                }
                egui::Popup::from_response(&define)
                    .open_bool(&mut self.defining)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.set_min_width(180.);
                        let name = ui.add(
                            egui::TextEdit::singleline(&mut self.name)
                                .hint_text("name")
                                .desired_width(170.),
                        );
                        record(ui, &mut self.rects, "Selection definition name", &name);
                        let pressed =
                            name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let button = ui.add_enabled(
                            !self.name.trim().is_empty() && !self.text.trim().is_empty(),
                            egui::Button::new("Define"),
                        );
                        record(ui, &mut self.rects, "Define selection", &button);
                        if (pressed || button.clicked())
                            && !self.name.trim().is_empty()
                            && !self.text.trim().is_empty()
                        {
                            action = Some(Action::Define {
                                name: self.name.trim().into(),
                                expression: self.text.clone(),
                            });
                        }
                    });
            });
            if apply {
                action = self.target.map(|object| Action::Apply {
                    object,
                    expression: self.text.clone(),
                    remember: true,
                });
            }
            let now = ui.input(|input| input.time);
            if self.evaluated_text != self.text {
                self.evaluated_text = self.text.clone();
                self.evaluation = None;
                self.deadline = now + 0.25;
            }
            if self.evaluation.is_none() && !self.text.trim().is_empty() {
                if now >= self.deadline {
                    self.evaluation = Some(selection::evaluate(state, self.target, &self.text));
                } else {
                    ui.ctx()
                        .request_repaint_after(Duration::from_secs_f64(self.deadline - now));
                }
            }
            if let Some(evaluation) = &self.evaluation {
                if let Some(error) = &evaluation.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                } else if let Some(count) = evaluation.count {
                    ui.weak(format!("{count} atoms"));
                }
            }
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            ui.separator();
            if let Some(applied) =
                self.builder
                    .show(ui, state, self.target, revision, &self.current)
            {
                action = self.target.map(|object| Action::Apply {
                    object,
                    expression: applied.expression,
                    remember: false,
                });
            }
            self.rects.extend(self.builder.rects.clone());
        });
        if let Some(Action::Apply { expression, .. } | Action::Define { expression, .. }) = &action
        {
            if let Some(error) = selection::evaluate(state, self.target, expression).error {
                self.error = Some(error);
                return None;
            }
        }
        action
    }
}

fn control(
    ui: &mut egui::Ui,
    rects: &mut BTreeMap<String, [f32; 4]>,
    icon: Icon,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            shell::icon_button(ui, icon, label, false, egui::vec2(28., 26.))
        })
        .inner;
    record(ui, rects, label, &response);
    response
}

fn record(
    ui: &egui::Ui,
    rects: &mut BTreeMap<String, [f32; 4]>,
    label: &str,
    response: &egui::Response,
) {
    if ui.is_rect_visible(response.rect) {
        let rect = response.rect;
        rects.insert(
            label.into(),
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_preserves_drafts_until_applied_selection_or_context_changes() {
        let mut state = DesktopAppState::new();
        state.new_scene("first", true);
        state.open_bytes("selection.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let object = state.active_object_summaries()[0].uid;
        let mut pane = SelectionPane::default();
        pane.refresh(&state, 0);
        pane.text = "name C1".into();
        pane.builder.draft.set_keyword("name");
        pane.builder
            .draft
            .fields
            .insert("value".into(), "C1".into());
        state.remember_selection("chain A");
        pane.refresh(&state, 1);
        assert_eq!(pane.text, "name C1");
        assert_eq!(pane.current, "");
        assert_eq!(pane.builder.draft.fields["value"], "C1");
        state
            .set_molecule_selection_text(object, "chain A")
            .unwrap();
        pane.refresh(&state, 2);
        assert_eq!(pane.text, "chain A");
        assert_eq!(pane.current, "chain A");
        state.new_scene("second", true);
        pane.refresh(&state, 3);
        assert!(pane.target.is_none());
        assert!(pane.text.is_empty());
        assert_eq!(pane.builder.draft.keyword, "hierarchical");
    }
}
