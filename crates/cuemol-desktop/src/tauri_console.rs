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
    pub(super) snapshot: Option<Box<AppSnapshot>>,
    running: bool,
}

pub(super) fn dialect(name: &str) -> Result<Dialect, String> {
    match name {
        "CueMol" => Ok(Dialect::CueMol),
        "PyMOL" => Ok(Dialect::PyMol),
        "ChimeraX" => Ok(Dialect::ChimeraX),
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
        self.sync_view_camera()?;
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
        if !self.desktop.console_dialect_enabled(dialect.label()) {
            return Err("Console plugin or dialect is disabled.".into());
        }
        if let Some(pending) = self.remote_console.take() {
            pending.finish(
                &self.log_lines,
                false,
                "Console replaced by another submission.",
            );
        }
        self.console_run = None;
        self.console_session.finish_submission()?;
        self.console_session.begin_submission(source, dialect);
        self.sync_view_camera()?;
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
        if let Some(pending) = self.remote_console.take() {
            pending.finish(
                &self.log_lines,
                false,
                "Console stopped; completed edits are kept.",
            );
        }
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
        let changed = run
            .checkpoint
            .update(&self.desktop, &self.camera, &self.selection);
        let view_changed = cuemol_command::is_view_property_write(&self.desktop, name, args);
        self.console_session.complete_record(
            changed && (run.checkpoint.scene_changed() || view_changed) && !result.is_null(),
        );
        if name == "save_scene" && !result.is_null() {
            if let Some(scene) = self.desktop.active_scene() {
                self.desktop.clear_scene_modified(scene);
            }
        }
        if changed && (cuemol_command::tool_changes_scene(name) || view_changed) {
            self.console_session.scene_changed();
            run.changed = true;
            self.invalidate_snapshot(false);
            self.touch_structure_details();
            self.sync_active_scene_properties();
        }
        if changed && run.checkpoint.scene_changed() && cuemol_command::tool_records_history(name) {
            if let Some(before) = run.undo.as_mut() {
                cuemol_command::retain_animation_seed_in_undo(
                    &mut before.desktop,
                    &self.desktop,
                    name,
                );
                self.update_grouped_undo(
                    before.clone(),
                    self.console_session.action_record.clone(),
                );
            }
        }
        run.revision = self.snapshot_revision;
        run.camera = self.camera.clone();
        self.console_session.accept_revision(self.snapshot_revision);
        Ok(())
    }

    fn advance_console(&mut self) -> Result<Update, String> {
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
                        | Action::ChimeraX(_)
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

impl AppModel {
    pub(super) fn start_remote_console(&mut self, request: cuemol_command::transport::ApiRequest) {
        let parsed = (|| {
            if !self.desktop.console_remote_enabled() {
                return Err("Command line access is disabled.".into());
            }
            if self.console_run.is_some() || self.remote_console.is_some() {
                return Err("The console is already running.".into());
            }
            let source = request.message["source"]
                .as_str()
                .filter(|source| !source.trim().is_empty())
                .ok_or("A console source is required.")?
                .to_owned();
            let dialect = cuemol_command::transport::console_dialect(&request.message)?;
            if !self.desktop.console_dialect_enabled(dialect.label()) {
                return Err("Console dialect is disabled.".into());
            }
            if let Some(directory) = request.message["cwd"].as_str() {
                self.console_session
                    .set_working_directory(Path::new(directory))?;
            }
            self.begin_remote_edit()?;
            Ok::<_, String>((source, dialect))
        })();
        let (source, dialect) = match parsed {
            Ok(value) => value,
            Err(error) => {
                cuemol_command::transport::reject_console(request, error);
                return;
            }
        };
        self.agent.cancel("Command line submission started.");
        let pending = cuemol_command::transport::ConsoleReply::new(request, &self.log_lines);
        match self.start_console(&source, dialect) {
            Ok(_) => {
                self.remote_console = Some(pending);
                let _ = self.poll_console();
            }
            Err(error) => pending.finish(&self.log_lines, false, &error),
        }
    }
    pub(super) fn poll_console(&mut self) -> Result<Update, String> {
        if self.remote_console.is_some() && !self.desktop.console_remote_enabled() {
            return Ok(self.stop_console());
        }
        let result = self.advance_console();
        if let Some(pending) = &mut self.remote_console {
            pending.collect(&self.log_lines);
        }
        if self.console_run.is_none() {
            if let Some(pending) = self.remote_console.take() {
                let status = result
                    .as_ref()
                    .err()
                    .map(String::as_str)
                    .unwrap_or(&self.status);
                pending.finish(
                    &self.log_lines,
                    result.is_ok() && self.status == "Console executed",
                    status,
                );
            }
        }
        result
    }
}

#[cfg(test)]
mod remote_tests {
    use super::*;
    fn request(
        source: &str,
    ) -> (
        cuemol_command::transport::ApiRequest,
        std::sync::mpsc::Receiver<Option<Value>>,
    ) {
        let (reply, receiver) = std::sync::mpsc::sync_channel(1);
        (
            cuemol_command::transport::ApiRequest {
                endpoint: cuemol_command::transport::ApiEndpoint::Console,
                message: serde_json::json!({"source":source,"dialect":"cuemol"}),
                reply,
            },
            receiver,
        )
    }
    #[test]
    fn remote_console_uses_gui_history_and_cancels_when_access_is_disabled() {
        let mut model = super::super::tests::model_without_gpu();
        model.desktop.open_bytes("probe.pdb",b"ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let uid = model.desktop.active_object_summaries()[0].uid;
        model
            .desktop
            .scene_manager_mut()
            .object_mut(uid)
            .unwrap()
            .set_name("probe");
        let (req, rx) = request("rename probe, changed");
        model.start_remote_console(req);
        assert_eq!(rx.recv().unwrap().unwrap()["ok"], false);
        model
            .desktop
            .put_config_value("console:remote-access", "true");
        let (req, rx) = request("hide probe; rename probe, changed");
        model.start_remote_console(req);
        assert_eq!(rx.recv().unwrap().unwrap()["ok"], true);
        assert_eq!(model.undo_stack.len(), 1);
        let (req, rx) = request("undo");
        model.start_remote_console(req);
        assert_eq!(rx.recv().unwrap().unwrap()["ok"], true);
        assert_eq!(
            model.desktop.scene_manager().object(uid).unwrap().name(),
            "probe"
        );
        let source = std::iter::repeat_n("pwd", 200)
            .chain(["rename probe, late"])
            .collect::<Vec<_>>()
            .join(";");
        let (req, rx) = request(&source);
        model.start_remote_console(req);
        assert!(rx.try_recv().is_err());
        assert!(model.console_run.is_some());
        model
            .desktop
            .put_config_value("console:remote-access", "false");
        model.poll_console().unwrap();
        assert_eq!(rx.recv().unwrap().unwrap()["ok"], false);
        assert!(model.console_run.is_none());
        assert_eq!(
            model.desktop.scene_manager().object(uid).unwrap().name(),
            "probe"
        );
    }
}
