use super::*;
use cuemol_desktop_core::PreparedFile;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace=cuemolLoader, js_name=parseFile)]
    async fn parse_file(file: web_sys::File) -> Result<JsValue, JsValue>;
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
        self.source_generation = self.source_generation.wrapping_add(1);
        self.source_queue.clear();
        self.source_active = false;
        self.source_scene = None;
        self.busy = false;
        self.message = "File loading cancelled.".into();
    }

    pub(super) fn queue_source(&mut self, file: web_sys::File) {
        if !self.source_active && self.source_queue.is_empty() {
            self.source_scene = self.state.active_scene();
        }
        self.source_queue.push_back(file);
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
        let Some(file) = self.source_queue.pop_front() else {
            return;
        };
        self.source_active = true;
        self.busy = true;
        self.source_generation = self.source_generation.wrapping_add(1);
        self.source_scene = self.state.active_scene();
        let generation = self.source_generation;
        let name = file.name();
        let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        self.message = format!("Loading {name}…");
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
            spawn_local(async move {
                let result = parse_file(file).await.map_err(js_error).and_then(|data| {
                    PreparedFile::decode(&js_sys::Uint8Array::new(&data).to_vec())
                        .map_err(|error| error.to_string())
                });
                match result {
                    Ok(file) => enqueue(Event::Prepared(generation, name, file)),
                    Err(error) => enqueue(Event::SourceError(generation, error)),
                }
                enqueue(Event::SourceFinished(generation));
            });
        }
    }

    pub(super) fn apply_prepared(&mut self, name: String, file: PreparedFile) {
        if !self.validate_source_scene() {
            return;
        }
        let source = file
            .scene_source()
            .map(|source| (file.name().to_string(), source));
        let before = self.capture_history();
        match self.state.open_prepared(file) {
            Ok(loaded) => {
                self.source_scene = self.state.active_scene();
                self.navigation.cancel();
                self.revision = self.revision.wrapping_add(1);
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
                if matches!(loaded, GuiLoadedPath::Scene { .. })
                    && [".qsc", ".qsl", ".pse"]
                        .iter()
                        .any(|suffix| filename.ends_with(suffix))
                {
                    self.frame = 0;
                    self.playing = false;
                    self.camera = self.state.active_view_camera().unwrap_or_default();
                } else {
                    self.fit(false);
                }
                self.message = format!("Loaded {name}");
            }
            Err(error) => {
                self.restore_history(before);
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
            Err(error) => self.message = error.to_string(),
        }
    }
}
