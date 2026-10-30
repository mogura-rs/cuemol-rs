use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[path = "inspector_fonts.rs"]
mod fonts;

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
    structured_choices: Option<Vec<String>>,
    structured_value: Option<String>,
    rgba: Option<[u8; 4]>,
    schema: Option<Value>,
}

impl Property {
    fn displayed_value(&self, structured: bool) -> &str {
        self.structured_value
            .as_deref()
            .filter(|_| structured)
            .unwrap_or(&self.value)
    }
}

#[derive(Debug, Clone)]
struct ColorDraft {
    key: String,
    rgba: egui::Color32,
}

#[derive(Debug, Default)]
struct NumericEdit {
    id: Option<egui::Id>,
    value: Option<f64>,
    dirty: bool,
    optional_value: Option<f64>,
}

#[derive(Debug)]
struct SelectionDraft {
    key: String,
    original: String,
    value: String,
    anchor: egui::Rect,
    layer: egui::LayerId,
    fresh: bool,
    close_requested: bool,
}

#[derive(Debug, Default)]
pub struct Inspector {
    pub visible: bool,
    pub rects: BTreeMap<String, [f32; 4]>,
    pub target: Option<InspectorTarget>,
    revision: u64,
    camera: Option<Camera>,
    view_size: Option<(u32, u32)>,
    context: (Option<u64>, Option<u64>),
    sections: Vec<Value>,
    rows: Vec<Property>,
    generic_order: Vec<usize>,
    structured_order: Vec<usize>,
    committed: BTreeMap<String, String>,
    filter: String,
    tab: usize,
    error: String,
    color_draft: Option<ColorDraft>,
    numeric_edits: BTreeMap<String, NumericEdit>,
    local_toggles: BTreeMap<String, bool>,
    font_generation: u32,
    font_listener: bool,
    font_previews: fonts::FontPreviews,
    selection_draft: Option<SelectionDraft>,
    selection_builder: crate::selection::Builder,
    selection_operands: BTreeMap<String, cuemol_command::selection::Draft>,
}

impl Inspector {
    fn clear_properties(&mut self) {
        self.rows.clear();
        self.sections.clear();
        self.generic_order.clear();
        self.structured_order.clear();
        self.committed.clear();
        self.numeric_edits.clear();
        self.local_toggles.clear();
        self.color_draft = None;
        self.selection_draft = None;
        self.selection_operands.clear();
    }

