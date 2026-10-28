//! Seekable trajectory access. Indexes retain offsets, never decoded coordinates.
use crate::{read_dcd_bytes, read_trr_bytes, read_xtc_bytes};
use cuemol_core::{CueMolError, Result};
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
enum Format {
    Dcd(Vec<u8>),
    Mdcrd { has_box: bool },
    Xtc,
    Trr,
    Netcdf(crate::netcdf::NetcdfFileLayout),
}

#[derive(Debug, Clone, Copy)]
struct Span {
    offset: u64,
    len: u64,
}

#[derive(Debug)]
enum Source {
    File(File),
    Memory(Cursor<Arc<[u8]>>),
}
impl Read for Source {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::File(file) => file.read(buffer),
            Self::Memory(data) => data.read(buffer),
        }
    }
}
impl Seek for Source {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::File(file) => file.seek(position),
            Self::Memory(data) => data.seek(position),
        }
    }
}

/// An open file and a sampled frame index, shared by GUI undo snapshots via `Arc`.
#[derive(Debug)]
pub struct IndexedTrajectory {
    file: Mutex<Source>,
    _temporary: Option<tempfile::TempPath>,
    file_len: u64,
    format: Format,
    spans: Vec<Span>,
    atom_count: usize,
    stride: usize,
    frame_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrajectoryFrame {
    pub coordinates: Vec<f32>,
    pub cell: Option<[f64; 6]>,
}

impl IndexedTrajectory {
    pub fn open(path: impl AsRef<Path>, stride: usize, expected: Option<usize>) -> Result<Self> {
        Self::open_with_amber_box(path, stride, expected, None)
    }

    pub fn open_with_amber_box(
        path: impl AsRef<Path>,
        stride: usize,
        expected: Option<usize>,
        has_box: Option<bool>,
    ) -> Result<Self> {
        if stride == 0 {
            return Err(CueMolError::invalid_argument(
                "trajectory stride must be positive",
            ));
        }
        let path = path.as_ref();
        let (file, temporary) = if crate::fileio::detect_compression(path)?
            == crate::fileio::CompressionEncoding::Raw
        {
            (File::open(path)?, None)
        } else {
            let mut spool = tempfile::NamedTempFile::new()?;
            crate::fileio::decompress_to(path, &mut spool)?;
            let (mut file, path) = spool.into_parts();
            file.rewind()?;
            (file, Some(path))
        };
        let file_len = file.metadata()?.len();
        Self::from_source(
            Source::File(file),
            file_len,
            path,
            stride,
            expected,
            has_box,
            temporary,
        )
    }

    fn from_source(
        mut file: Source,
        file_len: u64,
        path: &Path,
        stride: usize,
        expected: Option<usize>,
        has_box: Option<bool>,
        temporary: Option<tempfile::TempPath>,
    ) -> Result<Self> {
        if stride == 0 {
            return Err(CueMolError::invalid_argument(
                "trajectory stride must be positive",
            ));
        }

        let mut magic = [0; 4];
        file.read_exact(&mut magic)?;
        file.rewind()?;
        let (format, spans, atom_count, frame_count) = if &magic[..3] == b"CDF" {
            let layout = crate::netcdf::NetcdfFileLayout::open(&mut file)?;
            let count = layout.frame_count();
            let atoms = layout.atom_count();
            (
                Format::Netcdf(layout),
                Vec::new(),
                atoms,
                count.div_ceil(stride),
            )
        } else if u32::from_le_bytes(magic) == 84 || u32::from_be_bytes(magic) == 84 {
            let (prefix, spans, atoms) = index_dcd(&mut file, file_len, stride)?;
            let count = spans.len();
            (Format::Dcd(prefix), spans, atoms, count)
        } else if is_mdcrd_path(path) {
            let atoms = expected.ok_or_else(|| {
                CueMolError::invalid_argument("MDCRD requires a topology atom count")
            })?;
            let mut index = crate::mdcrd_index::MdcrdIndexer::new(atoms, stride, has_box)?;
            let mut buffer = [0; 65536];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                index.push(&buffer[..count])?;
            }
            let index = index.finish()?;
            let spans: Vec<_> = index
                .spans
                .into_iter()
                .map(|(offset, len)| Span { offset, len })
                .collect();
            let count = spans.len();
            (
                Format::Mdcrd {
                    has_box: index.has_box,
                },
                spans,
                atoms,
                count,
            )
        } else {
            let xtc = matches!(i32::from_be_bytes(magic), 1995 | 2023);
            if !xtc && i32::from_be_bytes(magic) != 1993 {
                return Err(CueMolError::parse(
                    "not a supported DCD, XTC, TRR or NetCDF3 trajectory",
                ));
            }
            let (spans, atoms) = index_xdr(&mut file, file_len, stride, xtc)?;
            let count = spans.len();
            (
                if xtc { Format::Xtc } else { Format::Trr },
                spans,
                atoms,
                count,
            )
        };
        if expected.is_some_and(|expected| expected != atom_count) {
            return Err(CueMolError::parse(format!(
                "trajectory atom count {atom_count} does not match topology"
            )));
        }
        if frame_count == 0 {
            return Err(CueMolError::parse(
                "trajectory contains no coordinate frames",
            ));
        }
        Ok(Self {
            file: Mutex::new(file),
            _temporary: temporary,
            file_len,
            format,
            spans,
            atom_count,
            stride,
            frame_count,
        })
    }

