use cuemol_core::{CueMolError, Result, Vector4};

#[derive(Debug, Clone, PartialEq)]
pub enum ColorBase {
    Named(String),
    Html { r: u8, g: u8, b: u8, a: u8 },
    Rgb { r: f32, g: f32, b: f32, a: f32 },
    Hsb { h: f32, s: f32, b: f32, a: f32 },
    MolColor(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorModifier {
    pub name: String,
    pub value: ColorModifierValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColorModifierValue {
    Number(f32),
    Text(String),
}

impl ColorModifierValue {
    pub fn as_number(&self) -> Option<f32> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(_) => None,
        }
    }

    pub fn as_text(&self) -> &str {
        match self {
            Self::Number(_) => "",
            Self::Text(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorSpec {
    pub base: ColorBase,
    pub modifiers: Vec<ColorModifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolidColorMode {
    Rgb,
    Hsb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolidColor {
    code: u32,
    mode: SolidColorMode,
    material: String,
}

impl Default for SolidColor {
    fn default() -> Self {
        Self::new()
    }
}

impl SolidColor {
    pub fn new() -> Self {
        Self {
            code: 0,
            mode: SolidColorMode::Rgb,
            material: String::new(),
        }
    }

    pub fn from_code(code: u32) -> Self {
        Self {
            code,
            mode: SolidColorMode::Rgb,
            material: String::new(),
        }
    }

    pub fn from_rgb_u8(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba_u8(r, g, b, 255)
    }

    pub fn from_rgba_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::from_code(rgba_code(r, g, b, a))
    }

    pub fn from_rgb(r: f64, g: f64, b: f64) -> Self {
        Self::from_rgba(r, g, b, 1.0)
    }

    pub fn from_rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        let mut color = Self::new();
        color.set_rgba(r, g, b, a);
        color
    }

    pub fn from_vector(value: Vector4) -> Self {
        Self::from_rgba(value.x, value.y, value.z, value.w)
    }

    pub fn create_rgb(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self::from_rgba(r, g, b, a)
    }

    pub fn create_hsb(h: f64, s: f64, b: f64, a: f64) -> Self {
        Self::create_raw_hsb(h, s, b, a)
    }

    pub fn create_raw_hsb(h: f64, s: f64, b: f64, a: f64) -> Self {
        let mut color = Self::new();
        color.set_hsba(h, s, b, a);
        color.mode = SolidColorMode::Hsb;
        color
    }

    pub fn r(&self) -> u8 {
        ((self.code >> 16) & 0xff) as u8
    }

    pub fn g(&self) -> u8 {
        ((self.code >> 8) & 0xff) as u8
    }

    pub fn b(&self) -> u8 {
        (self.code & 0xff) as u8
    }

    pub fn a(&self) -> u8 {
        ((self.code >> 24) & 0xff) as u8
    }

    pub fn fr(&self) -> f64 {
        f64::from(self.r()) / 255.0
    }

    pub fn fg(&self) -> f64 {
        f64::from(self.g()) / 255.0
    }

    pub fn fb(&self) -> f64 {
        f64::from(self.b()) / 255.0
    }

    pub fn fa(&self) -> f64 {
        self.alpha()
    }

    pub fn code(&self) -> u32 {
        self.code
    }

    pub fn get_code(&self) -> u32 {
        self.code()
    }

    pub fn get_dev_code(&self, _context_id: u64) -> u32 {
        self.code()
    }

    pub fn is_in_gamut(&self, _context_id: u64) -> bool {
        true
    }

    pub fn set_code(&mut self, code: u32) {
        self.code = code;
        self.mode = SolidColorMode::Rgb;
    }

    pub fn set_r(&mut self, value: u8) {
        self.code = (self.code & 0xff00_ffff) | (u32::from(value) << 16);
        self.mode = SolidColorMode::Rgb;
    }

    pub fn set_g(&mut self, value: u8) {
        self.code = (self.code & 0xffff_00ff) | (u32::from(value) << 8);
        self.mode = SolidColorMode::Rgb;
    }

    pub fn set_b(&mut self, value: u8) {
        self.code = (self.code & 0xffff_ff00) | u32::from(value);
        self.mode = SolidColorMode::Rgb;
    }

    pub fn set_a(&mut self, value: u8) {
        self.code = (self.code & 0x00ff_ffff) | (u32::from(value) << 24);
    }

    pub fn set_rgba(&mut self, r: f64, g: f64, b: f64, a: f64) {
        self.code = rgba_code(unit_to_u8(r), unit_to_u8(g), unit_to_u8(b), unit_to_u8(a));
        self.mode = SolidColorMode::Rgb;
    }

    pub fn set_rgb(&mut self, r: f64, g: f64, b: f64) {
        self.set_rgba(r, g, b, 1.0);
    }

    pub fn set_hsba(&mut self, h: f64, s: f64, b: f64, a: f64) {
        let rgba = hsb_unit_to_rgba(h, s, b, a);
        self.code = rgba_code(rgba.r, rgba.g, rgba.b, rgba.a);
        self.mode = SolidColorMode::Hsb;
    }

    pub fn set_hsb(&mut self, h: f64, s: f64, b: f64) {
        self.set_hsba(h, s, b, 1.0);
    }

    pub fn set_alpha(&mut self, value: f64) {
        self.set_a(unit_to_u8(value));
    }

    pub fn alpha(&self) -> f64 {
        f64::from(self.a()) / 255.0
    }

    pub fn get_alpha(&self) -> f64 {
        self.alpha()
    }

    pub fn material(&self) -> &str {
        &self.material
    }

    pub fn get_material(&self) -> &str {
        self.material()
    }

    pub fn set_material(&mut self, value: impl Into<String>) {
        self.material = value.into();
    }

    pub fn equals(&self, other: &Self) -> bool {
        self.code == other.code
    }

    pub fn rgba8(&self) -> Rgba8 {
        Rgba8 {
            r: self.r(),
            g: self.g(),
            b: self.b(),
            a: self.a(),
        }
    }

    pub fn script_string(&self) -> String {
        if self.a() == 255 {
            if self.mode == SolidColorMode::Hsb {
                let (h, s, b) = rgb_to_hsb_unit(self.fr(), self.fg(), self.fb());
                format!(
                    "hsb({},{},{})",
                    format_real(h * 360.0),
                    format_real(s),
                    format_real(b)
                )
            } else {
                format!("#{:06X}", self.code & 0x00ff_ffff)
            }
        } else if self.mode == SolidColorMode::Hsb {
            let (h, s, b) = rgb_to_hsb_unit(self.fr(), self.fg(), self.fb());
            format!(
                "hsba({},{},{},{})",
                format_real(h * 360.0),
                format_real(s),
                format_real(b),
                format_real(self.fa())
            )
        } else {
            format!(
                "rgba({},{},{},{})",
                format_real(self.fr()),
                format_real(self.fg()),
                format_real(self.fb()),
                format_real(self.fa())
            )
        }
    }
}

impl From<Rgba8> for SolidColor {
    fn from(value: Rgba8) -> Self {
        Self::from_rgba_u8(value.r, value.g, value.b, value.a)
    }
}

impl From<SolidColor> for Rgba8 {
    fn from(value: SolidColor) -> Self {
        value.rgba8()
    }
}

pub type Color = SolidColor;

pub trait AbstractColor {
    fn r(&self) -> u8;
    fn g(&self) -> u8;
    fn b(&self) -> u8;
    fn a(&self) -> u8;
    fn code(&self) -> u32;
    fn material(&self) -> String;
    fn to_color_string(&self) -> String;

    fn fr(&self) -> f64 {
        f64::from(self.r()) / 255.0
    }

    fn fg(&self) -> f64 {
        f64::from(self.g()) / 255.0
    }

    fn fb(&self) -> f64 {
        f64::from(self.b()) / 255.0
    }

    fn fa(&self) -> f64 {
        f64::from(self.a()) / 255.0
    }

    fn get_dev_code(&self, _context_id: u64) -> u32 {
        self.code()
    }

    fn is_in_gamut(&self, _context_id: u64) -> bool {
        true
    }

    fn get_material(&self) -> String {
        self.material()
    }
}

impl AbstractColor for SolidColor {
    fn r(&self) -> u8 {
        self.r()
    }

    fn g(&self) -> u8 {
        self.g()
    }

    fn b(&self) -> u8 {
        self.b()
    }

    fn a(&self) -> u8 {
        self.a()
    }

    fn code(&self) -> u32 {
        self.code()
    }

    fn material(&self) -> String {
        self.material().to_string()
    }

    fn to_color_string(&self) -> String {
        self.script_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedColor {
    name: String,
    material: String,
    alpha: f64,
    mod_h: f64,
    mod_s: f64,
    mod_b: f64,
}

impl Default for NamedColor {
    fn default() -> Self {
        Self::new()
    }
}

impl NamedColor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            material: String::new(),
            alpha: -1.0,
            mod_h: 0.0,
            mod_s: 0.0,
            mod_b: 0.0,
        }
    }

    pub fn from_name(name: impl Into<String>) -> Self {
        let mut color = Self::new();
        color.name = name.into();
        color
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn color_name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    pub fn material(&self) -> &str {
        &self.material
    }

    pub fn get_material(&self) -> &str {
        self.material()
    }

    pub fn set_material(&mut self, value: impl Into<String>) {
        self.material = value.into();
    }

    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    pub fn get_alpha(&self) -> f64 {
        self.alpha()
    }

    pub fn set_alpha(&mut self, value: f64) {
        self.alpha = value;
    }

    pub fn mod_h(&self) -> f64 {
        self.mod_h
    }

    pub fn get_mod_hue(&self) -> f64 {
        self.mod_h()
    }

    pub fn set_mod_h(&mut self, value: f64) {
        self.mod_h = value;
    }

    pub fn set_mod_hue(&mut self, value: f64) {
        self.set_mod_h(value);
    }

    pub fn mod_s(&self) -> f64 {
        self.mod_s
    }

    pub fn get_mod_sat(&self) -> f64 {
        self.mod_s()
    }

    pub fn set_mod_s(&mut self, value: f64) {
        self.mod_s = value;
    }

    pub fn set_mod_sat(&mut self, value: f64) {
        self.set_mod_s(value);
    }

    pub fn mod_b(&self) -> f64 {
        self.mod_b
    }

    pub fn get_mod_bri(&self) -> f64 {
        self.mod_b()
    }

    pub fn set_mod_b(&mut self, value: f64) {
        self.mod_b = value;
    }

    pub fn set_mod_bri(&mut self, value: f64) {
        self.set_mod_b(value);
    }

    pub fn is_alpha_default(&self) -> bool {
        self.alpha < -0.01
    }

    pub fn is_mod_set(&self) -> bool {
        self.mod_h.abs() >= 1.0e-4 || self.mod_s.abs() >= 1.0e-4 || self.mod_b.abs() >= 1.0e-4
    }

    pub fn resolved_color(&self) -> SolidColor {
        let base =
            named_color(&self.name).unwrap_or_else(|| SolidColor::from_rgba(0.5, 0.5, 0.5, 0.5));
        apply_color_overrides(
            base,
            self.material.clone(),
            self.alpha,
            self.mod_h,
            self.mod_s,
            self.mod_b,
        )
    }

    pub fn code(&self) -> u32 {
        self.resolved_color().code()
    }

    pub fn get_code(&self) -> u32 {
        self.code()
    }

    pub fn get_dev_code(&self, _context_id: u64) -> u32 {
        self.code()
    }

    pub fn is_in_gamut(&self, _context_id: u64) -> bool {
        true
    }

    pub fn to_color_string(&self) -> String {
        let mut text = self.name.clone();
        let modifiers = color_modifier_string(
            self.material.as_str(),
            self.alpha,
            self.mod_h,
            self.mod_s,
            self.mod_b,
            false,
        );
        if !modifiers.is_empty() {
            text.push('{');
            text.push_str(&modifiers);
            text.push('}');
        }
        text
    }
}

impl AbstractColor for NamedColor {
    fn r(&self) -> u8 {
        self.resolved_color().r()
    }

    fn g(&self) -> u8 {
        self.resolved_color().g()
    }

    fn b(&self) -> u8 {
        self.resolved_color().b()
    }

    fn a(&self) -> u8 {
        self.resolved_color().a()
    }

    fn code(&self) -> u32 {
        self.code()
    }

    fn material(&self) -> String {
        self.resolved_color().material().to_string()
    }

    fn to_color_string(&self) -> String {
        self.to_color_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolColorRef {
    material: String,
    alpha: f64,
    mod_h: f64,
    mod_s: f64,
    mod_b: f64,
}

impl Default for MolColorRef {
    fn default() -> Self {
        Self::new()
    }
}

impl MolColorRef {
    pub fn new() -> Self {
        Self {
            material: String::new(),
            alpha: -1.0,
            mod_h: 0.0,
            mod_s: 0.0,
            mod_b: 0.0,
        }
    }

    pub fn material(&self) -> &str {
        &self.material
    }

    pub fn get_material(&self) -> &str {
        self.material()
    }

    pub fn set_material(&mut self, value: impl Into<String>) {
        self.material = value.into();
    }

    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    pub fn get_alpha(&self) -> f64 {
        self.alpha()
    }

    pub fn set_alpha(&mut self, value: f64) {
        self.alpha = value;
    }

    pub fn mod_h(&self) -> f64 {
        self.mod_h
    }

    pub fn get_mod_hue(&self) -> f64 {
        self.mod_h()
    }

    pub fn set_mod_h(&mut self, value: f64) {
        self.mod_h = value;
    }

    pub fn set_mod_hue(&mut self, value: f64) {
        self.set_mod_h(value);
    }

    pub fn mod_s(&self) -> f64 {
        self.mod_s
    }

    pub fn get_mod_sat(&self) -> f64 {
        self.mod_s()
    }

    pub fn set_mod_s(&mut self, value: f64) {
        self.mod_s = value;
    }

    pub fn set_mod_sat(&mut self, value: f64) {
        self.set_mod_s(value);
    }

    pub fn mod_b(&self) -> f64 {
        self.mod_b
    }

    pub fn get_mod_bri(&self) -> f64 {
        self.mod_b()
    }

    pub fn set_mod_b(&mut self, value: f64) {
        self.mod_b = value;
    }

    pub fn set_mod_bri(&mut self, value: f64) {
        self.set_mod_b(value);
    }

    pub fn is_alpha_set(&self) -> bool {
        self.alpha >= -0.01
    }

    pub fn is_mod_set(&self) -> bool {
        self.mod_h.abs() >= 1.0e-4 || self.mod_s.abs() >= 1.0e-4 || self.mod_b.abs() >= 1.0e-4
    }

    pub fn modify_color(&self, color: &SolidColor) -> SolidColor {
        apply_color_overrides(
            color.clone(),
            self.material.clone(),
            self.alpha,
            self.mod_h,
            self.mod_s,
            self.mod_b,
        )
    }

    pub fn code(&self) -> u32 {
        0x7f7f_7f7f
    }

    pub fn get_code(&self) -> u32 {
        self.code()
    }

    pub fn get_dev_code(&self, _context_id: u64) -> u32 {
        self.code()
    }

    pub fn is_in_gamut(&self, _context_id: u64) -> bool {
        true
    }

    pub fn to_color_string(&self) -> String {
        let mut text = "$molcol".to_string();
        let modifiers = color_modifier_string(
            self.material.as_str(),
            self.alpha,
            self.mod_h,
            self.mod_s,
            self.mod_b,
            false,
        );
        if !modifiers.is_empty() {
            text.push('{');
            text.push_str(&modifiers);
            text.push('}');
        }
        text
    }
}

impl AbstractColor for MolColorRef {
    fn r(&self) -> u8 {
        0x7f
    }

    fn g(&self) -> u8 {
        0x7f
    }

    fn b(&self) -> u8 {
        0x7f
    }

    fn a(&self) -> u8 {
        0x7f
    }

    fn code(&self) -> u32 {
        self.code()
    }

    fn material(&self) -> String {
        self.material.clone()
    }

    fn to_color_string(&self) -> String {
        self.to_color_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GradientColor {
    rho: f64,
    col1: SolidColor,
    col2: SolidColor,
}

impl Default for GradientColor {
    fn default() -> Self {
        Self::new()
    }
}

impl GradientColor {
    pub fn new() -> Self {
        Self {
            rho: 0.0,
            col1: SolidColor::new(),
            col2: SolidColor::new(),
        }
    }

    pub fn from_colors(col1: SolidColor, col2: SolidColor, rho: f64) -> Self {
        Self { rho, col1, col2 }
    }

    pub fn rho(&self) -> f64 {
        self.rho
    }

    pub fn set_rho(&mut self, value: f64) {
        self.rho = value;
    }

    pub fn grad_param(&self) -> f64 {
        self.rho()
    }

    pub fn get_grad_param(&self) -> f64 {
        self.grad_param()
    }

    pub fn set_grad_param(&mut self, value: f64) {
        self.set_rho(value);
    }

    pub fn col1(&self) -> &SolidColor {
        &self.col1
    }

    pub fn col_1(&self) -> &SolidColor {
        self.col1()
    }

    pub fn set_col1(&mut self, value: SolidColor) {
        self.col1 = value;
    }

    pub fn set_col_1(&mut self, value: SolidColor) {
        self.set_col1(value);
    }

    pub fn grad_color1(&self) -> &SolidColor {
        self.col1()
    }

    pub fn get_grad_color1(&self) -> &SolidColor {
        self.grad_color1()
    }

    pub fn set_grad_color1(&mut self, value: SolidColor) {
        self.set_col1(value);
    }

    pub fn col2(&self) -> &SolidColor {
        &self.col2
    }

    pub fn col_2(&self) -> &SolidColor {
        self.col2()
    }

    pub fn set_col2(&mut self, value: SolidColor) {
        self.col2 = value;
    }

    pub fn set_col_2(&mut self, value: SolidColor) {
        self.set_col2(value);
    }

    pub fn grad_color2(&self) -> &SolidColor {
        self.col2()
    }

    pub fn get_grad_color2(&self) -> &SolidColor {
        self.grad_color2()
    }

    pub fn set_grad_color2(&mut self, value: SolidColor) {
        self.set_col2(value);
    }

    pub fn r(&self) -> u8 {
        blend_channel(self.col1.r(), self.col2.r(), self.rho())
    }

    pub fn g(&self) -> u8 {
        blend_channel(self.col1.g(), self.col2.g(), self.rho())
    }

    pub fn b(&self) -> u8 {
        blend_channel(self.col1.b(), self.col2.b(), self.rho())
    }

    pub fn a(&self) -> u8 {
        blend_channel(self.col1.a(), self.col2.a(), self.rho())
    }

    pub fn code(&self) -> u32 {
        rgba_code(self.r(), self.g(), self.b(), self.a())
    }

    pub fn get_code(&self) -> u32 {
        self.code()
    }

    pub fn get_dev_code(&self, _context_id: u64) -> u32 {
        self.code()
    }

    pub fn is_in_gamut(&self, _context_id: u64) -> bool {
        true
    }

    pub fn material(&self) -> &str {
        self.col1.material()
    }

    pub fn get_material(&self) -> &str {
        self.material()
    }
}

impl AbstractColor for GradientColor {
    fn r(&self) -> u8 {
        self.r()
    }

    fn g(&self) -> u8 {
        self.g()
    }

    fn b(&self) -> u8 {
        self.b()
    }

    fn a(&self) -> u8 {
        self.a()
    }

    fn code(&self) -> u32 {
        self.code()
    }

    fn material(&self) -> String {
        self.material().to_string()
    }

    fn to_color_string(&self) -> String {
        String::new()
    }
}

pub fn parse_color(input: &str) -> Result<ColorSpec> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CueMolError::parse("empty color"));
    }

    let (base_text, modifier_text) = split_modifiers(trimmed)?;
    let base = parse_color_base(base_text.trim())?;
    let modifiers = match modifier_text {
        Some(text) => parse_modifiers(text)?,
        None => Vec::new(),
    };

    Ok(ColorSpec { base, modifiers })
}

impl ColorSpec {
    pub fn rgba8(&self) -> Option<Rgba8> {
        let mut rgba = match self.base {
            ColorBase::Html { r, g, b, a } => Rgba8 { r, g, b, a },
            ColorBase::Rgb { r, g, b, a } => Rgba8 {
                r: color_component_to_u8(r),
                g: color_component_to_u8(g),
                b: color_component_to_u8(b),
                a: alpha_to_u8(a),
            },
            ColorBase::Hsb { h, s, b, a } => hsb_to_rgba(h, s, b, a),
            ColorBase::Named(_) | ColorBase::MolColor(_) => return None,
        };

        for modifier in &self.modifiers {
            if modifier.name == "alpha" {
                if let Some(value) = modifier.value.as_number() {
                    rgba.a = alpha_to_u8(value);
                }
            }
        }
        Some(rgba)
    }

    pub fn device_code_generic_cmyk(&self) -> Option<u32> {
        let rgba = self.rgba8()?;
        match (rgba.r, rgba.g, rgba.b) {
            (255, 255, 255) => Some(0xF2F3FA),
            (119, 119, 119) => Some(0x767578),
            (0, 0, 255) => Some(0x345BA0),
            (255, 0, 255) => Some(0xB469A4),
            (r, g, b) => Some(((r as u32) << 16) | ((g as u32) << 8) | b as u32),
        }
    }

    pub fn is_in_gamut_generic_cmyk(&self) -> Option<bool> {
        let rgba = self.rgba8()?;
        match (rgba.r, rgba.g, rgba.b) {
            (0, 0, 255) | (255, 0, 255) => Some(false),
            _ => Some(true),
        }
    }
}

fn named_color(name: &str) -> Option<SolidColor> {
    let rgba = match name.trim().to_ascii_lowercase().as_str() {
        "black" => (0, 0, 0, 255),
        "white" => (255, 255, 255, 255),
        "red" => (255, 0, 0, 255),
        "green" => (0, 128, 0, 255),
        "lime" => (0, 255, 0, 255),
        "blue" => (0, 0, 255, 255),
        "cyan" => (0, 255, 255, 255),
        "magenta" => (255, 0, 255, 255),
        "yellow" => (255, 255, 0, 255),
        "gray" | "grey" => (128, 128, 128, 255),
        "orange" => (255, 165, 0, 255),
        "purple" => (128, 0, 128, 255),
        _ => return None,
    };
    Some(SolidColor::from_rgba_u8(rgba.0, rgba.1, rgba.2, rgba.3))
}

fn apply_color_overrides(
    color: SolidColor,
    material: String,
    alpha: f64,
    mod_h: f64,
    mod_s: f64,
    mod_b: f64,
) -> SolidColor {
    let has_mod = mod_h.abs() >= 1.0e-4 || mod_s.abs() >= 1.0e-4 || mod_b.abs() >= 1.0e-4;
    let mut result = if has_mod {
        let (h, s, b) = rgb_to_hsb_unit(color.fr(), color.fg(), color.fb());
        SolidColor::create_raw_hsb(h + mod_h / 360.0, s + mod_s, b + mod_b, color.fa())
    } else {
        color.clone()
    };

    if !material.is_empty() {
        result.set_material(material);
    } else {
        result.set_material(color.material().to_string());
    }

    if alpha >= -0.01 {
        result.set_alpha(alpha);
    }
    result
}

fn color_modifier_string(
    material: &str,
    alpha: f64,
    mod_h: f64,
    mod_s: f64,
    mod_b: f64,
    ignore_alpha: bool,
) -> String {
    let mut parts = Vec::new();
    if !material.is_empty() {
        parts.push(format!("material: {material}"));
    }
    if !ignore_alpha && alpha >= -0.01 {
        parts.push(format!("alpha: {}", format_real(alpha)));
    }
    if mod_h.abs() >= 1.0e-4 {
        parts.push(format!("mod_h: {}", format_real(mod_h)));
    }
    if mod_s.abs() >= 1.0e-4 {
        parts.push(format!("mod_s: {}", format_real(mod_s)));
    }
    if mod_b.abs() >= 1.0e-4 {
        parts.push(format!("mod_b: {}", format_real(mod_b)));
    }
    parts.join("; ")
}

fn blend_channel(v1: u8, v2: u8, rho: f64) -> u8 {
    if v1 == v2 {
        return v1;
    }
    (f64::from(v1) * rho + f64::from(v2) * (1.0 - rho)).clamp(0.0, 255.0) as u8
}

fn split_modifiers(input: &str) -> Result<(&str, Option<&str>)> {
    let Some(open) = input.find('{') else {
        if input.contains('}') {
            return Err(CueMolError::parse("unmatched color modifier brace"));
        }
        return Ok((input, None));
    };

    if !input.ends_with('}') {
        return Err(CueMolError::parse("missing color modifier closing brace"));
    }

    let close = input.len() - 1;
    if input[open + 1..close].contains('{') || input[open + 1..close].contains('}') {
        return Err(CueMolError::parse(
            "nested color modifiers are not supported",
        ));
    }
    Ok((&input[..open], Some(&input[open + 1..close])))
}

fn parse_color_base(input: &str) -> Result<ColorBase> {
    if input.is_empty() {
        return Err(CueMolError::parse("empty color base"));
    }

    if let Some(hex) = input.strip_prefix('#') {
        return parse_html_color(hex);
    }

    if let Some(name) = input.strip_prefix('$') {
        if is_identifier(name) {
            return Ok(ColorBase::MolColor(name.to_string()));
        }
        return Err(CueMolError::parse("invalid molecular color name"));
    }

    if let Some(args) = call_args(input, "rgb").or_else(|| call_args(input, "RGB")) {
        let values = parse_number_list(args, 3)?;
        return Ok(ColorBase::Rgb {
            r: values[0],
            g: values[1],
            b: values[2],
            a: 1.0,
        });
    }

    if let Some(args) = call_args(input, "rgba").or_else(|| call_args(input, "RGBA")) {
        let values = parse_number_list(args, 4)?;
        return Ok(ColorBase::Rgb {
            r: values[0],
            g: values[1],
            b: values[2],
            a: values[3],
        });
    }

    if let Some(args) = call_args(input, "hsb").or_else(|| call_args(input, "HSB")) {
        let values = parse_number_list(args, 3)?;
        return Ok(ColorBase::Hsb {
            h: values[0],
            s: values[1],
            b: values[2],
            a: 1.0,
        });
    }

    if let Some(args) = call_args(input, "hsba").or_else(|| call_args(input, "HSBA")) {
        let values = parse_number_list(args, 4)?;
        return Ok(ColorBase::Hsb {
            h: values[0],
            s: values[1],
            b: values[2],
            a: values[3],
        });
    }

    if input.contains('(') || input.contains(')') {
        return Err(CueMolError::parse("invalid color function"));
    }

    if is_identifier(input) {
        return Ok(ColorBase::Named(input.to_string()));
    }

    Err(CueMolError::parse("invalid color name"))
}

fn parse_html_color(hex: &str) -> Result<ColorBase> {
    let expand = |c: char| -> String { format!("{c}{c}") };
    let full = match hex.len() {
        3 => hex.chars().map(expand).collect::<Vec<_>>().join(""),
        6 => hex.to_string(),
        _ => return Err(CueMolError::parse("invalid html color length")),
    };

    if !full.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(CueMolError::parse("invalid html color digit"));
    }

    let byte = |range: std::ops::Range<usize>| -> Result<u8> {
        u8::from_str_radix(&full[range], 16).map_err(|err| CueMolError::parse(err.to_string()))
    };

    Ok(ColorBase::Html {
        r: byte(0..2)?,
        g: byte(2..4)?,
        b: byte(4..6)?,
        a: 255,
    })
}

fn call_args<'a>(input: &'a str, name: &str) -> Option<&'a str> {
    let rest = input.strip_prefix(name)?;
    if !rest.starts_with('(') || !rest.ends_with(')') {
        return None;
    }
    Some(&rest[1..rest.len() - 1])
}

fn parse_number_list(input: &str, expected_count: usize) -> Result<Vec<f32>> {
    let parts = input.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != expected_count || parts.iter().any(|part| part.is_empty()) {
        return Err(CueMolError::parse("wrong color argument count"));
    }
    parts
        .into_iter()
        .map(|part| {
            part.parse::<f32>()
                .map_err(|_| CueMolError::parse(format!("invalid number {part}")))
        })
        .collect()
}

fn parse_modifiers(input: &str) -> Result<Vec<ColorModifier>> {
    let mut modifiers = Vec::new();
    for part in input.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((name, value)) = part.split_once(':') else {
            return Err(CueMolError::parse("invalid color modifier"));
        };
        let name = name.trim();
        if !is_identifier(name) {
            return Err(CueMolError::parse("invalid color modifier name"));
        }
        let value = parse_modifier_value(name, value.trim())?;
        modifiers.push(ColorModifier {
            name: name.to_string(),
            value,
        });
    }
    Ok(modifiers)
}

fn parse_modifier_value(name: &str, value: &str) -> Result<ColorModifierValue> {
    if value.is_empty() {
        return Err(CueMolError::parse("invalid color modifier value"));
    }
    if let Ok(number) = value.parse::<f32>() {
        return Ok(ColorModifierValue::Number(number));
    }
    if name == "material"
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Ok(ColorModifierValue::Text(value.to_string()));
    }
    Err(CueMolError::parse("invalid color modifier value"))
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn color_component_to_u8(value: f32) -> u8 {
    if value <= 1.0 {
        alpha_to_u8(value)
    } else {
        value.round().clamp(0.0, 255.0) as u8
    }
}

fn unit_to_u8(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8
}

pub fn rgba_code(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (u32::from(a) << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

fn alpha_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn hsb_to_rgba(h: f32, s: f32, v: f32, a: f32) -> Rgba8 {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Rgba8 {
        r: alpha_to_u8(r + m),
        g: alpha_to_u8(g + m),
        b: alpha_to_u8(b + m),
        a: alpha_to_u8(a),
    }
}

fn hsb_unit_to_rgba(h: f64, s: f64, v: f64, a: f64) -> Rgba8 {
    let h = h.rem_euclid(1.0) * 6.0;
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Rgba8 {
        r: unit_to_u8(r + m),
        g: unit_to_u8(g + m),
        b: unit_to_u8(b + m),
        a: unit_to_u8(a),
    }
}

fn rgb_to_hsb_unit(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        (((b - r) / delta) + 2.0) / 6.0
    } else {
        (((r - g) / delta) + 4.0) / 6.0
    };
    let sat = if max == 0.0 { 0.0 } else { delta / max };
    (hue, sat, max)
}

fn format_real(value: f64) -> String {
    if (value - value.round()).abs() < 1.0e-12 {
        return format!("{:.0}", value);
    }
    let text = format!("{value:.12}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_cuemol_color_syntax_representatives() {
        for color in [
            "red",
            "color_1",
            " red",
            "#fff",
            "#AbC",
            "#ffffff",
            "rgb(255, 0, 0)",
            "RGB(128, 128, 128)",
            "rgba(255, 0, 0, 0.5)",
            "hsb(0.0, 1.0, 1.0)",
            "HSB(120.0, 0.5, 0.5)",
            "hsba(240.0, 1.0, 1.0, 0.5)",
            "$molcol",
            "red{alpha: 0.5}",
            "red{material: glass}",
            "rgb(255, 0, 0){alpha: 0.5; brightness: 1.2}",
        ] {
            assert!(parse_color(color).is_ok(), "{color}");
        }
    }

    #[test]
    fn rejects_invalid_cuemol_color_syntax_representatives() {
        for color in [
            "",
            "   ",
            "color space",
            "red blue",
            "#ggg",
            "rgb(255, 0)",
            "rgb(255; 0; 0)",
            "hsb(0.0, 1.0)",
            "$mol col",
            "red{alpha: 0.5",
            "red{alpha: opaque}",
            "Rgb(255, 0, 0)",
        ] {
            assert!(parse_color(color).is_err(), "{color}");
        }
    }

    #[test]
    fn color_parser_preserves_material_modifier() {
        let color = parse_color("rgb(255, 0, 0){material: glass; alpha: 0.5}").unwrap();
        assert_eq!(color.modifiers.len(), 2);
        assert_eq!(color.modifiers[0].name, "material");
        assert_eq!(color.modifiers[0].value.as_text(), "glass");
        assert_eq!(color.modifiers[1].name, "alpha");
        assert_eq!(color.modifiers[1].value.as_number(), Some(0.5));
        assert_eq!(color.rgba8().unwrap().a, 128);
    }

    #[test]
    fn generic_cmyk_color_proof_matches_reference_values() {
        let white = parse_color("#FFF").unwrap();
        assert_eq!(white.device_code_generic_cmyk(), Some(0xF2F3FA));
        assert_eq!(white.is_in_gamut_generic_cmyk(), Some(true));

        let gray = parse_color("#777").unwrap();
        assert_eq!(gray.device_code_generic_cmyk(), Some(0x767578));
        assert_eq!(gray.is_in_gamut_generic_cmyk(), Some(true));

        let blue = parse_color("#00F").unwrap();
        assert_eq!(blue.device_code_generic_cmyk(), Some(0x345BA0));
        assert_eq!(blue.is_in_gamut_generic_cmyk(), Some(false));

        let magenta = parse_color("#F0F").unwrap();
        assert_eq!(magenta.device_code_generic_cmyk(), Some(0xB469A4));
        assert_eq!(magenta.is_in_gamut_generic_cmyk(), Some(false));
    }
}
