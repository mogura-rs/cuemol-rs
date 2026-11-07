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
pub struct SecondaryDialog {
    pub scene: Option<Uid>,
    pub object: Option<Uid>,
    pub assign: bool,
    pub ignore_bulge: bool,
    pub gap_fill: bool,
    pub angle: f64,
    pub selection: String,
    pub kind: usize,
    pub error: String,
    pub rects: BTreeMap<String, [f32; 4]>,
    builder: crate::selection::Builder,
    choosing: bool,
}

const TYPES: [(&str, &str); 5] = [
    ("coil", "Coil"),
    ("strand", "β strand"),
    ("helix", "α helix"),
    ("helix310", "3-10 helix"),
    ("helixpi", "π helix"),
];

impl Default for SecondaryDialog {
    fn default() -> Self {
        Self {
            scene: None,
            object: None,
            assign: false,
            ignore_bulge: false,
            gap_fill: false,
            angle: 120.,
            selection: String::new(),
            kind: 0,
            error: String::new(),
            rects: BTreeMap::new(),
            builder: crate::selection::Builder::for_molecule(),
            choosing: false,
        }
    }
}

impl SecondaryDialog {
    pub fn opened(&mut self, state: &DesktopAppState, selected: Option<Uid>) {
        let objects = state.active_object_summaries();
        let valid = |id| objects.iter().any(|o| Some(o.uid) == id);
        *self = Self {
            scene: state.active_scene(),
            object: self
                .object
                .filter(|_| valid(self.object))
                .or(selected.filter(|_| valid(selected)))
                .or_else(|| objects.first().map(|o| o.uid)),
            ..Self::default()
        };
    }

    pub fn validation_error(&self, state: &DesktopAppState) -> Option<&'static str> {
        if self.scene != state.active_scene() {
            return Some("The active scene changed. Reopen Reassign secondary structure.");
        }
        if !state
            .active_object_summaries()
            .iter()
            .any(|o| Some(o.uid) == self.object)
        {
            return Some("Select a molecule.");
        }
        if self.assign && self.kind >= TYPES.len() {
            return Some("Select a secondary structure type.");
        }
        if !self.assign
            && self.gap_fill
            && (!self.angle.is_finite() || !(0.0..=180.0).contains(&self.angle))
        {
            return Some("Helix angle must be between 0 and 180 degrees.");
        }
        None
    }

    pub fn arguments(&self) -> Value {
        json!({
            "objId": self.object.map(Uid::get),
            "selection": self.assign.then_some(&self.selection),
            "type": self.assign.then(|| TYPES.get(self.kind).map(|t| t.0)).flatten(),
            "ignoreBulge": self.ignore_bulge,
            "helixGapAngle": if self.gap_fill { self.angle } else { 0. },
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
        egui::ScrollArea::vertical()
            .id_salt("secondary-fields")
            .max_height((ui.ctx().content_rect().height() - 200.).max(120.))
            .show(ui, |ui| {
                ui.strong("Molecule");
                let objects = state.active_object_summaries();
                let text = objects
                    .iter()
                    .find(|o| Some(o.uid) == self.object)
                    .map_or("Select molecule", |o| o.name.as_str());
                let combo = egui::ComboBox::from_id_salt("secondary-molecule")
                    .selected_text(text)
                    .width(300.)
                    .show_ui(ui, |ui| {
                        for object in &objects {
                            record(
                                &mut self.rects,
                                &format!("Molecule {}", object.name),
                                ui.selectable_value(
                                    &mut self.object,
                                    Some(object.uid),
                                    &object.name,
                                ),
                            );
                        }
                    });
                record(&mut self.rects, "Molecule", combo.response);
                ui.horizontal(|ui| {
                    record(
                        &mut self.rects,
                        "Recalculate",
                        ui.radio_value(&mut self.assign, false, "Recalculate"),
                    );
                    record(
                        &mut self.rects,
                        "Assign",
                        ui.radio_value(&mut self.assign, true, "Assign"),
                    );
                });
                ui.group(|ui| {
                    ui.strong("Recalculate");
                    ui.add_enabled_ui(!self.assign, |ui| {
                        record(
                            &mut self.rects,
                            "Ignore beta bulge",
                            ui.checkbox(&mut self.ignore_bulge, "Ignore β bulge"),
                        );
                        record(
                            &mut self.rects,
                            "Gap fill",
                            ui.checkbox(&mut self.gap_fill, "Helix gap-fill angle (°)"),
                        );
                        ui.horizontal(|ui| {
                            ui.label("Angle");
                            record(
                                &mut self.rects,
                                "Angle",
                                ui.add_enabled(
                                    self.gap_fill,
                                    egui::DragValue::new(&mut self.angle)
                                        .range(0.0..=180.0)
                                        .speed(1.),
                                ),
                            );
                        });
                    });
                });
                ui.group(|ui| {
                    ui.strong("Assign");
                    ui.add_enabled_ui(self.assign, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Selection");
                            record(
                                &mut self.rects,
                                "Selection",
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.selection)
                                        .desired_width((ui.available_width() - 75.).max(120.)),
                                ),
                            );
                            let choose = ui.button("Choose…");
                            if choose.clicked() {
                                self.choosing = !self.choosing;
                                self.builder.opened();
                            }
                            record(&mut self.rects, "Choose selection", choose);
                        });
                        if self.choosing {
                            if let Some(applied) =
                                self.builder
                                    .show(ui, state, self.object, revision, &self.selection)
                            {
                                self.selection = applied.expression;
                                self.choosing = false;
                            }
                            self.rects.extend(self.builder.rects.clone());
                        }
                        let combo = egui::ComboBox::from_label("Type")
                            .selected_text(TYPES.get(self.kind).map_or("Select type", |t| t.1))
                            .show_ui(ui, |ui| {
                                for (i, (_, name)) in TYPES.iter().enumerate() {
                                    record(
                                        &mut self.rects,
                                        &format!("Type {name}"),
                                        ui.selectable_value(&mut self.kind, i, *name),
                                    );
                                }
                            });
                        record(&mut self.rects, "Type", combo.response);
                    });
                });
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
        format!("Secondary {label}"),
        [r.min.x, r.min.y, r.width(), r.height()],
    );
}
