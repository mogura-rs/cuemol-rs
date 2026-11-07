use super::*;
use cuemol_desktop_core::PreparedFile;

pub(super) enum Source {
    File(
        web_sys::File,
        Option<Rc<RefCell<cuemol_desktop_core::FileOpenBatch>>>,
    ),
    Options(
        web_sys::File,
        Option<Rc<RefCell<cuemol_desktop_core::FileOpenBatch>>>,
    ),
    Camera(web_sys::File, String),
    Morph(web_sys::File, Rc<RefCell<morph::Import>>),
    Pdb(cuemol_command::pdb::DownloadFile),
}

pub(super) struct PendingOptions {
    file: web_sys::File,
    dialog: cuemol_ui::file_options::FileOptions,
    pub(super) companion: Option<web_sys::File>,
}

impl Source {
    fn name(&self) -> String {
        match self {
            Self::File(file, _)
            | Self::Options(file, _)
            | Self::Camera(file, _)
            | Self::Morph(file, _) => file.name(),
            Self::Pdb(file) => file.filename.clone(),
        }
    }

    async fn open(self, signal: Option<web_sys::AbortSignal>) -> Result<web_sys::File, JsValue> {
        let download = match self {
            Self::File(file, _)
            | Self::Options(file, _)
            | Self::Camera(file, _)
            | Self::Morph(file, _) => return Ok(file),
            Self::Pdb(download) => download,
        };
        let options = web_sys::RequestInit::new();
        options.set_signal(signal.as_ref());
        let response = JsFuture::from(
            web_sys::window()
                .ok_or_else(|| JsValue::from_str("Browser window is unavailable"))?
                .fetch_with_str_and_init(&download.url, &options),
        )
        .await?
        .dyn_into::<web_sys::Response>()?;
        if !response.ok() {
            return Err(JsValue::from_str(&format!("HTTP {}", response.status())));
        }
        let blob = JsFuture::from(response.blob()?).await?;
        let parts = js_sys::Array::new();
        parts.push(&blob);
        web_sys::File::new_with_blob_sequence(&parts, &download.filename)
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace=cuemolLoader, js_name=parseFile)]
    async fn parse_file(file: web_sys::File, pdb_kind: Option<String>) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_namespace=cuemolLoader, js_name=inspectFile)]
    async fn inspect_file(file: web_sys::File) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_namespace=cuemolLoader, js_name=parseFileWithOptions)]
    async fn parse_file_options(
        file: web_sys::File,
        options: String,
        companion: Option<web_sys::File>,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace=cuemolLoader, js_name=cancel)]
    fn cancel_worker();
}

impl App {
    pub(super) fn current_source(&self, generation: u64) -> bool {
        self.source_active && generation == self.source_generation
    }

    pub(super) fn validate_source_scene(&mut self) -> bool {
        if (self.source_active || !self.source_queue.is_empty())
            && self.source_scene != self.state.active_scene()
        {
            self.cancel_load();
            self.message = "File loading cancelled because the active scene changed.".into();
            return false;
        }
        true
    }

    pub(super) fn cancel_load(&mut self) {
        if !self.source_active && self.source_queue.is_empty() {
            return;
        }
        cancel_worker();
        if let Some(abort) = self.source_abort.take() {
            abort.abort();
        }
        self.source_generation = self.source_generation.wrapping_add(1);
        self.source_queue.clear();
        self.source_pdb = None;
        self.source_batch = None;
        self.source_setup = None;
        self.source_morph = None;
        self.source_target = None;
        self.source_options = None;
        if self.source_active {
            self.busy = false;
        }
        self.source_active = false;
        self.source_scene = None;
        self.message = "File loading cancelled.".into();
    }

    pub(super) fn queue_source(&mut self, file: web_sys::File) {
        self.queue_request(Source::Options(file, None));
    }

    pub(super) fn queue_request(&mut self, source: Source) {
        if !self.source_active && self.source_queue.is_empty() {
            self.source_scene = self.state.active_scene();
        }
        self.source_queue.push_back(source);
    }

