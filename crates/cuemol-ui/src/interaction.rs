use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Accept,
    Cancel,
}

#[derive(Debug)]
pub struct InteractionDialog {
    pub scene: Option<Uid>,
    pub object: Option<Uid>,
    pub object2: Option<Uid>,
    pub selection1: String,
    pub selection2: String,
    pub use_object2: bool,
    pub use_selection2: bool,
    pub r_min: f64,
    pub r_max: f64,
    pub max_count: usize,
    pub hbond: bool,
    pub renderer_name: String,
    pub error: String,
    pub rects: BTreeMap<String, [f32; 4]>,
    builders: [crate::selection::Builder; 2],
    editing_selection: Option<usize>,
}

impl Default for InteractionDialog {
    fn default() -> Self {
        Self {
            scene: None,
            object: None,
            object2: None,
            selection1: String::new(),
            selection2: String::new(),
            use_object2: false,
            use_selection2: false,
            r_min: 2.5,
            r_max: 3.5,
            max_count: 30,
            hbond: false,
            renderer_name: "measure".into(),
            error: String::new(),
            rects: BTreeMap::new(),
            builders: std::array::from_fn(|_| crate::selection::Builder::for_molecule()),
            editing_selection: None,
        }
    }
}

impl InteractionDialog {
    pub fn opened(&mut self, state: &DesktopAppState, selected: Option<Uid>) {
        *self = Self {
            scene: state.active_scene(),
            object: self.object,
            object2: self.object2,
            ..Self::default()
        };
        let objects = state.active_object_summaries();
        if !objects.iter().any(|o| Some(o.uid) == self.object) {
            self.object = selected
                .filter(|id| objects.iter().any(|o| o.uid == *id))
                .or_else(|| objects.first().map(|o| o.uid));
        }
        if !objects.iter().any(|o| Some(o.uid) == self.object2) {
            self.object2 = objects
                .iter()
                .find(|o| Some(o.uid) != self.object)
                .or_else(|| objects.first())
                .map(|o| o.uid);
        }
    }

    pub fn second_object(&self) -> Option<Uid> {
        self.object2
            .filter(|id| self.use_object2 && Some(*id) != self.object)
    }

    pub fn second_selection(&self) -> Option<&str> {
        (self.second_object().is_some() || self.use_selection2).then_some(self.selection2.as_str())
    }

