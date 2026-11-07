use super::*;
use base64::Engine;
use cuemol_command::render_editor::{catalog, quality_steps, RenderEditor, RenderTargets};
use cuemol_command::render_history::{RenderHistory, RenderSource};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) struct Rendering {
    editor: RenderEditor,
    targets: RenderTargets,
    history: RenderHistory,
    job: Option<RenderJob>,
    message: String,
    clock: Instant,
    scene_changed: bool,
}

impl Default for Rendering {
    fn default() -> Self {
        Self {
            editor: Default::default(),
            targets: Default::default(),
            history: Default::default(),
            job: None,
            message: "No render result yet. Press Start Render to render the current view.".into(),
            clock: Instant::now(),
            scene_changed: false,
        }
    }
}

struct RenderJob {
    receiver: mpsc::Receiver<Result<Value, String>>,
    cancel: Arc<AtomicBool>,
    progress: Option<cuemol_command::pov::PovProgress>,
    thread: Option<std::thread::JoinHandle<()>>,
    source: RenderSource,
    started: Instant,
}

impl Drop for RenderJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !thread.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }
}

impl AppModel {
    pub(super) fn take_rendering_change(&mut self) -> bool {
        std::mem::take(&mut self.rendering.scene_changed)
    }

    pub(super) fn rendering_pending(&self) -> bool {
        self.rendering.job.is_some() || self.rendering.editor.pending()
    }

    pub(super) fn flush_rendering(&mut self) -> Result<(), String> {
        let mut panel = std::mem::take(&mut self.rendering);
        let result = panel.sync(self, true);
        self.rendering = panel;
        result
    }

    pub(super) fn poll_rendering(&mut self) {
        let mut panel = std::mem::take(&mut self.rendering);
        panel.poll();
        if let Err(error) = panel.sync(self, false) {
            panel.message = error;
        }
        self.rendering = panel;
    }

    pub(super) fn rendering_request(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let mut panel = std::mem::take(&mut self.rendering);
        let result = panel.request(self, operation, args);
        if let Err(error) = &result {
            panel.message = error.clone();
        }
        self.rendering = panel;
        result
    }
}

impl Rendering {
    fn apply(&mut self, model: &mut AppModel, scene: u64, settings: &str) -> Result<(), String> {
        model.begin_remote_edit()?;
        model.stop_console();
        model
            .agent
            .cancel("Render settings changed outside the agent.");
        let before = model.capture_script_undo();
        cuemol_command::call_tool(
            &mut model.desktop,
            &mut model.camera,
            "set_render_settings",
            &json!({"sceneId":scene,"settings":settings}),
        )
        .map_err(|error| error.to_string())?;
        if !before.desktop.same_scene_undo_state(&model.desktop) {
            model.status = "Change render settings".into();
            model.push_undo_for_scene(before, Some(Uid::new(scene)));
            self.scene_changed = true;
        }
        Ok(())
    }

    fn sync(&mut self, model: &mut AppModel, force: bool) -> Result<(), String> {
        self.targets.refresh(&model.desktop);
        let scene = self.targets.scene();
        if self
            .editor
            .due(scene, self.clock.elapsed().as_secs_f64(), force)
        {
            let original = self
                .editor
                .scene()
                .ok_or("Render scene is no longer available")?;
            let settings = self.editor.settings_argument();
            if let Err(error) = self.apply(model, original, &settings) {
                self.editor.discard_pending();
                return Err(error);
            }
            let stored = cuemol_command::scene_render_settings(&model.desktop, Uid::new(original))
                .map_err(|error| error.to_string())?;
            self.editor.saved(&stored);
        }
        let stored = scene
            .and_then(|scene| {
                cuemol_command::scene_render_settings(&model.desktop, Uid::new(scene)).ok()
            })
            .unwrap_or_default();
        self.editor.sync(scene, &stored);
        Ok(())
    }