    pub fn inspect(
        &mut self,
        state: &mut DesktopAppState,
        camera: &mut Camera,
        target: Option<InspectorTarget>,
        revision: u64,
    ) {
        let view = target.as_ref().is_some_and(|target| target.kind == "view");
        let font_generation = cuemol_desktop_core::font_catalog::generation();
        let active_view = state
            .active_scene()
            .and_then(|id| state.scene_manager().scene(id))
            .and_then(|scene| scene.active_view_uid());
        let context = (
            state.active_scene().map(|id| id.get()),
            active_view.map(|id| id.get()),
        );
        let view_size = if view {
            active_view
                .and_then(|id| state.command_manager().view(id))
                .map(|view| (view.width(), view.height()))
        } else {
            None
        };
        if target == self.target
            && revision == self.revision
            && context == self.context
            && view_size == self.view_size
            && font_generation == self.font_generation
            && (!view || self.camera.as_ref() == Some(camera))
        {
            return;
        }
        let drafts: BTreeMap<_, _> = if target == self.target && context == self.context {
            self.rows
                .iter()
                .filter(|row| {
                    row.allowed.is_empty()
                        && !matches!(row.kind.as_str(), "integer" | "real" | "boolean" | "preset")
                        && self.committed.get(&row.key) != Some(&row.value)
                })
                .map(|row| {
                    (
                        row.key.clone(),
                        (self.committed.get(&row.key).cloned(), row.value.clone()),
                    )
                })
                .collect()
        } else {
            BTreeMap::new()
        };
        if target != self.target || context != self.context {
            self.numeric_edits.clear();
            self.local_toggles.clear();
            self.selection_draft = None;
            self.selection_builder = crate::selection::Builder::default();
            self.selection_operands.clear();
        }
        if target != self.target {
            self.visible = target.is_some();
            self.filter.clear();
            self.color_draft = None;
            self.tab = usize::from(target.as_ref().is_some_and(|target| target.kind == "view"));
        }
        self.target = target;
        self.revision = revision;
        self.context = context;
        self.view_size = view_size;
        self.camera = view.then(|| camera.clone());
        if font_generation != self.font_generation {
            self.font_generation = font_generation;
            self.font_previews.clear();
        }
        let Some(target) = &self.target else {
            self.clear_properties();
            self.error.clear();
            return;
        };
        match cuemol_command::call_tool(
            state,
            camera,
            "list_node_props",
            &json!({"nodeType":target.kind,"nodeId":target.id}),
        ) {
            Ok(data) => {
                self.sections = data["sections"].as_array().cloned().unwrap_or_default();
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
                        structured_choices: None,
                        structured_value: None,
                        rgba: row["rgba"].as_array().and_then(|values| {
                            let values = values
                                .iter()
                                .map(|v| v.as_u64().and_then(|v| u8::try_from(v).ok()))
                                .collect::<Option<Vec<_>>>()?;
                            values.try_into().ok()
                        }),
                        schema: None,
                    })
                    .collect();
                for section in &self.sections {
                    for definition in section["rows"].as_array().into_iter().flatten() {
                        let key = definition["key"].as_str().unwrap_or("");
                        let mut schema = definition.clone();
                        schema["category"] = section["title"].clone();
                        if let Some(row) = self.rows.iter_mut().find(|row| row.key == key) {
                            row.structured_value = schema["display"].as_str().map(str::to_string);
                            if schema["blankNegative"] == true {
                                row.structured_value = Some(
                                    if row.value.parse::<f64>().is_ok_and(|value| value < 0.) {
                                        String::new()
                                    } else {
                                        row.value.clone()
                                    },
                                );
                            }
                            row.structured_choices = schema["choices"].as_array().map(|choices| {
                                choices
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_string)
                                    .collect()
                            });
                            row.schema = Some(schema);
                        } else if definition["options"].is_array() || definition["kind"] == "toggle"
                        {
                            let empty = Vec::new();
                            let options = definition["options"].as_array().unwrap_or(&empty);
                            let selected = options.iter().find(|option| {
                                option["patch"].as_object().is_some_and(|patch| {
                                    patch.iter().all(|(key, value)| {
                                        self.rows.iter().find(|row| &row.key == key).is_some_and(
                                            |row| {
                                                equivalent(&row.value, value.as_str().unwrap_or(""))
                                            },
                                        )
                                    })
                                })
                            });
                            let toggle = definition["kind"] == "toggle";
                            let reset: Vec<_> = definition["reset"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_str)
                                .filter_map(|key| self.rows.iter().find(|row| row.key == key))
                                .collect();
                            let has_default = reset.iter().any(|row| row.has_default);
                            let is_default = reset.iter().all(|row| row.is_default);
                            let display = if definition["local"] == true
                                && self.local_toggles.get(key) == Some(&true)
                            {
                                Some("true")
                            } else {
                                definition["display"].as_str()
                            };
                            self.rows.push(Property {
                                key: key.into(),
                                value: display
                                    .or_else(|| selected.and_then(|value| value["label"].as_str()))
                                    .unwrap_or("Custom")
                                    .into(),
                                kind: if toggle { "toggle" } else { "preset" }.into(),
                                category: section["title"].as_str().unwrap_or("").into(),
                                readonly: false,
                                has_default,
                                is_default,
                                default_value: None,
                                allowed: options
                                    .iter()
                                    .filter_map(|option| {
                                        option["label"].as_str().map(str::to_string)
                                    })
                                    .collect(),
                                rgba: None,
                                structured_choices: None,
                                structured_value: None,
                                schema: Some(schema),
                            });
                        }
                    }
                }
                self.generic_order = self
                    .rows
                    .iter()
                    .enumerate()
                    .filter_map(|(index, row)| {
                        (!matches!(row.kind.as_str(), "preset" | "toggle")).then_some(index)
                    })
                    .collect();
                self.structured_order = self
                    .sections
                    .iter()
                    .flat_map(|section| section["rows"].as_array().into_iter().flatten())
                    .filter_map(|definition| {
                        self.rows
                            .iter()
                            .position(|row| definition["key"] == row.key)
                    })
                    .collect();
                self.committed = self
                    .rows
                    .iter()
                    .map(|row| (row.key.clone(), row.value.clone()))
                    .collect();
                if self
                    .selection_draft
                    .as_ref()
                    .is_some_and(|draft| self.committed.get(&draft.key) != Some(&draft.original))
                {
                    self.selection_draft = None;
                }
                for row in &mut self.rows {
                    if let Some((previous, draft)) = drafts.get(&row.key) {
                        if previous.as_ref() == Some(&row.value) {
                            row.value.clone_from(draft);
                        }
                    }
                }
                self.error.clear();
            }
            Err(error) => {
                self.clear_properties();
                self.error = error.to_string();
            }
        }
    }

    pub fn panel(
        &mut self,
        ctx: &egui::Context,
        state: &DesktopAppState,
    ) -> Vec<(&'static str, Value)> {
        self.rects.clear();
        let mut changes = Vec::new();
        if !self.visible || self.target.is_none() {
            return changes;
        }
        if !self.font_listener
            && self.rows.iter().any(|row| {
                row.schema
                    .as_ref()
                    .is_some_and(|schema| schema["font"] == true)
            })
        {
            self.font_listener = true;
            let ctx = ctx.clone();
            cuemol_desktop_core::font_catalog::on_change(move || ctx.request_repaint());
        }
        let target = self.target.clone().unwrap();
        egui::SidePanel::right("inspector").default_width(300.).width_range(240.0..=600.).resizable(true).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Inspector");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close = ui.small_button("×").on_hover_text("Close Inspector");
                    record(&mut self.rects, "Close Inspector", close.rect);
                    if close.clicked() { self.visible = false; self.selection_draft = None; }
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
                let structured = self.tab == 0 && !self.sections.is_empty();
                let mut groups = self.rows.iter().filter(|row| row.kind != "preset" && (filter.is_empty() || row.key.to_lowercase().contains(&filter)))
                    .map(|row| if self.tab == 0 { row.category.clone() } else { String::new() }).collect::<Vec<_>>();
                groups.sort_by_key(|name| (match name.as_str() { "Common" => 0, "Appearance" => 1, "Geometry" => 2, "Identity" => 4, _ => 3 }, name.clone()));
                groups.dedup();
                if structured { groups = self.sections.iter().filter_map(|section| section["title"].as_str().map(str::to_string)).collect(); }
                for group in groups {
                    let mut fields = |ui: &mut egui::Ui| {
                        for &index in if structured { &self.structured_order } else { &self.generic_order } {
                            let row = &mut self.rows[index];
                            let schema = row.schema.as_ref().filter(|_| structured);
                            let category = schema.and_then(|schema| schema["category"].as_str()).unwrap_or(&row.category);
                            if (structured && schema.is_none()) || (!structured && row.kind == "preset")
                                || (self.tab == 0 && category != group) || (!filter.is_empty() && !row.key.to_lowercase().contains(&filter)) { continue; }
                            let editable = !row.readonly || schema.is_some_and(|schema| schema["editable"] == true);
                            let enabled = schema.is_none_or(|schema| schema["disabled"] != true)
                                && schema.and_then(|schema| schema["enabledWhen"].as_str()).is_none_or(|key| self.committed.get(key).is_some_and(|value| value == "true"));
                            let label = schema.and_then(|schema| schema["label"].as_str()).unwrap_or_else(|| if self.tab == 0 { row.key.rsplit('.').next().unwrap() } else { row.key.as_str() }).to_string();
                            ui.push_id(egui::Id::new((&target.kind, target.id, self.context, &row.key)), |ui| {
                                let mut changed = false;
                                let label_width = (ui.available_width() * 0.36).min(105.);
                                let field_width = (ui.available_width() - label_width - 24. - 2. * ui.spacing().item_spacing.x).max(32.);
                                ui.horizontal(|ui| {
                                    let label = ui.add_sized([label_width, 24.], egui::Label::new(label).wrap());
                                    label.on_hover_text(format!("{} ({})", row.key, row.kind));
                                    ui.add_enabled_ui(editable && enabled, |ui| {
                                        ui.allocate_ui_with_layout(egui::vec2(field_width, 24.), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                            ui.set_width(field_width);
                                            changed = edit_field(ui, row, &self.committed, FieldEditors {
                                                rects: &mut self.rects,
                                                color: &mut self.color_draft,
                                                numeric_edits: &mut self.numeric_edits,
                                                font_previews: &mut self.font_previews,
                                                selection: &mut self.selection_draft,
                                            }, structured);
                                        });
                                    });
                                    if row.has_default && !row.readonly {
                                        let reset = ui.add_enabled(!row.is_default, egui::Button::new("↺").small())
                                            .on_hover_text(format!("Reset {} to default{}", row.key, row.default_value.as_ref().map_or(String::new(), |value| format!(" ({value})"))));
                                        if ui.is_rect_visible(reset.rect) { record(&mut self.rects, &format!("Reset {}", row.key), reset.rect); }
                                        if reset.clicked() {
                                            let keys = row.schema.as_ref().filter(|_| structured).and_then(|schema| schema.get("reset")).cloned().unwrap_or_else(|| json!([row.key]));
                                            self.numeric_edits.remove(&row.key);
                                            changes.push(("reset_node_props", json!({"nodeType":target.kind,"nodeId":target.id,"props":keys})));
                                        }
                                    } else { ui.add_space(22.); }
                                });
                                if changed && display_changed(row, &self.committed, structured) {
                                    if structured && row.kind == "toggle" && row.schema.as_ref().is_some_and(|schema| schema["local"] == true) {
                                        self.local_toggles.insert(row.key.clone(), row.value == "true");
                                        self.committed.insert(row.key.clone(), row.value.clone());
                                    }
                                    let change = property_change(&target, row, &self.committed, structured);
                                    if change.0 != "set_node_props" || change.1["updates"].as_array().is_none_or(|updates| !updates.is_empty()) { changes.push(change); }
                                }
                            });
                        }
                    };
                    if self.tab == 0 && filter.is_empty() {
                        let expanded = if structured { self.sections.iter().find(|section| section["title"] == group).is_some_and(|section| section["expanded"] == true) } else { matches!(group.as_str(), "Common" | "Appearance" | "Geometry") };
                        let header = egui::CollapsingHeader::new(&group).default_open(expanded).show(ui, fields);
                        record(&mut self.rects, &format!("Section {group}"), header.header_response.rect);
                    } else { fields(ui); }
                }
            });
        });
        if let Some(mut draft) = self.selection_draft.take() {
            if draft.fresh {
                self.selection_builder.opened();
                self.selection_builder.draft = self
                    .selection_operands
                    .get(&draft.key)
                    .cloned()
                    .unwrap_or_default();
                draft.fresh = false;
            }
            let molecule =
                cuemol_command::selection::property_molecule(state, &target.kind, target.id);
            let mut open = !draft.close_requested;
            draft.close_requested = false;
            let mut quick = false;
            let cancelled = ctx.input(|input| input.key_pressed(egui::Key::Escape));
            let room = ctx.content_rect();
            let close_behavior = if egui::Popup::is_any_open(ctx)
                || ctx.input(|input| {
                    input
                        .pointer
                        .interact_pos()
                        .is_some_and(|pos| draft.anchor.contains(pos))
                }) {
                egui::PopupCloseBehavior::IgnoreClicks
            } else {
                egui::PopupCloseBehavior::CloseOnClickOutside
            };
            let popup = egui::Popup::new(
                egui::Id::new("inspector-selection-picker"),
                ctx.clone(),
                draft.anchor,
                draft.layer,
            )
            .open_bool(&mut open)
            .close_behavior(close_behavior)
            .width(300.0_f32.min(room.width() - 24.))
            .show(|ui| {
                ui.set_max_width(300.0_f32.min(room.width() - 24.));
                let height = (room.bottom() - draft.anchor.bottom())
                    .max(draft.anchor.top() - room.top())
                    - 24.;
                ui.set_max_height(height.max(100.));
                egui::ScrollArea::vertical()
                    .max_height(height.max(100.))
                    .show(ui, |ui| {
                        ui.monospace(&draft.value);
                        if let Some(action) = self.selection_builder.show(
                            ui,
                            state,
                            molecule,
                            self.revision,
                            &draft.value,
                        ) {
                            draft.value = action.expression;
                            quick = action.quick;
                        }
                    });
            });
            if let Some(popup) = popup {
                record(&mut self.rects, "Selection popup", popup.response.rect);
            }
            self.rects.extend(self.selection_builder.rects.clone());
            self.selection_operands
                .insert(draft.key.clone(), self.selection_builder.draft.clone());
            if let Some(row) = self.rows.iter_mut().find(|row| row.key == draft.key) {
                row.value = if cancelled {
                    draft.original.clone()
                } else {
                    draft.value.clone()
                };
                if cancelled {
                    open = false;
                }
                if quick {
                    open = false;
                }
                if !open && !cancelled && draft.value != draft.original {
                    match state
                        .active_scene()
                        .map(|scene| state.parse_scene_selector(scene, &draft.value))
                    {
                        Some(Ok(_)) => changes.push(property_change(
                            &target,
                            row,
                            &self.committed,
                            self.tab == 0,
                        )),
                        Some(Err(error)) => {
                            self.error = error.to_string();
                            open = true;
                        }
                        None => {
                            self.error = "No active scene".into();
                        }
                    }
                }
                if open {
                    self.selection_draft = Some(draft);
                }
            }
        }
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

