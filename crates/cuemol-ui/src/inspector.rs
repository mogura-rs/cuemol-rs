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
    schema: Option<Value>,
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
}

impl Inspector {
    pub fn inspect(
        &mut self,
        state: &mut DesktopAppState,
        camera: &mut Camera,
        target: Option<InspectorTarget>,
        revision: u64,
    ) {
        let view = target.as_ref().is_some_and(|target| target.kind == "view");
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
                            row.schema = Some(schema);
                        } else if let Some(options) = definition["options"].as_array() {
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
                            self.rows.push(Property {
                                key: key.into(),
                                value: selected
                                    .and_then(|value| value["label"].as_str())
                                    .unwrap_or("Custom")
                                    .into(),
                                kind: "preset".into(),
                                category: section["title"].as_str().unwrap_or("").into(),
                                readonly: false,
                                has_default: false,
                                is_default: false,
                                default_value: None,
                                allowed: options
                                    .iter()
                                    .filter_map(|option| {
                                        option["label"].as_str().map(str::to_string)
                                    })
                                    .collect(),
                                rgba: None,
                                schema: Some(schema),
                            });
                        }
                    }
                }
                self.generic_order = self
                    .rows
                    .iter()
                    .enumerate()
                    .filter_map(|(index, row)| (row.kind != "preset").then_some(index))
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
                            let enabled = schema.and_then(|schema| schema["enabledWhen"].as_str()).is_none_or(|key| self.committed.get(key).is_some_and(|value| value == "true"));
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
                                            changed = edit_field(ui, row, &mut self.rects, &mut self.color_draft, &mut self.numeric_edits, structured);
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
                                    changes.push(property_change(&target, row, &self.committed, self.tab == 0));
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

fn property_change(
    target: &InspectorTarget,
    row: &Property,
    committed: &BTreeMap<String, String>,
    structured: bool,
) -> (&'static str, Value) {
    let schema = row.schema.as_ref().filter(|_| structured);
    let mut updates = Vec::new();
    if row.kind == "preset" {
        if let Some(patch) = schema
            .and_then(|schema| schema["options"].as_array())
            .and_then(|options| options.iter().find(|option| option["label"] == row.value))
            .and_then(|option| option["patch"].as_object())
        {
            updates.extend(
                patch
                    .iter()
                    .map(|(key, value)| json!({"prop":key,"value":value})),
            );
        }
    } else {
        updates.push(json!({"prop":row.key,"value":row.value}));
        if row.value == "true" {
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

fn edit_field(
    ui: &mut egui::Ui,
    row: &mut Property,
    rects: &mut BTreeMap<String, [f32; 4]>,
    color: &mut Option<ColorDraft>,
    numeric_edits: &mut BTreeMap<String, NumericEdit>,
    structured: bool,
) -> bool {
    let (response, changed) = if !row.allowed.is_empty() {
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
            .selected_text(label(&row.value))
            .show_ui(ui, |ui| {
                for value in &row.allowed {
                    let option = ui.selectable_value(&mut row.value, value.clone(), label(value));
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
        let edit = numeric_edits.entry(row.key.clone()).or_default();
        if edit.value != Some(value) && !edit.dirty {
            if let Some(id) = edit.id {
                ui.data_mut(|data| data.remove::<String>(id));
            }
        }
        let previous = value;
        let mut editor = egui::DragValue::new(&mut value)
            .speed(if row.kind == "integer" { 1. } else { 0.01 })
            .update_while_editing(false);
        if row.kind == "integer" {
            editor = editor.max_decimals(0);
        }
        let response = ui.add_sized([ui.available_width(), 22.], editor);
        edit.id = Some(response.id);
        edit.value = Some(value);
        edit.dirty =
            response.has_focus() && (edit.dirty || response.changed() && value == previous);
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
}
