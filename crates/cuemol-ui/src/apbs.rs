use cuemol_core::Uid;
use cuemol_desktop_core::{DesktopAppState, GuiApbsChargeMethod, GuiApbsOptions};
use std::collections::BTreeMap;

pub const TITLE: &str = "APBS electrostatic potential";
pub const FORCE_FIELDS: [&str; 5] = ["charmm", "amber", "tyl06", "peoepb", "swanson"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start,
    Stop,
    Close,
}

#[derive(Debug)]
pub struct ApbsDialog {
    pub scene: Option<Uid>,
    pub object: Option<Uid>,
    pub use_selection: bool,
    pub selection: String,
    pub name: String,
    pub force_field: String,
    pub options: GuiApbsOptions,
    pub error: String,
    pub status: String,
    pub log: String,
    pub generation: u64,
    pub rects: BTreeMap<String, [f32; 4]>,
    builder: crate::selection::Builder,
    choosing: bool,
}

impl Default for ApbsDialog {
    fn default() -> Self {
        Self {
            scene: None,
            object: None,
            use_selection: false,
            selection: String::new(),
            name: String::new(),
            force_field: "charmm".into(),
            options: GuiApbsOptions::default(),
            error: String::new(),
            status: String::new(),
            log: String::new(),
            generation: 0,
            rects: BTreeMap::new(),
            builder: crate::selection::Builder::for_molecule(),
            choosing: false,
        }
    }
}

impl ApbsDialog {
    pub fn opened(&mut self, state: &DesktopAppState, force_field: &str) {
        let objects = state.active_object_summaries();
        *self = Self {
            scene: state.active_scene(),
            object: self
                .object
                .filter(|id| objects.iter().any(|o| o.uid == *id))
                .or_else(|| objects.first().map(|o| o.uid)),
            force_field: FORCE_FIELDS
                .iter()
                .find(|f| **f == force_field)
                .unwrap_or(&"charmm")
                .to_string(),
            generation: self.generation.wrapping_add(1),
            ..Self::default()
        };
        self.suggest_name(state);
    }

    fn suggest_name(&mut self, state: &DesktopAppState) {
        self.name = self
            .object
            .and_then(|id| state.suggested_apbs_potential_name(id).ok())
            .unwrap_or_default();
    }

