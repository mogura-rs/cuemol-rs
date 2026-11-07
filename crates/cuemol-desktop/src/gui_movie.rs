use super::*;
use cuemol_command::movie::{MovieEvent, MovieJob};

impl GuiApplication {
    pub(super) fn initialize_native_movie_options(&mut self) {
        if self.ray_panel.movie_loaded {
            return;
        }
        let temporary = match cuemol_command::movie::sessions::session_directory() {
            Ok(path) => path.to_string_lossy().into_owned(),
            Err(error) => {
                self.ray_panel.message = error;
                String::new()
            }
        };
        self.ray_panel.load_movie_options(&self.state, &temporary);
        self.ray_panel.refresh_movie_outputs();
    }

    pub(super) fn request_native_render(&mut self, movie: bool) {
        self.open_native_render();
        if self.ray_job.is_some() || self.ray_movie_job.is_some() {
            return;
        }
        if movie {
            self.initialize_native_movie_options();
        }
        self.ray_panel.request_mode(movie);
    }

    pub(super) fn persist_native_movie_options(&mut self) {
        if !self.ray_panel.movie_loaded {
            return;
        }
        self.ray_panel.movie.persist(&mut self.state);
        let path = Path::new(self.config_path.trim());
        if path.is_absolute() {
            if let Err(error) = self.state.save_config_file(path) {
                self.ray_panel.message = error.to_string();
            }
        }
    }

