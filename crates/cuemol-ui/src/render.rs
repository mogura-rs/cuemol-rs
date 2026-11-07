use base64::Engine;
use cuemol_command::render_editor::{catalog, quality_steps, RenderEditor};
use cuemol_command::render_history::{RenderHistory, RenderSource};
use egui::{Context, Response, TextureHandle};
use serde_json::json;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
mod movie;

#[derive(Default, PartialEq, Eq)]
pub enum RenderAction {
    #[default]
    None,
    Start,
    Stop,
    Save,
    Copy,
    UseSettings,
    Undo,
    Redo,
    Target(u64),
    MovieMode(bool),
    StartMovie(bool),
    ReencodeMovie,
    CleanupMovie,
    PickMovieFolder,
    SaveMovie,
    OpenMovie,
    RevealMovie,
    #[cfg(target_arch = "wasm32")]
    MovieFrame(u32),
}

pub struct RenderControls {
    pub can_start: bool,
    pub pov_error: Option<String>,
    pub movie_error: Option<String>,
    pub view_size: [u32; 2],
}

struct PreviewTile {
    texture: TextureHandle,
    origin: egui::Vec2,
}

pub struct RenderPanel {
    tab: usize,
    pub editor: RenderEditor,
    pub targets: cuemol_command::render_editor::RenderTargets,
    preview: Option<Vec<PreviewTile>>,
    pub png: Option<Arc<[u8]>>,
    pub history: RenderHistory,
    pub movie_mode: bool,
    pub focused: bool,
    pub movie: cuemol_command::movie::MovieOptions,
    pub movie_loaded: bool,
    pub temporary_directory: String,
    pub available_frames: u32,
    pub has_movie_outputs: bool,
    pub requested_movie_frame: Option<(u64, u32)>,
    live_preview: bool,
    movie_action: RenderAction,
    confirm_movie_replace: bool,
    confirm_movie_cleanup: bool,
    controls_height: f32,
    pending_source: Option<RenderSource>,
    viewer: ImageFraming,
    confirm_clear: bool,
    pub message: String,
    started: Option<f64>,
    pub rects: BTreeMap<String, [f32; 4]>,
}

