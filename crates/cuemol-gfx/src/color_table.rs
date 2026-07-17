use crate::{GradientColor, SolidColor};
use cuemol_core::Vector4;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct IntColor {
    pub cid1: i16,
    pub cid2: i16,
    pub rho: u8,
}

impl IntColor {
    pub fn solid(cid: i16) -> Self {
        Self {
            cid1: cid,
            cid2: -1,
            rho: 0,
        }
    }

    pub fn rho_f(self) -> f64 {
        f64::from(self.rho) / 255.0
    }

    pub fn get_rho_f(self) -> f64 {
        self.rho_f()
    }

    pub fn set_rho_f(&mut self, value: f64) {
        self.rho = ColorTable::conv_rho(value, false);
    }

    pub fn rho_i(self) -> u8 {
        self.rho
    }

    pub fn get_rho_i(self) -> u8 {
        self.rho_i()
    }

    pub fn set_rho_i(&mut self, value: u8) {
        self.rho = value;
    }

    pub fn is_gradient(self) -> bool {
        self.cid2 >= 0
    }

    pub fn is_grad(self) -> bool {
        self.is_gradient()
    }
}

impl PartialEq for IntColor {
    fn eq(&self, other: &Self) -> bool {
        if self.cid2 < 0 && other.cid2 < 0 {
            self.cid1 == other.cid1
        } else if self.cid2 >= 0 && other.cid2 >= 0 {
            self.cid1 == other.cid1 && self.cid2 == other.cid2 && self.rho == other.rho
        } else {
            false
        }
    }
}

impl Eq for IntColor {}

#[derive(Debug, Clone, PartialEq)]
pub enum ColorTableColor {
    Solid(SolidColor),
    Gradient(GradientColor),
}

impl From<SolidColor> for ColorTableColor {
    fn from(value: SolidColor) -> Self {
        Self::Solid(value)
    }
}

impl From<&SolidColor> for ColorTableColor {
    fn from(value: &SolidColor) -> Self {
        Self::Solid(value.clone())
    }
}

impl From<GradientColor> for ColorTableColor {
    fn from(value: GradientColor) -> Self {
        Self::Gradient(value)
    }
}

impl From<&GradientColor> for ColorTableColor {
    fn from(value: &GradientColor) -> Self {
        Self::Gradient(value.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClutEntry {
    code: u32,
    material: String,
}

#[derive(Debug, Clone, Default)]
pub struct ColorTable {
    clut: Vec<ClutEntry>,
    gradients: BTreeMap<(i16, i16), (IntColor, i32)>,
}

impl ColorTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn size(&self) -> usize {
        self.clut.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clut.is_empty()
    }

    pub fn new_color<C>(
        &mut self,
        color: C,
        default_material: &str,
        context_id: Option<u64>,
    ) -> IntColor
    where
        C: Into<ColorTableColor>,
    {
        match color.into() {
            ColorTableColor::Solid(color) => {
                let material = material_or_default(color.material(), default_material);
                IntColor::solid(self.intern_color(&color, material, context_id))
            }
            ColorTableColor::Gradient(color) => {
                let material1 = material_or_default(color.col1().material(), default_material);
                let material2 = material_or_default(color.col2().material(), default_material);
                let mut cid1 = self.intern_color(color.col1(), material1, context_id);
                let mut cid2 = self.intern_color(color.col2(), material2, context_id);
                let mut rho = Self::conv_rho(color.rho(), false);
                if cid1 > cid2 {
                    std::mem::swap(&mut cid1, &mut cid2);
                    rho = Self::conv_rho(color.rho(), true);
                }
                let value = IntColor { cid1, cid2, rho };
                self.append_gradient(value);
                value
            }
        }
    }

    pub fn new_color_default<C>(&mut self, color: C, default_material: &str) -> IntColor
    where
        C: Into<ColorTableColor>,
    {
        self.new_color(color, default_material, None)
    }

    pub fn conv_rho(rho: f64, inverse: bool) -> u8 {
        let mut value = rho.clamp(0.0, 1.0);
        if inverse {
            value = 1.0 - value;
        }
        (value * 255.0 + 0.5) as u8
    }

    pub fn import(&mut self, source: &ColorTable) -> usize {
        let offset = self.clut.len();
        self.clut.extend(source.clut.iter().cloned());
        offset
    }

    pub fn get_color(&self, color: &IntColor) -> Option<ColorTableColor> {
        let first = self.entry(color.cid1)?;
        let mut color1 = SolidColor::from_code(first.code);
        color1.set_material(first.material.clone());
        if !color.is_gradient() {
            return Some(ColorTableColor::Solid(color1));
        }

        let second = self.entry(color.cid2)?;
        let mut color2 = SolidColor::from_code(second.code);
        color2.set_material(second.material.clone());
        Some(ColorTableColor::Gradient(GradientColor::from_colors(
            color1,
            color2,
            color.rho_f(),
        )))
    }

    pub fn get_material(&self, color: &IntColor) -> Option<&str> {
        self.entry(color.cid1).map(|entry| entry.material.as_str())
    }

    pub fn get_rgba_byte_color(&self, color: &IntColor) -> Option<[u8; 4]> {
        let first = rgba_bytes(self.entry(color.cid1)?.code);
        if !color.is_gradient() {
            return Some(first);
        }
        let second = rgba_bytes(self.entry(color.cid2)?.code);
        let rho = u16::from(color.rho);
        let inverse = 255 - rho;
        let mut result = [0; 4];
        for index in 0..4 {
            result[index] =
                ((u16::from(first[index]) * rho + u16::from(second[index]) * inverse) / 255) as u8;
        }
        Some(result)
    }

    pub fn get_rgba_vec_color(&self, color: &IntColor) -> Option<Vector4> {
        let rgba = self.get_rgba_byte_color(color)?;
        Some(Vector4::new4(
            f64::from(rgba[0]) / 255.0,
            f64::from(rgba[1]) / 255.0,
            f64::from(rgba[2]) / 255.0,
            f64::from(rgba[3]) / 255.0,
        ))
    }

    pub fn append_gradient(&mut self, color: IntColor) {
        self.gradients
            .entry((color.cid1, color.cid2))
            .or_insert((color, 0));
    }

    pub fn index_gradients(&mut self) {
        for (index, (_, gradient_index)) in self.gradients.values_mut().enumerate() {
            *gradient_index = index as i32;
        }
    }

    pub fn gradient_count(&self) -> usize {
        self.gradients.len()
    }

    pub fn get_grad_index(&self, color: &IntColor) -> i32 {
        self.gradients
            .get(&(color.cid1, color.cid2))
            .map_or(-1, |(_, index)| *index)
    }

    pub fn find_grad_by_index(&self, index: i32) -> Option<&IntColor> {
        self.gradients
            .values()
            .find(|(_, gradient_index)| *gradient_index == index)
            .map(|(color, _)| color)
    }

    fn intern_color(&mut self, color: &SolidColor, material: &str, context_id: Option<u64>) -> i16 {
        let code = context_id.map_or_else(|| color.code(), |id| color.get_dev_code(id));
        if let Some(index) = self
            .clut
            .iter()
            .position(|entry| entry.code == code && entry.material == material)
        {
            return index as i16;
        }
        let index = self.clut.len();
        self.clut.push(ClutEntry {
            code,
            material: material.to_string(),
        });
        index as i16
    }

    fn entry(&self, index: i16) -> Option<&ClutEntry> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.clut.get(index))
    }
}

