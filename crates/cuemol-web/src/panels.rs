use super::*;

impl App {
    pub(super) fn select_tool(&mut self, name: &str) {
        self.console_tool = name.into();
        self.form_tool.clear();
        self.show_console = true;
    }
    pub(super) fn open_properties(&mut self, kind: &str, id: Option<u64>) {
        match cuemol_command::call_tool(
            &mut self.state,
            &mut self.camera,
            "list_node_props",
            &json!({"nodeType":kind,"nodeId":id}),
        ) {
            Ok(data) => {
                let rows = data["properties"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| {
                        (
                            p["key"].as_str().unwrap_or("").into(),
                            p["value"].as_str().unwrap_or("").into(),
                        )
                    })
                    .collect();
                self.properties = Some((kind.into(), id, rows));
            }
            Err(e) => self.message = e.to_string(),
        }
    }
    pub(super) fn properties_ui(&mut self, ctx: &egui::Context) {
        let Some((kind, id, mut rows)) = self.properties.take() else {
            return;
        };
        let mut open = true;
        egui::Window::new(format!("{kind} properties")).id(egui::Id::new("properties")).open(&mut open).default_width(460.).show(ctx,|ui|{
            egui::ScrollArea::vertical().max_height(480.).show(ui,|ui|{
                egui::Grid::new("property-grid").striped(true).show(ui,|ui|{for (key,value) in &mut rows {
                    ui.label(key.as_str());
                    let changed=if value=="true"||value=="false"{let mut flag=value=="true";let changed=ui.checkbox(&mut flag,"").changed();if changed{*value=flag.to_string();}changed}else{ui.text_edit_singleline(value);false};
                    if ui.small_button("Apply").clicked()||changed{self.call("set_node_prop",json!({"nodeType":kind,"nodeId":id,"prop":key,"value":value}));}
                    ui.end_row();
                }});
            });
        });
        if open {
            self.properties = Some((kind, id, rows));
        }
    }
    pub(super) fn settings_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        egui::Window::new("Settings").open(&mut open).default_width(620.).show(ctx,|ui|{
            ui.label("Settings are saved in this browser. Use the same tab-separated configuration format as the native application.");
            ui.add(egui::TextEdit::multiline(&mut self.settings_text).code_editor().desired_rows(8).desired_width(f32::INFINITY));
            ui.separator();ui.label("Startup JavaScript");
            ui.label("Runs when the viewer opens. Saved scenes and the cuemol scripting API are available.");
            ui.add(egui::TextEdit::multiline(&mut self.startup_text).code_editor().desired_rows(10).desired_width(f32::INFINITY));
            ui.horizontal(|ui|{
                if ui.button("Save").clicked(){
                    let previous=self.state.clone();
                    match self.state.load_config_text(&self.settings_text){
                        Ok(_)=>{if let Some(storage)=web_sys::window().and_then(|w|w.local_storage().ok().flatten()){
                            let result=storage.set_item("cuemol.config",&self.settings_text).and_then(|_|storage.set_item("cuemol.startup",&self.startup_text));
                            self.message=result.map(|_|"Settings saved.".to_string()).unwrap_or_else(js_error);
                        }else{self.message="Browser storage is unavailable.".into();}}
                        Err(e)=>{self.state=previous;self.message=e.to_string();}
                    }
                }
                if ui.button("Run startup now").clicked(){self.call("execute_script",json!({"source":self.startup_text}));}
            });
        });
        self.show_settings = open;
    }
    pub(super) fn tools_ui(&mut self, ctx: &egui::Context) {
        if !self.show_console {
            return;
        }
        let mut open = true;
        egui::Window::new("Commands")
            .open(&mut open)
            .default_width(600.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Filter");
                    ui.text_edit_singleline(&mut self.tool_filter);
                });
                egui::ComboBox::from_id_salt("command-selector")
                    .selected_text(&self.console_tool)
                    .width(350.)
                    .show_ui(ui, |ui| {
                        let filter = self.tool_filter.to_lowercase();
                        for tool in cuemol_command::tools_list() {
                            let name = tool["name"].as_str().unwrap();
                            if filter.is_empty()
                                || name.contains(&filter)
                                || tool["description"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_lowercase()
                                    .contains(&filter)
                            {
                                ui.selectable_value(&mut self.console_tool, name.into(), name);
                            }
                        }
                    });
                let Some(tool) = cuemol_command::tools_list()
                    .iter()
                    .find(|t| t["name"] == self.console_tool)
                else {
                    return;
                };
                ui.label(tool["description"].as_str().unwrap_or(""));
                let props = tool["inputSchema"]["properties"].as_object().unwrap();
                let required = tool["inputSchema"]["required"].as_array();
                if self.form_tool != self.console_tool {
                    self.form_tool = self.console_tool.clone();
                    self.form_values.clear();
                    for (key, spec) in props {
                        let is_required = required.is_some_and(|r| r.iter().any(|v| v == key));
                        let kind = spec["type"]
                            .as_str()
                            .or_else(|| spec["type"][0].as_str())
                            .unwrap_or("string");
                        let value = if matches!(
                            key.as_str(),
                            "objId" | "nodeId" | "movId" | "refId" | "fromId" | "toId"
                        ) {
                            self.selected
                                .map(|id| id.get().to_string())
                                .unwrap_or_default()
                        } else if key == "selection" {
                            if self.selection.is_empty() {
                                "*".into()
                            } else {
                                self.selection.clone()
                            }
                        } else if key == "nodeType" {
                            "object".into()
                        } else if is_required {
                            if let Some(value) = spec["enum"]
                                .as_array()
                                .and_then(|v| v.first())
                                .and_then(Value::as_str)
                            {
                                value.into()
                            } else {
                                match kind {
                                    "boolean" => "false".into(),
                                    "number" | "integer" => "0".into(),
                                    "array" => "[]".into(),
                                    _ => String::new(),
                                }
                            }
                        } else {
                            String::new()
                        };
                        self.form_values.insert(key.clone(), value);
                    }
                }
                egui::ScrollArea::vertical()
                    .max_height(360.)
                    .show(ui, |ui| {
                        egui::Grid::new("command-fields")
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                for (key, spec) in props {
                                    let is_required =
                                        required.is_some_and(|r| r.iter().any(|v| v == key));
                                    ui.label(format!(
                                        "{key}{}",
                                        if is_required { " *" } else { "" }
                                    ))
                                    .on_hover_text(spec["description"].as_str().unwrap_or(""));
                                    let value = self.form_values.get_mut(key).unwrap();
                                    if let Some(choices) = spec["enum"].as_array() {
                                        egui::ComboBox::from_id_salt(key)
                                            .selected_text(if value.is_empty() {
                                                "Default"
                                            } else {
                                                value.as_str()
                                            })
                                            .show_ui(ui, |ui| {
                                                if !is_required {
                                                    ui.selectable_value(
                                                        value,
                                                        String::new(),
                                                        "Default",
                                                    );
                                                }
                                                for choice in
                                                    choices.iter().filter_map(Value::as_str)
                                                {
                                                    ui.selectable_value(
                                                        value,
                                                        choice.into(),
                                                        choice,
                                                    );
                                                }
                                            });
                                    } else {
                                        ui.text_edit_singleline(value);
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                if ui.button("Run").clicked() {
                    let mut args = serde_json::Map::new();
                    let mut error = None;
                    for (key, spec) in props {
                        let value = &self.form_values[key];
                        let required = required.is_some_and(|r| r.iter().any(|v| v == key));
                        if value.is_empty() && !required {
                            continue;
                        }
                        let kind = spec["type"]
                            .as_str()
                            .or_else(|| spec["type"][0].as_str())
                            .unwrap_or("string");
                        let parsed = if kind == "string" {
                            Ok(json!(value))
                        } else {
                            serde_json::from_str(value)
                        };
                        match parsed {
                            Ok(v) => {
                                args.insert(key.clone(), v);
                            }
                            Err(e) => {
                                error = Some(format!("{key}: {e}"));
                                break;
                            }
                        }
                    }
                    if let Some(error) = error {
                        self.message = error;
                    } else {
                        self.console_args = json!(args).to_string();
                        self.call(&self.console_tool.clone(), json!(args));
                    }
                }
                ui.collapsing("JSON arguments", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.console_args)
                            .code_editor()
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    if ui.button("Run JSON").clicked() {
                        match serde_json::from_str(&self.console_args) {
                            Ok(args) => self.call(&self.console_tool.clone(), args),
                            Err(e) => self.message = e.to_string(),
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(150.)
                    .show(ui, |ui| {
                        ui.monospace(&self.message);
                    });
            });
        self.show_console = open;
    }
    pub(super) fn timeline_ui(&mut self, ctx: &egui::Context) {
        if !self.show_timeline {
            return;
        }
        let mut open = true;
        egui::Window::new("Animation")
            .open(&mut open)
            .default_width(500.)
            .show(ctx, |ui| {
                let data = cuemol_command::call_tool(
                    &mut self.state,
                    &mut self.camera,
                    "list_anims",
                    &json!({}),
                )
                .unwrap_or(Value::Null);
                ui.horizontal(|ui| {
                    for action in ["play", "pause", "stop"] {
                        if ui.button(action).clicked() {
                            self.call("animate", json!({"action":action}));
                        }
                    }
                    if ui.button("Add element…").clicked() {
                        self.select_tool("add_anim");
                    }
                });
                let duration = data["mgr"]["lengthMs"].as_f64().unwrap_or(0.);
                let mut time = data["mgr"]["elapsedMs"].as_f64().unwrap_or(0.);
                if ui
                    .add(egui::Slider::new(&mut time, 0.0..=duration.max(1.)).suffix(" ms"))
                    .changed()
                {
                    self.call("animate", json!({"action":"seek","timeMs":time}));
                }
                for motion in data["elements"].as_array().into_iter().flatten() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{}  {}–{} ms",
                            motion["name"].as_str().unwrap_or(""),
                            motion["absStartMs"],
                            motion["absEndMs"]
                        ));
                        if ui.small_button("Time…").clicked() {
                            self.select_tool("set_anim_time");
                        }
                        if ui.small_button("×").clicked() {
                            self.call(
                                "remove_anim",
                                json!({"element":format!("#{}",motion["uid"])}),
                            );
                        }
                    });
                }
            });
        self.show_timeline = open;
    }
}