impl std::fmt::Debug for RenderPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderPanel")
            .field("editor", &self.editor)
            .field("has_preview", &self.preview.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for RenderPanel {
    fn default() -> Self {
        Self {
            tab: 0,
            editor: RenderEditor::default(),
            targets: Default::default(),
            preview: None,
            png: None,
            history: RenderHistory::default(),
            movie_mode: false,
            focused: true,
            movie: Default::default(),
            movie_loaded: false,
            temporary_directory: String::new(),
            available_frames: 0,
            has_movie_outputs: false,
            requested_movie_frame: None,
            live_preview: false,
            movie_action: RenderAction::None,
            confirm_movie_replace: false,
            confirm_movie_cleanup: false,
            controls_height: 240.,
            pending_source: None,
            viewer: ImageFraming::default(),
            confirm_clear: false,
            message: "No render result yet. Press Start Render to render the current view.".into(),
            started: None,
            rects: BTreeMap::new(),
        }
    }
}

impl RenderPanel {
    pub fn begin(&mut self, ctx: &Context, source: RenderSource) {
        self.started = Some(ctx.input(|i| i.time));
        self.pending_source = Some(source);
        self.message = "Rendering image…".into();
    }

    pub fn failed(&mut self, message: impl Into<String>) {
        self.started = None;
        self.pending_source = None;
        self.message = message.into();
    }

    pub fn filename(&self) -> String {
        let filename = self
            .history
            .current()
            .map(|entry| entry.filename())
            .unwrap_or_else(|| "render.png".into());
        match self.history.frame() {
            Some(frame) => format!(
                "{}-frame{}.png",
                filename.trim_end_matches(".png"),
                frame + 1
            ),
            None => filename,
        }
    }

    pub fn result(&mut self, ctx: &Context, value: &Value) -> Result<(), String> {
        let png: Arc<[u8]> = base64::engine::general_purpose::STANDARD
            .decode(value["_image"].as_str().ok_or("Render returned no image")?)
            .map_err(|e| e.to_string())?
            .into();
        let image = image::load_from_memory(&png)
            .map_err(|e| e.to_string())?
            .into_rgba8();
        let size = [image.width(), image.height()];
        let elapsed = self
            .started
            .map(|start| (ctx.input(|i| i.time) - start).max(0.));
        self.history.push(
            png.clone(),
            self.pending_source.clone().unwrap_or_default(),
            size,
            elapsed,
        )?;
        self.started = None;
        self.pending_source = None;
        self.live_preview = false;
        self.replace_preview(ctx, png, image);
        self.message = format!(
            "Rendered {} × {}{}",
            size[0],
            size[1],
            elapsed.map(|s| format!(" in {s:.1} s")).unwrap_or_default()
        );
        if let Some(log) = value["_log"].as_str().filter(|log| !log.is_empty()) {
            self.message.push('\n');
            self.message.push_str(log);
        }
        Ok(())
    }

    fn replace_preview(&mut self, ctx: &Context, png: Arc<[u8]>, image: image::RgbaImage) {
        let size = [image.width() as usize, image.height() as usize];
        let limit = ctx.input(|input| input.max_texture_side).max(1);
        let mut tiles = Vec::new();
        for y in (0..size[1]).step_by(limit) {
            for x in (0..size[0]).step_by(limit) {
                let tile_size = [(size[0] - x).min(limit), (size[1] - y).min(limit)];
                let tile = image::imageops::crop_imm(
                    &image,
                    x as u32,
                    y as u32,
                    tile_size[0] as u32,
                    tile_size[1] as u32,
                )
                .to_image();
                let color = egui::ColorImage::from_rgba_unmultiplied(tile_size, tile.as_raw());
                tiles.push(PreviewTile {
                    texture: ctx.load_texture(
                        "render-preview",
                        color,
                        egui::TextureOptions::LINEAR,
                    ),
                    origin: egui::vec2(x as f32, y as f32),
                });
            }
        }
        self.preview = Some(tiles);
        self.viewer
            .replace(egui::vec2(size[0] as f32, size[1] as f32));
        self.png = Some(png);
        ctx.request_repaint();
    }

    pub fn select_result(&mut self, ctx: &Context, index: usize) -> Result<(), String> {
        let (png, image) = self.history.select_with(index, |png| {
            image::load_from_memory(png)
                .map(|image| image.into_rgba8())
                .map_err(|e| e.to_string())
        })?;
        self.replace_preview(ctx, png, image);
        self.live_preview = false;
        self.requested_movie_frame = None;
        Ok(())
    }

    pub fn clear_results(&mut self) {
        self.history.clear();
        self.preview = None;
        self.png = None;
        self.viewer = ImageFraming::default();
        self.confirm_clear = false;
        self.live_preview = false;
        self.requested_movie_frame = None;
    }

    fn record(&mut self, label: &str, response: &Response) {
        let r = response.rect;
        self.rects
            .insert(label.into(), [r.min.x, r.min.y, r.width(), r.height()]);
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        open: &mut bool,
        running: bool,
        controls: RenderControls,
    ) -> RenderAction {
        self.rects.clear();
        self.movie_action = RenderAction::None;
        let mut action = RenderAction::None;
        let available = ctx.content_rect().size();
        let size = egui::vec2(
            (available.x - 40.).min(1000.),
            (available.y - 80.).min(700.),
        );
        let window = egui::Window::new("Rendering")
            .id(egui::Id::new("render-window"))
            .open(open)
            .collapsible(false)
            .default_size(size)
            .default_pos(ctx.content_rect().center() - size * 0.5)
            .max_size(available - egui::vec2(24., 40.))
            .min_size(egui::vec2(320., 360.))
            .show(ctx, |ui| {
                let height = ui.available_height().max(300.);
                if ui.available_width() >= 650. {
                    let left_width = ui.available_width() - 304.;
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(left_width, height),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                action = self.preview(
                                    ui,
                                    (height - self.controls_height).max(80.),
                                    running,
                                );
                                let top = ui.cursor().top();
                                let run = self.actions(ui, running, &controls);
                                let measured =
                                    ui.cursor().top() - top + ui.spacing().item_spacing.y;
                                if (self.controls_height - measured).abs() > 0.5 {
                                    self.controls_height = measured;
                                    ctx.request_repaint();
                                }
                                if run != RenderAction::None {
                                    action = run;
                                }
                            },
                        );
                        ui.separator();
                        ui.allocate_ui_with_layout(
                            egui::vec2(288., height),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.settings(ui, running, &controls);
                            },
                        );
                    });
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        action = self.preview(ui, 240., running);
                        let run = self.actions(ui, running, &controls);
                        if run != RenderAction::None {
                            action = run;
                        }
                        self.settings(ui, running, &controls);
                    });
                }
            });
        if let Some(window) = window {
            self.record("Rendering window", &window.response);
            if ctx.input(|input| input.pointer.any_pressed()) {
                if let Some(position) = ctx.pointer_latest_pos() {
                    let layer = ctx.layer_id_at(position);
                    if layer.is_none_or(|layer| layer.order <= egui::Order::Middle) {
                        self.focused = layer == Some(window.response.layer_id);
                    }
                }
            }
            if self.focused
                && !ctx.wants_keyboard_input()
                && !egui::Popup::is_any_open(ctx)
                && ctx.memory(|memory| memory.top_modal_layer().is_none())
            {
                for (modifiers, key, edit) in [
                    (
                        egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                        egui::Key::Z,
                        RenderAction::Redo,
                    ),
                    (egui::Modifiers::COMMAND, egui::Key::Y, RenderAction::Redo),
                    (egui::Modifiers::COMMAND, egui::Key::Z, RenderAction::Undo),
                ] {
                    if ctx.input_mut(|input| input.consume_key(modifiers, key)) {
                        action = edit;
                        break;
                    }
                }
            }
        }
        if self.confirm_clear {
            egui::Window::new("Clear render history")
                .id(egui::Id::new("clear-render-history"))
                .collapsible(false).resizable(false).show(ctx, |ui| {
                    ui.label("Discard every render and delete its temporary image? The scenes are unaffected.");
                    ui.horizontal(|ui| {
                        let cancel = ui.button("Cancel");
                        self.record("Cancel clear render history", &cancel);
                        if cancel.clicked() { self.confirm_clear = false; ctx.request_repaint(); }
                        let clear = ui.button("Clear");
                        self.record("Confirm clear render history", &clear);
                        if clear.clicked() { self.clear_results(); ctx.request_repaint(); }
                    });
                });
        }
        self.movie_confirmations(ctx, running);
        if self.movie_action != RenderAction::None {
            action = std::mem::take(&mut self.movie_action);
        }
        action
    }

    fn preview(&mut self, ui: &mut egui::Ui, height: f32, running: bool) -> RenderAction {
        let mut action = RenderAction::None;
        let top = ui.cursor().top();
        if self.png.is_some() && !self.live_preview {
            ui.horizontal_wrapped(|ui| {
                let index = self.history.selected().unwrap_or(0);
                for (label, enabled, target) in [
                    ("Previous", index > 0, index.saturating_sub(1)),
                    ("Next", index + 1 < self.history.len(), index + 1),
                ] {
                    let response = ui.add_enabled(enabled, egui::Button::new(label));
                    self.record(&format!("{label} render"), &response);
                    if response.clicked() {
                        if let Err(error) = self.select_result(ui.ctx(), target) {
                            self.message = error;
                        }
                    }
                }
                ui.label(format!(
                    "{} / {}",
                    self.history.selected().unwrap_or(0) + 1,
                    self.history.len()
                ));
                let response = ui.add_enabled(
                    !running
                        && self
                            .history
                            .current()
                            .is_some_and(|r| !r.source.settings.is_empty()),
                    egui::Button::new("Use settings"),
                );
                self.record("Use render settings", &response);
                if response.clicked() {
                    action = RenderAction::UseSettings;
                }
                let response = ui.button("Clear").on_hover_text("Clear render history");
                self.record("Clear render history", &response);
                if response.clicked() {
                    self.confirm_clear = true;
                }
                for (label, key, value) in [
                    ("Save", "Save render", RenderAction::Save),
                    ("Copy", "Copy render", RenderAction::Copy),
                ] {
                    let response = ui.button(label);
                    self.record(key, &response);
                    if response.clicked() {
                        action = value;
                    }
                }
                ui.menu_button("Settings", |ui| {
                    if let Some(record) = self.history.current() {
                        ui.label(format!("Scene: {}", record.source.scene_name));
                        egui::ScrollArea::vertical()
                            .max_height(240.)
                            .show(ui, |ui| {
                                for (key, value) in &record.source.settings {
                                    ui.label(format!(
                                        "{key}: {}",
                                        value
                                            .as_str()
                                            .map(str::to_string)
                                            .unwrap_or_else(|| value.to_string())
                                    ));
                                }
                            });
                    }
                });
                for (label, factor) in [("−", 0.8), ("+", 1.25)] {
                    let response = ui.button(label);
                    self.record(
                        if factor < 1. {
                            "Render zoom out"
                        } else {
                            "Render zoom in"
                        },
                        &response,
                    );
                    if response.clicked() {
                        self.viewer
                            .zoom(self.viewer.scale * factor, egui::Vec2::ZERO);
                    }
                }
                let response = ui.button("100%");
                self.record("Render actual size", &response);
                if response.clicked() {
                    self.viewer.zoom(1., egui::Vec2::ZERO);
                }
                let response = ui.button("Fit");
                self.record("Fit render", &response);
                if response.clicked() {
                    self.viewer.fit();
                }
                if let Some(record) = self.history.current() {
                    ui.weak(format!(
                        "{} · {} × {} · {:.0}%",
                        record.source.scene_name,
                        record.size[0],
                        record.size[1],
                        self.viewer.scale * 100.
                    ));
                }
            });
            self.movie_result_actions(ui, running, &mut action);
        }
        let toolbar = ui.cursor().top() - top;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), (height - toolbar).max(80.)),
            egui::Sense::click_and_drag(),
        );
        self.record("Render preview", &response);
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0., ui.visuals().panel_fill);
        self.viewer.viewport = rect.size();
        if let Some(tiles) = &self.preview {
            if !self.viewer.fitted {
                self.viewer.fit();
            }
            if response.dragged() {
                self.viewer.pan(ui.input(|i| i.pointer.delta()));
            }
            if response.hovered() {
                let (scroll, modifiers, pinch, pointer) = ui.input(|i| {
                    (
                        i.smooth_scroll_delta,
                        i.modifiers,
                        i.zoom_delta(),
                        i.pointer.hover_pos(),
                    )
                });
                let anchor = pointer.unwrap_or(rect.center()) - rect.center();
                if pinch != 1. {
                    self.viewer.zoom(self.viewer.scale * pinch, anchor);
                } else if modifiers.ctrl || modifiers.command {
                    self.viewer
                        .zoom(self.viewer.scale * (scroll.y * 0.002).exp(), anchor);
                } else {
                    self.viewer.pan(scroll);
                }
            }
            self.viewer.clamp_center();
            let display = self.viewer.size * self.viewer.scale;
            let image_rect =
                egui::Rect::from_min_size(rect.center() - display * self.viewer.center, display);
            let visible = image_rect.intersect(rect);
            if visible.is_positive() {
                let origin = visible.min;
                for y in 0..(visible.height() / 12.).ceil() as u32 {
                    for x in 0..(visible.width() / 12.).ceil() as u32 {
                        let cell = egui::Rect::from_min_size(
                            origin + egui::vec2(x as f32 * 12., y as f32 * 12.),
                            egui::vec2(12., 12.),
                        )
                        .intersect(visible);
                        painter.rect_filled(
                            cell,
                            0.,
                            egui::Color32::from_gray(if (x + y) % 2 == 0 { 48 } else { 64 }),
                        );
                    }
                }
                for tile in tiles {
                    let tile_rect = egui::Rect::from_min_size(
                        image_rect.min + tile.origin * self.viewer.scale,
                        tile.texture.size_vec2() * self.viewer.scale,
                    );
                    if tile_rect.intersects(visible) {
                        painter.image(
                            tile.texture.id(),
                            tile_rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                            egui::Color32::WHITE,
                        );
                    }
                }
            }
        } else {
            let text = painter.layout(
                "No render result yet.\nPress Start Render to render the current view.".into(),
                egui::FontId::proportional(13.),
                ui.visuals().weak_text_color(),
                (rect.width() - 40.).max(80.),
            );
            painter.galley(
                rect.center() - text.size() * 0.5,
                text,
                ui.visuals().weak_text_color(),
            );
        }
        action
    }

    fn actions(
        &mut self,
        ui: &mut egui::Ui,
        running: bool,
        controls: &RenderControls,
    ) -> RenderAction {
        let mut action = RenderAction::None;
        let available = self.editor.backend() != "povray" || controls.pov_error.is_none();
        let can_start =
            controls.can_start && available && (!self.movie_mode || controls.movie_error.is_none());
        ui.separator();
        ui.add_enabled_ui(!running, |ui| {
            ui.horizontal(|ui| {
                for (mode, label) in [(false, "Still"), (true, "Movie")] {
                    let response = ui.selectable_label(self.movie_mode == mode, label);
                    self.record(&format!("Render mode {label}"), &response);
                    if response.clicked() {
                        action = RenderAction::MovieMode(mode);
                    }
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            let response = ui.add_enabled(!running && can_start, egui::Button::new("Start Render"));
            self.record("Start Render", &response);
            if response.clicked() {
                if self.movie_mode {
                    if self.has_movie_outputs && !self.movie.use_temp_dir {
                        self.confirm_movie_replace = true;
                    } else {
                        action = RenderAction::StartMovie(self.movie.use_temp_dir);
                    }
                } else {
                    action = RenderAction::Start;
                }
            }
            if self.movie_mode {
                let response = ui.add_enabled(
                    !running
                        && self.movie.make_movie
                        && self.available_frames > 0
                        && controls.movie_error.is_none(),
                    egui::Button::new("Re-encode"),
                );
                self.record("Re-encode movie", &response);
                if response.clicked() {
                    action = RenderAction::ReencodeMovie;
                }
                let response = ui.add_enabled(
                    !running && self.has_movie_outputs && controls.movie_error.is_none(),
                    egui::Button::new("Cleanup"),
                );
                self.record("Cleanup movie", &response);
                if response.clicked() {
                    self.confirm_movie_cleanup = true;
                }
            }
            if running {
                ui.spinner();
                let response = ui.button("Stop");
                self.record("Stop render", &response);
                if response.clicked() {
                    action = RenderAction::Stop;
                }
            }
            ui.add_enabled_ui(!running, |ui| {
                let before = self.editor.backend().to_string();
                let mut backend = before.clone();
                let response = egui::ComboBox::from_id_salt("render-backend")
                    .selected_text(match backend.as_str() {
                        "povray" => "POV-Ray",
                        "umbreon_npr" => "Umbreon (NPR)",
                        _ => "Umbreon",
                    })
                    .show_ui(ui, |ui| {
                        for (id, label) in [
                            ("umbreon", "Umbreon"),
                            ("umbreon_npr", "Umbreon (NPR)"),
                            ("povray", "POV-Ray"),
                        ] {
                            let response = ui.selectable_value(&mut backend, id.into(), label);
                            self.record(&format!("Render backend {id}"), &response);
                        }
                    })
                    .response;
                self.record("Render backend", &response);
                if backend != before {
                    self.editor.values.insert("backend".into(), json!(backend));
                    self.editor.edited(ui.input(|i| i.time));
                    if self.tab == 2 {
                        self.tab = 1;
                    }
                }
            });
        });
        ui.add_enabled_ui(!running, |ui| {
            ui.horizontal(|ui| {
                ui.label("Target");
                let current = self.targets.view();
                let label = self
                    .targets
                    .current()
                    .map_or("No render target", |target| target.label.as_str())
                    .to_string();
                let targets = self.targets.views.clone();
                let response = egui::ComboBox::from_id_salt("render-target")
                    .selected_text(label)
                    .width(ui.available_width().min(360.))
                    .show_ui(ui, |ui| {
                        for target in targets {
                            let response =
                                ui.selectable_label(current == Some(target.view), &target.label);
                            self.record(&format!("Render target {}", target.view), &response);
                            if response.clicked() {
                                action = RenderAction::Target(target.view);
                            }
                        }
                    })
                    .response;
                self.record("Render target", &response);
            });
        });
        if self.editor.backend() == "povray" {
            if let Some(error) = &controls.pov_error {
                ui.colored_label(ui.visuals().warn_fg_color, error);
            }
        }
        if self.movie_mode {
            if let Some(error) = &controls.movie_error {
                ui.label(error);
            }
        }
        ui.separator();
        ui.weak("Log");
        egui::ScrollArea::vertical()
            .id_salt("render-result-log")
            .max_height(64.)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(egui::Label::new(&self.message).wrap());
            });
        action
    }

    fn settings(&mut self, ui: &mut egui::Ui, running: bool, controls: &RenderControls) {
        crate::shell::header(ui, "Render Settings");
        let before = self.editor.values.clone();
        ui.horizontal(|ui| {
            for (i, label) in [(0, "Image"), (1, "Render"), (2, "Detail")] {
                if i == 2 && self.editor.backend() != "umbreon_npr" {
                    continue;
                }
                let response = ui.selectable_value(&mut self.tab, i, label);
                self.record(&format!("Render settings {label}"), &response);
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("render-settings-scroll")
            .show(ui, |ui| {
                ui.add_enabled_ui(!running && self.editor.scene().is_some(), |ui| {
                    match self.tab {
                        0 => self.image_controls(ui, controls.view_size),
                        2 => {
                            if let Ok(mut hatch) =
                                cuemol_command::hatch_params(&self.editor.values, "umbreon_npr")
                            {
                                let old_layers = hatch.layers_spec.clone();
                                let old_tone = hatch.tone_spec.clone();
                                crate::hatch::hatch_detail_controls(ui, &mut hatch);
                                if hatch.layers_spec != old_layers {
                                    self.editor.values.insert(
                                        "umbreon_npr.hatchLayersSpec".into(),
                                        json!(hatch.layers_spec),
                                    );
                                }
                                if hatch.tone_spec != old_tone {
                                    self.editor.values.insert(
                                        "umbreon_npr.hatchToneSpec".into(),
                                        json!(hatch.tone_spec),
                                    );
                                }
                            }
                        }
                        _ => self.render_controls(ui),
                    }
                });
            });
        if self.editor.values != before {
            self.editor.edited(ui.input(|i| i.time));
        }
        if self.editor.pending() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(300));
        }
    }

    fn image_controls(&mut self, ui: &mut egui::Ui, view_size: [u32; 2]) {
        if self.movie_mode {
            self.movie_image_controls(ui);
            return;
        }
        ui.strong("Size");
        let size = self.editor.pixels();
        let presets = [
            ("Current view", view_size, None),
            ("100 × 100 (72 dpi)", [100, 100], Some(72.)),
            ("300 × 300 (300 dpi)", [300, 300], Some(300.)),
            ("600 × 600 (300 dpi)", [600, 600], Some(300.)),
            ("1200 × 1200 (600 dpi)", [1200, 1200], Some(600.)),
        ];
        let label = presets
            .iter()
            .skip(1)
            .find(|(_, pixels, dpi)| *pixels == size && *dpi == Some(self.editor.number("dpi")))
            .map_or("Custom", |(label, _, _)| *label);
        let response = egui::ComboBox::from_id_salt("render-size-preset")
            .selected_text(label)
            .width(ui.available_width() - 8.)
            .show_ui(ui, |ui| {
                ui.selectable_label(label == "Custom", "Custom").clicked();
                for (label, pixels, dpi) in presets {
                    if ui.selectable_label(false, label).clicked() {
                        self.editor.set_size(pixels, dpi);
                    }
                }
            })
            .response;
        self.record("Render size preset", &response);
        for key in ["width", "height"] {
            let label = if key == "width" { "Width" } else { "Height" };
            let mut value = self.editor.number(key);
            let unit = self.editor.text("unit").to_string();
            let (min, max, speed) = match unit.as_str() {
                "in" => (0.1, 60., 0.5),
                "mm" => (1., 1500., 10.),
                "cm" => (0.1, 150., 1.),
                _ => (1., 8192., 100.),
            };
            let response = ui
                .horizontal(|ui| {
                    ui.label(label);
                    ui.add(
                        egui::DragValue::new(&mut value)
                            .range(min..=max)
                            .speed(speed)
                            .max_decimals(if unit == "px" { 0 } else { 3 })
                            .suffix(format!(" {unit}")),
                    )
                })
                .inner;
            self.record(&format!("Render {label}"), &response);
            if response.changed() {
                self.editor.values.insert(key.into(), json!(value));
            }
        }
        let old_unit = self.editor.text("unit").to_string();
        let mut unit = old_unit.clone();
        let response = egui::ComboBox::from_label("Size unit")
            .selected_text(&unit)
            .show_ui(ui, |ui| {
                for value in ["px", "in", "mm", "cm"] {
                    let response = ui.selectable_value(&mut unit, value.into(), value);
                    self.record(&format!("Render unit {value}"), &response);
                }
            })
            .response;
        self.record("Render size unit", &response);
        if unit != old_unit {
            self.editor.set_unit(&unit);
        }
        ui.horizontal(|ui| {
            ui.label("DPI");
            let mut dpi = self.editor.number("dpi");
            let response = ui.add(
                egui::DragValue::new(&mut dpi)
                    .range(1. ..=4800.)
                    .max_decimals(3),
            );
            self.record("Render DPI", &response);
            if response.changed() {
                self.editor.values.insert("dpi".into(), json!(dpi));
            }
            egui::ComboBox::from_id_salt("render-dpi-preset")
                .selected_text("Presets")
                .show_ui(ui, |ui| {
                    for dpi in [72, 150, 300, 600, 1200, 2400] {
                        if ui.selectable_label(false, dpi.to_string()).clicked() {
                            self.editor.values.insert("dpi".into(), json!(dpi));
                        }
                    }
                });
        });
        ui.weak(format!(
            "{} × {} pixels",
            self.editor.pixels()[0],
            self.editor.pixels()[1]
        ));
        ui.separator();
        ui.strong("Output");
        let rows = catalog()["common"].as_array().unwrap();
        for row in rows
            .iter()
            .filter(|row| row["group"] == "Image" && row["type"] == "boolean")
        {
            if self.editor.backend() != "povray" && row["key"] != "transparentBg" {
                continue;
            }
            self.property(ui, row, false);
        }
    }

    fn render_controls(&mut self, ui: &mut egui::Ui) {
        if self.editor.backend() != "povray" {
            ui.strong("Quality");
            let before = self.editor.lighting();
            let mut lighting = before.to_string();
            let response = egui::ComboBox::from_label("Lighting")
                .selected_text(match before {
                    "gi" => "Global Illumination",
                    "ao" => "Ambient Occlusion",
                    _ => "Raytrace only",
                })
                .show_ui(ui, |ui| {
                    for (id, label) in [
                        ("none", "Raytrace only"),
                        ("ao", "Ambient Occlusion"),
                        ("gi", "Global Illumination"),
                    ] {
                        if id == "gi" && self.editor.backend() == "umbreon_npr" {
                            continue;
                        }
                        let response = ui.selectable_value(&mut lighting, id.into(), label);
                        self.record(&format!("Render lighting {id}"), &response);
                    }
                })
                .response;
            self.record("Render lighting", &response);
            if lighting != before {
                self.editor.apply_lighting(&lighting);
            }
            for (axis, label) in [
                ("aa", "Supersampling"),
                ("ao", "AO quality"),
                ("giLighting", "GI lighting"),
                ("shadows", "Shadows"),
            ] {
                if (axis == "ao" && self.editor.lighting() != "ao")
                    || (axis == "giLighting" && self.editor.lighting() != "gi")
                {
                    continue;
                }
                let before = self.editor.quality_step(axis).to_string();
                let mut selected = before.clone();
                let choices = quality_steps(axis);
                let current = choices
                    .iter()
                    .find(|(id, _, _)| *id == before)
                    .map_or("Custom", |(_, label, _)| *label);
                let response = egui::ComboBox::from_label(label)
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if before == "custom" {
                            ui.label("Custom");
                        }
                        for (id, label, _) in choices {
                            let response = ui.selectable_value(&mut selected, id.into(), label);
                            self.record(&format!("Render quality {axis} {id}"), &response);
                        }
                    })
                    .response;
                self.record(&format!("Render {label}"), &response);
                if selected != before {
                    self.editor.apply_quality(axis, &selected);
                }
            }
        }
        let backend = self.editor.backend().to_string();
        let rows = catalog()[&backend].as_array().cloned().unwrap_or_default();
        let common = catalog()["common"].as_array().unwrap();
        let groups = [
            "Camera",
            "Quality",
            "Hatching",
            "Antialiasing",
            "Lights",
            "Ambient Occlusion",
            "Shadows",
            "Edges",
            "Transparency",
            "Global Illumination",
            "POV-Ray",
        ];
        for group in groups {
            if (group == "Quality" && backend != "povray")
                || (group == "Ambient Occlusion" && self.editor.lighting() != "ao")
                || (group == "Global Illumination"
                    && (backend == "umbreon_npr" || self.editor.lighting() != "gi"))
            {
                continue;
            }
            let selected = common
                .iter()
                .map(|p| (p, false))
                .chain(rows.iter().map(|p| (p, true)))
                .filter(|(row, _)| row["group"] == group)
                .filter(|(row, _)| !matches!(row["key"].as_str(), Some("aoEnabled" | "useGI")))
                .filter(|(row, _)| {
                    backend == "povray"
                        || !matches!(
                            row["key"].as_str(),
                            Some("stereoMode" | "stereoDepth" | "numThreads")
                        )
                })
                .collect::<Vec<_>>();
            if selected.is_empty() {
                continue;
            }
            let response = egui::CollapsingHeader::new(group)
                .id_salt(("render-group", group))
                .default_open(matches!(group, "Camera" | "Hatching" | "POV-Ray"))
                .show(ui, |ui| {
                    for (row, local) in selected {
                        self.property(ui, row, local);
                    }
                });
            self.record(&format!("Render group {group}"), &response.header_response);
        }
    }

    fn property(&mut self, ui: &mut egui::Ui, row: &Value, local: bool) {
        let key = row["key"].as_str().unwrap();
        let path = if local {
            self.editor.key(key)
        } else {
            key.into()
        };
        let label = row["label"].as_str().unwrap();
        let before = self
            .editor
            .values
            .get(&path)
            .cloned()
            .unwrap_or(Value::Null);
        let mut value = before.clone();
        let enabled = match key {
            "hatchInkColor" => self.editor.flag(&self.editor.key("hatchCustomInk")),
            "hatchPaperColor" => self.editor.flag(&self.editor.key("hatchCustomPaper")),
            _ => true,
        };
        ui.push_id(&path, |ui| {
            ui.add_enabled_ui(enabled, |ui| match row["type"].as_str().unwrap() {
                "boolean" => {
                    let mut checked = value.as_bool().unwrap_or(false);
                    let response = ui.checkbox(&mut checked, label);
                    self.record(
                        if key == "transparentBg" {
                            "Render transparent"
                        } else {
                            label
                        },
                        &response,
                    );
                    value = json!(checked);
                }
                "enum" => {
                    let options = row["options"].as_array().unwrap();
                    let values = row["enumValues"].as_array().unwrap_or(options);
                    let index = values.iter().position(|v| {
                        *v == value || v.as_f64().zip(value.as_f64()).is_some_and(|(a, b)| a == b)
                    });
                    let current = index
                        .map(|i| options[i].as_str().unwrap())
                        .unwrap_or("Custom");
                    let response = egui::ComboBox::from_label(label)
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for (choice, stored) in options.iter().zip(values) {
                                let response = ui.selectable_value(
                                    &mut value,
                                    stored.clone(),
                                    choice.as_str().unwrap(),
                                );
                                self.record(
                                    &format!("Render {key} {}", choice.as_str().unwrap()),
                                    &response,
                                );
                            }
                        })
                        .response;
                    self.record(label, &response);
                }
                "color" => {
                    let text = value.as_str().unwrap_or("#000000");
                    let parsed = u32::from_str_radix(text.trim_start_matches('#'), 16).unwrap_or(0);
                    let mut rgb = [(parsed >> 16) as u8, (parsed >> 8) as u8, parsed as u8];
                    let response = ui
                        .horizontal(|ui| {
                            ui.label(label);
                            ui.color_edit_button_srgb(&mut rgb)
                        })
                        .inner;
                    self.record(label, &response);
                    if response.changed() {
                        value = json!(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]));
                    }
                }
                _ => {
                    let integer = row["type"] == "integer";
                    let mut number = value.as_f64().unwrap_or(0.);
                    let response = ui
                        .horizontal(|ui| {
                            ui.label(label);
                            ui.add(
                                egui::DragValue::new(&mut number)
                                    .range(
                                        row["min"].as_f64().unwrap_or(0.)
                                            ..=row["max"].as_f64().unwrap_or(10000.),
                                    )
                                    .speed(row["step"].as_f64().unwrap_or(0.1))
                                    .max_decimals(if integer { 0 } else { 3 }),
                            )
                        })
                        .inner;
                    self.record(label, &response);
                    if response.changed() {
                        value = if integer {
                            json!(number.round() as i32)
                        } else {
                            json!(number)
                        };
                    }
                }
            });
        });
        if value != before {
            self.editor.values.insert(path, value);
            if key == "hatchStyle" {
                for key in ["hatchLayersSpec", "hatchToneSpec"] {
                    self.editor.values.insert(self.editor.key(key), json!(""));
                }
            }
        }
    }
}