    pub fn validation_error(&self, state: &DesktopAppState) -> Option<&'static str> {
        if self.scene != state.active_scene() {
            return Some("The active scene changed. Reopen APBS electrostatic potential.");
        }
        if !state
            .active_object_summaries()
            .iter()
            .any(|o| Some(o.uid) == self.object)
        {
            return Some("Select a molecule.");
        }
        let o = &self.options;
        for (value, max, error) in [
            (
                o.temperature,
                1000.,
                "Temperature must be between 0.1 and 1000 K.",
            ),
            (
                o.grid_spacing,
                10.,
                "Grid size must be between 0.1 and 10 Å.",
            ),
            (
                o.solvent_dielectric,
                200.,
                "Water dielectric must be between 0.1 and 200.",
            ),
            (
                o.solute_dielectric,
                200.,
                "Protein dielectric must be between 0.1 and 200.",
            ),
        ] {
            if !value.is_finite() || !(0.1..=max).contains(&value) {
                return Some(error);
            }
        }
        None
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        revision: u64,
        running: bool,
        stopping: bool,
        paths: Option<(&str, &str)>,
    ) -> Option<Action> {
        self.rects.clear();
        let mut action = None;
        egui::ScrollArea::vertical()
            .id_salt("apbs-fields")
            .max_height((ui.ctx().content_rect().height() - 230.).max(120.))
            .show(ui, |ui| {
                ui.add_enabled_ui(!running, |ui| {
                    let objects = state.active_object_summaries();
                    let old = self.object;
                    let combo = egui::ComboBox::from_label("Target molecule")
                        .selected_text(
                            objects
                                .iter()
                                .find(|o| Some(o.uid) == self.object)
                                .map_or("Select molecule", |o| o.name.as_str()),
                        )
                        .show_ui(ui, |ui| {
                            for o in &objects {
                                record(
                                    &mut self.rects,
                                    &format!("Target {}", o.name),
                                    ui.selectable_value(&mut self.object, Some(o.uid), &o.name),
                                );
                            }
                        });
                    record(&mut self.rects, "Target molecule", combo.response);
                    if self.object != old {
                        self.suggest_name(state);
                        self.choosing = false;
                        self.error.clear();
                    }
                    record(
                        &mut self.rects,
                        "Use selection",
                        ui.checkbox(&mut self.use_selection, "Selection"),
                    );
                    ui.add_enabled_ui(self.use_selection, |ui| {
                        ui.horizontal(|ui| {
                            record(
                                &mut self.rects,
                                "Selection",
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.selection)
                                        .desired_width(300.),
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
                    });
                    ui.horizontal(|ui| {
                        ui.label("Potential name");
                        record(
                            &mut self.rects,
                            "Potential name",
                            ui.text_edit_singleline(&mut self.name),
                        );
                    });
                    ui.separator();
                    ui.strong("Charge calculation method");
                    let combo = egui::ComboBox::from_label("Method")
                        .selected_text(
                            if self.options.charge_method == GuiApbsChargeMethod::Pdb2pqr {
                                "Use PDB2PQR"
                            } else {
                                "Use internal method"
                            },
                        )
                        .show_ui(ui, |ui| {
                            for (value, label) in [
                                (GuiApbsChargeMethod::Pdb2pqr, "Use PDB2PQR"),
                                (GuiApbsChargeMethod::Internal, "Use internal method"),
                            ] {
                                record(
                                    &mut self.rects,
                                    label,
                                    ui.selectable_value(
                                        &mut self.options.charge_method,
                                        value,
                                        label,
                                    ),
                                );
                            }
                        });
                    record(&mut self.rects, "Method", combo.response);
                    if self.options.charge_method == GuiApbsChargeMethod::Pdb2pqr {
                        let combo = egui::ComboBox::from_label("Force field")
                            .selected_text(&self.force_field)
                            .show_ui(ui, |ui| {
                                for field in FORCE_FIELDS {
                                    record(
                                        &mut self.rects,
                                        &format!("Force field {field}"),
                                        ui.selectable_value(
                                            &mut self.force_field,
                                            field.into(),
                                            field,
                                        ),
                                    );
                                }
                            });
                        record(&mut self.rects, "Force field", combo.response);
                    } else {
                        record(
                            &mut self.rects,
                            "Use hydrogen atoms",
                            ui.checkbox(&mut self.options.use_hydrogen, "Use hydrogen atoms"),
                        );
                    }
                    ui.separator();
                    ui.strong("APBS options");
                    record(
                        &mut self.rects,
                        "Solve non-linear PBE",
                        ui.checkbox(&mut self.options.use_npbe, "Solve non-linear PBE"),
                    );
                    for (label, value, max, step) in [
                        (
                            "Temperature (K)",
                            &mut self.options.temperature,
                            1000.,
                            0.01,
                        ),
                        (
                            "Max grid size (Å)",
                            &mut self.options.grid_spacing,
                            10.,
                            0.1,
                        ),
                        (
                            "Water dielectric",
                            &mut self.options.solvent_dielectric,
                            200.,
                            0.01,
                        ),
                        (
                            "Protein dielectric",
                            &mut self.options.solute_dielectric,
                            200.,
                            0.1,
                        ),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(label);
                            record(
                                &mut self.rects,
                                label,
                                ui.add(egui::DragValue::new(value).range(0.1..=max).speed(step)),
                            );
                        });
                    }
                });
                if !self.log.is_empty() {
                    ui.collapsing("Calculation log", |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("apbs-log")
                            .max_height(140.)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.monospace(&self.log);
                            });
                    });
                }
            });
        let missing = match paths {
            None => Some(
                "APBS runs in the native application. Import the resulting DX map in the browser.",
            ),
            Some((apbs, _)) if apbs.trim().is_empty() => {
                Some("Set the APBS executable in Settings > Tools.")
            }
            Some((_, pqr))
                if self.options.charge_method == GuiApbsChargeMethod::Pdb2pqr
                    && pqr.trim().is_empty() =>
            {
                Some("Set the PDB2PQR executable in Settings > Tools.")
            }
            _ => None,
        };
        let invalid = self.validation_error(state);
        if let Some(message) = missing.or(invalid) {
            ui.label(message);
        }
        if !self.error.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, &self.error);
        }
        if !self.status.is_empty() {
            ui.horizontal(|ui| {
                if running {
                    ui.spinner();
                }
                ui.label(&self.status);
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            let start = ui.add_enabled(
                if running {
                    !stopping
                } else {
                    missing.is_none() && invalid.is_none()
                },
                egui::Button::new(if running { "Stop" } else { "Start" }),
            );
            if start.clicked() {
                action = Some(if running { Action::Stop } else { Action::Start });
            }
            record(
                &mut self.rects,
                if running { "Stop" } else { "Start" },
                start,
            );
            let close = ui.add_enabled(!running, egui::Button::new("Close"));
            if close.clicked() {
                action = Some(Action::Close);
            }
            record(&mut self.rects, "Close", close);
        });
        action
    }
}

fn record(rects: &mut BTreeMap<String, [f32; 4]>, name: &str, response: egui::Response) {
    let r = response.rect;
    rects.insert(
        format!("APBS {name}"),
        [r.min.x, r.min.y, r.width(), r.height()],
    );
}