    pub(super) fn open_get_pdb(&mut self) {
        self.pdb_dialog.reset();
        if let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        {
            if let Some(text) = storage
                .get_item("cuemol.getPdbDialog.history")
                .ok()
                .flatten()
            {
                if let Ok(history) = serde_json::from_str::<Vec<String>>(&text) {
                    self.pdb_dialog.history = history
                        .iter()
                        .filter_map(|value| cuemol_command::pdb::normalize_history(value))
                        .take(20)
                        .collect();
                }
            }
        }
        self.show_get_pdb = true;
    }

    pub(super) fn queue_pdb(&mut self, choice: cuemol_command::pdb::DownloadChoice) {
        match choice.files() {
            Ok(files) => {
                self.cancel_console("PDB download started; remaining console commands cancelled.");
                for file in files {
                    self.queue_request(Source::Pdb(file));
                }
                self.message = format!("Queued download {}.", choice.accession.id());
            }
            Err(error) => self.message = error,
        }
    }

    pub(super) fn queue_bytes(&mut self, name: &str, bytes: &[u8]) {
        let parts = js_sys::Array::new();
        parts.push(&js_sys::Uint8Array::from(bytes));
        match web_sys::File::new_with_u8_array_sequence(&parts, name) {
            Ok(file) => self.queue_source(file),
            Err(error) => self.message = js_error(error),
        }
    }

    pub(super) fn start_next_source(&mut self) {
        if !self.validate_source_scene() || self.source_active || self.busy {
            return;
        }
        let Some(source) = self.source_queue.pop_front() else {
            return;
        };
        self.source_active = true;
        self.busy = true;
        self.source_generation = self.source_generation.wrapping_add(1);
        self.source_scene = self.state.active_scene();
        let generation = self.source_generation;
        let name = source.name();
        let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        self.message = format!("Loading {name}…");
        self.source_batch = None;
        self.source_setup = None;
        self.source_target = None;
        let (file, camera_replace) = match source {
            Source::File(file, batch) => {
                self.source_batch = batch;
                (file, None)
            }
            Source::Options(file, batch) => {
                self.source_batch = batch;
                self.inspect_file_options(file, generation);
                return;
            }
            Source::Camera(file, name) => (file, Some(name)),
            Source::Morph(file, batch) => {
                if !self.current_morph_import(&batch.borrow()) {
                    enqueue(Event::SourceFinished(generation));
                    return;
                }
                self.source_morph = Some(batch.clone());
                self.start_prepared_source_options(
                    Source::Morph(file, batch),
                    name,
                    generation,
                    Some((
                        cuemol_desktop_core::GuiFileReaderOptions::Molecule(
                            cuemol_desktop_core::molecule_reader_options_for_format("pdb").unwrap(),
                        ),
                        None,
                    )),
                );
                return;
            }
            remote => {
                self.start_prepared_source(remote, name, generation);
                return;
            }
        };
        if matches!(
            extension.as_str(),
            "dcd" | "xtc" | "trr" | "nc" | "netcdf" | "mdcrd" | "crd"
        ) {
            let Some(id) = self.selected else {
                enqueue(Event::SourceError(
                    generation,
                    "Load and select a topology first.".into(),
                ));
                enqueue(Event::SourceFinished(generation));
                return;
            };
            let atoms = self
                .state
                .scene_manager()
                .object(id)
                .map(|object| object.mol().atom_count());
            self.message = "Indexing trajectory…".into();
            spawn_local(async move {
                let result = async {
                    let trajectory = BrowserTrajectory::open(file, atoms).await?;
                    let first = trajectory.frame(0).await?;
                    Ok::<_, JsValue>((trajectory, first))
                }
                .await;
                match result {
                    Ok((trajectory, first)) => {
                        enqueue(Event::Trajectory(generation, id, trajectory, first))
                    }
                    Err(error) => enqueue(Event::SourceError(generation, js_error(error))),
                }
                enqueue(Event::SourceFinished(generation));
            });
        } else if extension == "cam" {
            spawn_local(async move {
                if file.size() > 8_388_608. {
                    enqueue(Event::SourceError(
                        generation,
                        "Camera files must be smaller than 8 MiB.".into(),
                    ));
                } else {
                    match JsFuture::from(file.text()).await {
                        Ok(text) => enqueue(Event::CameraFile(
                            generation,
                            file,
                            text.as_string().unwrap_or_default(),
                            camera_replace,
                        )),
                        Err(error) => enqueue(Event::SourceError(generation, js_error(error))),
                    }
                }
                enqueue(Event::SourceFinished(generation));
            });
        } else if matches!(extension.as_str(), "icc" | "icm") {
            spawn_local(async move {
                match JsFuture::from(file.array_buffer()).await {
                    Ok(bytes) => enqueue(Event::Profile(
                        generation,
                        name,
                        js_sys::Uint8Array::new(&bytes).to_vec(),
                    )),
                    Err(error) => enqueue(Event::SourceError(generation, js_error(error))),
                }
                enqueue(Event::SourceFinished(generation));
            });
        } else {
            self.start_prepared_source(Source::File(file, None), name, generation);
        }
    }

