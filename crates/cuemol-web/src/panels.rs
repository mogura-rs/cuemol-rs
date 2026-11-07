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
            if name == "list_trajectories" {
                let _ = reply.try_send(Ok(self.trajectory_list()));
                continue;
            }
            if name == "set_frame" {
                if let Some(id) = arguments["objId"].as_u64().map(Uid::new) {
                    if let Some(group) = self.trajectories.get(&id) {
                        let frame = arguments["frame"]
                            .as_u64()
                            .and_then(|v| usize::try_from(v).ok())
                            .unwrap_or(usize::MAX);
                        let native_count = self.state.trajectory_len(id);
                        if frame >= native_count + group.frame_count() {
                            let _ = reply.try_send(Err("Frame out of range".into()));
                            continue;
                        }
                        if let Some((source, local_index)) = frame
                            .checked_sub(native_count)
                            .and_then(|index| group.source(index))
                        {
                            let revision = self.revision;
                            let cancelled = self.agent.operation_cancel();
                            spawn_local(async move {
                                let result = source.frame(local_index).await.map_err(js_error);
                                enqueue(Event::AgentFrame(
                                    id, frame, result, reply, revision, cancelled,
                                ));
                            });
                            continue;
                        }
                    }
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
            let mutating = cuemol_command::tool_changes_scene(&name);
            let undo = (mutating && (!self.agent.changed_scene || name == "execute_script"))
                .then(|| self.capture_history());
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
            if result.is_ok() && name == "set_frame" {
                if let (Some(id), Some(frame)) =
                    (arguments["objId"].as_u64(), arguments["frame"].as_u64())
                {
                    self.set_trajectory_frame_index(Uid::new(id), frame as usize);
                }
            }
            let changed = name != "execute_script"
                || undo.as_ref().is_none_or(|before| {
                    before.camera != self.camera || !before.state.same_undo_state(&self.state)
                });
            if result.is_ok() && mutating && changed {
                let history_changed = undo.as_ref().is_some_and(|undo| {
                    undo.camera != self.camera || !undo.state.same_scene_undo_state(&self.state)
                });
                if let Some(mut undo) =
                    undo.filter(|_| history_changed && !self.agent.changed_scene)
                {
                    cuemol_command::retain_animation_seed_in_undo(
                        &mut undo.state,
                        &self.state,
                        &name,
                    );
                    self.state.mark_scene_changes_since(&undo.state);
                    self.history.push(undo);
                    self.future.clear();
                    if self.history.len() > 12 {
                        self.history.remove(0);
                    }
                }
                self.agent.changed_scene |= history_changed;
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
        if let Some(history) = self.agent.take_prompt_history_changed() {
            if let Some(storage) =
                web_sys::window().and_then(|window| window.local_storage().ok().flatten())
            {
                let _ = storage.set_item("cuemol.agent.promptHistory", &history);
            }
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
        let action = self.render_panel.show(
            ctx,
            &mut open,
            self.render_cancel.is_some(),
            cuemol_ui::RenderControls {
                size: &mut self.render_size,
                transparent: &mut self.render_transparent,
                hatching: &mut self.render_hatching,
                lighting: &mut self.render_lighting,
                hatch: &mut self.render_hatch,
            },
        );
        self.ui_rects.extend(self.render_panel.rects.clone());
        self.show_render = open;
        if action == cuemol_ui::RenderAction::Stop {
            self.call("cancel_render", json!({}));
        }
        if action == cuemol_ui::RenderAction::Save {
            if let Some(bytes) = &self.render_panel.png {
                if let Err(error) = download("cuemol.png", bytes, "image/png") {
                    self.message = js_error(error);
                }
            }
        }
        let render = action == cuemol_ui::RenderAction::Start;
        if render {
            let backend = if self.render_hatching {
                "umbreon_npr"
            } else {
                "umbreon"
            };
            let mut settings = format!(
                "backend={backend}; {}; transparentBg={}; projection={}; clipPlane={}",
                self.render_panel.image_settings(self.render_size),
                self.render_transparent,
                if self.render_lighting.perspective {
                    "perspective"
                } else {
                    "orthographic"
                },
                self.render_lighting.use_clip_z
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
                Ok(_) => {
                    self.render_panel.begin(ctx);
                    self.render_async(
                        "get_screenshot",
                        &json!({"width":self.render_size[0],"height":self.render_size[1]}),
                        None,
                        true,
                    );
                }
                Err(e) => self.message = e.to_string(),
            }
        }
    }

    pub(super) fn select_tool(&mut self, name: &str) {
        if let Some(kind) = cuemol_ui::surface::Kind::from_command(name) {
            self.surface.open(kind, &self.state, self.selected);
            return;
        }
        if let Some(kind) = cuemol_ui::molecule_edit::Kind::from_command(name) {
            self.molecule_edits.open(kind, &self.state, self.selected);
            return;
        }
        if name == "analyze_interactions" {
            self.interaction.opened(&self.state, self.selected);
            self.show_interaction = true;
            return;
        }
        if name == "superpose" {
            self.superposition.opened(&self.state);
            self.show_superposition = true;
            return;
        }
        if name == "set_secondary_structure" {
            self.secondary.opened(&self.state, self.selected);
            self.show_secondary = true;
            return;
        }
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
        if !self.show_settings {
            self.settings_draft = None;
            return;
        }
        let mut open = true;
        let mut draft = self.settings_draft.take().unwrap_or_else(|| {
            cuemol_desktop_core::preferences::Preferences::from_state(&self.state)
        });
        let previous = draft.clone();
        let mut panel = std::mem::take(&mut self.settings_panel);
        let mut actions = Vec::new();
        let mut reloaded = false;
        let window=egui::Window::new("Settings").open(&mut open)
            .default_size([800.,580.])
            .max_width((ctx.content_rect().width()-40.).max(320.)).min_size([600.,320.])
            .max_height((ctx.content_rect().height()-50.).max(180.))
            .show(ctx,|ui| {
                actions=panel.show(ui,&mut draft,true,|category,ui|match category {
                    cuemol_ui::settings::Category::Agent=>self.agent.settings_ui(ui),
                    cuemol_ui::settings::Category::Rendering=> {
                        if let Some(scene)=self.state.active_scene() {
                            let mut proof=self.state.scene_manager().scene(scene).and_then(|s|s.scene_attribute("use_colproof")).is_some_and(|v|v=="true");
                            if ui.checkbox(&mut proof,"Use color proofing").changed() {self.call("set_node_prop",json!({"nodeType":"scene","prop":"use_colproof","value":proof.to_string()}));}
                            ui.label(format!("ICC profile: {}",self.state.config_value("color:icc_profile")));
                            if ui.button("Import ICC profile…").clicked() {choose_file();}
                        }
                    }
                    cuemol_ui::settings::Category::Advanced=> {
                        ui.label("Settings are saved in this browser. The configuration format is shared with the native application.");
                        ui.add(egui::TextEdit::multiline(&mut self.settings_text).code_editor().desired_rows(8).desired_width(f32::INFINITY));
                        ui.separator();ui.label("Startup JavaScript");
                        ui.label("Runs when the viewer opens. Saved scenes and the cuemol scripting API are available.");
                        ui.add(egui::TextEdit::multiline(&mut self.startup_text).code_editor().desired_rows(8).desired_width(f32::INFINITY));
                        ui.horizontal(|ui| {
                            let save=ui.button("Save");self.record_ui_rect("Settings Save",&save);
                            if save.clicked() {
                                let previous=self.state.clone();
                                match self.state.load_config_text(&self.settings_text) {
                                    Ok(_)=> {
                                        reloaded=true;
                                        self.agent.settings.enabled=self.state.plugin_enabled("agent");
                                        if !self.agent.settings.enabled {self.agent.cancel("AI Agent plugin disabled.");}
                                        self.dark=self.state.config_value("layout:theme")!="light";
                                        self.dirty=true;
                                        if let Some(storage)=web_sys::window().and_then(|w|w.local_storage().ok().flatten()) {
                                            self.message=storage.set_item("cuemol.config",&self.settings_text).and_then(|_|storage.set_item("cuemol.startup",&self.startup_text)).map(|_|"Settings saved.".into()).unwrap_or_else(js_error);
                                        } else {self.message="Browser storage is unavailable.".into();}
                                    }
                                    Err(error)=>{self.state=previous;self.message=error.to_string();}
                                }
                            }
                            if ui.button("Run startup now").clicked() {self.call("execute_script",json!({"source":self.startup_text}));}
                        });
                    }
                    _=>{}
                });
            });
        for (label, rect) in &panel.rects {
            self.ui_rects.insert(label.clone(), *rect);
        }
        self.settings_panel = panel;
        if draft != previous {
            match draft.apply(&mut self.state) {
                Ok(()) => {
                    self.dark = draft.dark_mode;
                    self.agent.settings.enabled = self.state.plugin_enabled("agent");
                    if !self.agent.settings.enabled {
                        self.agent.cancel("AI Agent plugin disabled.");
                    }
                    self.settings_text = self.state.config_text();
                    self.dirty = true;
                    if let Some(storage) =
                        web_sys::window().and_then(|w| w.local_storage().ok().flatten())
                    {
                        if let Err(error) = storage
                            .set_item("cuemol.config", &self.settings_text)
                            .and_then(|_| {
                                storage.set_item(
                                    "cuemol.theme",
                                    if self.dark { "dark" } else { "light" },
                                )
                            })
                        {
                            self.message = js_error(error);
                        }
                    }
                }
                Err(error) => self.message = error,
            }
        }
        if let Some(window) = window {
            self.record_ui_rect("Settings window", &window.response);
        }
        for action in actions {
            if action == cuemol_ui::settings::Action::CheckUpdates {
                if let Some(window) = web_sys::window() {
                    let _ = window.location().reload();
                }
            }
        }
        self.settings_draft = if open && !reloaded { Some(draft) } else { None };
        self.show_settings = open;
    }
    pub(super) fn molecule_edit_ui(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.molecule_edits.active else {
            return;
        };
        if self.molecule_edits.draft().scene != self.state.active_scene() {
            self.molecule_edits.active = None;
            self.message = "The active scene changed. Reopen the molecule edit dialog.".into();
            return;
        }
        let mut open = true;
        let mut action = None;
        let window = egui::Window::new(kind.title())
            .open(&mut open)
            .collapsible(false)
            .default_width(550.)
            .show(ctx, |ui| {
                action = self
                    .molecule_edits
                    .draft_mut()
                    .show(ui, &self.state, self.revision);
            });
        self.ui_rects
            .extend(self.molecule_edits.draft().rects.clone());
        if let Some(window) = window {
            self.record_ui_rect("Molecule edit window", &window.response);
        }
        match action {
            Some(cuemol_ui::molecule_edit::Action::Accept) => {
                let draft = self.molecule_edits.draft();
                let args = draft
                    .validation_error(&self.state)
                    .map_or_else(|| draft.arguments(), Err);
                match args {
                    Ok(args) => {
                        if self.call_shared(kind.command(), args) {
                            let count = serde_json::from_str::<serde_json::Value>(&self.message)
                                .ok()
                                .and_then(|v| v[kind.result_key()].as_u64())
                                .unwrap_or(0);
                            if kind == cuemol_ui::molecule_edit::Kind::Chain || count > 0 {
                                let draft = self.molecule_edits.draft();
                                self.selected = if kind == cuemol_ui::molecule_edit::Kind::Merge {
                                    draft.target
                                } else {
                                    draft.object
                                };
                                self.state.remember_selection(&draft.selection);
                                self.persist_selection_history();
                                self.message = format!("{}: {count} atom(s)", kind.title());
                                open = false;
                            } else {
                                self.message = kind.no_result().into();
                                self.molecule_edits
                                    .draft_mut()
                                    .error
                                    .clone_from(&self.message);
                            }
                        } else {
                            self.molecule_edits
                                .draft_mut()
                                .error
                                .clone_from(&self.message);
                        }
                    }
                    Err(error) => self.molecule_edits.draft_mut().error = error.into(),
                }
            }
            Some(cuemol_ui::molecule_edit::Action::Cancel) => open = false,
            None => {}
        }
        if !open {
            self.molecule_edits.active = None;
        }
    }

    pub(super) fn surface_ui(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.surface.active else {
            return;
        };
        if self.surface.scene != self.state.active_scene() {
            self.surface.close();
            self.message = "The active scene changed. Reopen the surface dialog.".into();
            return;
        }
        let mut open = true;
        let mut action = None;
        let window = egui::Window::new(kind.title())
            .open(&mut open)
            .collapsible(false)
            .default_width(530.)
            .show(ctx, |ui| {
                action = self.surface.show(ui, &self.state, self.revision);
            });
        self.ui_rects.extend(self.surface.rects.clone());
        if let Some(window) = window {
            self.record_ui_rect("Surface window", &window.response);
        }
        match action {
            Some(cuemol_ui::surface::Action::Accept) => {
                if let Some(error) = self.surface.validation_error(&self.state) {
                    self.surface.error = error.into();
                } else if self.call_shared(kind.command(), self.surface.arguments(&self.state)) {
                    self.surface.remember(&mut self.state);
                    self.persist_selection_history();
                    self.message = format!("{} applied", kind.title());
                    open = false;
                } else {
                    self.surface.error.clone_from(&self.message);
                }
            }
            Some(cuemol_ui::surface::Action::Cancel) => open = false,
            None => {}
        }
        if !open {
            self.surface.close();
        }
    }

    pub(super) fn superposition_ui(&mut self, ctx: &egui::Context) {
        if !self.show_superposition {
            return;
        }
        if self.superposition.scene != self.state.active_scene() {
            self.show_superposition = false;
            self.message = "The active scene changed. Reopen Molecular superposition.".into();
            return;
        }
        let mut open = true;
        let mut action = None;
        let window = egui::Window::new("Molecular superposition")
            .open(&mut open)
            .collapsible(false)
            .default_width(550.)
            .show(ctx, |ui| {
                action = self
                    .superposition
                    .show(ui, &self.state, self.revision, false);
            });
        self.ui_rects.extend(self.superposition.rects.clone());
        if let Some(window) = window {
            self.record_ui_rect("Superpose window", &window.response);
        }
        match action {
            Some(cuemol_ui::superposition::Action::Accept) => {
                if let Some(error) = self.superposition.validation_error(&self.state) {
                    self.superposition.error = error.into();
                } else if self.call_shared("superpose", self.superposition.arguments()) {
                    self.superposition.remember(&mut self.state);
                    self.persist_selection_history();
                    self.settings_text = self.state.config_text();
                    if let Some(storage) =
                        web_sys::window().and_then(|w| w.local_storage().ok().flatten())
                    {
                        let _ = storage.set_item("cuemol.config", &self.settings_text);
                    }
                    self.selected = self.superposition.moving;
                    self.message = "Molecular superposition applied".into();
                    open = false;
                } else {
                    self.superposition.error.clone_from(&self.message);
                }
            }
            Some(cuemol_ui::superposition::Action::Cancel) => open = false,
            None => {}
        }
        self.show_superposition = open;
    }

    pub(super) fn secondary_ui(&mut self, ctx: &egui::Context) {
        if !self.show_secondary {
            return;
        }
        if self.secondary.scene != self.state.active_scene() {
            self.show_secondary = false;
            self.message = "The active scene changed. Reopen Reassign secondary structure.".into();
            return;
        }
        let mut open = true;
        let mut action = None;
        let window = egui::Window::new("Reassign secondary structure")
            .open(&mut open)
            .collapsible(false)
            .default_width(550.)
            .show(ctx, |ui| {
                action = self.secondary.show(ui, &self.state, self.revision);
            });
        self.ui_rects.extend(self.secondary.rects.clone());
        if let Some(window) = window {
            self.record_ui_rect("Secondary window", &window.response);
        }
        match action {
            Some(cuemol_ui::secondary::Action::Accept) => {
                if let Some(error) = self.secondary.validation_error(&self.state) {
                    self.secondary.error = error.into();
                } else if self.call_shared("set_secondary_structure", self.secondary.arguments()) {
                    if self.secondary.assign {
                        self.state.remember_selection(&self.secondary.selection);
                        self.persist_selection_history();
                    }
                    self.selected = self.secondary.object;
                    self.message = "Secondary structure assigned".into();
                    open = false;
                } else {
                    self.secondary.error.clone_from(&self.message);
                }
            }
            Some(cuemol_ui::secondary::Action::Cancel) => open = false,
            None => {}
        }
        self.show_secondary = open;
    }

    pub(super) fn interaction_ui(&mut self, ctx: &egui::Context) {
        if !self.show_interaction {
            return;
        }
        if self.interaction.scene != self.state.active_scene() {
            self.show_interaction = false;
            self.message = "The active scene changed. Reopen Interaction analysis.".into();
            return;
        }
        let mut open = true;
        let mut action = None;
        let window = egui::Window::new("Interaction analysis")
            .open(&mut open)
            .collapsible(false)
            .default_width(550.)
            .show(ctx, |ui| {
                action = self.interaction.show(ui, &self.state, self.revision);
            });
        self.ui_rects.extend(self.interaction.rects.clone());
        if let Some(window) = window {
            self.record_ui_rect("Interaction window", &window.response);
        }
        match action {
            Some(cuemol_ui::interaction::Action::Accept) => {
                let args = self.interaction.arguments();
                if self.call_shared("analyze_interactions", args) {
                    let count = serde_json::from_str::<serde_json::Value>(&self.message)
                        .ok()
                        .and_then(|v| v["count"].as_u64())
                        .unwrap_or(0);
                    if count > 0 {
                        for selection in [
                            Some(self.interaction.selection1.clone()),
                            self.interaction.second_selection().map(str::to_owned),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            self.state.remember_selection(&selection);
                        }
                        self.persist_selection_history();
                        self.selected = self.interaction.object;
                        self.message = format!("Defined {count} interaction(s)");
                        open = false;
                    } else {
                        self.message = "No interaction was found".into();
                        self.interaction.error.clone_from(&self.message);
                    }
                } else {
                    self.interaction.error.clone_from(&self.message);
                }
            }
            Some(cuemol_ui::interaction::Action::Cancel) => open = false,
            None => {}
        }
        self.show_interaction = open;
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
                let run = ui.button("Run");
                self.record_ui_rect("Command Run", &run);
                if run.clicked() {
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
        let previous = self.animation_panel.selected;
        let output = self
            .animation_panel
            .show_state(ui, &self.state, self.revision);
        self.ui_rects.extend(self.animation_panel.rects.clone());
        for (tool, args) in output.commands {
            self.call(tool, args);
        }
        if previous != self.animation_panel.selected || output.inspect.is_some() {
            self.inspector_target =
                self.animation_panel
                    .selected
                    .map(|uid| cuemol_ui::InspectorTarget {
                        kind: "animation".into(),
                        id: Some(uid),
                    });
            self.animation_inspector.visible = self.animation_panel.selected.is_some();
        }
    }
}
