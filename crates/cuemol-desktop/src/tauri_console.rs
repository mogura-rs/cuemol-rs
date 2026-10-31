use super::*;
use cuemol_command::console::{same_camera_view, Action, Dialect};
use cuemol_ui::console_runtime::{ConsoleRunner, ConsoleStep};
use serde_json::Value;

pub(super) struct Execution {
    checkpoint: cuemol_ui::console_runtime::ConsoleCheckpoint,
    runner: ConsoleRunner,
    undo: Option<AppModelUndo>,
    revision: u64,
    camera: Camera,
    changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Update {
    snapshot: Option<Box<AppSnapshot>>,
    running: bool,
}

pub(super) fn dialect(name: &str) -> Result<Dialect, String> {
    match name {
        "CueMol" => Ok(Dialect::CueMol),
        "PyMOL" => Ok(Dialect::PyMol),
        "JavaScript" => Ok(Dialect::JavaScript),
        _ => Err(format!("Unknown console dialect: {name}")),
    }
}

impl AppModel {
    pub(super) fn start_library_action(
        &mut self,
        source: &str,
        macro_name: Option<&str>,
        label: &str,
        targets: &[u64],
    ) -> Result<Update, String> {
        let runner = ConsoleRunner::library(source, macro_name, targets)?;
        self.stop_console();
        self.persist_camera()?;
        self.console_session
            .begin_submission(label, Dialect::CueMol);
        self.console_session.sync_revision(self.snapshot_revision);
        self.console_run = Some(Execution {
            checkpoint: cuemol_ui::console_runtime::ConsoleCheckpoint::new(
                &self.desktop,
                &self.camera,
                &self.selection,
            ),
            runner,
            undo: Some(self.capture_script_undo()),
            revision: self.snapshot_revision,
            camera: self.camera.clone(),
            changed: false,
        });
        self.poll_console()
    }
    pub(super) fn start_console(
        &mut self,
        source: &str,
        dialect: Dialect,
    ) -> Result<Update, String> {
        self.console_run = None;
        self.console_session.finish_submission()?;
        self.console_session.begin_submission(source, dialect);
        self.persist_camera()?;
        self.console_session.sync_revision(self.snapshot_revision);
        self.console_run = Some(Execution {
            checkpoint: cuemol_ui::console_runtime::ConsoleCheckpoint::new(
                &self.desktop,
                &self.camera,
                &self.selection,
            ),
            runner: ConsoleRunner::new(source, dialect)?,
            undo: Some(self.capture_script_undo()),
            revision: self.snapshot_revision,
            camera: self.camera.clone(),
            changed: false,
        });
        self.poll_console()
    }

    pub(super) fn stop_console(&mut self) -> Update {
        if self.console_run.take().is_some() {
            if let Err(error) = self.console_session.finish_submission() {
                self.log_lines.push(error);
            }
            self.status = "Console stopped; completed edits are kept.".into();
            self.log_lines.push(self.status.clone());
        }
        self.console_update(true)
    }

    fn console_update(&self, changed: bool) -> Update {
        Update {
            snapshot: changed.then(|| Box::new(self.snapshot())),
            running: self.console_run.is_some(),
        }
    }

    fn console_result(
        &mut self,
        run: &mut Execution,
        name: &str,
        args: &Value,
        result: &Value,
    ) -> Result<(), String> {
        self.log_lines.extend(
            cuemol_command::console::format_output(result)
                .lines()
                .take(200)
                .map(str::to_owned),
        );
        if name == "set_mol_selection" && !result.is_null() {
            self.selection = args["selection"].as_str().unwrap_or("").into();
        } else if name == "execute_script" {
            if let Some(selection) = result["selection"].as_str() {
                self.selection = selection.into();
            }
            self.restore_script_view();
        }
        let changed = cuemol_command::tool_records_history(name)
            && run
                .checkpoint
                .update(&self.desktop, &self.camera, &self.selection);
        self.console_session
            .complete_record(changed && run.checkpoint.scene_changed() && !result.is_null());
        if name == "save_scene" && !result.is_null() {
            if let Some(scene) = self.desktop.active_scene() {
                self.modified_scenes.remove(&scene);
            }
        }
        if changed && run.checkpoint.scene_changed() && cuemol_command::tool_records_history(name) {
            if let Some(before) = run.undo.take() {
                self.status = "Console command".into();
                self.push_undo(before);
                if let Some(entry) = self.undo_stack.last_mut() {
                    entry.record = self.console_session.action_record.clone();
                    entry.description = entry.record.label();
                }
            }
        }
        if changed && cuemol_command::tool_changes_scene(name) {
            self.console_session.scene_changed();
            run.changed = true;
            self.invalidate_snapshot(false);
            self.touch_structure_details();
            self.sync_active_scene_properties();
        }
        run.revision = self.snapshot_revision;
        run.camera = self.camera.clone();
        self.console_session.accept_revision(self.snapshot_revision);
        Ok(())
    }