    pub fn validation_error(&self, state: &DesktopAppState) -> Option<&'static str> {
        if self.scene != state.active_scene() {
            return Some("The active scene changed. Reopen Interaction analysis.");
        }
        let objects = state.active_object_summaries();
        if !objects.iter().any(|o| Some(o.uid) == self.object) {
            return Some("Select a molecule.");
        }
        if self.use_object2 && !objects.iter().any(|o| Some(o.uid) == self.object2) {
            return Some("Select a second molecule.");
        }
        if !self.r_min.is_finite() || !self.r_max.is_finite() || self.r_min < 0.0 {
            return Some("Enter a valid non-negative distance range.");
        }
        if self.r_min >= self.r_max {
            return Some("Min distance must be smaller than max distance.");
        }
        if self.max_count == 0 {
            return Some("Max labels must be greater than zero.");
        }
        None
    }

    pub fn arguments(&self) -> Value {
        json!({
            "objId": self.object.map(Uid::get),
            "selection": self.selection1,
            "obj2Id": self.second_object().map(Uid::get),
            "selection2": self.second_selection(),
            "minDist": self.r_min,
            "maxDist": self.r_max,
            "maxLabels": self.max_count,
            "includeCarbon": !self.hbond,
            "labelSet": if self.renderer_name.trim().is_empty() {"measure"} else {self.renderer_name.trim()},
        })
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        revision: u64,
    ) -> Option<Action> {
        self.rects.clear();
        let mut action = None;
        let objects = state.active_object_summaries();
        egui::ScrollArea::vertical()
            .id_salt("interaction-fields")
            .max_height((ui.ctx().content_rect().height() - 200.).max(120.))
            .show(ui, |ui| {
                ui.strong("Molecule");
                molecule(ui, &objects, &mut self.object, "Molecule", &mut self.rects);
                self.selection_field(ui, state, revision, 0);
                record(
                    &mut self.rects,
                    "Interaction second molecule",
                    ui.checkbox(&mut self.use_object2, "Second molecule"),
                );
                if self.use_object2 {
                    molecule(
                        ui,
                        &objects,
                        &mut self.object2,
                        "Molecule 2",
                        &mut self.rects,
                    );
                }
                record(
                    &mut self.rects,
                    "Interaction second selection",
                    ui.checkbox(&mut self.use_selection2, "Second selection"),
                );
                if self.use_object2 || self.use_selection2 {
                    self.selection_field(ui, state, revision, 1);
                }
                ui.separator();
                ui.strong("Parameters");
                egui::Grid::new("interaction-parameters")
                    .spacing([12., 8.])
                    .show(ui, |ui| {
                        for (label, value) in [
                            ("Min distance (Å)", &mut self.r_min),
                            ("Max distance (Å)", &mut self.r_max),
                        ] {
                            ui.label(label);
                            record(
                                &mut self.rects,
                                &format!("Interaction {label}"),
                                ui.add(egui::DragValue::new(value).speed(0.1).range(0.0..=20.0)),
                            );
                            ui.end_row();
                        }
                        ui.label("Max labels");
                        record(
                            &mut self.rects,
                            "Interaction Max labels",
                            ui.add(egui::DragValue::new(&mut self.max_count).range(1..=1000)),
                        );
                        ui.end_row();
                        ui.label("Hydrogen bond (N, O) only");
                        record(
                            &mut self.rects,
                            "Interaction hydrogen bonds",
                            ui.checkbox(&mut self.hbond, ""),
                        );
                        ui.end_row();
                        ui.label("Label set name");
                        record(
                            &mut self.rects,
                            "Interaction label set",
                            ui.add_sized(
                                [250., 22.],
                                egui::TextEdit::singleline(&mut self.renderer_name),
                            ),
                        );
                        ui.end_row();
                    });
            });
        ui.separator();
        let invalid = self.validation_error(state);
        if let Some(error) = invalid {
            ui.colored_label(ui.visuals().error_fg_color, error);
        } else if !self.error.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, &self.error);
        }
        ui.horizontal(|ui| {
            let accept = ui.add_enabled(invalid.is_none(), egui::Button::new("OK"));
            if accept.clicked() {
                action = Some(Action::Accept);
            }
            record(&mut self.rects, "Interaction OK", accept);
            let cancel = ui.button("Cancel");
            if cancel.clicked() {
                action = Some(Action::Cancel);
            }
            record(&mut self.rects, "Interaction Cancel", cancel);
        });
        action
    }

    fn selection_field(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        revision: u64,
        index: usize,
    ) {
        ui.push_id(index, |ui| {
            let value = if index == 0 {
                &mut self.selection1
            } else {
                &mut self.selection2
            };
            ui.horizontal(|ui| {
                ui.label(format!("Selection {}", index + 1));
                let response = ui.add(
                    egui::TextEdit::singleline(value)
                        .desired_width((ui.available_width() - 75.).max(120.)),
                );
                record(
                    &mut self.rects,
                    &format!("Interaction selection {}", index + 1),
                    response,
                );
                let button = ui.button("Choose…");
                if button.clicked() {
                    self.editing_selection = if self.editing_selection == Some(index) {
                        None
                    } else {
                        Some(index)
                    };
                    self.builders[index].opened();
                }
                record(
                    &mut self.rects,
                    &format!("Interaction choose selection {}", index + 1),
                    button,
                );
            });
            if self.editing_selection == Some(index) {
                let molecule = if index == 1 && self.use_object2 {
                    self.object2
                } else {
                    self.object
                };
                if let Some(applied) =
                    self.builders[index].show(ui, state, molecule, revision, value)
                {
                    *value = applied.expression;
                    self.editing_selection = None;
                }
                self.rects.extend(self.builders[index].rects.clone());
            }
        });
    }
}

fn molecule(
    ui: &mut egui::Ui,
    objects: &[cuemol_desktop_core::GuiObjectSummary],
    selected: &mut Option<Uid>,
    label: &str,
    rects: &mut BTreeMap<String, [f32; 4]>,
) {
    let text = objects
        .iter()
        .find(|o| Some(o.uid) == *selected)
        .map_or("Select molecule", |o| o.name.as_str());
    let combo = egui::ComboBox::from_id_salt(label)
        .selected_text(text)
        .width(300.)
        .show_ui(ui, |ui| {
            for object in objects {
                let response = ui.selectable_value(selected, Some(object.uid), &object.name);
                record(
                    rects,
                    &format!("Interaction {label} {}", object.name),
                    response,
                );
            }
        });
    record(rects, &format!("Interaction {label}"), combo.response);
}

fn record(rects: &mut BTreeMap<String, [f32; 4]>, label: &str, response: egui::Response) {
    let r = response.rect;
    rects.insert(label.into(), [r.min.x, r.min.y, r.width(), r.height()]);
}
