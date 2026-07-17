use cuemol_core::{CueMolError, Result};

pub const RT_COLOR_RGBA8: u32 = 0x01;
pub const RT_DEPTH_TEX: u32 = 0x02;
pub const RT_COLOR_NEAREST: u32 = 0x04;
pub const RT_NORMAL_RGBA16F: u32 = 0x08;
pub const RT_COLOR_RGBA16F: u32 = 0x10;

pub const RT_TU_COLOR: i32 = 0;
pub const RT_TU_DEPTH: i32 = 1;
pub const RT_TU_NORMAL: i32 = 2;
pub const RT_TU_NOISE: i32 = 3;
pub const RT_TU_SMAA_AREA: i32 = 4;
pub const RT_TU_SMAA_SEARCH: i32 = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct RenderTarget {
    width: usize,
    height: usize,
    flags: u32,
    color: Vec<[f32; 4]>,
    normal: Option<Vec<[f32; 4]>>,
    depth: Option<Vec<f32>>,
    bound: bool,
    bound_color_textures: Vec<(usize, i32)>,
    bound_depth_texture: Option<i32>,
}

impl RenderTarget {
    pub fn new(width: usize, height: usize, flags: u32) -> Result<Self> {
        let mut target = Self {
            width: 0,
            height: 0,
            flags,
            color: Vec::new(),
            normal: None,
            depth: None,
            bound: false,
            bound_color_textures: Vec::new(),
            bound_depth_texture: None,
        };
        target.resize(width, height)?;
        Ok(target)
    }

    pub fn bind(&mut self) {
        self.bound = true;
    }

    pub fn unbind(&mut self) {
        self.bound = false;
    }

    pub fn is_bound(&self) -> bool {
        self.bound
    }

    pub fn clear(&mut self, red: f32, green: f32, blue: f32, alpha: f32) {
        self.color.fill([red, green, blue, alpha]);
        if let Some(normal) = &mut self.normal {
            normal.fill([0.0; 4]);
        }
        if let Some(depth) = &mut self.depth {
            depth.fill(1.0);
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) -> Result<()> {
        if width == 0 || height == 0 {
            return Err(CueMolError::invalid_argument(
                "render target dimensions must be positive",
            ));
        }
        if self.width == width && self.height == height {
            return Ok(());
        }
        let size = width
            .checked_mul(height)
            .ok_or_else(|| CueMolError::invalid_argument("render target size overflow"))?;
        self.width = width;
        self.height = height;
        self.color = vec![[0.0; 4]; size];
        self.normal = (self.flags & RT_NORMAL_RGBA16F != 0).then(|| vec![[0.0; 4]; size]);
        self.depth = (self.flags & RT_DEPTH_TEX != 0).then(|| vec![1.0; size]);
        Ok(())
    }

    pub fn bind_color_texture(&mut self, attachment: usize, texture_unit: i32) -> Result<()> {
        if attachment > usize::from(self.normal.is_some()) {
            return Err(CueMolError::invalid_argument(
                "render target color attachment is unavailable",
            ));
        }
        self.bound_color_textures
            .retain(|(index, _)| *index != attachment);
        self.bound_color_textures.push((attachment, texture_unit));
        Ok(())
    }

    pub fn bind_depth_texture(&mut self, texture_unit: i32) -> Result<()> {
        if self.depth.is_none() {
            return Err(CueMolError::invalid_argument(
                "render target has no depth texture",
            ));
        }
        self.bound_depth_texture = Some(texture_unit);
        Ok(())
    }

    pub fn unbind_textures(&mut self) {
        self.bound_color_textures.clear();
        self.bound_depth_texture = None;
    }

    pub fn bound_color_textures(&self) -> &[(usize, i32)] {
        &self.bound_color_textures
    }

    pub fn bound_depth_texture(&self) -> Option<i32> {
        self.bound_depth_texture
    }

    pub fn read_color(
        &self,
        attachment: usize,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        components: usize,
    ) -> Result<Vec<u8>> {
        if !matches!(components, 3 | 4) {
            return Err(CueMolError::invalid_argument(
                "color readback must use three or four components",
            ));
        }
        if x.checked_add(width).is_none_or(|end| end > self.width)
            || y.checked_add(height).is_none_or(|end| end > self.height)
        {
            return Err(CueMolError::invalid_argument(
                "color readback rectangle is out of range",
            ));
        }
        let source = self.color_attachment(attachment)?;
        let mut output = Vec::with_capacity(width * height * components);
        for row in y..y + height {
            for column in x..x + width {
                let pixel = source[row * self.width + column];
                output.extend(
                    pixel[..components]
                        .iter()
                        .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8),
                );
            }
        }
        Ok(output)
    }

