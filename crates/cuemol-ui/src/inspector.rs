use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorTarget {
    pub kind: String,
    pub id: Option<u64>,
}

#[derive(Debug)]
struct Property {
    key: String,
    value: String,
    kind: String,
    category: String,
    readonly: bool,
    has_default: bool,
    is_default: bool,
    default_value: Option<String>,
    allowed: Vec<String>,
    rgba: Option<[u8; 4]>,
}

#[derive(Debug, Clone)]
struct ColorDraft {
    key: String,
    rgba: egui::Color32,
}

#[derive(Debug, Default)]
pub struct Inspector {
    pub visible: bool,
    pub rects: BTreeMap<String, [f32; 4]>,
    pub target: Option<InspectorTarget>,
    revision: u64,
    rows: Vec<Property>,
    committed: BTreeMap<String, String>,
    filter: String,
    tab: usize,
    error: String,
    color_draft: Option<ColorDraft>,
}

impl Inspector {
    pub fn inspect(
        &mut self,
        state: &mut DesktopAppState,
        camera: &mut Camera,
        target: Option<InspectorTarget>,
        revision: u64,
    ) {
        if target == self.target && revision == self.revision {
            return;
        }
        if target != self.target {
            self.visible = target.is_some();
            self.filter.clear();
            self.color_draft = None;
            self.tab = usize::from(target.as_ref().is_some_and(|target| target.kind == "view"));
        }
        self.target = target;
        self.revision = revision;
        let Some(target) = &self.target else {
            self.rows.clear();
            return;
        };
        match cuemol_command::call_tool(
            state,
            camera,
            "list_node_props",
            &json!({"nodeType":target.kind,"nodeId":target.id}),
        ) {
            Ok(data) => {
                self.rows = data["properties"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|row| Property {
                        key: row["key"].as_str().unwrap_or("").into(),
                        value: row["value"]
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| row["value"].to_string()),
                        kind: row["type"].as_str().unwrap_or("string").into(),
                        category: row["category"].as_str().unwrap_or("Properties").into(),
                        readonly: row["readonly"] == true,
                        has_default: row["hasdefault"] == true,
                        is_default: row["isdefault"] == true,
                        default_value: row["defaultValue"].as_str().map(str::to_string),
                        allowed: row["allowed"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect(),
                        rgba: row["rgba"].as_array().and_then(|values| {
                            let values = values
                                .iter()
                                .map(|v| v.as_u64().and_then(|v| u8::try_from(v).ok()))
                                .collect::<Option<Vec<_>>>()?;
                            values.try_into().ok()
                        }),
                    })
                    .collect();
                self.committed = self
                    .rows
                    .iter()
                    .map(|row| (row.key.clone(), row.value.clone()))
                    .collect();
                self.error.clear();
            }
            Err(error) => {
                self.rows.clear();
                self.error = error.to_string();
            }
        }
    }