    pub fn from_bytes(bytes: &[u8], stride: usize, expected: Option<usize>) -> Result<Self> {
        Self::from_bytes_with_name(bytes, "trajectory", stride, expected)
    }

    pub fn from_bytes_with_name(
        bytes: &[u8],
        name: &str,
        stride: usize,
        expected: Option<usize>,
    ) -> Result<Self> {
        Self::from_source(
            Source::Memory(Cursor::new(Arc::from(bytes))),
            bytes.len() as u64,
            Path::new(name),
            stride,
            expected,
            None,
            None,
        )
    }

    pub fn format_name(&self) -> &'static str {
        match self.format {
            Format::Dcd(_) => "DCD",
            Format::Xtc => "XTC",
            Format::Trr => "TRR",
            Format::Netcdf(_) => "NETCDF",
            Format::Mdcrd { .. } => "MDCRD",
        }
    }

    pub fn atom_count(&self) -> usize {
        self.atom_count
    }
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
    pub fn index_bytes(&self) -> usize {
        self.spans.capacity() * std::mem::size_of::<Span>()
    }

    pub fn read_frame(&self, index: usize) -> Result<TrajectoryFrame> {
        if index >= self.frame_count {
            return Err(CueMolError::invalid_argument(
                "trajectory frame index out of range",
            ));
        }
        let mut file = self
            .file
            .lock()
            .map_err(|_| CueMolError::Io("trajectory file lock poisoned".into()))?;
        if matches!(&*file, Source::File(source) if source.metadata()?.len() != self.file_len) {
            return Err(CueMolError::Io(
                "trajectory file size changed; reopen the trajectory".into(),
            ));
        }
        if let Format::Netcdf(layout) = &self.format {
            return layout.read_frame(&mut *file, index * self.stride);
        }
        let span = self.spans[index];
        let prefix: &[u8] = match &self.format {
            Format::Dcd(prefix) => prefix,
            _ => &[],
        };
        let len = usize::try_from(span.len)
            .ok()
            .and_then(|len| len.checked_add(prefix.len()))
            .ok_or_else(|| CueMolError::parse("trajectory frame is too large"))?;
        let mut input = vec![0; len];
        input[..prefix.len()].copy_from_slice(prefix);
        file.seek(SeekFrom::Start(span.offset))?;
        file.read_exact(&mut input[prefix.len()..])?;
        if let Format::Mdcrd { has_box } = self.format {
            return crate::mdcrd_index::decode_mdcrd_frame(&input, self.atom_count, has_box);
        }
        let mut block = match self.format {
            Format::Dcd(_) => read_dcd_bytes(&input)?,
            Format::Xtc => read_xtc_bytes(&input)?,
            Format::Trr => read_trr_bytes(&input)?,
            Format::Netcdf(_) | Format::Mdcrd { .. } => unreachable!(),
        };
        let cell = block.cell(0);
        let coordinates = block
            .take_frame(0)
            .ok_or_else(|| CueMolError::parse("missing trajectory coordinates"))?;
        Ok(TrajectoryFrame { coordinates, cell })
    }
}

