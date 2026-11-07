use cuemol_core::{Result, Uid};
use cuemol_desktop_core::{
    renderer_history,
    renderer_setup::{Kind, Target},
    DesktopAppState, GuiMapCenterPolicy,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Create,
    Cancel,
}

pub struct RendererDialog {
    pub target: Target,
    pub renderer_type: String,
    pub renderer_name: String,
    pub object_name: String,
    pub selection_enabled: bool,
    pub selection: String,
    pub recenter: bool,
    pub map_center: GuiMapCenterPolicy,
    pub group: Option<String>,
    pub error: String,
    pub rects: BTreeMap<String, [f32; 4]>,
    name_edited: bool,
    choosing: bool,
    builder: crate::selection::Builder,
}

impl RendererDialog {
    pub fn new(state: &DesktopAppState, object: Uid, group: Option<String>) -> Result<Self> {
        let mut target = Target::for_object(state, object)?;
        let group = group.filter(|name| !name.trim().is_empty());
        if group.is_some() {
            target.options.retain(|option| !option.preset);
        }
        let renderer_type = state
            .renderer_history_type(&target.class, &target.options)
            .or_else(|| renderer_history::default_type(&target.options))
            .unwrap_or_default();
        let renderer_name = state.suggest_renderer_name(target.scene, &renderer_type)?;
        let selection = state
            .molecule_selection_text(object)
            .unwrap_or_default()
            .trim()
            .to_string();
        Ok(Self {
            object_name: target.name.clone(),
            target,
            renderer_type,
            renderer_name,
            selection_enabled: !selection.is_empty(),
            selection: if selection.is_empty() {
                "*".into()
            } else {
                selection
            },
            recenter: true,
            map_center: GuiMapCenterPolicy::Auto,
            group,
            error: String::new(),
            rects: BTreeMap::new(),
            name_edited: false,
            choosing: false,
            builder: crate::selection::Builder::for_molecule(),
        })
    }

    pub fn title(&self) -> String {
        self.group.as_ref().map_or_else(
            || "New Renderer".into(),
            |name| format!("New Renderer (group: {name})"),
        )
    }

    pub fn is_preset(&self) -> bool {
        self.target
            .options
            .iter()
            .any(|option| option.name == self.renderer_type && option.preset)
    }

