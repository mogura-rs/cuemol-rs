use super::*;
use cuemol_ui::shell::{self, Activity, Icon};

impl App {
    pub(super) fn shell_shortcut(&mut self, action: &str) {
        if self.pending_close.is_some() {
            return;
        }
        match action {
            "new-tab" => self.call("create_scene", json!({})),
            "close-tab" => {
                if let Some(scene) = self.state.active_scene() {
                    self.request_close_tab(scene);
                }
            }
            "save-scene" | "save-scene-as" => {
                self.save_scene_download();
            }
            "reload-scene" => self.reload_scene(),
            "undo" | "redo" => self.call(action, json!({})),
            "settings" => {
                self.settings_text = self.state.config_text();
                self.show_settings = true;
            }
            "toggle-palette" => {
                let collapsed = self.state.config_value("layout:tool-palette-collapsed") != "true";
                self.state
                    .put_config_value("layout:tool-palette-collapsed", collapsed.to_string());
                self.settings_text = self.state.config_text();
                if let Some(storage) =
                    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
                {
                    let _ = storage.set_item("cuemol.config", &self.settings_text);
                }
            }
            _ => {}
        }
    }

    fn request_close_tab(&mut self, id: Uid) {
        if !self.state.scene_is_just_created(id) {
            self.pending_close = Some(id);
        } else {
            self.close_tab(id);
        }
    }

    fn close_tab(&mut self, id: Uid) {
        let active = self.state.active_scene() == Some(id);
        self.call(
            "close_scene",
            json!({"sceneId":id.get(),"discardChanges":true}),
        );
        if self.state.scene_manager().scene(id).is_none() {
            self.trajectories
                .retain(|uid, _| self.state.scene_manager().object(*uid).is_some());
            if active {
                self.selected = None;
                self.selection.clear();
                self.camera = self.state.active_view_camera().unwrap_or_default();
            }
            self.playing = false;
            self.dirty = true;
        }
    }

