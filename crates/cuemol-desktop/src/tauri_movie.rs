use super::*;
use cuemol_command::movie::{self, files, MovieEvent, MovieJob, MovieOptions};

#[derive(Default)]
pub(super) struct MoviePanel {
    pub mode: bool,
    pub options: MovieOptions,
    pub job: Option<MovieJob>,
    pub preview: Option<Preview>,
    pub temporary: String,
    loaded: bool,
    preview_at: Option<Instant>,
    revision: u64,
    available_frames: u32,
    has_outputs: bool,
}

pub(super) struct Preview {
    pub png: Arc<[u8]>,
    size: [u32; 2],
    frame: u32,
}

impl Rendering {
    pub(super) fn initialize_movie(&mut self, model: &AppModel) -> Result<(), String> {
        if !self.movie.loaded {
            self.movie.temporary = movie::sessions::session_directory()?
                .to_string_lossy()
                .into_owned();
            self.movie.options = MovieOptions::load(&model.desktop, &self.movie.temporary);
            self.movie.loaded = true;
            self.refresh_movie_outputs();
        }
        Ok(())
    }

    pub(super) fn persist_movie_options(&self, model: &mut AppModel) -> Result<(), String> {
        if self.movie.loaded {
            self.movie.options.persist(&mut model.desktop);
            model.save_config()?;
        }
        Ok(())
    }

    pub(super) fn request_mode(&mut self, model: &AppModel, movie: bool) -> Result<(), String> {
        if movie {
            self.initialize_movie(model)?;
        }
        self.movie.mode = movie;
        self.editor.set_size(
            if movie { [320, 240] } else { [1200, 1200] },
            if movie { None } else { Some(600.) },
        );
        if self.targets.scene().is_some() {
            self.editor.edited(self.clock.elapsed().as_secs_f64());
        }
        Ok(())
    }

    pub(super) fn refresh_movie_outputs(&mut self) {
        let options = &self.movie.options;
        let directory = Path::new(&options.output_dir);
        self.movie.available_frames = movie::base_name(&options.base_name)
            .and_then(|base| files::animation_frame_count(directory, base))
            .unwrap_or(0);
        self.movie.has_outputs = movie::base_name(&options.base_name)
            .and_then(|base| files::has_outputs(directory, base))
            .unwrap_or(false);
    }

    pub(super) fn check_movie_options(&self, args: &Value) -> Result<(), String> {
        if let Some(expected) = args.get("movieOptions") {
            let expected: MovieOptions =
                serde_json::from_value(expected.clone()).map_err(|error| error.to_string())?;
            if serde_json::to_value(expected).unwrap()
                != serde_json::to_value(&self.movie.options).unwrap()
            {
                return Err("Movie output settings changed; repeat the operation".into());
            }
        }
        Ok(())
    }

    pub(super) fn start_movie(
        &mut self,
        model: &mut AppModel,
        args: &Value,
        reencode: bool,
    ) -> Result<(), String> {
        self.initialize_movie(model)?;
        self.check_movie_options(args)?;
        let options = self.movie.options.clone();
        let job = if reencode {
            let source = self
                .history
                .current()
                .filter(|record| {
                    record.movie.as_ref().is_some_and(|movie| {
                        movie.frames_available
                            && movie::base_name(&options.base_name)
                                .is_ok_and(|base| base == movie.base_name)
                            && Path::new(&movie.output_dir).canonicalize().ok()
                                == Path::new(&options.output_dir).canonicalize().ok()
                    })
                })
                .map(|record| record.source.clone())
                .unwrap_or_default();
            MovieJob::reencode(&model.desktop, options, source, model.agent_wake.clone())?
        } else {
            let overwrite = options.use_temp_dir || args["overwrite"].as_bool().unwrap_or(false);
            MovieJob::start(
                &model.desktop,
                &model.camera,
                &json!({"sceneId":self.targets.scene(),"viewId":self.targets.view()}),
                options,
                self.editor.pixels(),
                overwrite,
                model.agent_wake.clone(),
            )?
        };
        if !reencode {
            let (directory, base) = job.output_identity();
            self.history.invalidate_movie_outputs(directory, base);
            self.movie.revision += 1;
        }
        self.message = job.message();
        self.movie.job = Some(job);
        self.movie.preview = None;
        self.movie.preview_at = None;
        self.persist_movie_options(model)
    }

