use cuemol_desktop_core::GuiPngExportOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Pixel,
    Millimeter,
    Centimeter,
    Inch,
}

impl Unit {
    pub const ALL: [Self; 4] = [Self::Pixel, Self::Millimeter, Self::Centimeter, Self::Inch];

    pub fn label(self) -> &'static str {
        match self {
            Self::Pixel => "pixel",
            Self::Millimeter => "mm",
            Self::Centimeter => "cm",
            Self::Inch => "inch",
        }
    }

    pub fn preference_value(self) -> &'static str {
        match self {
            Self::Pixel => "px",
            Self::Millimeter => "mm",
            Self::Centimeter => "cm",
            Self::Inch => "in",
        }
    }

    pub fn from_preference(value: &str) -> Option<Self> {
        match value.trim() {
            "px" => Some(Self::Pixel),
            "mm" => Some(Self::Millimeter),
            "cm" => Some(Self::Centimeter),
            "in" => Some(Self::Inch),
            _ => None,
        }
    }
}

pub fn png_pixels_to_unit(pixels: u32, unit: Unit, dpi: u32) -> f64 {
    let pixels = f64::from(pixels);
    let dpi = f64::from(dpi.max(1));
    match unit {
        Unit::Pixel => pixels,
        Unit::Millimeter => pixels / dpi * 25.4,
        Unit::Centimeter => pixels / dpi * 2.54,
        Unit::Inch => pixels / dpi,
    }
}

pub fn png_unit_to_pixels(value: f64, unit: Unit, dpi: u32) -> u32 {
    let dpi = f64::from(dpi.max(1));
    let pixels = match unit {
        Unit::Pixel => value,
        Unit::Millimeter => value / 25.4 * dpi,
        Unit::Centimeter => value / 2.54 * dpi,
        Unit::Inch => value * dpi,
    };
    if !pixels.is_finite() || pixels <= 0.0 {
        return 1;
    }
    pixels.round().clamp(1.0, f64::from(u32::MAX)) as u32
}

#[derive(Debug, Clone)]
pub struct ImageExportEditor {
    pub width: u32,
    pub height: u32,
    pub aspect: f64,
    pub resolution_dpi: u32,
    pub unit: Unit,
    pub retain_aspect: bool,
    pub transparent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Accept,
    Cancel,
}

impl ImageExportEditor {
    pub fn new(width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            width,
            height,
            aspect: f64::from(width) / f64::from(height),
            resolution_dpi: 150,
            unit: Unit::Pixel,
            retain_aspect: true,
            transparent: false,
        }
    }

    pub fn options(&self) -> GuiPngExportOptions {
        GuiPngExportOptions {
            width: self.width,
            height: self.height,
            resolution_dpi: f64::from(self.resolution_dpi),
            transparent: self.transparent,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, file_name: &str) -> Action {
        let mut open = true;
        let mut action = Action::None;
        egui::Window::new("PNG options")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.strong("Image size");
                egui::Grid::new("png_export_size")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Resolution (DPI):");
                        egui::ComboBox::from_id_salt("png_export_dpi")
                            .selected_text(self.resolution_dpi.to_string())
                            .show_ui(ui, |ui| {
                                for dpi in [72, 150, 300, 600] {
                                    ui.selectable_value(
                                        &mut self.resolution_dpi,
                                        dpi,
                                        dpi.to_string(),
                                    );
                                }
                            });
                        ui.end_row();

                        ui.label("Unit:");
                        egui::ComboBox::from_id_salt("png_export_unit")
                            .selected_text(self.unit.label())
                            .show_ui(ui, |ui| {
                                for unit in Unit::ALL {
                                    ui.selectable_value(&mut self.unit, unit, unit.label());
                                }
                            });
                        ui.end_row();

                        ui.label("Width:");
                        let mut width =
                            png_pixels_to_unit(self.width, self.unit, self.resolution_dpi);
                        if ui
                            .add(
                                egui::DragValue::new(&mut width)
                                    .range(0.01..=100_000.0)
                                    .speed(if self.unit == Unit::Pixel { 1.0 } else { 0.1 }),
                            )
                            .changed()
                        {
                            self.width =
                                png_unit_to_pixels(width, self.unit, self.resolution_dpi).max(1);
                            if self.retain_aspect {
                                self.height =
                                    (f64::from(self.width) / self.aspect).round().max(1.0) as u32;
                            }
                        }
                        ui.end_row();

                        ui.label("Height:");
                        let mut height =
                            png_pixels_to_unit(self.height, self.unit, self.resolution_dpi);
                        if ui
                            .add_enabled(
                                !self.retain_aspect,
                                egui::DragValue::new(&mut height)
                                    .range(0.01..=100_000.0)
                                    .speed(if self.unit == Unit::Pixel { 1.0 } else { 0.1 }),
                            )
                            .changed()
                        {
                            self.height =
                                png_unit_to_pixels(height, self.unit, self.resolution_dpi).max(1);
                        }
                        ui.end_row();

                        ui.label("Retain aspect ratio:");
                        if ui.checkbox(&mut self.retain_aspect, "").changed() && self.retain_aspect
                        {
                            self.height =
                                (f64::from(self.width) / self.aspect).round().max(1.0) as u32;
                        }
                        ui.end_row();
                    });
                ui.separator();
                ui.strong("Output");
                ui.label(format!("Pixel size: {} x {}", self.width, self.height));
                ui.checkbox(&mut self.transparent, "Transparent PNG");
                ui.weak(file_name);
                ui.separator();
                let error = self.options().validate().err();
                if let Some(error) = &error {
                    ui.colored_label(ui.visuals().error_fg_color, error.to_string());
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(error.is_none(), egui::Button::new("OK"))
                        .clicked()
                    {
                        action = Action::Accept;
                    }
                    if ui.button("Cancel").clicked() {
                        action = Action::Cancel;
                    }
                });
            });
        if !open {
            Action::Cancel
        } else {
            action
        }
    }
}
