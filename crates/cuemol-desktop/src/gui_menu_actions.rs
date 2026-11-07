use super::*;
use crate::application_menu::MenuState;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_menu_navigation_keeps_history_and_quit_flushes_render_edits() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        let ctx = egui::Context::default();
        app.application_menu_action(&ctx, &mut camera, "view-perspective");
        app.application_menu_action(&ctx, &mut camera, "center-mark-axis");
        assert!(camera.perspective);
        assert_eq!(camera.center_mark, CenterMark::Axis);
        assert!(app.undo_stack.is_empty());
        assert!(app.application_menu_state(&ctx, &camera).perspective);
        app.open_native_render();
        app.ray_panel.editor.set_size([84, 60], None);
        app.ray_panel.editor.edited(1.);
        app.application_menu_action(&ctx, &mut camera, "window-render");
        assert_eq!(app.ray_panel.editor.pixels(), [84, 60]);
        app.application_menu_action(&ctx, &mut camera, "quit");
        assert!(app.exit_confirmation);
        assert!(!app.exit_ready);
        assert_eq!(
            cuemol_command::render_settings(&app.state).unwrap()["width"],
            84
        );
        assert_eq!(app.undo_stack.len(), 1);
    }

    #[test]
    fn application_menu_text_undo_does_not_undo_the_scene() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        let ctx = egui::Context::default();
        app.application_menu_action(&ctx, &mut camera, "bg-white");
        assert_eq!(app.undo_stack.len(), 1);
        let text = egui::Id::new("menu-input");
        egui::text_edit::TextEditState::default().store(&ctx, text);
        app.clipboard_focus.text = Some(text);
        app.application_menu_action(&ctx, &mut camera, "undo");
        assert_eq!(app.undo_stack.len(), 1);
        assert_eq!(app.background, SceneBackground::White);
        app.clipboard_focus.begin_frame(&ctx);
        assert!(ctx.input(|input| input.events.iter().any(|event| matches!(event, egui::Event::Key { key: egui::Key::Z, modifiers, .. } if modifiers.command))));
        app.clipboard_focus.text = None;
        app.application_menu_action(&ctx, &mut camera, "undo");
        assert!(app.undo_stack.is_empty());
        assert_eq!(app.background, SceneBackground::Black);
    }
}

impl GuiApplication {
    pub(super) fn application_menu_blocked(&self, ctx: &egui::Context) -> bool {
        ctx.memory(|memory| memory.top_modal_layer().is_some())
            || self.about.is_open()
            || self.updates.is_open()
            || self.show_options
            || self.show_scene_props
            || self.show_view_props
            || self.show_get_pdb
            || self.tool_dialog.is_some()
            || self.molecule_edits.active.is_some()
            || self.error_alert.is_some()
            || self.exit_confirmation
            || (self.render_visible && self.ray_panel.confirmation_open())
            || self.png_export_draft.is_some()
            || self.image_export_job.is_some()
            || self.scene_save_draft.is_some()
            || self.new_tab_draft.is_some()
            || self.new_camera_draft.is_some()
            || self.camera_rename_draft.is_some()
            || self.camera_delete_draft.is_some()
            || self.camera_visibility_draft.is_some()
            || self.style_properties_draft.is_some()
            || self.renderer_style_create_draft.is_some()
            || self.renderer_style_edit_draft.is_some()
            || self.animation_properties_draft.is_some()
            || self.pdb_download_receiver.is_some()
            || self.style_delete_draft.is_some()
            || self.multiple_delete_draft.is_some()
            || self.close_tab_draft.is_some()
            || self.reload_scene_draft.is_some()
            || self.atom_intr_edit_draft.is_some()
            || self.renderer_setup_draft.is_some()
            || self.md_trajectory_draft.is_some()
            || self.object_properties_draft.is_some()
            || self.renderer_properties_draft.is_some()
            || self.renderer_group_rename_draft.is_some()
            || self.new_renderer.is_some()
            || self.new_renderer_group.is_some()
    }

    pub(super) fn application_menu_state(
        &self,
        ctx: &egui::Context,
        camera: &CameraController,
    ) -> MenuState {
        MenuState {
            has_scene: self.state.active_scene().is_some(),
            background: match self.background {
                SceneBackground::White => "white",
                SceneBackground::Black => "black",
                _ => "",
            }
            .into(),
            color_proofing: self.color_proofing,
            perspective: camera.perspective,
            center_mark: match camera.center_mark {
                CenterMark::Cross => "cross",
                CenterMark::Axis => "axis",
                CenterMark::None => "none",
            }
            .into(),
            palette_visible: self.state.config_value("layout:tool-palette-collapsed") != "true",
            recent: self
                .recent_paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            blocked: self.application_menu_blocked(ctx),
        }
    }

