use crate::PixelBuffer;
use cuemol_core::{ByteArray, CueMolError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextImgBuf {
    text: String,
    font: String,
    buffer: PixelBuffer,
}

impl Default for TextImgBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl TextImgBuf {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            font: String::new(),
            buffer: PixelBuffer::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn get_text(&self) -> &str {
        self.text()
    }

    pub fn set_text(&mut self, value: impl Into<String>) {
        self.text = value.into();
    }

    pub fn font(&self) -> &str {
        &self.font
    }

    pub fn get_font(&self) -> &str {
        self.font()
    }

    pub fn set_font(&mut self, value: impl Into<String>) {
        self.font = value.into();
    }

    pub fn width(&self) -> i32 {
        self.buffer.width() as i32
    }

    pub fn get_width(&self) -> i32 {
        self.width()
    }

    pub fn set_width(&mut self, value: i32) {
        self.buffer.set_width(value.max(0) as usize);
    }

    pub fn height(&self) -> i32 {
        self.buffer.height() as i32
    }

    pub fn get_height(&self) -> i32 {
        self.height()
    }

    pub fn set_height(&mut self, value: i32) {
        self.buffer.set_height(value.max(0) as usize);
    }

    pub fn depth(&self) -> i32 {
        self.buffer.depth() as i32
    }

    pub fn set_depth(&mut self, value: i32) {
        self.buffer.set_depth(value.max(0) as usize);
    }

    pub fn len(&self) -> usize {
        self.buffer.size()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.size() == 0
    }

    pub fn data(&self) -> &[u8] {
        self.buffer.data()
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        self.buffer.data_mut()
    }

    pub fn resize(&mut self, size: usize) {
        self.buffer.resize(size);
    }

    pub fn set_at(&mut self, index: usize, value: i32) -> Result<()> {
        let target = self
            .buffer
            .data_mut()
            .get_mut(index)
            .ok_or_else(|| CueMolError::invalid_argument("text image index out of bounds"))?;
        *target = value as u8;
        Ok(())
    }

    pub fn get_at(&self, index: usize) -> Result<i32> {
        self.buffer
            .data()
            .get(index)
            .copied()
            .map(i32::from)
            .ok_or_else(|| CueMolError::invalid_argument("text image index out of bounds"))
    }

    pub fn set_at_2d(&mut self, x: i32, y: i32, value: i32) -> Result<()> {
        let index = self.index_2d(x, y)?;
        self.set_at(index, value)
    }

    pub fn set_at2_d(&mut self, x: i32, y: i32, value: i32) -> Result<()> {
        self.set_at_2d(x, y, value)
    }

    pub fn set_at_2_d(&mut self, x: i32, y: i32, value: i32) -> Result<()> {
        self.set_at_2d(x, y, value)
    }

    pub fn get_at_2d(&self, x: i32, y: i32) -> Result<i32> {
        let index = self.index_2d(x, y)?;
        self.get_at(index)
    }

    pub fn get_at2_d(&self, x: i32, y: i32) -> Result<i32> {
        self.get_at_2d(x, y)
    }

    pub fn get_at_2_d(&self, x: i32, y: i32) -> Result<i32> {
        self.get_at_2d(x, y)
    }

    pub fn set_data_from_rgba(&mut self, src: &ByteArray) -> Result<()> {
        let values = src.copy_to_u8()?;
        let expected = self.len() * 4;
        if values.len() != expected {
            return Err(CueMolError::invalid_argument(
                "RGBA byte array length must match text image size",
            ));
        }
        for (dst, rgba) in self
            .buffer
            .data_mut()
            .iter_mut()
            .zip(values.chunks_exact(4))
        {
            *dst = rgba[3];
        }
        Ok(())
    }

    pub fn set_data_from_r_g_b_a(&mut self, src: &ByteArray) -> Result<()> {
        self.set_data_from_rgba(src)
    }

    pub fn alpha_from_rgba(src: &ByteArray) -> Result<Vec<u8>> {
        let values = src.copy_to_u8()?;
        if values.len() % 4 != 0 {
            return Err(CueMolError::invalid_argument(
                "RGBA byte array length must be divisible by four",
            ));
        }
        Ok(values.chunks_exact(4).map(|rgba| rgba[3]).collect())
    }

    fn index_2d(&self, x: i32, y: i32) -> Result<usize> {
        if x < 0 || y < 0 || x >= self.width() || y >= self.height() {
            return Err(CueMolError::invalid_argument(
                "text image 2D index out of bounds",
            ));
        }
        Ok((y * self.width() + x) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_img_buf_matches_qif_buffer_operations() {
        let mut buf = TextImgBuf::new();
        buf.set_text("label");
        buf.set_font("12px sans-serif");
        buf.set_width(2);
        buf.set_height(2);
        buf.resize(4);

        assert_eq!(buf.text(), "label");
        assert_eq!(buf.font(), "12px sans-serif");
        assert_eq!(buf.len(), 4);
        buf.set_at(0, 0x44).unwrap();
        assert_eq!(buf.get_at(0).unwrap(), 0x44);
        buf.set_at_2d(1, 1, 7).unwrap();
        assert_eq!(buf.get_at_2d(1, 1).unwrap(), 7);
        assert!(buf.get_at_2d(2, 0).is_err());

        let rgba = ByteArray::copy_from_u8(&[
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
            0xff, 0x12,
        ]);
        buf.set_data_from_rgba(&rgba).unwrap();
        assert_eq!(buf.data(), &[0x44, 0x88, 0xcc, 0x12]);
    }
}
