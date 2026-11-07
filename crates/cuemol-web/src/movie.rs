use super::*;
use cuemol_command::movie::{MovieOptions, MovieResult, MovieScene};
use cuemol_command::render_history::RenderSource;
use cuemol_ui::RenderAction;

#[derive(Default)]
pub(super) struct Movie {
    initialized: bool,
    initializing: bool,
    error: Option<String>,
    generation: u64,
    pub running: bool,
    cancel: Option<Arc<AtomicBool>>,
    abort: Option<web_sys::AbortController>,
    job: Option<String>,
    stats_key: String,
    stats_generation: u64,
    frame_request: u64,
    frame_loading: bool,
    pending_frame: Option<FrameRequest>,
    preferences: String,
    last_progress: f64,
    pub progress: Value,
}

struct FrameRequest {
    request: u64,
    record: u64,
    frame: u32,
    directory: String,
    base: String,
}

pub(super) enum Completion {
    Initialized(Result<Value, String>),
    Stats(u64, Result<Value, String>),
    Picked(u64, MovieOptions, Result<Value, String>),
    Cleaned(Result<Value, String>),
    Begun(u64, String, String, String, bool),
    Preview(u64, u32, u32, Arc<[u8]>),
    Finished(u64, Result<Outcome, String>),
    Frame(u64, u64, u32, Result<Arc<[u8]>, String>),
    File(Result<(), String>),
}

pub(super) struct Outcome {
    movie: MovieResult,
    source: RenderSource,
    size: [u32; 2],
    elapsed: f64,
    preview: Option<Arc<[u8]>>,
    error: Option<String>,
}

fn error(value: JsValue) -> String {
    js_sys::Reflect::get(&value, &"message".into())
        .ok()
        .and_then(|value| value.as_string())
        .unwrap_or_else(|| js_error(value))
}

fn value(value: &Value) -> JsValue {
    js_sys::JSON::parse(&value.to_string()).unwrap()
}

fn json_value(value: JsValue) -> Result<Value, String> {
    let text = js_sys::JSON::stringify(&value).map_err(error)?;
    serde_json::from_str(&String::from(text)).map_err(|e| e.to_string())
}

fn invoke(name: &str, arguments: &[JsValue]) -> Result<JsValue, String> {
    let api = js_sys::Reflect::get(&js_sys::global(), &"cuemolMovies".into()).map_err(error)?;
    let method = js_sys::Reflect::get(&api, &name.into())
        .map_err(error)?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "Browser movie support is unavailable".to_string())?;
    let args = js_sys::Array::new();
    for argument in arguments {
        args.push(argument);
    }
    method.apply(&api, &args).map_err(error)
}

async fn resolve(promise: Result<JsValue, String>) -> Result<JsValue, String> {
    JsFuture::from(js_sys::Promise::resolve(&promise?))
        .await
        .map_err(error)
}

async fn call(name: &str, arguments: &[JsValue]) -> Result<JsValue, String> {
    resolve(invoke(name, arguments)).await
}

fn notify(completion: Completion) {
    enqueue(Event::Movie(completion));
}

fn now() -> f64 {
    js_sys::Date::now() * 0.001
}

async fn read_frame(directory: &str, base: &str, index: u32) -> Result<Arc<[u8]>, String> {
    let bytes = call("readFrame", &[directory.into(), base.into(), index.into()]).await?;
    Ok(js_sys::Uint8Array::new(&bytes).to_vec().into())
}

