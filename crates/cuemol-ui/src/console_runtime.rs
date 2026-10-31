use cuemol_command::console::{Action, Dialect, Session, Submission};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};

#[derive(Debug)]
enum Pending {
    Download {
        job: cuemol_agent::DownloadJob,
        name: String,
        args: Value,
    },
    Render {
        receiver: mpsc::Receiver<Result<Value, String>>,
        cancel: Arc<AtomicBool>,
        name: String,
        args: Value,
        path: Option<String>,
        remember: bool,
    },
}

#[derive(Debug)]
pub struct ConsoleRunner {
    submission: Submission,
    pending: Option<Pending>,
    ready: VecDeque<Action>,
    single_command: bool,
}

pub enum ConsoleStep {
    Action(Action),
    Completed {
        name: String,
        args: Value,
        result: Value,
    },
    Pending,
    Done,
}

#[derive(Debug)]
pub struct ConsoleCheckpoint {
    state: DesktopAppState,
    camera: Camera,
    selection: String,
    scene_changed: bool,
}

impl ConsoleCheckpoint {
    pub fn new(state: &DesktopAppState, camera: &Camera, selection: &str) -> Self {
        Self {
            state: state.clone(),
            camera: camera.clone(),
            selection: selection.into(),
            scene_changed: false,
        }
    }

    pub fn update(&mut self, state: &DesktopAppState, camera: &Camera, selection: &str) -> bool {
        let scene_changed = !cuemol_command::console::same_camera_view(&self.camera, camera)
            || self.selection != selection
            || !self.state.same_scene_undo_state(state);
        let changed = scene_changed || !self.state.same_undo_state(state);
        if changed {
            *self = Self::new(state, camera, selection);
        }
        self.scene_changed = scene_changed;
        changed
    }

    pub fn scene_changed(&self) -> bool {
        self.scene_changed
    }
}

impl ConsoleRunner {
    pub fn library(
        source: &str,
        macro_name: Option<&str>,
        targets: &[u64],
    ) -> Result<Self, String> {
        Ok(Self {
            submission: Submission::library(source, macro_name, targets)?,
            pending: None,
            ready: VecDeque::new(),
            single_command: false,
        })
    }

    pub fn skip_failed_target(&mut self) -> bool {
        self.pending = None;
        self.ready.clear();
        self.submission.skip_failed_target()
    }

