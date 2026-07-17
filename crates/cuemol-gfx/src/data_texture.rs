use std::path::Path;

use cuemol_core::{CueMolError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTexture {
    width: usize,
    height: usize,
    components: usize,
    linear_filter: bool,
    data: Vec<u8>,
    bound_unit: Option<i32>,
}

impl DataTexture {
    pub fn new(
        width: usize,
        height: usize,
        components: usize,
        linear_filter: bool,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(CueMolError::invalid_argument(
                "data texture dimensions must be positive",
            ));
        }
        if !matches!(components, 1 | 2) {
            return Err(CueMolError::invalid_argument(
                "data texture must have one or two components",
            ));
        }
        let data = data.into();
        let expected_size = width
            .checked_mul(height)
            .and_then(|value| value.checked_mul(components))
            .ok_or_else(|| CueMolError::invalid_argument("data texture size overflow"))?;
        if data.len() != expected_size {
            return Err(CueMolError::invalid_argument(
                "data texture byte count does not match its dimensions",
            ));
        }
        Ok(Self {
            width,
            height,
            components,
            linear_filter,
            data,
            bound_unit: None,
        })
    }

    pub fn from_file(
        path: impl AsRef<Path>,
        width: usize,
        height: usize,
        components: usize,
        linear_filter: bool,
    ) -> Result<Self> {
        Self::new(
            width,
            height,
            components,
            linear_filter,
            std::fs::read(path)?,
        )
    }

    pub fn bind(&mut self, texture_unit: i32) {
        self.bound_unit = Some(texture_unit);
    }

    pub fn unbind(&mut self) {
        self.bound_unit = None;
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

    pub fn components(&self) -> usize {
        self.components
    }

    pub fn uses_linear_filter(&self) -> bool {
        self.linear_filter
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn bound_unit(&self) -> Option<i32> {
        self.bound_unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_texture_validates_layout_and_tracks_binding() {
        assert!(DataTexture::new(2, 2, 3, false, vec![0; 12]).is_err());
        assert!(DataTexture::new(2, 2, 2, false, vec![0; 7]).is_err());
        let mut texture = DataTexture::new(2, 2, 2, true, vec![7; 8]).unwrap();
        assert_eq!(
            (texture.width(), texture.height(), texture.components()),
            (2, 2, 2)
        );
        assert!(texture.uses_linear_filter());
        texture.bind(4);
        assert_eq!(texture.bound_unit(), Some(4));
        texture.unbind();
        assert_eq!(texture.bound_unit(), None);
    }
}