#[derive(Debug)]
struct ImageFraming {
    size: egui::Vec2,
    viewport: egui::Vec2,
    center: egui::Vec2,
    scale: f32,
    fitted: bool,
}

impl Default for ImageFraming {
    fn default() -> Self {
        Self {
            size: egui::Vec2::splat(1.),
            viewport: egui::Vec2::ZERO,
            center: egui::Vec2::splat(0.5),
            scale: 1.,
            fitted: false,
        }
    }
}

impl ImageFraming {
    fn fit_scale(&self) -> f32 {
        (self.viewport.x / self.size.x)
            .min(self.viewport.y / self.size.y)
            .clamp(0.05, 8.)
    }
    fn fit(&mut self) {
        self.scale = self.fit_scale();
        self.center = egui::Vec2::splat(0.5);
        self.fitted = true;
    }
    fn replace(&mut self, size: egui::Vec2) {
        let previous_fit = self.fit_scale();
        self.size = size;
        if self.fitted {
            self.scale = (self.scale * self.fit_scale() / previous_fit).clamp(0.05, 8.);
        }
    }
    fn zoom(&mut self, scale: f32, anchor: egui::Vec2) {
        let at = self.center + anchor / (self.size * self.scale);
        self.scale = scale.clamp(0.05, 8.);
        self.center = at - anchor / (self.size * self.scale);
        self.clamp_center();
    }
    fn pan(&mut self, delta: egui::Vec2) {
        self.center -= delta / (self.size * self.scale);
        self.clamp_center();
    }
    fn clamp_center(&mut self) {
        for i in 0..2 {
            let half = (self.viewport[i] / (self.size[i] * self.scale) * 0.5).min(0.5);
            self.center[i] = self.center[i].clamp(half, 1. - half);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(color: [u8; 4], size: [u32; 2]) -> Value {
        let image = image::RgbaImage::from_pixel(size[0], size[1], image::Rgba(color));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        serde_json::json!({"_image": base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())})
    }

    fn source(name: &str, dpi: u32) -> RenderSource {
        RenderSource {
            scene_name: name.into(),
            settings: serde_json::json!({"dpi": dpi}).as_object().unwrap().clone(),
        }
    }

    #[test]
    fn result_history_keeps_image_settings_and_framing_across_failures() {
        let ctx = Context::default();
        let mut panel = RenderPanel::default();
        panel.begin(&ctx, source("first", 72));
        panel
            .result(&ctx, &rendered([255, 0, 0, 128], [10, 20]))
            .unwrap();
        let first = panel.png.clone().unwrap();
        panel.viewer.viewport = egui::vec2(100., 100.);
        panel.viewer.fit();
        panel.viewer.zoom(8., egui::Vec2::ZERO);
        panel.begin(&ctx, source("second", 300));
        panel
            .result(&ctx, &rendered([0, 0, 255, 255], [20, 40]))
            .unwrap();
        assert_eq!(panel.history.len(), 2);
        assert_eq!(panel.viewer.scale, 4.);
        assert_eq!(panel.filename(), "second-20x40.png");
        panel.select_result(&ctx, 0).unwrap();
        assert_eq!(panel.png.as_deref(), Some(first.as_ref()));
        assert_eq!(panel.history.current().unwrap().source.settings["dpi"], 72);
        assert_eq!(panel.viewer.scale, 8.);
        panel.begin(&ctx, source("failed", 600));
        assert!(panel
            .result(&ctx, &serde_json::json!({"_image": "invalid"}))
            .is_err());
        panel.failed("Render cancelled");
        assert_eq!(panel.history.len(), 2);
        assert_eq!(panel.history.selected(), Some(0));
        assert_eq!(panel.png.as_deref(), Some(first.as_ref()));
        assert!(panel.started.is_none() && panel.pending_source.is_none());
        assert!(panel.select_result(&ctx, 50).is_err());
        assert_eq!(panel.history.selected(), Some(0));
        panel.clear_results();
        assert!(panel.history.is_empty() && panel.png.is_none() && panel.preview.is_none());
    }

    #[test]
    fn large_render_previews_preserve_original_pixels_with_bounded_textures() {
        let ctx = Context::default();
        ctx.input_mut(|input| input.max_texture_side = 64);
        let value = rendered([23, 45, 67, 128], [150, 100]);
        let mut panel = RenderPanel::default();
        panel.result(&ctx, &value).unwrap();
        let expected = base64::engine::general_purpose::STANDARD
            .decode(value["_image"].as_str().unwrap())
            .unwrap();
        assert_eq!(panel.png.as_deref(), Some(expected.as_slice()));
        assert_eq!(panel.viewer.size, egui::vec2(150., 100.));
        let tiles = panel.preview.as_ref().unwrap();
        assert_eq!(tiles.len(), 6);
        assert_eq!(tiles[5].origin, egui::vec2(128., 64.));
        assert_eq!(tiles[5].texture.size(), [22, 36]);
        assert!(tiles
            .iter()
            .all(|tile| tile.texture.size().iter().all(|size| *size <= 64)));
        assert_eq!(
            tiles
                .iter()
                .map(|tile| tile.texture.size()[0] * tile.texture.size()[1])
                .sum::<usize>(),
            150 * 100
        );
        panel
            .result(&ctx, &rendered([0, 0, 0, 255], [10, 20]))
            .unwrap();
        panel.select_result(&ctx, 0).unwrap();
        assert_eq!(panel.preview.as_ref().unwrap().len(), 6);
        assert_eq!(panel.png.as_deref(), Some(expected.as_slice()));
    }

    #[test]
    fn still_history_restores_physical_units_after_movie_display_conversion() {
        let ctx = Context::default();
        let mut panel = RenderPanel::default();
        let stored = json!({"unit":"in", "width":2, "height":1, "dpi":150})
            .as_object()
            .unwrap()
            .clone();
        panel.editor.sync(Some(1), &stored);
        panel.begin(
            &ctx,
            RenderSource {
                scene_name: "Still".into(),
                settings: stored.clone(),
            },
        );
        panel
            .result(&ctx, &rendered([0, 0, 0, 255], [300, 150]))
            .unwrap();
        panel.movie_mode = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| panel.movie_image_controls(ui));
        });
        assert_eq!(panel.editor.text("unit"), "px");
        assert_eq!(panel.editor.pixels(), [300, 150]);
        assert!(!panel.editor.pending());
        panel.restore_movie_options();
        panel.editor.sync(Some(1), &stored);
        assert!(!panel.movie_mode);
        assert_eq!(panel.editor.text("unit"), "in");
        assert_eq!(panel.editor.number("width"), 2.);
        assert_eq!(panel.editor.number("height"), 1.);
        assert_eq!(panel.editor.number("dpi"), 150.);
        assert_eq!(panel.editor.pixels(), [300, 150]);
    }

    #[test]
    fn zoom_stays_at_pointer_and_pan_is_bounded() {
        let mut framing = ImageFraming::default();
        framing.viewport = egui::vec2(200., 100.);
        framing.replace(egui::vec2(400., 200.));
        framing.fit();
        assert_eq!(framing.scale, 0.5);
        let anchor = egui::vec2(25., -10.);
        let before = framing.center + anchor / (framing.size * framing.scale);
        framing.zoom(2., anchor);
        let after = framing.center + anchor / (framing.size * framing.scale);
        assert!((before - after).length() < 1e-6);
        framing.pan(egui::vec2(1e6, -1e6));
        assert_eq!(framing.center, egui::vec2(0.125, 0.875));
        framing.fit();
        assert_eq!(framing.center, egui::Vec2::splat(0.5));
        framing.zoom(100., egui::Vec2::ZERO);
        assert_eq!(framing.scale, 8.);
        framing.zoom(0.0001, egui::Vec2::ZERO);
        assert_eq!(framing.scale, 0.05);
    }
}