    fn close_tab_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.pending_close else {
            return;
        };
        let Some(scene) = self.state.scene_manager().scene(id) else {
            self.pending_close = None;
            return;
        };
        let name = scene.name().to_string();
        let response = egui::Modal::new(egui::Id::new("close-scene-dialog")).show(ctx, |ui| {
            ui.heading(format!("Close {name}?"));
            ui.label("Save a scene download to keep its objects, renderers and cameras.");
            ui.horizontal(|ui| {
                let save = ui.button("Save and close");
                self.record_ui_rect("Save and close tab", &save);
                if save.clicked() {
                    if self.state.active_scene() != Some(id) {
                        self.call("switch_scene", json!({"sceneId":id.get()}));
                    }
                    if self.save_scene_download() {
                        self.pending_close = None;
                        self.close_tab(id);
                    }
                }
                let discard = ui.button("Close without saving");
                self.record_ui_rect("Close without saving", &discard);
                if discard.clicked() {
                    self.pending_close = None;
                    self.close_tab(id);
                }
                let cancel = ui.button("Cancel");
                self.record_ui_rect("Cancel closing tab", &cancel);
                if cancel.clicked() {
                    self.pending_close = None;
                }
            });
        });
        if response.should_close() {
            self.pending_close = None;
        }
    }

    pub(super) fn record_ui_rect(&mut self, name: &str, response: &egui::Response) {
        let rect = response.rect;
        self.ui_rects.insert(
            name.into(),
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
        );
    }

    pub(super) fn shell(&mut self, ctx: &egui::Context) {
        shell::theme(ctx, !self.dark);
        self.agent.docked = true;
        self.shell_menu(ctx);
        self.shell_toolbar(ctx);
        egui::TopBottomPanel::bottom("status")
            .exact_height(24.)
            .frame(
                egui::Frame::NONE
                    .fill(if self.dark {
                        egui::Color32::from_rgb(26, 58, 92)
                    } else {
                        egui::Color32::from_rgb(41, 121, 181)
                    })
                    .inner_margin(egui::Margin::symmetric(8, 3)),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(egui::Color32::from_gray(230));
                ui.horizontal(|ui| {
                    if self.source_active {
                        ui.spinner();
                        let response = ui.small_button("Cancel loading");
                        self.record_ui_rect("Cancel loading", &response);
                        if response.clicked() {
                            self.cancel_load();
                        }
                    }
                    if self.render_cancel.is_some() {
                        ui.spinner();
                        if ui.small_button("Cancel render").clicked() {
                            self.call("cancel_render", json!({}));
                        }
                    }
                    ui.add(
                        egui::Label::new(self.message.lines().next().unwrap_or("Ready")).truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!(
                            "{} atoms",
                            self.state
                                .active_object_summaries()
                                .iter()
                                .map(|o| o.atom_count)
                                .sum::<usize>()
                        ));
                    });
                });
            });
        if self.agent.visible {
            self.activity = Some(Activity::Agent);
            self.agent.visible = false;
        }
        let before = self.activity;
        if shell::activity_bar_observed(ctx, &mut self.activity, |activity, rect| {
            self.ui_rects.insert(
                activity.title().into(),
                [rect.min.x, rect.min.y, rect.width(), rect.height()],
            );
        }) {
            self.settings_text = self.state.config_text();
            self.show_settings = true;
        }
        if before != self.activity {
            if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten())
            {
                let _ = storage.set_item(
                    "cuemol.activity",
                    self.activity.map(Activity::key).unwrap_or("none"),
                );
            }
        }
        self.shell_sidebar(ctx);
        if self.activity == Some(Activity::Actions) {
            self.actions_panel.inspector(ctx);
        } else {
            self.inspector.inspect(
                &mut self.state,
                &mut self.camera,
                self.inspector_target.clone(),
                self.revision,
            );
            for (tool, args) in self.inspector.panel(ctx) {
                self.call(tool, args);
            }
        }
        self.shell_bottom(ctx);
        egui::TopBottomPanel::top("scene-tabs")
            .exact_height(30.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for scene in self.state.scene_summaries() {
                        let selected = ui.selectable_label(scene.active, &scene.name);
                        self.record_ui_rect(&format!("Tab {}", scene.uid.get()), &selected);
                        if selected.clicked() {
                            self.call("switch_scene", json!({"sceneId":scene.uid.get()}));
                        }
                        let close = ui.small_button("×").on_hover_text("Close Tab");
                        self.record_ui_rect(&format!("Close tab {}", scene.uid.get()), &close);
                        if close.clicked() {
                            self.request_close_tab(scene.uid);
                        }
                    }
                    if ui.small_button("+").on_hover_text("New Tab").clicked() {
                        self.call("create_scene", json!({}));
                    }
                });
            });
        self.close_tab_dialog(ctx);
        let mut open = self.show_get_pdb;
        egui::Window::new("Get PDB")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("PDB identifier");
                let response = ui.text_edit_singleline(&mut self.pdb_id);
                self.record_ui_rect("Get PDB identifier", &response);
                let response = ui.button("Fetch");
                self.record_ui_rect("Fetch PDB", &response);
                if response.clicked() {
                    self.fetch();
                }
            });
        self.show_get_pdb = open;
    }
    fn shell_menu(&mut self, ctx: &egui::Context) {
        let response = egui::TopBottomPanel::top("menubar")
            .exact_height(26.)
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("New Tab").clicked() {
                            self.call("create_scene", json!({}));
                            ui.close();
                        }
                        if ui.button("Open File…").clicked() {
                            choose_file();
                            ui.close();
                        }
                        if ui.button("Open Scene…").clicked() {
                            choose_file();
                            ui.close();
                        }
                        if ui.button("Get PDB…").clicked() {
                            self.show_get_pdb = true;
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Save Scene").clicked() {
                            self.save_scene_download();
                            ui.close();
                        }
                        if ui.button("Save File As…").clicked() {
                            self.save_object_download();
                            ui.close();
                        }
                        let save_view = ui.button("Save current view…");
                        self.record_ui_rect("Save current view", &save_view);
                        if save_view.clicked() {
                            let size = self.target.as_ref().map_or(self.render_size, |target| target.size);
                            self.render_async("get_screenshot", &json!({"width":size[0],"height":size[1]}), Some("view.png".into()), false);
                            ui.close();
                        }
                        if ui.button("Save Scene As…").clicked() {
                            self.save_scene_download();
                            ui.close();
                        }
                        if ui.button("Reload Scene").clicked() {
                            self.reload_scene();
                            ui.close();
                        }
                        let close = ui.button("Close Tab").on_hover_text("Ctrl/Cmd+Alt+W in browsers");
                        self.record_ui_rect("Close Tab menu", &close);
                        if close.clicked() {
                            self.shell_shortcut("close-tab");
                            ui.close();
                        }
                    });
                    let edit_menu = ui.menu_button("Edit", |ui| {
                        let undo = ui.add_enabled(self.clipboard_focus.text.is_some() || !self.history.is_empty(), egui::Button::new("Undo"));
                        self.record_ui_rect("Edit Undo", &undo);
                        if undo.clicked() {
                            if self.clipboard_focus.text.is_some() { self.clipboard_action(ctx, "undo"); } else { self.call("undo", json!({})); }
                            ui.close();
                        }
                        let redo = ui.add_enabled(self.clipboard_focus.text.is_some() || !self.future.is_empty(), egui::Button::new("Redo"));
                        self.record_ui_rect("Edit Redo", &redo);
                        if redo.clicked() {
                            if self.clipboard_focus.text.is_some() { self.clipboard_action(ctx, "redo"); } else { self.call("redo", json!({})); }
                            ui.close();
                        }
                        for (label, action) in [("Cut", "cut"), ("Copy", "copy"), ("Paste", "paste"), ("Select All", "select-all")] {
                            let response = ui.button(label);
                            self.record_ui_rect(&format!("Edit {label}"), &response);
                            if response.clicked() { self.clipboard_action(ctx, action); ui.close(); }
                        }
                        if ui.button("Clear undo data").clicked() {
                            self.call("clear_undo", json!({}));
                            ui.close();
                        }
                        ui.separator();
                        for (label, command) in [
                            ("Merge molecule…", "merge_molecules"),
                            ("Delete mol atoms…", "delete_atoms"),
                            ("Change chain ID…", "rename_chain"),
                            ("Change residue number…", "renumber_residues"),
                        ] {
                            let button = ui.button(label);
                            self.record_ui_rect(label, &button);
                            if button.clicked() { self.select_tool(command); ui.close(); }
                        }
                    });
                    self.record_ui_rect("Edit menu", &edit_menu.response);
                    let scene_menu = ui.menu_button("Scene", |ui| {
                        ui.menu_button("Background", |ui| {
                            for (label, value) in [("White", "white"), ("Black", "black")] {
                                let button = ui.button(label);
                                self.record_ui_rect(&format!("Scene background {label}"), &button);
                                if button.clicked() {
                                    self.call("set_node_prop", json!({"nodeType":"scene","prop":"bgcolor","value":value}));
                                    ui.close();
                                }
                            }
                        });
                        let mut proof = self.state.active_scene().and_then(|id| self.state.scene_manager().scene(id))
                            .and_then(|scene| scene.scene_attribute("use_colproof")).is_some_and(|value| value == "true");
                        if ui.checkbox(&mut proof, "Use color proofing").changed() {
                            self.call("set_node_prop", json!({"nodeType":"scene","prop":"use_colproof","value":proof.to_string()}));
                        }
                        let properties = ui.button("Properties…");
                        self.record_ui_rect("Scene Properties", &properties);
                        if properties.clicked() { self.open_properties("scene", None); ui.close(); }
                    });
                    self.record_ui_rect("Scene menu", &scene_menu.response);
                    let view_menu = ui.menu_button("View", |ui| {
                        for (value, label) in [(true,"Perspective"),(false,"Orthographic")] {
                            if ui.selectable_label(self.camera.is_perspective() == value, label).clicked() {
                                self.call("set_projection", json!({"perspective":value}));
                                ui.close();
                            }
                        }
                        ui.menu_button("Center mark", |ui| {
                            for (value,label) in [("crosshair","Cross"),("axis","Axis"),("none","None")] {
                                if ui.button(label).clicked() {
                                    self.call("set_node_prop", json!({"nodeType":"view","prop":"centerMark","value":value}));
                                    ui.close();
                                }
                            }
                        });
                        let properties = ui.button("View property…");
                        self.record_ui_rect("View Properties", &properties);
                        if properties.clicked() { self.open_properties("view", None); ui.close(); }
                        ui.separator();
                        if cuemol_ui::navigation::palette_menu(ui, &mut self.state) {
                            self.settings_text = self.state.config_text();
                            if let Some(storage) =
                                web_sys::window().and_then(|w| w.local_storage().ok().flatten())
                            {
                                let _ = storage.set_item("cuemol.config", &self.settings_text);
                            }
                        }
                        ui.separator();
                        for activity in Activity::ALL {
                            if ui.button(activity.title()).clicked() {
                                self.activity = Some(activity);
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.checkbox(&mut self.dark, "Dark theme").changed() {
                            if let Some(storage) =
                                web_sys::window().and_then(|w| w.local_storage().ok().flatten())
                            {
                                let _ = storage.set_item(
                                    "cuemol.theme",
                                    if self.dark { "dark" } else { "light" },
                                );
                            }
                        }
                        ui.menu_button("Stereo", |ui| {
                            for (mode,value,label) in [(cuemol_qsys::StereoMode::None,"none","None"),
                                (cuemol_qsys::StereoMode::Parallel,"para","Parallel"),
                                (cuemol_qsys::StereoMode::Cross,"cross","Cross")] {
                                if ui.selectable_label(self.camera.stereo_mode()==mode,label).clicked() {
                                    self.call("set_node_prop",json!({"nodeType":"view","prop":"stereoMode","value":value}));
                                }
                            }
                            let mut swap=self.state.active_view_options().swap_stereo_eyes;
                            if ui.checkbox(&mut swap,"Swap eyes").changed() {
                                self.call("set_node_prop",json!({"nodeType":"view","prop":"swapStereoEyes","value":swap.to_string()}));
                            }
                            let mut angle=self.camera.stereo_distance();
                            ui.horizontal(|ui| {
                                ui.label("Eye angle (°)");
                                if ui.add(egui::DragValue::new(&mut angle).speed(0.1).range(0.0..=180.0)).changed() {
                                    self.call("set_node_prop",json!({"nodeType":"view","prop":"stereoDist","value":angle.to_string()}));
                                }
                            });
                        });
                        if ui.button("Fit scene").clicked() {
                            self.fit(false);
                            ui.close();
                        }
                    });
                    self.record_ui_rect("View menu", &view_menu.response);
                    ui.menu_button("Rendering", |ui| {
                        if ui.button("Render image…").clicked() {
                            self.open_render();
                            ui.close();
                        }
                        if ui.button("Save PNG").clicked() {
                            self.export_png();
                            ui.close();
                        }
                    });
                    let tools_menu = ui.menu_button("Tools", |ui| {
                        for (label, command) in [
                            ("Molecular superposition…", "superpose"),
                            ("Interaction…", "analyze_interactions"),
                            ("Reassign secondary str…", "set_secondary_structure"),
                            ("Mol morphing animation…", "convert_to_morph"),
                            ("Mol surface generation…", "create_surface"),
                            ("Mol surface cutter…", "cut_surface"),
                            ("APBS elepot calculation…", "calc_elepot"),
                        ] {
                            let button = ui.button(label);
                            self.record_ui_rect(label, &button);
                            if button.clicked() { self.select_tool(command); ui.close(); }
                        }
                        ui.separator();
                        if ui.button("Commands…").clicked() {
                            self.show_console = true;
                            ui.close();
                        }
                        if ui.button("AI Agent").clicked() {
                            self.activity = Some(Activity::Agent);
                            ui.close();
                        }
                        let settings = ui.button("Settings");
                        self.record_ui_rect("Settings", &settings);
                        if settings.clicked() {
                            self.settings_text = self.state.config_text();
                            self.show_settings = true;
                            ui.close();
                        }
                    });
                    self.record_ui_rect("Tools menu", &tools_menu.response);
                    ui.menu_button("Window", |ui| {
                        for tab in [
                            "Console",
                            "Output",
                            "Animation",
                            "Sequence",
                            "MD Trajectory",
                        ] {
                            if ui.button(tab).clicked() {
                                self.bottom_tab = tab.into();
                                self.bottom_open = true;
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Help", |ui| {
                        if ui.button("Mouse and keyboard").clicked() {
                            self.show_help = true;
                            ui.close();
                        }
                        ui.hyperlink_to("CueMol documentation", "https://www.cuemol.org/");
                    });
                });
            });
        self.clipboard_focus.menu(response.response.rect);
    }
    fn shell_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar")
            .exact_height(36.)
            .frame(
                egui::Frame::NONE
                    .fill(if self.dark {
                        egui::Color32::from_rgb(26, 28, 36)
                    } else {
                        egui::Color32::from_rgb(224, 225, 229)
                    })
                    .inner_margin(egui::Margin::symmetric(8, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let compact = ui.available_width() < 1000.;
                    for (label, icon) in [
                        ("New Tab", Icon::New),
                        ("Open File", Icon::File),
                        ("Save As", Icon::Save),
                        ("Open Scene", Icon::Open),
                        ("Reload Scene", Icon::Reload),
                        ("Save Scene", Icon::Save),
                        ("Get PDB", Icon::Fetch),
                        ("Render", Icon::Render),
                        ("Undo", Icon::Undo),
                        ("Redo", Icon::Redo),
                    ] {
                        if matches!(
                            label,
                            "Open File" | "Open Scene" | "Get PDB" | "Render" | "Undo"
                        ) {
                            ui.separator();
                        }
                        if matches!(label, "Undo" | "Redo") {
                            let stack = if label == "Undo" {
                                &self.history
                            } else {
                                &self.future
                            };
                            let descriptions: Vec<_> = stack
                                .iter()
                                .rev()
                                .map(|entry| entry.record.label())
                                .collect();
                            let (steps, rects) =
                                shell::toolbar_history(ui, icon, label, compact, &descriptions);
                            for (name, rect) in rects {
                                self.ui_rects.insert(
                                    name,
                                    [rect.min.x, rect.min.y, rect.width(), rect.height()],
                                );
                            }
                            for _ in 0..steps.unwrap_or(0) {
                                self.call(if label == "Undo" { "undo" } else { "redo" }, json!({}));
                            }
                            continue;
                        }
                        let enabled = match label {
                            "Save As" | "Reload Scene" | "Save Scene" | "Render" => {
                                self.state.active_scene().is_some()
                            }
                            _ => true,
                        };
                        let response = ui
                            .add_enabled_ui(enabled, |ui| {
                                shell::toolbar_button(ui, icon, label, compact)
                            })
                            .inner;
                        self.ui_rects.insert(
                            label.into(),
                            [
                                response.rect.min.x,
                                response.rect.min.y,
                                response.rect.width(),
                                response.rect.height(),
                            ],
                        );
                        if response.clicked() {
                            match label {
                                "New Tab" => self.call("create_scene", json!({})),
                                "Open File" | "Open Scene" => choose_file(),
                                "Save As" => self.save_object_download(),
                                "Save Scene" => {
                                    self.save_scene_download();
                                }
                                "Reload Scene" => self.reload_scene(),
                                "Get PDB" => self.show_get_pdb = true,
                                "Render" => self.open_render(),
                                "Undo" => self.call("undo", json!({})),
                                "Redo" => self.call("redo", json!({})),
                                _ => {}
                            }
                        }
                    }
                });
            });
    }
    fn save_scene_download(&mut self) -> bool {
        if let Err(error) = self.state.set_active_view_camera(&self.camera) {
            self.message = error.to_string();
            return false;
        }
        let mut saved_camera = self.camera.clone();
        saved_camera.set_name("__current");
        if let Err(error) = self.state.set_active_scene_camera(saved_camera) {
            self.message = error.to_string();
            return false;
        }
        let external = self
            .trajectories
            .iter()
            .filter(|(id, _)| {
                self.state.scene_manager().scene_uid_for_object(**id) == self.state.active_scene()
            })
            .flat_map(|(id, group)| group.blocks.iter().map(move |block| (*id, block.clone())))
            .collect::<Vec<_>>();
        let sources = external
            .iter()
            .map(|(id, block)| (*id, block.source_type()))
            .collect::<Vec<_>>();
        let result = self
            .state
            .active_scene_parts_with_external_trajectories(&sources)
            .map_err(|error| error.to_string())
            .and_then(|fragments| {
                let parts = js_sys::Array::new();
                for (index, fragment) in fragments.iter().enumerate() {
                    parts.push(&js_sys::Uint8Array::from(fragment.as_slice()));
                    if let Some((_, block)) = external.get(index) {
                        parts.push(block.file());
                    }
                }
                download_parts("scene.qsc", &parts, "application/octet-stream").map_err(js_error)
            });
        if let Err(error) = result {
            self.message = error;
            return false;
        }
        self.history.clear();
        self.future.clear();
        self.message = "Scene download started.".into();
        true
    }
    fn save_object_download(&mut self) {
        let Some(id) = self.selected else {
            self.message = "Select a molecule to save.".into();
            return;
        };
        let Some(object) = self.state.scene_manager().object(id) else {
            return;
        };
        let text = cuemol_molstr::write_pdb_string(object.mol());
        if let Err(error) = download("molecule.pdb", text.as_bytes(), "chemical/x-pdb") {
            self.message = js_error(error);
        }
    }

    fn reload_scene(&mut self) {
        if let Some((name, bytes)) = &self.last_scene {
            enqueue(Event::Data(name.clone(), bytes.as_ref().clone()));
        } else {
            self.message = "Open a scene file before reloading it.".into();
        }
    }
    fn open_render(&mut self) {
        if let Ok(settings) = cuemol_command::render_settings(&self.state) {
            self.render_panel
                .read_settings(&settings, &mut self.render_size);
            self.render_hatching = settings.get("backend").is_some_and(|v| v == "umbreon_npr");
            if let Ok(params) = cuemol_command::rendering_params(
                &settings,
                if self.render_hatching {
                    "umbreon_npr"
                } else {
                    "umbreon"
                },
            ) {
                self.render_lighting = params;
            }
            self.render_lighting.perspective = settings
                .get("projection")
                .is_some_and(|v| v == "perspective");
            self.render_transparent = settings.get("transparentBg").is_some_and(|v| v == true);
            if let Ok(hatch) = cuemol_command::hatch_params(&settings, "umbreon_npr") {
                self.render_hatch = hatch;
            }
        }
        self.show_render = true;
    }
    fn shell_sidebar(&mut self, ctx: &egui::Context) {
        let Some(activity) = self.activity else {
            return;
        };
        egui::SidePanel::left("sidebar")
            .default_width(260.)
            .width_range(180.0..=620.)
            .show(ctx, |ui| {
                shell::header(ui, activity.title());
                if activity == Activity::Agent {
                    if let Some(prompt) = self.agent.show_docked(ui) {
                        self.agent.start(
                            &mut self.state,
                            &mut self.camera,
                            prompt,
                            ctx,
                            self.revision,
                        );
                    }
                    return;
                }
                let (a, b) = match activity {
                    Activity::Explorer => ("Scene", "Color"),
                    Activity::View => ("Cameras", "View"),
                    Activity::Selection => ("Molecular structure", "Selection"),
                    Activity::Crystal => ("Symmetry", "Density map"),
                    Activity::Actions => ("History", "Actions"),
                    _ => unreachable!(),
                };
                egui::TopBottomPanel::top(egui::Id::new(("sidebar-top", activity.key())))
                    .default_height((ui.available_height() * 0.5).max(50.))
                    .height_range(34.0..=ui.available_height() - 40.)
                    .resizable(true)
                    .show_inside(ui, |ui| {
                        egui::CollapsingHeader::new(a.to_uppercase())
                            .default_open(true)
                            .show(ui, |ui| {
                                egui::ScrollArea::both()
                                    .id_salt((activity.key(), 0))
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| match activity {
                                        Activity::Explorer => self.scene_tree(ui),
                                        Activity::View => self.camera_list(ui),
                                        Activity::Selection => self.structure_tree(ui),
                                        Activity::Crystal => {
                                            self.object_picker(ui);
                                            for (label, tool) in [
                                                ("Symmetry settings…", "set_symmetry"),
                                                ("Show symmetry mates…", "show_symmetry"),
                                            ] {
                                                let button = ui.button(label);
                                                self.record_ui_rect(label, &button);
                                                if button.clicked() {
                                                    self.select_tool(tool);
                                                }
                                            }
                                        }
                                        Activity::Actions => {
                                            let done: Vec<_> = self
                                                .history
                                                .iter()
                                                .map(|entry| entry.record.clone())
                                                .collect();
                                            let future: Vec<_> = self
                                                .future
                                                .iter()
                                                .map(|entry| entry.record.clone())
                                                .collect();
                                            let intent = self.actions_panel.history(
                                                ui,
                                                &done,
                                                &future,
                                                self.busy || self.console_run.is_some(),
                                            );
                                            self.ui_rects.extend(
                                                self.actions_panel
                                                    .rects
                                                    .iter()
                                                    .map(|(key, value)| (key.clone(), *value)),
                                            );
                                            if let Some(intent) = intent {
                                                self.apply_action_intent(intent);
                                            }
                                        }
                                        _ => {}
                                    });
                            });
                    });
                egui::CollapsingHeader::new(b.to_uppercase())
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt((activity.key(), 1))
                            .show(ui, |ui| match activity {
                                Activity::Explorer => self.color_controls(ui),
                                Activity::View => self.view_controls(ui),
                                Activity::Selection => self.selection_controls(ui),
                                Activity::Crystal => {
                                    for (label, tool) in [
                                        ("Fetch map…", "fetch_map"),
                                        ("Create density renderer…", "create_renderer"),
                                        ("Map properties…", "list_node_props"),
                                    ] {
                                        if ui.button(label).clicked() {
                                            self.select_tool(tool);
                                        }
                                    }
                                }
                                Activity::Actions => {
                                    let targets = cuemol_command::console::actions::targets(
                                        &self.state,
                                        &self
                                            .selected
                                            .into_iter()
                                            .map(Uid::get)
                                            .collect::<Vec<_>>(),
                                    );
                                    let names = targets
                                        .into_iter()
                                        .map(|(_, name)| name)
                                        .collect::<Vec<_>>();
                                    let intent = self.actions_panel.library(
                                        ui,
                                        &names,
                                        self.busy || self.console_run.is_some(),
                                    );
                                    self.ui_rects.extend(
                                        self.actions_panel
                                            .rects
                                            .iter()
                                            .map(|(key, value)| (key.clone(), *value)),
                                    );
                                    if let Some(intent) = intent {
                                        self.apply_action_intent(intent);
                                    }
                                }
                                _ => {}
                            });
                    });
            });
    }
    fn scene_tree(&mut self, ui: &mut egui::Ui) {
        let data = cuemol_command::call_tool(
            &mut self.state,
            &mut self.camera,
            "get_scene_state",
            &json!({}),
        )
        .unwrap_or(Value::Null);
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.;
            for scene in self.state.scene_summaries() {
                let response = shell::tree_row(
                    ui,
                    &format!("Scene: {}", scene.name),
                    Icon::History,
                    false,
                    None,
                    None,
                );
                self.record_ui_rect(&format!("scene:{}", scene.uid.get()), &response);
                if response.clicked() {
                    self.call("switch_scene", json!({"sceneId":scene.uid.get()}));
                    self.open_properties("scene", Some(scene.uid.get()));
                }
            }
            for object in data["objects"].as_array().into_iter().flatten() {
                let Some(id) = object["id"].as_u64() else {
                    continue;
                };
                let key = egui::Id::new(("object-expanded", id));
                let mut open = ui
                    .ctx()
                    .data_mut(|d| d.get_persisted::<bool>(key))
                    .unwrap_or(true);
                let before = object["visible"] == true;
                let mut visible = before;
                let label = format!(
                    "{} ({})",
                    object["name"].as_str().unwrap_or("Object"),
                    object["className"].as_str().unwrap_or("Object")
                );
                let response = ui
                    .push_id(id, |ui| {
                        shell::tree_row(
                            ui,
                            &label,
                            Icon::Crystal,
                            self.selected.map(Uid::get) == Some(id),
                            Some(&mut open),
                            Some(&mut visible),
                        )
                    })
                    .inner;
                self.record_ui_rect(&format!("object:{id}"), &response);
                self.ui_rects.insert(
                    format!("object-visibility:{id}"),
                    [response.rect.right(), response.rect.top(), 24., 22.],
                );
                self.ui_rects.insert(
                    format!("object-expand:{id}"),
                    [response.rect.left() - 18., response.rect.top(), 18., 22.],
                );
                ui.ctx().data_mut(|d| d.insert_persisted(key, open));
                if visible != before {
                    self.call(
                        "set_visible",
                        json!({"nodeType":"object","nodeId":id,"visible":visible}),
                    );
                }
                if response.clicked() {
                    self.selected = Some(Uid::new(id));
                    self.open_properties("object", Some(id));
                    self.paint_selected = 0;
                }
                response.context_menu(|ui| {
                    for (label, action) in [("Copy", "copy"), ("Cut", "cut"), ("Paste", "paste")] {
                        if ui.button(label).clicked() {
                            self.selected = Some(Uid::new(id));
                            self.open_properties("object", Some(id));
                            self.clipboard_focus.scope = Some(cuemol_ui::clipboard::Scope::Tree);
                            self.clipboard_focus.text = None;
                            self.clipboard_action(ui.ctx(), action);
                            ui.close();
                        }
                    }
                    if ui.button("Center").clicked() {
                        self.call("focus_node", json!({"nodeType":"object","nodeId":id}));
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        self.call("delete_node", json!({"nodeType":"object","nodeId":id}));
                        ui.close();
                    }
                });
                if open {
                    ui.indent(id, |ui| {
                        for renderer in object["renderers"].as_array().into_iter().flatten() {
                            if renderer["type"] == "*selection" {
                                continue;
                            }
                            let Some(rid) = renderer["id"].as_u64() else {
                                continue;
                            };
                            let before = renderer["visible"] == true;
                            let mut visible = before;
                            let selected = self
                                .inspector_target
                                .as_ref()
                                .is_some_and(|t| t.kind == "renderer" && t.id == Some(rid));
                            let label = format!(
                                "{} ({})",
                                renderer["name"].as_str().unwrap_or("Renderer"),
                                renderer["type"].as_str().unwrap_or("")
                            );
                            let response = ui
                                .push_id(rid, |ui| {
                                    shell::tree_row(
                                        ui,
                                        &label,
                                        Icon::Atom,
                                        selected,
                                        None,
                                        Some(&mut visible),
                                    )
                                })
                                .inner;
                            self.record_ui_rect(&format!("renderer:{rid}"), &response);
                            self.ui_rects.insert(
                                format!("renderer-visibility:{rid}"),
                                [response.rect.right(), response.rect.top(), 24., 22.],
                            );
                            if visible != before {
                                self.call(
                                    "set_visible",
                                    json!({"nodeType":"renderer","nodeId":rid,"visible":visible}),
                                );
                            }
                            if response.clicked() {
                                self.selected = Some(Uid::new(id));
                                self.open_properties("renderer", Some(rid));
                                self.paint_selected = 0;
                            }
                            response.context_menu(|ui| {
                                for (label, action) in
                                    [("Copy", "copy"), ("Cut", "cut"), ("Paste", "paste")]
                                {
                                    if ui.button(label).clicked() {
                                        self.selected = Some(Uid::new(id));
                                        self.open_properties("renderer", Some(rid));
                                        self.clipboard_focus.scope =
                                            Some(cuemol_ui::clipboard::Scope::Tree);
                                        self.clipboard_focus.text = None;
                                        self.clipboard_action(ui.ctx(), action);
                                        ui.close();
                                    }
                                }
                                if ui.button("Delete").clicked() {
                                    self.call(
                                        "delete_node",
                                        json!({"nodeType":"renderer","nodeId":rid}),
                                    );
                                    ui.close();
                                }
                            });
                        }
                    });
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("representation-kind").selected_text(&self.representation).show_ui(ui,|ui| {
                for kind in cuemol_desktop_core::MOLECULE_RENDERER_TYPES {ui.selectable_value(&mut self.representation,kind.to_string(),*kind);}
            });
            if shell::icon_button(ui,Icon::New,"Add representation",false,egui::vec2(22.,22.)).clicked() {
                if let Some(id)=self.selected {self.call("create_renderer",json!({"objId":id.get(),"rendererType":self.representation,"selection":if self.selection.is_empty(){"*"}else{&self.selection}}));}
            }
        });
        self.clipboard_focus
            .region(cuemol_ui::clipboard::Scope::Tree, ui);
    }
    fn object_picker(&mut self, ui: &mut egui::Ui) {
        let objects = self.state.active_object_summaries();
        let name = objects
            .iter()
            .find(|o| Some(o.uid) == self.selected)
            .map(|o| o.name.as_str())
            .unwrap_or("Select molecule");
        egui::ComboBox::from_label("Molecule")
            .selected_text(name)
            .show_ui(ui, |ui| {
                for object in objects {
                    ui.selectable_value(&mut self.selected, Some(object.uid), object.name);
                }
            });
    }
    fn color_controls(&mut self, ui: &mut egui::Ui) {
        let Some(object_id) = self.selected else {
            ui.label("Select a molecule or renderer.");
            return;
        };
        let objects = self.state.active_object_summaries();
        let Some(object) = objects.iter().find(|o| o.uid == object_id) else {
            return;
        };
        let renderer_id = self
            .inspector_target
            .as_ref()
            .filter(|t| t.kind == "renderer")
            .and_then(|t| t.id)
            .map(Uid::new)
            .filter(|id| self.state.renderer_coloring_state(object_id, *id).is_some());
        let (kind, id, title, color_type, entries) = if let Some(id) = renderer_id {
            let (color_type, _, entries) =
                self.state.renderer_coloring_state(object_id, id).unwrap();
            let name = self
                .state
                .renderer_summaries(object_id)
                .into_iter()
                .find(|r| r.uid == id)
                .map(|r| r.name)
                .unwrap_or_default();
            (
                "renderer",
                id,
                format!("{name} (renderer)"),
                color_type,
                entries,
            )
        } else {
            let Some(mol) = self.state.scene_manager().object(object_id) else {
                return;
            };
            (
                "object",
                object_id,
                format!("{} (object)", object.name),
                mol.coloring_type().unwrap_or("CPKColoring").to_string(),
                mol.paint_coloring().to_vec(),
            )
        };
        let target = json!({"nodeType":kind,"nodeId":id.get()});
        let modes = [
            ("PaintColoring", "paint", "Paint"),
            ("CPKColoring", "cpk", "CPK"),
            ("RainbowColoring", "rainbow", "Rainbow"),
            ("BfacColoring", "bfac", "B factor"),
            ("SolidColoring", "solid", "Solid"),
            ("MolColorRef", "resetdef", "Inherited"),
        ];
        let mut mode = modes.iter().position(|m| m.0 == color_type).unwrap_or(5);
        let before = mode;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("color-target")
                .width((ui.available_width() - 88.).max(80.))
                .selected_text(title)
                .show_ui(ui, |ui| {
                    for object in &objects {
                        if ui
                            .selectable_label(id == object.uid, format!("{} (object)", object.name))
                            .clicked()
                        {
                            self.selected = Some(object.uid);
                            self.inspector_target = Some(cuemol_ui::InspectorTarget {
                                kind: "object".into(),
                                id: Some(object.uid.get()),
                            });
                            self.paint_selected = 0;
                        }
                        for renderer in self.state.renderer_summaries(object.uid) {
                            if renderer.renderer_type == "*selection" {
                                continue;
                            }
                            if ui
                                .selectable_label(
                                    id == renderer.uid,
                                    format!("{} ({})", renderer.name, renderer.renderer_type),
                                )
                                .clicked()
                            {
                                self.selected = Some(object.uid);
                                self.inspector_target = Some(cuemol_ui::InspectorTarget {
                                    kind: "renderer".into(),
                                    id: Some(renderer.uid.get()),
                                });
                                self.paint_selected = 0;
                            }
                        }
                    }
                });
            let response = egui::ComboBox::from_id_salt("color-mode")
                .width(72.)
                .selected_text(modes[mode].2)
                .show_ui(ui, |ui| {
                    for (i, (_, _, label)) in modes.iter().enumerate() {
                        let response = ui.selectable_value(&mut mode, i, *label);
                        self.record_ui_rect(&format!("Color mode {label}"), &response);
                    }
                })
                .response;
            self.record_ui_rect("Color mode", &response);
        });
        if mode != before {
            let mut args = target.clone();
            args["mode"] = json!(format!("paint-type-{}", modes[mode].1));
            self.call("set_renderer_coloring", args);
            self.paint_selected = 0;
        }
        if color_type == "PaintColoring" {
            self.paint_selected = self.paint_selected.min(entries.len().saturating_sub(1));
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 104.).max(0.));
                for (icon, label, command, by) in [
                    (Icon::New, "Add paint", "add_paint_at", 0),
                    (Icon::Up, "Move paint up", "move_paint", -1),
                    (Icon::Down, "Move paint down", "move_paint", 1),
                    (Icon::Delete, "Delete paint", "remove_paint", 0),
                ] {
                    let response = ui
                        .add_enabled_ui(command == "add_paint_at" || !entries.is_empty(), |ui| {
                            shell::icon_button(ui, icon, label, false, egui::vec2(20., 20.))
                        })
                        .inner;
                    self.record_ui_rect(label, &response);
                    if response.clicked() {
                        let mut args = target.clone();
                        if command == "add_paint_at" {
                            args["selection"] = json!(if self.selection.is_empty() {
                                "*"
                            } else {
                                &self.selection
                            });
                            args["color"] = json!("white");
                            self.paint_selected = entries.len();
                        } else {
                            args["number"] = json!(self.paint_selected + 1);
                        }
                        if command == "move_paint" {
                            args["by"] = json!(by);
                            self.paint_selected = (self.paint_selected as i64 + by)
                                .clamp(0, entries.len().saturating_sub(1) as i64)
                                as usize;
                        }
                        self.call(command, args);
                    }
                }
            });
            ui.separator();
            egui::Grid::new(("paint-entries", id.get()))
                .num_columns(2)
                .min_col_width((ui.available_width() - 12.) * 0.45)
                .show(ui, |ui| {
                    ui.weak("Selection");
                    ui.weak("Color");
                    ui.end_row();
                    for (index, (selection, color)) in entries.iter().enumerate() {
                        let response = ui.selectable_label(self.paint_selected == index, selection);
                        self.record_ui_rect(&format!("paint:{index}"), &response);
                        if response.clicked() {
                            self.paint_selected = index;
                        }
                        if response.double_clicked() {
                            self.paint_editor = Some((
                                kind.into(),
                                id.get(),
                                index,
                                selection.clone(),
                                color.clone(),
                            ));
                        }
                        let response = ui
                            .horizontal(|ui| {
                                let rgb = self.state.active_style_color(color);
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(14., 12.),
                                    egui::Sense::hover(),
                                );
                                self.ui_rects.insert(
                                    format!("paint-swatch:{index}"),
                                    [rect.min.x, rect.min.y, rect.width(), rect.height()],
                                );
                                if let Some(c) = rgb {
                                    ui.painter().rect_filled(
                                        rect,
                                        0.,
                                        egui::Color32::from_rgb(c.r, c.g, c.b),
                                    );
                                }
                                ui.selectable_label(false, color)
                            })
                            .inner;
                        self.record_ui_rect(&format!("paint-color:{index}"), &response);
                        if response.clicked() {
                            self.paint_selected = index;
                            self.paint_editor = Some((
                                kind.into(),
                                id.get(),
                                index,
                                selection.clone(),
                                color.clone(),
                            ));
                        }
                        ui.end_row();
                    }
                });
        }
        if let Some((kind, id, index, mut selection, mut color)) = self.paint_editor.take() {
            let mut open = true;
            let mut apply = false;
            egui::Window::new("Paint entry")
                .id(egui::Id::new("paint-editor"))
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    ui.label("Selection");
                    let response = ui.text_edit_singleline(&mut selection);
                    self.record_ui_rect("Paint selection", &response);
                    ui.label("Color");
                    let response = ui.text_edit_singleline(&mut color);
                    self.record_ui_rect("Paint color", &response);
                    let response = ui.button("Apply");
                    self.record_ui_rect("Apply paint", &response);
                    apply = response.clicked();
                });
            if apply {
                self.call("set_paint",json!({"nodeType":kind,"nodeId":id,"number":index+1,"selection":selection,"color":color}));
            } else if open {
                self.paint_editor = Some((kind, id, index, selection, color));
            }
        }
    }
    fn camera_list(&mut self, ui: &mut egui::Ui) {
        let data = cuemol_command::call_tool(
            &mut self.state,
            &mut self.camera,
            "list_cameras",
            &json!({}),
        )
        .unwrap_or(Value::Null);
        for camera in data
            .as_array()
            .or_else(|| data["cameras"].as_array())
            .into_iter()
            .flatten()
        {
            if let Some(name) = camera["name"].as_str() {
                let row = ui.selectable_label(self.clipboard_camera.as_deref() == Some(name), name);
                self.record_ui_rect(&format!("camera:{name}"), &row);
                if row.clicked() {
                    self.clipboard_camera = Some(name.into());
                }
                if row.double_clicked() {
                    self.call("apply_camera", json!({"name":name}));
                }
                row.context_menu(|ui| {
                    for (label, action) in [("Copy", "copy"), ("Cut", "cut"), ("Paste", "paste")] {
                        if ui.button(label).clicked() {
                            self.clipboard_camera = Some(name.into());
                            self.clipboard_focus.scope = Some(cuemol_ui::clipboard::Scope::Cameras);
                            self.clipboard_focus.text = None;
                            self.clipboard_action(ui.ctx(), action);
                            ui.close();
                        }
                    }
                });
            }
        }
        if ui.button("Save current camera…").clicked() {
            self.select_tool("save_camera");
        }
        self.clipboard_focus
            .region(cuemol_ui::clipboard::Scope::Cameras, ui);
    }
    fn view_controls(&mut self, ui: &mut egui::Ui) {
        let mut perspective = self.camera.is_perspective();
        if ui.checkbox(&mut perspective, "Perspective").changed() {
            self.camera.set_perspective(perspective);
        }
        let mut slab = self.camera.slab_depth();
        if ui
            .add(
                egui::Slider::new(&mut slab, 0.1..=self.camera.cam_dist() * 2.)
                    .text("Slab")
                    .logarithmic(true),
            )
            .changed()
        {
            self.camera.set_slab_depth(slab);
        }
        let mut zoom = self.camera.zoom();
        if ui
            .add(
                egui::DragValue::new(&mut zoom)
                    .prefix("Zoom ")
                    .range(0.1..=100000.),
            )
            .changed()
        {
            self.camera.set_zoom(zoom);
        }
        if ui.button("Fit scene").clicked() {
            self.fit(false);
        }
        if ui.button("View properties…").clicked() {
            self.open_properties("view", None);
        }
    }
    fn structure_tree(&mut self, ui: &mut egui::Ui) {
        self.object_picker(ui);
        for row in self.state.active_sequence_summaries() {
            if Some(row.object_uid) != self.selected {
                continue;
            }
            egui::CollapsingHeader::new(format!("Chain {}", row.chain_id)).show(ui, |ui| {
                ui.monospace(&row.sequence);
                if ui.button("Select chain").clicked() {
                    self.selection = format!("chain {}", row.chain_id);
                    self.dirty = true;
                }
            });
        }
    }
    fn selection_controls(&mut self, ui: &mut egui::Ui) {
        if ui
            .add(
                egui::TextEdit::multiline(&mut self.selection)
                    .hint_text("chain A and resi 10:30")
                    .desired_width(f32::INFINITY)
                    .desired_rows(3),
            )
            .changed()
        {
            self.dirty = true;
        }
        ui.horizontal(|ui|{if ui.button("Center").clicked(){if let Some(id)=self.selected{self.call("center_view",json!({"objId":id.get(),"selection":if self.selection.is_empty(){"*"}else{&self.selection},"zoom":false}));}}if ui.button("Zoom").clicked(){self.fit(true);}if ui.button("Clear").clicked(){self.selection.clear();self.dirty=true;}});
        for value in ["protein", "nucleic", "all", "none"] {
            if ui.button(value).clicked() {
                self.selection = value.into();
                self.dirty = true;
            }
        }
    }
    fn apply_sequence_action(
        &mut self,
        action: cuemol_ui::sequence::SequenceAction,
        ctx: &egui::Context,
    ) {
        use cuemol_ui::sequence::SequenceAction;
        let (object_uid, expression, center) = match action {
            SequenceAction::Range {
                object_uid,
                anchor_selection,
                range_selection,
                toggle,
                center_selection,
            } => {
                match self.state.molecule_residue_range_selection_text(
                    object_uid,
                    "",
                    &anchor_selection,
                    &range_selection,
                    toggle,
                ) {
                    Ok(expression) => (object_uid, expression, center_selection),
                    Err(error) => {
                        self.message = error.to_string();
                        return;
                    }
                }
            }
            SequenceAction::Selection {
                object_uid,
                expression,
                ..
            } => (object_uid, expression, None),
            SequenceAction::Center {
                object_uid,
                selection,
            } => {
                match self
                    .state
                    .molecule_residue_pivot_for_text(object_uid, &selection)
                {
                    Ok(Some(center)) => {
                        self.camera.set_center(center);
                        self.message = "Center here".into();
                    }
                    Ok(None) => {}
                    Err(error) => self.message = error.to_string(),
                }
                return;
            }
            SequenceAction::Copy(sequence) => {
                ctx.copy_text(sequence);
                self.message = "Sequence copied".into();
                return;
            }
        };
        if self.apply_navigation_commands(vec![cuemol_command::navigation::Command {
            name: "set_mol_selection",
            args: json!({"objId":object_uid.get(), "selection":expression}),
        }]) {
            if let Some(selection) = center {
                match self
                    .state
                    .molecule_residue_pivot_for_text(object_uid, &selection)
                {
                    Ok(Some(center)) => self.camera.set_center(center),
                    Ok(None) => {}
                    Err(error) => self.message = error.to_string(),
                }
            }
        }
    }

    fn shell_bottom(&mut self, ctx: &egui::Context) {
        if self.message != self.last_log {
            self.logs.push(self.message.chars().take(4096).collect());
            self.last_log = self.message.clone();
            if self.logs.len() > 1000 {
                self.logs.remove(0);
            }
        }
        egui::TopBottomPanel::bottom(if self.bottom_open {
            "bottom-panel"
        } else {
            "bottom-panel-collapsed"
        })
        .default_height(if self.bottom_open { 200. } else { 30. })
        .height_range(if self.bottom_open {
            100.0..=600.0
        } else {
            30.0..=30.0
        })
        .resizable(self.bottom_open)
        .show(ctx, |ui| {
            ui.set_min_height(ui.available_height());
            let compact = shell::compact_tabs(ui);
            ui.horizontal(|ui| {
                if compact {
                    ui.spacing_mut().item_spacing.x = 2.;
                }
                shell::tab_strip(ui, |ui| {
                    for tab in shell::BOTTOM_TABS {
                        let response = shell::tab(ui, tab, self.bottom_tab == tab, compact);
                        if response.clicked() {
                            self.bottom_tab = tab.into();
                        }
                        self.record_ui_rect(tab, &response);
                        if response.clicked() {
                            self.bottom_open = true;
                        }
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if self.bottom_open {
                        "Collapse panel"
                    } else {
                        "Expand panel"
                    };
                    let response = shell::icon_button(
                        ui,
                        if self.bottom_open {
                            Icon::Down
                        } else {
                            Icon::Up
                        },
                        label,
                        false,
                        egui::vec2(20., 20.),
                    );
                    self.record_ui_rect(label, &response);
                    if response.clicked() {
                        self.bottom_open = !self.bottom_open;
                    }
                });
            });
            if !self.bottom_open {
                return;
            }
            ui.separator();
            match self.bottom_tab.as_str() {
                "Console" => {
                    let intent = self.console.show(
                        ui,
                        &self.state,
                        &self.console_session,
                        &self.logs,
                        self.busy || self.console_run.is_some(),
                    );
                    self.ui_rects.extend(
                        self.console
                            .rects
                            .iter()
                            .map(|(name, rect)| (name.clone(), *rect)),
                    );
                    self.console_intent(intent);
                }
                "Output" => {
                    if ui.small_button("Clear").clicked() {
                        self.logs.clear();
                    }
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .min_scrolled_height(0.)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &self.logs {
                                ui.monospace(line);
                            }
                        });
                }
                "Sequence" => {
                    let action = self.sequence_panel.show(ui, &mut self.state, self.revision);
                    self.ui_rects.extend(
                        self.sequence_panel
                            .rects
                            .iter()
                            .map(|(name, rect)| (name.clone(), *rect)),
                    );
                    if let Some(action) = action {
                        self.apply_sequence_action(action, ui.ctx());
                    }
                }
                "Animation" => {
                    self.timeline_contents(ui);
                }
                "MD Trajectory" => {
                    if let Some((_, frames, current_frame)) = self.trajectory_info() {
                        ui.horizontal(|ui| {
                            let label = if self.playing { "Pause" } else { "Play" };
                            ui.toggle_value(&mut self.playing, label);
                            let mut frame = current_frame;
                            if ui
                                .add(egui::Slider::new(&mut frame, 0..=frames - 1).text("Frame"))
                                .changed()
                            {
                                self.request_frame(frame);
                            }
                            ui.add(
                                egui::DragValue::new(&mut self.fps)
                                    .range(1.0..=120.)
                                    .suffix(" fps"),
                            );
                        });
                        ui.label(format!("{} indexed frames", frames));
                    } else {
                        ui.label("Open a topology, then choose a trajectory file.");
                    }
                    if ui.button("Open trajectory…").clicked() {
                        choose_file();
                    }
                }
                _ => {}
            }
        });
    }
}

impl App {
    pub(super) fn viewport_palette(&mut self, ctx: &egui::Context) {
        let Some(rect) = self.ui_rects.get("Viewport").copied() else {
            return;
        };
        let viewport =
            egui::Rect::from_min_size(egui::pos2(rect[0], rect[1]), egui::vec2(rect[2], rect[3]));
        let (rects, command, changed) =
            cuemol_ui::navigation::palette(ctx, viewport, &mut self.navigation, &mut self.state);
        for (label, rect) in rects {
            self.ui_rects.insert(
                label.into(),
                [rect.min.x, rect.min.y, rect.width(), rect.height()],
            );
        }
        if let Some(command) = command {
            self.apply_navigation_commands(vec![command]);
        }
        if changed {
            self.settings_text = self.state.config_text();
            if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten())
            {
                let _ = storage.set_item("cuemol.config", &self.settings_text);
            }
        }
    }
}