    pub(super) fn start_native_movie(
        &mut self,
        ctx: &egui::Context,
        camera: &cuemol_qsys::Camera,
        overwrite: bool,
        reencode: bool,
    ) {
        if self.ray_job.is_some() || self.ray_movie_job.is_some() {
            return;
        }
        self.initialize_native_movie_options();
        let wake_context = ctx.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || wake_context.request_repaint());
        let options = self.ray_panel.movie.clone();
        let result = if reencode {
            let source = self
                .ray_panel
                .history
                .current()
                .filter(|record| {
                    record.movie.as_ref().is_some_and(|movie| {
                        movie.frames_available
                            && Path::new(&movie.output_dir).canonicalize().ok()
                                == Path::new(&options.output_dir).canonicalize().ok()
                            && cuemol_command::movie::base_name(&options.base_name)
                                .is_ok_and(|base| base == movie.base_name)
                    })
                })
                .map(|record| record.source.clone())
                .unwrap_or_default();
            MovieJob::reencode(&self.state, options, source, wake)
        } else {
            let settings = self.ray_panel.editor.settings_argument();
            let mut camera = camera.clone();
            if let Err(error) = self.apply_native_render_settings_to(
                self.ray_panel.targets.scene(),
                &settings,
                &mut camera,
            ) {
                self.ray_panel.failed(error.to_string());
                return;
            }
            MovieJob::start(
                &self.state,
                &camera,
                &serde_json::json!({"sceneId":self.ray_panel.targets.scene(),"viewId":self.ray_panel.targets.view()}),
                options,
                self.ray_panel.editor.pixels(),
                overwrite,
                wake,
            )
        };
        match result {
            Ok(job) => {
                if !reencode {
                    let (directory, base) = job.output_identity();
                    self.invalidate_native_movie_outputs(ctx, directory, base);
                }
                self.ray_panel.message = job.message();
                self.ray_movie_job = Some(job);
                self.ray_movie_preview_at = None;
                self.persist_native_movie_options();
            }
            Err(error) => {
                self.ray_panel.failed(error);
                self.ray_panel.refresh_movie_outputs();
            }
        }
    }

    pub(super) fn invalidate_native_movie_outputs(
        &mut self,
        ctx: &egui::Context,
        directory: &Path,
        base: &str,
    ) {
        self.ray_panel
            .history
            .invalidate_movie_outputs(directory, base);
        if let Some(index) = self.ray_panel.history.selected() {
            if let Err(error) = self.ray_panel.select_result(ctx, index) {
                self.ray_panel.failed(error);
            }
        }
    }

    pub(super) fn poll_native_movie(&mut self, ctx: &egui::Context) {
        let Some(mut job) = self.ray_movie_job.take() else {
            return;
        };
        let event = job.poll();
        match event {
            Some(MovieEvent::Finished(outcome)) => {
                if let Err(error) = self.ray_panel.movie_result(ctx, outcome) {
                    if let Some(index) = self.ray_panel.history.selected() {
                        let _ = self.ray_panel.select_result(ctx, index);
                    }
                    self.ray_panel.failed(error);
                }
                self.ray_movie_preview_at = None;
                self.set_status(
                    self.ray_panel
                        .message
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            event => {
                self.ray_panel.message = job.message();
                if let Some(MovieEvent::Frame { path, .. }) = event {
                    if self
                        .ray_movie_preview_at
                        .is_none_or(|at| at.elapsed() >= Duration::from_millis(250))
                        || job.completed() == job.total()
                    {
                        if let Err(error) = self.ray_panel.movie_preview(ctx, &path) {
                            self.ray_panel.message = error;
                        }
                        self.ray_movie_preview_at = Some(Instant::now());
                    }
                }
                self.ray_movie_job = Some(job);
            }
        }
    }

    pub(super) fn native_movie_file_action(&mut self, action: cuemol_ui::RenderAction) {
        let Some(path) = self
            .ray_panel
            .history
            .current()
            .and_then(|record| record.movie.as_ref())
            .and_then(|movie| movie.movie_path.as_ref())
            .map(PathBuf::from)
        else {
            return;
        };
        let result = match action {
            cuemol_ui::RenderAction::SaveMovie => {
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                let Some(destination) = rfd::FileDialog::new().set_file_name(filename).save_file()
                else {
                    return;
                };
                cuemol_command::movie::files::save_movie_as(&path, &destination)
            }
            cuemol_ui::RenderAction::OpenMovie => crate::open_system_path(&path),
            cuemol_ui::RenderAction::RevealMovie => crate::reveal_system_path(&path),
            _ => return,
        };
        self.ray_panel.message = result
            .map(|_| "Movie action completed".into())
            .unwrap_or_else(|error| error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn application(directory: &Path) -> (GuiApplication, cuemol_qsys::Camera) {
        let mut app = GuiApplication::new();
        app.config_path = directory.join("config.tsv").to_string_lossy().into_owned();
        let mut camera = cuemol_qsys::Camera::new();
        app.state.new_scene("Movie target", true);
        app.state.open_bytes("ligand.pdb", b"HETATM    1  C1  LIG A   1       1.000   0.000   0.000  1.00 10.00           C  \nEND\n").unwrap();
        cuemol_command::call_tool(
            &mut app.state,
            &mut camera,
            "add_anim",
            &serde_json::json!({"type":"spin"}),
        )
        .unwrap();
        let scene = app.state.active_scene().unwrap();
        let mut animation = app.state.scene_animation(scene).unwrap().unwrap();
        animation.set_length(time_from_seconds(0.09));
        app.state
            .set_scene_animation(scene, Some(&animation))
            .unwrap();
        app.apply_native_render_settings("backend=umbreon; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1; umbreon.denoise=None", &mut camera).unwrap();
        app.ray_panel.movie_loaded = true;
        app.ray_panel
            .movie
            .set_custom(directory.to_string_lossy().into_owned());
        app.ray_panel.movie.make_movie = false;
        app.request_native_render(true);
        app.ray_panel.editor.set_size([32, 24], None);
        (app, camera)
    }

    fn finish(app: &mut GuiApplication, ctx: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while app.ray_movie_job.is_some() {
            app.poll_native_render(ctx);
            assert!(Instant::now() < deadline, "{}", app.ray_panel.message);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn native_movie_stop_counts_a_frame_saved_before_its_completion_is_polled() {
        let directory = tempfile::tempdir().unwrap();
        let (mut app, camera) = application(directory.path());
        let ctx = egui::Context::default();
        app.start_native_movie(&ctx, &camera, false, false);
        let path = directory.path().join("movie_frm_0000.png");
        let deadline = Instant::now() + Duration::from_secs(30);
        while !path.is_file() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        app.ray_movie_job.as_ref().unwrap().cancel();
        finish(&mut app, &ctx);
        assert!(app.ray_panel.message.contains("cancelled"));
        let record = app.ray_panel.history.current().unwrap();
        assert_eq!(record.movie.as_ref().unwrap().frame_count, 1);
        assert_eq!(
            cuemol_command::movie::files::animation_frame_count(directory.path(), "movie").unwrap(),
            1
        );
        assert_eq!(
            app.ray_panel.png.as_deref().unwrap(),
            std::fs::read(path).unwrap()
        );
    }

    #[test]
    fn native_movie_window_uses_shared_frames_history_and_keeps_running_after_close() {
        let directory = tempfile::tempdir().unwrap();
        let (mut app, camera) = application(directory.path());
        let ctx = egui::Context::default();
        app.start_native_movie(&ctx, &camera, false, false);
        assert!(app.ray_movie_job.is_some(), "{}", app.ray_panel.message);
        app.tool_dialog = None;
        app.state.new_scene("Edit during movie", true);
        let edited = app.state.clone();
        finish(&mut app, &ctx);
        assert!(app.state.same_undo_state(&edited));
        assert_eq!(app.ray_panel.history.len(), 1);
        let record = app.ray_panel.history.current().unwrap();
        assert_eq!(record.size, [32, 24]);
        assert_eq!(record.source.scene_name, "Movie target");
        assert_eq!(record.movie.as_ref().unwrap().frame_count, 3);
        let first_png = app.ray_panel.png.clone().unwrap();
        assert_eq!(app.ray_panel.filename(), "Movie target-32x24-frame3.png");
        app.start_native_movie(&ctx, &camera, true, false);
        assert!(app.ray_movie_job.is_some(), "{}", app.ray_panel.message);
        assert!(
            !app.ray_panel
                .history
                .current()
                .unwrap()
                .movie
                .as_ref()
                .unwrap()
                .frames_available
        );
        assert!(app
            .ray_panel
            .history
            .select_frame_with(0, |_| Ok(()))
            .is_err());
        finish(&mut app, &ctx);
        assert_eq!(app.ray_panel.history.len(), 2);
        app.ray_panel.select_result(&ctx, 0).unwrap();
        assert_eq!(app.ray_panel.png.as_deref(), Some(first_png.as_ref()));
        assert!(app.state.same_undo_state(&edited));
        let mut restored = DesktopAppState::new();
        restored
            .load_config_file(directory.path().join("config.tsv"))
            .unwrap();
        let options = cuemol_command::movie::MovieOptions::load(&restored, "new temporary session");
        assert_eq!(options.output_dir, directory.path().to_string_lossy());
        assert!(!options.make_movie && !options.use_temp_dir);
    }

    #[test]
    fn native_movie_mode_requests_reset_the_size_without_changing_backend_settings() {
        let directory = tempfile::tempdir().unwrap();
        let (mut app, _) = application(directory.path());
        app.ray_panel.movie.fps = 29.97;
        app.ray_panel
            .editor
            .values
            .insert("umbreon.aoDistance".into(), serde_json::json!(4.));
        app.request_native_render(true);
        assert_eq!(app.ray_panel.editor.pixels(), [320, 240]);
        app.ray_panel.editor.set_size([800, 600], None);
        app.request_native_render(true);
        assert_eq!(app.ray_panel.editor.pixels(), [320, 240]);
        assert_eq!(app.ray_panel.editor.number("umbreon.aoDistance"), 4.);
        app.ray_panel.editor.set_size([800, 600], None);
        app.ray_panel.editor.edited(1.);
        app.tool_dialog = None;
        app.open_native_render();
        assert!(app.ray_panel.movie_mode);
        assert_eq!(app.ray_panel.editor.pixels(), [800, 600]);
        assert_eq!(app.ray_panel.movie.fps, 29.97);
        let before = app.state.clone();
        let undo = app.undo_stack.len();
        app.open_native_render();
        assert!(app.state.same_undo_state(&before));
        assert_eq!(app.undo_stack.len(), undo);
        app.request_native_render(false);
        assert_eq!(app.ray_panel.editor.pixels(), [1200, 1200]);
        assert_eq!(app.ray_panel.editor.number("dpi"), 600.);
        assert_eq!(app.ray_panel.movie.fps, 29.97);
        assert!(!app.ray_panel.movie_mode);
    }
}
