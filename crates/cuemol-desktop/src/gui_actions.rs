use super::*;
use cuemol_command::console::actions;
use cuemol_ui::actions::Intent;

impl GuiApplication {
    pub(super) fn action_targets(&self) -> Vec<(u64, String)> {
        let mut targets = self.workspace_multiple_selection.clone();
        if targets.is_empty() {
            targets.extend(self.current_workspace_target());
        }
        let mut ids = std::collections::BTreeSet::new();
        for target in targets {
            match target {
                NativeWorkspaceTarget::Object { scene_uid, owner }
                | NativeWorkspaceTarget::Renderer {
                    scene_uid, owner, ..
                } if Some(scene_uid) == self.state.active_scene() => {
                    ids.insert(owner.uid().get());
                }
                _ => {}
            }
        }
        actions::targets(&self.state, &ids.into_iter().collect::<Vec<_>>())
    }

    pub(super) fn apply_action_intent(
        &mut self,
        intent: Intent,
        camera: &mut CameraController,
        ctx: &egui::Context,
    ) {
        let directory = cuemol_command::config::directory().join("actions");
        let result = (|| -> std::result::Result<(), String> {
            match intent {
                Intent::MoveTo(index) => {
                    if self.console_run.is_some() {
                        return Err("Wait for the current action or stop it first.".into());
                    }
                    let index = index.min(self.undo_stack.len() + self.redo_stack.len());
                    while self.undo_stack.len() > index {
                        self.undo(camera);
                    }
                    while self.undo_stack.len() < index {
                        self.redo(camera);
                    }
                }
                Intent::Run {
                    source,
                    macro_name,
                    label,
                } => {
                    let targets = self
                        .action_targets()
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>();
                    let runner = cuemol_ui::console_runtime::ConsoleRunner::library(
                        &source,
                        macro_name.as_deref(),
                        &targets,
                    )?;
                    self.cancel_console("Replaced by an action.");
                    self.console_session
                        .begin_submission(&label, cuemol_command::console::Dialect::CueMol);
                    self.console_session.sync_revision(self.snapshot_revision);
                    let view = camera.to_qsys_camera("__current");
                    let _ = self.state.set_active_view_camera(&view);
                    let mut undo = self.capture_script_undo();
                    undo.camera = Some(view.clone());
                    self.console_run = Some(GuiConsoleExecution {
                        checkpoint: cuemol_ui::console_runtime::ConsoleCheckpoint::new(
                            &self.state,
                            &view,
                            &self.selection_text,
                        ),
                        runner,
                        undo: Some(undo),
                        camera: view,
                        revision: self.snapshot_revision,
                        changed: false,
                    });
                    self.poll_console(ctx);
                    self.apply_script_view(camera);
                }
                Intent::Export { name, source } => {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Save actions")
                        .add_filter("CueMol actions", &["qax"])
                        .set_file_name(&name)
                        .save_file()
                    {
                        std::fs::write(path, source).map_err(|error| error.to_string())?;
                    }
                }
                Intent::Save { name, source } => {
                    actions::save_library(&directory, &name, &source)?;
                    self.actions_panel.saved();
                }
                Intent::Rename { from, to } => {
                    actions::rename_library_file(&directory, &from, &to)?;
                    self.actions_panel.saved();
                }
                Intent::Delete(name) => {
                    actions::delete_library_file(&directory, &name)?;
                }
                Intent::Import => {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Import actions")
                        .add_filter("CueMol actions", &["qax"])
                        .pick_file()
                    {
                        let source =
                            std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
                        let name = path.file_stem().unwrap_or_default().to_string_lossy();
                        actions::save_library(&directory, &name, &source)?;
                    }
                }
                Intent::Refresh => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.actions_panel.error = error.clone();
            self.set_status(error);
        }
        self.actions_panel.files = actions::read_library(&directory);
    }
}
