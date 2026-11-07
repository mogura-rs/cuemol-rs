use cuemol_desktop_core::{
    DesktopAppState, GuiFileOpenMetadata, GuiFileReaderOptions, GuiFileRendererTarget,
    GuiMapCenterPolicy, GuiMapType, GuiMoleculeReaderKind, GuiRendererSetup,
    GuiRendererSetupOption,
};
use std::collections::BTreeMap;

type Rects = BTreeMap<String, [f32; 4]>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Open,
    Cancel,
    ChooseCompanion,
    ClearCompanion,
}

#[derive(Debug)]
pub struct FileOptions {
    pub metadata: GuiFileOpenMetadata,
    pub setup: GuiRendererSetup,
    pub rects: Rects,
    options: Vec<GuiRendererSetupOption>,
    renderer_names: BTreeMap<String, String>,
    default_reader: GuiFileReaderOptions,
    selection_enabled: bool,
    selection: String,
    renderer_name_edited: bool,
    reader_open: bool,
}

impl FileOptions {
    pub fn new(name: &str, metadata: GuiFileOpenMetadata, state: &DesktopAppState) -> Self {
        let mut fallback = None;
        let scene = state.active_scene().unwrap_or_else(|| {
            let mut empty = DesktopAppState::new();
            let scene = empty.ensure_active_scene();
            fallback = Some(empty);
            scene
        });
        let state = fallback.as_ref().unwrap_or(state);
        let mut options = match metadata.target {
            GuiFileRendererTarget::Molecule => state
                .molecule_renderer_setup_options(scene)
                .unwrap_or_default(),
            GuiFileRendererTarget::DensityMap => state.density_map_renderer_setup_options(),
            GuiFileRendererTarget::Surface => state.surface_renderer_setup_options(),
        };
        if metadata.target == GuiFileRendererTarget::Molecule {
            options.insert(
                0,
                GuiRendererSetupOption {
                    name: "auto".into(),
                    description: "Automatic representations".into(),
                    preset: false,
                },
            );
        }
        let renderer_names = options
            .iter()
            .map(|option| {
                let name = state
                    .suggest_renderer_name(scene, &option.name)
                    .unwrap_or_else(|_| format!("{}1", option.name));
                (option.name.clone(), name)
            })
            .collect();
        let renderer_type = options
            .first()
            .map(|option| option.name.clone())
            .unwrap_or_default();
        let mut base = name.to_string();
        for suffix in [".gz", ".bz2", ".xz"] {
            if base.to_ascii_lowercase().ends_with(suffix) {
                base.truncate(base.len() - suffix.len());
                break;
            }
        }
        if let Some((stem, _)) = base.rsplit_once('.') {
            base = stem.to_string();
        }
        if base.is_empty() {
            base = "object".into();
        }
        let setup = GuiRendererSetup {
            object_name: state.suggest_object_name(scene, &base).unwrap_or(base),
            renderer_name: state
                .suggest_renderer_name(scene, &renderer_type)
                .unwrap_or_else(|_| format!("{renderer_type}1")),
            renderer_type,
            selection: None,
            recenter: metadata.target != GuiFileRendererTarget::DensityMap,
            map_center_policy: (metadata.target == GuiFileRendererTarget::DensityMap)
                .then_some(GuiMapCenterPolicy::Auto),
        };
        Self {
            default_reader: metadata.reader.clone(),
            metadata,
            setup,
            options,
            renderer_names,
            rects: BTreeMap::new(),
            selection_enabled: false,
            selection: String::new(),
            renderer_name_edited: false,
            reader_open: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, companion: Option<&str>) -> Option<Action> {
        self.rects.clear();
        let mut action = None;
        let previous_type = self.setup.renderer_type.clone();
        egui::Grid::new("file_open_renderer")
            .num_columns(2)
            .spacing([12., 8.])
            .show(ui, |ui| {
                ui.label("Object name");
                record(
                    &mut self.rects,
                    "File object name",
                    ui.text_edit_singleline(&mut self.setup.object_name),
                );
                ui.end_row();
                ui.label("Renderer type");
                let selected = self
                    .options
                    .iter()
                    .find(|item| item.name == self.setup.renderer_type)
                    .map(|item| item.description.as_str())
                    .unwrap_or(&self.setup.renderer_type);
                let combo = egui::ComboBox::from_id_salt("file_renderer_kind")
                    .selected_text(selected)
                    .width(260.)
                    .show_ui(ui, |ui| {
                        for option in &self.options {
                            let response = ui.selectable_value(
                                &mut self.setup.renderer_type,
                                option.name.clone(),
                                &option.description,
                            );
                            record(
                                &mut self.rects,
                                &format!("File renderer {}", option.name),
                                response,
                            );
                        }
                    });
                record(&mut self.rects, "File renderer type", combo.response);
                ui.end_row();
                ui.label("Renderer name");
                let response = ui.add_enabled(
                    self.setup.renderer_type != "auto",
                    egui::TextEdit::singleline(&mut self.setup.renderer_name),
                );
                self.renderer_name_edited |= response.changed();
                record(&mut self.rects, "File renderer name", response);
                ui.end_row();
                if self.metadata.target == GuiFileRendererTarget::Molecule {
                    record(
                        &mut self.rects,
                        "File selection enabled",
                        ui.checkbox(&mut self.selection_enabled, "Selection"),
                    );
                    record(
                        &mut self.rects,
                        "File selection",
                        ui.add_enabled(
                            self.selection_enabled,
                            egui::TextEdit::singleline(&mut self.selection),
                        ),
                    );
                    ui.end_row();
                }
            });
        if previous_type != self.setup.renderer_type && !self.renderer_name_edited {
            self.setup.renderer_name = self
                .renderer_names
                .get(&self.setup.renderer_type)
                .cloned()
                .unwrap_or_else(|| {
                    format!("{}1", self.setup.renderer_type.trim_start_matches('*'))
                });
        }
        self.setup.selection = self
            .selection_enabled
            .then(|| self.selection.trim().to_string());
        if let Some(policy) = &mut self.setup.map_center_policy {
            for (value, label) in [
                (GuiMapCenterPolicy::Auto, "Auto (from map type)"),
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
                    &format!("File {label}"),
                    ui.radio_value(policy, value, label),
                );
            }
        } else {
            record(
                &mut self.rects,
                "File recenter",
                ui.checkbox(&mut self.setup.recenter, "Recenter view"),
            );
        }
        if let Some(label) = self.metadata.reader.companion_label() {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{label}:"));
                ui.label(companion.unwrap_or("No file selected"));
                let choose = ui.button("Choose…");
                if choose.clicked() {
                    action = Some(Action::ChooseCompanion);
                }
                record(&mut self.rects, "File choose companion", choose);
                if companion.is_some() && ui.small_button("Clear").clicked() {
                    action = Some(Action::ClearCompanion);
                }
            });
            if !self.metadata.reader.companion_required() {
                ui.weak("Optional. Without coordinates, only the topology is loaded.");
            }
        }
        if matches!(
            self.metadata.reader,
            GuiFileReaderOptions::Molecule(_)
                | GuiFileReaderOptions::Ccp4(_)
                | GuiFileReaderOptions::Mtz(_)
        ) {
            ui.separator();
            let modified = self.metadata.reader != self.default_reader;
            let heading = if modified {
                "Reader options (modified)"
            } else {
                "Reader options"
            };
            let response = egui::CollapsingHeader::new(heading)
                .id_salt("file_reader_options")
                .open(Some(self.reader_open))
                .show(ui, |ui| {
                    reader_controls(ui, &mut self.metadata, &mut self.rects);
                    let reset = ui.button("Reset reader options");
                    if reset.clicked() {
                        self.metadata.reader = self.default_reader.clone();
                    }
                    record(&mut self.rects, "File reset reader", reset);
                });
            if response.header_response.clicked() {
                self.reader_open = !self.reader_open;
            }
            record(
                &mut self.rects,
                "File reader options",
                response.header_response,
            );
        }
        action
    }

    pub fn footer(&mut self, ui: &mut egui::Ui, companion: Option<&str>) -> Option<Action> {
        let mut action = None;
        ui.separator();
        let valid = !self.setup.object_name.trim().is_empty()
            && !self.setup.renderer_name.trim().is_empty()
            && (!self.selection_enabled || !self.selection.trim().is_empty())
            && (!self.metadata.reader.companion_required() || companion.is_some());
        ui.horizontal(|ui| {
            let open = ui.add_enabled(valid, egui::Button::new("Open"));
            if open.clicked() {
                action = Some(Action::Open);
            }
            record(&mut self.rects, "File options Open", open);
            let cancel = ui.button("Cancel");
            if cancel.clicked() {
                action = Some(Action::Cancel);
            }
            record(&mut self.rects, "File options Cancel", cancel);
        });
        action
    }
}

