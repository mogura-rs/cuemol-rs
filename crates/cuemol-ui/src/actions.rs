use cuemol_command::console::actions::{self, LibraryFile, Record, RecordView};
use cuemol_desktop_core::DesktopAppState;
use std::collections::{BTreeMap, BTreeSet};

pub enum Intent {
    MoveTo(usize),
    Export {
        name: String,
        source: String,
    },
    Save {
        name: String,
        source: String,
    },
    Rename {
        from: String,
        to: String,
    },
    Delete(String),
    Refresh,
    Import,
    Run {
        source: String,
        macro_name: Option<String>,
        label: String,
    },
}

#[derive(Debug, Default)]
pub struct ActionsPanel {
    pub files: Vec<LibraryFile>,
    selected: BTreeSet<usize>,
    anchor: Option<usize>,
    expanded: BTreeSet<usize>,
    inspect: Option<RecordView>,
    entry: Option<(bool, String, Option<String>)>,
    save: Option<Save>,
    rename: Option<(String, String)>,
    pub error: String,
    pub rects: BTreeMap<String, [f32; 4]>,
}

#[derive(Debug)]
struct Save {
    name: String,
    records: Vec<Record>,
    source: Option<String>,
    as_macro: bool,
}

impl ActionsPanel {
    pub fn with_files(files: Vec<LibraryFile>) -> Self {
        Self {
            files,
            ..Self::default()
        }
    }
    fn mark(&mut self, name: &str, response: &egui::Response) {
        let rect = response.interact_rect;
        self.rects.insert(
            name.into(),
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
        );
    }

    fn select(&mut self, id: usize, ids: &[usize], modifiers: egui::Modifiers) {
        if modifiers.shift {
            let a = self
                .anchor
                .and_then(|id| ids.iter().position(|value| *value == id));
            let b = ids.iter().position(|value| *value == id);
            if !modifiers.command {
                self.selected.clear();
            }
            if let (Some(a), Some(b)) = (a, b) {
                self.selected
                    .extend(ids[a.min(b)..=a.max(b)].iter().copied());
            } else {
                self.selected.insert(id);
            }
        } else {
            if !modifiers.command {
                self.selected.clear();
            }
            if !self.selected.insert(id) {
                self.selected.remove(&id);
            }
            self.anchor = Some(id);
        }
    }

    pub fn history(
        &mut self,
        ui: &mut egui::Ui,
        done: &[Record],
        future: &[Record],
        busy: bool,
    ) -> Option<Intent> {
        self.rects.clear();
        let all: Vec<_> = done.iter().chain(future.iter().rev()).collect();
        let ids: Vec<_> = std::iter::once(0)
            .chain(all.iter().map(|record| record.id()))
            .collect();
        self.selected.retain(|id| ids.contains(id));
        self.expanded.retain(|id| ids.contains(id));
        let selected: Vec<Record> = all
            .iter()
            .filter(|record| self.selected.contains(&record.id()))
            .map(|record| (*record).clone())
            .collect();
        let mut intent = None;
        ui.horizontal_wrapped(|ui| {
            for (label, enabled, action) in [
                (
                    "Undo",
                    !done.is_empty(),
                    Intent::MoveTo(done.len().saturating_sub(1)),
                ),
                ("Redo", !future.is_empty(), Intent::MoveTo(done.len() + 1)),
            ] {
                let response = ui.add_enabled(enabled && !busy, egui::Button::new(label));
                self.mark(&format!("History {label}"), &response);
                if response.clicked() {
                    intent = Some(action);
                }
            }
            let response = ui.add_enabled(
                !busy && self.selected.len() == 1,
                egui::Button::new("Go to step"),
            );
            self.mark("History Go to step", &response);
            if response.clicked() {
                if let Some(index) = ids.iter().position(|id| self.selected.contains(id)) {
                    intent = Some(Intent::MoveTo(index));
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            let enabled = !selected.is_empty() && !busy;
            for label in ["Copy commands", "Save QAX…", "Save to Actions…"] {
                let response = ui.add_enabled(enabled, egui::Button::new(label));
                self.mark(&format!("History {label}"), &response);
                if response.clicked() {
                    match label {
                        "Copy commands" => ui.ctx().copy_text(actions::commands(&selected)),
                        "Save QAX…" => {
                            intent = Some(Intent::Export {
                                name: "actions.qax".into(),
                                source: actions::export(&selected),
                            })
                        }
                        _ => {
                            self.save = Some(Save {
                                name: "My action".into(),
                                records: selected.clone(),
                                source: None,
                                as_macro: true,
                            })
                        }
                    }
                }
            }
        });
        for (index, id) in ids.iter().enumerate() {
            let row = all.get(index.wrapping_sub(1));
            let label = row
                .map(|record| record.label())
                .unwrap_or("Initial state".into());
            let entries = row.map(|record| record.entry_count()).unwrap_or(0);
            let current = index == done.len();
            let response = ui
                .horizontal(|ui| {
                    if entries > 1 {
                        let expand = crate::shell::icon_button(
                            ui,
                            if self.expanded.contains(id) {
                                crate::shell::Icon::Down
                            } else {
                                crate::shell::Icon::Right
                            },
                            "Expand operations",
                            false,
                            egui::vec2(18., 18.),
                        );
                        self.mark(&format!("History expand {index}"), &expand);
                        if expand.clicked() {
                            if !self.expanded.insert(*id) {
                                self.expanded.remove(id);
                            }
                        }
                    } else {
                        ui.add_space(18.);
                    }
                    let text = egui::RichText::new(format!(
                        "{}{}",
                        if current { "● " } else { "" },
                        label
                    ));
                    ui.selectable_label(
                        self.selected.contains(id),
                        if index > done.len() {
                            text.weak()
                        } else {
                            text
                        },
                    )
                })
                .inner;
            self.mark(&format!("History step {index}"), &response);
            if response.clicked() {
                self.select(*id, &ids, ui.input(|i| i.modifiers));
            }
            if response.double_clicked() {
                self.inspect = row.map(|record| record.view());
            }
            response.context_menu(|ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("Go to step"))
                    .clicked()
                {
                    intent = Some(Intent::MoveTo(index));
                    ui.close();
                }
                if ui.button("Inspect").clicked() {
                    self.inspect = row.map(|record| record.view());
                    ui.close();
                }
            });
            if self.expanded.contains(id) {
                if let Some(record) = row {
                    for (entry_index, entry) in record.view().entries.into_iter().enumerate() {
                        let response = ui
                            .indent((*id, entry_index), |ui| {
                                ui.selectable_label(false, &entry.command)
                            })
                            .inner;
                        if response.clicked() {
                            self.select(*id, &ids, ui.input(|i| i.modifiers));
                            let mut view = record.view();
                            view.entries = vec![entry];
                            self.inspect = Some(view);
                        }
                    }
                }
            }
        }
        intent
    }