    pub(super) fn poll_movie(&mut self) {
        let Some(mut job) = self.movie.job.take() else {
            return;
        };
        match job.poll() {
            Some(MovieEvent::Finished(outcome)) => {
                self.movie.preview = None;
                self.movie.preview_at = None;
                self.movie.revision += 1;
                self.message = outcome.error.clone().unwrap_or_else(|| {
                    format!(
                        "Rendered {} frames in {:.1} s",
                        outcome.movie.frame_count, outcome.elapsed_seconds
                    )
                });
                if outcome.movie.frame_count > 0 {
                    let result = files::frame_path(
                        Path::new(&outcome.movie.output_dir),
                        &outcome.movie.base_name,
                        outcome.movie.frame_count - 1,
                    )
                    .and_then(|path| fs::read(path).map_err(|error| error.to_string()))
                    .and_then(|png| {
                        self.history.push_movie(
                            png.into(),
                            outcome.source,
                            outcome.size,
                            Some(outcome.elapsed_seconds),
                            outcome.movie,
                        )
                    });
                    if let Err(error) = result {
                        self.message.push_str(&format!("\n{error}"));
                    }
                }
                self.refresh_movie_outputs();
            }
            event => {
                self.message = job.message();
                if let Some(MovieEvent::Frame { path, index }) = event {
                    if self
                        .movie
                        .preview_at
                        .is_none_or(|at| at.elapsed() >= Duration::from_millis(250))
                        || job.completed() == job.total()
                    {
                        match fs::read(&path)
                            .map_err(|error| error.to_string())
                            .and_then(|png| {
                                let size = image::ImageReader::with_format(
                                    std::io::Cursor::new(&png),
                                    image::ImageFormat::Png,
                                )
                                .into_dimensions()
                                .map_err(|error| error.to_string())?;
                                Ok(Preview {
                                    png: png.into(),
                                    size: [size.0, size.1],
                                    frame: index,
                                })
                            }) {
                            Ok(preview) => {
                                self.movie.preview = Some(preview);
                                self.movie.preview_at = Some(Instant::now());
                                self.movie.revision += 1;
                            }
                            Err(error) => self.message.push_str(&format!("\n{error}")),
                        }
                    }
                }
                self.movie.job = Some(job);
            }
        }
    }

