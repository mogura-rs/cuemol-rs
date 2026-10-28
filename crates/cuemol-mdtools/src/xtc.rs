//! GROMACS XTC trajectory reader.

use crate::trajectory::{TrajBlock, TrajBlockReader};
use crate::xdr::XdrInStream;
use cuemol_core::{CueMolError, Result};
use cuemol_qsys::{InOutHandler, ObjReader};
use std::path::Path;

const XTC_MAGIC: i32 = 1995;
const XTC_NEW_MAGIC: i32 = 2023;
const MAX_UNCOMPRESSED_ATOMS: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XtcReadOptions {
    pub skip_no: usize,
    /// Expected atom count from the associated topology.
    pub expected_atom_count: Option<usize>,
    /// File-order atom indices to retain. `None` loads every atom.
    pub atom_indices: Option<Vec<usize>>,
}

impl Default for XtcReadOptions {
    fn default() -> Self {
        Self {
            skip_no: 1,
            expected_atom_count: None,
            atom_indices: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XtcTrajReader {
    reader: ObjReader,
    block_reader: TrajBlockReader,
    options: XtcReadOptions,
}

impl XtcTrajReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("xtctraj", "GROMACS XTC trajectory (*.xtc)", "*.xtc"),
            block_reader: TrajBlockReader::with_lazy_load(true),
            options: XtcReadOptions::default(),
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut ObjReader {
        &mut self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn block_reader(&self) -> &TrajBlockReader {
        &self.block_reader
    }

    pub fn block_reader_mut(&mut self) -> &mut TrajBlockReader {
        &mut self.block_reader
    }

    pub fn lazy_load(&self) -> bool {
        self.block_reader.lazy_load()
    }

    pub fn is_lazy_load(&self) -> bool {
        self.lazy_load()
    }

    pub fn set_lazy_load(&mut self, value: bool) {
        self.block_reader.set_lazy_load(value);
    }

    pub fn target_trajectory_uid(&self) -> u64 {
        self.block_reader.target_trajectory_uid()
    }

    pub fn targ_traj_uid(&self) -> u64 {
        self.target_trajectory_uid()
    }

    pub fn get_targ_traj_uid(&self) -> u64 {
        self.target_trajectory_uid()
    }

    pub fn set_targ_traj_uid(&mut self, value: u64) {
        self.block_reader.set_targ_traj_uid(value);
    }

    pub fn set_target_trajectory_uid(&mut self, value: u64) {
        self.set_targ_traj_uid(value);
    }

    pub fn options(&self) -> &XtcReadOptions {
        &self.options
    }

    pub fn set_options(&mut self, value: XtcReadOptions) {
        self.options = value;
    }

    pub fn skip_no(&self) -> usize {
        self.options.skip_no
    }

    pub fn get_skip_no(&self) -> usize {
        self.skip_no()
    }

    pub fn nevery(&self) -> usize {
        self.skip_no()
    }

    pub fn set_skip_no(&mut self, value: usize) {
        self.options.skip_no = value;
    }

    pub fn set_nevery(&mut self, value: usize) {
        self.set_skip_no(value);
    }

    pub fn create_default_obj(&self) -> TrajBlock {
        TrajBlock::default()
    }

    pub fn can_handle_content(&self, input: &[u8]) -> bool {
        sniff_xtc_content(input)
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<TrajBlock> {
        read_xtc_bytes_with_options(input, &self.options)
    }

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<TrajBlock> {
        read_xtc_file_with_options(path, &self.options)
    }
}

impl Default for XtcTrajReader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for XtcTrajReader {
    type Target = ObjReader;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}

impl std::ops::DerefMut for XtcTrajReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reader
    }
}

pub fn sniff_xtc_content(input: &[u8]) -> bool {
    input
        .get(..4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(i32::from_be_bytes)
        .is_some_and(|magic| matches!(magic, XTC_MAGIC | XTC_NEW_MAGIC))
}

pub fn read_xtc_file(path: impl AsRef<Path>) -> Result<TrajBlock> {
    read_xtc_file_with_options(path, &XtcReadOptions::default())
}

pub fn read_xtc_file_with_options(
    path: impl AsRef<Path>,
    options: &XtcReadOptions,
) -> Result<TrajBlock> {
    let input = crate::fileio::read_bytes_maybe_compressed(path)?;
    read_xtc_bytes_with_options(&input, options)
}

pub fn read_xtc_bytes(input: &[u8]) -> Result<TrajBlock> {
    read_xtc_bytes_with_options(input, &XtcReadOptions::default())
}

pub fn read_xtc_bytes_with_options(input: &[u8], options: &XtcReadOptions) -> Result<TrajBlock> {
    if options.skip_no == 0 {
        return Err(CueMolError::invalid_argument(
            "XTC frame skip must be at least one",
        ));
    }

    let mut xdr = XdrInStream::new(input);
    let mut block = None;
    let mut file_atom_count = None;
    let mut atom_indices = Vec::new();
    let mut frame_index = 0_usize;

    while let Some(magic) = xdr.read_i32_optional()? {
        if !matches!(magic, XTC_MAGIC | XTC_NEW_MAGIC) {
            return Err(CueMolError::parse("XTC invalid frame magic"));
        }
        let long_format = magic == XTC_NEW_MAGIC;
        let natoms = positive_atom_count(xdr.read_i32()?)?;
        let _step = xdr.read_i32()?;
        let _time = xdr.read_f32()?;
        let cell = xdr.read_gmx_box(false)?;
        let repeated_count = positive_atom_count(xdr.read_i32()?)?;
        if repeated_count != natoms {
            return Err(CueMolError::parse("XTC contradictory atom count in frame"));
        }

        if let Some(expected) = options.expected_atom_count {
            if expected != natoms {
                return Err(CueMolError::parse(format!(
                    "XTC atom count {natoms} does not match expected {expected}"
                )));
            }
        }
        if let Some(first_count) = file_atom_count {
            if first_count != natoms {
                return Err(CueMolError::parse(
                    "XTC varying atom count is not supported",
                ));
            }
        } else {
            file_atom_count = Some(natoms);
            atom_indices = options
                .atom_indices
                .clone()
                .unwrap_or_else(|| (0..natoms).collect());
            if atom_indices.iter().any(|&index| index >= natoms) {
                return Err(CueMolError::invalid_argument(
                    "XTC atom selection index out of range",
                ));
            }
            block = Some(TrajBlock::new(atom_indices.len()));
        }

        let coordinates = if natoms <= MAX_UNCOMPRESSED_ATOMS {
            xdr.read_f32_array(natoms * 3)?
        } else {
            xdr.read_compressed_coordinates(natoms, long_format)?.0
        };
        if frame_index.is_multiple_of(options.skip_no) {
            let mut selected = Vec::with_capacity(atom_indices.len() * 3);
            for &atom_index in &atom_indices {
                let offset = atom_index * 3;
                selected.extend(
                    coordinates[offset..offset + 3]
                        .iter()
                        .map(|value| value * 10.0),
                );
            }
            block
                .as_mut()
                .expect("XTC block initialized")
                .append_frame(selected, Some(cell))?;
        }
        frame_index += 1;
    }

    block.ok_or_else(|| CueMolError::parse("XTC contains no frames"))
}

fn positive_atom_count(value: i32) -> Result<usize> {
    if value <= 0 {
        return Err(CueMolError::parse("XTC atom count must be positive"));
    }
    Ok(value as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_i32(output: &mut Vec<u8>, value: i32) {
        output.extend_from_slice(&value.to_be_bytes());
    }

    fn push_box(output: &mut Vec<u8>) {
        for value in [1.0_f32, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0] {
            output.extend_from_slice(&value.to_be_bytes());
        }
    }

    fn push_header(output: &mut Vec<u8>, magic: i32, natoms: usize, frame: usize) {
        push_i32(output, magic);
        push_i32(output, natoms as i32);
        push_i32(output, frame as i32);
        output.extend_from_slice(&(frame as f32 * 0.5).to_be_bytes());
        push_box(output);
        push_i32(output, natoms as i32);
    }

    fn uncompressed_fixture() -> Vec<u8> {
        let mut output = Vec::new();
        for frame in 0..3 {
            push_header(&mut output, XTC_MAGIC, 3, frame);
            for atom in 0..3 {
                for axis in 0..3 {
                    let value = (frame * 100 + atom * 10 + axis) as f32 / 10.0;
                    output.extend_from_slice(&value.to_be_bytes());
                }
            }
        }
        output
    }

    fn compressed_fixture(magic: i32) -> Vec<u8> {
        let mut output = Vec::new();
        push_header(&mut output, magic, 10, 0);
        output.extend_from_slice(&1000.0_f32.to_be_bytes());
        for value in [0, 0, 0, 9, 0, 0] {
            push_i32(&mut output, value);
        }
        push_i32(&mut output, 9);

        let mut bits = BitWriter::default();
        encode_mixed(&mut bits, [1, 0, 0], [10, 1, 1], 4);
        bits.push(1, 1);
        bits.push(4, 5);
        encode_mixed(&mut bits, [3, 4, 4], [8, 8, 8], 9);
        encode_mixed(&mut bits, [2, 0, 0], [10, 1, 1], 4);
        bits.push(1, 1);
        bits.push(1, 5);
        for atom in 3..10 {
            encode_mixed(&mut bits, [atom, 0, 0], [10, 1, 1], 4);
            bits.push(0, 1);
        }
        let bytes = bits.finish();
        if magic == XTC_NEW_MAGIC {
            push_i32(&mut output, 0);
        }
        push_i32(&mut output, bytes.len() as i32);
        output.extend_from_slice(&bytes);
        output.resize(output.len().next_multiple_of(4), 0);
        output
    }

    #[test]
    fn reads_uncompressed_frames_with_sampling_and_atom_selection() {
        let input = uncompressed_fixture();
        assert!(sniff_xtc_content(&input));
        let block = read_xtc_bytes_with_options(
            &input,
            &XtcReadOptions {
                skip_no: 2,
                expected_atom_count: Some(3),
                atom_indices: Some(vec![2, 0]),
            },
        )
        .unwrap();
        assert_eq!(block.atom_count(), 2);
        assert_eq!(block.frame_count(), 2);
        assert_eq!(
            block.frame(1).unwrap(),
            &[220.0, 221.0, 222.0, 200.0, 201.0, 202.0]
        );
        assert_eq!(block.cell(0), Some([10.0, 20.0, 30.0, 90.0, 90.0, 90.0]));
    }

    #[test]
    fn decompresses_classic_and_long_format_coordinate_blocks() {
        for magic in [XTC_MAGIC, XTC_NEW_MAGIC] {
            let block = read_xtc_bytes(&compressed_fixture(magic)).unwrap();
            assert_eq!(block.atom_count(), 10);
            assert_eq!(block.frame_count(), 1);
            let frame = block.frame(0).unwrap();
            for atom in 0..10 {
                assert!((frame[atom * 3] - atom as f32 / 100.0).abs() < 1.0e-6);
                assert_eq!(&frame[atom * 3 + 1..atom * 3 + 3], &[0.0, 0.0]);
            }
        }
    }

    #[test]
    fn rejects_bad_counts_truncation_and_invalid_options() {
        let input = uncompressed_fixture();
        assert!(read_xtc_bytes(&input[..input.len() - 1]).is_err());

        let mut contradictory = input.clone();
        contradictory[55] = 4;
        assert!(read_xtc_bytes(&contradictory).is_err());
        assert!(read_xtc_bytes_with_options(
            &input,
            &XtcReadOptions {
                skip_no: 0,
                ..XtcReadOptions::default()
            }
        )
        .is_err());
        assert!(read_xtc_bytes_with_options(
            &input,
            &XtcReadOptions {
                expected_atom_count: Some(4),
                ..XtcReadOptions::default()
            }
        )
        .is_err());
    }

    #[derive(Debug, Default)]
    struct BitWriter {
        bytes: Vec<u8>,
        bit_offset: usize,
    }

    impl BitWriter {
        fn push(&mut self, value: u64, count: u32) {
            for shift in (0..count).rev() {
                if self.bit_offset.is_multiple_of(8) {
                    self.bytes.push(0);
                }
                let bit = ((value >> shift) & 1) as u8;
                let output_shift = 7 - self.bit_offset % 8;
                *self.bytes.last_mut().unwrap() |= bit << output_shift;
                self.bit_offset += 1;
            }
        }

        fn finish(self) -> Vec<u8> {
            self.bytes
        }
    }

    fn encode_mixed(bits: &mut BitWriter, values: [u32; 3], sizes: [u32; 3], bit_count: u32) {
        let value = u64::from(values[0]) * u64::from(sizes[1]) * u64::from(sizes[2])
            + u64::from(values[1]) * u64::from(sizes[2])
            + u64::from(values[2]);
        let mut remaining = bit_count;
        let mut byte_index = 0;
        while remaining >= 8 {
            bits.push((value >> (byte_index * 8)) & 0xff, 8);
            byte_index += 1;
            remaining -= 8;
        }
        if remaining > 0 {
            bits.push(value >> (byte_index * 8), remaining);
        }
    }
}
