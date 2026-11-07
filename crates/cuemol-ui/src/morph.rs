use cuemol_core::{CueMolError, Result, Uid};
use cuemol_desktop_core::DesktopAppState;
use std::collections::BTreeMap;

pub const NAME_COLUMN_DEFAULT_WIDTH: f32 = 180.;
pub const COLUMN_MIN_WIDTH: f32 = 100.;
const SPLITTER_WIDTH: f32 = 5.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Convert,
    AddPdb,
    AddSource(Uid),
    Remove(usize),
    Close,
}

#[derive(Debug)]
pub struct MorphDialog {
    pub scene: Option<Uid>,
    pub object: Option<Uid>,
    pub selected: Option<usize>,
    pub name_width: f32,
    pub error: String,
    pub rects: BTreeMap<String, [f32; 4]>,
    pub generation: u64,
    source: Option<Uid>,
    choosing_source: bool,
    drag_start_width: Option<f32>,
}

impl Default for MorphDialog {
    fn default() -> Self {
        Self {
            scene: None,
            object: None,
            selected: None,
            name_width: NAME_COLUMN_DEFAULT_WIDTH,
            error: String::new(),
            rects: BTreeMap::new(),
            generation: 0,
            source: None,
            choosing_source: false,
            drag_start_width: None,
        }
    }
}

impl MorphDialog {
    pub fn opened(&mut self, state: &DesktopAppState) {
        *self = Self {
            scene: state.active_scene(),
            object: target_objects(state).first().map(|o| o.uid),
            generation: self.generation.wrapping_add(1),
            name_width: self.name_width,
            ..Self::default()
        };
    }

    pub fn target(&self, state: &DesktopAppState) -> Result<Uid> {
        if self.scene != state.active_scene() {
            return Err(CueMolError::invalid_argument(
                "The active scene changed. Reopen Morph animation tool.",
            ));
        }
        self.object
            .filter(|id| target_objects(state).iter().any(|o| o.uid == *id))
            .ok_or_else(|| CueMolError::invalid_argument("Select a molecule."))
    }

    pub fn insert_before(&self) -> Option<usize> {
        self.selected.map(|index| index + 1)
    }

    pub fn added_frame(&mut self, frame_count: usize) {
        self.selected = self
            .insert_before()
            .map(|i| i.min(frame_count.saturating_sub(1)));
        self.error.clear();
    }

