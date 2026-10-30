use super::*;
use cuemol_ui::shell::{self, Activity, Icon};
use serde_json::json;

impl GuiApplication {
    pub(super) fn tritium_layout(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        self.menu_bar(ctx, camera);
        if self.show_toolbar {
            self.tritium_toolbar(ctx, camera);
        }
        self.status_bar(ctx);
        let before = self.activity;
        if self.agent.visible {
            self.activity = Some(Activity::Agent);
            self.show_workspace = true;
            self.agent.visible = false;
        }
        if shell::activity_bar(ctx, &mut self.activity) {
            self.open_options_dialog();
        }
        if before != self.activity {
            self.show_workspace = self.activity.is_some();
            self.state.put_config_value(
                "layout:activity",
                self.activity.map(Activity::key).unwrap_or("none"),
            );
        }
        if self.show_workspace {
            self.tritium_sidebar(ctx, camera);
        }
        let target = match self.current_workspace_target() {
            Some(NativeWorkspaceTarget::Object { owner, .. }) => Some(cuemol_ui::InspectorTarget {
                kind: "object".into(),
                id: Some(match owner {
                    NativePropertyObject::Molecule(id)
                    | NativePropertyObject::DensityMap(id)
                    | NativePropertyObject::Surface(id)
                    | NativePropertyObject::Lightweight(id) => id.get(),
                }),
            }),
            Some(NativeWorkspaceTarget::Renderer { renderer_uid, .. }) => {
                Some(cuemol_ui::InspectorTarget {
                    kind: "renderer".into(),
                    id: Some(renderer_uid.get()),
                })
            }
            _ => None,
        };
        let mut view = camera.to_qsys_camera("__current");
        self.inspector
            .inspect(&mut self.state, &mut view, target, self.snapshot_revision);
        for args in self.inspector.panel(ctx) {
            self.tritium_command("set_node_prop", args, camera);
        }
        self.tritium_bottom(ctx, camera);
        self.scene_tab_strip(ctx, camera);
        self.central_view(ctx, camera);
        self.viewport_palette(ctx, camera);
    }
    fn tritium_toolbar(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::TopBottomPanel::top("toolbar")
            .exact_height(36.)
            .frame(
                egui::Frame::NONE
                    .fill(if self.light_theme {
                        egui::Color32::from_rgb(224, 225, 229)
                    } else {
                        egui::Color32::from_rgb(26, 28, 36)
                    })
                    .inner_margin(egui::Margin::symmetric(8, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let compact = ui.available_width() < 1000.;
                    if shell::toolbar_button(ui, Icon::New, "New Tab", compact).clicked() {
                        self.open_new_tab_dialog();
                    }
                    ui.separator();
                    if shell::toolbar_button(ui, Icon::File, "Open File", compact).clicked() {
                        self.choose_open_file(camera);
                    }
                    if shell::toolbar_button(ui, Icon::Save, "Save As", compact).clicked() {
                        self.choose_save_selected_object();
                    }
                    ui.separator();
                    if shell::toolbar_button(ui, Icon::Open, "Open Scene", compact).clicked() {
                        self.choose_open_scene(camera);
                    }
                    if shell::toolbar_button(ui, Icon::Reload, "Reload Scene", compact).clicked() {
                        self.reload_current_path(camera);
                    }
                    if shell::toolbar_button(ui, Icon::Save, "Save Scene", compact).clicked() {
                        self.save_current_scene(camera);
                    }
                    ui.separator();
                    if shell::toolbar_button(ui, Icon::Fetch, "Get PDB", compact).clicked() {
                        self.show_get_pdb = true;
                    }
                    ui.separator();
                    if shell::toolbar_button(ui, Icon::Render, "Render", compact).clicked() {
                        self.open_native_render();
                    }
                    ui.separator();
                    ui.add_enabled_ui(!self.undo_stack.is_empty(), |ui| {
                        if shell::toolbar_button(ui, Icon::Undo, "Undo", compact).clicked() {
                            self.undo(camera);
                        }
                    });
                    ui.add_enabled_ui(!self.redo_stack.is_empty(), |ui| {
                        if shell::toolbar_button(ui, Icon::Redo, "Redo", compact).clicked() {
                            self.redo(camera);
                        }
                    });
                });
            });
    }
    fn tritium_sidebar(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        let Some(activity) = self.activity else {
            return;
        };
        let response = egui::SidePanel::left("tritium-sidebar")
            .default_width(self.left_panel_width)
            .width_range(180.0..=620.)
            .resizable(true)
            .show(ctx, |ui| {
                shell::header(ui, activity.title());
                if activity == Activity::Agent {
                    if let Some(prompt) = self.agent.show_docked(ui) {
                        let mut view = camera.to_qsys_camera("__current");
                        self.agent.start(
                            &mut self.state,
                            &mut view,
                            prompt,
                            ctx,
                            self.snapshot_revision,
                        );
                    }
                    return;
                }
                if activity == Activity::Actions {
                    self.action_sidebar(ui, camera);
                    return;
                }
                let (first, second) = match activity {
                    Activity::Explorer => (Some(LeftPanel::Workspace), LeftPanel::Coloring),
                    Activity::View => (None, LeftPanel::View),
                    Activity::Selection => (Some(LeftPanel::MolStruct), LeftPanel::Selection),
                    Activity::Crystal => (Some(LeftPanel::Symmetry), LeftPanel::DensityMap),
                    _ => unreachable!(),
                };
                let first_title = first.map(LeftPanel::title).unwrap_or("Cameras");
                let height = (ui.available_height() * 0.48).max(70.);
                egui::TopBottomPanel::top(egui::Id::new(("activity-top", activity.key())))
                    .default_height(height)
                    .height_range(34.0..=ui.available_height() - 40.)
                    .resizable(true)
                    .show_inside(ui, |ui| {
                        egui::CollapsingHeader::new(if activity == Activity::Explorer {
                            "SCENE"
                        } else {
                            first_title
                        })
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::ScrollArea::both()
                                .id_salt((activity.key(), "first"))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if let Some(panel) = first {
                                        self.left_panel_body(ui, camera, panel);
                                    } else {
                                        self.camera_sidebar(ui, camera);
                                    }
                                });
                        });
                    });
                egui::CollapsingHeader::new(if activity == Activity::Explorer {
                    "COLOR"
                } else {
                    second.title()
                })
                .default_open(true)
                .show(ui, |ui| {
                    egui::ScrollArea::both()
                        .id_salt((activity.key(), "second"))
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.left_panel_body(ui, camera, second));
                });
            });
        let width = response.response.rect.width();
        if (width - self.left_panel_width).abs() > 0.5 {
            self.left_panel_width = width;
            self.state
                .put_config_value("left_panel:width", width.to_string());
        }
    }
    fn camera_sidebar(&mut self, ui: &mut egui::Ui, camera: &mut CameraController) {
        let Some(scene) = self.state.active_scene() else {
            return;
        };
        for saved in self.state.camera_summaries(scene) {
            ui.horizontal(|ui| {
                if ui.selectable_label(false, &saved.name).double_clicked() {
                    self.tritium_command("apply_camera", json!({"name":saved.name}), camera);
                }
                if ui.small_button("Apply").clicked() {
                    self.tritium_command("apply_camera", json!({"name":saved.name}), camera);
                }
                if ui
                    .small_button("×")
                    .on_hover_text("Delete camera")
                    .clicked()
                {
                    self.delete_workspace_camera(scene, &saved.name);
                }
            });
        }
        if ui.button("Save current camera…").clicked() {
            self.open_new_camera_dialog(scene, camera);
        }
    }
    fn action_sidebar(&mut self, ui: &mut egui::Ui, camera: &mut CameraController) {
        egui::CollapsingHeader::new("History")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.undo_stack.is_empty(), egui::Button::new("Undo"))
                        .clicked()
                    {
                        self.undo(camera);
                    }
                    if ui
                        .add_enabled(!self.redo_stack.is_empty(), egui::Button::new("Redo"))
                        .clicked()
                    {
                        self.redo(camera);
                    }
                });
                ui.label(format!(
                    "{} completed steps · {} redo steps",
                    self.undo_stack.len(),
                    self.redo_stack.len()
                ));
            });
        egui::CollapsingHeader::new("Actions")
            .default_open(true)
            .show(ui, |ui| {
                if ui.button("Open script…").clicked() {
                    self.choose_execute_script();
                }
                if ui.button("Console").clicked() {
                    self.bottom_tab = BottomTab::Console;
                    self.show_bottom_panel = true;
                }
            });
    }
    fn tritium_command(
        &mut self,
        name: &str,
        args: serde_json::Value,
        camera: &mut CameraController,
    ) {
        let mut view = camera.to_qsys_camera("__current");
        let mut undo = self.capture_undo();
        undo.camera = Some(view.clone());
        match cuemol_command::call_tool(&mut self.state, &mut view, name, &args) {
            Ok(_) => {
                if cuemol_command::tool_records_history(name) {
                    self.push_undo_snapshot(undo);
                }
                *camera = CameraController::from_qsys_camera(&view);
                self.rebuild_snapshot();
            }
            Err(error) => self.set_status(error.to_string()),
        }
    }
    fn tritium_bottom(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        let panel = egui::TopBottomPanel::bottom("tritium-bottom")
            .resizable(self.show_bottom_panel)
            .default_height(if self.show_bottom_panel {
                self.bottom_panel_height
            } else {
                30.
            })
            .height_range(if self.show_bottom_panel {
                100.0..=600.0
            } else {
                30.0..=30.0
            });
        let response = panel.show(ctx, |ui| {
            let previous = self.bottom_tab;
            let compact = shell::compact_tabs(ui);
            ui.horizontal(|ui| {
                if compact {
                    ui.spacing_mut().item_spacing.x = 2.;
                }
                shell::tab_strip(ui, |ui| {
                    for (tab, label) in [
                        (BottomTab::Console, "Console"),
                        (BottomTab::Output, "Output"),
                        (BottomTab::Sequence, "Sequence"),
                        (BottomTab::Animation, "Animation"),
                        (BottomTab::Trajectory, "MD Trajectory"),
                    ] {
                        if shell::tab(ui, label, self.bottom_tab == tab, compact).clicked() {
                            self.bottom_tab = tab;
                            self.show_bottom_panel = true;
                        }
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if shell::icon_button(
                        ui,
                        if self.show_bottom_panel {
                            Icon::Down
                        } else {
                            Icon::Up
                        },
                        if self.show_bottom_panel {
                            "Collapse panel"
                        } else {
                            "Expand panel"
                        },
                        false,
                        egui::vec2(20., 20.),
                    )
                    .clicked()
                    {
                        self.show_bottom_panel = !self.show_bottom_panel;
                    }
                });
            });
            if previous != self.bottom_tab {
                self.state
                    .put_config_value("bottom_panel:tab", self.bottom_tab.config_name());
            }
            if !self.show_bottom_panel {
                return;
            }
            ui.separator();
            match self.bottom_tab {
                BottomTab::Console => self.output_tab(ui),
                BottomTab::Output => {
                    ui.horizontal(|ui| {
                        if ui.small_button("Clear").clicked() {
                            self.clear_log();
                        }
                        if ui.small_button("Save As…").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name("output.log")
                                .save_file()
                            {
                                if let Err(e) = fs::write(path, self.log_lines.join("\n")) {
                                    self.set_status(e.to_string());
                                }
                            }
                        }
                    });
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .min_scrolled_height(0.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &self.log_lines {
                                ui.monospace(line);
                            }
                        });
                }
                BottomTab::Animation => {
                    self.animation_playback_controls(ui, camera);
                    egui::ScrollArea::both().show(ui, |ui| self.animation_section(ui, camera));
                }
                BottomTab::Sequence => self.sequence_tab(ui, camera),
                BottomTab::Trajectory => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.trajectory_tab(ui));
                }
            }
        });
        if self.show_bottom_panel {
            self.bottom_panel_height = response.response.rect.height();
            self.state
                .put_config_value("bottom_panel:height", self.bottom_panel_height.to_string());
        }
        self.state
            .put_config_value("bottom_panel:visible", self.show_bottom_panel.to_string());
    }
    fn viewport_palette(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        let Some(rect) = self.viewport_rect else {
            return;
        };
        let (_, command, changed) =
            cuemol_ui::navigation::palette(ctx, rect, &mut self.navigation, &mut self.state);
        if let Some(command) = command {
            self.apply_navigation_commands(camera, vec![command]);
        }
        if changed {
            if let Err(error) = self.persist_config_state() {
                self.set_status(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    #[test]
    fn tritium_activity_buttons_switch_and_collapse_the_sidebar() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280., 900.))
            .build(|ctx| app.ui(ctx, &mut camera));
        harness.run();
        harness.get_by_label("Crystal").click();
        harness.run();
        harness.get_by_label("Crystal").click();
        harness.run();
        harness.get_by_label("Explorer").click();
        harness.run();
        harness.get_by_label("MD Trajectory").click();
        harness.run();
        if let Ok(path) = std::env::var("CUEMOL_TRITIUM_SNAPSHOT") {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            harness
                .render()
                .expect("render shell")
                .save(path)
                .expect("save shell");
        }
        drop(harness);
        assert_eq!(app.activity, Some(Activity::Explorer));
        assert_eq!(app.bottom_tab, BottomTab::Trajectory);
        assert!(app
            .viewport_rect
            .is_some_and(|rect| rect.left() >= 44. && rect.width() > 300. && rect.height() > 100.));
    }
    #[test]
    fn compact_bottom_tabs_remain_clickable_beside_the_inspector() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        let scene = app.state.active_scene().unwrap();
        let molecule = cuemol_molstr::read_pdb_str(
            "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let object = app
            .state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("tabs", molecule);
        app.selected_object = Some(object);
        app.show_bottom_panel = true;
        app.bottom_panel_height = 240.;
        let viewports = std::cell::RefCell::new(Vec::new());
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(900., 650.))
            .build(|ctx| {
                app.ui(ctx, &mut camera);
                viewports.borrow_mut().push(app.viewport_rect.unwrap());
            });
        harness.run();
        harness.get_by_label("MD Trajectory").click();
        harness.run();
        let expanded = *viewports.borrow().last().unwrap();
        harness.get_by_label("Collapse panel").click();
        harness.run();
        assert!(viewports.borrow().last().unwrap().height() > expanded.height());
        harness.get_by_label("Expand panel").click();
        harness.run();
        assert_eq!(*viewports.borrow().last().unwrap(), expanded);
        drop(harness);
        assert_eq!(app.bottom_tab, BottomTab::Trajectory);
        assert!(app.show_bottom_panel);
        assert!(app.viewport_rect.unwrap().width() < 500.);
    }
    #[test]
    fn log_growth_keeps_the_viewport_fixed_during_atom_clicks() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.bottom_tab = BottomTab::Console;
        app.show_bottom_panel = true;
        app.bottom_panel_height = 200.0;
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024., 768.),
            )),
            ..Default::default()
        };
        for _ in 0..4 {
            let _ = ctx.run(input.clone(), |ctx| app.ui(ctx, &mut camera));
        }
        let viewport = app.viewport_rect.unwrap();
        for i in 0..100 {
            app.log_lines.push(format!("Picked atom {i}"));
        }
        for _ in 0..4 {
            let _ = ctx.run(input.clone(), |ctx| app.ui(ctx, &mut camera));
        }
        assert_eq!(app.viewport_rect.unwrap(), viewport);
    }

    #[test]
    fn inspector_property_edits_form_a_reversible_scene_change() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        let scene = app.state.active_scene().unwrap();
        let mol=cuemol_molstr::read_pdb_str("HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n").unwrap();
        let id = app
            .state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("before", mol);
        app.selected_object = Some(id);
        app.tritium_command(
            "set_node_prop",
            json!({"nodeType":"object","nodeId":id.get(),"prop":"name","value":"after"}),
            &mut camera,
        );
        assert_eq!(
            app.state.scene_manager().object(id).unwrap().name(),
            "after"
        );
        assert_eq!(app.undo_stack.len(), 1);
        app.undo(&mut camera);
        assert_eq!(
            app.state.scene_manager().object(id).unwrap().name(),
            "before"
        );
        app.redo(&mut camera);
        assert_eq!(
            app.state.scene_manager().object(id).unwrap().name(),
            "after"
        );
    }
}