fn equivalent(left: &str, right: &str) -> bool {
    left == right
        || left
            .parse::<f64>()
            .ok()
            .zip(right.parse::<f64>().ok())
            .is_some_and(|(left, right)| (left - right).abs() < 1e-6)
}

fn blank_negative_value(value: &str) -> String {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.)
        .map(|value| if value == 0. { 0. } else { value })
        .unwrap_or(-1.)
        .to_string()
}

fn display_changed(row: &Property, committed: &BTreeMap<String, String>, structured: bool) -> bool {
    let schema = row.schema.as_ref().filter(|_| structured);
    if schema.is_some_and(|schema| schema["blankNegative"] == true) {
        return committed
            .get(&row.key)
            .map(|value| blank_negative_value(value))
            != Some(blank_negative_value(row.displayed_value(structured)));
    }
    let original = schema
        .filter(|schema| schema["local"] != true)
        .and_then(|schema| schema["display"].as_str())
        .or_else(|| committed.get(&row.key).map(String::as_str));
    original != Some(row.displayed_value(structured))
}

fn property_change(
    target: &InspectorTarget,
    row: &Property,
    committed: &BTreeMap<String, String>,
    structured: bool,
) -> (&'static str, Value) {
    let schema = row.schema.as_ref().filter(|_| structured);
    let value = row.displayed_value(structured);
    let mut updates = Vec::new();
    if row.kind == "toggle" {
        if let Some(patch) = schema.and_then(|schema| schema["patches"][value].as_object()) {
            updates.extend(
                patch
                    .iter()
                    .map(|(key, value)| json!({"prop":key,"value":value})),
            );
        }
    } else if row.kind == "preset" {
        if let Some(patch) = schema
            .and_then(|schema| schema["options"].as_array())
            .and_then(|options| options.iter().find(|option| option["label"] == value))
            .and_then(|option| option["patch"].as_object())
        {
            updates.extend(
                patch
                    .iter()
                    .map(|(key, value)| json!({"prop":key,"value":value})),
            );
        }
    } else {
        if let Some(writes) = schema.and_then(|schema| schema["writes"].as_array()) {
            if let Ok(value) = value.parse::<f64>() {
                for write in writes {
                    let result = write["inverse"].as_f64().map_or_else(
                        || value * write["scale"].as_f64().unwrap_or(1.),
                        |numerator| numerator / value,
                    ) + write["offset"].as_f64().unwrap_or(0.);
                    if result.is_finite() {
                        updates.push(json!({"prop":write["prop"],"value":result.to_string()}));
                    } else {
                        updates.clear();
                        break;
                    }
                }
            }
        } else if let Some(targets) = schema.and_then(|schema| schema["targets"].as_array()) {
            updates.extend(targets.iter().map(|key| json!({"prop":key,"value":value})));
        } else if schema.is_some_and(|schema| schema["blankNegative"] == true) {
            updates.push(json!({"prop":row.key,"value":blank_negative_value(value)}));
        } else {
            updates.push(json!({"prop":row.key,"value":value}));
        }
        if value == "true" {
            if let Some(seeds) = schema.and_then(|schema| schema["seedWhenEnabled"].as_object()) {
                updates.extend(
                    seeds
                        .iter()
                        .filter(|(key, _)| committed.get(*key).is_none_or(String::is_empty))
                        .map(|(key, value)| json!({"prop":key,"value":value})),
                );
            }
        }
    }
    if updates.len() == 1 {
        (
            "set_node_prop",
            json!({"nodeType":target.kind,"nodeId":target.id,"prop":updates[0]["prop"],"value":updates[0]["value"],"cascadeGroupVisibility":structured}),
        )
    } else {
        (
            "set_node_props",
            json!({"nodeType":target.kind,"nodeId":target.id,"updates":updates,"cascadeGroupVisibility":structured}),
        )
    }
}