    pub fn library(
        &mut self,
        ui: &mut egui::Ui,
        target_names: &[String],
        busy: bool,
    ) -> Option<Intent> {
        let mut intent = None;
        let files = std::mem::take(&mut self.files);
        let chosen = self.entry.as_ref().and_then(|(bundled, file, mac)| {
            files
                .iter()
                .find(|value| value.bundled == *bundled && &value.name == file)
                .map(|value| (value, mac.clone()))
        });
        let macro_info = chosen.as_ref().and_then(|(file, mac)| {
            file.macros
                .iter()
                .find(|value| Some(&value.name) == mac.as_ref())
        });
        let can_run = chosen.as_ref().is_some_and(|(file, mac)| {
            file.error.is_none()
                && if mac.is_some() {
                    macro_info.is_some_and(|value| {
                        value.error.is_none()
                            && (value.target_type.is_none() || !target_names.is_empty())
                    })
                } else {
                    file.action_count > 0
                }
        });
        ui.horizontal_wrapped(|ui| {
            let run = ui.add_enabled(can_run && !busy, egui::Button::new("Run"));
            self.mark("Actions Run", &run);
            if run.clicked() {
                if let Some((file, mac)) = &chosen {
                    intent = Some(Intent::Run {
                        source: file.source.clone(),
                        macro_name: mac.clone(),
                        label: macro_info
                            .map(|value| value.label.clone())
                            .unwrap_or(file.name.clone()),
                    });
                }
            }
            for (label, action) in [("Refresh", Intent::Refresh), ("Import…", Intent::Import)] {
                let response = ui.button(label);
                self.mark(&format!("Actions {label}"), &response);
                if response.clicked() {
                    intent = Some(action);
                }
            }
        });
        if target_names.is_empty() {
            ui.weak("Target: select an object in the scene tree");
        } else {
            ui.label(format!("Target: {}", target_names.join(", ")));
        }
        for bundled in [true, false] {
            egui::CollapsingHeader::new(if bundled { "Presets" } else { "User Actions" })
                .default_open(true)
                .show(ui, |ui| {
                    for file in files.iter().filter(|file| file.bundled == bundled) {
                        if file.action_count > 0 || file.error.is_some() {
                            let key = (file.bundled, file.name.clone(), None);
                            let response =
                                ui.selectable_label(self.entry.as_ref() == Some(&key), &file.name);
                            self.mark(&format!("Actions file {}", file.name), &response);
                            if response.clicked() {
                                self.entry = Some(key);
                            }
                            if let Some(error) = &file.error {
                                response.on_hover_text(error);
                            }
                        }
                        for mac in &file.macros {
                            let key = (file.bundled, file.name.clone(), Some(mac.name.clone()));
                            let response = ui
                                .selectable_label(self.entry.as_ref() == Some(&key), &mac.label)
                                .on_hover_text(mac.error.as_ref().unwrap_or(&mac.description));
                            self.mark(&format!("Actions preset {}", mac.name), &response);
                            if response.clicked() {
                                self.entry = Some(key);
                            }
                        }
                    }
                });
        }
        if let Some((file, mac)) = chosen {
            if let Some(error) = &file.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            } else if let Some(info) = file
                .macros
                .iter()
                .find(|value| Some(&value.name) == mac.as_ref())
            {
                ui.label(&info.description);
                if let Some(error) = &info.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            }
            ui.horizontal_wrapped(|ui| {
                if file.bundled {
                    if ui.button("Copy to User Actions…").clicked() {
                        match mac
                            .as_deref()
                            .map(|name| file.copy_macro(name))
                            .unwrap_or(Ok(file.source.clone()))
                        {
                            Ok(source) => {
                                self.save = Some(Save {
                                    name: mac.clone().unwrap_or(file.name.clone()),
                                    records: Vec::new(),
                                    source: Some(source),
                                    as_macro: false,
                                })
                            }
                            Err(error) => self.error = error,
                        }
                    }
                } else {
                    if ui.button("Rename…").clicked() {
                        self.rename = Some((file.name.clone(), file.name.clone()));
                    }
                    ui.menu_button("Delete…", |ui| {
                        if ui.button("Delete this user action").clicked() {
                            intent = Some(Intent::Delete(file.name.clone()));
                            self.entry = None;
                            ui.close();
                        }
                    });
                }
                if ui.button("Export…").clicked() {
                    intent = Some(Intent::Export {
                        name: format!("{}.qax", file.name),
                        source: file.source.clone(),
                    });
                }
            });
        }
        self.files = files;
        intent
    }

    pub fn inspector(&mut self, ctx: &egui::Context) {
        if let Some(view) = &self.inspect {
            let mut open = true;
            egui::SidePanel::right("action_inspector")
                .default_width(280.)
                .width_range(180.0..=620.)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Inspector");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::shell::icon_button(
                                ui,
                                crate::shell::Icon::Close,
                                "Close inspector",
                                false,
                                egui::vec2(20., 20.),
                            )
                            .clicked()
                            {
                                open = false;
                            }
                        });
                    });
                    ui.separator();
                    ui.heading(&view.label);
                    ui.weak(format!("Source: {}", view.source));
                    if view.entries.is_empty() {
                        ui.label("No operation was recorded for this edit.");
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for entry in &view.entries {
                            ui.monospace(&entry.command);
                            for (key, value) in &entry.args {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(key);
                                    ui.monospace(value);
                                });
                            }
                            ui.separator();
                        }
                    });
                });
            if !open {
                self.inspect = None;
            }
        }
    }

    pub fn dialogs(&mut self, ctx: &egui::Context, state: &DesktopAppState) -> Option<Intent> {
        let mut intent = None;
        if let Some(save) = &mut self.save {
            let mut open = true;
            egui::Window::new("Save to Actions").id(egui::Id::new("action_save")).open(&mut open).show(ctx, |ui| {
                ui.label("Name");
                let field = ui.text_edit_singleline(&mut save.name);
                let rect = field.interact_rect;
                self.rects.insert("Actions name".into(), [rect.min.x, rect.min.y, rect.width(), rect.height()]);
                if save.source.is_none() {
                    ui.checkbox(&mut save.as_macro, "Reusable macro for another object");
                    ui.weak("A macro can refer to one object and renderers created by the selected steps.");
                }
                let save_button = ui.button("Save");
                let rect = save_button.interact_rect;
                self.rects.insert("Actions Save".into(), [rect.min.x, rect.min.y, rect.width(), rect.height()]);
                if save_button.clicked() {
                    let source = save.source.clone().map(Ok).unwrap_or_else(|| if save.as_macro { actions::make_macro(&save.records, &save.name, &save.name, state) } else { Ok(actions::export(&save.records)) });
                    match actions::validate_file_name(&save.name).and(source) {
                        Ok(source) => intent = Some(Intent::Save { name: save.name.clone(), source }),
                        Err(error) => self.error = error,
                    }
                }
                if !self.error.is_empty() { ui.colored_label(ui.visuals().error_fg_color, &self.error); }
            });
            if !open {
                self.save = None;
            }
        }
        if let Some((old, name)) = &mut self.rename {
            let mut open = true;
            egui::Window::new("Rename action")
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.text_edit_singleline(name);
                    if ui.button("Rename").clicked() {
                        intent = Some(Intent::Rename {
                            from: old.clone(),
                            to: name.clone(),
                        });
                    }
                    if !self.error.is_empty() {
                        ui.colored_label(ui.visuals().error_fg_color, &self.error);
                    }
                });
            if !open {
                self.rename = None;
            }
        }
        intent
    }

    pub fn saved(&mut self) {
        self.save = None;
        self.rename = None;
        self.error.clear();
    }
}
