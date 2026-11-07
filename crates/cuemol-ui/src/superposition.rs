use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const HISTORY_KEY: &str = "cuemol2.ui.histories.mol_superpose";
const ALGORITHMS: [(&str, &str); 3] = [("USalign", "US-align"), ("SSM", "SSM"), ("LSQ", "LSQ")];
const PIVOTS: [(&str, &str); 5] = [
    ("auto", "Auto (CA / C3')"),
    ("CA", "CA"),
    ("C3'", "C3'"),
    ("C4'", "C4'"),
    ("P", "P"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Accept,
    Cancel,
}

#[derive(Debug)]
pub struct SuperpositionDialog {
    pub scene: Option<Uid>,
    pub reference: Option<Uid>,
    pub moving: Option<Uid>,
    pub reference_selection: String,
    pub moving_selection: String,
    pub algorithm: String,
    pub pivot: String,
    pub recenter: bool,
    pub use_property: bool,
    pub write_rmsd: bool,
    pub error: String,
    pub rects: BTreeMap<String, [f32; 4]>,
    builder: crate::selection::Builder,
    choosing: Option<bool>,
}

impl Default for SuperpositionDialog {
    fn default() -> Self {
        Self {
            scene: None,
            reference: None,
            moving: None,
            reference_selection: String::new(),
            moving_selection: String::new(),
            algorithm: "USalign".into(),
            pivot: "auto".into(),
            recenter: true,
            use_property: false,
            write_rmsd: false,
            error: String::new(),
            rects: BTreeMap::new(),
            builder: crate::selection::Builder::for_molecule(),
            choosing: None,
        }
    }
}

impl SuperpositionDialog {
    pub fn opened(&mut self, state: &DesktopAppState) {
        *self = Self::default();
        self.scene = state.active_scene();
        let history: Value =
            serde_json::from_str(&state.config_value(HISTORY_KEY)).unwrap_or(Value::Null);
        let objects = state.active_object_summaries();
        let valid = |key: &str| {
            history[key]
                .as_u64()
                .map(Uid::new)
                .filter(|id| objects.iter().any(|o| o.uid == *id))
        };
        self.reference = valid("refObjId").or_else(|| objects.first().map(|o| o.uid));
        self.moving =
            valid("movObjId").or_else(|| objects.get(1).or(objects.first()).map(|o| o.uid));
        if let Some(value) = history["algo"]
            .as_str()
            .filter(|value| ALGORITHMS.iter().any(|a| a.0 == *value))
        {
            self.algorithm = value.into();
        }
        if let Some(value) = history["pivot"]
            .as_str()
            .filter(|value| PIVOTS.iter().any(|p| p.0 == *value))
        {
            self.pivot = value.into();
        }
        self.recenter = history["autoRecenter"].as_bool().unwrap_or(true);
        self.use_property = history["useprop"].as_bool().unwrap_or(false);
    }

    pub fn remember(&self, state: &mut DesktopAppState) {
        state.put_config_value(
            HISTORY_KEY,
            json!({
                "refObjId": self.reference.map(Uid::get), "movObjId": self.moving.map(Uid::get),
                "algo": self.algorithm, "pivot": self.pivot,
                "autoRecenter": self.recenter, "useprop": self.use_property,
            })
            .to_string(),
        );
        state.remember_selection(&self.reference_selection);
        state.remember_selection(&self.moving_selection);
    }

    pub fn validation_error(&self, state: &DesktopAppState) -> Option<&'static str> {
        if self.scene != state.active_scene() {
            return Some("The active scene changed. Reopen Molecular superposition.");
        }
        let objects = state.active_object_summaries();
        if ![self.reference, self.moving]
            .iter()
            .all(|id| objects.iter().any(|o| Some(o.uid) == *id))
        {
            return Some("Select reference and moving molecules.");
        }
        if !ALGORITHMS.iter().any(|a| a.0 == self.algorithm) {
            return Some("Select a superposition method.");
        }
        if self.algorithm == "USalign" && !PIVOTS.iter().any(|p| p.0 == self.pivot) {
            return Some("Select a pivot atom.");
        }
        None
    }

    pub fn arguments(&self) -> Value {
        json!({"refId": self.reference.map(Uid::get), "movId": self.moving.map(Uid::get),
            "refSel": self.reference_selection, "movSel": self.moving_selection,
            "algo": self.algorithm, "pivot": self.pivot, "recenter": self.recenter,
            "keepAsProperty": self.use_property})
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        revision: u64,
        rmsd_available: bool,
    ) -> Option<Action> {
        self.rects.clear();
        let mut action = None;
        egui::ScrollArea::vertical()
            .id_salt("superposition-fields")
            .max_height((ui.ctx().content_rect().height() - 200.).max(120.))
            .show(ui, |ui| {
                ui.strong("Algorithm");
                ui.horizontal(|ui| {
                    ui.label("Method");
                    for (value, label) in ALGORITHMS {
                        record(
                            &mut self.rects,
                            &format!("Method {value}"),
                            ui.radio_value(&mut self.algorithm, value.into(), label),
                        );
                    }
                });
                ui.add_enabled_ui(self.algorithm == "USalign", |ui| {
                    let combo = egui::ComboBox::from_label("Pivot atom")
                        .selected_text(
                            PIVOTS
                                .iter()
                                .find(|p| p.0 == self.pivot)
                                .map_or("Select pivot", |p| p.1),
                        )
                        .show_ui(ui, |ui| {
                            for (value, label) in PIVOTS {
                                record(
                                    &mut self.rects,
                                    &format!("Pivot {value}"),
                                    ui.selectable_value(&mut self.pivot, value.into(), label),
                                );
                            }
                        });
                    record(&mut self.rects, "Pivot atom", combo.response);
                });
                for moving in [false, true] {
                    let label = if moving { "Moving" } else { "Reference" };
                    ui.push_id(label, |ui| {
                        ui.group(|ui| {
                            ui.strong(label);
                            let object = if moving {
                                &mut self.moving
                            } else {
                                &mut self.reference
                            };
                            let objects = state.active_object_summaries();
                            let text = objects
                                .iter()
                                .find(|o| Some(o.uid) == *object)
                                .map_or("Select molecule", |o| o.name.as_str());
                            let before = *object;
                            let combo = egui::ComboBox::from_id_salt("molecule")
                                .selected_text(text)
                                .width(300.)
                                .show_ui(ui, |ui| {
                                    for item in &objects {
                                        record(
                                            &mut self.rects,
                                            &format!("{label} molecule {}", item.name),
                                            ui.selectable_value(object, Some(item.uid), &item.name),
                                        );
                                    }
                                });
                            record(
                                &mut self.rects,
                                &format!("{label} molecule"),
                                combo.response,
                            );
                            if *object != before {
                                self.choosing = None;
                            }
                            let selection = if moving {
                                &mut self.moving_selection
                            } else {
                                &mut self.reference_selection
                            };
                            ui.horizontal(|ui| {
                                ui.label("Selection");
                                record(
                                    &mut self.rects,
                                    &format!("{label} selection"),
                                    ui.add(
                                        egui::TextEdit::singleline(selection)
                                            .desired_width((ui.available_width() - 75.).max(120.)),
                                    ),
                                );
                                let choose = ui.button("Choose…");
                                if choose.clicked() {
                                    self.choosing = if self.choosing == Some(moving) {
                                        None
                                    } else {
                                        Some(moving)
                                    };
                                    self.builder.opened();
                                }
                                record(
                                    &mut self.rects,
                                    &format!("{label} choose selection"),
                                    choose,
                                );
                            });
                            if self.choosing == Some(moving) {
                                if let Some(applied) =
                                    self.builder.show(ui, state, *object, revision, selection)
                                {
                                    *selection = applied.expression;
                                    self.choosing = None;
                                }
                                self.rects.extend(self.builder.rects.clone());
                            }
                        });
                    });
                }
                record(
                    &mut self.rects,
                    "Auto recenter",
                    ui.checkbox(&mut self.recenter, "Auto recenter"),
                );
                record(
                    &mut self.rects,
                    "Use property",
                    ui.checkbox(&mut self.use_property, "Use xformMat property"),
                );
                if rmsd_available {
                    ui.add_enabled_ui(self.algorithm == "LSQ", |ui| {
                        record(
                            &mut self.rects,
                            "Write RMSD",
                            ui.checkbox(&mut self.write_rmsd, "Write RMSD info file"),
                        );
                    });
                }
            });
        ui.separator();
        let invalid = self.validation_error(state);
        if let Some(error) =
            invalid.or_else(|| (!self.error.is_empty()).then_some(self.error.as_str()))
        {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.horizontal(|ui| {
            let accept = ui.add_enabled(invalid.is_none(), egui::Button::new("OK"));
            if accept.clicked() {
                action = Some(Action::Accept);
            }
            record(&mut self.rects, "OK", accept);
            let cancel = ui.button("Cancel");
            if cancel.clicked() {
                action = Some(Action::Cancel);
            }
            record(&mut self.rects, "Cancel", cancel);
        });
        action
    }
}

fn record(rects: &mut BTreeMap<String, [f32; 4]>, label: &str, response: egui::Response) {
    let r = response.rect;
    rects.insert(
        format!("Superpose {label}"),
        [r.min.x, r.min.y, r.width(), r.height()],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_history_restores_valid_choices_and_resets_selections() {
        let mut state = DesktopAppState::new();
        state.open_bytes("one.pdb", b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let mut form = SuperpositionDialog::default();
        form.opened(&state);
        assert_eq!(form.reference, form.moving);
        assert!(form.validation_error(&state).is_none());
        assert_eq!(form.algorithm, "USalign");
        assert_eq!(form.pivot, "auto");
        assert!(form.recenter && !form.use_property);
        form.algorithm = "LSQ".into();
        form.pivot = "P".into();
        form.recenter = false;
        form.use_property = true;
        form.reference_selection = "name CA".into();
        form.moving_selection = "chain A".into();
        form.write_rmsd = true;
        form.remember(&mut state);
        let saved = state.config_value(HISTORY_KEY);
        form.opened(&state);
        assert_eq!(form.algorithm, "LSQ");
        assert_eq!(form.pivot, "P");
        assert!(!form.recenter && form.use_property);
        assert!(
            form.reference_selection.is_empty()
                && form.moving_selection.is_empty()
                && !form.write_rmsd
        );
        form.algorithm = "SSM".into();
        form.opened(&state);
        assert_eq!(form.algorithm, "LSQ");
        assert_eq!(state.config_value(HISTORY_KEY), saved);
        state.put_config_value(HISTORY_KEY, r#"{"algo":"bad","pivot":"bad","refObjId":-1,"movObjId":4,"autoRecenter":"false","useprop":5}"#);
        form.opened(&state);
        assert_eq!(form.algorithm, "USalign");
        assert_eq!(form.pivot, "auto");
        assert!(form.recenter && !form.use_property);
        assert_eq!(form.reference, form.moving);
        assert!(form.validation_error(&state).is_none());
        form.scene = None;
        assert!(form
            .validation_error(&state)
            .unwrap()
            .contains("active scene changed"));
    }
}