async fn run(
    generation: u64,
    mut scene: Option<MovieScene>,
    mut source: RenderSource,
    mut size: [u32; 2],
    options: MovieOptions,
    overwrite: bool,
    cancel: Arc<AtomicBool>,
    signal: web_sys::AbortSignal,
) -> Result<Outcome, String> {
    let started = now();
    let reencode = scene.is_none();
    let total = scene
        .as_ref()
        .map(|scene| {
            cuemol_command::movie::frame_count(
                scene.duration(),
                options.fps,
                options.dup_last_frame,
            )
        })
        .transpose()?;
    let first = scene
        .as_mut()
        .map(|scene| scene.prepare_frame(0, options.fps, size))
        .transpose()
        .map_err(|e| e.to_string())?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Movie render cancelled".into());
    }
    let info = json_value(
        call(
            "begin",
            &[
                value(&json!(options)),
                value(&json!(size)),
                overwrite.into(),
                reencode.into(),
                signal.into(),
            ],
        )
        .await?,
    )?;
    let job = info["job"]
        .as_str()
        .ok_or("Movie output returned no job")?
        .to_string();
    let directory = info["directory"]
        .as_str()
        .ok_or("Movie output returned no folder")?
        .to_string();
    let base = info["baseName"]
        .as_str()
        .ok_or("Movie output returned no base name")?
        .to_string();
    if let Ok(actual) = serde_json::from_value::<[u32; 2]>(info["size"].clone()) {
        size = actual;
    }
    for (key, value) in [
        ("width", json!(size[0])),
        ("height", json!(size[1])),
        ("unit", json!("px")),
    ] {
        source.settings.insert(key.into(), value);
    }
    notify(Completion::Begun(
        generation,
        job.clone(),
        directory.clone(),
        base.clone(),
        reencode,
    ));
    let mut preview = None;
    let result: Result<(), String> = async {
        let mut first = first;
        let mut preview_at = 0.;
        if let Some(scene) = scene.as_mut() {
            let total = total.unwrap();
            for index in 0..total {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Movie render cancelled".into());
                }
                let render = match first.take() {
                    Some(render) => render,
                    None => scene
                        .prepare_frame(index, options.fps, size)
                        .map_err(|e| e.to_string())?,
                };
                let output = render
                    .run_async_with_cancel(Some(&cancel))
                    .await
                    .map_err(|e| e.to_string())?;
                if cancel.load(Ordering::Relaxed) {
                    return Err("Movie render cancelled".into());
                }
                let png: Arc<[u8]> = base64::engine::general_purpose::STANDARD
                    .decode(
                        output["_image"]
                            .as_str()
                            .ok_or("Render returned no image")?,
                    )
                    .map_err(|e| e.to_string())?
                    .into();
                call(
                    "writeFrame",
                    &[
                        job.as_str().into(),
                        index.into(),
                        js_sys::Uint8Array::from(png.as_ref()).into(),
                    ],
                )
                .await?;
                if now() - preview_at >= 0.25 || index + 1 == total {
                    EVENTS.with(|events| {
                        events.borrow_mut().retain(|event| {
                            !matches!(event,
                        Event::Movie(Completion::Preview(id, ..)) if *id == generation)
                        })
                    });
                    notify(Completion::Preview(
                        generation,
                        index + 1,
                        total,
                        png.clone(),
                    ));
                    preview_at = now();
                }
                preview = Some(png);
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Movie render cancelled".into());
        }
        if options.make_movie || reencode {
            call("encode", &[job.as_str().into()]).await?;
        }
        Ok(())
    }
    .await;
    if preview.is_none() {
        if let Ok(status) = invoke("status", &[job.as_str().into()]).and_then(json_value) {
            if let Some(count) = status["frameCount"].as_u64().filter(|count| *count > 0) {
                preview = read_frame(&directory, &base, count as u32 - 1).await.ok();
            }
        }
    }
    let movie: MovieResult =
        serde_json::from_value(json_value(call("finish", &[job.as_str().into()]).await?)?)
            .map_err(|e| e.to_string())?;
    let error = result.err().map(|message| {
        if cancel.load(Ordering::Relaxed) && movie.movie_path.is_none() {
            "Movie render cancelled; completed frames were kept".into()
        } else {
            message
        }
    });
    Ok(Outcome {
        movie,
        source,
        size,
        elapsed: (now() - started).max(0.),
        preview,
        error,
    })
}

