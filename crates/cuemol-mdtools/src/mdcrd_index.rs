//! Streaming frame boundaries for AMBER ASCII trajectories.
use crate::TrajectoryFrame;
use cuemol_core::{CueMolError, Result};

#[derive(Debug, Clone)]
pub struct MdcrdLayout {
    pub spans: Vec<(u64, u64)>,
    pub atoms: usize,
    pub has_box: bool,
}

/// Accepts arbitrarily split byte chunks and retains only line and frame offsets.
pub struct MdcrdIndexer {
    atoms: usize,
    stride: usize,
    has_box: Option<bool>,
    title: bool,
    line: Vec<u8>,
    offset: u64,
    line_start: u64,
    frame_start: Option<u64>,
    frame_end: u64,
    coordinates: usize,
    frames: usize,
    spans: Vec<(u64, u64)>,
}
impl MdcrdIndexer {
    pub fn new(atoms: usize, stride: usize, has_box: Option<bool>) -> Result<Self> {
        if atoms == 0 || atoms.checked_mul(3).is_none() || stride == 0 {
            return Err(CueMolError::invalid_argument(
                "MDCRD requires an atom count and positive stride",
            ));
        }
        Ok(Self {
            atoms,
            stride,
            has_box,
            title: false,
            line: Vec::new(),
            offset: 0,
            line_start: 0,
            frame_start: None,
            frame_end: 0,
            coordinates: 0,
            frames: 0,
            spans: Vec::new(),
        })
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<()> {
        for byte in bytes {
            self.offset += 1;
            if *byte == b'\n' {
                self.complete_line()?;
                self.line.clear();
                self.line_start = self.offset;
            } else {
                if self.line.len() >= 4096 {
                    return Err(CueMolError::parse("MDCRD line exceeds 4096 bytes"));
                }
                self.line.push(*byte);
            }
        }
        Ok(())
    }
    fn complete_line(&mut self) -> Result<()> {
        if !self.title {
            self.title = true;
            return Ok(());
        }
        let line = std::str::from_utf8(&self.line)
            .map_err(|_| CueMolError::parse("MDCRD is not ASCII text"))?
            .trim_end_matches('\r');
        if line.trim().is_empty() {
            return Ok(());
        }
        let count = if line.is_ascii() && line.len() % 8 == 0 {
            line.as_bytes()
                .chunks(8)
                .filter(|field| field.iter().any(|b| !b.is_ascii_whitespace()))
                .count()
        } else {
            line.split_ascii_whitespace().count()
        };
        if count == 0 || count > 10 {
            return Err(CueMolError::parse("Invalid MDCRD coordinate row"));
        }
        if self.coordinates == self.atoms * 3 {
            let boxed = *self.has_box.get_or_insert(count == 3 && self.atoms > 1);
            if boxed {
                if count != 3 {
                    return Err(CueMolError::parse("MDCRD box row must have three lengths"));
                }
                self.frame_end = self.offset;
                self.finish_frame();
                return Ok(());
            }
            self.finish_frame();
        }
        if self.frame_start.is_none() {
            self.frame_start = Some(self.line_start);
        }
        self.coordinates += count;
        if self.coordinates > self.atoms * 3 {
            return Err(CueMolError::parse(
                "MDCRD coordinate row crosses a frame boundary",
            ));
        }
        self.frame_end = self.offset;
        if self.coordinates == self.atoms * 3 && self.has_box == Some(false) {
            self.finish_frame();
        }
        Ok(())
    }
    fn finish_frame(&mut self) {
        if self.frames % self.stride == 0 {
            let start = self.frame_start.expect("coordinate frame started");
            self.spans.push((start, self.frame_end - start));
        }
        self.frames += 1;
        self.frame_start = None;
        self.coordinates = 0;
    }
    pub fn finish(mut self) -> Result<MdcrdLayout> {
        if !self.line.is_empty() {
            self.complete_line()?;
        }
        if self.coordinates == self.atoms * 3 && self.has_box != Some(true) {
            self.finish_frame();
        }
        if self.coordinates != 0 || self.frames == 0 {
            return Err(CueMolError::parse("Empty or truncated MDCRD trajectory"));
        }
        Ok(MdcrdLayout {
            spans: self.spans,
            atoms: self.atoms,
            has_box: self.has_box.unwrap_or(false),
        })
    }
}

pub fn decode_mdcrd_frame(bytes: &[u8], atoms: usize, has_box: bool) -> Result<TrajectoryFrame> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| CueMolError::parse("MDCRD is not ASCII text"))?;
    let mut trajectory = crate::read_amber_mdcrd_str(&format!("frame\n{text}"), atoms, has_box)?;
    if trajectory.frames.len() != 1 {
        return Err(CueMolError::parse("MDCRD indexed frame has invalid length"));
    }
    let frame = trajectory.frames.pop().unwrap();
    let mut coordinates = Vec::with_capacity(atoms * 3);
    for p in frame.positions {
        for value in [p.x, p.y, p.z] {
            if !(value as f32).is_finite() {
                return Err(CueMolError::parse("Invalid MDCRD coordinate"));
            }
            coordinates.push(value as f32);
        }
    }
    let cell = frame
        .cell
        .map(|c| [c.a(), c.b(), c.c(), c.alpha(), c.beta(), c.gamma()]);
    if cell.is_some_and(|c| c[..3].iter().any(|v| !v.is_finite() || *v <= 0.)) {
        return Err(CueMolError::parse("Invalid MDCRD box lengths"));
    }
    Ok(TrajectoryFrame { coordinates, cell })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(boxed: bool, newline: &str) -> Vec<u8> {
        let mut text = format!("AMBER trajectory{newline}");
        for frame in 0..7 {
            for row in (0..15).collect::<Vec<_>>().chunks(10) {
                for index in row {
                    text.push_str(&format!("{:8.3}", frame as f64 + *index as f64 * -0.125));
                }
                text.push_str(newline);
            }
            if boxed {
                text.push_str(&format!(
                    "{:8.3}{:8.3}{:8.3}{newline}",
                    30. + frame as f64,
                    40.,
                    50.
                ));
            }
        }
        text.into_bytes()
    }
    #[test]
    fn split_chunks_crlf_stride_and_random_frames_match_eager_reader() {
        for boxed in [false, true] {
            for newline in ["\n", "\r\n"] {
                let bytes = fixture(boxed, newline);
                let eager =
                    crate::read_amber_mdcrd_str(std::str::from_utf8(&bytes).unwrap(), 5, boxed)
                        .unwrap();
                for chunk_size in [1, 7, 16, 65, 65536] {
                    let mut index = MdcrdIndexer::new(5, 2, None).unwrap();
                    for chunk in bytes.chunks(chunk_size) {
                        index.push(chunk).unwrap();
                    }
                    let index = index.finish().unwrap();
                    assert_eq!(index.has_box, boxed);
                    assert_eq!(index.spans.len(), 4);
                    for i in [3, 0, 2, 1] {
                        let (start, len) = index.spans[i];
                        let frame = decode_mdcrd_frame(
                            &bytes[start as usize..(start + len) as usize],
                            5,
                            boxed,
                        )
                        .unwrap();
                        for (actual, expected) in frame
                            .coordinates
                            .chunks_exact(3)
                            .zip(&eager.frames[i * 2].positions)
                        {
                            assert_eq!(
                                actual,
                                &[expected.x as f32, expected.y as f32, expected.z as f32]
                            );
                        }
                        assert_eq!(frame.cell.is_some(), boxed);
                        if boxed {
                            assert_eq!(frame.cell.unwrap()[0], 30. + (i * 2) as f64);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn indexed_file_and_compressed_sources_keep_only_sampled_offsets() {
        use std::io::Write;
        for compressed in [false, true] {
            let bytes = fixture(true, "\n");
            let mut file = tempfile::Builder::new()
                .suffix(if compressed { ".mdcrd.gz" } else { ".mdcrd" })
                .tempfile()
                .unwrap();
            if compressed {
                let mut encoder =
                    flate2::write::GzEncoder::new(file.as_file_mut(), flate2::Compression::fast());
                encoder.write_all(&bytes).unwrap();
                encoder.finish().unwrap();
            } else {
                file.write_all(&bytes).unwrap();
            }
            let indexed = crate::IndexedTrajectory::open(file.path(), 2, Some(5)).unwrap();
            assert_eq!(indexed.frame_count(), 4);
            assert!(indexed.index_bytes() <= 256);
            for i in [3, 0, 2, 1] {
                let frame = indexed.read_frame(i).unwrap();
                assert_eq!(frame.coordinates[0], (i * 2) as f32);
                assert_eq!(frame.cell.unwrap()[0], 30. + (i * 2) as f64);
            }
        }
    }

    #[test]
    fn detects_missing_coordinates_or_cell_and_handles_final_row_without_newline() {
        let mut bytes = fixture(true, "\n");
        bytes.pop();
        let mut index = MdcrdIndexer::new(5, 1, Some(true)).unwrap();
        index.push(&bytes).unwrap();
        assert_eq!(index.finish().unwrap().spans.len(), 7);
        for remove in [24, 40, 80] {
            let mut index = MdcrdIndexer::new(5, 1, Some(true)).unwrap();
            let result = index.push(&bytes[..bytes.len() - remove]);
            assert!(result.is_err() || index.finish().is_err());
        }
    }
}
