use super::*;
use cuemol_ui::shell::{self, Activity, Icon};
#[cfg(test)]
use serde_json::json;

impl GuiApplication {
    pub(super) fn cancel_numeric_previews(&mut self, camera: &mut CameraController) {
        let mut changes = self.inspector.cancel_numeric_edit();
        changes.extend(self.density_panel.cancel());
        changes.extend(self.animation_inspector.cancel());
        for (tool, args) in changes {
            self.tritium_command(tool, args, camera);
        }
    }

    pub(super) fn tritium_layout(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        if self
            .activity
            .is_some_and(|activity| !activity.enabled(&self.state))
        {
            self.activity = Some(Activity::Explorer);
        }
        if !self.state.plugin_tab_enabled(self.bottom_tab.config_name()) {
            self.bottom_tab = BottomTab::Output;
        }
        if !self.state.plugin_enabled("mdtools") {
            self.trajectory_playing = false;
            self.md_trajectory_draft = None;
        }
        if !self.state.plugin_enabled("agent") {
            self.agent.cancel("AI Agent plugin disabled.");
            self.agent.visible = false;
        }
        if !self
            .state
            .console_dialect_enabled(self.console.dialect.label())
        {
            self.console.dialect = cuemol_command::console::Dialect::CueMol;
        }
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
        if shell::activity_bar_filtered(
            ctx,
            &mut self.activity,
            |activity| activity.enabled(&self.state),
            |_, _| {},
        ) {
            self.open_options_dialog();
        }
        if before != self.activity {
            self.show_workspace = self.activity.is_some();
            self.state.put_config_value(
                "layout:activity",
                self.activity.map(Activity::key).unwrap_or("none"),
            );
        }
        self.density_panel.rects.clear();
        self.symmetry_panel.rects.clear();
        self.camera_panel.rects.clear();
        if self.show_workspace {
            self.tritium_sidebar(ctx, camera);
        }
        for (tool, args) in self.symmetry_panel.dialog(ctx, &self.state) {
            self.tritium_command(tool, args, camera);
        }
        let camera_actions = self.camera_panel.dialog(ctx, &self.state);
        self.camera_panel_actions(ctx, camera_actions, camera);
        if self.density_panel.rects.is_empty() {
            for (tool, args) in self.density_panel.cancel() {
                self.tritium_command(tool, args, camera);
            }
        }
        if let Some(context) = &mut self.inspector_context {
            context.id = if context.kind == "scene" {
                self.state.active_scene().map(|id| id.get())
            } else if context.kind == "view" {
                self.state
                    .active_scene()
                    .and_then(|id| self.state.scene_manager().scene(id))
                    .and_then(|scene| scene.active_view_uid())
                    .map(|id| id.get())
            } else {
                context.id
            };
        }
        let target =
            self.inspector_context
                .clone()
                .or_else(|| match self.current_workspace_target() {
                    Some(NativeWorkspaceTarget::Object { owner, .. }) => {
                        Some(cuemol_ui::InspectorTarget {
                            kind: "object".into(),
                            id: Some(match owner {
                                NativePropertyObject::Molecule(id)
                                | NativePropertyObject::DensityMap(id)
                                | NativePropertyObject::Surface(id)
                                | NativePropertyObject::Lightweight(id) => id.get(),
                            }),
                        })
                    }
                    Some(NativeWorkspaceTarget::Renderer { renderer_uid, .. }) => {
                        Some(cuemol_ui::InspectorTarget {
                            kind: "renderer".into(),
                            id: Some(renderer_uid.get()),
                        })
                    }
                    _ => None,
                });
        if self.activity == Some(Activity::Actions) {
            for (tool, args) in self.inspector.cancel_numeric_edit() {
                self.tritium_command(tool, args, camera);
            }
            for (tool, args) in self.animation_inspector.cancel() {
                self.tritium_command(tool, args, camera);
            }
            self.actions_panel.inspector(ctx);
        } else if let Some(uid) = target
            .as_ref()
            .filter(|target| target.kind == "animation")
            .and_then(|target| target.id)
        {
            for (tool, args) in self.inspector.cancel_numeric_edit() {
                self.tritium_command(tool, args, camera);
            }
            for (tool, args) in
                self.animation_inspector
                    .panel(ctx, &self.state, uid, self.snapshot_revision)
            {
                self.tritium_command(tool, args, camera);
            }
        } else {
            for (tool, args) in self.animation_inspector.cancel() {
                self.tritium_command(tool, args, camera);
            }
            let mut view = camera.to_qsys_camera("__current");
            self.inspector
                .inspect(&mut self.state, &mut view, target, self.snapshot_revision);
            for (tool, args) in self.inspector.panel(ctx, &self.state) {
                self.tritium_command(tool, args, camera);
            }
        }
        self.tritium_bottom(ctx, camera);
        self.scene_tab_strip(ctx, camera);
        self.central_view(ctx, camera);
        self.viewport_palette(ctx, camera);
        if let Some(intent) = self.actions_panel.dialogs(ctx, &self.state) {
            self.apply_action_intent(intent, camera, ctx);
        }
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
                    for (label, icon, undo) in
                        [("Undo", Icon::Undo, true), ("Redo", Icon::Redo, false)]
                    {
                        let stack = if undo {
                            &self.undo_stack
                        } else {
                            &self.redo_stack
                        };
                        let descriptions: Vec<_> = stack
                            .iter()
                            .rev()
                            .map(|entry| entry.record.label())
                            .collect();
                        let (steps, _) =
                            shell::toolbar_history(ui, icon, label, compact, &descriptions);
                        for _ in 0..steps.unwrap_or(0) {
                            if undo {
                                self.undo(camera);
                            } else {
                                self.redo(camera);
                            }
                        }
                    }
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
                if activity == Activity::Catalog {
                    cuemol_ui::catalog::show(ui);
                    return;
                }
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
        let actions = self.camera_panel.show(ui, &self.state);
        self.camera_panel_actions(ui.ctx(), actions, camera);
        self.clipboard_focus
            .region(cuemol_ui::clipboard::Scope::Cameras, ui);
    }
    fn camera_panel_actions(
        &mut self,
        ctx: &egui::Context,
        actions: Vec<cuemol_ui::camera::Intent>,
        camera: &mut CameraController,
    ) {
        use cuemol_ui::camera::Intent;
        if actions.is_empty() && self.camera_panel.rects.is_empty() {
            return;
        }
        let Some(scene) = self.state.active_scene() else {
            return;
        };
        for action in actions {
            self.select_camera_panel_row(scene);
            let selection = self.workspace_aux_selection;
            match action {
                Intent::Command(tool, args) => self.tritium_command(tool, args, camera),
                Intent::Clipboard(action) => {
                    self.clipboard_focus.scope = Some(cuemol_ui::clipboard::Scope::Cameras);
                    self.clipboard_focus.text = None;
                    self.clipboard_action(ctx, action);
                }
                Intent::Load => {
                    self.choose_load_workspace_camera_file(scene, camera);
                    if let Some(saved) = self
                        .workspace_aux_selection
                        .and_then(|selection| self.camera_summary_for_selection(selection))
                    {
                        self.camera_panel.selected = Some(saved.name);
                    }
                }
                Intent::SaveFile(save_as) => {
                    if let Some(selection) = selection {
                        self.choose_save_workspace_camera_file(selection, save_as);
                    }
                }
                Intent::Reload => {
                    if let Some(selection) = selection {
                        self.reload_workspace_camera_file(selection);
                    }
                }
            }
        }
        self.select_camera_panel_row(scene);
    }
    fn select_camera_panel_row(&mut self, scene: cuemol_core::Uid) {
        if let Some(index) = self
            .state
            .camera_summaries(scene)
            .iter()
            .position(|camera| self.camera_panel.selected.as_ref() == Some(&camera.name))
        {
            self.workspace_aux_selection = Some(WorkspaceAuxSelection::Camera {
                scene_uid: scene,
                camera_index: index,
            });
            self.selected_object = None;
            self.selected_renderer = None;
        } else if matches!(
            self.workspace_aux_selection,
            Some(WorkspaceAuxSelection::Camera { .. })
        ) {
            self.workspace_aux_selection = None;
        }
    }
    fn action_sidebar(&mut self, ui: &mut egui::Ui, camera: &mut CameraController) {
        let done: Vec<_> = self
            .undo_stack
            .iter()
            .map(|entry| entry.record.clone())
            .collect();
        let future: Vec<_> = self
            .redo_stack
            .iter()
            .map(|entry| entry.record.clone())
            .collect();
        let busy = self.console_run.is_some();
        egui::CollapsingHeader::new("History")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("action_history")
                    .max_height((ui.available_height() * 0.5).max(100.))
                    .show(ui, |ui| {
                        if let Some(intent) = self.actions_panel.history(ui, &done, &future, busy) {
                            self.apply_action_intent(intent, camera, ui.ctx());
                        }
                    });
            });
        let targets = self.action_targets();
        let names: Vec<_> = targets.iter().map(|(_, name)| name.clone()).collect();
        egui::CollapsingHeader::new("Actions")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("action_library")
                    .show(ui, |ui| {
                        if let Some(intent) = self.actions_panel.library(ui, &names, busy) {
                            self.apply_action_intent(intent, camera, ui.ctx());
                        }
                    });
            });
    }

    pub(super) fn tritium_command(
        &mut self,
        name: &str,
        args: serde_json::Value,
        camera: &mut CameraController,
    ) {
        self.try_tritium_command(name, args, camera);
    }

    pub(super) fn try_tritium_command(
        &mut self,
        name: &str,
        args: serde_json::Value,
        camera: &mut CameraController,
    ) -> bool {
        let mut view = camera.to_qsys_camera("__current");
        if name == "edit_animation_property" {
            self.cancel_console("Scene changed outside the console; remaining commands cancelled.");
            return match cuemol_command::prepare_animation_edit(&mut self.state, &mut view, &args) {
                Ok(Some((tool, command))) => self.try_tritium_command(tool, command, camera),
                Ok(None) => {
                    *camera = CameraController::from_qsys_camera(&view);
                    self.sync_active_scene_properties();
                    self.rebuild_snapshot();
                    true
                }
                Err(error) => {
                    self.set_status(error.to_string());
                    false
                }
            };
        }
        if name == "edit_node_property" {
            self.cancel_console("Scene changed outside the console; remaining commands cancelled.");
            return match cuemol_command::prepare_property_edit(&mut self.state, &mut view, &args) {
                Ok(Some(command)) => self.try_tritium_command("set_node_prop", command, camera),
                Ok(None) => {
                    self.sync_active_scene_properties();
                    self.rebuild_snapshot();
                    true
                }
                Err(error) => {
                    self.set_status(error.to_string());
                    false
                }
            };
        }
        if cuemol_command::tool_changes_scene(name) {
            self.cancel_numeric_previews(camera);
        }
        let record_history = cuemol_command::tool_records_history(name)
            && !cuemol_command::is_view_property_write(&self.state, name, &args);
        let undo = record_history.then(|| {
            let mut undo = self.capture_undo();
            undo.camera = Some(view.clone());
            undo.record =
                cuemol_command::console::actions::Record::operation(&self.state, name, &args);
            undo
        });
        match cuemol_command::call_tool(&mut self.state, &mut view, name, &args) {
            Ok(_) => {
                if let Some(mut undo) =
                    undo.filter(|before| !before.state.same_scene_undo_state(&self.state))
                {
                    cuemol_command::retain_animation_seed_in_undo(
                        &mut undo.state,
                        &self.state,
                        name,
                    );
                    self.push_undo_snapshot(undo);
                }
                *camera = CameraController::from_qsys_camera(&view);
                if name == "set_mol_selection" {
                    self.selected_object = args["objId"].as_u64().map(cuemol_core::Uid::new);
                    self.molstruct_object = self.selected_object;
                    self.selected_renderer = None;
                    self.workspace_aux_selection = None;
                    self.selection_text = args["selection"].as_str().unwrap_or_default().into();
                }
                if cuemol_command::selection::remember_property_edit(&mut self.state, name, &args) {
                    self.selection_history = self.state.selection_history();
                }
                self.sync_active_scene_properties();
                self.rebuild_snapshot();
                true
            }
            Err(error) => {
                if let Some(undo) = undo {
                    self.restore_gui_undo_state(undo);
                }
                self.set_status(error.to_string());
                false
            }
        }
    }
    fn tritium_bottom(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        let was_expanded = self.show_bottom_panel;
        let panel = egui::TopBottomPanel::bottom(if was_expanded {
            "tritium-bottom"
        } else {
            "tritium-bottom-collapsed"
        })
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
            ui.set_min_height(ui.available_height());
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
                        if !self.state.plugin_tab_enabled(tab.config_name()) {
                            continue;
                        }
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
                    self.animation_timeline_section(ui, camera);
                }
                BottomTab::Sequence => self.sequence_tab(ui, camera),
                BottomTab::Trajectory => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.trajectory_tab(ui));
                }
            }
        });
        if was_expanded && self.show_bottom_panel {
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
    fn plugin_options_apply_cancel_and_remove_selected_contributions() {
        let mut app = GuiApplication::new();
        let directory = tempfile::tempdir().unwrap();
        app.config_path = directory.path().join("config.tsv").to_string_lossy().into();
        let mut camera = CameraController::default();
        for accept in [false, true] {
            app.open_options_dialog();
            app.settings_panel.category = cuemol_ui::settings::Category::Installed;
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1280., 900.))
                .build(|ctx| app.ui(ctx, &mut camera));
            harness.run();
            assert!(harness.query_by_label("MD Trajectory").is_none());
            assert!(harness.query_by_label("Component Catalog").is_some());
            harness.get_by_label("MD Tools").click();
            harness.get_by_label("Component Catalog").click();
            harness.run();
            harness
                .get_by_label(if accept { "OK" } else { "Cancel" })
                .click();
            harness.run();
            assert_eq!(harness.query_by_label("MD Trajectory").is_some(), accept);
            if accept {
                harness.get_by_label("Component Catalog").click();
                harness.run();
                assert!(harness.query_by_label("Forms").is_some());
            }
            drop(harness);
            assert_eq!(app.state.plugin_enabled("mdtools"), accept);
        }
        app.bottom_tab = BottomTab::Trajectory;
        app.trajectory_playing = true;
        let old = app.capture_undo();
        app.undo_stack.push(old);
        app.state.set_plugin_enabled("mdtools", false).unwrap();
        app.state.set_plugin_enabled("catalog", false).unwrap();
        app.undo(&mut camera);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280., 900.))
            .build(|ctx| app.ui(ctx, &mut camera));
        harness.run();
        assert!(harness.query_by_label("MD Trajectory").is_none());
        assert!(harness.query_by_label("Component Catalog").is_none());
        drop(harness);
        assert_eq!(app.bottom_tab, BottomTab::Output);
        assert_eq!(app.activity, Some(Activity::Explorer));
        assert!(!app.trajectory_playing);
        let mut restored = DesktopAppState::new();
        restored.load_config_text(&app.state.config_text()).unwrap();
        assert!(!restored.plugin_enabled("mdtools"));
    }

    #[test]
    fn camera_keyboard_navigation_retains_list_focus_for_rename() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.activity = Some(Activity::View);
        app.show_workspace = true;
        let scene = app.state.active_scene().unwrap();
        for name in ["__current", "front", "back"] {
            let mut saved = cuemol_qsys::Camera::new();
            saved.set_name(name);
            app.state.save_scene_camera(scene, saved, false).unwrap();
        }
        let ctx = egui::Context::default();
        let frame = |app: &mut GuiApplication, camera: &mut CameraController, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1024., 768.),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| app.ui(ctx, camera),
            );
        };
        for _ in 0..3 {
            frame(&mut app, &mut camera, vec![]);
        }
        let [x, y, w, h] = app.camera_panel.rects["camera:front"];
        let pos = egui::pos2(x + w / 2., y + h / 2.);
        for pressed in [true, false] {
            frame(
                &mut app,
                &mut camera,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        for key in [egui::Key::ArrowDown, egui::Key::F2] {
            frame(
                &mut app,
                &mut camera,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
            frame(
                &mut app,
                &mut camera,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
        }
        assert_eq!(app.camera_panel.selected.as_deref(), Some("back"));
        assert!(app.camera_panel.rects.contains_key("Camera rename"));
    }

    #[test]
    fn inspector_view_writes_and_empty_resets_keep_redo() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.tritium_command(
            "set_node_prop",
            json!({"nodeType":"scene","prop":"bgcolor","value":"red"}),
            &mut camera,
        );
        app.undo(&mut camera);
        assert_eq!(app.redo_stack.len(), 1);
        app.tritium_command("reset_node_props", json!({"nodeType":"scene"}), &mut camera);
        app.tritium_command(
            "set_node_prop",
            json!({"nodeType":"view","prop":"zoom","value":"25"}),
            &mut camera,
        );
        assert_eq!(camera.to_qsys_camera("test").zoom(), 25.);
        assert!(app.undo_stack.is_empty());
        assert_eq!(app.redo_stack.len(), 1);
        app.redo(&mut camera);
        assert_eq!(
            app.state
                .scene_manager()
                .scene(app.state.active_scene().unwrap())
                .unwrap()
                .scene_attribute("bgcolor"),
            Some("red")
        );
    }
    #[test]
    fn tritium_activity_buttons_switch_and_collapse_the_sidebar() {
        let mut app = GuiApplication::new();
        app.state.set_plugin_enabled("mdtools", true).unwrap();
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
    fn animation_bottom_panel_preserves_configured_height_with_few_elements() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.bottom_tab = BottomTab::Animation;
        app.show_bottom_panel = true;
        app.bottom_panel_height = 300.;
        app.try_tritium_command(
            "add_anim",
            serde_json::json!({"type":"spin","name":"single"}),
            &mut camera,
        );
        let ctx = egui::Context::default();
        for frame in 0..6 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1024., 768.),
                    )),
                    time: Some(frame as f64 / 60.),
                    focused: true,
                    ..Default::default()
                },
                |ctx| app.ui(ctx, &mut camera),
            );
            assert!(
                (app.bottom_panel_height - 300.).abs() < 2.,
                "frame {frame}: {}",
                app.bottom_panel_height
            );
        }
    }

    #[test]
    fn compact_bottom_tabs_remain_clickable_beside_the_inspector() {
        let mut app = GuiApplication::new();
        app.state.set_plugin_enabled("mdtools", true).unwrap();
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
        for source in [
            "hide probe",
            "hide probe\nshow probe",
            "help\n".repeat(8).as_str(),
        ] {
            app.console.source = source.into();
            for _ in 0..4 {
                let _ = ctx.run(input.clone(), |ctx| app.ui(ctx, &mut camera));
            }
            assert_eq!(app.viewport_rect.unwrap(), viewport);
        }
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