    pub fn panel(&mut self, ctx: &egui::Context) -> Vec<(&'static str, Value)> {
        self.rects.clear();
        let mut changes = Vec::new();
        if !self.visible || self.target.is_none() {
            return changes;
        }
        let target = self.target.clone().unwrap();
        egui::SidePanel::right("inspector").default_width(300.).width_range(240.0..=600.).resizable(true).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Inspector");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close = ui.small_button("×").on_hover_text("Close Inspector");
                    record(&mut self.rects, "Close Inspector", close.rect);
                    if close.clicked() { self.visible = false; }
                });
            });
            ui.separator();
            let name = self.rows.iter().find(|row| row.key == "name").map(|row| row.value.as_str()).unwrap_or(&target.kind);
            ui.label(egui::RichText::new(name).strong());
            ui.weak(&target.kind);
            ui.horizontal(|ui| {
                record(&mut self.rects, "Properties tab", ui.selectable_value(&mut self.tab, 0, "Properties").rect);
                record(&mut self.rects, "Generic tab", ui.selectable_value(&mut self.tab, 1, "Generic").rect);
            });
            let filter = ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Filter properties").desired_width(f32::INFINITY));
            record(&mut self.rects, "Filter properties", filter.rect);
            let modified = self.rows.iter().filter(|row| row.has_default && !row.is_default && !row.readonly).count();
            let reset = ui.add_enabled(modified > 0, egui::Button::new("Reset all to default"));
            record(&mut self.rects, "Reset all to default", reset.rect);
            if reset.clicked() {
                changes.push(("reset_node_props", json!({"nodeType":target.kind,"nodeId":target.id})));
            }
            if !self.error.is_empty() { ui.colored_label(egui::Color32::LIGHT_RED, &self.error); }
            let filter = self.filter.to_lowercase();
            egui::ScrollArea::vertical().id_salt("inspector-fields").show(ui, |ui| {
                let mut groups = self.rows.iter().filter(|row| filter.is_empty() || row.key.to_lowercase().contains(&filter))
                    .map(|row| if self.tab == 0 { row.category.clone() } else { String::new() }).collect::<Vec<_>>();
                groups.sort_by_key(|name| (match name.as_str() { "Common" => 0, "Appearance" => 1, "Geometry" => 2, "Identity" => 4, _ => 3 }, name.clone()));
                groups.dedup();
                for group in groups {
                    let mut fields = |ui: &mut egui::Ui| {
                        for row in &mut self.rows {
                            if (self.tab == 0 && row.category != group) || (!filter.is_empty() && !row.key.to_lowercase().contains(&filter)) { continue; }
                            ui.push_id(egui::Id::new(&row.key), |ui| {
                                let mut changed = false;
                                let label_width = (ui.available_width() * 0.36).min(105.);
                                let field_width = (ui.available_width() - label_width - 24. - 2. * ui.spacing().item_spacing.x).max(32.);
                                ui.horizontal(|ui| {
                                    let label = if self.tab == 0 { row.key.rsplit('.').next().unwrap() } else { row.key.as_str() };
                                    let label = ui.add_sized([label_width, 24.], egui::Label::new(label).wrap());
                                    label.on_hover_text(format!("{} ({})", row.key, row.kind));
                                    ui.add_enabled_ui(!row.readonly, |ui| {
                                        ui.allocate_ui_with_layout(egui::vec2(field_width, 24.), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                            ui.set_width(field_width);
                                            changed = edit_field(ui, row, &mut self.rects, &mut self.color_draft);
                                        });
                                    });
                                    if row.has_default && !row.readonly {
                                        let reset = ui.add_enabled(!row.is_default, egui::Button::new("↺").small())
                                            .on_hover_text(format!("Reset {} to default{}", row.key, row.default_value.as_ref().map_or(String::new(), |value| format!(" ({value})"))));
                                        if ui.is_rect_visible(reset.rect) { record(&mut self.rects, &format!("Reset {}", row.key), reset.rect); }
                                        if reset.clicked() {
                                            changes.push(("reset_node_props", json!({"nodeType":target.kind,"nodeId":target.id,"props":[row.key]})));
                                        }
                                    } else { ui.add_space(22.); }
                                });
                                if changed && self.committed.get(&row.key) != Some(&row.value) {
                                    changes.push(("set_node_prop", json!({"nodeType":target.kind,"nodeId":target.id,"prop":row.key,"value":row.value,"cascadeGroupVisibility":self.tab == 0})));
                                }
                            });
                        }
                    };
                    if self.tab == 0 && filter.is_empty() {
                        egui::CollapsingHeader::new(&group).default_open(matches!(group.as_str(), "Common" | "Appearance" | "Geometry")).show(ui, fields);
                    } else { fields(ui); }
                }
            });
        });
        if let Some(draft) = &mut self.color_draft {
            let mut open = true;
            let mut apply = false;
            let mut cancel = false;
            egui::Window::new(format!("Color: {}", draft.key))
                .id(egui::Id::new("inspector-color"))
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.color_edit_button_srgba(&mut draft.rgba);
                    let [r, g, b, a] = draft.rgba.to_srgba_unmultiplied();
                    ui.label(format!("#{r:02x}{g:02x}{b:02x}{a:02x}"));
                    ui.horizontal(|ui| {
                        let button = ui.button("Apply");
                        record(&mut self.rects, "Apply color", button.rect);
                        apply = button.clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                });
            if apply {
                let [r, g, b, a] = draft.rgba.to_srgba_unmultiplied();
                let value = format!("rgba({r},{g},{b},{})", f32::from(a) / 255.);
                changes.push(("set_node_prop", json!({"nodeType":target.kind,"nodeId":target.id,"prop":draft.key,"value":value})));
            }
            if !open || cancel || apply {
                self.color_draft = None;
            }
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

fn edit_field(
    ui: &mut egui::Ui,
    row: &mut Property,
    rects: &mut BTreeMap<String, [f32; 4]>,
    color: &mut Option<ColorDraft>,
) -> bool {
    let (response, changed) = if !row.allowed.is_empty() {
        let mut changed = false;
        let response = egui::ComboBox::from_id_salt("value")
            .width(ui.available_width())
            .selected_text(&row.value)
            .show_ui(ui, |ui| {
                for value in &row.allowed {
                    let option = ui.selectable_value(&mut row.value, value.clone(), value);
                    record(rects, &format!("{} option {value}", row.key), option.rect);
                    changed |= option.changed();
                }
            })
            .response;
        (response, changed)
    } else if row.kind == "boolean" {
        let mut flag = row.value == "true";
        let response = ui.checkbox(&mut flag, "");
        let changed = response.changed();
        if changed {
            row.value = flag.to_string();
        }
        (response, changed)
    } else if matches!(row.kind.as_str(), "integer" | "real") && row.value.parse::<f64>().is_ok() {
        let mut value = row.value.parse::<f64>().unwrap();
        let mut editor = egui::DragValue::new(&mut value)
            .speed(if row.kind == "integer" { 1. } else { 0.01 })
            .update_while_editing(false);
        if row.kind == "integer" {
            editor = editor.max_decimals(0);
        }
        let response = ui.add_sized([ui.available_width(), 22.], editor);
        if response.changed() {
            row.value = value.to_string();
        }
        let changed = (response.changed() && !response.dragged()) || response.drag_stopped();
        (response, changed)
    } else {
        if row.kind == "color" {
            if let Some(rgba) = row.rgba {
                let rgba =
                    egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
                let response = ui
                    .add(
                        egui::Button::new(" ")
                            .fill(rgba)
                            .min_size(egui::vec2(22., 22.)),
                    )
                    .on_hover_text("Choose color");
                record(rects, &format!("Choose color {}", row.key), response.rect);
                if response.clicked() {
                    *color = Some(ColorDraft {
                        key: row.key.clone(),
                        rgba,
                    });
                }
            }
        }
        let response =
            ui.add(egui::TextEdit::singleline(&mut row.value).desired_width(ui.available_width()));
        let changed = response.lost_focus()
            || (response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
        (response, changed)
    };
    if ui.is_rect_visible(response.rect) {
        record(rects, &row.key, response.rect);
    }
    changed && !row.readonly
}
