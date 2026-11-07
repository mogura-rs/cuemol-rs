use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Generate,
    Regenerate,
    Cut,
}

impl Kind {
    pub fn from_command(command: &str) -> Option<Self> {
        match command {
            "create_surface" => Some(Self::Generate),
            "recalc_surface" => Some(Self::Regenerate),
            "cut_surface" => Some(Self::Cut),
            _ => None,
        }
    }

    pub fn command(self) -> &'static str {
        match self {
            Self::Generate => "create_surface",
            Self::Regenerate => "recalc_surface",
            Self::Cut => "cut_surface",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Generate => "Mol surface generation",
            Self::Regenerate => "Regenerate surface",
            Self::Cut => "MolSurf cutting tool",
        }
    }
}

#[derive(Clone, Copy)]
pub enum Action {
    Accept,
    Cancel,
}

const BACKENDS: [(&str, &str); 4] = [
    ("auto", "Auto"),
    ("meshms", "MeshMS"),
    ("ball", "BALL"),
    ("rust", "Rust"),
];
const MODES: [(&str, &str); 4] = [
    ("full", "Complete"),
    ("separate", "Separately"),
    ("sect", "Section only"),
    ("body", "No section"),
];

#[derive(Debug)]
pub struct SurfaceDialog {
    pub active: Option<Kind>,
    pub scene: Option<Uid>,
    pub molecule: Option<Uid>,
    pub surface: Option<Uid>,
    pub name: String,
    pub selection: String,
    pub use_selection: bool,
    pub density: f64,
    pub probe_radius: f64,
    pub backend: String,
    pub mode: String,
    pub error: String,
    pub rects: BTreeMap<String, [f32; 4]>,
    generate_backend: String,
    regenerate_backend: String,
    cut_target: Option<Uid>,
    builder: crate::selection::Builder,
    choosing: bool,
}

impl Default for SurfaceDialog {
    fn default() -> Self {
        Self {
            active: None,
            scene: None,
            molecule: None,
            surface: None,
            name: String::new(),
            selection: String::new(),
            use_selection: false,
            density: 2.,
            probe_radius: 1.4,
            backend: "auto".into(),
            mode: "full".into(),
            error: String::new(),
            rects: BTreeMap::new(),
            generate_backend: "auto".into(),
            regenerate_backend: "auto".into(),
            cut_target: None,
            builder: crate::selection::Builder::for_molecule(),
            choosing: false,
        }
    }
}

pub fn clamp_density(value: f64) -> f64 {
    if value.is_finite() {
        value.round().clamp(1., 10.)
    } else {
        2.
    }
}

impl SurfaceDialog {
    pub fn close(&mut self) {
        match self.active {
            Some(Kind::Generate) => self.generate_backend.clone_from(&self.backend),
            Some(Kind::Regenerate) => self.regenerate_backend.clone_from(&self.backend),
            Some(Kind::Cut) => self.cut_target = self.surface,
            None => {}
        }
        self.active = None;
    }

    pub fn open(&mut self, kind: Kind, state: &DesktopAppState, selected: Option<Uid>) {
        self.close();
        self.active = Some(kind);
        self.scene = state.active_scene();
        self.error.clear();
        self.rects.clear();
        self.choosing = false;
        self.use_selection = false;
        self.selection.clear();
        self.density = 2.;
        self.probe_radius = 1.4;
        self.mode = "full".into();
        match kind {
            Kind::Generate => {
                let objects = state.active_object_summaries();
                let valid = |id| objects.iter().any(|o| Some(o.uid) == id);
                self.molecule = self
                    .molecule
                    .filter(|_| valid(self.molecule))
                    .or(selected.filter(|_| valid(selected)))
                    .or_else(|| objects.first().map(|o| o.uid));
                self.backend.clone_from(&self.generate_backend);
                self.suggest_name(state);
            }
            Kind::Regenerate => {
                self.surface = selected;
                self.backend.clone_from(&self.regenerate_backend);
                if let Some(surface) = selected.and_then(|id| state.surface_summary(id)) {
                    self.name = surface.name;
                    self.selection = surface.original_selection;
                    self.density = clamp_density(surface.original_density);
                    self.probe_radius = surface.original_probe_radius;
                }
            }
            Kind::Cut => {
                let surfaces = state.active_surface_summaries();
                let valid = |id| surfaces.iter().any(|o| Some(o.uid) == id);
                self.surface = self
                    .cut_target
                    .filter(|_| valid(self.cut_target))
                    .or(selected.filter(|_| valid(selected)))
                    .or_else(|| surfaces.first().map(|o| o.uid));
                self.density = 5.;
            }
        }
    }