    pub(super) fn movie_request(
        &mut self,
        model: &mut AppModel,
        operation: &str,
        args: &Value,
    ) -> Result<bool, String> {
        if matches!(operation, "mode" | "movieEdit" | "reencode" | "cleanup") && self.running() {
            return Err("Wait for the current render to finish".into());
        }
        match operation {
            "mode" => self.request_mode(
                model,
                args["movie"].as_bool().ok_or("Render mode is required")?,
            )?,
            "movieEdit" => {
                self.initialize_movie(model)?;
                self.check_movie_options(args)?;
                let key = args["key"].as_str().ok_or("Movie property is required")?;
                let value = &args["value"];
                match key {
                    "useTempDir" => match value.as_bool() {
                        Some(true) => self.movie.options.select_temporary(&self.movie.temporary),
                        Some(false) => self.movie.options.select_custom(),
                        None => return Err("Output location is required".into()),
                    },
                    "outputDir" => self
                        .movie
                        .options
                        .set_custom(value.as_str().ok_or("Output folder is required")?.into()),
                    "baseName" | "fps" | "makeMovie" | "movieFormat" | "dupLastFrame"
                    | "bitrateKbps" => self.movie.options.patch(&json!({key:value})),
                    _ => return Err("Unknown movie property".into()),
                }
                if matches!(key, "useTempDir" | "outputDir" | "baseName") {
                    self.refresh_movie_outputs();
                }
                self.persist_movie_options(model)?;
            }
            "reencode" => self.start_movie(model, args, true)?,
            "cleanup" => {
                self.initialize_movie(model)?;
                self.check_movie_options(args)?;
                if args["confirmed"].as_bool() != Some(true) {
                    return Err("Confirm movie output cleanup first".into());
                }
                let count = MovieJob::cleanup(&self.movie.options)?;
                let base = movie::base_name(&self.movie.options.base_name)?;
                self.history
                    .invalidate_movie_outputs(Path::new(&self.movie.options.output_dir), base);
                self.movie.revision += 1;
                self.movie.preview = None;
                self.refresh_movie_outputs();
                self.message = format!("Removed {count} movie output files");
            }
            "frame" => {
                let mut selected = args.clone();
                selected
                    .as_object_mut()
                    .ok_or("Frame request is required")?
                    .remove("imageKey");
                self.selected(&selected)?;
                let index = args["index"]
                    .as_u64()
                    .and_then(|index| u32::try_from(index).ok())
                    .ok_or("Frame index is required")?;
                self.history.select_frame_with(index, |png| {
                    image::load_from_memory(png)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                })?;
            }
            "saveMovie" | "openMovie" | "revealMovie" => {
                let record = self.selected(args)?;
                let path = PathBuf::from(
                    record
                        .movie
                        .as_ref()
                        .and_then(|movie| movie.movie_path.as_ref())
                        .ok_or("No encoded movie is available")?,
                );
                match operation {
                    "saveMovie" => files::save_movie_as(
                        &path,
                        Path::new(
                            args["path"]
                                .as_str()
                                .filter(|path| !path.is_empty())
                                .ok_or("Output path is required")?,
                        ),
                    )?,
                    "openMovie" => crate::open_system_path(&path)?,
                    _ => crate::reveal_system_path(&path)?,
                }
                self.message = "Movie action completed".into();
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn image_key(&self) -> Value {
        if self.movie.preview.is_some() {
            json!(format!("live:{}", self.movie.revision))
        } else {
            json!(format!(
                "{:?}:{:?}:{}",
                self.history.current().map(|record| record.id),
                self.history.frame(),
                self.movie.revision
            ))
        }
    }

    pub(super) fn movie_snapshot(&self, model: &AppModel, value: &mut Value) {
        value["movieMode"] = json!(self.movie.mode);
        value["movieOptions"] = json!(self.movie.options);
        value["movieFormats"] = json!(movie::FORMATS);
        value["movieSizes"] = json!(movie::SIZE_PRESETS);
        value["availableFrames"] = json!(self.movie.available_frames);
        value["hasMovieOutputs"] = json!(self.movie.has_outputs);
        value["movieError"] = if self.movie.mode && self.movie.options.make_movie {
            json!(cuemol_command::external::resolve(&model.desktop, "ffmpeg")
                .err()
                .map(|error| error.to_string()))
        } else {
            Value::Null
        };
        value["imageKey"] = self.image_key();
        value["livePreview"] = json!(self.movie.preview.is_some());
        value["frame"] = json!(self.history.frame());
        value["preview"] = if let Some(preview) = &self.movie.preview {
            json!({"size":preview.size,"frame":preview.frame})
        } else {
            self.history
                .current()
                .map_or(Value::Null, |record| json!({"size":record.size}))
        };
        if let Some(job) = &self.movie.job {
            value["progress"] = json!({"completed":job.completed(),"total":job.total()});
        }
        if let Some(record) = self.history.current() {
            if record.movie.is_some() {
                value["filename"] = json!(format!(
                    "{}-frame{}.png",
                    record.filename().trim_end_matches(".png"),
                    self.history.frame().unwrap_or(0) + 1
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(directory: &Path) -> AppModel {
        let mut model = super::super::super::tests::model_without_gpu();
        model.config_path = Some(directory.join("config.tsv"));
        model.desktop.new_scene("Movie target", true);
        model.desktop.open_bytes("ligand.pdb", b"HETATM    1  C1  LIG A   1       1.000   0.000   0.000  1.00 10.00           C  \nEND\n").unwrap();
        cuemol_command::call_tool(
            &mut model.desktop,
            &mut model.camera,
            "add_anim",
            &json!({"type":"spin"}),
        )
        .unwrap();
        let scene = model.desktop.active_scene().unwrap();
        let mut animation = model.desktop.scene_animation(scene).unwrap().unwrap();
        animation.set_length(cuemol_core::time_from_seconds(0.09));
        model
            .desktop
            .set_scene_animation(scene, Some(&animation))
            .unwrap();
        model
            .rendering_request("open", &json!({"movie":true}))
            .unwrap();
        for (key, value) in [
            ("width", json!(32)),
            ("height", json!(24)),
            ("umbreon.useGI", json!(false)),
            ("umbreon.aoEnabled", json!(false)),
            ("umbreon.supersample", json!(1)),
            ("umbreon.denoise", json!("None")),
        ] {
            model
                .rendering_request(
                    "edit",
                    &json!({"sceneId":scene.get(),"key":key,"value":value}),
                )
                .unwrap();
        }
        for (key, value) in [
            ("outputDir", json!(directory)),
            ("fps", json!(29.97)),
            ("makeMovie", json!(false)),
        ] {
            model
                .rendering_request(
                    "movieEdit",
                    &json!({"sceneId":scene.get(),"key":key,"value":value}),
                )
                .unwrap();
        }
        model
    }
    fn finish(model: &mut AppModel) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        while model.rendering.running() {
            model.poll_rendering();
            assert!(Instant::now() < deadline, "{}", model.rendering.message);
            std::thread::sleep(Duration::from_millis(5));
        }
        model.rendering_request("poll", &json!({})).unwrap()
    }
    fn selected(state: &Value) -> Value {
        json!({"imageId":state["current"]["id"],"imageKey":state["imageKey"]})
    }

    #[test]
    fn rendering_movie_browser_numbers_and_target_physical_sizes_are_normalized() {
        let directory = tempfile::tempdir().unwrap();
        let mut model = model(directory.path());
        let scene = model.desktop.active_scene().unwrap();
        model.rendering.movie.options.fps = 30.;
        let mut expected = serde_json::to_value(&model.rendering.movie.options).unwrap();
        expected["fps"] = json!(30);
        model.rendering_request("movieEdit", &json!({"sceneId":scene.get(),"key":"outputDir","value":directory.path(),"movieOptions":expected})).unwrap();
        assert!(!model.rendering.movie.options.use_temp_dir);
        let first_view = model.desktop.active_view_summary().unwrap().uid.get();
        let second = model.desktop.new_scene("Physical size", true).result_scene;
        let second_view = model.desktop.active_view_summary().unwrap().uid.get();
        cuemol_command::call_tool(
            &mut model.desktop,
            &mut model.camera,
            "set_render_settings",
            &json!({"sceneId":second.get(),"settings":"width=2; height=1; unit=in; dpi=150"}),
        )
        .unwrap();
        model
            .rendering_request("target", &json!({"viewId":first_view}))
            .unwrap();
        let state = model
            .rendering_request("target", &json!({"viewId":second_view}))
            .unwrap();
        assert_eq!(state["values"]["unit"], "px");
        assert_eq!(state["values"]["width"], 300.);
        assert_eq!(state["values"]["height"], 150.);
        assert_eq!(state["pixels"], json!([300, 150]));
        let source = RenderSource::capture_scene(&model.desktop, second).unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(1, 1)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        model
            .rendering
            .history
            .push(png.into_inner().into(), source, [300, 150], None)
            .unwrap();
        let id = model.rendering.history.current().unwrap().id;
        let state = model
            .rendering_request("useSettings", &json!({"imageId":id}))
            .unwrap();
        assert_eq!(state["movieMode"], false);
        assert_eq!(state["values"]["unit"], "in");
        assert_eq!(state["values"]["width"], 2.);
    }

    #[test]
    fn rendering_movie_history_frames_survive_close_and_live_edits() {
        let directory = tempfile::tempdir().unwrap();
        let mut model = model(directory.path());
        model.rendering_request("start", &json!({})).unwrap();
        model.rendering_request("close", &json!({})).unwrap();
        assert!(model.rendering_pending());
        model.desktop.new_scene("Edit during movie", true);
        let edited = model.desktop.clone();
        let state = finish(&mut model);
        assert_eq!(state["current"]["movie"]["frameCount"], 3);
        assert_eq!(state["current"]["size"], json!([32, 24]));
        assert_eq!(state["current"]["source"]["scene_name"], "Movie target");
        assert!(model.desktop.same_scene_undo_state(&edited));
        let old_preview = model.rendering.history.image(0).unwrap();
        let mut args = selected(&state);
        args["index"] = json!(0);
        let first = model.rendering_request("frame", &args).unwrap();
        assert_eq!(first["frame"], 0);
        assert!(first.get("image").is_some());
        assert!(model
            .rendering_request("poll", &selected(&first))
            .unwrap()
            .get("image")
            .is_none());
        let path = directory.path().join("selected.png");
        let mut save = selected(&first);
        save["path"] = json!(path);
        model.rendering_request("save", &save).unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            fs::read(directory.path().join("movie_frm_0000.png")).unwrap()
        );
        assert!(model
            .rendering_request(
                "save",
                &json!({"imageId":state["current"]["id"],"imageKey":state["imageKey"],"path":path})
            )
            .is_err());
        assert!(model.rendering_request("start", &json!({})).is_err());
        let options = serde_json::to_value(&model.rendering.movie.options).unwrap();
        assert!(model
            .rendering_request("cleanup", &json!({"movieOptions":options}))
            .is_err());
        assert_eq!(
            files::animation_frame_count(directory.path(), "movie").unwrap(),
            3
        );
        let changed = json!({"outputDir":"different"});
        assert!(model
            .rendering_request("cleanup", &json!({"movieOptions":changed,"confirmed":true}))
            .is_err());
        let state = model
            .rendering_request("cleanup", &json!({"movieOptions":options,"confirmed":true}))
            .unwrap();
        assert_eq!(state["current"]["movie"]["framesAvailable"], false);
        assert_eq!(model.rendering.history.image(0).unwrap(), old_preview);
        assert_eq!(
            files::animation_frame_count(directory.path(), "movie").unwrap(),
            0
        );
        assert!(model.rendering_request("frame", &args).is_err());
        let mut restored = DesktopAppState::new();
        restored
            .load_config_file(directory.path().join("config.tsv"))
            .unwrap();
        let options = MovieOptions::load(&restored, "new session");
        assert_eq!(options.fps, 29.97);
        assert_eq!(options.output_dir, directory.path().to_string_lossy());
    }

    #[test]
    fn rendering_movie_stop_records_already_saved_frame_and_repeated_modes_reset_size() {
        let directory = tempfile::tempdir().unwrap();
        let mut model = model(directory.path());
        model
            .rendering_request("open", &json!({"movie":true}))
            .unwrap();
        assert_eq!(model.rendering.editor.pixels(), [320, 240]);
        model.rendering.editor.set_size([32, 24], None);
        model.rendering_request("start", &json!({})).unwrap();
        let path = directory.path().join("movie_frm_0000.png");
        let deadline = Instant::now() + Duration::from_secs(30);
        while !path.is_file() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        model.rendering.movie.job.as_ref().unwrap().cancel();
        let state = finish(&mut model);
        assert!(state["message"].as_str().unwrap().contains("cancelled"));
        assert_eq!(state["current"]["movie"]["frameCount"], 1);
        assert_eq!(
            files::animation_frame_count(directory.path(), "movie").unwrap(),
            1
        );
        model
            .rendering_request("open", &json!({"movie":false}))
            .unwrap();
        assert_eq!(model.rendering.editor.pixels(), [1200, 1200]);
        assert_eq!(model.rendering.editor.number("dpi"), 600.);
        assert_eq!(model.rendering.movie.options.fps, 29.97);
    }

    #[test]
    #[ignore = "requires FFmpeg"]
    fn rendering_movie_reencodes_existing_frames_and_saves_encoded_output() {
        let directory = tempfile::tempdir().unwrap();
        let mut model = model(directory.path());
        model.rendering_request("start", &json!({})).unwrap();
        finish(&mut model);
        let before = (0..3)
            .map(|index| {
                fs::read(files::frame_path(directory.path(), "movie", index).unwrap()).unwrap()
            })
            .collect::<Vec<_>>();
        let scene = model.desktop.active_scene().unwrap();
        model.desktop.set_scene_animation(scene, None).unwrap();
        for (key, value) in [("makeMovie", json!(true)), ("fps", json!(12.5))] {
            model
                .rendering_request(
                    "movieEdit",
                    &json!({"sceneId":scene.get(),"key":key,"value":value}),
                )
                .unwrap();
        }
        let active = model.desktop.clone();
        model
            .rendering_request(
                "reencode",
                &json!({"movieOptions":model.rendering.movie.options}),
            )
            .unwrap();
        let state = finish(&mut model);
        assert_eq!(state["historyLength"], 2);
        assert_eq!(state["current"]["movie"]["fps"], 12.5);
        assert_eq!(state["current"]["source"]["scene_name"], "Movie target");
        assert!(model.desktop.same_scene_undo_state(&active));
        let path = PathBuf::from(state["current"]["movie"]["moviePath"].as_str().unwrap());
        assert!(path.is_file());
        for (index, bytes) in before.iter().enumerate() {
            assert_eq!(
                &fs::read(files::frame_path(directory.path(), "movie", index as u32).unwrap())
                    .unwrap(),
                bytes
            );
        }
        let destination = directory.path().join("saved movie.mp4");
        let mut args = selected(&state);
        args["path"] = json!(destination);
        model.rendering_request("saveMovie", &args).unwrap();
        assert_eq!(fs::read(destination).unwrap(), fs::read(path).unwrap());
        model.rendering_request("clear", &json!({})).unwrap();
        assert!(directory.path().join("movie.mp4").is_file());
    }
}