    fn inspect_file_options(&mut self, file: web_sys::File, generation: u64) {
        self.message = format!("Reading options for {}…", file.name());
        spawn_local(async move {
            let result = async {
                let info = inspect_file(file.clone()).await.map_err(js_error)?;
                serde_json::from_str(
                    &info
                        .as_string()
                        .ok_or_else(|| "Invalid file metadata response".to_string())?,
                )
                .map_err(|error| error.to_string())
            }
            .await;
            enqueue(Event::FileOptions(generation, file, result));
        });
    }

    pub(super) fn receive_file_options(
        &mut self,
        generation: u64,
        file: web_sys::File,
        result: Result<Option<cuemol_desktop_core::GuiFileOpenMetadata>, String>,
    ) {
        match result {
            Ok(Some(metadata)) => {
                let dialog =
                    cuemol_ui::file_options::FileOptions::new(&file.name(), metadata, &self.state);
                self.message = format!("Choose options for {}", file.name());
                self.source_options = Some(PendingOptions {
                    file,
                    dialog,
                    companion: None,
                });
                self.busy = false;
            }
            Ok(None) => {
                self.source_active = false;
                self.busy = false;
                self.source_queue
                    .push_front(Source::File(file, self.source_batch.take()));
                self.start_next_source();
            }
            Err(error) => {
                enqueue(Event::SourceError(generation, error));
                enqueue(Event::SourceFinished(generation));
            }
        }
    }