    pub(super) fn poll_console(&mut self) -> Result<Update, String> {
        let Some(mut run) = self.console_run.take() else {
            return Ok(self.console_update(false));
        };
        if run.revision != self.snapshot_revision || !same_camera_view(&run.camera, &self.camera) {
            self.status =
                "Scene or camera changed outside the console; remaining commands cancelled.".into();
            self.log_lines.push(self.status.clone());
            if let Err(error) = self.console_session.finish_submission() {
                self.log_lines.push(error);
            }
            return Ok(self.console_update(true));
        }
        let prior_lines = self.log_lines.len();
        let prior_status = self.status.clone();
        let mut keep = true;
        for _ in 0..64 {
            let result = (|| -> Result<bool, String> {
                let step = run.runner.next(
                    &mut self.console_session,
                    &mut self.desktop,
                    &mut self.camera,
                    Arc::new(|| {}),
                )?;
                match step {
                    ConsoleStep::Pending => {
                        self.status = "Console running".into();
                        return Ok(false);
                    }
                    ConsoleStep::Done => {
                        self.status = "Console executed".into();
                        keep = false;
                        return Ok(false);
                    }
                    ConsoleStep::Completed { name, args, result } => {
                        self.console_result(&mut run, &name, &args, &result)?
                    }
                    ConsoleStep::Action(action) => match action {
                        Action::Tool { name, args } => {
                            if matches!(name.as_str(), "undo" | "redo") {
                                if run.changed || !run.runner.allows_undo() {
                                    return Err("Undo and redo must be submitted alone".into());
                                }
                                if name == "undo" {
                                    self.undo(0)?;
                                } else {
                                    self.redo(0)?;
                                }
                            } else if name == "clear_undo" {
                                self.undo_stack.clear();
                                self.redo_stack.clear();
                            } else {
                                let result = cuemol_command::call_tool(
                                    &mut self.desktop,
                                    &mut self.camera,
                                    &name,
                                    &args,
                                );
                                match result {
                                    Ok(value) => {
                                        self.console_result(&mut run, &name, &args, &value)?
                                    }
                                    Err(error) => {
                                        self.console_result(&mut run, &name, &args, &Value::Null)?;
                                        return Err(error.to_string());
                                    }
                                }
                            }
                        }
                        Action::Transaction(label) => {
                            self.console_session.begin_transaction(&label)?;
                            run.undo = Some(self.capture_script_undo());
                            run.changed = false;
                        }
                        Action::WriteFile { path, bytes, .. } => {
                            fs::write(&path, bytes.as_ref()).map_err(|e| e.to_string())?;
                            self.log_lines.push(format!("Saved {path}"));
                        }
                        Action::Output(text) | Action::Echo(text) => self
                            .log_lines
                            .extend(text.lines().take(200).map(str::to_owned)),
                        Action::Clear => self.log_lines.clear(),
                        Action::Quit => {
                            self.log_lines.push("Close this window to exit.".into());
                            keep = false;
                            return Ok(false);
                        }
                        Action::Source(_)
                        | Action::Native(_)
                        | Action::Qax(_)
                        | Action::QaxOp(_)
                        | Action::RememberSetting { .. }
                        | Action::KeepChain { .. }
                        | Action::Render { .. } => {
                            unreachable!("console runner expands this action")
                        }
                    },
                }
                Ok(true)
            })();
            match result {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    self.log_lines.push(error);
                    self.status = "Console error".into();
                    if run.runner.skip_failed_target() {
                        continue;
                    }
                    keep = false;
                    break;
                }
            }
        }
        let changed = self.log_lines.len() != prior_lines
            || self.status != prior_status
            || run.revision != self.snapshot_revision;
        if self.log_lines.len() > 1000 {
            self.log_lines.drain(..self.log_lines.len() - 1000);
        }
        if keep {
            run.revision = self.snapshot_revision;
            run.camera = self.camera.clone();
            self.console_run = Some(run);
        } else if let Err(error) = self.console_session.finish_submission() {
            self.log_lines.push(error);
        }
        Ok(self.console_update(changed))
    }
}
