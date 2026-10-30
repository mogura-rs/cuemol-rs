use super::*;
use cuemol_ui::shell::{self, Activity, Icon};

impl App {
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
        if shell::activity_bar(ctx, &mut self.activity) {
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
        self.inspector.inspect(
            &mut self.state,
            &mut self.camera,
            self.inspector_target.clone(),
            self.revision,
        );
        for args in self.inspector.panel(ctx) {
            self.call("set_node_prop", args);
        }
        self.shell_bottom(ctx);
        egui::TopBottomPanel::top("scene-tabs")
            .exact_height(30.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for scene in self.state.scene_summaries() {
                        let label = scene.name;
                        if ui.selectable_label(scene.active, label).clicked() {
                            self.call("switch_scene", json!({"sceneId":scene.uid.get()}));
                        }
                    }
                    if ui.small_button("+").on_hover_text("New Tab").clicked() {
                        self.call("create_scene", json!({}));
                    }
                });
            });
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
        egui::TopBottomPanel::top("menubar")
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
                        if ui.button("Save As…").clicked() {
                            self.save_object_download();
                            ui.close();
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui
                            .add_enabled(!self.history.is_empty(), egui::Button::new("Undo"))
                            .clicked()
                        {
                            self.call("undo", json!({}));
                            ui.close();
                        }
                        if ui
                            .add_enabled(!self.future.is_empty(), egui::Button::new("Redo"))
                            .clicked()
                        {
                            self.call("redo", json!({}));
                            ui.close();
                        }
                    });
                    ui.menu_button("View", |ui| {
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
                        ("Open File", Icon::Open),
                        ("Save As", Icon::Save),
                        ("Open Scene", Icon::Open),
                        ("Reload Scene", Icon::Reload),
                        ("Save Scene", Icon::Save),
                        ("Get PDB", Icon::Open),
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
                        let enabled = match label {
                            "Undo" => !self.history.is_empty(),
                            "Redo" => !self.future.is_empty(),
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
                                "Save Scene" => self.save_scene_download(),
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
    fn save_scene_download(&mut self) {
        if let Err(error) = self.state.set_active_view_camera(&self.camera) {
            self.message = error.to_string();
            return;
        }
        let mut saved_camera = self.camera.clone();
        saved_camera.set_name("__current");
        if let Err(error) = self.state.set_active_scene_camera(saved_camera) {
            self.message = error.to_string();
            return;
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
        }
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
                        egui::CollapsingHeader::new(a)
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
                                                ("Symmetry settings", "list_node_props"),
                                                ("Generate mates…", "symmetry"),
                                            ] {
                                                if ui.button(label).clicked() {
                                                    self.select_tool(tool);
                                                }
                                            }
                                        }
                                        Activity::Actions => {
                                            ui.label(format!(
                                                "{} completed steps",
                                                self.history.len()
                                            ));
                                            if ui.button("Undo").clicked() {
                                                self.call("undo", json!({}));
                                            }
                                            if ui.button("Redo").clicked() {
                                                self.call("redo", json!({}));
                                            }
                                        }
                                        _ => {}
                                    });
                            });
                    });
                egui::CollapsingHeader::new(b)
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
                                    if ui.button("Commands…").clicked() {
                                        self.show_console = true;
                                    }
                                    if ui.button("Run JavaScript…").clicked() {
                                        self.select_tool("execute_script");
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
        for scene in self.state.scene_summaries() {
            if ui.selectable_label(scene.active, &scene.name).clicked() {
                self.call("switch_scene", json!({"sceneId":scene.uid.get()}));
                self.open_properties("scene", Some(scene.uid.get()));
            }
        }
        for object in data["objects"].as_array().into_iter().flatten() {
            let Some(id) = object["id"].as_u64() else {
                continue;
            };
            ui.horizontal(|ui| {
                let mut visible = object["visible"] == true;
                if ui.checkbox(&mut visible, "").changed() {
                    self.call(
                        "set_visible",
                        json!({"nodeType":"object","nodeId":id,"visible":visible}),
                    );
                }
                let response = ui.selectable_label(
                    self.selected.map(Uid::get) == Some(id),
                    object["name"].as_str().unwrap_or("Object"),
                );
                self.ui_rects.insert(
                    format!("object:{id}"),
                    [
                        response.rect.min.x,
                        response.rect.min.y,
                        response.rect.width(),
                        response.rect.height(),
                    ],
                );
                if response.clicked() {
                    self.selected = Some(Uid::new(id));
                    self.open_properties("object", Some(id));
                }
                response.context_menu(|ui| {
                    if ui.button("Center").clicked() {
                        self.call("focus_node", json!({"nodeType":"object","nodeId":id}));
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        self.call("delete_node", json!({"nodeType":"object","nodeId":id}));
                        ui.close();
                    }
                });
            });
            ui.indent(id, |ui| {
                for renderer in object["renderers"].as_array().into_iter().flatten() {
                    if renderer["type"] == "*selection" {
                        continue;
                    }
                    let Some(rid) = renderer["id"].as_u64() else {
                        continue;
                    };
                    ui.horizontal(|ui| {
                        let mut visible = renderer["visible"] == true;
                        if ui.checkbox(&mut visible, "").changed() {
                            self.call(
                                "set_visible",
                                json!({"nodeType":"renderer","nodeId":rid,"visible":visible}),
                            );
                        }
                        let response = ui.selectable_label(
                            self.inspector_target
                                .as_ref()
                                .is_some_and(|t| t.kind == "renderer" && t.id == Some(rid)),
                            renderer["name"].as_str().unwrap_or("Renderer"),
                        );
                        if response.clicked() {
                            self.selected = Some(Uid::new(id));
                            self.open_properties("renderer", Some(rid));
                        }
                        response.context_menu(|ui| {
                            if ui.button("Delete").clicked() {
                                self.call(
                                    "delete_node",
                                    json!({"nodeType":"renderer","nodeId":rid}),
                                );
                                ui.close();
                            }
                        });
                    });
                }
            });
        }
        ui.separator();
        egui::ComboBox::from_label("Representation")
            .selected_text(&self.representation)
            .show_ui(ui, |ui| {
                for kind in cuemol_desktop_core::MOLECULE_RENDERER_TYPES {
                    ui.selectable_value(&mut self.representation, kind.to_string(), *kind);
                }
            });
        if ui.button("Add representation").clicked() {
            if let Some(id) = self.selected {
                self.call("create_renderer",json!({"objId":id.get(),"rendererType":self.representation,"selection":if self.selection.is_empty(){"*"}else{&self.selection}}));
            }
        }
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
        self.object_picker(ui);
        egui::ComboBox::from_label("Coloring")
            .selected_text(&self.coloring)
            .show_ui(ui, |ui| {
                for kind in ["cpk", "rainbow", "bfac"] {
                    ui.selectable_value(&mut self.coloring, kind.into(), kind);
                }
            });
        if ui.button("Apply").clicked() {
            if let Some(id) = self.selected {
                self.call("set_renderer_coloring",json!({"nodeId":id.get(),"nodeType":"object","mode":format!("paint-type-{}",self.coloring)}));
            }
        }
        if ui.button("Paint selection…").clicked() {
            self.select_tool("paint");
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
                if ui.button(name).clicked() {
                    self.call("apply_camera", json!({"name":name}));
                }
            }
        }
        if ui.button("Save current camera…").clicked() {
            self.select_tool("save_camera");
        }
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
    fn shell_bottom(&mut self, ctx: &egui::Context) {
        if self.message != self.last_log {
            self.logs.push(self.message.chars().take(4096).collect());
            self.last_log = self.message.clone();
            if self.logs.len() > 1000 {
                self.logs.remove(0);
            }
        }
        egui::TopBottomPanel::bottom("bottom-panel")
            .default_height(if self.bottom_open { 200. } else { 30. })
            .height_range(if self.bottom_open {
                100.0..=600.0
            } else {
                30.0..=30.0
            })
            .resizable(self.bottom_open)
            .show(ctx, |ui| {
                ui.set_min_height(ui.available_height());
                ui.horizontal(|ui| {
                    for tab in [
                        "Console",
                        "Output",
                        "Animation",
                        "Sequence",
                        "MD Trajectory",
                    ] {
                        let response = ui.selectable_value(&mut self.bottom_tab, tab.into(), tab);
                        self.record_ui_rect(tab, &response);
                        if response.clicked() {
                            self.bottom_open = true;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(if self.bottom_open { "▾" } else { "▴" })
                            .clicked()
                        {
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
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .min_scrolled_height(0.)
                            .max_height((ui.available_height() - 30.).max(20.))
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for line in &self.logs {
                                    ui.monospace(line);
                                }
                            });
                        ui.horizontal(|ui| {
                            ui.monospace("Js>");
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut self.console_source)
                                    .desired_width(ui.available_width() - 50.),
                            );
                            self.record_ui_rect("Console input", &response);
                            let run = ui.button("Run");
                            self.record_ui_rect("Console Run", &run);
                            if run.clicked()
                                || (response.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            {
                                let source = std::mem::take(&mut self.console_source);
                                self.call("execute_script", json!({"source":source}));
                            }
                        });
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
                        egui::ScrollArea::both().show(ui, |ui| {
                            for row in self.state.active_sequence_view(&self.selection) {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{} / {}", row.object_name, row.chain_id));
                                    for residue in row.residues {
                                        let response = ui
                                            .selectable_label(residue.selected, &residue.letter)
                                            .on_hover_text(format!(
                                                "{} {}",
                                                residue.res_name, residue.residue_index
                                            ));
                                        if response.clicked() {
                                            self.selected = Some(row.object_uid);
                                            self.selection = residue.selection;
                                            self.dirty = true;
                                        }
                                    }
                                });
                            }
                        });
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
                                    .add(
                                        egui::Slider::new(&mut frame, 0..=frames - 1).text("Frame"),
                                    )
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