    pub(super) fn file_options_ui(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.source_options.take() else {
            return;
        };
        let mut open = true;
        let mut action = None;
        egui::Window::new("Open file options")
            .open(&mut open)
            .collapsible(false)
            .default_width(460.)
            .resizable(true)
            .show(ctx, |ui| {
                ui.label(pending.file.name());
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 150.).max(200.))
                    .show(ui, |ui| {
                        action = pending.dialog.show(
                            ui,
                            pending
                                .companion
                                .as_ref()
                                .map(|file| file.name())
                                .as_deref(),
                        );
                    });
                action = pending
                    .dialog
                    .footer(
                        ui,
                        pending
                            .companion
                            .as_ref()
                            .map(|file| file.name())
                            .as_deref(),
                    )
                    .or(action);
            });
        self.ui_rects.extend(pending.dialog.rects.clone());
        use cuemol_ui::file_options::Action;
        if !open {
            action = Some(Action::Cancel);
        }
        match action {
            Some(Action::Open) => {
                self.state.remember_renderer_type(
                    &pending.dialog.metadata.object_class,
                    &pending.dialog.setup.renderer_type,
                );
                if let Some(selection) = pending.dialog.setup.selection.as_deref() {
                    self.state.remember_selection(selection);
                    self.persist_selection_history();
                }
                self.settings_text = self.state.config_text();
                if let Some(storage) =
                    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
                {
                    let _ = storage.set_item("cuemol.config", &self.settings_text);
                }
                self.source_setup = Some(pending.dialog.setup);
                self.source_target = Some(pending.dialog.metadata.target);
                let reader = pending.dialog.metadata.reader;
                let name = pending.file.name();
                self.busy = true;
                self.message = format!("Loading {name}…");
                self.start_prepared_source_options(
                    Source::File(pending.file, None),
                    name,
                    self.source_generation,
                    Some((reader, pending.companion)),
                );
            }
            Some(Action::Cancel) => {
                self.message = format!("Skipped {}", pending.file.name());
                enqueue(Event::SourceFinished(self.source_generation));
            }
            Some(Action::ChooseCompanion) => {
                if let Some(input) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.get_element_by_id("companion-file"))
                {
                    let _ =
                        input.set_attribute("data-generation", &self.source_generation.to_string());
                }
                choose_file_input("companion-file");
                self.source_options = Some(pending);
            }
            Some(Action::ClearCompanion) => {
                pending.companion = None;
                self.source_options = Some(pending);
            }
            None => self.source_options = Some(pending),
        }
    }

    fn start_prepared_source(&mut self, source: Source, name: String, generation: u64) {
        self.start_prepared_source_options(source, name, generation, None);
    }

    fn start_prepared_source_options(
        &mut self,
        source: Source,
        name: String,
        generation: u64,
        options: Option<(
            cuemol_desktop_core::GuiFileReaderOptions,
            Option<web_sys::File>,
        )>,
    ) {
        self.source_pdb = match &source {
            Source::Pdb(file) => Some(file.clone()),
            _ => None,
        };
        let pdb_kind = self.source_pdb.as_ref().map(|file| {
            match file.kind {
                cuemol_desktop_core::GuiPdbFileKind::Coordinates => "coordinates",
                cuemol_desktop_core::GuiPdbFileKind::TwoFoFc => "2fofc",
                cuemol_desktop_core::GuiPdbFileKind::FoFc => "fofc",
                cuemol_desktop_core::GuiPdbFileKind::Emdb => "emdb",
            }
            .to_owned()
        });
        self.source_abort = if matches!(&source, Source::Pdb(_)) {
            match web_sys::AbortController::new() {
                Ok(abort) => Some(abort),
                Err(error) => {
                    enqueue(Event::SourceError(generation, js_error(error)));
                    enqueue(Event::SourceFinished(generation));
                    return;
                }
            }
        } else {
            None
        };
        let signal = self
            .source_abort
            .as_ref()
            .map(web_sys::AbortController::signal);
        spawn_local(async move {
            let result = async {
                let file = source.open(signal).await.map_err(js_error)?;
                let data = if let Some((options, companion)) = options {
                    parse_file_options(
                        file,
                        serde_json::to_string(&options).map_err(|error| error.to_string())?,
                        companion,
                    )
                    .await
                } else {
                    parse_file(file, pdb_kind).await
                }
                .map_err(js_error)?;
                PreparedFile::decode(&js_sys::Uint8Array::new(&data).to_vec())
                    .map_err(|error| error.to_string())
            }
            .await;
            match result {
                Ok(file) => enqueue(Event::Prepared(generation, name, file)),
                Err(error) => enqueue(Event::SourceError(generation, error)),
            }
            enqueue(Event::SourceFinished(generation));
        });
    }

    pub(super) fn apply_prepared(&mut self, name: String, file: PreparedFile) {
        if !self.validate_source_scene() {
            return;
        }
        if self.source_morph.is_some() {
            self.apply_morph_prepared(file);
            return;
        }
        let source = file
            .scene_source()
            .map(|source| (file.name().to_string(), source));
        let style_name = file.style_name();
        let previous_selection = self.selected;
        let before = self.capture_history();
        let batch_before = self
            .source_batch
            .as_ref()
            .map(|batch| batch.borrow().clone());
        let _ = self.state.set_active_view_camera(&self.camera);
        let result = if let Some(batch) = &self.source_batch {
            batch.borrow_mut().open_prepared(&mut self.state, file)
        } else {
            self.state.open_prepared(file)
        }
        .and_then(|loaded| {
            if let Some(setup) = &self.source_setup {
                if self.source_target == Some(cuemol_desktop_core::GuiFileRendererTarget::Surface) {
                    let scene = self
                        .state
                        .active_scene()
                        .ok_or_else(|| cuemol_core::CueMolError::not_found("scene"))?;
                    let surface = self
                        .state
                        .surface_summaries(scene)
                        .last()
                        .cloned()
                        .ok_or_else(|| cuemol_core::CueMolError::not_found("loaded surface"))?;
                    self.state
                        .rename_surface(surface.uid, setup.object_name.clone())?;
                    for renderer in &surface.renderers {
                        self.state
                            .destroy_surface_renderer(surface.uid, renderer.uid)?;
                    }
                    let renderer = self.state.add_surface_renderer(surface.uid)?;
                    self.state.rename_surface_renderer(
                        surface.uid,
                        renderer,
                        setup.renderer_name.clone(),
                    )?;
                } else {
                    self.state.configure_loaded_bytes(
                        &loaded,
                        setup,
                        Some(self.camera.center()),
                    )?;
                }
            }
            if let Some(download) = &self.source_pdb {
                cuemol_command::pdb::configure_downloaded(
                    &mut self.state,
                    &mut self.camera,
                    &loaded,
                    download,
                )?;
            }
            Ok(loaded)
        });
        match result {
            Ok(loaded) => {
                self.source_scene = self.state.active_scene();
                self.navigation.cancel();
                self.revision = self.revision.wrapping_add(1);
                if source.is_none() {
                    self.state.mark_scene_changes_since(&before.state);
                }
                self.history.push(before);
                if self.history.len() > 12 {
                    self.history.remove(0);
                }
                self.future.clear();
                if source.is_some() {
                    self.last_scene = source;
                }
                self.selected = match loaded {
                    GuiLoadedPath::Object { object_uid, .. } => Some(object_uid),
                    _ if style_name.is_some() => previous_selection,
                    _ => None,
                };
                self.dirty = true;
                self.rebuild();
                let mut filename = name.to_ascii_lowercase();
                for suffix in [".gz", ".bz2", ".xz"] {
                    if let Some(base) = filename.strip_suffix(suffix) {
                        filename = base.to_string();
                        break;
                    }
                }
                if let Some(style_name) = style_name {
                    let target = self
                        .state
                        .active_scene()
                        .and_then(|scene| self.state.find_style_target(scene, &style_name));
                    self.open_style_editor(target);
                } else if matches!(loaded, GuiLoadedPath::Scene { .. })
                    && [".qsc", ".qsl", ".pse"]
                        .iter()
                        .any(|suffix| filename.ends_with(suffix))
                {
                    self.frame = 0;
                    self.playing = false;
                    self.camera = self.state.active_view_camera().unwrap_or_default();
                } else if self.source_pdb.is_none() {
                    if let GuiLoadedPath::Map { scene_uid, map_uid } = loaded {
                        let policy = self
                            .source_setup
                            .as_ref()
                            .and_then(|setup| setup.map_center_policy)
                            .unwrap_or_default();
                        let mut view = cuemol_qsys::View::new(Uid::new(1), scene_uid);
                        view.set_camera(&self.camera);
                        let size = self
                            .ui_rects
                            .get("Viewport")
                            .copied()
                            .unwrap_or([0., 0., 800., 600.]);
                        view.set_view_size(size[2].max(1.) as u32, size[3].max(1.) as u32);
                        if let Err(error) = self.state.apply_density_map_center_policy(
                            map_uid,
                            &[],
                            policy,
                            &mut view,
                        ) {
                            self.message = error.to_string();
                        }
                        self.camera = view.camera();
                        self.dirty = true;
                    } else if self
                        .source_setup
                        .as_ref()
                        .is_none_or(|setup| setup.recenter)
                    {
                        self.fit(false);
                    }
                }
                self.message = format!("Loaded {name}");
            }
            Err(error) => {
                self.restore_history(before);
                if let (Some(batch), Some(previous)) = (&self.source_batch, batch_before) {
                    *batch.borrow_mut() = previous;
                }
                if self.source_pdb.is_some() {
                    self.source_queue.clear();
                }
                self.message = error.to_string();
            }
        }
    }

    pub(super) fn apply_profile(&mut self, name: &str, bytes: &[u8]) {
        match cuemol_gfx::cms::register_icc_profile(name, bytes) {
            Ok(()) => {
                self.checkpoint();
                self.state.put_config_value("color:icc_profile", name);
                if let Some(scene) = self
                    .state
                    .active_scene()
                    .and_then(|id| self.state.scene_manager_mut().scene_mut(id))
                {
                    scene.set_scene_attribute("use_colproof", "true");
                    scene.set_scene_attribute("icc_filename", name);
                }
                self.dirty = true;
                self.message = format!("Loaded ICC profile {name}");
                if let Some(storage) =
                    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
                {
                    let result = storage
                        .set_item(
                            &format!("cuemol.icc.{name}"),
                            &base64::engine::general_purpose::STANDARD.encode(bytes),
                        )
                        .and_then(|_| storage.set_item("cuemol.config", &self.state.config_text()));
                    if result.is_err() {
                        self.message.push_str(" (browser storage is full)");
                    }
                }
            }
            Err(error) => {
                self.message = error.to_string();
            }
        }
    }
}
