use super::{clock, element_arg, number, parse_clock, record, text, Command};
use crate::numeric::NumericEdit;
use cuemol_desktop_core::DesktopAppState;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Debug)]
pub struct ElementInspector {
    pub rects: BTreeMap<String, [f32; 4]>,
    pub visible: bool,
    target: Option<(u64, u64)>,
    revision: Option<u64>,
    data: Value,
    values: BTreeMap<String, String>,
    numeric: BTreeMap<String, NumericEdit>,
    generic: bool,
    error: String,
}

impl ElementInspector {
    pub fn cancel(&mut self) {
        for (key, edit) in &mut self.numeric {
            if let Some(drag) = edit.drag.take() {
                self.values.insert(key.clone(), drag.original);
            }
            edit.cancelled = true;
        }
        self.revision = None;
    }

    pub fn panel(
        &mut self,
        ctx: &egui::Context,
        state: &DesktopAppState,
        uid: u64,
        revision: u64,
    ) -> Vec<Command> {
        self.rects.clear();
        let target = state.active_scene().map(|sid| (sid.get(), uid));
        if target != self.target {
            self.cancel();
            self.numeric.clear();
            self.values.clear();
            self.target = target;
            self.visible = true;
            self.generic = false;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape) || !input.focused) {
            self.cancel();
        }
        if !ctx.input(|input| input.pointer.primary_down()) {
            for edit in self.numeric.values_mut() {
                edit.cancelled = false;
            }
        }
        if self.revision != Some(revision) && !self.numeric.values().any(|edit| edit.drag.is_some())
        {
            match cuemol_command::animation_element_detail(state, uid) {
                Ok(data) => {
                    let mut values: BTreeMap<String, String> = data["properties"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(key, value)| (key.clone(), value.as_str().unwrap_or("").into()))
                        .collect();
                    values.insert("startMs".into(), number(&data, "startMs").to_string());
                    values.insert(
                        "durationMs".into(),
                        (number(&data, "endMs") - number(&data, "startMs")).to_string(),
                    );
                    self.values = values;
                    self.data = data;
                    self.error.clear();
                }
                Err(error) => {
                    self.visible = !matches!(error, cuemol_core::CueMolError::NotFound(_));
                    self.error = error.to_string();
                    self.data = Value::Null;
                    self.values.clear();
                }
            }
            self.revision = Some(revision);
        }
        if !self.visible {
            return Vec::new();
        }
        let mut changes = Vec::new();
        egui::SidePanel::right("animation_inspector").default_width(300.).width_range(240.0..=600.).resizable(true).show(ctx,|ui| {
            ui.horizontal(|ui| {
                ui.strong("Inspector");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                    let close=ui.small_button("×").on_hover_text("Close Inspector");
                    record(&mut self.rects,"Close animation Inspector",close.rect);
                    if close.clicked() {self.visible=false; self.cancel();}
                });
            });
            ui.separator();
            ui.strong(self.values.get("name").map(String::as_str).unwrap_or("Animation element"));
            ui.weak(text(&self.data,"type"));
            ui.horizontal(|ui| {
                record(&mut self.rects,"Animation Properties tab",ui.selectable_value(&mut self.generic,false,"Properties").rect);
                record(&mut self.rects,"Animation Generic tab",ui.selectable_value(&mut self.generic,true,"Generic").rect);
            });
            if !self.error.is_empty() {ui.colored_label(ui.visuals().error_fg_color,&self.error);return;}
            if let Some(error)=self.data["resolveError"].as_str() {ui.colored_label(ui.visuals().warn_fg_color,error);}
            let reset=ui.button("Reset all to default");
            record(&mut self.rects,"Animation reset all",reset.rect);
            if reset.clicked() {
                let keys=self.data["properties"].as_object().into_iter().flatten().map(|(key,_)|key.as_str()).filter(|key|resettable(key)).collect::<Vec<_>>();
                if !keys.is_empty() {changes.push(("reset_anim_prop",json!({"element":element_arg(uid),"props":keys.join(",")})));self.revision=None;}
            }
            egui::ScrollArea::vertical().id_salt("animation_properties_scroll").show(ui,|ui| {
                if self.generic {
                    let keys=self.data["properties"].as_object().into_iter().flatten().map(|(key,_)|key.clone()).collect::<Vec<_>>();
                    for key in keys { self.field(ui,uid,&key,&key,None,&mut changes); }
                } else {
                    ui.strong("Common");
                    self.field(ui,uid,"name","Name",None,&mut changes);
                    self.field(ui,uid,"disabled","Disabled",None,&mut changes);
                    self.field(ui,uid,"timeRefName","Relative to",None,&mut changes);
                    self.field(ui,uid,"startMs","Start time",None,&mut changes);
                    self.field(ui,uid,"durationMs","Duration",None,&mut changes);
                    if number(&self.data,"startMs")<0. {ui.weak("The saved negative start is preserved until edited.");}
                    self.field(ui,uid,"quadric","Quadric",Some(json!({"min":0,"max":50,"step":5,"unit":"%","decimals":0})),&mut changes);
                    ui.separator();
                    ui.strong(text(&self.data,"type"));
                    let keys=self.data["properties"].as_object().into_iter().flatten().map(|(key,_)|key.clone()).filter(|key|!matches!(key.as_str(),"uid"|"name"|"disabled"|"timeRefName"|"start"|"end"|"quadric")).collect::<Vec<_>>();
                    for key in keys {
                        let (label,schema)=match key.as_str() {
                            "angle"=>("Rotation angle",Some(json!({"min":0,"max":360,"step":5,"unit":"°","decimals":0}))),
                            "axis"=>("Spin axis",None), "endcam"=>("Target camera",None),
                            "ignorecenter"=>("Ignore center",None), "ignorerotate"=>("Ignore rotation",None),
                            "ignorezoom"=>("Ignore zoom",None), "ignoreslab"=>("Ignore slab",None),
                            "keepquatpositive"=>("Positive quaternion",None),"rend"=>("Target renderers",None),
                            "mol"=>("Target MorphMol",None), "hide"=>("Hide",None), "fade"=>("Fade",None),
                            "tgt_alpha"=>("Target opacity",Some(json!({"min":0,"max":1,"step":0.1}))),
                            "direction"=>("Direction angle",Some(json!({"min":0,"max":360,"step":1,"unit":"°","decimals":0}))),
                            "distance"=>("Distance",Some(json!({"min":0,"max":2,"step":0.1}))),
                            "startValue"=>("Start value",Some(json!({"min":0,"max":1,"step":0.1}))),
                            "endValue"=>("End value",Some(json!({"min":0,"max":1,"step":0.1}))),
                            _=>(key.as_str(),None),
                        };
                        self.field(ui,uid,&key,label,schema,&mut changes);
                    }
                }
            });
        });
        changes
    }

    fn field(
        &mut self,
        ui: &mut egui::Ui,
        uid: u64,
        key: &str,
        label: &str,
        schema: Option<Value>,
        changes: &mut Vec<Command>,
    ) {
        let mut value = self.values.get(key).cloned().unwrap_or_default();
        let original = match key {
            "startMs" => number(&self.data, "startMs").to_string(),
            "durationMs" => {
                (number(&self.data, "endMs") - number(&self.data, "startMs")).to_string()
            }
            _ => text(&self.data["properties"], key).to_string(),
        };
        let readonly = matches!(key, "uid" | "start" | "end" | "absStart" | "absEnd");
        let percent = key == "quadric" && !self.generic;
        if percent {
            let percent_value = value.parse::<f64>().unwrap_or(0.) * 100.;
            value = if percent_value == 0. {
                "0".into()
            } else {
                percent_value.to_string()
            };
        }
        let mut changed = false;
        ui.push_id((uid, key), |ui| {
            ui.horizontal(|ui| {
                let width = (ui.available_width() * 0.36).min(100.);
                ui.add_sized([width, 24.], egui::Label::new(label).wrap());
                ui.add_enabled_ui(!readonly, |ui| {
                    let width = (ui.available_width() - 28.).max(32.);
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, 24.),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_width(width);
                            if key == "timeRefName" && !self.generic {
                                let choices = self.data["references"]
                                    .as_array()
                                    .cloned()
                                    .unwrap_or_default();
                                let missing = !value.is_empty()
                                    && !choices.iter().any(|item| item["name"] == value);
                                let title = if value.is_empty() {
                                    "(absolute)".into()
                                } else if missing {
                                    format!("(missing: {value})")
                                } else {
                                    value.clone()
                                };
                                let combo = egui::ComboBox::from_id_salt("reference")
                                    .width(width)
                                    .selected_text(title)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut value,
                                            String::new(),
                                            "(absolute)",
                                        );
                                        for item in choices {
                                            let name = text(&item, "name");
                                            let response = ui.add_enabled(
                                                item["error"].is_null(),
                                                egui::Button::selectable(value == name, name),
                                            );
                                            record(
                                                &mut self.rects,
                                                format!("Animation reference {name}"),
                                                response.rect,
                                            );
                                            if response.clicked() {
                                                value = name.into();
                                            }
                                            if let Some(error) = item["error"].as_str() {
                                                response.on_hover_text(error);
                                            }
                                        }
                                    });
                                record(
                                    &mut self.rects,
                                    "Animation property timeRefName",
                                    combo.response.rect,
                                );
                                changed = value != original;
                            } else if matches!(key, "startMs" | "durationMs") {
                                let mut number = value.parse::<f64>().unwrap_or(0.);
                                let response = ui.add_sized(
                                    [width, 24.],
                                    egui::DragValue::new(&mut number)
                                        .speed(10.)
                                        .range(
                                            (if key == "startMs" {
                                                super::number(&self.data, "startMs").min(0.)
                                            } else {
                                                0.
                                            })
                                                ..=f64::MAX,
                                        )
                                        .clamp_existing_to_range(false)
                                        .custom_formatter(|value, _| clock(value))
                                        .custom_parser(parse_clock)
                                        .update_while_editing(false),
                                );
                                record(
                                    &mut self.rects,
                                    format!("Animation property {key}"),
                                    response.rect,
                                );
                                let edit = self.numeric.entry(key.into()).or_default();
                                if response.drag_started() {
                                    edit.drag = Some(crate::numeric::NumericDrag {
                                        original: original.clone(),
                                        was_default: false,
                                        realtime: false,
                                    });
                                }
                                if edit.cancelled {
                                    number = original.parse().unwrap_or(0.);
                                } else {
                                    changed = (response.changed() && !response.dragged())
                                        || response.drag_stopped();
                                }
                                if response.drag_stopped() {
                                    edit.drag = None;
                                }
                                value = number.to_string();
                            } else if value == "true" || value == "false" {
                                let mut enabled = value == "true";
                                let response = ui.checkbox(&mut enabled, "");
                                record(
                                    &mut self.rects,
                                    format!("Animation property {key}"),
                                    response.rect,
                                );
                                changed = response.changed();
                                value = enabled.to_string();
                            } else if value.parse::<f64>().is_ok()
                                && !matches!(
                                    key,
                                    "name" | "timeRefName" | "rend" | "mol" | "endcam"
                                )
                            {
                                let original_display = if percent {
                                    (original.parse::<f64>().unwrap_or(0.) * 100.).to_string()
                                } else {
                                    original.clone()
                                };
                                let (response, commit) = crate::numeric::field(
                                    ui,
                                    &format!("Animation property {key}"),
                                    "real",
                                    &mut value,
                                    schema.as_ref(),
                                    &original_display,
                                    false,
                                    self.numeric.entry(key.into()).or_default(),
                                    &mut self.rects,
                                );
                                record(
                                    &mut self.rects,
                                    format!("Animation property {key}"),
                                    response.rect,
                                );
                                changed = commit;
                            } else if matches!(key, "endcam" | "mol") && !self.generic {
                                let options = self.data
                                    [if key == "mol" { "molecules" } else { "cameras" }]
                                .as_array()
                                .cloned()
                                .unwrap_or_default();
                                let combo = egui::ComboBox::from_id_salt("target")
                                    .width(width)
                                    .selected_text(if value.is_empty() { "(none)" } else { &value })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut value, String::new(), "(none)");
                                        for option in options.iter().filter_map(Value::as_str) {
                                            ui.selectable_value(&mut value, option.into(), option);
                                        }
                                    });
                                record(
                                    &mut self.rects,
                                    format!("Animation property {key}"),
                                    combo.response.rect,
                                );
                                changed = value != original;
                            } else if key == "rend" && !self.generic {
                                let mut selected = value
                                    .split(',')
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_string)
                                    .collect::<BTreeSet<_>>();
                                ui.vertical(|ui| {
                                    for renderer in
                                        self.data["renderers"].as_array().into_iter().flatten()
                                    {
                                        let name = text(renderer, "name");
                                        let mut on = selected.contains(name);
                                        let response = ui.checkbox(
                                            &mut on,
                                            format!("{}/{name}", text(renderer, "object")),
                                        );
                                        record(
                                            &mut self.rects,
                                            format!("Animation renderer {name}"),
                                            response.rect,
                                        );
                                        if response.changed() {
                                            if on {
                                                selected.insert(name.into());
                                            } else {
                                                selected.remove(name);
                                            }
                                            changed = true;
                                        }
                                    }
                                    if self.data["renderers"].as_array().is_none_or(Vec::is_empty) {
                                        ui.weak("(no renderers)");
                                    }
                                });
                                value = selected.into_iter().collect::<Vec<_>>().join(",");
                            } else {
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut value).desired_width(width),
                                );
                                record(
                                    &mut self.rects,
                                    format!("Animation property {key}"),
                                    response.rect,
                                );
                                changed = response.lost_focus()
                                    || response.has_focus()
                                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                            }
                        },
                    );
                });
                if resettable(key) {
                    let response = ui.small_button("↺").on_hover_text("Reset to default");
                    record(
                        &mut self.rects,
                        format!("Animation reset {key}"),
                        response.rect,
                    );
                    if response.clicked() {
                        changes.push((
                            "reset_anim_prop",
                            json!({"element":element_arg(uid),"props":key}),
                        ));
                        self.revision = None;
                    }
                }
            })
        });
        if percent {
            value = (value.parse::<f64>().unwrap_or(0.) / 100.).to_string();
        }
        self.values.insert(key.into(), value.clone());
        if changed && value != original && !readonly {
            if matches!(key, "startMs" | "durationMs") {
                let start = self
                    .values
                    .get("startMs")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.);
                let duration = self
                    .values
                    .get("durationMs")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.);
                changes.push(("set_anim_time",json!({"element":element_arg(uid),"startMs":start,"endMs":start+duration.max(0.)})));
            } else {
                changes.push((
                    "set_anim_generic_prop",
                    json!({"element":element_arg(uid),"prop":key,"value":value}),
                ));
            }
            self.revision = None;
        }
    }
}
fn resettable(key: &str) -> bool {
    !matches!(
        key,
        "name"
            | "timeRefName"
            | "start"
            | "end"
            | "uid"
            | "absStart"
            | "absEnd"
            | "startMs"
            | "durationMs"
    )
}
