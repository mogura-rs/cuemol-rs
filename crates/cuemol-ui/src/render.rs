use base64::Engine;
use cuemol_render::{HatchRenderParams, UmbreonRenderParams};
use egui::{Context, Response, TextureHandle};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Default, PartialEq, Eq)]
pub enum RenderAction {
    #[default]
    None,
    Start,
    Stop,
    Save,
}

pub struct RenderControls<'a> {
    pub size: &'a mut [u32; 2],
    pub transparent: &'a mut bool,
    pub hatching: &'a mut bool,
    pub lighting: &'a mut UmbreonRenderParams,
    pub hatch: &'a mut HatchRenderParams,
}

pub struct RenderPanel {
    tab: usize,
    dpi: u32,
    unit: String,
    dimensions: [f64; 2],
    preview: Option<TextureHandle>,
    pub png: Option<Vec<u8>>,
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
            dpi: 600,
            unit: "px".into(),
            dimensions: [1200.; 2],
            preview: None,
            png: None,
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
            .and_then(Value::as_u64)
            .unwrap_or(600)
            .clamp(1, 4800) as u32;
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
            "in" => self.dpi as f64,
            "cm" => self.dpi as f64 / 2.54,
            "mm" => self.dpi as f64 / 25.4,
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

    pub fn begin(&mut self, ctx: &Context) {
        self.started = Some(ctx.input(|i| i.time));
        self.message = "Rendering image…".into();
    }

    pub fn result(&mut self, ctx: &Context, value: &Value) -> Result<(), String> {
        let png = base64::engine::general_purpose::STANDARD
            .decode(value["_image"].as_str().ok_or("Render returned no image")?)
            .map_err(|e| e.to_string())?;
        let image = image::load_from_memory(&png)
            .map_err(|e| e.to_string())?
            .into_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        self.preview =
            Some(ctx.load_texture("render-preview", image, egui::TextureOptions::LINEAR));
        self.png = Some(png);
        let elapsed = self
            .started
            .take()
            .map(|start| ctx.input(|i| i.time) - start);
        self.message = format!(
            "Rendered {} × {}{}",
            size[0],
            size[1],
            elapsed.map(|s| format!(" in {s:.1} s")).unwrap_or_default()
        );
        Ok(())
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
                                self.preview(ui, (height - 128.).max(160.));
                                action = self.actions(ui, running, controls.hatching);
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
                        self.preview(ui, 180.);
                        action = self.actions(ui, running, controls.hatching);
                        self.settings(ui, running, &mut controls);
                    });
                }
            });
        action
    }

    fn preview(&mut self, ui: &mut egui::Ui, height: f32) {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::hover(),
        );
        self.record("Render preview", &response);
        ui.painter().rect_filled(rect, 0., ui.visuals().panel_fill);
        if let Some(texture) = &self.preview {
            let size = texture.size_vec2();
            let scale = (rect.width() / size.x).min(rect.height() / size.y).min(1.);
            let image_rect = egui::Rect::from_center_size(rect.center(), size * scale);
            for y in 0..(image_rect.height() / 12.).ceil() as u32 {
                for x in 0..(image_rect.width() / 12.).ceil() as u32 {
                    let cell = egui::Rect::from_min_size(
                        image_rect.min + egui::vec2(x as f32 * 12., y as f32 * 12.),
                        egui::vec2(12., 12.),
                    )
                    .intersect(image_rect);
                    ui.painter().rect_filled(
                        cell,
                        0.,
                        egui::Color32::from_gray(if (x + y) % 2 == 0 { 48 } else { 64 }),
                    );
                }
            }
            ui.painter().image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                egui::Color32::WHITE,
            );
        } else {
            let text = ui.painter().layout(
                "No render result yet.\nPress Start Render to render the current view.".into(),
                egui::FontId::proportional(13.),
                ui.visuals().weak_text_color(),
                (rect.width() - 40.).max(80.),
            );
            ui.painter().galley(
                rect.center() - text.size() * 0.5,
                text,
                ui.visuals().weak_text_color(),
            );
        }
    }

    fn actions(&mut self, ui: &mut egui::Ui, running: bool, hatching: &mut bool) -> RenderAction {
        let mut action = RenderAction::None;
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            let response = ui.add_enabled(!running, egui::Button::new("Start Render"));
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
            let response = ui.add_enabled(self.png.is_some(), egui::Button::new("Save PNG…"));
            self.record("Save render", &response);
            if response.clicked() {
                action = RenderAction::Save;
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
        ui.label(&self.message);
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
                                    ("640 × 480 (72 dpi)", [640, 480], 72),
                                    ("1200 × 1200 (600 dpi)", [1200, 1200], 600),
                                    ("1920 × 1080 (300 dpi)", [1920, 1080], 300),
                                    ("2400 × 2400 (600 dpi)", [2400, 2400], 600),
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
                            let response =
                                ui.add(egui::DragValue::new(&mut self.dpi).range(1..=4800));
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