    pub fn read_color_into(
        &self,
        attachment: usize,
        rectangle: [usize; 4],
        components: usize,
        output: &mut [u8],
    ) -> Result<usize> {
        let data = self.read_color(
            attachment,
            rectangle[0],
            rectangle[1],
            rectangle[2],
            rectangle[3],
            components,
        )?;
        if output.len() < data.len() {
            return Err(CueMolError::invalid_argument(
                "color readback buffer is too small",
            ));
        }
        output[..data.len()].copy_from_slice(&data);
        Ok(data.len())
    }

    pub fn set_color_pixel(
        &mut self,
        attachment: usize,
        x: usize,
        y: usize,
        value: [f32; 4],
    ) -> Result<()> {
        let width = self.width;
        if x >= width || y >= self.height {
            return Err(CueMolError::invalid_argument(
                "render target pixel is out of range",
            ));
        }
        self.color_attachment_mut(attachment)?[y * width + x] = value;
        Ok(())
    }

    pub fn color_attachment(&self, attachment: usize) -> Result<&[[f32; 4]]> {
        match attachment {
            0 => Ok(&self.color),
            1 => self.normal.as_deref().ok_or_else(|| {
                CueMolError::invalid_argument("render target normal attachment is unavailable")
            }),
            _ => Err(CueMolError::invalid_argument(
                "render target color attachment is unavailable",
            )),
        }
    }

    pub fn color_attachment_mut(&mut self, attachment: usize) -> Result<&mut [[f32; 4]]> {
        match attachment {
            0 => Ok(&mut self.color),
            1 => self.normal.as_deref_mut().ok_or_else(|| {
                CueMolError::invalid_argument("render target normal attachment is unavailable")
            }),
            _ => Err(CueMolError::invalid_argument(
                "render target color attachment is unavailable",
            )),
        }
    }

    pub fn depth(&self) -> Option<&[f32]> {
        self.depth.as_deref()
    }

    pub fn depth_mut(&mut self) -> Option<&mut [f32]> {
        self.depth.as_deref_mut()
    }

    pub fn has_normal(&self) -> bool {
        self.normal.is_some()
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn get_width(&self) -> usize {
        self.width()
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn get_height(&self) -> usize {
        self.height()
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn uses_nearest_color_filter(&self) -> bool {
        self.flags & RT_COLOR_NEAREST != 0
    }

    pub fn uses_float_color(&self) -> bool {
        self.flags & RT_COLOR_RGBA16F != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_target_allocates_requested_attachments_and_bindings() {
        let mut target =
            RenderTarget::new(2, 3, RT_COLOR_RGBA8 | RT_DEPTH_TEX | RT_NORMAL_RGBA16F).unwrap();
        assert!(target.has_normal());
        assert_eq!(target.depth().unwrap().len(), 6);
        target.bind();
        assert!(target.is_bound());
        target.bind_color_texture(1, RT_TU_NORMAL).unwrap();
        target.bind_depth_texture(RT_TU_DEPTH).unwrap();
        assert_eq!(target.bound_color_textures(), &[(1, RT_TU_NORMAL)]);
        assert_eq!(target.bound_depth_texture(), Some(RT_TU_DEPTH));
        target.unbind_textures();
        target.unbind();
        assert!(!target.is_bound());
    }

    #[test]
    fn clear_resize_and_readback_preserve_framebuffer_coordinates() {
        let mut target = RenderTarget::new(2, 2, RT_COLOR_RGBA8 | RT_DEPTH_TEX).unwrap();
        target.clear(0.25, 0.5, 1.0, 0.0);
        assert_eq!(target.depth().unwrap(), &[1.0; 4]);
        target
            .set_color_pixel(0, 1, 0, [1.0, 0.0, 0.0, 1.0])
            .unwrap();
        assert_eq!(
            target.read_color(0, 0, 0, 2, 1, 4).unwrap(),
            vec![64, 128, 255, 0, 255, 0, 0, 255]
        );
        target.resize(3, 1).unwrap();
        assert_eq!(target.color_attachment(0).unwrap().len(), 3);
    }
}