fn is_mdcrd_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let name = name
        .strip_suffix(".gz")
        .or_else(|| name.strip_suffix(".bz2"))
        .or_else(|| name.strip_suffix(".xz"))
        .unwrap_or(&name);
    name.ends_with(".mdcrd") || name.ends_with(".crd")
}

fn u32_at(bytes: &[u8], offset: usize, little: bool) -> u32 {
    let word = bytes[offset..offset + 4]
        .try_into()
        .expect("validated header size");
    if little {
        u32::from_le_bytes(word)
    } else {
        u32::from_be_bytes(word)
    }
}

fn seek_forward(file: &mut Source, bytes: u64, file_len: u64) -> Result<()> {
    let end = file
        .stream_position()?
        .checked_add(bytes)
        .filter(|end| *end <= file_len)
        .ok_or_else(|| CueMolError::parse("truncated trajectory record"))?;
    file.seek(SeekFrom::Start(end))?;
    Ok(())
}

fn dcd_record(
    file: &mut Source,
    file_len: u64,
    little: bool,
    expected: Option<u32>,
) -> Result<u32> {
    let mut word = [0; 4];
    file.read_exact(&mut word)?;
    let len = u32_at(&word, 0, little);
    if expected.is_some_and(|expected| expected != len) {
        return Err(CueMolError::parse("DCD invalid record size"));
    }
    seek_forward(file, u64::from(len), file_len)?;
    file.read_exact(&mut word)?;
    if u32_at(&word, 0, little) != len {
        return Err(CueMolError::parse("DCD record marker mismatch"));
    }
    Ok(len)
}

fn index_dcd(
    file: &mut Source,
    file_len: u64,
    stride: usize,
) -> Result<(Vec<u8>, Vec<Span>, usize)> {
    let mut header = [0; 92];
    file.read_exact(&mut header)?;
    let little = u32::from_le_bytes(header[..4].try_into().unwrap()) == 84;
    if &header[4..8] != b"CORD" || u32_at(&header, 88, little) != 84 {
        return Err(CueMolError::parse("invalid DCD header"));
    }
    let count = u32_at(&header, 8, little) as usize;
    if count == 0 || count > i32::MAX as usize || u32_at(&header, 40, little) != 0 {
        return Err(CueMolError::parse("empty DCD or unsupported fixed atoms"));
    }
    let cell = u32_at(&header, 48, little) != 0;
    dcd_record(file, file_len, little, None)?;
    let mut natom = [0; 12];
    file.read_exact(&mut natom)?;
    let atoms = u32_at(&natom, 4, little);
    if u32_at(&natom, 0, little) != 4
        || u32_at(&natom, 8, little) != 4
        || atoms == 0
        || atoms > i32::MAX as u32
    {
        return Err(CueMolError::parse("invalid DCD atom count"));
    }
    let coord_bytes = atoms
        .checked_mul(4)
        .ok_or_else(|| CueMolError::parse("DCD atom count overflow"))?;
    let start = file.stream_position()?;
    // Rebuild a minimal title so even unusually large titles stay off the heap.
    let one = if little {
        1_u32.to_le_bytes()
    } else {
        1_u32.to_be_bytes()
    };
    header[8..12].copy_from_slice(&one);
    let mut prefix = header.to_vec();
    let four = if little {
        4_u32.to_le_bytes()
    } else {
        4_u32.to_be_bytes()
    };
    prefix.extend(four);
    prefix.extend([0; 4]);
    prefix.extend(four);
    prefix.extend(natom);
    let minimum_frame = u64::from(coord_bytes) * 3 + 24 + if cell { 56 } else { 0 };
    if count as u64 > file_len.saturating_sub(start) / minimum_frame {
        return Err(CueMolError::parse("truncated DCD trajectory"));
    }
    let mut spans = Vec::new();
    for index in 0..count {
        let offset = file.stream_position()?;
        if cell {
            dcd_record(file, file_len, little, Some(48))?;
        }
        for _ in 0..3 {
            dcd_record(file, file_len, little, Some(coord_bytes))?;
        }
        if index.is_multiple_of(stride) {
            spans.push(Span {
                offset,
                len: file.stream_position()? - offset,
            });
        }
    }
    Ok((prefix, spans, atoms as usize))
}