    pub(super) fn application_menu_action(
        &mut self,
        ctx: &egui::Context,
        camera: &mut CameraController,
        id: &str,
    ) {
        if let Some(path) = id.strip_prefix("recent:") {
            self.open_path = path.into();
            self.request_open_recent_path(camera);
            return;
        }
        match id {
            "about" => self.about.open(cuemol_command::about::native_rows(
                cuemol_command::about::Frontend::Egui,
            )),
            "check-updates" => self.updates.check(ctx),
            "options" => self.open_options_dialog(),
            "quit" => self.request_application_exit(),
            "new-tab" => self.open_new_tab_dialog(),
            "open-file" => self.choose_open_file(camera),
            "clear-recent" => self.clear_recent_paths(),
            "save-file-as" => self.choose_save_selected_object(),
            "save-current-view" => self.choose_save_current_view(camera),
            "close-tab" => self.close_current_scene(camera),
            "open-scene" => self.choose_open_scene(camera),
            "reload-scene" => self.reload_current_path(camera),
            "save-scene" => self.save_current_scene(camera),
            "save-scene-as" => self.choose_save_scene(),
            "undo" if self.clipboard_focus.text.is_none() => self.undo(camera),
            "redo" if self.clipboard_focus.text.is_none() => self.redo(camera),
            "cut" | "copy" | "paste" | "select-all" | "undo" | "redo" => {
                self.clipboard_action(ctx, id)
            }
            "clear-undo" => self.clear_undo_data(),
            "merge-mol" => self.open_molecule_edit(cuemol_ui::molecule_edit::Kind::Merge),
            "delete-mol-atoms" => self.open_molecule_edit(cuemol_ui::molecule_edit::Kind::Delete),
            "change-chain-id" => self.open_molecule_edit(cuemol_ui::molecule_edit::Kind::Chain),
            "change-resid-num" => self.open_molecule_edit(cuemol_ui::molecule_edit::Kind::Residue),
            "image-render" => self.request_native_render(false),
            "movie-render" => self.request_native_render(true),
            "export-png" => {
                self.choose_export_image(camera, cuemol_command::image_export::ImageExportKind::Png)
            }
            "export-umbreon" => self.choose_export_image(
                camera,
                cuemol_command::image_export::ImageExportKind::Umbreon,
            ),
            "export-pov" => self.choose_export_geometry(
                camera,
                cuemol_command::scene_export::SceneExportFormat::Pov,
            ),
            "export-stl" => self.choose_export_geometry(
                camera,
                cuemol_command::scene_export::SceneExportFormat::Stl,
            ),
            "export-mqo" => self.choose_export_geometry(
                camera,
                cuemol_command::scene_export::SceneExportFormat::Mqo,
            ),
            "bg-white" | "bg-black" => {
                let value = if id == "bg-white" { "white" } else { "black" };
                self.background = if id == "bg-white" {
                    SceneBackground::White
                } else {
                    SceneBackground::Black
                };
                self.persist_scene_background(value);
            }
            "color-proof" => {
                self.color_proofing = !self.color_proofing;
                self.persist_scene_color_proofing();
            }
            "scene-props" => self.open_scene_properties(),
            "view-perspective" => camera.perspective = true,
            "view-orthographic" => camera.perspective = false,
            "center-mark-cross" => camera.center_mark = CenterMark::Cross,
            "center-mark-axis" => camera.center_mark = CenterMark::Axis,
            "center-mark-none" => camera.center_mark = CenterMark::None,
            "view-tool-palette" => {
                let collapsed = self.state.config_value("layout:tool-palette-collapsed") != "true";
                self.state
                    .put_config_value("layout:tool-palette-collapsed", collapsed.to_string());
                if let Err(error) = self.persist_config_state() {
                    self.set_status(error);
                }
            }
            "view-props" => self.open_view_properties(camera),
            "mol-superpose" => self.open_superposition_dialog(),
            "interaction" => self.open_interaction_dialog(),
            "reassign-2ndry" => self.open_secondary_dialog(),
            "morph-anim" => self.open_morph_tool(),
            "mol-surf" => self.open_surface_generation_tool(),
            "surf-cutter" => self.open_surface_cutter_tool(),
            "apbs" => self.open_apbs_tool(),
            "window-main" => {
                self.ray_panel.focused = false;
                self.main_focus_requested = true;
            }
            "window-render" => self.open_native_render(),
            _ => {}
        }
    }
}