    pub fn apply(&mut self, state: &mut DesktopAppState, action: Action) -> Result<bool> {
        let target = self.target(state)?;
        if action == Action::Convert {
            let (_, changed) = state.initialize_morph_object(target)?;
            self.error.clear();
            return Ok(changed);
        }
        if state.morph_frame_value(target).is_none() {
            return Err(CueMolError::invalid_argument(
                "Convert the molecule to MorphMol first.",
            ));
        }
        match action {
            Action::AddSource(source) => {
                if source == target
                    || !ordinary_source(state, source)
                    || !state
                        .active_object_summaries()
                        .iter()
                        .any(|o| o.uid == source)
                {
                    return Err(CueMolError::invalid_argument(
                        "Select an ordinary MolCoord molecule.",
                    ));
                }
                let summary = state.insert_morph_frame_from_object(
                    target,
                    source,
                    "",
                    self.insert_before(),
                )?;
                self.added_frame(summary.frame_count);
                self.choosing_source = false;
            }
            Action::Remove(index) => {
                state.remove_morph_frame(target, index)?;
                self.selected = None;
            }
            _ => return Ok(false),
        }
        self.error.clear();
        Ok(true)
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &DesktopAppState,
        busy: bool,
    ) -> Option<Action> {
        self.rects.clear();
        let mut action = None;
        let objects = target_objects(state);
        let previous_object = self.object;
        ui.add_enabled_ui(!busy, |ui| {
            let label = objects
                .iter()
                .find(|o| Some(o.uid) == self.object)
                .map_or("Select molecule", |o| o.name.as_str());
            let combo = egui::ComboBox::from_label("Target molecule")
                .selected_text(label)
                .width(300.)
                .show_ui(ui, |ui| {
                    for object in &objects {
                        record(
                            &mut self.rects,
                            &format!("Target {}", object.name),
                            ui.selectable_value(&mut self.object, Some(object.uid), &object.name),
                        );
                    }
                });
            record(&mut self.rects, "Target molecule", combo.response);
        });
        if previous_object != self.object {
            self.selected = None;
            self.source = None;
            self.choosing_source = false;
            self.error.clear();
            self.generation = self.generation.wrapping_add(1);
        }
        let valid = self.target(state).is_ok();
        let morph = self
            .object
            .is_some_and(|id| state.morph_frame_value(id).is_some());
        let frames = self
            .object
            .map(|id| state.morph_frame_summaries(id))
            .unwrap_or_default();
        if self.selected.is_some_and(|index| index >= frames.len()) {
            self.selected = None;
        }
        if !morph {
            let convert = ui.add_enabled(valid && !busy, egui::Button::new("Convert to MorphMol"));
            if convert.clicked() {
                action = Some(Action::Convert);
            }
            record(&mut self.rects, "Convert to MorphMol", convert);
        } else {
            ui.add_enabled_ui(valid && !busy, |ui| {
                egui::Frame::new()
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .show(ui, |ui| {
                        let available = ui
                            .available_width()
                            .max(2. * COLUMN_MIN_WIDTH + SPLITTER_WIDTH);
                        let maximum =
                            (available - COLUMN_MIN_WIDTH - SPLITTER_WIDTH).max(COLUMN_MIN_WIDTH);
                        let name_width = self.name_width.clamp(COLUMN_MIN_WIDTH, maximum);
                        let source_width =
                            (available - name_width - SPLITTER_WIDTH).max(COLUMN_MIN_WIDTH);
                        let height = ui.spacing().interact_size.y;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.;
                            ui.add_sized(
                                [name_width, height],
                                egui::Label::new(egui::RichText::new("Name").strong()),
                            );
                            let (rect, splitter) = ui.allocate_exact_size(
                                egui::vec2(SPLITTER_WIDTH, height),
                                egui::Sense::drag(),
                            );
                            let splitter = splitter
                                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
                                .on_hover_text("Resize Name column; double-click to reset");
                            ui.painter().line_segment(
                                [rect.center_top(), rect.center_bottom()],
                                ui.visuals().widgets.noninteractive.bg_stroke,
                            );
                            if splitter.double_clicked() {
                                self.name_width = NAME_COLUMN_DEFAULT_WIDTH;
                                self.drag_start_width = None;
                            } else {
                                if splitter.drag_started() {
                                    self.drag_start_width = Some(name_width);
                                }
                                if splitter.dragged() {
                                    if let Some(initial) = self.drag_start_width {
                                        self.name_width = (initial + splitter.drag_delta().x)
                                            .clamp(COLUMN_MIN_WIDTH, maximum);
                                    }
                                }
                                if splitter.drag_stopped() {
                                    self.drag_start_width = None;
                                }
                            }
                            record(&mut self.rects, "Resize Name column", splitter);
                            ui.add_sized(
                                [source_width, height],
                                egui::Label::new(egui::RichText::new("Source").strong()),
                            );
                        });
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .id_salt("morph-frames")
                            .min_scrolled_height(120.)
                            .max_height(220.)
                            .show(ui, |ui| {
                                for frame in &frames {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 0.;
                                        let selected = self.selected == Some(frame.index);
                                        let name = ui
                                            .add_sized(
                                                [name_width, height],
                                                egui::Button::selectable(selected, &frame.name),
                                            )
                                            .on_hover_text(&frame.name);
                                        ui.allocate_space(egui::vec2(SPLITTER_WIDTH, height));
                                        let source = ui
                                            .add_sized(
                                                [source_width, height],
                                                egui::Button::selectable(selected, &frame.source),
                                            )
                                            .on_hover_text(&frame.source);
                                        if name.clicked() || source.clicked() {
                                            self.selected = Some(frame.index);
                                        }
                                        record(
                                            &mut self.rects,
                                            &format!("Frame {}", frame.index),
                                            name,
                                        );
                                    });
                                }
                            });
                    });
                ui.horizontal(|ui| {
                    let can_delete = self
                        .selected
                        .and_then(|i| frames.get(i))
                        .is_some_and(|f| f.source_type != "<this>");
                    let delete = ui.add_enabled(can_delete, egui::Button::new("Delete"));
                    if delete.clicked() {
                        action = self.selected.map(Action::Remove);
                    }
                    record(&mut self.rects, "Delete frame", delete);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let add = ui.menu_button(
                            format!("Add {}", egui_phosphor::regular::CARET_DOWN),
                            |ui| {
                                let pdb = ui.button("Add PDB file...");
                                if pdb.clicked() {
                                    action = Some(Action::AddPdb);
                                    ui.close();
                                }
                                record(&mut self.rects, "Add PDB file", pdb);
                                let molecule = ui.button("Add mol...");
                                if molecule.clicked() {
                                    self.choosing_source = true;
                                    self.source = objects
                                        .iter()
                                        .find(|o| {
                                            Some(o.uid) != self.object
                                                && ordinary_source(state, o.uid)
                                        })
                                        .map(|o| o.uid);
                                    ui.close();
                                }
                                record(&mut self.rects, "Add mol", molecule);
                            },
                        );
                        record(&mut self.rects, "Add frame", add.response);
                    });
                });
                if self.choosing_source {
                    ui.group(|ui| {
                        ui.strong("Add molecule frame");
                        let sources: Vec<_> = objects
                            .iter()
                            .filter(|o| Some(o.uid) != self.object && ordinary_source(state, o.uid))
                            .collect();
                        let label = sources
                            .iter()
                            .find(|o| Some(o.uid) == self.source)
                            .map_or("Select molecule", |o| o.name.as_str());
                        let combo = egui::ComboBox::from_id_salt("morph-source")
                            .selected_text(label)
                            .width(300.)
                            .show_ui(ui, |ui| {
                                for object in &sources {
                                    record(
                                        &mut self.rects,
                                        &format!("Source {}", object.name),
                                        ui.selectable_value(
                                            &mut self.source,
                                            Some(object.uid),
                                            &object.name,
                                        ),
                                    );
                                }
                            });
                        record(&mut self.rects, "Source molecule", combo.response);
                        ui.horizontal(|ui| {
                            let add = ui.add_enabled(
                                sources.iter().any(|o| Some(o.uid) == self.source),
                                egui::Button::new("Add"),
                            );
                            if add.clicked() {
                                action = self.source.map(Action::AddSource);
                            }
                            record(&mut self.rects, "Confirm molecule frame", add);
                            let cancel = ui.button("Cancel");
                            if cancel.clicked() {
                                self.choosing_source = false;
                            }
                            record(&mut self.rects, "Cancel molecule frame", cancel);
                        });
                    });
                }
            });
        }
        if let Err(error) = self.target(state) {
            ui.colored_label(ui.visuals().error_fg_color, error.to_string());
        } else if !self.error.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, &self.error);
        }
        ui.separator();
        let close = ui.button("Close");
        if close.clicked() {
            action = Some(Action::Close);
        }
        record(&mut self.rects, "Close morph", close);
        action
    }
}

fn record(rects: &mut BTreeMap<String, [f32; 4]>, name: &str, response: egui::Response) {
    let rect = response.rect;
    rects.insert(
        format!("Morph {name}"),
        [rect.min.x, rect.min.y, rect.width(), rect.height()],
    );
}

fn target_objects(state: &DesktopAppState) -> Vec<cuemol_desktop_core::GuiObjectSummary> {
    state
        .active_object_summaries()
        .into_iter()
        .filter(|summary| {
            state
                .scene_manager()
                .object(summary.uid)
                .is_some_and(|object| matches!(object.object_type(), "MolCoord" | "MorphMol"))
        })
        .collect()
}

fn ordinary_source(state: &DesktopAppState, id: Uid) -> bool {
    state.morph_frame_value(id).is_none()
        && state
            .scene_manager()
            .object(id)
            .is_some_and(|o| o.object_type() == "MolCoord")
}
