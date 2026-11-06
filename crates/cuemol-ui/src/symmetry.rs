use cuemol_core::Uid;
use cuemol_desktop_core::{DesktopAppState, GuiCrystalSummary};
use serde_json::{json, Value};
use std::collections::BTreeMap;

type Changes = Vec<(&'static str, Value)>;
pub const LATTICES: [(&str, &str); 7] = [
    ("TRICLINIC", "Triclinic"),
    ("MONOCLINIC", "Monoclinic"),
    ("ORTHORHOMBIC", "Orthorhombic"),
    ("TRIGONAL", "Trigonal"),
    ("TETRAGONAL", "Tetragonal"),
    ("HEXAGONAL", "Hexagonal"),
    ("CUBIC", "Cubic"),
];

fn lattice_label(lattice: &str) -> &str {
    LATTICES
        .iter()
        .find(|(key, _)| *key == lattice)
        .map_or("Unknown", |(_, label)| *label)
}

#[derive(Debug)]
struct CrystalDraft {
    scene: Uid,
    object: Uid,
    name: String,
    original: Option<GuiCrystalSummary>,
    lattice: String,
    group: i32,
    restrict: bool,
    values: [f64; 6],
}

fn restrict_cell(values: &mut [f64; 6], lattice: &str, group: &str, restrict: bool) -> [bool; 6] {
    let mut locked = [false; 6];
    if !restrict {
        return locked;
    }
    match lattice {
        "MONOCLINIC" => {
            values[3] = 90.;
            values[5] = 90.;
            locked[3] = true;
            locked[5] = true;
        }
        "ORTHORHOMBIC" | "TETRAGONAL" | "CUBIC" => {
            values[3..].fill(90.);
            locked[3..].fill(true);
            if lattice != "ORTHORHOMBIC" {
                values[1] = values[0];
                locked[1] = true;
            }
            if lattice == "CUBIC" {
                values[2] = values[0];
                locked[2] = true;
            }
        }
        "TRIGONAL" | "HEXAGONAL" => {
            values[1] = values[0];
            locked[1] = true;
            if group.trim().starts_with('R') {
                values[2] = values[0];
                locked[2] = true;
            } else {
                values[3..].copy_from_slice(&[90., 90., 120.]);
                locked[3..].fill(true);
            }
        }
        _ => {}
    }
    locked
}

#[derive(Debug, Default)]
pub struct SymmetryPanel {
    pub rects: BTreeMap<String, [f32; 4]>,
    pub target: Option<Uid>,
    scene: Option<Uid>,
    confirmation: Option<(Uid, Uid)>,
    draft: Option<CrystalDraft>,
}

impl SymmetryPanel {
    pub fn open_editor(&mut self, state: &DesktopAppState, object: Uid) {
        let Some(scene) = state.active_scene() else {
            return;
        };
        let Some(summary) = state
            .active_object_summaries()
            .into_iter()
            .find(|item| item.uid == object)
        else {
            return;
        };
        let original = state.crystal_summary(object);
        let (lattice, group, values) = original.as_ref().map_or(
            ("TRICLINIC".into(), 1, [1., 1., 1., 90., 90., 90.]),
            |cell| {
                (
                    cell.lattice.clone(),
                    cell.space_group_number.unwrap_or(1),
                    [cell.a, cell.b, cell.c, cell.alpha, cell.beta, cell.gamma],
                )
            },
        );
        self.scene = Some(scene);
        self.confirmation = None;
        self.draft = Some(CrystalDraft {
            scene,
            object,
            name: summary.name,
            original,
            lattice,
            group,
            restrict: false,
            values,
        });
    }

    fn validate_context(&mut self, state: &DesktopAppState) {
        if self.scene != state.active_scene() {
            self.scene = state.active_scene();
            self.target = None;
            self.confirmation = None;
            self.draft = None;
        }
        let exists = |scene, object| {
            state.active_scene() == Some(scene)
                && state.scene_manager().scene_uid_for_object(object) == Some(scene)
        };
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| !exists(draft.scene, draft.object))
        {
            self.draft = None;
        }
        if self
            .confirmation
            .is_some_and(|(scene, object)| !exists(scene, object))
        {
            self.confirmation = None;
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        preferred: Option<Uid>,
    ) -> Changes {
        self.rects.clear();
        self.validate_context(state);
        let mut changes = Vec::new();
        let objects = state.active_object_summaries();
        let maps = state.active_density_map_summaries();
        let targets = objects
            .iter()
            .map(|item| (item.uid, item.name.as_str(), true))
            .chain(
                maps.iter()
                    .map(|item| (item.uid, item.name.as_str(), false)),
            )
            .collect::<Vec<_>>();
        let valid = |id: &Uid| targets.iter().any(|(uid, _, _)| uid == id);
        self.target = self
            .target
            .filter(valid)
            .or(preferred.filter(valid))
            .or_else(|| targets.first().map(|(id, _, _)| *id));
        let selected = targets.iter().find(|(id, _, _)| Some(*id) == self.target);
        let name = selected.map_or("(no objects)", |(_, name, _)| *name);
        let response = ui
            .add_enabled_ui(!targets.is_empty(), |ui| {
                egui::ComboBox::from_id_salt("symmetry-object")
                    .width((ui.available_width() - 8.).max(40.))
                    .selected_text(name)
                    .show_ui(ui, |ui| {
                        for (id, name, _) in &targets {
                            let response = ui.selectable_value(&mut self.target, Some(*id), *name);
                            record(
                                &mut self.rects,
                                &format!("Symmetry target {name}"),
                                response.rect,
                            );
                        }
                    })
                    .response
            })
            .inner;
        record(&mut self.rects, "Symmetry object", response.rect);
        let molecule = targets
            .iter()
            .find(|(id, _, _)| Some(*id) == self.target)
            .is_some_and(|(_, _, molecule)| *molecule);
        let crystal = self.target.and_then(|id| {
            if molecule {
                state.crystal_summary(id)
            } else {
                state.density_crystal_summary(id)
            }
        });
        let valid_cell = crystal
            .as_ref()
            .is_some_and(|cell| cell.a >= 0.1 && cell.b >= 0.1 && cell.c >= 0.1);
        let mut heading = crystal.as_ref().map_or_else(
            || "Unknown".into(),
            |cell| lattice_label(&cell.lattice).to_string(),
        );
        if let Some(cell) = crystal
            .as_ref()
            .filter(|cell| !cell.space_group_name.is_empty())
        {
            heading.push_str(&format!(", S.g.: {}", cell.space_group_name));
        }
        let response = ui
            .add(egui::Label::new(&heading).truncate())
            .on_hover_text(&heading);
        record(&mut self.rects, "Symmetry crystal", response.rect);
        let values = crystal
            .as_ref()
            .map(|cell| [cell.a, cell.b, cell.c, cell.alpha, cell.beta, cell.gamma]);
        let width = ((ui.available_width() - 2. * ui.spacing().item_spacing.x) / 3.).max(20.);
        egui::Grid::new("symmetry-cell")
            .num_columns(3)
            .min_col_width(width)
            .max_col_width(width)
            .spacing([4., 2.])
            .show(ui, |ui| {
                for (index, axis) in ["a", "b", "c", "α", "β", "γ"].iter().enumerate() {
                    let text = values.map_or_else(
                        || format!("{axis}= -"),
                        |values| {
                            format!(
                                "{axis}= {:.2}{}",
                                values[index],
                                if index < 3 { " Å" } else { "°" }
                            )
                        },
                    );
                    let response = ui
                        .add_sized([width, 20.], egui::Label::new(&text).truncate())
                        .on_hover_text(text);
                    record(
                        &mut self.rects,
                        &format!("Symmetry readout {axis}"),
                        response.rect,
                    );
                    if index % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        ui.horizontal_wrapped(|ui| {
            let change = ui.add_enabled(molecule, egui::Button::new("Change ..."));
            record(&mut self.rects, "Symmetry change", change.rect);
            if change.clicked() {
                if let Some(id) = self.target {
                    if crystal.is_some() {
                        self.open_editor(state, id);
                    } else {
                        self.confirmation = state.active_scene().map(|scene| (scene, id));
                    }
                }
            }
            let menu = ui
                .add_enabled_ui(molecule && valid_cell, |ui| {
                    ui.menu_button("Symm mol", |ui| {
                        for (label, extent) in [
                            ("20 Å", Some(20.)),
                            ("50 Å", Some(50.)),
                            ("100 Å", Some(100.)),
                            ("200 Å", Some(200.)),
                            ("Unit cell", None),
                        ] {
                            let response = ui.button(label);
                            record(
                                &mut self.rects,
                                &format!("Symmetry extent {label}"),
                                response.rect,
                            );
                            if response.clicked() {
                                changes.push((
                                    "show_symmetry",
                                    json!({"objId":self.target.unwrap().get(),"extent":extent}),
                                ));
                                ui.close();
                            }
                        }
                    })
                    .response
                })
                .inner;
            record(&mut self.rects, "Symmetry mates", menu.rect);
            let cell = ui.add_enabled(valid_cell, egui::Button::new("Unit cell"));
            record(&mut self.rects, "Symmetry unit cell", cell.rect);
            if cell.clicked() {
                changes.push((
                    "show_unit_cell",
                    json!({"objId":self.target.unwrap().get()}),
                ));
            }
        });
        changes
    }

    pub fn dialog(&mut self, ctx: &egui::Context, state: &DesktopAppState) -> Changes {
        self.validate_context(state);
        let mut changes = Vec::new();
        if let Some((_, object)) = self.confirmation {
            let mut create = false;
            let mut cancel = false;
            let modal = egui::Modal::new(egui::Id::new("symmetry-create")).show(ctx, |ui| {
                ui.heading("Create crystal information");
                ui.label("The selected molecule has no crystal information. Create it?");
                ui.horizontal(|ui| {
                    let ok = ui.button("OK");
                    record(&mut self.rects, "Symmetry create OK", ok.rect);
                    create = ok.clicked();
                    let no = ui.button("Cancel");
                    record(&mut self.rects, "Symmetry create Cancel", no.rect);
                    cancel = no.clicked();
                });
            });
            if create {
                self.open_editor(state, object);
            } else if cancel || modal.should_close() {
                self.confirmation = None;
            }
            return changes;
        }
        let Some(draft) = self.draft.as_mut() else {
            return changes;
        };
        let mut apply = false;
        let mut cancel = false;
        let mut effective = draft.values;
        let modal = egui::Modal::new(egui::Id::new("symmetry-change")).show(ctx, |ui| {
            ui.set_width(350.);
            ui.heading("Change symmetry");
            ui.label(&draft.name);
            ui.separator();
            ui.strong("Symmetry");
            let response = egui::ComboBox::from_label("Crystal system")
                .selected_text(lattice_label(&draft.lattice))
                .show_ui(ui, |ui| {
                    for (key, label) in LATTICES {
                        let response = ui.selectable_value(&mut draft.lattice, key.into(), label);
                        record(
                            &mut self.rects,
                            &format!("Symmetry lattice {label}"),
                            response.rect,
                        );
                    }
                })
                .response;
            record(&mut self.rects, "Symmetry lattice", response.rect);
            let groups = state.space_group_summaries(&draft.lattice);
            if !groups.iter().any(|item| item.id == draft.group) {
                draft.group = groups.first().map_or(1, |item| item.id);
            }
            let label = groups
                .iter()
                .find(|item| item.id == draft.group)
                .map_or("(unknown)", |item| item.canonical_name.as_str());
            let response = egui::ComboBox::from_label("Space group")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    for item in &groups {
                        let response =
                            ui.selectable_value(&mut draft.group, item.id, &item.canonical_name);
                        record(
                            &mut self.rects,
                            &format!("Symmetry group {}", item.canonical_name),
                            response.rect,
                        );
                    }
                })
                .response;
            record(&mut self.rects, "Symmetry group", response.rect);
            ui.label(format!("Space group number: {}", draft.group));
            let mut bio = false;
            ui.add_enabled(false, egui::Checkbox::new(&mut bio, "Biomolecules only"));
            ui.separator();
            ui.strong("Cell dimension");
            let response = ui.checkbox(&mut draft.restrict, "Restrict by symmetry");
            record(&mut self.rects, "Symmetry restrict", response.rect);
            let label = groups
                .iter()
                .find(|item| item.id == draft.group)
                .map_or("", |item| item.canonical_name.as_str());
            let locks = restrict_cell(&mut effective, &draft.lattice, label, draft.restrict);
            egui::Grid::new("symmetry-edit-cell")
                .num_columns(4)
                .show(ui, |ui| {
                    for index in [0, 3, 1, 4, 2, 5] {
                        let key = ["a", "b", "c", "alpha", "beta", "gamma"][index];
                        ui.label(["a (Å)", "b (Å)", "c (Å)", "α (°)", "β (°)", "γ (°)"][index]);
                        let response = ui.add_enabled(
                            !locks[index],
                            egui::DragValue::new(&mut effective[index]).speed(0.1),
                        );
                        if response.changed() && !locks[index] {
                            draft.values[index] = effective[index];
                        }
                        record(
                            &mut self.rects,
                            &format!("Symmetry edit {key}"),
                            response.rect,
                        );
                        if index >= 3 {
                            ui.end_row();
                        }
                    }
                });
            effective = draft.values;
            restrict_cell(&mut effective, &draft.lattice, label, draft.restrict);
            let valid = effective
                .iter()
                .all(|value| value.is_finite() && *value > 0.)
                && effective[3..].iter().all(|value| *value < 180.);
            if !valid {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    "Use positive lengths and angles between 0 and 180 degrees.",
                );
            }
            ui.horizontal(|ui| {
                let response = ui.add_enabled(valid, egui::Button::new("OK"));
                record(&mut self.rects, "Symmetry edit OK", response.rect);
                apply = response.clicked();
                let response = ui.button("Cancel");
                record(&mut self.rects, "Symmetry edit Cancel", response.rect);
                cancel = response.clicked();
            });
        });
        if apply {
            let unchanged = draft.original.as_ref().is_some_and(|cell| {
                cell.space_group_number == Some(draft.group)
                    && [cell.a, cell.b, cell.c, cell.alpha, cell.beta, cell.gamma]
                        .iter()
                        .zip(effective)
                        .all(|(a, b)| (a - b).abs() < 1e-3)
            });
            if !unchanged {
                let [a, b, c, alpha, beta, gamma] = effective;
                changes.push(("set_symmetry", json!({"objId":draft.object.get(),"spaceGroup":draft.group,"a":a,"b":b,"c":c,"alpha":alpha,"beta":beta,"gamma":gamma})));
            }
        }
        if apply || cancel || modal.should_close() {
            self.draft = None;
        }
        changes
    }
}

fn record(rects: &mut BTreeMap<String, [f32; 4]>, name: &str, rect: egui::Rect) {
    rects.insert(
        name.into(),
        [rect.min.x, rect.min.y, rect.width(), rect.height()],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (DesktopAppState, Uid) {
        let mut state = DesktopAppState::new();
        let scene = state.ensure_active_scene();
        let object = state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("probe", Default::default());
        (state, object)
    }

    fn frame(
        panel: &mut SymmetryPanel,
        ctx: &egui::Context,
        state: &DesktopAppState,
        events: Vec<egui::Event>,
    ) -> Changes {
        let mut changes = Vec::new();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 650.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::SidePanel::left("test-symmetry")
                    .exact_width(260.)
                    .show(ctx, |ui| {
                        changes.extend(panel.show(ui, state, None));
                    });
                changes.extend(panel.dialog(ctx, state));
            },
        );
        changes
    }

    fn click(
        panel: &mut SymmetryPanel,
        ctx: &egui::Context,
        state: &DesktopAppState,
        key: &str,
    ) -> Changes {
        frame(panel, ctx, state, Vec::new());
        frame(panel, ctx, state, Vec::new());
        let [x, y, w, h] = panel.rects[key];
        let pos = egui::pos2(x + w / 2., y + h / 2.);
        frame(
            panel,
            ctx,
            state,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        frame(
            panel,
            ctx,
            state,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        )
    }

    #[test]
    fn symmetry_lattice_constraints_match_reference_cases() {
        let initial = [42., 20., 30., 70., 80., 100.];
        for (lattice, group, expected, locks) in [
            ("TRICLINIC", "P 1", initial, [false; 6]),
            (
                "MONOCLINIC",
                "P 2",
                [42., 20., 30., 90., 80., 90.],
                [false, false, false, true, false, true],
            ),
            (
                "ORTHORHOMBIC",
                "P 2 2 2",
                [42., 20., 30., 90., 90., 90.],
                [false, false, false, true, true, true],
            ),
            (
                "TETRAGONAL",
                "P 4",
                [42., 42., 30., 90., 90., 90.],
                [false, true, false, true, true, true],
            ),
            (
                "CUBIC",
                "P 2 3",
                [42., 42., 42., 90., 90., 90.],
                [false, true, true, true, true, true],
            ),
            (
                "TRIGONAL",
                "R 3",
                [42., 42., 42., 70., 80., 100.],
                [false, true, true, false, false, false],
            ),
            (
                "HEXAGONAL",
                "R 3",
                [42., 42., 42., 70., 80., 100.],
                [false, true, true, false, false, false],
            ),
            (
                "HEXAGONAL",
                "P 6",
                [42., 42., 30., 90., 90., 120.],
                [false, true, false, true, true, true],
            ),
        ] {
            let mut values = initial;
            assert_eq!(
                restrict_cell(&mut values, lattice, group, false),
                [false; 6]
            );
            assert_eq!(values, initial);
            assert_eq!(restrict_cell(&mut values, lattice, group, true), locks);
            assert_eq!(values, expected);
        }
    }

    #[test]
    fn symmetry_creation_cancel_defaults_and_unchanged_edits_do_not_mutate() {
        let (mut state, object) = state();
        let ctx = egui::Context::default();
        let mut panel = SymmetryPanel::default();
        frame(&mut panel, &ctx, &state, Vec::new());
        assert!(click(&mut panel, &ctx, &state, "Symmetry mates").is_empty());
        assert!(click(&mut panel, &ctx, &state, "Symmetry unit cell").is_empty());
        click(&mut panel, &ctx, &state, "Symmetry change");
        assert!(panel.confirmation.is_some());
        click(&mut panel, &ctx, &state, "Symmetry create Cancel");
        assert!(panel.confirmation.is_none());
        assert!(state.crystal_summary(object).is_none());
        click(&mut panel, &ctx, &state, "Symmetry change");
        click(&mut panel, &ctx, &state, "Symmetry create OK");
        assert_eq!(
            panel.draft.as_ref().unwrap().values,
            [1., 1., 1., 90., 90., 90.]
        );
        let changes = click(&mut panel, &ctx, &state, "Symmetry edit OK");
        assert_eq!(changes.len(), 1);
        let (tool, args) = &changes[0];
        assert_eq!(*tool, "set_symmetry");
        assert_eq!(args["objId"], object.get());
        cuemol_command::call_tool(&mut state, &mut cuemol_qsys::Camera::new(), tool, args).unwrap();
        assert_eq!(state.crystal_summary(object).unwrap().a, 1.);
        click(&mut panel, &ctx, &state, "Symmetry change");
        assert!(click(&mut panel, &ctx, &state, "Symmetry edit OK").is_empty());
        click(&mut panel, &ctx, &state, "Symmetry change");
        panel.draft.as_mut().unwrap().values[0] = 42.;
        assert!(click(&mut panel, &ctx, &state, "Symmetry edit Cancel").is_empty());
        assert_eq!(state.crystal_summary(object).unwrap().a, 1.);
        panel.open_editor(&state, object);
        state.new_scene("other", true);
        frame(&mut panel, &ctx, &state, Vec::new());
        assert!(panel.draft.is_none());
        assert!(panel.target.is_none());
    }

    #[test]
    fn symmetry_cell_actions_reuse_renderers_and_keep_the_view() {
        let (mut state, object) = state();
        state
            .change_crystal_info(object, 20., 30., 40., 90., 90., 90., 19)
            .unwrap();
        let ctx = egui::Context::default();
        let mut panel = SymmetryPanel::default();
        let mut camera = cuemol_qsys::Camera::new();
        camera.set_center(cuemol_core::Vector4::new3(4., 5., 6.));
        let original = camera.clone();
        frame(&mut panel, &ctx, &state, Vec::new());
        for _ in 0..2 {
            let changes = click(&mut panel, &ctx, &state, "Symmetry unit cell");
            assert_eq!(changes.len(), 1);
            for (tool, args) in changes {
                cuemol_command::call_tool(&mut state, &mut camera, tool, &args).unwrap();
            }
        }
        assert_eq!(state.renderer_summaries(object).len(), 1);
        click(&mut panel, &ctx, &state, "Symmetry mates");
        let changes = click(&mut panel, &ctx, &state, "Symmetry extent 50 Å");
        assert_eq!(changes.len(), 1);
        for (tool, args) in changes {
            cuemol_command::call_tool(&mut state, &mut camera, tool, &args).unwrap();
        }
        let renderer = state
            .renderer_summaries(object)
            .into_iter()
            .find(|renderer| renderer.renderer_type == "*symm")
            .unwrap();
        assert_eq!(
            state
                .renderer_property(object, renderer.uid, "extent")
                .as_deref(),
            Some("50")
        );
        assert_eq!(
            state
                .renderer_property(object, renderer.uid, "center")
                .as_deref(),
            Some(camera.center().to_string().as_str())
        );
        assert_eq!(camera, original);
    }

    #[test]
    fn symmetry_restriction_toggle_keeps_the_unconstrained_draft() {
        let (state, object) = state();
        let ctx = egui::Context::default();
        let mut panel = SymmetryPanel::default();
        panel.open_editor(&state, object);
        let draft = panel.draft.as_mut().unwrap();
        draft.lattice = "CUBIC".into();
        draft.values = [42., 25., 31., 70., 80., 100.];
        click(&mut panel, &ctx, &state, "Symmetry restrict");
        assert_eq!(
            panel.draft.as_ref().unwrap().values,
            [42., 25., 31., 70., 80., 100.]
        );
        click(&mut panel, &ctx, &state, "Symmetry restrict");
        let changes = click(&mut panel, &ctx, &state, "Symmetry edit OK");
        assert_eq!(changes[0].1["b"], 25.);
        assert_eq!(changes[0].1["alpha"], 70.);
    }
}
