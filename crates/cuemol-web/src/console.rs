use super::*;
use cuemol_command::console::Action;
use cuemol_ui::{
    console::ConsoleIntent,
    console_runtime::{ConsoleRunner, ConsoleStep},
};

pub(super) struct ConsoleExecution {
    runner: ConsoleRunner,
    undo: Option<HistoryEntry>,
    revision: u64,
    camera: Camera,
    waiting: bool,
    changed: bool,
}

impl App {
    pub(super) fn console_intent(&mut self, intent: Option<ConsoleIntent>) {
        match intent {
            Some(ConsoleIntent::Run) => self.execute_console_source(),
            Some(ConsoleIntent::Stop) => {
                self.cancel_console("Console stopped; completed edits are kept.")
            }
            Some(ConsoleIntent::Clear) => self.logs.clear(),
            Some(ConsoleIntent::Help) => {
                let text = self
                    .console_session
                    .help_dialect(self.console.dialect, "")
                    .unwrap_or_else(|error| error);
                self.logs.extend(text.lines().map(str::to_string));
            }
            _ => {}
        }
    }

    fn execute_console_source(&mut self) {
        self.cancel_console("Console replaced by another submission.");
        let source = std::mem::take(&mut self.console.source);
        if source.trim().is_empty() {
            return;
        }
        self.console.remember(&source);
        if let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        {
            let _ = storage.set_item(
                "cuemol.console.history",
                &serde_json::to_string(&self.console.history).unwrap(),
            );
        }
        let runner = match ConsoleRunner::new(&source, self.console.dialect) {
            Ok(runner) => runner,
            Err(error) => {
                self.message = error;
                return;
            }
        };
        self.console_session
            .begin_submission(&source, self.console.dialect);
        self.console_session.sync_revision(self.revision);
        self.console_generation = self.console_generation.wrapping_add(1);
        self.console_run = Some(ConsoleExecution {
            runner,
            undo: Some(self.capture_history()),
            revision: self.revision,
            camera: self.camera.clone(),
            waiting: false,
            changed: false,
        });
        let context = CONTEXT.with(|ctx| ctx.borrow().clone()).unwrap_or_default();
        self.poll_console(&context);
    }

    pub(super) fn cancel_console(&mut self, reason: &str) {
        if self.console_run.take().is_some() {
            if let Err(error) = self.console_session.finish_submission() {
                self.logs.push(error);
            }
            self.console_generation = self.console_generation.wrapping_add(1);
            self.logs.push(reason.into());
            self.message = reason.into();
        }
    }

    fn console_result(
        &mut self,
        run: &mut ConsoleExecution,
        name: &str,
        args: &Value,
        result: &Value,
    ) -> Result<(), String> {
        self.logs.extend(
            cuemol_command::console::format_output(result)
                .lines()
                .take(200)
                .map(str::to_string),
        );
        if name == "set_mol_selection" && !result.is_null() {
            self.selection = args["selection"].as_str().unwrap_or("").into();
            self.selected = args["objId"].as_u64().map(Uid::new);
        } else if name == "execute_script" {
            if let Some(selection) = result["selection"].as_str() {
                self.selection = selection.into();
                if let Some(id) = self.selected.or_else(|| {
                    self.state
                        .active_object_summaries()
                        .first()
                        .map(|object| object.uid)
                }) {
                    self.state
                        .set_molecule_selection_text(id, selection)
                        .map_err(|e| e.to_string())?;
                    self.selected = Some(id);
                }
            }
        }
        let changed = run.undo.as_ref().is_none_or(|before| {
            !before.state.same_undo_state(&self.state)
                || before.selection != self.selection
                || before.camera != self.camera
        });
        self.console_session.complete_record(changed);
        if changed && cuemol_command::tool_records_history(name) {
            if let Some(before) = run.undo.take() {
                self.history.push(before);
                if self.history.len() > 12 {
                    self.history.remove(0);
                }
                self.future.clear();
            }
        }
        if changed && cuemol_command::tool_changes_scene(name) {
            self.console_session.scene_changed();
            run.changed = true;
            self.revision = self.revision.wrapping_add(1);
            self.navigation.cancel();
            self.dirty = true;
        }
        run.revision = self.revision;
        run.camera = self.camera.clone();
        self.console_session.accept_revision(self.revision);
        Ok(())
    }

