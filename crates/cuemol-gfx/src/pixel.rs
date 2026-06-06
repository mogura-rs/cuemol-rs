#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelBuffer {
    width: usize,
    height: usize,
    depth: usize,
    data: Vec<u8>,
    rep_handle: Option<usize>,
}

impl Default for PixelBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelBuffer {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            depth: 8,
            data: Vec::new(),
            rep_handle: None,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn set_width(&mut self, value: usize) {
        self.width = value;
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn set_height(&mut self, value: usize) {
        self.height = value;
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn set_depth(&mut self, value: usize) {
        self.depth = value;
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn resize(&mut self, size: usize) {
        self.data.resize(size, 0);
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn at(&self, index: usize) -> u8 {
        self.data[index]
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.rep_handle = None;
    }

    pub fn rep_handle(&self) -> Option<usize> {
        self.rep_handle
    }

    pub fn set_rep_handle(&mut self, value: Option<usize>) {
        self.rep_handle = value;
    }
}
