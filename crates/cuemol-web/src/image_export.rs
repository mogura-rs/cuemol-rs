use super::*;
use cuemol_command::image_export::{ImageExportKind, ImageExportTarget};
use cuemol_ui::image_export::{Action, ImageExportEditor};

#[derive(Default)]
pub(super) struct ImageExport {
    generation: u64,
    draft: Option<Draft>,
    running: Option<Running>,
}

struct Draft {
    target: ImageExportTarget,
    selection: String,
    kind: ImageExportKind,
    editor: ImageExportEditor,
    destination: Option<Destination>,
}

struct Running {
    cancel: Arc<AtomicBool>,
    abort: web_sys::AbortController,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.abort.abort();
    }
}

#[derive(serde::Deserialize)]
pub(super) struct Destination {
    id: String,
    name: String,
}

pub(super) enum Completion {
    Picked(u64, Result<Option<Destination>, String>),
    Finished(u64, Result<String, String>),
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace=cuemolImageExports, js_name=pick)]
    fn pick_image(name: &str) -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(catch, js_namespace=cuemolImageExports, js_name=write)]
    fn write_image(
        id: &str,
        bytes: &js_sys::Uint8Array,
        signal: &web_sys::AbortSignal,
    ) -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(js_namespace=cuemolImageExports, js_name=release)]
    fn release_image(id: &str);
}

fn error(value: JsValue) -> String {
    js_sys::Reflect::get(&value, &"message".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| js_error(value))
}

impl ImageExport {
    pub(super) fn active(&self) -> bool {
        self.draft.is_some() || self.running.is_some()
    }

    pub(super) fn status(&self) -> Value {
        if let Some(draft) = &self.draft {
            json!({"phase":if draft.destination.is_some() {"options"} else {"picker"},
                "width":draft.editor.width,"height":draft.editor.height,"dpi":draft.editor.resolution_dpi,
                "aspect":draft.editor.retain_aspect,"transparent":draft.editor.transparent,
                "kind":if draft.kind==ImageExportKind::Png {"png"} else {"umbreon"}})
        } else {
            json!({"phase":if self.running.is_some() {"running"} else {"idle"}})
        }
    }
}

impl App {
    pub(super) fn choose_image_export(&mut self, kind: ImageExportKind) {
        if self.busy || self.movie.running || self.image_export.active() {
            self.message = "Wait for the current operation to finish.".into();
            return;
        }
        let target = match ImageExportTarget::capture(&self.state, &self.camera, &json!({})) {
            Ok(target) => target,
            Err(error) => {
                self.message = error.to_string();
                return;
            }
        };
        let size = self.target.as_ref().map_or([1, 1], |target| target.size);
        let generation = self.image_export.generation.wrapping_add(1);
        self.image_export.generation = generation;
        self.image_export.draft = Some(Draft {
            target,
            kind,
            selection: self.selection.clone(),
            editor: ImageExportEditor::new(size[0], size[1]),
            destination: None,
        });
        let picked = pick_image("cuemol.png");
        spawn_local(async move {
            let result = async {
                let value = JsFuture::from(picked.map_err(error)?)
                    .await
                    .map_err(error)?;
                let json = js_sys::JSON::stringify(&value).map_err(error)?;
                serde_json::from_str(&String::from(json)).map_err(|e| e.to_string())
            }
            .await;
            enqueue(Event::ImageExport(Completion::Picked(generation, result)));
        });
    }

    pub(super) fn cancel_image_export(&mut self) {
        if let Some(draft) = self.image_export.draft.take() {
            if let Some(destination) = draft.destination {
                release_image(&destination.id);
            }
            self.image_export.generation = self.image_export.generation.wrapping_add(1);
        }
        if let Some(running) = &self.image_export.running {
            running.cancel.store(true, Ordering::Relaxed);
            running.abort.abort();
            cancel_oidn();
        }
    }