fn record(rects: &mut BTreeMap<String, [f32; 4]>, name: &str, rect: egui::Rect) {
    rects.insert(
        name.into(),
        [rect.min.x, rect.min.y, rect.width(), rect.height()],
    );
}

struct FieldEditors<'a> {
    rects: &'a mut BTreeMap<String, [f32; 4]>,
    color: &'a mut Option<ColorDraft>,
    numeric_edits: &'a mut BTreeMap<String, NumericEdit>,
    font_previews: &'a mut fonts::FontPreviews,
    selection: &'a mut Option<SelectionDraft>,
}

fn edit_field(
    ui: &mut egui::Ui,
    row: &mut Property,
    committed: &BTreeMap<String, String>,
    editors: FieldEditors<'_>,
    structured: bool,
) -> bool {
    let FieldEditors {
        rects,
        color,
        numeric_edits,
        font_previews,
        selection,
    } = editors;
    let schema = row.schema.as_ref().filter(|_| structured);
    let value_text = if structured {
        row.structured_value.as_mut().unwrap_or(&mut row.value)
    } else {
        &mut row.value
    };
    let choices = row
        .structured_choices
        .as_deref()
        .filter(|_| structured)
        .unwrap_or(&row.allowed);
    let (response, changed) = if structured && row.kind == "selection" {
        let width = (ui.available_width() - 26.).max(24.);
        let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
        let response = ui.add_sized(
            [width, 22.],
            egui::TextEdit::singleline(value_text).hint_text("* (all atoms)"),
        );
        let caret = crate::shell::icon_button(
            ui,
            crate::shell::Icon::Down,
            "Build selection",
            false,
            egui::vec2(22., 22.),
        );
        record(rects, &format!("Build {}", row.key), caret.rect);
        if let Some(draft) = selection.as_mut().filter(|draft| draft.key == row.key) {
            draft.anchor = response.rect.union(caret.rect);
            draft.layer = caret.layer_id;
        }
        if caret.clicked() {
            if let Some(draft) = selection.as_mut().filter(|draft| draft.key == row.key) {
                draft.close_requested = true;
            } else {
                *selection = Some(SelectionDraft {
                    key: row.key.clone(),
                    original: value_text.clone(),
                    value: value_text.clone(),
                    anchor: response.rect.union(caret.rect),
                    layer: caret.layer_id,
                    fresh: true,
                    close_requested: false,
                });
            }
        }
        if response.changed() {
            if let Some(draft) = selection.as_mut().filter(|draft| draft.key == row.key) {
                draft.value.clone_from(value_text);
            }
        }
        let mut changed = (response.lost_focus()
            || response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
            && selection.as_ref().is_none_or(|draft| draft.key != row.key);
        if escape && (response.has_focus() || response.lost_focus()) {
            if let Some(original) = committed.get(&row.key) {
                value_text.clone_from(original);
            }
            response.surrender_focus();
            changed = false;
        }
        (response, changed)
    } else if !choices.is_empty() {
        let mut changed = false;
        let label = |value: &str| {
            row.schema
                .as_ref()
                .filter(|_| structured)
                .and_then(|schema| schema["labels"][value].as_str())
                .unwrap_or(value)
                .to_string()
        };
        let response = egui::ComboBox::from_id_salt("value")
            .width(ui.available_width())
            .selected_text(label(value_text))
            .show_ui(ui, |ui| {
                for value in choices {
                    let option = if schema.is_some_and(|schema| schema["font"] == true) {
                        let mut option = font_previews.option(ui, value, value_text == value);
                        if option.clicked() && value_text != value {
                            value_text.clone_from(value);
                            option.mark_changed();
                        }
                        option
                    } else {
                        ui.selectable_value(value_text, value.clone(), label(value))
                    };
                    if ui.is_rect_visible(option.rect) {
                        record(rects, &format!("{} option {value}", row.key), option.rect);
                    }
                    changed |= option.changed();
                }
            })
            .response;
        if response.clicked() && schema.is_some_and(|schema| schema["font"] == true) {
            cuemol_desktop_core::font_catalog::request();
        }
        (response, changed)
    } else if matches!(row.kind.as_str(), "boolean" | "toggle") {
        let mut flag = value_text == "true";
        let response = ui.checkbox(&mut flag, "");
        let changed = response.changed();
        if changed {
            *value_text = flag.to_string();
        }
        (response, changed)
    } else if matches!(row.kind.as_str(), "integer" | "real")
        && value_text.parse::<f64>().is_ok()
        && !schema.is_some_and(|schema| schema["blankNegative"] == true)
    {
        let mut value = value_text.parse::<f64>().unwrap();
        let edit = numeric_edits.entry(row.key.clone()).or_default();
        let optional = schema.and_then(|schema| schema.get("optional"));
        let mut optional_on = value >= 0.;
        let mut toggled = false;
        if let Some(optional) = optional {
            if optional_on {
                edit.optional_value = Some(value);
            }
            let gate = ui
                .checkbox(&mut optional_on, "")
                .on_hover_text(optional["label"].as_str().unwrap_or("Enabled"));
            record(rects, &format!("Enable {}", row.key), gate.rect);
            if gate.changed() {
                value = if optional_on {
                    edit.optional_value
                        .unwrap_or(optional["on"].as_f64().unwrap_or(0.))
                } else {
                    optional["off"].as_f64().unwrap_or(-1.)
                };
                *value_text = value.to_string();
                toggled = true;
            }
            if !optional_on {
                value = edit
                    .optional_value
                    .unwrap_or(optional["on"].as_f64().unwrap_or(0.));
            }
        }
        if edit.value != Some(value) && !edit.dirty {
            if let Some(id) = edit.id {
                ui.data_mut(|data| data.remove::<String>(id));
            }
        }
        let previous = value;
        let step = schema
            .and_then(|schema| schema["step"].as_f64())
            .unwrap_or(if row.kind == "integer" { 1. } else { 0.01 });
        let speed = ui.input(|input| {
            if input.modifiers.ctrl || input.modifiers.command {
                schema
                    .and_then(|schema| schema["coarseStep"].as_f64())
                    .unwrap_or(step * 10.)
            } else if input.modifiers.shift_only() {
                schema
                    .and_then(|schema| schema["fineStep"].as_f64())
                    .map_or(step, |fine| fine * 10.)
            } else {
                step
            }
        });
        let mut editor = egui::DragValue::new(&mut value)
            .speed(speed)
            .update_while_editing(false);
        if let Some(schema) = schema {
            if let Some((min, max)) = schema["min"].as_f64().zip(schema["max"].as_f64()) {
                editor = editor.range(min..=max).clamp_existing_to_range(false);
            }
            if let Some(decimals) = schema["decimals"].as_u64() {
                editor = editor.max_decimals(decimals as usize);
            }
            if let Some(unit) = schema["unit"].as_str().filter(|unit| !unit.is_empty()) {
                editor = editor.suffix(format!(" {unit}"));
            }
        }
        if row.kind == "integer" {
            editor = editor.max_decimals(0);
        }
        let response = ui
            .add_enabled_ui(optional.is_none() || optional_on, |ui| {
                ui.add_sized([ui.available_width(), 22.], editor)
            })
            .inner;
        if value != previous && schema.is_some() {
            let fine = schema
                .and_then(|schema| schema["fineStep"].as_f64())
                .unwrap_or(step / 10.);
            if fine.is_finite() && fine > 0. {
                let scale = 10_f64.powf((-fine.log10().floor()).max(0.));
                value = (value * scale).round() / scale;
            }
        }
        edit.id = Some(response.id);
        edit.value = Some(value);
        edit.dirty =
            response.has_focus() && (edit.dirty || response.changed() && value == previous);
        if response.changed() && (optional.is_none() || optional_on) {
            *value_text = value.to_string();
        }
        let changed =
            toggled || (response.changed() && !response.dragged()) || response.drag_stopped();
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
        let response = ui.add(
            egui::TextEdit::singleline(value_text)
                .desired_width(ui.available_width())
                .hint_text(
                    schema
                        .and_then(|schema| schema["placeholder"].as_str())
                        .unwrap_or(""),
                ),
        );
        let changed = response.lost_focus()
            || (response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
        (response, changed)
    };
    if ui.is_rect_visible(response.rect) {
        record(rects, &row.key, response.rect);
    }
    changed
        && (!row.readonly
            || structured
                && row
                    .schema
                    .as_ref()
                    .is_some_and(|schema| schema["editable"] == true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_popup_keeps_nested_choices_open_for_editing() {
        let mut state = DesktopAppState::new();
        state.new_scene("selection", true);
        state.open_bytes("selection.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let object = state.active_object_summaries()[0].uid;
        let mut inspector = Inspector::default();
        inspector.inspect(
            &mut state,
            &mut Camera::new(),
            Some(InspectorTarget {
                kind: "object".into(),
                id: Some(object.get()),
            }),
            0,
        );
        inspector.filter = "sel".into();
        let ctx = egui::Context::default();
        let mut frame = 0;
        let mut step = |inspector: &mut Inspector, events: Vec<egui::Event>| {
            frame += 1;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280., 900.),
                    )),
                    time: Some(frame as f64 / 60.),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    assert!(inspector.panel(ctx, &state).is_empty());
                },
            );
        };
        step(&mut inspector, vec![]);
        step(&mut inspector, vec![]);
        for key in [
            "Build sel",
            "Selection tab Term",
            "Selection keyword",
            "Selection keyword chain",
            "sel",
        ] {
            let [x, y, w, h] = inspector.rects[key];
            let pos = egui::pos2(x + w / 2., y + h / 2.);
            for pressed in [true, false] {
                step(
                    &mut inspector,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        },
                    ],
                );
            }
            step(&mut inspector, vec![]);
            step(&mut inspector, vec![]);
            assert!(inspector.selection_draft.is_some(), "closed after {key}");
        }
        assert_eq!(inspector.selection_builder.draft.keyword, "chain");
        assert!(inspector.rects.contains_key("Selection value"));
        let [x, y, w, h] = inspector.rects["sel"];
        let pos = egui::pos2(x + w / 2., y + h / 2.);
        step(
            &mut inspector,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
        step(&mut inspector, vec![]);
        assert!(
            inspector.selection_draft.is_some(),
            "closed after same-frame click"
        );
    }

    #[test]
    fn map_center_reset_tracks_both_explicit_flags() {
        let mut state = DesktopAppState::new();
        state.open_bytes("map.dx", b"object 1 class gridpositions counts 2 2 2\norigin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 2 2 2\nobject 3 class array type double rank 0 items 8 data follows\n0 1 2 3 4 5 6 7\n").unwrap();
        let map = state.density_map_summaries(state.active_scene().unwrap())[0].uid;
        let renderer = state.add_density_renderer(map, "contour").unwrap();
        cuemol_command::call_tool(&mut state, &mut Camera::new(), "reset_node_props", &json!({"nodeType":"renderer","nodeId":renderer.get(),"props":["autoupdate","dragupdate"]})).unwrap();
        state
            .set_auxiliary_renderer_property(map, renderer, "dragupdate", "true")
            .unwrap();
        let target = Some(InspectorTarget {
            kind: "renderer".into(),
            id: Some(renderer.get()),
        });
        let mut inspector = Inspector::default();
        inspector.inspect(&mut state, &mut Camera::new(), target, 1);
        assert!(
            inspector
                .rows
                .iter()
                .find(|row| row.key == "autoupdate")
                .unwrap()
                .is_default
        );
        assert!(
            !inspector
                .rows
                .iter()
                .find(|row| row.key == "dragupdate")
                .unwrap()
                .is_default
        );
        let center = inspector
            .rows
            .iter()
            .find(|row| row.key == "center-update")
            .unwrap();
        assert!(center.has_default);
        assert!(!center.is_default);
    }

    #[test]
    fn deleted_renderer_clears_cached_fields_before_painting() {
        let mut state = DesktopAppState::new();
        state.open_bytes("deleted.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let object = state.active_object_summaries()[0].uid;
        let renderer = state
            .scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .add_renderer("tube", "Tube");
        let target = Some(InspectorTarget {
            kind: "renderer".into(),
            id: Some(renderer.get()),
        });
        let mut inspector = Inspector::default();
        let mut camera = Camera::new();
        inspector.inspect(&mut state, &mut camera, target.clone(), 0);
        assert!(!inspector.structured_order.is_empty());
        assert!(state
            .scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .destroy_renderer(renderer));
        inspector.inspect(&mut state, &mut camera, target, 1);
        assert!(!inspector.error.is_empty());
        let ctx = egui::Context::default();
        for tab in 0..2 {
            inspector.tab = tab;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert!(inspector.panel(ctx, &state).is_empty());
            });
        }
        assert!(inspector.rows.is_empty());
        assert!(inspector.sections.is_empty());
        assert!(inspector.structured_order.is_empty());
        assert!(inspector.generic_order.is_empty());
    }

    #[test]
    fn tube_axes_and_nucleic_ratios_preserve_the_other_dimension() {
        let mut state = DesktopAppState::new();
        state.open_bytes("axes.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let object = state.active_object_summaries()[0].uid;
        let mut camera = Camera::new();
        for kind in ["tube", "nucl"] {
            let renderer = state
                .scene_manager_mut()
                .object_mut(object)
                .unwrap()
                .add_renderer(kind, kind);
            let target = InspectorTarget {
                kind: "renderer".into(),
                id: Some(renderer.get()),
            };
            let mut inspector = Inspector::default();
            let (first, second) = if kind == "tube" {
                ("section.width", "section.tuber")
            } else {
                ("base_size", "base_thick")
            };
            let set = |state: &mut DesktopAppState, camera: &mut Camera, key: &str, value: &str| {
                cuemol_command::call_tool(state,camera,"set_node_prop",&json!({"nodeType":"renderer","nodeId":renderer.get(),"prop":key,"value":value})).unwrap();
            };
            set(&mut state, &mut camera, first, "1");
            set(&mut state, &mut camera, second, "0.5");
            inspector.inspect(&mut state, &mut camera, Some(target.clone()), 1);
            let row = inspector
                .rows
                .iter_mut()
                .find(|row| row.key == first)
                .unwrap();
            row.value = "2".into();
            let (tool, args) = property_change(&target, row, &inspector.committed, true);
            assert_eq!(tool, "set_node_props");
            assert_eq!(args["updates"].as_array().unwrap().len(), 2);
            cuemol_command::call_tool(&mut state, &mut camera, tool, &args).unwrap();
            inspector.inspect(&mut state, &mut camera, Some(target.clone()), 2);
            let row = inspector
                .rows
                .iter_mut()
                .find(|row| row.key == second)
                .unwrap();
            assert_eq!(
                row.displayed_value(true),
                if kind == "tube" { "0.5" } else { "50" }
            );
            assert_eq!(row.value, if kind == "tube" { "0.25" } else { "1" });
            row.structured_value = Some(if kind == "tube" { "0.2" } else { "25" }.into());
            let (tool, args) = property_change(&target, row, &inspector.committed, true);
            cuemol_command::call_tool(&mut state, &mut camera, tool, &args).unwrap();
            let values = state
                .molecule_renderer_properties(object, renderer)
                .unwrap();
            assert_eq!(values[first], "2");
            assert_eq!(values[second], if kind == "tube" { "0.1" } else { "0.5" });
        }
    }

    #[test]
    fn cartoon_helix_switches_pages_and_writes_both_junctions() {
        let mut state = DesktopAppState::new();
        state.open_bytes("helix.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let object = state.active_object_summaries()[0].uid;
        let renderer = state
            .scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .add_renderer("cartoon", "cartoon");
        let target = InspectorTarget {
            kind: "renderer".into(),
            id: Some(renderer.get()),
        };
        let mut camera = Camera::new();
        let mut inspector = Inspector::default();
        state
            .set_renderer_property(object, renderer, "helix_ribbon", "false")
            .unwrap();
        inspector.inspect(&mut state, &mut camera, Some(target.clone()), 1);
        assert!(inspector
            .structured_order
            .iter()
            .any(|index| inspector.rows[*index].key == "helix_extend"));
        assert!(!inspector
            .structured_order
            .iter()
            .any(|index| inspector.rows[*index].key == "ribhelix.width"));
        state
            .set_renderer_property(object, renderer, "helix_ribbon", "true")
            .unwrap();
        inspector.inspect(&mut state, &mut camera, Some(target.clone()), 2);
        assert!(!inspector
            .structured_order
            .iter()
            .any(|index| inspector.rows[*index].key == "helix_extend"));
        let row = inspector
            .rows
            .iter_mut()
            .find(|row| row.key == "ribhelix_head.arrow")
            .unwrap();
        row.structured_value = Some("80".into());
        let (tool, args) = property_change(&target, row, &inspector.committed, true);
        cuemol_command::call_tool(&mut state, &mut camera, tool, &args).unwrap();
        let values = state
            .molecule_renderer_properties(object, renderer)
            .unwrap();
        assert_eq!(values["ribhelix_head.arrow"], "2.6");
        assert_eq!(values["ribhelix_tail.arrow"], "2.6");
    }

    #[test]
    fn view_refreshes_after_navigation_without_a_scene_revision() {
        let mut state = DesktopAppState::new();
        state.new_scene("scene", true);
        let mut camera = Camera::new();
        let target = Some(InspectorTarget {
            kind: "view".into(),
            id: None,
        });
        let mut inspector = Inspector::default();
        inspector.inspect(&mut state, &mut camera, target.clone(), 0);
        let original_name = inspector.committed["name"].clone();
        inspector
            .rows
            .iter_mut()
            .find(|row| row.key == "name")
            .unwrap()
            .value = "Pending view".into();
        camera.set_zoom(23.);
        inspector.inspect(&mut state, &mut camera, target, 0);
        assert_eq!(inspector.committed["zoom"], "23");
        assert_eq!(inspector.committed["name"], original_name);
        assert_eq!(
            inspector
                .rows
                .iter()
                .find(|row| row.key == "name")
                .unwrap()
                .value,
            "Pending view"
        );
        assert!(inspector.rows.iter().all(|row| !row.has_default));
        let id = state.active_view_summary().unwrap().uid;
        state
            .command_manager_mut()
            .view_mut(id)
            .unwrap()
            .set_view_size(997, 613);
        inspector.inspect(
            &mut state,
            &mut camera,
            Some(InspectorTarget {
                kind: "view".into(),
                id: None,
            }),
            0,
        );
        assert_eq!(inspector.committed["width"], "997");
        assert_eq!(inspector.committed["height"], "613");
        cuemol_command::call_tool(
            &mut state,
            &mut camera,
            "set_node_prop",
            &json!({"nodeType":"view","prop":"name","value":"External rename"}),
        )
        .unwrap();
        inspector.inspect(
            &mut state,
            &mut camera,
            Some(InspectorTarget {
                kind: "view".into(),
                id: None,
            }),
            1,
        );
        let name = inspector
            .rows
            .iter_mut()
            .find(|row| row.key == "name")
            .unwrap();
        assert_eq!(name.value, "External rename");
        name.value = "Pending view".into();
        state.new_scene("other", true);
        inspector.inspect(
            &mut state,
            &mut camera,
            Some(InspectorTarget {
                kind: "view".into(),
                id: None,
            }),
            0,
        );
        assert_eq!(
            inspector
                .rows
                .iter()
                .find(|row| row.key == "name")
                .unwrap()
                .value,
            inspector.committed["name"]
        );
        assert_ne!(inspector.committed["name"], "Pending view");
    }

    #[test]
    fn scene_presets_are_derived_and_proofing_seeds_only_an_empty_profile() {
        let mut state = DesktopAppState::new();
        state.new_scene("scene", true);
        let mut camera = Camera::new();
        let target = InspectorTarget {
            kind: "scene".into(),
            id: None,
        };
        let mut inspector = Inspector::default();
        inspector.inspect(&mut state, &mut camera, Some(target.clone()), 0);
        let preset = inspector
            .rows
            .iter_mut()
            .find(|row| row.key == "ao-preset")
            .unwrap();
        assert_eq!(preset.value, "Low");
        preset.value = "High".into();
        let (tool, args) = property_change(&target, preset, &inspector.committed, true);
        cuemol_command::call_tool(&mut state, &mut camera, tool, &args).unwrap();
        inspector.inspect(&mut state, &mut camera, Some(target.clone()), 1);
        assert_eq!(inspector.committed["aoRadius"], "12");
        assert_eq!(inspector.committed["ao-preset"], "High");
        let proof = inspector
            .rows
            .iter_mut()
            .find(|row| row.key == "use_colproof")
            .unwrap();
        proof.value = "true".into();
        let (tool, args) = property_change(&target, proof, &inspector.committed, true);
        assert_eq!(tool, "set_node_props");
        cuemol_command::call_tool(&mut state, &mut camera, tool, &args).unwrap();
        inspector
            .committed
            .insert("icc_filename".into(), "custom.icc".into());
        assert_eq!(
            property_change(&target, proof, &inspector.committed, true).0,
            "set_node_prop"
        );
        cuemol_command::call_tool(
            &mut state,
            &mut camera,
            "set_node_prop",
            &json!({"nodeType":"scene","prop":"aoSteps","value":"9"}),
        )
        .unwrap();
        inspector.inspect(&mut state, &mut camera, Some(target), 2);
        assert_eq!(inspector.committed["ao-preset"], "Custom");
    }

    #[test]
    fn escape_cancels_selection_text_after_text_edit_releases_focus() {
        let mut state = DesktopAppState::new();
        state.open_bytes("selection.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let object = state.active_object_summaries()[0].uid;
        let mut inspector = Inspector::default();
        inspector.inspect(
            &mut state,
            &mut Camera::new(),
            Some(InspectorTarget {
                kind: "object".into(),
                id: Some(object.get()),
            }),
            0,
        );
        inspector.filter = "sel".into();
        let original = inspector.committed["sel"].clone();
        let context = egui::Context::default();
        let mut time = 0.;
        let mut step = |inspector: &mut Inspector, events| {
            time += 0.02;
            let mut changes = Vec::new();
            let _ = context.run(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280., 900.),
                    )),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |context| {
                    changes = inspector.panel(context, &state);
                },
            );
            changes
        };
        for _ in 0..2 {
            assert!(step(&mut inspector, vec![]).is_empty());
        }
        let [x, y, w, h] = inspector.rects["sel"];
        let pos = egui::pos2(x + w / 2., y + h / 2.);
        for pressed in [true, false] {
            assert!(step(
                &mut inspector,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        pressed,
                        button: egui::PointerButton::Primary,
                        modifiers: Default::default()
                    }
                ]
            )
            .is_empty());
        }
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        assert!(step(
            &mut inspector,
            vec![
                key(
                    egui::Key::A,
                    egui::Modifiers {
                        command: true,
                        ctrl: true,
                        ..Default::default()
                    }
                ),
                egui::Event::Text("name C1".into())
            ]
        )
        .is_empty());
        assert_eq!(
            inspector
                .rows
                .iter()
                .find(|row| row.key == "sel")
                .unwrap()
                .value,
            "name C1"
        );
        assert!(step(
            &mut inspector,
            vec![key(egui::Key::Escape, Default::default())]
        )
        .is_empty());
        assert_eq!(
            inspector
                .rows
                .iter()
                .find(|row| row.key == "sel")
                .unwrap()
                .value,
            original
        );
        assert!(step(&mut inspector, vec![]).is_empty());
    }
}