fn read_be(file: &mut Source) -> Result<u32> {
    let mut word = [0; 4];
    file.read_exact(&mut word)?;
    Ok(u32::from_be_bytes(word))
}

fn index_xdr(
    file: &mut Source,
    file_len: u64,
    stride: usize,
    xtc: bool,
) -> Result<(Vec<Span>, usize)> {
    let mut spans = Vec::new();
    let mut atom_count = None;
    let mut index = 0_usize;
    while file.stream_position()? < file_len {
        let offset = file.stream_position()?;
        let magic = read_be(file)?;
        let (atoms, has_coordinates) = if xtc {
            if !matches!(magic, 1995 | 2023) {
                return Err(CueMolError::parse("invalid XTC magic"));
            }
            let atoms = read_be(file)?;
            seek_forward(file, 44, file_len)?;
            if read_be(file)? != atoms {
                return Err(CueMolError::parse("XTC contradictory atom count"));
            }
            if atoms <= 9 {
                seek_forward(file, u64::from(atoms) * 12, file_len)?;
            } else {
                seek_forward(file, 32, file_len)?;
                let length = if magic == 2023 {
                    (u64::from(read_be(file)?) << 32) | u64::from(read_be(file)?)
                } else {
                    u64::from(read_be(file)?)
                };
                let padded = length
                    .checked_add(3)
                    .ok_or_else(|| CueMolError::parse("XTC length overflow"))?
                    & !3;
                seek_forward(file, padded, file_len)?;
            }
            (atoms, true)
        } else {
            if magic != 1993 {
                return Err(CueMolError::parse("invalid TRR magic"));
            }
            let declared = u64::from(read_be(file)?);
            let length = u64::from(read_be(file)?);
            if declared != length && declared != length + 1 {
                return Err(CueMolError::parse("TRR invalid version string"));
            }
            seek_forward(file, (length + 3) & !3, file_len)?;
            let mut sizes = [0_u64; 10];
            for size in &mut sizes {
                *size = u64::from(read_be(file)?);
            }
            let atoms = read_be(file)?;
            seek_forward(file, 8, file_len)?;
            let scalar = if sizes[2] > 0 {
                sizes[2] / 9
            } else {
                let size = sizes[7..]
                    .iter()
                    .copied()
                    .find(|size| *size > 0)
                    .unwrap_or(0);
                size / (u64::from(atoms).max(1) * 3)
            };
            if !matches!(scalar, 4 | 8)
                || (sizes[2] > 0 && sizes[2] != scalar * 9)
                || sizes[7..]
                    .iter()
                    .any(|size| *size != 0 && *size != u64::from(atoms) * 3 * scalar)
            {
                return Err(CueMolError::parse("TRR invalid block sizes"));
            }
            seek_forward(file, scalar * 2 + sizes.iter().sum::<u64>(), file_len)?;
            (atoms, sizes[7] > 0)
        };
        if atoms == 0 || atoms > i32::MAX as u32 || atom_count.is_some_and(|count| count != atoms) {
            return Err(CueMolError::parse(
                "invalid or varying trajectory atom count",
            ));
        }
        atom_count = Some(atoms);
        if has_coordinates && index.is_multiple_of(stride) {
            spans.push(Span {
                offset,
                len: file.stream_position()? - offset,
            });
        }
        index += 1;
    }
    Ok((spans, atom_count.unwrap_or(0) as usize))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TrajBlock;
    use std::io::Write;

    fn dcd(atoms: usize, count: usize, big: bool) -> Vec<u8> {
        let word = |n: u32| {
            if big {
                n.to_be_bytes()
            } else {
                n.to_le_bytes()
            }
        };
        let mut out = Vec::new();
        let mut record = |bytes: &[u8]| {
            out.extend(word(bytes.len() as u32));
            out.extend(bytes);
            out.extend(word(bytes.len() as u32));
        };
        let mut header = [0; 84];
        header[..4].copy_from_slice(b"CORD");
        header[4..8].copy_from_slice(&word(count as u32));
        record(&header);
        record(&[0; 4]);
        record(&word(atoms as u32));
        for frame in 0..count {
            for axis in 0..3 {
                let bytes: Vec<_> = (0..atoms)
                    .flat_map(|atom| word(((frame * 100 + axis * 10 + atom) as f32).to_bits()))
                    .collect();
                record(&bytes);
            }
        }
        out
    }

    fn check(bytes: &[u8], expected: &TrajBlock) {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        let indexed = IndexedTrajectory::open(file.path(), 2, Some(expected.atom_count())).unwrap();
        assert_eq!(indexed.frame_count(), expected.frame_count().div_ceil(2));
        assert!(indexed.index_bytes() <= indexed.frame_count().max(4) * 32);
        for i in (0..indexed.frame_count()).rev() {
            let frame = indexed.read_frame(i).unwrap();
            assert_eq!(frame.coordinates, expected.frame(i * 2).unwrap());
            assert_eq!(frame.cell, expected.cell(i * 2));
        }
        assert!(indexed.read_frame(indexed.frame_count()).is_err());
        assert!(IndexedTrajectory::open(file.path(), 0, None).is_err());
        assert!(IndexedTrajectory::open(file.path(), 1, Some(expected.atom_count() + 1)).is_err());
    }

    #[test]
    fn dcd_seek_matches_eager_both_byte_orders() {
        for big in [false, true] {
            let bytes = dcd(12, 7, big);
            check(&bytes, &read_dcd_bytes(&bytes).unwrap());
        }
    }

    #[test]
    fn xtc_seek_matches_eager() {
        let mut bytes = Vec::new();
        for frame in 0..7 {
            for v in [1995_i32, 2, frame, 0] {
                bytes.extend(v.to_be_bytes());
            }
            for v in [1_f32, 0., 0., 0., 2., 0., 0., 0., 3.] {
                bytes.extend(v.to_be_bytes());
            }
            bytes.extend(2_i32.to_be_bytes());
            for atom in 0..6 {
                bytes.extend(((frame * 10 + atom) as f32).to_be_bytes());
            }
        }
        check(&bytes, &read_xtc_bytes(&bytes).unwrap());
    }

    #[test]
    fn trr_seek_matches_eager_in_both_precisions() {
        for scalar in [4_i32, 8] {
            let mut bytes = Vec::new();
            for frame in 0..7 {
                for v in [1993_i32, 4, 4] {
                    bytes.extend(v.to_be_bytes());
                }
                bytes.extend(b"test");
                for v in [
                    0,
                    0,
                    9 * scalar,
                    0,
                    0,
                    0,
                    0,
                    6 * scalar,
                    6 * scalar,
                    0,
                    2,
                    frame,
                    0,
                ] {
                    bytes.extend(v.to_be_bytes());
                }
                let mut real = |v: f64| {
                    if scalar == 4 {
                        bytes.extend((v as f32).to_be_bytes());
                    } else {
                        bytes.extend(v.to_be_bytes());
                    }
                };
                real(0.);
                real(0.);
                for v in [1., 0., 0., 0., 2., 0., 0., 0., 3.] {
                    real(v);
                }
                for atom in 0..12 {
                    real((frame * 10 + atom) as f64);
                }
            }
            check(&bytes, &read_trr_bytes(&bytes).unwrap());
        }
    }

    #[test]
    fn compressed_sources_are_spooled_and_removed() {
        let bytes = dcd(12, 7, false);
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&bytes).unwrap();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&enc.finish().unwrap()).unwrap();
        let indexed = IndexedTrajectory::open(file.path(), 1, None).unwrap();
        let temporary = indexed._temporary.as_ref().unwrap().to_path_buf();
        assert_eq!(indexed.read_frame(6).unwrap().coordinates[0], 600.);
        drop(indexed);
        assert!(!temporary.exists());
    }

    #[test]
    fn rejects_truncated_records_and_changed_files() {
        let bytes = dcd(12, 7, false);
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&bytes).unwrap();
        let indexed = IndexedTrajectory::open(file.path(), 1, None).unwrap();
        file.as_file_mut()
            .set_len((bytes.len() - 1) as u64)
            .unwrap();
        assert!(indexed.read_frame(0).is_err());
        assert!(IndexedTrajectory::open(file.path(), 1, None).is_err());
    }
}