    pub fn validation_error(&self, state: &DesktopAppState) -> Option<&'static str> {
        if state.active_scene() != Some(self.target.scene) {
            return Some("The active scene changed. Reopen New Renderer.");
        }
        let Ok(current) = Target::for_object(state, self.target.object) else {
            return Some("The target object was removed. Reopen New Renderer.");
        };
        if current.scene != self.target.scene {
            return Some("The target object changed scenes. Reopen New Renderer.");
        }
        if self
            .group
            .as_ref()
            .is_some_and(|name| !current.groups.contains(name))
        {
            return Some("The target group was removed or renamed. Reopen New Renderer.");
        }
        if !current.options.iter().any(|o| o.name == self.renderer_type)
            || (self.group.is_some() && self.is_preset())
        {
            return Some("Choose a compatible renderer type.");
        }
        if self.renderer_name.trim().is_empty() || self.renderer_name.contains(':') {
            return Some("Enter a renderer name without a colon.");
        }
        None
    }

    pub fn arguments(&self) -> Value {
        json!({
            "objId": self.target.object.get(),
            "rendererType": self.renderer_type,
            "name": self.renderer_name.trim(),
            "selection": (self.target.kind == Kind::Molecule && self.selection_enabled && !self.is_preset())
                .then(|| if self.selection.trim().is_empty() { "*" } else { self.selection.trim() }),
            "centerView": self.recenter,
            "mapCenterPolicy": match self.map_center {
                GuiMapCenterPolicy::Auto => "auto",
                GuiMapCenterPolicy::SetMapCenter => "setMapCenter",
                GuiMapCenterPolicy::MoveViewCenter => "moveViewCenter",
            },
            "group": self.group,
        })
    }

    pub fn remember(&self, state: &mut DesktopAppState) {
        state.remember_renderer_type(&self.target.class, &self.renderer_type);
        if let Some(selection) = self.arguments()["selection"].as_str() {
            state.remember_selection(selection);
        }
    }

    fn follow_name(&mut self, state: &DesktopAppState) {
        if !self.name_edited {
            if let Ok(name) = state.suggest_renderer_name(self.target.scene, &self.renderer_type) {
                self.renderer_name = name;
            }
        }
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
            .id_salt("new-renderer-fields")
            .max_height((ui.ctx().content_rect().height() - 200.).max(120.))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(&self.target.name);
                    ui.weak(&self.target.class);
                });
                ui.separator();
                egui::Grid::new("new-renderer-fields-grid")
                    .num_columns(2)
                    .spacing([12., 8.])
                    .show(ui, |ui| {
                        ui.label("Object name");
                        record(
                            &mut self.rects,
                            "Object name",
                            ui.text_edit_singleline(&mut self.object_name),
                        );
                        ui.end_row();
                        ui.label("Renderer type");
                        let previous = self.renderer_type.clone();
                        let label = self
                            .target
                            .options
                            .iter()
                            .find(|o| o.name == self.renderer_type)
                            .map_or(self.renderer_type.as_str(), |o| o.description.as_str());
                        let combo = egui::ComboBox::from_id_salt("new-renderer-type")
                            .selected_text(label)
                            .width(300.)
                            .height(320.)
                            .show_ui(ui, |ui| {
                                for preset in [true, false] {
                                    if !self.target.options.iter().any(|o| o.preset == preset) {
                                        continue;
                                    }
                                    ui.weak(if preset { "Presets" } else { "Renderer types" });
                                    for option in
                                        self.target.options.iter().filter(|o| o.preset == preset)
                                    {
                                        record(
                                            &mut self.rects,
                                            &format!("Type {}", option.name),
                                            ui.selectable_value(
                                                &mut self.renderer_type,
                                                option.name.clone(),
                                                &option.description,
                                            ),
                                        );
                                    }
                                }
                            });
                        record(&mut self.rects, "Type", combo.response);
                        if self.renderer_type != previous {
                            self.follow_name(state);
                        }
                        ui.end_row();
                        ui.label("Renderer name");
                        let response = ui.text_edit_singleline(&mut self.renderer_name);
                        if response.changed() {
                            self.name_edited = !self.renderer_name.trim().is_empty();
                            self.follow_name(state);
                        }
                        record(&mut self.rects, "Name", response);
                        ui.end_row();
                    });
                if self.target.kind == Kind::Molecule {
                    ui.add_enabled_ui(!self.is_preset(), |ui| {
                        record(
                            &mut self.rects,
                            "Selection enabled",
                            ui.checkbox(&mut self.selection_enabled, "Selection"),
                        );
                        ui.add_enabled_ui(self.selection_enabled, |ui| {
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
                                if let Some(applied) = self.builder.show(
                                    ui,
                                    state,
                                    Some(self.target.object),
                                    revision,
                                    &self.selection,
                                ) {
                                    self.selection = applied.expression;
                                    self.choosing = false;
                                }
                                self.rects.extend(self.builder.rects.clone());
                            }
                        });
                    });
                }
                ui.separator();
                if self.target.kind == Kind::Map {
                    ui.label("View after loading");
                    for (value, label) in [
                        (GuiMapCenterPolicy::Auto, "Auto (by map kind)"),
                        (
                            GuiMapCenterPolicy::SetMapCenter,
                            "Set map center (keep the view)",
                        ),
                        (
                            GuiMapCenterPolicy::MoveViewCenter,
                            "Move view center (fit the map)",
                        ),
                    ] {
                        record(
                            &mut self.rects,
                            label,
                            ui.radio_value(&mut self.map_center, value, label),
                        );
                    }
                } else {
                    record(
                        &mut self.rects,
                        "Recenter",
                        ui.checkbox(&mut self.recenter, "Recenter view"),
                    );
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
            let create = ui.add_enabled(invalid.is_none(), egui::Button::new("Create"));
            if create.clicked() {
                action = Some(Action::Create);
            }
            record(&mut self.rects, "Create", create);
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
        format!("New renderer {label}"),
        [r.min.x, r.min.y, r.width(), r.height()],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const PDB: &[u8] =
        b"ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n";

    #[test]
    fn renderer_form_shares_history_filters_group_presets_and_preserves_user_names() {
        let mut state = DesktopAppState::new();
        state.open_bytes("molecule.pdb", PDB).unwrap();
        let object = state.active_object_summaries()[0].uid;
        let mut form = RendererDialog::new(&state, object, None).unwrap();
        assert_eq!(form.renderer_type, "simple");
        assert!(!form.selection_enabled);
        assert!(form.recenter);
        state.set_molecule_selection_text(object, "aid 1").unwrap();
        state.remember_renderer_type("MolCoord", "cpk");
        form = RendererDialog::new(&state, object, None).unwrap();
        assert_eq!(form.renderer_type, "cpk");
        assert!(form.selection_enabled);
        assert_eq!(form.arguments()["selection"], "aid 1");
        let name = form.renderer_name.clone();
        state
            .new_renderer_from_setup(form.target.scene, object, "cpk", &name, None)
            .unwrap();
        form.follow_name(&state);
        assert_ne!(form.renderer_name, name);
        form.renderer_name = "chosen".into();
        form.name_edited = true;
        form.renderer_type = "ballstick".into();
        form.follow_name(&state);
        assert_eq!(form.renderer_name, "chosen");
        let preset = form
            .target
            .options
            .iter()
            .find(|o| o.preset)
            .unwrap()
            .name
            .clone();
        form.renderer_type = preset.clone();
        assert!(form.arguments()["selection"].is_null());
        form.remember(&mut state);
        assert_eq!(
            RendererDialog::new(&state, object, None)
                .unwrap()
                .renderer_type,
            preset
        );
        state.create_renderer_group(object, "bundle").unwrap();
        let grouped = RendererDialog::new(&state, object, Some("bundle".into())).unwrap();
        assert_eq!(grouped.renderer_type, "simple");
        assert!(grouped.target.options.iter().all(|o| !o.preset));
        assert!(grouped.validation_error(&state).is_none());
        assert_eq!(grouped.arguments()["group"], "bundle");
        assert!(RendererDialog::new(&state, object, Some("removed".into()))
            .unwrap()
            .validation_error(&state)
            .is_some());
        state.new_scene("another", true);
        assert!(grouped
            .validation_error(&state)
            .unwrap()
            .contains("active scene"));
    }

    #[test]
    fn map_form_uses_separate_history_and_has_no_molecular_selection() {
        let mut state = DesktopAppState::new();
        let dx = b"object 1 class gridpositions counts 2 2 2\norigin 10 20 30\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 2 2 2\nobject 3 class array type double rank 0 items 8 data follows\n0 1 2 3 4 5 6 7\n";
        state.open_bytes("potential.dx", dx).unwrap();
        let object = state.density_map_summaries(state.active_scene().unwrap())[0].uid;
        state.remember_renderer_type("MolCoord", "cpk");
        let mut form = RendererDialog::new(&state, object, None).unwrap();
        assert_eq!(form.target.class, "ElePotMap");
        assert_eq!(form.target.kind, Kind::Map);
        assert_eq!(form.renderer_type, "contour");
        assert_eq!(form.arguments()["mapCenterPolicy"], "auto");
        form.selection_enabled = true;
        assert!(form.arguments()["selection"].is_null());
        form.renderer_type = "isosurf".into();
        form.remember(&mut state);
        assert_eq!(
            RendererDialog::new(&state, object, None)
                .unwrap()
                .renderer_type,
            "isosurf"
        );
    }
}