    pub fn suggest_name(&mut self, state: &DesktopAppState) {
        let Some(object) = state
            .active_object_summaries()
            .into_iter()
            .find(|o| Some(o.uid) == self.molecule)
        else {
            self.name.clear();
            return;
        };
        self.name = state
            .suggest_object_name(self.scene.unwrap(), &format!("sf_{}", object.name))
            .unwrap_or_default();
    }

    pub fn validation_error(&self, state: &DesktopAppState) -> Option<&'static str> {
        if self.scene != state.active_scene() || self.scene.is_none() {
            return Some("The active scene changed. Reopen the surface dialog.");
        }
        match self.active? {
            Kind::Generate => {
                if !state
                    .active_object_summaries()
                    .iter()
                    .any(|o| Some(o.uid) == self.molecule)
                {
                    return Some("Select a molecule.");
                }
                if !self.probe_radius.is_finite() || !(0.1..=10.).contains(&self.probe_radius) {
                    return Some("Probe radius must be between 0.1 and 10 Å.");
                }
            }
            Kind::Regenerate | Kind::Cut => {
                let Some(surface) = state
                    .active_surface_summaries()
                    .into_iter()
                    .find(|s| Some(s.uid) == self.surface)
                else {
                    return Some("Select a molecular surface.");
                };
                if self.active == Some(Kind::Regenerate)
                    && !state
                        .active_object_summaries()
                        .iter()
                        .any(|o| o.name == surface.original_molecule)
                {
                    return Some("The original molecule is missing.");
                }
            }
        }
        let range = if self.active == Some(Kind::Cut) {
            0.1..=50.
        } else {
            1. ..=10.
        };
        if !self.density.is_finite() || !range.contains(&self.density) {
            return Some("Surface density is outside the allowed range.");
        }
        if self.active == Some(Kind::Cut) {
            if !MODES.iter().any(|m| m.0 == self.mode) {
                return Some("Select a cross section type.");
            }
        } else if !BACKENDS.iter().any(|b| b.0 == self.backend) {
            return Some("Select a surface algorithm.");
        }
        None
    }

    pub fn arguments(&self, state: &DesktopAppState) -> Value {
        match self.active {
            Some(Kind::Generate) => {
                let mut name = self.name.trim().to_string();
                if name.is_empty() {
                    if let Some(object) = self
                        .molecule
                        .and_then(|id| state.scene_manager().object(id))
                    {
                        name = state
                            .suggest_object_name(
                                self.scene.unwrap(),
                                &format!("sf_{}", object.name()),
                            )
                            .unwrap_or_default();
                    }
                }
                json!({"objId":self.molecule.map(Uid::get),"selection":if self.use_selection { &self.selection } else { "" },"name":name,"density":clamp_density(self.density),"probeRadius":self.probe_radius,"backend":self.backend})
            }
            Some(Kind::Regenerate) => {
                json!({"surfId":self.surface.map(Uid::get),"density":clamp_density(self.density),"backend":(self.backend != "auto").then_some(&self.backend)})
            }
            Some(Kind::Cut) => {
                json!({"surfId":self.surface.map(Uid::get),"density":self.density,"mode":self.mode})
            }
            None => Value::Null,
        }
    }

    pub fn remember(&self, state: &mut DesktopAppState) {
        if self.active == Some(Kind::Generate) && self.use_selection {
            state.remember_selection(&self.selection);
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        revision: u64,
    ) -> Option<Action> {
        let kind = self.active?;
        self.rects.clear();
        let mut action = None;
        ui.strong("Target");
        match kind {
            Kind::Generate => {
                let objects = state.active_object_summaries();
                let before = self.molecule;
                let name = objects
                    .iter()
                    .find(|o| Some(o.uid) == self.molecule)
                    .map_or("Select molecule", |o| o.name.as_str());
                ui.horizontal(|ui| {
                    ui.label("Molecule");
                    let combo = egui::ComboBox::from_id_salt("surface-molecule")
                        .selected_text(name)
                        .width(280.)
                        .show_ui(ui, |ui| {
                            for object in &objects {
                                record(
                                    &mut self.rects,
                                    &format!("Molecule {}", object.name),
                                    ui.selectable_value(
                                        &mut self.molecule,
                                        Some(object.uid),
                                        &object.name,
                                    ),
                                );
                            }
                        });
                    record(&mut self.rects, "Molecule", combo.response);
                });
                if self.molecule != before {
                    self.suggest_name(state);
                    self.choosing = false;
                }
                record(
                    &mut self.rects,
                    "Use selection",
                    ui.checkbox(&mut self.use_selection, "Use selection"),
                );
                ui.add_enabled_ui(self.use_selection, |ui| {
                    ui.horizontal(|ui| {
                        record(
                            &mut self.rects,
                            "Selection",
                            ui.add(
                                egui::TextEdit::singleline(&mut self.selection)
                                    .desired_width((ui.available_width() - 80.).max(100.)),
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
                                .show(ui, state, self.molecule, revision, &self.selection)
                        {
                            self.selection = applied.expression;
                            self.choosing = false;
                        }
                        self.rects.extend(self.builder.rects.clone());
                    }
                });
            }
            Kind::Regenerate => {
                let molecule = self
                    .surface
                    .and_then(|id| state.surface_summary(id))
                    .map_or(String::new(), |s| s.original_molecule);
                for (label, value) in [
                    ("Surface object", self.name.clone()),
                    ("Molecule", molecule),
                    (
                        "Selection",
                        if self.selection.is_empty() {
                            "(all atoms)".into()
                        } else {
                            self.selection.clone()
                        },
                    ),
                    (
                        "Probe radius (Å)",
                        format!("{:.6}", self.probe_radius)
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                            .into(),
                    ),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        let mut text = value;
                        ui.add(
                            egui::TextEdit::singleline(&mut text)
                                .interactive(false)
                                .desired_width(300.),
                        );
                    });
                }
            }
            Kind::Cut => {
                let surfaces = state.active_surface_summaries();
                let name = surfaces
                    .iter()
                    .find(|s| Some(s.uid) == self.surface)
                    .map_or("Select surface", |s| s.name.as_str());
                let combo = egui::ComboBox::from_label("Target surface")
                    .selected_text(name)
                    .show_ui(ui, |ui| {
                        for surface in &surfaces {
                            record(
                                &mut self.rects,
                                &format!("Target {}", surface.name),
                                ui.selectable_value(
                                    &mut self.surface,
                                    Some(surface.uid),
                                    &surface.name,
                                ),
                            );
                        }
                    });
                record(&mut self.rects, "Target surface", combo.response);
            }
        }
        ui.separator();
        ui.strong(if kind == Kind::Cut {
            "Cross section"
        } else {
            "Surface"
        });
        if kind == Kind::Generate {
            ui.horizontal(|ui| {
                ui.label("Object name");
                record(
                    &mut self.rects,
                    "Name",
                    ui.text_edit_singleline(&mut self.name),
                );
            });
        }
        if kind == Kind::Cut {
            let combo = egui::ComboBox::from_label("Cross section type")
                .selected_text(
                    MODES
                        .iter()
                        .find(|m| m.0 == self.mode)
                        .map_or("Select mode", |m| m.1),
                )
                .show_ui(ui, |ui| {
                    for (mode, label) in MODES {
                        record(
                            &mut self.rects,
                            &format!("Mode {mode}"),
                            ui.selectable_value(&mut self.mode, mode.into(), label),
                        );
                    }
                });
            record(&mut self.rects, "Mode", combo.response);
            ui.horizontal(|ui| {
                ui.label("Section mesh density (/Å)");
                record(
                    &mut self.rects,
                    "Density",
                    ui.add(
                        egui::DragValue::new(&mut self.density)
                            .range(0.1..=50.)
                            .speed(0.5),
                    ),
                );
            });
        } else {
            ui.horizontal(|ui| {
                ui.label("Point density (/Å)");
                record(
                    &mut self.rects,
                    "Density slider",
                    ui.add(
                        egui::Slider::new(&mut self.density, 1. ..=10.)
                            .step_by(1.)
                            .show_value(false),
                    ),
                );
                record(
                    &mut self.rects,
                    "Density",
                    ui.add(
                        egui::DragValue::new(&mut self.density)
                            .range(1. ..=10.)
                            .speed(1.)
                            .max_decimals(0),
                    ),
                );
            });
            if kind == Kind::Generate {
                ui.horizontal(|ui| {
                    ui.label("Probe radius (Å)");
                    record(
                        &mut self.rects,
                        "Probe radius slider",
                        ui.add(
                            egui::Slider::new(&mut self.probe_radius, 0.1..=10.)
                                .step_by(0.1)
                                .show_value(false),
                        ),
                    );
                    record(
                        &mut self.rects,
                        "Probe radius",
                        ui.add(
                            egui::DragValue::new(&mut self.probe_radius)
                                .range(0.1..=10.)
                                .speed(0.1)
                                .max_decimals(1),
                        ),
                    );
                });
            }
            let combo = egui::ComboBox::from_label("Algorithm")
                .selected_text(
                    BACKENDS
                        .iter()
                        .find(|b| b.0 == self.backend)
                        .map_or("Auto", |b| b.1),
                )
                .show_ui(ui, |ui| {
                    for (backend, label) in BACKENDS {
                        record(
                            &mut self.rects,
                            &format!("Algorithm {backend}"),
                            ui.selectable_value(&mut self.backend, backend.into(), label),
                        );
                    }
                });
            record(&mut self.rects, "Algorithm", combo.response);
        }
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
        format!("Surface {label}"),
        [r.min.x, r.min.y, r.width(), r.height()],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_dialog_resets_inputs_and_keeps_independent_session_choices() {
        let mut state = DesktopAppState::new();
        state.open_bytes("source.pdb", b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let object = state.active_object_summaries()[0].uid;
        state
            .set_molecule_selection_text(object, "chain A")
            .unwrap();
        let mut form = SurfaceDialog::default();
        form.open(Kind::Generate, &state, Some(object));
        assert_eq!(form.density, 2.);
        assert_eq!(form.probe_radius, 1.4);
        assert!(!form.use_selection);
        assert!(form.selection.is_empty());
        assert_eq!(form.arguments(&state)["selection"], "");
        assert!(form.validation_error(&state).is_none());
        form.backend = "rust".into();
        form.density = 9.;
        form.probe_radius = 3.;
        form.use_selection = true;
        form.selection = "chain A".into();
        form.close();
        form.open(Kind::Generate, &state, None);
        assert_eq!(form.backend, "rust");
        assert_eq!(form.density, 2.);
        assert_eq!(form.probe_radius, 1.4);
        assert!(!form.use_selection);
        assert!(form.selection.is_empty());
        let name = form.name.clone();
        let surface = state
            .create_molecular_surface_object_with_backend(object, "chain A", &name, 2., 1.8, "rust")
            .unwrap();
        form.open(Kind::Generate, &state, None);
        assert_eq!(form.name, format!("{name}(1)"));
        form.name.clear();
        assert_eq!(form.arguments(&state)["name"], format!("{name}(1)"));
        form.open(Kind::Regenerate, &state, Some(surface));
        assert_eq!(form.backend, "auto");
        assert_eq!(form.selection, "chain A");
        assert_eq!(form.probe_radius, 1.8);
        assert!(form.arguments(&state)["backend"].is_null());
        form.backend = "ball".into();
        form.close();
        form.open(Kind::Regenerate, &state, Some(surface));
        assert_eq!(form.backend, "ball");
        form.open(Kind::Generate, &state, None);
        assert_eq!(form.backend, "rust");
        form.open(Kind::Cut, &state, Some(surface));
        assert_eq!(form.density, 5.);
        assert_eq!(form.mode, "full");
        form.density = 51.;
        assert!(form.validation_error(&state).is_some());
        form.mode = "separate".into();
        form.open(Kind::Cut, &state, None);
        assert_eq!(form.surface, Some(surface));
        assert_eq!(form.density, 5.);
        assert_eq!(form.mode, "full");
        form.open(Kind::Regenerate, &state, Some(surface));
        state.destroy_object(object).unwrap();
        assert_eq!(
            form.validation_error(&state),
            Some("The original molecule is missing.")
        );
        form.scene = None;
        assert!(form
            .validation_error(&state)
            .unwrap()
            .contains("active scene changed"));
    }
}
