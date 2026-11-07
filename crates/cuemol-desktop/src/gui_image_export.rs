use super::*;
use cuemol_command::image_export::{ImageExportKind, ImageExportTarget};
use cuemol_ui::image_export::{Action, ImageExportEditor};

pub(super) struct ImageExportJob {
    receiver: mpsc::Receiver<cuemol_core::Result<PathBuf>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for ImageExportJob {
    fn drop(&mut self) {
        cuemol_command::external::cancel_workers(&self.cancel, [&self.receiver]);
    }
}

impl GuiApplication {
    pub(super) fn choose_export_image(&mut self, camera: &CameraController, kind: ImageExportKind) {
        if self.image_export_job.is_some() {
            self.set_status("An image export is already running");
            return;
        }
        let suggested = PathBuf::from(&self.export_path).with_extension("png");
        if let Some(path) = native_file_dialog(
            NativeFileDialog::SaveRenderedImage,
            &suggested.to_string_lossy(),
        ) {
            self.open_png_export_dialog(path, camera, kind);
        }
    }

    pub(super) fn open_png_export_dialog(
        &mut self,
        path: PathBuf,
        camera: &CameraController,
        kind: ImageExportKind,
    ) {
        if self.image_export_job.is_some() {
            self.set_status("An image export is already running");
            return;
        }
        let target = ImageExportTarget::capture(
            &self.state,
            &camera.to_qsys_camera("__current"),
            &serde_json::json!({}),
        );
        let mut target = match target {
            Ok(target) => target,
            Err(error) => {
                self.set_error(error);
                return;
            }
        };
        target.swap_stereo_eyes = camera.swap_stereo_eyes;
        let (width, height) = self
            .viewport_rect
            .map(|rect| {
                (
                    (rect.width().round() as u32).max(1),
                    (rect.height().round() as u32).max(1),
                )
            })
            .unwrap_or((1280, 960));
        self.png_export_draft = Some(NativePngExportDraft {
            path,
            target,
            kind,
            selection: self.selection_text.clone(),
            editor: ImageExportEditor::new(width, height),
        });
    }

    pub(super) fn png_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.png_export_draft.take() else {
            return;
        };
        match draft.editor.show(ctx, &draft.path.to_string_lossy()) {
            Action::None => self.png_export_draft = Some(draft),
            Action::Cancel => {}
            Action::Accept => self.start_image_export(ctx, draft),
        }
    }

    fn start_image_export(&mut self, ctx: &egui::Context, draft: NativePngExportDraft) {
        if self.image_export_job.is_some() {
            self.set_error(CueMolError::invalid_argument(
                "An image export is already running",
            ));
            return;
        }
        let prepared = crate::image_job::PreparedImageExport::capture(
            &self.state,
            &draft.target,
            draft.kind,
            draft.editor.options(),
            &draft.selection,
        );
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.set_error(error);
                return;
            }
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        let context = ctx.clone();
        let spawn = std::thread::Builder::new()
            .name("image-export".into())
            .spawn(move || {
                let result = prepared
                    .write(&draft.path, &worker_cancel)
                    .map(|()| draft.path);
                let _ = sender.send(result);
                context.request_repaint();
            });
        match spawn {
            Ok(_) => {
                self.image_export_job = Some(ImageExportJob { receiver, cancel });
                self.set_status("Exporting image");
            }
            Err(error) => self.set_error(CueMolError::Io(error.to_string())),
        }
    }

    pub(super) fn poll_image_export(&mut self, ctx: &egui::Context) {
        let Some(job) = self.image_export_job.as_ref() else {
            return;
        };
        let result = match job.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(CueMolError::invalid_argument(
                "Image export worker stopped",
            ))),
        };
        if let Some(result) = result {
            let cancelled = job.cancel.load(Ordering::Relaxed);
            self.image_export_job = None;
            match result {
                Ok(path) => {
                    self.export_path = path.to_string_lossy().into_owned();
                    self.set_status(format!("Exported image to {}", path.display()));
                }
                Err(_) if cancelled => self.set_status("Image export cancelled"),
                Err(error) => self.set_error(error),
            }
            return;
        }
        let mut open = true;
        egui::Window::new("Image export")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Exporting image");
                });
                if ui
                    .add_enabled(
                        !job.cancel.load(Ordering::Relaxed),
                        egui::Button::new("Cancel"),
                    )
                    .clicked()
                {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            });
        if !open {
            job.cancel.store(true, Ordering::Relaxed);
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;

    fn settings(app: &mut GuiApplication) {
        cuemol_command::call_tool(
            &mut app.state,
            &mut cuemol_qsys::Camera::new(),
            "set_render_settings",
            &serde_json::json!({"settings":"backend=povray; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1"}),
        ).unwrap();
    }

    #[test]
    fn image_export_options_cancel_keeps_live_and_saved_camera_and_existing_file() {
        let mut app = GuiApplication::new();
        app.config_path.clear();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing.png");
        fs::write(&path, b"existing image").unwrap();
        let camera = CameraController {
            ortho_scale: 73.,
            target: Vec3::new(8., 3., -2.),
            ..Default::default()
        };
        let before = app.state.clone();
        for kind in [ImageExportKind::Png, ImageExportKind::Umbreon] {
            app.open_png_export_dialog(path.clone(), &camera, kind);
            {
                let mut harness = egui_kittest::Harness::new(|ctx| app.png_export_dialog(ctx));
                harness.get_by_label("Cancel").click();
                harness.run();
            }
            assert!(app.png_export_draft.is_none());
            assert!(app.image_export_job.is_none());
            assert!(app.state.same_undo_state(&before));
            assert_eq!(
                app.state.modified_scene_uids(),
                before.modified_scene_uids()
            );
            assert!(app.undo_stack.is_empty());
            assert_eq!(fs::read(&path).unwrap(), b"existing image");
        }
    }

    #[test]
    fn image_export_worker_keeps_target_camera_and_concurrent_edits() {
        let mut app = GuiApplication::new();
        app.config_path.clear();
        app.state.open_bytes("atom.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        settings(&mut app);
        let camera = CameraController {
            ortho_scale: 4.,
            perspective: false,
            ..Default::default()
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("image.png");
        app.open_png_export_dialog(path.clone(), &camera, ImageExportKind::Umbreon);
        let mut draft = app.png_export_draft.take().unwrap();
        draft.editor.width = 32;
        draft.editor.height = 24;
        draft.editor.transparent = true;
        let expected = draft
            .target
            .prepare_umbreon(&app.state, draft.editor.options())
            .unwrap()
            .run_png_with_cancel(&AtomicBool::new(false))
            .unwrap();
        app.state.new_scene("other", true);
        let before = app.state.clone();
        let ctx = egui::Context::default();
        app.start_image_export(&ctx, draft);
        assert!(app.image_export_job.is_some());
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while app.image_export_job.is_some() && std::time::Instant::now() < deadline {
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.poll_image_export(ctx));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.image_export_job.is_none());
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert!(app.state.same_undo_state(&before));
        assert_eq!(
            app.state.modified_scene_uids(),
            before.modified_scene_uids()
        );
        assert!(app.undo_stack.is_empty());
        assert!(app.ray_panel.history.is_empty());
    }
}