fn material_or_default<'a>(material: &'a str, default_material: &'a str) -> &'a str {
    if material.is_empty() {
        default_material
    } else {
        material
    }
}

fn rgba_bytes(code: u32) -> [u8; 4] {
    [
        ((code >> 16) & 0xff) as u8,
        ((code >> 8) & 0xff) as u8,
        (code & 0xff) as u8,
        ((code >> 24) & 0xff) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_colors_are_deduplicated_by_code_and_material() {
        let mut table = ColorTable::new();
        let red = SolidColor::from_rgb_u8(255, 0, 0);
        let first = table.new_color_default(&red, "matte");
        let duplicate = table.new_color_default(&red, "matte");
        let other_material = table.new_color_default(&red, "glossy");

        assert_eq!(first, duplicate);
        assert_ne!(first.cid1, other_material.cid1);
        assert_eq!(table.size(), 2);
        assert_eq!(table.get_rgba_byte_color(&first), Some([255, 0, 0, 255]));
        assert_eq!(table.get_material(&first), Some("matte"));
    }

    #[test]
    fn gradients_are_canonicalized_reconstructed_and_indexed() {
        let mut table = ColorTable::new();
        let red = SolidColor::from_rgb_u8(255, 0, 0);
        let blue = SolidColor::from_rgb_u8(0, 0, 255);
        table.new_color_default(&blue, "default");
        let gradient = GradientColor::from_colors(red, blue, 0.25);
        let encoded = table.new_color_default(&gradient, "default");

        assert!(encoded.cid1 < encoded.cid2);
        assert_eq!(encoded.rho, ColorTable::conv_rho(0.75, false));
        assert_eq!(table.gradient_count(), 1);
        table.index_gradients();
        assert_eq!(table.get_grad_index(&encoded), 0);
        assert_eq!(table.find_grad_by_index(0), Some(&encoded));

        let bytes = table.get_rgba_byte_color(&encoded).unwrap();
        assert_eq!(bytes, [64, 0, 191, 255]);
        let ColorTableColor::Gradient(decoded) = table.get_color(&encoded).unwrap() else {
            panic!("expected gradient");
        };
        assert_eq!(decoded.r(), bytes[0]);
        assert_eq!(decoded.b(), bytes[2]);
    }

    #[test]
    fn import_appends_entries_and_reports_offset() {
        let mut source = ColorTable::new();
        let source_id = source.new_color_default(SolidColor::from_rgb_u8(1, 2, 3), "");
        let mut target = ColorTable::new();
        target.new_color_default(SolidColor::from_rgb_u8(4, 5, 6), "");
        assert_eq!(target.import(&source), 1);

        let imported = IntColor::solid(source_id.cid1 + 1);
        assert_eq!(target.get_rgba_byte_color(&imported), Some([1, 2, 3, 255]));
    }
}