    pub(super) fn image_export_ui(&mut self, ctx: &egui::Context) {
        if let Some(draft) = &mut self.image_export.draft {
            if let Some(destination) = &draft.destination {
                let action = draft.editor.show(ctx, &destination.name);
                self.ui_rects.extend(draft.editor.rects.clone());
                match action {
                    Action::Accept => self.start_image_export(),
                    Action::Cancel => self.cancel_image_export(),
                    Action::None => {}
                }
            }
        }
        if let Some(running) = &self.image_export.running {
            let cancelling = running.cancel.load(Ordering::Relaxed);
            let mut cancel = false;
            egui::Window::new("Export image")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.spinner();
                    ui.label(if cancelling {
                        "Cancelling…"
                    } else {
                        "Rendering and saving image…"
                    });
                    let response = ui.add_enabled(!cancelling, egui::Button::new("Cancel"));
                    self.record_ui_rect("Image export stop", &response);
                    cancel = response.clicked();
                });
            if cancel {
                self.cancel_image_export();
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    fn start_image_export(&mut self) {
        if self.busy || self.movie.running {
            self.message = "Wait for the current operation to finish.".into();
            return;
        }
        let Some(draft) = self.image_export.draft.take() else {
            return;
        };
        let Some(destination) = draft.destination else {
            return;
        };
        enum Job {
            Png(cuemol_desktop_core::image_export::PreparedPngExport),
            Umbreon(cuemol_command::PreparedRender),
        }
        let prepare = || -> Result<Job, String> {
            draft
                .target
                .validate(&self.state)
                .map_err(|e| e.to_string())?;
            let options = draft.editor.options();
            match draft.kind {
                ImageExportKind::Png => {
                    cuemol_desktop_core::image_export::PreparedPngExport::capture(
                        &self.state,
                        draft.target.scene,
                        &draft.target.camera,
                        options,
                        &draft.selection,
                        draft.target.swap_stereo_eyes,
                    )
                    .map(Job::Png)
                    .map_err(|e| e.to_string())
                }
                ImageExportKind::Umbreon => draft
                    .target
                    .prepare_umbreon(&self.state, options)
                    .map(Job::Umbreon)
                    .map_err(|e| e.to_string()),
            }
        };
        let prepared = match prepare() {
            Ok(prepared) => prepared,
            Err(error) => {
                release_image(&destination.id);
                self.message = error;
                return;
            }
        };
        let abort = match web_sys::AbortController::new() {
            Ok(abort) => abort,
            Err(error) => {
                release_image(&destination.id);
                self.message = js_error(error);
                return;
            }
        };
        let signal = abort.signal();
        let cancel = Arc::new(AtomicBool::new(false));
        self.render_cancel = Some(cancel.clone());
        self.image_export.running = Some(Running {
            cancel: cancel.clone(),
            abort,
        });
        self.busy = true;
        self.message = "Exporting image…".into();
        let generation = self.image_export.generation;
        let gpu = self.gpu.clone();
        spawn_local(async move {
            let result = async {
                let png = match prepared {
                    Job::Png(job) => job.run_async(&gpu.device, &gpu.queue, Some(&cancel)).await,
                    Job::Umbreon(job) => job.run_png_async_with_cancel(Some(&cancel)).await,
                }
                .map_err(|e| e.to_string())?;
                if cancel.load(Ordering::Relaxed) {
                    return Err("Image export cancelled".into());
                }
                let bytes = js_sys::Uint8Array::from(png.as_slice());
                drop(png);
                JsFuture::from(write_image(&destination.id, &bytes, &signal).map_err(error)?)
                    .await
                    .map_err(error)?;
                Ok(destination.name.clone())
            }
            .await;
            release_image(&destination.id);
            enqueue(Event::ImageExport(Completion::Finished(generation, result)));
        });
    }

    pub(super) fn image_export_completion(&mut self, completion: Completion) {
        match completion {
            Completion::Picked(generation, result) => {
                if generation != self.image_export.generation || self.image_export.draft.is_none() {
                    if let Ok(Some(destination)) = result {
                        release_image(&destination.id);
                    }
                    return;
                }
                match result {
                    Ok(Some(destination)) => {
                        self.image_export.draft.as_mut().unwrap().destination = Some(destination)
                    }
                    Ok(None) => self.image_export.draft = None,
                    Err(error) => {
                        self.image_export.draft = None;
                        self.message = error;
                    }
                }
            }
            Completion::Finished(generation, result) => {
                if generation != self.image_export.generation {
                    return;
                }
                let cancelled = self
                    .image_export
                    .running
                    .as_ref()
                    .is_some_and(|job| job.cancel.load(Ordering::Relaxed));
                self.message = match result {
                    Ok(name) => format!("Saved {name}"),
                    Err(_) if cancelled => "Image export cancelled".into(),
                    Err(error) => error,
                };
                self.image_export.running = None;
                self.render_cancel = None;
                self.busy = false;
            }
        }
    }
}
