use super::*;

impl RenderPanel {
    pub fn request_mode(&mut self, movie: bool) {
        self.movie_mode = movie;
        self.editor.set_size(
            if movie { [320, 240] } else { [1200, 1200] },
            (!movie).then_some(600.),
        );
    }

    pub fn load_movie_options(
        &mut self,
        state: &cuemol_desktop_core::DesktopAppState,
        temporary: &str,
    ) {
        if self.movie_loaded {
            return;
        }
        self.temporary_directory = temporary.into();
        self.movie = cuemol_command::movie::MovieOptions::load(state, temporary);
        self.movie_loaded = true;
    }

    pub fn restore_movie_options(&mut self) {
        self.editor = RenderEditor::default();
        let movie = self
            .history
            .current()
            .and_then(|record| record.movie.as_ref());
        self.movie_mode = movie.is_some();
        if let Some(movie) = movie {
            self.movie = movie.options.clone();
            if self.movie.use_temp_dir {
                self.movie.output_dir = self.temporary_directory.clone();
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn refresh_movie_outputs(&mut self) {
        let directory = std::path::Path::new(&self.movie.output_dir);
        let base = cuemol_command::movie::base_name(&self.movie.base_name);
        self.available_frames = base
            .as_ref()
            .ok()
            .and_then(|base| {
                cuemol_command::movie::files::animation_frame_count(directory, base).ok()
            })
            .unwrap_or(0);
        self.has_movie_outputs = base
            .ok()
            .and_then(|base| cuemol_command::movie::files::has_outputs(directory, base).ok())
            .unwrap_or(false);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn movie_preview(&mut self, ctx: &Context, path: &std::path::Path) -> Result<(), String> {
        let png: Arc<[u8]> = std::fs::read(path).map_err(|e| e.to_string())?.into();
        let image = image::load_from_memory(&png)
            .map_err(|e| e.to_string())?
            .into_rgba8();
        self.replace_preview(ctx, png, image);
        self.live_preview = true;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn movie_result(
        &mut self,
        ctx: &Context,
        outcome: cuemol_command::movie::MovieOutcome,
    ) -> Result<(), String> {
        let movie = outcome.movie;
        if movie.frame_count > 0 {
            let path = cuemol_command::movie::files::frame_path(
                std::path::Path::new(&movie.output_dir),
                &movie.base_name,
                movie.frame_count - 1,
            )?;
            let png: Arc<[u8]> = std::fs::read(path).map_err(|e| e.to_string())?.into();
            let image = image::load_from_memory(&png)
                .map_err(|e| e.to_string())?
                .into_rgba8();
            self.history.push_movie(
                png.clone(),
                outcome.source,
                outcome.size,
                Some(outcome.elapsed_seconds),
                movie.clone(),
            )?;
            self.replace_preview(ctx, png, image);
        }
        self.started = None;
        self.pending_source = None;
        self.live_preview = false;
        self.message = outcome.error.unwrap_or_else(|| {
            format!(
                "Rendered {} frames in {:.1} s{}",
                movie.frame_count,
                outcome.elapsed_seconds,
                if movie.movie_path.is_some() {
                    " and encoded the movie"
                } else {
                    ""
                }
            )
        });
        self.refresh_movie_outputs();
        Ok(())
    }

    pub(super) fn movie_image_controls(&mut self, ui: &mut egui::Ui) {
        ui.strong("Size");
        if self.editor.text("unit") != "px" {
            self.editor.set_unit("px");
        }
        let size = self.editor.pixels();
        let presets = cuemol_command::movie::SIZE_PRESETS;
        let label = presets
            .iter()
            .find(|(_, pixels)| *pixels == size)
            .map_or("Custom", |(label, _)| *label);
        let response = egui::ComboBox::from_id_salt("render-size-preset")
            .selected_text(label)
            .width(ui.available_width() - 8.)
            .show_ui(ui, |ui| {
                for (label, pixels) in presets {
                    let response = ui.selectable_label(size == pixels, label);
                    self.record(&format!("Movie size {label}"), &response);
                    if response.clicked() {
                        self.editor.set_size(pixels, None);
                    }
                }
            })
            .response;
        self.record("Render size preset", &response);
        for (key, label) in [("width", "Width"), ("height", "Height")] {
            let mut value = self.editor.number(key);
            let response = ui
                .horizontal(|ui| {
                    ui.label(label);
                    ui.add(
                        egui::DragValue::new(&mut value)
                            .range(1. ..=8192.)
                            .speed(10.)
                            .max_decimals(0)
                            .suffix(" px"),
                    )
                })
                .inner;
            self.record(&format!("Render {label}"), &response);
            if response.changed() {
                self.editor.values.insert(key.into(), json!(value));
            }
        }
        ui.separator();
        ui.strong("Output");
        for row in catalog()["common"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["group"] == "Image" && row["type"] == "boolean")
        {
            if self.editor.backend() == "povray" || row["key"] == "transparentBg" {
                self.property(ui, row, false);
            }
        }
        ui.separator();
        ui.strong("Movie");
        ui.horizontal(|ui| {
            ui.label("Location");
            let response = ui.radio(self.movie.use_temp_dir, "Temporary");
            self.record("Movie temporary location", &response);
            if response.clicked() {
                self.movie.select_temporary(&self.temporary_directory);
            }
            let response = ui.radio(!self.movie.use_temp_dir, "Custom");
            self.record("Movie custom location", &response);
            if response.clicked() {
                self.movie.select_custom();
            }
        });
        ui.label("Folder");
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 36.).max(40.);
            let response = ui.add_enabled(
                !self.movie.use_temp_dir,
                egui::TextEdit::singleline(&mut self.movie.output_dir).desired_width(width),
            );
            self.record("Movie folder", &response);
            let response = ui.button("…").on_hover_text("Choose movie output folder");
            self.record("Browse movie folder", &response);
            if response.clicked() {
                self.movie_action = RenderAction::PickMovieFolder;
            }
        });
        ui.label("Base name");
        let response = ui.text_edit_singleline(&mut self.movie.base_name);
        self.record("Movie base name", &response);
        ui.horizontal(|ui| {
            ui.label("Frame rate");
            let response = ui.add(
                egui::DragValue::new(&mut self.movie.fps)
                    .range(f64::MIN_POSITIVE..=f64::MAX)
                    .speed(0.1)
                    .max_decimals(3)
                    .suffix(" fps"),
            );
            self.record("Movie frame rate", &response);
            egui::ComboBox::from_id_salt("movie-fps-presets")
                .selected_text("Presets")
                .show_ui(ui, |ui| {
                    for fps in [24., 30., 60.] {
                        if ui
                            .selectable_label(self.movie.fps == fps, fps.to_string())
                            .clicked()
                        {
                            self.movie.fps = fps;
                        }
                    }
                });
        });
        let response = ui.checkbox(&mut self.movie.dup_last_frame, "Duplicate last frame");
        self.record("Movie duplicate last frame", &response);
        let response = ui.checkbox(&mut self.movie.make_movie, "Encode movie");
        self.record("Encode movie", &response);
        ui.add_enabled_ui(self.movie.make_movie, |ui| {
            let formats = cuemol_command::movie::FORMATS;
            let label = formats
                .iter()
                .find(|(id, _)| *id == self.movie.movie_format)
                .map_or("Unknown", |(_, label)| *label);
            let response = egui::ComboBox::from_label("Format")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    for (id, label) in formats {
                        let response =
                            ui.selectable_value(&mut self.movie.movie_format, id.into(), label);
                        self.record(&format!("Movie format {id}"), &response);
                    }
                })
                .response;
            self.record("Movie format", &response);
            ui.add_enabled_ui(self.movie.movie_format != "mov_raw", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Bitrate");
                    let response = ui.add(
                        egui::DragValue::new(&mut self.movie.bitrate_kbps)
                            .range(1..=u32::MAX)
                            .suffix(" kbps"),
                    );
                    self.record("Movie bitrate", &response);
                    egui::ComboBox::from_id_salt("movie-bitrate-presets")
                        .selected_text("Presets")
                        .show_ui(ui, |ui| {
                            for bitrate in [256, 1024, 10240] {
                                if ui
                                    .selectable_label(
                                        self.movie.bitrate_kbps == bitrate,
                                        bitrate.to_string(),
                                    )
                                    .clicked()
                                {
                                    self.movie.bitrate_kbps = bitrate;
                                }
                            }
                        });
                });
            });
        });
    }

    pub(super) fn movie_result_actions(
        &mut self,
        ui: &mut egui::Ui,
        running: bool,
        action: &mut RenderAction,
    ) {
        let Some(movie) = self.history.current().and_then(|entry| entry.movie.clone()) else {
            return;
        };
        if movie.frame_count > 1 {
            let mut frame = self.history.frame().unwrap_or(movie.frame_count - 1) + 1;
            let response = ui.add_enabled(
                !running && movie.frames_available,
                egui::Slider::new(&mut frame, 1..=movie.frame_count).text("Frame"),
            );
            self.record("Movie frame", &response);
            #[cfg(not(target_arch = "wasm32"))]
            if response.changed() {
                let result = self.history.select_frame_with(frame - 1, |png| {
                    image::load_from_memory(png)
                        .map(|image| image.into_rgba8())
                        .map_err(|e| e.to_string())
                });
                match result {
                    Ok((png, image)) => self.replace_preview(ui.ctx(), png, image),
                    Err(error) => self.message = error,
                }
            }
        }
        if !movie.frames_available {
            ui.weak("Movie frames were replaced or removed. The saved preview remains available.");
        }
        if movie.movie_path.is_some() {
            ui.horizontal_wrapped(|ui| {
                for (label, intent) in [
                    ("Save movie as", RenderAction::SaveMovie),
                    ("Open movie", RenderAction::OpenMovie),
                    ("Reveal movie", RenderAction::RevealMovie),
                ] {
                    let response = ui.button(label);
                    self.record(label, &response);
                    if response.clicked() {
                        *action = intent;
                    }
                }
            });
        }
    }

    pub(super) fn movie_confirmations(&mut self, ctx: &Context, running: bool) {
        if running {
            self.confirm_movie_replace = false;
            self.confirm_movie_cleanup = false;
            return;
        }
        for replace in [true, false] {
            if !(if replace {
                self.confirm_movie_replace
            } else {
                self.confirm_movie_cleanup
            }) {
                continue;
            }
            egui::Window::new(if replace {
                "Replace movie outputs"
            } else {
                "Clean up movie outputs"
            })
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let base =
                    cuemol_command::movie::base_name(&self.movie.base_name).unwrap_or("movie");
                ui.label(format!(
                    "Delete the numbered frames and encoded movies named {base} in this folder?"
                ));
                ui.label(&self.movie.output_dir);
                if replace {
                    ui.label("The new render will start after deletion.");
                }
                ui.horizontal(|ui| {
                    let response = ui.button("Cancel");
                    self.record("Cancel movie output deletion", &response);
                    if response.clicked() {
                        self.confirm_movie_replace = false;
                        self.confirm_movie_cleanup = false;
                    }
                    let response = ui.button(if replace {
                        "Replace and render"
                    } else {
                        "Delete outputs"
                    });
                    self.record(
                        if replace {
                            "Confirm replace movie outputs"
                        } else {
                            "Confirm cleanup movie outputs"
                        },
                        &response,
                    );
                    if response.clicked() {
                        self.confirm_movie_replace = false;
                        self.confirm_movie_cleanup = false;
                        self.movie_action = if replace {
                            RenderAction::StartMovie(true)
                        } else {
                            RenderAction::CleanupMovie
                        };
                    }
                });
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movie_image_tab_exposes_pixel_size_and_backend_independent_output_controls() {
        let ctx = egui::Context::default();
        let mut state = cuemol_desktop_core::DesktopAppState::new();
        state.new_scene("Movie", true);
        let mut panel = RenderPanel::default();
        panel.targets.refresh(&state);
        panel
            .editor
            .sync(panel.targets.scene(), &Default::default());
        panel.request_mode(true);
        panel.movie.fps = 29.97;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200., 1000.),
                )),
                ..Default::default()
            },
            |ctx| {
                panel.show(
                    ctx,
                    &mut true,
                    false,
                    super::super::RenderControls {
                        can_start: true,
                        pov_error: None,
                        movie_error: None,
                        view_size: [640, 480],
                    },
                );
            },
        );
        for name in [
            "Movie frame rate",
            "Movie format",
            "Movie folder",
            "Movie base name",
            "Movie duplicate last frame",
            "Encode movie",
            "Re-encode movie",
            "Cleanup movie",
        ] {
            assert!(panel.rects.contains_key(name), "{name}");
        }
        assert!(!panel.rects.contains_key("Render DPI"));
        assert!(!panel.rects.contains_key("Render size unit"));
        assert_eq!(panel.editor.text("unit"), "px");
        assert_eq!(panel.movie.fps, 29.97);
    }
}
