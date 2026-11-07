use base64::Engine;
use cuemol_command::render_history::{RenderHistory, RenderSource};
use cuemol_render::{HatchRenderParams, UmbreonRenderParams};
use egui::{Context, Response, TextureHandle};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default, PartialEq, Eq)]
pub enum RenderAction {
    #[default]
    None,
    Start,
    Stop,
    Save,
    Copy,
    UseSettings,
}

pub struct RenderControls<'a> {
    pub can_start: bool,
    pub size: &'a mut [u32; 2],
    pub transparent: &'a mut bool,
    pub hatching: &'a mut bool,
    pub lighting: &'a mut UmbreonRenderParams,
    pub hatch: &'a mut HatchRenderParams,
}

pub struct RenderPanel {
    tab: usize,
    dpi: f64,
    unit: String,
    dimensions: [f64; 2],
    preview: Option<TextureHandle>,
    pub png: Option<Arc<[u8]>>,
    pub history: RenderHistory,
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
            .field("dpi", &self.dpi)
            .field("unit", &self.unit)
            .field("has_preview", &self.preview.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for RenderPanel {
    fn default() -> Self {
        Self {
            tab: 0,
            dpi: 600.,
            unit: "px".into(),
            dimensions: [1200.; 2],
            preview: None,
            png: None,
            history: RenderHistory::default(),
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
    pub fn read_settings(&mut self, settings: &Map<String, Value>, size: &mut [u32; 2]) {
        self.dpi = settings
            .get("dpi")
            .and_then(Value::as_f64)
            .unwrap_or(600.)
            .clamp(1., 4800.);
        self.unit = settings
            .get("unit")
            .and_then(Value::as_str)
            .filter(|u| ["px", "in", "cm", "mm"].contains(u))
            .unwrap_or("px")
            .into();
        for (i, key) in ["width", "height"].iter().enumerate() {
            let value = settings
                .get(*key)
                .and_then(Value::as_f64)
                .unwrap_or(size[i] as f64 / self.scale());
            self.dimensions[i] = value;
            size[i] = (value * self.scale()).round().clamp(1., 8192.) as u32;
        }
    }

    fn scale(&self) -> f64 {
        match self.unit.as_str() {
            "in" => self.dpi,
            "cm" => self.dpi / 2.54,
            "mm" => self.dpi / 25.4,
            _ => 1.,
        }
    }

    pub fn image_settings(&self, size: [u32; 2]) -> String {
        format!(
            "width={}; height={}; unit={}; dpi={}",
            if self.unit == "px" {
                size[0] as f64
            } else {
                self.dimensions[0]
            },
            if self.unit == "px" {
                size[1] as f64
            } else {
                self.dimensions[1]
            },
            self.unit,
            self.dpi
        )
    }

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
        self.history
            .current()
            .map(|entry| entry.filename())
            .unwrap_or_else(|| "render.png".into())
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
        self.replace_preview(ctx, png, image);
        self.message = format!(
            "Rendered {} × {}{}",
            size[0],
            size[1],
            elapsed.map(|s| format!(" in {s:.1} s")).unwrap_or_default()
        );
        Ok(())
    }

    fn replace_preview(&mut self, ctx: &Context, png: Arc<[u8]>, image: image::RgbaImage) {
        let size = [image.width() as usize, image.height() as usize];
        let image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        self.preview =
            Some(ctx.load_texture("render-preview", image, egui::TextureOptions::LINEAR));
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
        Ok(())
    }

    pub fn clear_results(&mut self) {
        self.history.clear();
        self.preview = None;
        self.png = None;
        self.viewer = ImageFraming::default();
        self.confirm_clear = false;
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
        mut controls: RenderControls<'_>,
    ) -> RenderAction {
        self.rects.clear();
        let mut action = RenderAction::None;
        let available = ctx.content_rect().size();
        let size = egui::vec2(
            (available.x - 40.).min(1000.),
            (available.y - 80.).min(700.),
        );
        egui::Window::new("Rendering")
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
                                action = self.preview(ui, (height - 176.).max(120.), running);
                                let run = self.actions(
                                    ui,
                                    running,
                                    controls.can_start,
                                    controls.hatching,
                                );
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
                                self.settings(ui, running, &mut controls);
                            },
                        );
                    });
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        action = self.preview(ui, 240., running);
                        let run = self.actions(ui, running, controls.can_start, controls.hatching);
                        if run != RenderAction::None {
                            action = run;
                        }
                        self.settings(ui, running, &mut controls);
                    });
                }
            });
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
        action
    }

    fn preview(&mut self, ui: &mut egui::Ui, height: f32, running: bool) -> RenderAction {
        let mut action = RenderAction::None;
        let top = ui.cursor().top();
        if self.png.is_some() {
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
        if let Some(texture) = &self.preview {
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
                painter.image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                    egui::Color32::WHITE,
                );
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
        can_start: bool,
        hatching: &mut bool,
    ) -> RenderAction {
        let mut action = RenderAction::None;
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            let response = ui.add_enabled(!running && can_start, egui::Button::new("Start Render"));
            self.record("Start Render", &response);
            if response.clicked() {
                action = RenderAction::Start;
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
                egui::ComboBox::from_id_salt("render-backend")
                    .selected_text(if *hatching {
                        "Umbreon (NPR)"
                    } else {
                        "Umbreon"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(hatching, false, "Umbreon");
                        ui.selectable_value(hatching, true, "Umbreon (NPR)");
                    });
            });
        });
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

    fn settings(&mut self, ui: &mut egui::Ui, running: bool, controls: &mut RenderControls<'_>) {
        crate::shell::header(ui, "Render Settings");
        ui.horizontal(|ui| {
            for (i, label) in [(0, "Image"), (1, "Render")] {
                let response = ui.selectable_value(&mut self.tab, i, label);
                self.record(&format!("Render settings {label}"), &response);
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("render-settings-scroll")
            .show(ui, |ui| {
                ui.add_enabled_ui(!running, |ui| {
                    if self.tab == 0 {
                        ui.strong("Size");
                        ui.label("Preset");
                        egui::ComboBox::from_id_salt("render-size-preset")
                            .width(ui.available_width() - 8.)
                            .selected_text(format!(
                                "{} × {} ({} dpi)",
                                controls.size[0], controls.size[1], self.dpi
                            ))
                            .show_ui(ui, |ui| {
                                for (label, size, dpi) in [
                                    ("640 × 480 (72 dpi)", [640, 480], 72.),
                                    ("1200 × 1200 (600 dpi)", [1200, 1200], 600.),
                                    ("1920 × 1080 (300 dpi)", [1920, 1080], 300.),
                                    ("2400 × 2400 (600 dpi)", [2400, 2400], 600.),
                                ] {
                                    if ui
                                        .selectable_label(
                                            *controls.size == size && self.dpi == dpi,
                                            label,
                                        )
                                        .clicked()
                                    {
                                        *controls.size = size;
                                        self.dpi = dpi;
                                        self.unit = "px".into();
                                    }
                                }
                            });
                        ui.horizontal(|ui| {
                            for (i, label) in [(0, "Width"), (1, "Height")] {
                                let scale = self.scale();
                                let mut value = if self.unit == "px" {
                                    controls.size[i] as f64
                                } else {
                                    self.dimensions[i]
                                };
                                ui.label(label);
                                let response = ui.add(
                                    egui::DragValue::new(&mut value)
                                        .range(1. / scale..=8192. / scale)
                                        .speed(1. / scale)
                                        .max_decimals(if self.unit == "px" { 0 } else { 3 }),
                                );
                                self.record(&format!("Render {label}"), &response);
                                if response.changed() {
                                    self.dimensions[i] = value;
                                    controls.size[i] =
                                        (value * scale).round().clamp(1., 8192.) as u32;
                                }
                            }
                        });
                        ui.label("Size unit");
                        let old_unit = self.unit.clone();
                        let old_scale = self.scale();
                        egui::ComboBox::from_id_salt("render-unit")
                            .selected_text(&self.unit)
                            .show_ui(ui, |ui| {
                                for unit in ["px", "in", "cm", "mm"] {
                                    ui.selectable_value(&mut self.unit, unit.into(), unit);
                                }
                            });
                        if self.unit != old_unit {
                            for i in 0..2 {
                                let pixels = if old_unit == "px" {
                                    controls.size[i] as f64
                                } else {
                                    self.dimensions[i] * old_scale
                                };
                                self.dimensions[i] = pixels / self.scale();
                            }
                        }
                        ui.horizontal(|ui| {
                            ui.label("DPI");
                            let response = ui.add(
                                egui::DragValue::new(&mut self.dpi)
                                    .range(1.0..=4800.0)
                                    .max_decimals(3),
                            );
                            if response.changed() && self.unit != "px" {
                                for (i, dimension) in controls.size.iter_mut().enumerate() {
                                    *dimension = (self.dimensions[i] * self.scale())
                                        .round()
                                        .clamp(1., 8192.)
                                        as u32;
                                }
                            }
                            self.record("Render DPI", &response);
                        });
                        ui.add_space(8.);
                        ui.strong("Output");
                        let response = ui.checkbox(controls.transparent, "Transparent background");
                        self.record("Render transparent", &response);
                    } else {
                        ui.strong("Quality");
                        let p = &mut controls.lighting;
                        let mut quality = if p.use_gi {
                            2
                        } else if p.ao_samples > 0 {
                            1
                        } else {
                            0
                        };
                        let before = quality;
                        egui::ComboBox::from_label("Lighting")
                            .selected_text(
                                ["Raytrace only", "Ambient occlusion", "Global illumination"]
                                    [quality],
                            )
                            .show_ui(ui, |ui| {
                                for (i, label) in
                                    ["Raytrace only", "Ambient occlusion", "Global illumination"]
                                        .iter()
                                        .enumerate()
                                {
                                    ui.selectable_value(&mut quality, i, *label);
                                }
                            });
                        if quality != before {
                            p.use_gi = quality == 2;
                            p.ao_samples = if quality == 1 { 64 } else { 0 };
                        }
                        ui.horizontal(|ui| {
                            ui.label("Supersampling");
                            ui.add(
                                egui::DragValue::new(&mut p.supersample)
                                    .range(1..=8)
                                    .suffix("×"),
                            );
                        });
                        ui.separator();
                        ui.strong("Camera");
                        egui::ComboBox::from_label("Projection")
                            .selected_text(if p.perspective {
                                "Perspective"
                            } else {
                                "Orthographic"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut p.perspective, false, "Orthographic");
                                ui.selectable_value(&mut p.perspective, true, "Perspective");
                            });
                        ui.checkbox(&mut p.use_clip_z, "Enable clip plane");
                        crate::lighting_controls(ui, p, *controls.hatching);
                        if *controls.hatching {
                            crate::hatch_controls(ui, controls.hatch);
                        }
                    }
                });
            });
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