    fn poll(&mut self) {
        let Some(job) = &self.job else { return };
        let result = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("Render worker stopped".into()),
        };
        let job = self.job.take().unwrap();
        if job.cancel.load(Ordering::Relaxed) {
            self.message = "Render cancelled".into();
            return;
        }
        let outcome = result.and_then(|value| {
            let png = base64::engine::general_purpose::STANDARD
                .decode(value["_image"].as_str().ok_or("Render returned no image")?)
                .map_err(|error| error.to_string())?;
            let (width, height) = image::ImageReader::with_format(
                std::io::Cursor::new(&png),
                image::ImageFormat::Png,
            )
            .into_dimensions()
            .map_err(|error| error.to_string())?;
            let size = [width, height];
            let elapsed = job.started.elapsed().as_secs_f64();
            self.history
                .push(png.into(), job.source.clone(), size, Some(elapsed))?;
            self.message = format!("Rendered {} × {} in {elapsed:.1} s", size[0], size[1]);
            if let Some(log) = value["_log"].as_str().filter(|log| !log.is_empty()) {
                self.message.push('\n');
                self.message.push_str(log);
            }
            Ok(())
        });
        if let Err(error) = outcome {
            self.message = error;
        }
    }

    fn request(
        &mut self,
        model: &mut AppModel,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        self.poll();
        self.sync(model, false)?;
        if matches!(
            operation,
            "edit" | "quality" | "lighting" | "preset" | "hatch"
        ) {
            if self.job.is_some() {
                return Err("Wait for the current render to finish".into());
            }
            if args["sceneId"].as_u64() != self.editor.scene() || self.editor.scene().is_none() {
                return Err(
                    "The render target changed; repeat the edit on the current target".into(),
                );
            }
        }
        if matches!(operation, "start" | "useSettings")
            && args.get("sceneId").is_some()
            && (args["sceneId"].as_u64() != self.targets.scene()
                || (args.get("targetViewId").is_some()
                    && args["targetViewId"].as_u64() != self.targets.view()))
        {
            return Err("The render target changed; repeat the operation".into());
        }
        match operation {
            "poll" => {}
            "open" => {
                if let Some(backend) = args["backend"].as_str() {
                    if self.job.is_none() {
                        self.editor.values.insert("backend".into(), json!(backend));
                        self.editor.edited(self.clock.elapsed().as_secs_f64());
                        self.sync(model, true)?;
                    }
                }
            }
            "edit" => {
                let key = args["key"].as_str().ok_or("Render property is required")?;
                if !self.editor.values.contains_key(key) {
                    return Err("Unknown render property".into());
                }
                if key == "unit" {
                    let unit = args["value"].as_str().ok_or("Size unit is required")?;
                    if !["px", "in", "mm", "cm"].contains(&unit) {
                        return Err("Invalid size unit".into());
                    }
                    self.editor.set_unit(unit);
                } else {
                    self.editor.values.insert(key.into(), args["value"].clone());
                    if key.ends_with(".hatchStyle") {
                        for key in ["hatchLayersSpec", "hatchToneSpec"] {
                            self.editor.values.insert(self.editor.key(key), json!(""));
                        }
                    }
                }
                self.editor.edited(self.clock.elapsed().as_secs_f64());
            }
            "quality" => {
                let axis = args["axis"].as_str().unwrap_or("");
                let step = args["step"].as_str().unwrap_or("");
                if !quality_steps(axis).iter().any(|(id, _, _)| *id == step) {
                    return Err("Unknown render quality preset".into());
                }
                self.editor.apply_quality(axis, step);
                self.editor.edited(self.clock.elapsed().as_secs_f64());
            }
            "lighting" => {
                let lighting = args["value"].as_str().unwrap_or("");
                if !["none", "ao", "gi"].contains(&lighting) {
                    return Err("Unknown lighting mode".into());
                }
                self.editor.apply_lighting(lighting);
                self.editor.edited(self.clock.elapsed().as_secs_f64());
            }
            "hatch" => {
                let layers = args["layersSpec"]
                    .as_str()
                    .ok_or("Layer specification is required")?;
                let tone = args["toneSpec"]
                    .as_str()
                    .ok_or("Tone specification is required")?;
                let mut look =
                    cuemol_render::HatchLook::style(self.editor.text("umbreon_npr.hatchStyle"))
                        .map_err(|error| error.to_string())?;
                look.apply_spec(layers, 1)
                    .and_then(|_| look.apply_spec(tone, 6))
                    .map_err(|error| error.to_string())?;
                self.editor
                    .values
                    .insert("umbreon_npr.hatchLayersSpec".into(), json!(layers));
                self.editor
                    .values
                    .insert("umbreon_npr.hatchToneSpec".into(), json!(tone));
                self.editor.edited(self.clock.elapsed().as_secs_f64());
            }
            "preset" => {
                let preset = args["index"].as_u64().ok_or("Size preset is required")? as usize;
                let (size, dpi) = match preset {
                    0 => (self.view_size(model), None),
                    1 => ([100, 100], Some(72.)),
                    2 => ([300, 300], Some(300.)),
                    3 => ([600, 600], Some(300.)),
                    4 => ([1200, 1200], Some(600.)),
                    _ => return Err("Unknown size preset".into()),
                };
                self.editor.set_size(size, dpi);
                self.editor.edited(self.clock.elapsed().as_secs_f64());
            }
            "target" => {
                if self.job.is_some() {
                    return Err("Wait for the current render to finish".into());
                }
                self.sync(model, true)?;
                if !self
                    .targets
                    .select(args["viewId"].as_u64().ok_or("View is required")?)
                {
                    return Err("Render view is no longer available".into());
                }
                self.sync(model, false)?;
            }
            "close" | "flush" => self.sync(model, true)?,
            "undo" | "redo" => {
                self.sync(model, true)?;
                model.restore_animation_playback(false)?;
                model.animation_elapsed = 0.;
                if operation == "undo" {
                    model.undo(0)?;
                } else {
                    model.redo(0)?;
                }
                self.sync(model, false)?;
                self.scene_changed = true;
            }
            "start" => {
                if self.job.is_some() {
                    return Err("A render is already running".into());
                }
                self.editor.edited(self.clock.elapsed().as_secs_f64());
                self.sync(model, true)?;
                let scene = self
                    .targets
                    .scene()
                    .ok_or("Open a scene before rendering")?;
                let render = cuemol_command::prepare_render(
                    &model.desktop,
                    &model.camera,
                    "get_screenshot",
                    &json!({"sceneId":scene,"viewId":self.targets.view()}),
                )
                .map_err(|error| error.to_string())?
                .in_memory();
                let source = RenderSource::capture_scene(&model.desktop, Uid::new(scene))
                    .map_err(|error| error.to_string())?;
                let progress = render.pov_progress();
                let cancel = Arc::new(AtomicBool::new(false));
                let worker_cancel = cancel.clone();
                let wake = model.agent_wake.clone();
                let (sender, receiver) = mpsc::channel();
                let thread = std::thread::spawn(move || {
                    let _ = sender.send(
                        render
                            .run_with_cancel(&worker_cancel)
                            .map_err(|error| error.to_string()),
                    );
                    wake();
                });
                self.job = Some(RenderJob {
                    receiver,
                    cancel,
                    progress,
                    thread: Some(thread),
                    source,
                    started: Instant::now(),
                });
                self.message = "Rendering image…".into();
            }
            "stop" => {
                if let Some(job) = &self.job {
                    job.cancel.store(true, Ordering::Relaxed);
                    self.message = "Stopping render…".into();
                }
            }
            "select" => {
                self.history
                    .select(args["index"].as_u64().ok_or("Image index is required")? as usize)?;
            }
            "clear" => self.history.clear(),
            "useSettings" => {
                if self.job.is_some() {
                    return Err("Wait for the current render to finish".into());
                }
                self.sync(model, true)?;
                let settings = self.selected(args)?.source.settings_argument();
                let scene = self
                    .targets
                    .scene()
                    .ok_or("Render target is no longer available")?;
                self.apply(model, scene, &settings)?;
                self.sync(model, false)?;
                self.message = "Applied settings from the selected image".into();
            }
            "save" | "copy" => {
                self.selected(args)?;
                let png = self.history.image(self.history.selected().unwrap())?;
                if operation == "save" {
                    let path = args["path"]
                        .as_str()
                        .filter(|path| !path.is_empty())
                        .ok_or("Output path is required")?;
                    fs::write(path, &png).map_err(|error| error.to_string())?;
                    self.message = "Image saved".into();
                } else {
                    model.system_clipboard.write_png(&png)?;
                    self.message = "Image copied to clipboard".into();
                }
            }
            _ => return Err("Unknown rendering operation".into()),
        }
        self.snapshot(model, args, operation == "open")
    }

    fn selected(
        &self,
        args: &Value,
    ) -> Result<&cuemol_command::render_history::RenderRecord, String> {
        let record = self
            .history
            .current()
            .ok_or("No render result is selected")?;
        if args["imageId"].as_u64() != Some(record.id) {
            return Err("The selected image changed; repeat the operation".into());
        }
        Ok(record)
    }

    fn view_size(&self, model: &AppModel) -> [u32; 2] {
        if self.targets.view()
            == model
                .desktop
                .view_summaries()
                .iter()
                .find(|view| view.active)
                .map(|view| view.uid.get())
        {
            [model.view_width.max(1), model.view_height.max(1)]
        } else {
            self.targets
                .current()
                .map_or([100, 100], |target| target.size)
        }
    }

    fn snapshot(
        &self,
        model: &AppModel,
        args: &Value,
        include_catalog: bool,
    ) -> Result<Value, String> {
        let current = self.history.current();
        let mut value = json!({
            "values":self.editor.values,"sceneId":self.targets.scene(),"viewId":self.targets.view(),
            "targets":self.targets.views.iter().map(|target| json!({"sceneId":target.scene,"viewId":target.view,"label":target.label})).collect::<Vec<_>>(),
            "pixels":self.editor.pixels(),"lighting":self.editor.lighting(),
            "quality":(["aa","ao","giLighting","shadows"].into_iter().map(|axis| (axis.to_string(),json!({"selected":self.editor.quality_step(axis),"steps":quality_steps(axis).into_iter().map(|(id,label,_)| json!({"id":id,"label":label})).collect::<Vec<_>>()}))).collect::<serde_json::Map<_,_>>()),
            "running":self.job.is_some(),"message":self.message,
            "log":self.job.as_ref().and_then(|job|job.progress.as_ref()).map(|progress|progress.text()),
            "elapsed":self.job.as_ref().map(|job| job.started.elapsed().as_secs_f64()),
            "povError":if self.editor.backend()=="povray" {cuemol_command::pov::availability(&model.desktop).err()}else{None},
            "historyLength":self.history.len(),"selected":self.history.selected(),"current":current,
            "filename":current.map(|record|record.filename()),
        });
        if include_catalog {
            value["catalog"] = catalog().clone();
        }
        if self.editor.backend() == "umbreon_npr" {
            let mut look =
                cuemol_render::HatchLook::style(self.editor.text("umbreon_npr.hatchStyle"))
                    .map_err(|error| error.to_string())?;
            match look
                .apply_spec(self.editor.text("umbreon_npr.hatchLayersSpec"), 1)
                .and_then(|_| look.apply_spec(self.editor.text("umbreon_npr.hatchToneSpec"), 6))
            {
                Ok(()) => value["hatchSpec"] = json!(look.spec()),
                Err(error) => value["hatchError"] = json!(error.to_string()),
            }
            let mut default = cuemol_render::HatchLook::default();
            default.layers.push(cuemol_render::HatchLayer::default());
            value["defaultLayer"] = json!(default.spec().lines().next().unwrap());
        }
        if current.map(|record| record.id) != args["imageId"].as_u64() {
            value["image"] = if let Some(index) = self.history.selected() {
                json!(format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(self.history.image(index)?)
                ))
            } else {
                Value::Null
            };
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::model_without_gpu;
    use super::*;

    fn model() -> AppModel {
        let mut model = model_without_gpu();
        model.desktop.new_scene("render test", true);
        model
    }
    fn edit(model: &mut AppModel, key: &str, value: Value) {
        let scene = model.rendering.editor.scene();
        model
            .rendering_request("edit", &json!({"sceneId":scene,"key":key,"value":value}))
            .unwrap();
    }
    fn fixture() -> Arc<[u8]> {
        let image = image::RgbaImage::from_pixel(3, 2, image::Rgba([17, 44, 83, 120]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner().into()
    }
    fn wait(model: &mut AppModel) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while model.rendering.job.is_some() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            model.poll_rendering();
        }
        assert!(model.rendering.job.is_none(), "{}", model.rendering.message);
    }

    #[test]
    fn rendering_settings_debounce_target_staleness_and_undo() {
        let mut model = model();
        let first = model.desktop.active_scene().unwrap();
        let first_view = model.desktop.active_view_summary().unwrap().uid.get();
        model.rendering_request("open", &json!({})).unwrap();
        edit(&mut model, "dpi", json!(72));
        edit(&mut model, "dpi", json!(150));
        assert!(model.undo_stack.is_empty());
        assert!(!model.take_rendering_change());
        let second = model.desktop.new_scene("second", true).result_scene;
        model.rendering_request("poll", &json!({})).unwrap();
        assert_eq!(
            cuemol_command::scene_render_settings(&model.desktop, first).unwrap()["dpi"],
            150.
        );
        assert_eq!(model.undo_stack.len(), 1);
        assert!(model.take_rendering_change());
        assert!(!model.take_rendering_change());
        assert_eq!(model.rendering.editor.scene(), Some(second.get()));
        assert!(model
            .rendering_request(
                "edit",
                &json!({"sceneId":first.get(),"key":"dpi","value":72})
            )
            .is_err());
        model
            .rendering_request("target", &json!({"viewId":first_view}))
            .unwrap();
        edit(&mut model, "dpi", json!(300));
        model.rendering_request("close", &json!({})).unwrap();
        assert_eq!(model.desktop.active_scene(), Some(second));
        assert_eq!(model.undo_stack.len(), 2);
        model.undo(0).unwrap();
        let state = model.rendering_request("open", &json!({})).unwrap();
        assert_eq!(state["values"]["dpi"], 150.);
        assert_eq!(state["sceneId"], first.get());
        assert_eq!(model.desktop.active_scene(), Some(second));
        model.redo(0).unwrap();
        assert_eq!(
            model.rendering_request("poll", &json!({})).unwrap()["values"]["dpi"],
            300.
        );
    }

    #[test]
    fn rendering_history_save_copy_settings_and_image_cache() {
        let mut model = model();
        model.rendering_request("open", &json!({})).unwrap();
        let png = fixture();
        model
            .rendering
            .history
            .push(
                png.clone(),
                RenderSource {
                    scene_name: "sample".into(),
                    settings:
                        json!({"dpi":72,"backend":"umbreon_npr","umbreon_npr.hatchStyle":"pencil"})
                            .as_object()
                            .unwrap()
                            .clone(),
                },
                [3, 2],
                Some(0.1),
            )
            .unwrap();
        let state = model.rendering_request("poll", &json!({})).unwrap();
        let id = state["current"]["id"].clone();
        assert!(state["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(model
            .rendering_request("poll", &json!({"imageId":id}))
            .unwrap()
            .get("image")
            .is_none());
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.png");
        model
            .rendering_request("save", &json!({"imageId":id,"path":path}))
            .unwrap();
        assert_eq!(fs::read(path).unwrap(), png.as_ref());
        model
            .rendering_request("copy", &json!({"imageId":id}))
            .unwrap();
        let active = model.desktop.active_scene();
        let state = model
            .rendering_request("useSettings", &json!({"imageId":id}))
            .unwrap();
        assert_eq!(state["values"]["dpi"], 72.);
        assert_eq!(state["values"]["backend"], "umbreon_npr");
        assert_eq!(model.undo_stack.len(), 1);
        assert_eq!(model.desktop.active_scene(), active);
        assert!(model
            .rendering_request("copy", &json!({"imageId":999}))
            .is_err());
        model
            .rendering_request("clear", &json!({"imageId":id}))
            .unwrap();
        assert!(model.rendering.history.is_empty());
    }

    #[test]
    fn rendering_worker_uses_shared_output_and_preserves_previous_on_cancel_or_failure() {
        let mut model = model();
        model.rendering_request("open", &json!({})).unwrap();
        for (key, value) in [
            ("width", json!(8)),
            ("height", json!(6)),
            ("transparentBg", json!(true)),
            ("umbreon.useGI", json!(false)),
            ("umbreon.supersample", json!(1)),
        ] {
            edit(&mut model, key, value);
        }
        model.rendering_request("start", &json!({})).unwrap();
        wait(&mut model);
        assert_eq!(
            model.rendering.history.len(),
            1,
            "{}",
            model.rendering.message
        );
        assert_eq!(model.rendering.history.current().unwrap().size, [8, 6]);
        let previous = model.rendering.history.image(0).unwrap();
        let image = image::load_from_memory(&previous).unwrap().into_rgba8();
        assert!(image.pixels().all(|pixel| pixel[3] == 0));
        let camera = model.camera.clone();
        for failure in [false, true] {
            let (sender, receiver) = mpsc::channel();
            sender.send(if failure {Err("test renderer failure".into())}else{Ok(json!({"_image":base64::engine::general_purpose::STANDARD.encode(fixture())}))}).unwrap();
            model.rendering.job = Some(RenderJob {
                receiver,
                cancel: Arc::new(AtomicBool::new(!failure)),
                progress: None,
                thread: None,
                source: RenderSource::default(),
                started: Instant::now(),
            });
            model.poll_rendering();
            assert_eq!(model.rendering.history.len(), 1);
            assert_eq!(model.rendering.history.image(0).unwrap(), previous);
        }
        assert_eq!(model.camera.center(), camera.center());
    }

    #[test]
    fn rendering_backend_units_quality_and_npr_details_roundtrip() {
        let mut model = model();
        model
            .rendering_request("open", &json!({"backend":"povray"}))
            .unwrap();
        assert_eq!(model.undo_stack.len(), 1);
        model
            .rendering_request("open", &json!({"backend":"povray"}))
            .unwrap();
        assert_eq!(model.undo_stack.len(), 1);
        edit(&mut model, "backend", json!("umbreon_npr"));
        edit(&mut model, "unit", json!("in"));
        assert_eq!(model.rendering.editor.pixels(), [1200, 1200]);
        let scene = model.rendering.editor.scene();
        let state = model
            .rendering_request(
                "quality",
                &json!({"sceneId":scene,"axis":"aa","step":"high"}),
            )
            .unwrap();
        assert_eq!(state["values"]["umbreon_npr.supersample"], 3);
        let layers = "layer:kind=line,spacing=13,angle=37\nlayer:kind=dot,dotscale=2";
        let state=model.rendering_request("hatch", &json!({"sceneId":scene,"layersSpec":layers,"toneSpec":"tone:gamma=1.4\nink:levels=4"})).unwrap();
        assert!(state["hatchSpec"].as_str().unwrap().contains("gamma=1.4"));
        assert!(model
            .rendering_request(
                "hatch",
                &json!({"sceneId":scene,"layersSpec":"layer:unknown=2","toneSpec":""})
            )
            .is_err());
        model.rendering_request("close", &json!({})).unwrap();
        let state = model.rendering_request("open", &json!({})).unwrap();
        assert_eq!(state["values"]["umbreon_npr.hatchLayersSpec"], layers);
    }

    #[test]
    fn rendering_start_saves_displayed_defaults_and_immediate_undo_flushes_pending_edits() {
        let mut model = model();
        let state = model.rendering_request("open", &json!({})).unwrap();
        assert_eq!(state["pixels"], json!([1200, 1200]));
        model.rendering_request("start", &json!({})).unwrap();
        assert_eq!(
            model.rendering.job.as_ref().unwrap().source.settings["width"],
            1200.
        );
        model.rendering_request("stop", &json!({})).unwrap();
        wait(&mut model);
        edit(&mut model, "dpi", json!(72));
        let state = model.rendering_request("undo", &json!({})).unwrap();
        assert_eq!(state["values"]["dpi"], 600.);
        assert!(model.take_rendering_change());
        let state = model.rendering_request("redo", &json!({})).unwrap();
        assert_eq!(state["values"]["dpi"], 72.);
        assert!(model.take_rendering_change());
    }

    #[test]
    fn rendering_worker_drop_cancels_and_joins() {
        let cancel = Arc::new(AtomicBool::new(false));
        let worker = cancel.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let thread = std::thread::spawn(move || {
            while !worker.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(5));
            }
            done.store(true, Ordering::Relaxed);
        });
        let (_, receiver) = mpsc::channel();
        drop(RenderJob {
            receiver,
            cancel: cancel.clone(),
            progress: None,
            thread: Some(thread),
            source: RenderSource::default(),
            started: Instant::now(),
        });
        assert!(cancel.load(Ordering::Relaxed) && finished.load(Ordering::Relaxed));
    }
}