    pub(super) fn console_frame(
        &mut self,
        generation: u64,
        id: Uid,
        index: usize,
        result: Result<cuemol_mdtools::TrajectoryFrame, String>,
    ) {
        if generation != self.console_generation {
            return;
        }
        let Some(mut run) = self.console_run.take() else {
            return;
        };
        if run.revision != self.revision
            || !cuemol_command::console::same_camera_view(&run.camera, &self.camera)
        {
            self.logs.push(
                "Scene changed before the console frame was loaded; remaining commands cancelled."
                    .into(),
            );
            self.message = "Console cancelled".into();
            return;
        }
        let result = result.and_then(|frame| {
            self.state
                .apply_coordinate_frame(id, &frame)
                .map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => {
                self.set_trajectory_frame_index(id, index);
                let args = json!({"objId":id.get(),"frame":index});
                if let Err(error) = self.console_result(
                    &mut run,
                    "set_frame",
                    &args,
                    &json!({"objectId":id.get(),"frame":index}),
                ) {
                    self.logs.push(error);
                    self.message = "Console error".into();
                    return;
                }
                run.waiting = false;
                self.console_run = Some(run);
            }
            Err(error) => {
                self.logs.push(error);
                self.message = "Console error".into();
            }
        }
    }

    pub(super) fn poll_console(&mut self, ctx: &egui::Context) {
        let Some(mut run) = self.console_run.take() else {
            return;
        };
        if run.revision != self.revision
            || !cuemol_command::console::same_camera_view(&run.camera, &self.camera)
        {
            self.logs.push(
                "Scene or camera changed outside the console; remaining commands cancelled.".into(),
            );
            self.message = "Console cancelled".into();
            return;
        }
        if run.waiting {
            self.console_run = Some(run);
            return;
        }
        self.console_session.trajectory_frames = self
            .state
            .active_object_summaries()
            .into_iter()
            .filter_map(|object| {
                self.trajectory_state(object.uid)
                    .map(|(count, _)| (object.uid.get(), count))
            })
            .collect();
        let mut keep = true;
        let wake_context = ctx.clone();
        let console_wake: Arc<dyn Fn() + Send + Sync> =
            Arc::new(move || wake_context.request_repaint());
        for _ in 0..64 {
            let step = match run.runner.next(
                &mut self.console_session,
                &mut self.state,
                &mut self.camera,
                console_wake.clone(),
            ) {
                Ok(step) => step,
                Err(error) => {
                    self.logs.push(error);
                    self.message = "Console error".into();
                    keep = false;
                    break;
                }
            };
            match step {
                ConsoleStep::Pending => {
                    self.message = "Console running".into();
                    break;
                }
                ConsoleStep::Done => {
                    self.message = "Console executed".into();
                    keep = false;
                    break;
                }
                ConsoleStep::Completed { name, args, result } => {
                    if let Err(error) = self.console_result(&mut run, &name, &args, &result) {
                        self.logs.push(error);
                        self.message = "Console error".into();
                        keep = false;
                        break;
                    }
                }
                ConsoleStep::Action(action) => match action {
                    Action::Tool { name, args } => {
                        if matches!(name.as_str(), "undo" | "redo") {
                            if run.changed || !run.runner.allows_undo() {
                                self.logs
                                    .push("Undo and redo must be submitted alone".into());
                                self.message = "Console error".into();
                                keep = false;
                                break;
                            }
                            self.call(&name, args);
                            continue;
                        }
                        if name == "clear_undo" {
                            self.call(&name, args);
                            continue;
                        }
                        if name == "set_frame" {
                            if let (Some(id), Some(index)) = (
                                args["objId"].as_u64().map(Uid::new),
                                args["frame"]
                                    .as_u64()
                                    .and_then(|value| usize::try_from(value).ok()),
                            ) {
                                let native = self.state.trajectory_len(id);
                                if let Some((source, local)) =
                                    index.checked_sub(native).and_then(|offset| {
                                        self.trajectories
                                            .get(&id)
                                            .and_then(|group| group.source(offset))
                                    })
                                {
                                    let generation = self.console_generation;
                                    spawn_local(async move {
                                        enqueue(Event::ConsoleFrame(
                                            generation,
                                            id,
                                            index,
                                            source.frame(local).await.map_err(js_error),
                                        ));
                                    });
                                    run.waiting = true;
                                    self.message = "Console loading frame".into();
                                    break;
                                }
                            }
                        }
                        let result = if name == "list_trajectories" {
                            Ok(self.trajectory_list())
                        } else {
                            cuemol_command::call_tool(
                                &mut self.state,
                                &mut self.camera,
                                &name,
                                &args,
                            )
                            .map_err(|e| e.to_string())
                        };
                        match result {
                            Ok(result) => {
                                if name == "set_frame" {
                                    if let (Some(id), Some(index)) =
                                        (args["objId"].as_u64(), args["frame"].as_u64())
                                    {
                                        self.set_trajectory_frame_index(
                                            Uid::new(id),
                                            index as usize,
                                        );
                                    }
                                }
                                if let Err(error) =
                                    self.console_result(&mut run, &name, &args, &result)
                                {
                                    self.logs.push(error);
                                    self.message = "Console error".into();
                                    keep = false;
                                    break;
                                }
                            }
                            Err(error) => {
                                let _ = self.console_result(&mut run, &name, &args, &Value::Null);
                                self.logs.push(error);
                                self.message = "Console error".into();
                                keep = false;
                                break;
                            }
                        }
                    }
                    Action::Transaction(label) => {
                        if let Err(error) = self.console_session.begin_transaction(&label) {
                            self.logs.push(error);
                        }
                        run.undo = Some(self.capture_history());
                        run.changed = false;
                    }
                    Action::WriteFile { path, bytes, mime } => {
                        let filename = std::path::Path::new(&path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or(&path);
                        if let Err(error) = download(filename, bytes.as_ref(), &mime) {
                            self.logs.push(js_error(error));
                            self.message = "Console error".into();
                            keep = false;
                            break;
                        }
                        self.logs.push(format!("Saved {filename}"));
                    }
                    Action::Output(text) | Action::Echo(text) => {
                        self.logs.extend(text.lines().take(200).map(str::to_string))
                    }
                    Action::Clear => self.logs.clear(),
                    Action::Quit => {
                        self.logs.push("Close this browser tab to exit.".into());
                        keep = false;
                        break;
                    }
                    Action::Source(_)
                    | Action::Native(_)
                    | Action::Qax(_)
                    | Action::QaxOp(_)
                    | Action::RememberSetting { .. }
                    | Action::KeepChain { .. }
                    | Action::Render { .. } => unreachable!("console runner expands this action"),
                },
            }
        }
        if self.logs.len() > 1000 {
            self.logs.drain(..self.logs.len() - 1000);
        }
        if keep {
            run.revision = self.revision;
            run.camera = self.camera.clone();
            self.console_run = Some(run);
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        } else if let Err(error) = self.console_session.finish_submission() {
            self.logs.push(error);
        }
    }
}