    pub fn new(source: &str, dialect: Dialect) -> Result<Self, String> {
        Ok(Self {
            submission: Submission::new(source, dialect)?,
            pending: None,
            ready: VecDeque::new(),
            single_command: cuemol_command::console::split_commands(source)?.len() == 1,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.submission.is_empty() && self.pending.is_none() && self.ready.is_empty()
    }

    pub fn allows_undo(&self) -> bool {
        self.single_command && self.is_empty()
    }

    pub fn next(
        &mut self,
        session: &mut Session,
        state: &mut DesktopAppState,
        camera: &mut Camera,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<ConsoleStep, String> {
        if let Some(pending) = &self.pending {
            match pending {
                Pending::Download { job, .. } => {
                    let Some(result) = job.poll() else {
                        return Ok(ConsoleStep::Pending);
                    };
                    let Some(Pending::Download { name, args, .. }) = self.pending.take() else {
                        unreachable!()
                    };
                    let downloaded = result?;
                    let result = cuemol_command::load_fetched(
                        state,
                        camera,
                        &downloaded.plan,
                        &downloaded.bytes,
                    )
                    .map_err(|e| e.to_string())?;
                    return Ok(ConsoleStep::Completed { name, args, result });
                }
                Pending::Render { receiver, .. } => {
                    let result = match receiver.try_recv() {
                        Ok(result) => result,
                        Err(mpsc::TryRecvError::Empty) => return Ok(ConsoleStep::Pending),
                        Err(_) => return Err("Render worker stopped without a result".into()),
                    };
                    let Some(Pending::Render {
                        name,
                        args,
                        path,
                        remember,
                        ..
                    }) = self.pending.take()
                    else {
                        unreachable!()
                    };
                    let result = result?;
                    let bytes = if remember {
                        session.remember_render(state, camera, &result)?
                    } else {
                        use base64::Engine;
                        Arc::new(
                            base64::engine::general_purpose::STANDARD
                                .decode(
                                    result["_image"]
                                        .as_str()
                                        .ok_or("Rendered image is missing")?,
                                )
                                .map_err(|e| e.to_string())?,
                        )
                    };
                    if let Some(path) = path {
                        self.ready.push_back(Action::WriteFile {
                            path,
                            bytes,
                            mime: "image/png".into(),
                        });
                    }
                    return Ok(ConsoleStep::Completed {
                        name,
                        args,
                        result: json!({"width":result["width"],"height":result["height"],"rendered":true}),
                    });
                }
            }
        }
        let action = if let Some(action) = self.ready.pop_front() {
            action
        } else {
            let Some(action) = self.submission.next(session, state, camera)? else {
                return Ok(ConsoleStep::Done);
            };
            action
        };
        if let Action::Tool { name, args } = &action {
            session.prepare_record(state, name, args);
            if name == "save_scene" {
                self.ready.push_back(Action::Tool {
                    name: "clear_undo".into(),
                    args: json!({}),
                });
                #[cfg(target_arch = "wasm32")]
                {
                    let mut saved = state.clone();
                    saved
                        .set_active_view_camera(camera)
                        .map_err(|e| e.to_string())?;
                    let bytes = saved.active_scene_bytes().map_err(|e| e.to_string())?;
                    return Ok(ConsoleStep::Action(Action::WriteFile {
                        path: args["path"]
                            .as_str()
                            .ok_or("A scene path is required")?
                            .into(),
                        bytes: Arc::new(bytes),
                        mime: "application/octet-stream".into(),
                    }));
                }
            }
            if ["fetch_pdb", "fetch_url", "fetch_map"].contains(&name.as_str()) {
                let wake = wake.clone();
                self.pending = Some(Pending::Download {
                    job: cuemol_agent::DownloadJob::start(
                        name.clone(),
                        args.clone(),
                        Box::new(move || wake()),
                    ),
                    name: name.clone(),
                    args: args.clone(),
                });
                return Ok(ConsoleStep::Pending);
            }
        }
        let render = match &action {
            Action::Render {
                width,
                height,
                ray,
                dpi,
                path,
            } => {
                let args = json!({"width":width,"height":height,"fileName":"","style":if *ray {Some("gi")} else {None}});
                Some((
                    if *ray {
                        "render_image"
                    } else {
                        "get_screenshot"
                    },
                    args,
                    path.clone(),
                    *ray,
                    *dpi,
                ))
            }
            Action::Tool { name, args }
                if [
                    "render_image",
                    "get_screenshot",
                    "capture_view",
                    "export_image",
                ]
                .contains(&name.as_str())
                    || name == "export_scene"
                        && args["format"].as_str().unwrap_or("png") == "png" =>
            {
                let path = args["fileName"]
                    .as_str()
                    .or(args["path"].as_str())
                    .map(str::to_string);
                Some((
                    if name.starts_with("export_") {
                        "get_screenshot"
                    } else {
                        name
                    },
                    args.clone(),
                    path,
                    false,
                    args["dpi"].as_f64(),
                ))
            }
            _ => None,
        };
        if let Some((name, args, path, remember, dpi)) = render {
            let mut prepared = cuemol_command::prepare_render(state, camera, name, &args)
                .map_err(|e| e.to_string())?
                .in_memory();
            if let Some(dpi) = dpi {
                prepared = prepared.with_dpi(dpi);
            }
            let (sender, receiver) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            let wake = wake.clone();
            #[cfg(not(target_arch = "wasm32"))]
            std::thread::spawn(move || {
                let result = prepared
                    .run_with_cancel(&worker_cancel)
                    .map_err(|e| e.to_string());
                if !worker_cancel.load(Ordering::Relaxed) {
                    let _ = sender.send(result);
                    wake();
                }
            });
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let result = prepared
                    .run_async_with_cancel(Some(&worker_cancel))
                    .await
                    .map_err(|e| e.to_string());
                if !worker_cancel.load(Ordering::Relaxed) {
                    let _ = sender.send(result);
                    wake();
                }
            });
            self.pending = Some(Pending::Render {
                receiver,
                cancel,
                name: name.into(),
                args,
                path,
                remember,
            });
            return Ok(ConsoleStep::Pending);
        }
        Ok(ConsoleStep::Action(action))
    }
}

impl Drop for ConsoleRunner {
    fn drop(&mut self) {
        if let Some(Pending::Render { cancel, .. }) = &self.pending {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