impl App {
    fn load_movie_frame(&mut self, frame: FrameRequest) {
        self.movie.frame_loading = true;
        spawn_local(async move {
            notify(Completion::Frame(
                frame.request,
                frame.record,
                frame.frame,
                read_frame(&frame.directory, &frame.base, frame.frame).await,
            ));
        });
    }
    pub(super) fn movie_tick(&mut self, ctx: &egui::Context) {
        if self.show_render
            && !self.movie.initialized
            && !self.movie.initializing
            && self.movie.error.is_none()
        {
            self.movie.initializing = true;
            spawn_local(async {
                notify(Completion::Initialized(
                    call("initialize", &[]).await.and_then(json_value),
                ));
            });
        }
        if self.movie.initialized {
            let preferences = self.render_panel.movie.preferences().to_string();
            if preferences != self.movie.preferences {
                self.render_panel.movie.persist(&mut self.state);
                if let Some(storage) =
                    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
                {
                    let _ = storage.set_item("cuemol.movie", &preferences);
                }
                self.movie.preferences = preferences;
            }
            if self.show_render && !self.movie.running {
                let options = &self.render_panel.movie;
                let key = json!([options.use_temp_dir, options.output_dir, options.base_name])
                    .to_string();
                if key != self.movie.stats_key {
                    self.movie.stats_key = key;
                    self.movie.stats_generation = self.movie.stats_generation.wrapping_add(1);
                    let generation = self.movie.stats_generation;
                    let options = value(&json!(options));
                    self.render_panel.available_frames = 0;
                    self.render_panel.has_movie_outputs = false;
                    spawn_local(async move {
                        notify(Completion::Stats(
                            generation,
                            call("inspect", &[options]).await.and_then(json_value),
                        ));
                    });
                }
            }
        }
        if self.movie.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            if now() - self.movie.last_progress >= 0.1 {
                self.movie.last_progress = now();
                if let Some(job) = &self.movie.job {
                    if let Ok(progress) =
                        invoke("status", &[job.as_str().into()]).and_then(json_value)
                    {
                        if progress["phase"] == "encoding" {
                            self.render_panel.message = format!(
                                "Encoding movie: {} / {} frames",
                                progress["encoded"], progress["frameCount"]
                            );
                        }
                        self.movie.progress = progress;
                    }
                }
            }
        }
    }

    pub(super) fn movie_error(&self) -> Option<String> {
        self.movie.error.clone().or_else(|| {
            (!self.movie.initialized).then(|| "Preparing browser movie storage…".into())
        })
    }

    pub(super) fn cancel_movie(&mut self) {
        if let Some(abort) = &self.movie.abort {
            abort.abort();
        }
        if let Some(cancel) = &self.movie.cancel {
            cancel.store(true, Ordering::Relaxed);
            cancel_oidn();
        }
        if let Some(job) = &self.movie.job {
            let _ = invoke("cancel", &[job.as_str().into()]);
        }
    }

    pub(super) fn movie_action(&mut self, ctx: &egui::Context, action: &RenderAction) {
        match action {
            RenderAction::StartMovie(overwrite) => self.start_movie(ctx, *overwrite, false),
            RenderAction::ReencodeMovie => self.start_movie(ctx, false, true),
            RenderAction::PickMovieFolder if !self.movie.running => {
                let promise = invoke("pickDirectory", &[]);
                let generation = self.movie.generation;
                let options = self.render_panel.movie.clone();
                spawn_local(async move {
                    notify(Completion::Picked(
                        generation,
                        options,
                        resolve(promise).await.and_then(json_value),
                    ));
                });
            }
            RenderAction::CleanupMovie if !self.movie.running => {
                let options = value(&json!(self.render_panel.movie));
                let promise = invoke("cleanup", &[options]);
                spawn_local(async move {
                    notify(Completion::Cleaned(
                        resolve(promise).await.and_then(json_value),
                    ));
                });
            }
            RenderAction::MovieFrame(frame) if !self.movie.running => {
                if let Some(record) = self.render_panel.history.current() {
                    if let Some(movie) =
                        record.movie.as_ref().filter(|movie| movie.frames_available)
                    {
                        self.movie.frame_request = self.movie.frame_request.wrapping_add(1);
                        let request = FrameRequest {
                            request: self.movie.frame_request,
                            record: record.id,
                            directory: movie.output_dir.clone(),
                            base: movie.base_name.clone(),
                            frame: *frame,
                        };
                        if self.movie.frame_loading {
                            self.movie.pending_frame = Some(request);
                        } else {
                            self.load_movie_frame(request);
                        }
                    }
                }
            }
            RenderAction::SaveMovie | RenderAction::OpenMovie => {
                if let Some(path) = self
                    .render_panel
                    .history
                    .current()
                    .and_then(|record| record.movie.as_ref())
                    .and_then(|movie| movie.movie_path.as_ref())
                {
                    let method = if *action == RenderAction::SaveMovie {
                        "saveMovie"
                    } else {
                        "openMovie"
                    };
                    let promise = invoke(method, &[path.as_str().into()]);
                    spawn_local(async move {
                        notify(Completion::File(resolve(promise).await.map(|_| ())));
                    });
                }
            }
            _ => {}
        }
    }

    fn start_movie(&mut self, ctx: &egui::Context, overwrite: bool, reencode: bool) {
        if self.movie.running || self.render_cancel.is_some() || !self.movie.initialized {
            return;
        }
        if !self.flush_render_settings(ctx.input(|i| i.time), true) {
            return;
        }
        let options = self.render_panel.movie.clone();
        let size = self.render_panel.editor.pixels();
        let prepared = options.validate().and_then(|()| {
            if reencode {
                let source = self.render_panel.history.current().filter(|record| record.movie.as_ref().is_some_and(|movie| {
                    movie.frames_available && movie.options.output_dir == options.output_dir && movie.base_name == options.base_name
                })).map(|record| record.source.clone()).unwrap_or_default();
                Ok((None, source))
            } else {
                let scene = MovieScene::capture(&self.state, &self.camera, &json!({"sceneId":self.render_panel.targets.scene(),"viewId":self.render_panel.targets.view()}))
                    .map_err(|e| e.to_string())?;
                let source = scene.source().map_err(|e| e.to_string())?;
                Ok((Some(scene), source))
            }
        });
        let (scene, source) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.render_panel.failed(error);
                return;
            }
        };
        let abort = match web_sys::AbortController::new() {
            Ok(abort) => abort,
            Err(error) => {
                self.render_panel.failed(js_error(error));
                return;
            }
        };
        let signal = abort.signal();
        self.movie.abort = Some(abort);
        self.movie.running = true;
        self.movie.generation = self.movie.generation.wrapping_add(1);
        self.movie.frame_request = self.movie.frame_request.wrapping_add(1);
        self.movie.pending_frame = None;
        self.movie.stats_generation = self.movie.stats_generation.wrapping_add(1);
        let generation = self.movie.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.movie.cancel = Some(cancel.clone());
        self.movie.progress = json!({"phase":"preparing","frameCount":0});
        self.render_panel.message = "Preparing movie output…".into();
        spawn_local(async move {
            notify(Completion::Finished(
                generation,
                run(
                    generation, scene, source, size, options, overwrite, cancel, signal,
                )
                .await,
            ));
        });
    }

    pub(super) fn movie_completion(&mut self, ctx: &egui::Context, completion: Completion) {
        match completion {
            Completion::Initialized(result) => {
                self.movie.initializing = false;
                match result {
                    Ok(info) => {
                        self.movie.initialized = true;
                        let temporary = info["temporary"]
                            .as_str()
                            .unwrap_or("Browser temporary folder");
                        self.render_panel.load_movie_options(&self.state, temporary);
                        if let Some(saved) = web_sys::window()
                            .and_then(|w| w.local_storage().ok().flatten())
                            .and_then(|storage| storage.get_item("cuemol.movie").ok().flatten())
                            .and_then(|value| serde_json::from_str::<Value>(&value).ok())
                        {
                            self.render_panel.movie =
                                MovieOptions::from_preferences(&saved, temporary);
                        }
                    }
                    Err(error) => self.movie.error = Some(error),
                }
            }
            Completion::Stats(generation, result)
                if generation == self.movie.stats_generation && !self.movie.running =>
            {
                if let Ok(info) = result {
                    self.render_panel.available_frames = info["frameCount"]
                        .as_u64()
                        .unwrap_or(0)
                        .min(u32::MAX as u64)
                        as u32;
                    self.render_panel.has_movie_outputs =
                        info["hasOutputs"].as_bool().unwrap_or(false);
                }
            }
            Completion::Picked(generation, options, result)
                if generation == self.movie.generation
                    && !self.movie.running
                    && options == self.render_panel.movie =>
            {
                match result {
                    Ok(info) => self
                        .render_panel
                        .movie
                        .set_custom(info["label"].as_str().unwrap_or_default().into()),
                    Err(error) => self.render_panel.message = error,
                }
            }
            Completion::Cleaned(result) => match result {
                Ok(info) => {
                    self.render_panel.history.invalidate_movie_identity(
                        info["directory"].as_str().unwrap_or_default(),
                        info["baseName"].as_str().unwrap_or_default(),
                    );
                    self.movie.stats_key.clear();
                    self.render_panel.message =
                        "Movie outputs removed; cached previews were kept".into();
                }
                Err(error) => self.render_panel.message = error,
            },
            Completion::Begun(generation, job, directory, base, reencode)
                if generation == self.movie.generation =>
            {
                self.movie.job = Some(job);
                if !reencode {
                    self.render_panel
                        .history
                        .invalidate_movie_identity(&directory, &base);
                }
                if self
                    .movie
                    .cancel
                    .as_ref()
                    .is_some_and(|cancel| cancel.load(Ordering::Relaxed))
                {
                    self.cancel_movie();
                }
            }
            Completion::Preview(generation, completed, total, png)
                if generation == self.movie.generation =>
            {
                if let Err(error) = self.render_panel.movie_preview_bytes(ctx, png) {
                    self.render_panel.message = error;
                } else {
                    self.render_panel.message =
                        format!("Rendering movie: {completed} / {total} frames");
                }
            }
            Completion::Finished(generation, result) if generation == self.movie.generation => {
                self.movie.running = false;
                self.movie.job = None;
                self.movie.cancel = None;
                self.movie.abort = None;
                self.movie.stats_key.clear();
                match result {
                    Ok(outcome) => {
                        let mut message = outcome.error.unwrap_or_else(|| {
                            format!(
                                "Rendered {} frames in {:.1} s{}",
                                outcome.movie.frame_count,
                                outcome.elapsed,
                                if outcome.movie.movie_path.is_some() {
                                    " and encoded the movie"
                                } else {
                                    ""
                                }
                            )
                        });
                        if let Some(png) = outcome.preview {
                            if let Err(error) = self.render_panel.store_movie_result(
                                ctx,
                                png,
                                outcome.source,
                                outcome.size,
                                outcome.elapsed,
                                outcome.movie,
                            ) {
                                message = error;
                            }
                        }
                        self.render_panel.failed(message);
                    }
                    Err(error) => self.render_panel.failed(error),
                }
                self.message = self.render_panel.message.clone();
            }
            Completion::Frame(request, record, frame, result) => {
                self.movie.frame_loading = false;
                if request == self.movie.frame_request
                    && !self.movie.running
                    && self.render_panel.history.current().is_some_and(|current| {
                        current.id == record
                            && current
                                .movie
                                .as_ref()
                                .is_some_and(|movie| movie.frames_available)
                    })
                {
                    self.render_panel.requested_movie_frame = None;
                    if let Err(error) = result.and_then(|png| {
                        self.render_panel
                            .select_movie_frame_bytes(ctx, record, frame, png)
                    }) {
                        self.render_panel.message = error;
                    }
                }
                if let Some(next) = self.movie.pending_frame.take().filter(|next| {
                    !self.movie.running
                        && self.render_panel.history.current().is_some_and(|current| {
                            current.id == next.record
                                && current
                                    .movie
                                    .as_ref()
                                    .is_some_and(|movie| movie.frames_available)
                        })
                }) {
                    self.load_movie_frame(next);
                }
            }
            Completion::File(Err(error)) => self.render_panel.message = error,
            _ => {}
        }
    }
}