fn record(rects: &mut Rects, name: &str, response: egui::Response) {
    let r = response.rect;
    rects.insert(name.into(), [r.min.x, r.min.y, r.width(), r.height()]);
}
fn check(ui: &mut egui::Ui, rects: &mut Rects, label: &str, value: &mut bool) {
    record(rects, &format!("File {label}"), ui.checkbox(value, label));
}
fn reader_controls(ui: &mut egui::Ui, metadata: &mut GuiFileOpenMetadata, rects: &mut Rects) {
    match &mut metadata.reader {
        GuiFileReaderOptions::Molecule(options) => {
            check(
                ui,
                rects,
                "Ignore multiple models",
                &mut options.ignore_multiple_models,
            );
            check(
                ui,
                rects,
                "Ignore anisotropic U",
                &mut options.ignore_anisou,
            );
            check(
                ui,
                rects,
                "Ignore alternate conformations",
                &mut options.ignore_alt_conf,
            );
            if options.kind == GuiMoleculeReaderKind::Pdb {
                check(
                    ui,
                    rects,
                    "Use segment ID as chain",
                    &mut options.load_segid_as_chain,
                );
            }
            check(
                ui,
                rects,
                "Calculate secondary structure",
                &mut options.calculate_secondary_structure,
            );
            check(
                ui,
                rects,
                "Generate topology",
                &mut options.auto_generate_topology,
            );
        }
        GuiFileReaderOptions::Ccp4(options) => {
            if let Some(header) = metadata.map_header {
                ui.label(format!(
                    "Grid {} × {} × {} (mode {})",
                    header.counts[0], header.counts[1], header.counts[2], header.mode
                ));
                ui.label(format!(
                    "Estimated quantized storage: {:.1} MiB",
                    header.stored_voxels(options.subsample) as f64 / 1048576.
                ));
                if header.stored_voxels(options.subsample)
                    > cuemol_desktop_core::GuiMapHeader::LARGE_MAP_VOXELS
                {
                    ui.label(format!(
                        "Large map; suggested subsample: {}",
                        header.suggested_subsample()
                    ));
                }
            }
            ui.horizontal(|ui| {
                ui.label("Map type");
                let response = egui::ComboBox::from_id_salt("file_map_type")
                    .selected_text(options.map_type.as_str())
                    .show_ui(ui, |ui| {
                        for (kind, label) in [
                            (GuiMapType::Auto, "Auto (from header)"),
                            (GuiMapType::Xtal, "Crystallographic"),
                            (GuiMapType::Em, "Cryo-EM"),
                        ] {
                            record(
                                rects,
                                &format!("File map type {}", kind.as_str()),
                                ui.selectable_value(&mut options.map_type, kind, label),
                            );
                        }
                    });
                record(rects, "File map type", response.response);
            });
            ui.horizontal(|ui| {
                ui.label("Grid sampling stride");
                record(
                    rects,
                    "File grid stride",
                    ui.add(egui::DragValue::new(&mut options.subsample).range(1..=1024)),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Maximum stored voxels (0 = no limit)");
                record(
                    rects,
                    "File max voxels",
                    ui.add(
                        egui::DragValue::new(&mut options.max_voxels)
                            .range(0.0..=u32::MAX as f64)
                            .speed(1000.),
                    ),
                );
            });
            check(
                ui,
                rects,
                "Use header statistics",
                &mut options.use_header_stats,
            );
            check(ui, rects, "Normalize density", &mut options.normalize);
            ui.horizontal(|ui| {
                check(ui, rects, "Truncate minimum", &mut options.truncate_min);
                record(
                    rects,
                    "File truncate min",
                    ui.add_enabled(
                        options.truncate_min,
                        egui::DragValue::new(&mut options.min).speed(0.1),
                    ),
                );
            });
            ui.horizontal(|ui| {
                check(ui, rects, "Truncate maximum", &mut options.truncate_max);
                record(
                    rects,
                    "File truncate max",
                    ui.add_enabled(
                        options.truncate_max,
                        egui::DragValue::new(&mut options.max).speed(0.1),
                    ),
                );
            });
        }
        GuiFileReaderOptions::Mtz(options) => {
            ui.label(format!(
                "Resolution range: {:.3}–{:.3} Å",
                options.min_resolution, options.max_resolution
            ));
            let columns = options
                .columns
                .iter()
                .map(|column| (column.name.clone(), column.kind))
                .collect::<Vec<_>>();
            mtz_column(
                ui,
                rects,
                "Amplitude",
                &mut options.column_f,
                &columns,
                |kind| kind == 'F',
            );
            check(ui, rects, "Use phase", &mut options.use_phase);
            ui.add_enabled_ui(options.use_phase, |ui| {
                mtz_column(
                    ui,
                    rects,
                    "Phase",
                    &mut options.column_phi,
                    &columns,
                    |kind| kind == 'P',
                )
            });
            check(ui, rects, "Use weight", &mut options.use_weight);
            ui.add_enabled_ui(options.use_weight, |ui| {
                mtz_column(
                    ui,
                    rects,
                    "Weight",
                    &mut options.column_weight,
                    &columns,
                    |kind| kind == 'W',
                )
            });
            ui.horizontal(|ui| {
                ui.label("Maximum resolution (Å)");
                record(
                    rects,
                    "File resolution",
                    ui.add(egui::DragValue::new(&mut options.resolution).speed(0.1)),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Grid spacing");
                record(
                    rects,
                    "File grid spacing",
                    ui.add(
                        egui::DragValue::new(&mut options.grid_size)
                            .range(0.01..=10.)
                            .speed(0.01),
                    ),
                );
                if ui.small_button("Fine").clicked() {
                    options.grid_size = 0.25;
                }
                if ui.small_button("Coarse").clicked() {
                    options.grid_size = 0.333333;
                }
            });
        }
        _ => {}
    }
}
fn mtz_column(
    ui: &mut egui::Ui,
    rects: &mut Rects,
    label: &str,
    value: &mut String,
    columns: &[(String, char)],
    accepts: impl Fn(char) -> bool,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let response = egui::ComboBox::from_id_salt(("file_mtz_column", label))
            .selected_text(value.as_str())
            .show_ui(ui, |ui| {
                for (name, kind) in columns.iter().filter(|(_, kind)| accepts(*kind)) {
                    let _ = kind;
                    record(
                        rects,
                        &format!("File {label} {name}"),
                        ui.selectable_value(value, name.clone(), name),
                    );
                }
            });
        record(rects, &format!("File {label}"), response.response);
    });
}
