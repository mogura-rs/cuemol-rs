use super::*;

impl App {
    pub(super) fn agent_ui(&mut self, ctx: &egui::Context) {
        if self.agent.running() && self.agent.revision != self.revision {
            self.agent
                .cancel("Scene changed outside the agent; turn cancelled.");
        }
        while let Some(cuemol_ui::AgentCall {
            name,
            arguments,
            reply,
            download,
        }) = self.agent.poll(ctx)
        {
            if reply.is_closed() {
                continue;
            }
            if let Err(error) = cuemol_command::validate_args(&name, &arguments) {
                let _ = reply.try_send(Err(error));
                continue;
            }
            if let Some((id, trajectory)) = &self.trajectory {
                if name == "list_trajectories" {
                    let _=reply.try_send(Ok(json!({"trajectories":[{"objectId":id.get(),"frames":trajectory.frames,"frame":self.frame}]})));
                    continue;
                }
                if name == "set_frame" && arguments["objId"].as_u64() == Some(id.get()) {
                    let frame = arguments["frame"].as_u64().unwrap_or(u64::MAX);
                    if frame >= trajectory.frames as u64 {
                        let _ = reply.try_send(Err("Frame out of range".into()));
                        continue;
                    }
                    let (id, trajectory, revision) = (*id, trajectory.clone(), self.revision);
                    spawn_local(async move {
                        let result = trajectory.frame(frame as usize).await.map_err(js_error);
                        enqueue(Event::AgentFrame(
                            id,
                            frame as usize,
                            result,
                            reply,
                            revision,
                        ));
                    });
                    continue;
                }
            }
            if matches!(
                name.as_str(),
                "render_image" | "get_screenshot" | "capture_view"
            ) {
                match cuemol_command::validate_args(&name, &arguments).and_then(|()| {
                    cuemol_command::prepare_render(&self.state, &self.camera, &name, &arguments)
                        .map_err(|e| e.to_string())
                }) {
                    Ok(render) => {
                        let cancel = self.agent.operation_cancel();
                        let filename = (name == "render_image").then(|| {
                            arguments["fileName"]
                                .as_str()
                                .unwrap_or("cuemol.png")
                                .to_string()
                        });
                        spawn_local(async move {
                            let mut result = render
                                .run_async_with_cancel(Some(&cancel))
                                .await
                                .map_err(|e| e.to_string());
                            if reply.is_closed() {
                                return;
                            }
                            if let (Some(filename), Ok(value)) = (filename, result.as_ref()) {
                                result = base64::engine::general_purpose::STANDARD
                                    .decode(value["_image"].as_str().unwrap_or(""))
                                    .map_err(|e| e.to_string())
                                    .and_then(|bytes| {
                                        super::download(&filename, &bytes, "image/png")
                                            .map_err(js_error)
                                    })
                                    .map(|()| value.clone());
                            }
                            let _ = reply.send(result).await;
                        });
                    }
                    Err(error) => {
                        let _ = reply.try_send(Err(error));
                    }
                }
                continue;
            }
            let mutating = cuemol_command::tool_is_mutating(&name);
            let undo = (mutating && !self.agent.changed_scene).then(|| self.capture_history());
            let result = if let Some(download) = download {
                download.and_then(|data| {
                    cuemol_command::load_fetched(
                        &mut self.state,
                        &mut self.camera,
                        &data.plan,
                        &data.bytes,
                    )
                    .map_err(|e| e.to_string())
                })
            } else {
                cuemol_command::call_tool(&mut self.state, &mut self.camera, &name, &arguments)
                    .map_err(|e| e.to_string())
            };
            if result.is_ok() && mutating {
                if let Some(undo) = undo {
                    self.history.push(undo);
                    self.future.clear();
                    if self.history.len() > 12 {
                        self.history.remove(0);
                    }
                }
                self.agent.changed_scene = true;
                self.dirty = true;
                self.revision = self.revision.wrapping_add(1);
                self.agent.revision = self.revision;
            }
            let _ = reply.try_send(result);
        }
        if let Some(prompt) = self.agent.show(ctx) {
            self.agent.start(
                &mut self.state,
                &mut self.camera,
                prompt,
                ctx,
                self.revision,
            );
        }
        if self.agent.take_settings_changed() {
            if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten())
            {
                let _ = storage.set_item("cuemol.agent", &self.agent.settings_json());
            }
        }
    }

    pub(super) fn render_ui(&mut self, ctx: &egui::Context) {
        if !self.show_render {
            return;
        }
        let mut open = true;
        let mut render = false;
        egui::Window::new("Render image")
            .open(&mut open)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut self.render_size[0]).range(1..=8192));
                    ui.label("Height");
                    ui.add(egui::DragValue::new(&mut self.render_size[1]).range(1..=8192));
                });
                ui.checkbox(&mut self.render_hatching, "Hatched illustration");
                ui.checkbox(&mut self.render_transparent, "Transparent background");
                if self.render_hatching {
                    cuemol_ui::hatch_controls(ui, &mut self.render_hatch);
                }
                cuemol_ui::lighting_controls(ui, &mut self.render_lighting, self.render_hatching);
                render = ui.button("Render PNG").clicked();
            });
        self.show_render = open;
        if render {
            let backend = if self.render_hatching {
                "umbreon_npr"
            } else {
                "umbreon"
            };
            let mut settings = format!(
                "backend={backend}; width={}; height={}; unit=px; transparentBg={}",
                self.render_size[0], self.render_size[1], self.render_transparent
            );
            settings.push_str("; ");
            settings.push_str(&cuemol_command::rendering_settings(
                &self.render_lighting,
                backend,
            ));
            if self.render_hatching {
                settings.push_str("; ");
                settings.push_str(&cuemol_command::hatch_settings(&self.render_hatch));
            }
            match cuemol_command::call_tool(
                &mut self.state,
                &mut self.camera,
                "set_render_settings",
                &json!({"settings":settings}),
            ) {
                Ok(_) => self.export_png(),
                Err(e) => self.message = e.to_string(),
            }
        }
    }

    pub(super) fn select_tool(&mut self, name: &str) {
        self.console_tool = name.into();
        self.form_tool.clear();
        self.show_console = true;
    }
    pub(super) fn open_properties(&mut self, kind: &str, id: Option<u64>) {
        self.inspector_target = Some(cuemol_ui::InspectorTarget {
            kind: kind.into(),
            id,
        });
        self.inspector.visible = true;
    }
    pub(super) fn settings_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        egui::Window::new("Settings").open(&mut open).default_width(620.).show(ctx,|ui|{
            if let Some(scene) = self.state.active_scene() {
                let mut proof = self.state.scene_manager().scene(scene).and_then(|s| s.scene_attribute("use_colproof")).is_some_and(|v| v == "true");
                if ui.checkbox(&mut proof, "Use color proofing").changed() {
                    self.call("set_node_prop", json!({"nodeType":"scene", "prop":"use_colproof", "value":proof.to_string()}));
                }
                ui.label(format!("ICC profile: {}", self.state.config_value("color:icc_profile")));
                if ui.button("Import ICC profile…").clicked() { choose_file(); }
            }
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
                self.timeline_contents(ui);
            });
        self.show_timeline = open;
    }
    pub(super) fn timeline_contents(&mut self, ui: &mut egui::Ui) {
        let data =
            cuemol_command::call_tool(&mut self.state, &mut self.camera, "list_anims", &json!({}))
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
    }
}
