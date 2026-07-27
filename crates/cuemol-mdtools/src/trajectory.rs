//! Molecular-dynamics trajectory blocks and DCD reader.
//!
//! This ports the block-centric `mdtools::Trajectory`, `TrajBlock`, and
//! `DCDTrajReader` path. DCD frames are read eagerly, matching the current C++
//! implementation.

use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_molstr::MolCoord;
use cuemol_qsys::{InOutHandler, ObjReader};
use std::path::Path;

/// One contiguous group of trajectory coordinate frames.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrajBlock {
    start_index: usize,
    atom_count: usize,
    frames: Vec<Vec<f32>>,
    cells: Vec<Option<[f64; 6]>>,
}

impl TrajBlock {
    pub const CELL_SIZE: usize = 6;

    pub fn new(atom_count: usize) -> Self {
        Self {
            atom_count,
            ..Self::default()
        }
    }

    pub fn allocate(atom_count: usize, frame_count: usize) -> Self {
        Self {
            start_index: 0,
            atom_count,
            frames: vec![vec![0.0; atom_count.saturating_mul(3)]; frame_count],
            cells: vec![None; frame_count],
        }
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.cells.clear();
    }

    pub fn append_frame(&mut self, coordinates: Vec<f32>, cell: Option<[f64; 6]>) -> Result<()> {
        let expected = self.atom_count.saturating_mul(3);
        if coordinates.len() != expected {
            return Err(CueMolError::invalid_argument(format!(
                "trajectory frame has {} coordinates, expected {expected}",
                coordinates.len()
            )));
        }
        self.frames.push(coordinates);
        self.cells.push(cell);
        Ok(())
    }

    pub fn start_index(&self) -> usize {
        self.start_index
    }

    pub fn set_start_index(&mut self, value: usize) {
        self.start_index = value;
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn get_size(&self) -> usize {
        self.frame_count()
    }

    pub fn atom_count(&self) -> usize {
        self.atom_count
    }

    pub fn coordinate_count(&self) -> usize {
        self.atom_count.saturating_mul(3)
    }

    pub fn get_crd_size(&self) -> usize {
        self.coordinate_count()
    }

    pub fn frame(&self, index: usize) -> Option<&[f32]> {
        self.frames.get(index).map(Vec::as_slice)
    }

    pub fn frame_mut(&mut self, index: usize) -> Option<&mut [f32]> {
        self.frames.get_mut(index).map(Vec::as_mut_slice)
    }

    pub fn cell(&self, index: usize) -> Option<[f64; 6]> {
        self.cells.get(index).copied().flatten()
    }

    pub fn is_all_loaded(&self) -> bool {
        true
    }
}

/// Molecule plus one or more trajectory blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct Trajectory {
    molecule: MolCoord,
    blocks: Vec<TrajBlock>,
    current_frame: usize,
    frame_average_size: usize,
    total_frames: usize,
}

impl Trajectory {
    pub fn new(molecule: MolCoord) -> Self {
        Self {
            molecule,
            blocks: Vec::new(),
            current_frame: 0,
            frame_average_size: 0,
            total_frames: 0,
        }
    }

    pub fn molecule(&self) -> &MolCoord {
        &self.molecule
    }

    pub fn molecule_mut(&mut self) -> &mut MolCoord {
        &mut self.molecule
    }

    pub fn append(&mut self, block: TrajBlock) -> Result<()> {
        self.validate_block(&block)?;
        self.blocks.push(block);
        self.recompute_block_layout();
        if self.blocks.len() == 1 && self.total_frames > 0 {
            self.set_frame(0)?;
        }
        Ok(())
    }

    pub fn insert_block(&mut self, index: usize, block: TrajBlock) -> Result<()> {
        if index > self.blocks.len() {
            return Err(CueMolError::invalid_argument(
                "trajectory block index out of range",
            ));
        }
        self.validate_block(&block)?;
        self.blocks.insert(index, block);
        self.recompute_block_layout();
        self.refresh_current_frame()
    }

    pub fn remove_block(&mut self, index: usize) -> Result<TrajBlock> {
        if index >= self.blocks.len() {
            return Err(CueMolError::invalid_argument(
                "trajectory block index out of range",
            ));
        }
        let block = self.blocks.remove(index);
        self.recompute_block_layout();
        self.refresh_current_frame()?;
        Ok(block)
    }

    pub fn move_block(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.blocks.len() || to >= self.blocks.len() {
            return Err(CueMolError::invalid_argument(
                "trajectory block index out of range",
            ));
        }
        if from == to {
            return Ok(());
        }
        let block = self.blocks.remove(from);
        self.blocks.insert(to, block);
        self.recompute_block_layout();
        self.refresh_current_frame()
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn get_block_count(&self) -> usize {
        self.block_count()
    }

    pub fn block(&self, index: usize) -> Option<&TrajBlock> {
        self.blocks.get(index)
    }

    pub fn frame(&self) -> usize {
        self.current_frame
    }

    pub fn get_frame(&self) -> usize {
        self.frame()
    }

    pub fn frame_count(&self) -> usize {
        self.total_frames
    }

    pub fn get_frame_size(&self) -> usize {
        self.frame_count()
    }

    pub fn frame_average_size(&self) -> usize {
        self.frame_average_size
    }

    pub fn set_frame_average_size(&mut self, value: usize) {
        self.frame_average_size = value;
    }

    pub fn set_frame(&mut self, index: usize) -> Result<()> {
        let coordinates = self.coordinates_for_frame(index)?;
        for (atom, xyz) in self
            .molecule
            .atoms_mut()
            .iter_mut()
            .zip(coordinates.chunks_exact(3))
        {
            atom.position = Vector4::new3(xyz[0] as f64, xyz[1] as f64, xyz[2] as f64);
        }
        self.current_frame = index;
        self.molecule.fire_atoms_moved();
        Ok(())
    }

    pub fn set_dyn_frame(&mut self, index: usize) -> Result<()> {
        self.set_frame(index)
    }

    pub fn coordinates_for_frame(&self, index: usize) -> Result<Vec<f32>> {
        if index >= self.total_frames {
            return Err(CueMolError::invalid_argument(
                "trajectory frame index out of range",
            ));
        }
        if self.frame_average_size == 0 {
            return Ok(self.frame_slice(index)?.to_vec());
        }

        let start = index.saturating_sub(self.frame_average_size);
        let end = (index + self.frame_average_size).min(self.total_frames - 1);
        let mut average = vec![0.0; self.molecule.atom_count().saturating_mul(3)];
        let mut count = 0_usize;
        for frame_index in start..=end {
            for (sum, value) in average.iter_mut().zip(self.frame_slice(frame_index)?) {
                *sum += *value;
            }
            count += 1;
        }
        let scale = 1.0 / count as f32;
        for value in &mut average {
            *value *= scale;
        }
        Ok(average)
    }

    fn validate_block(&self, block: &TrajBlock) -> Result<()> {
        if block.atom_count() != self.molecule.atom_count() {
            return Err(CueMolError::invalid_argument(format!(
                "trajectory block atom count {} does not match molecule atom count {}",
                block.atom_count(),
                self.molecule.atom_count()
            )));
        }
        Ok(())
    }

    fn recompute_block_layout(&mut self) {
        let mut start = 0;
        for block in &mut self.blocks {
            block.set_start_index(start);
            start += block.frame_count();
        }
        self.total_frames = start;
        if self.total_frames == 0 {
            self.current_frame = 0;
        } else {
            self.current_frame = self.current_frame.min(self.total_frames - 1);
        }
    }

    fn refresh_current_frame(&mut self) -> Result<()> {
        if self.total_frames == 0 {
            self.current_frame = 0;
            return Ok(());
        }
        self.set_frame(self.current_frame.min(self.total_frames - 1))
    }

    fn frame_slice(&self, index: usize) -> Result<&[f32]> {
        for block in &self.blocks {
            let end = block.start_index() + block.frame_count();
            if (block.start_index()..end).contains(&index) {
                return block
                    .frame(index - block.start_index())
                    .ok_or_else(|| CueMolError::not_found("trajectory frame"));
            }
        }
        Err(CueMolError::not_found("trajectory frame"))
    }
}

impl std::ops::Deref for Trajectory {
    type Target = MolCoord;

    fn deref(&self) -> &Self::Target {
        &self.molecule
    }
}

impl std::ops::DerefMut for Trajectory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.molecule
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcdReadOptions {
    pub skip_no: usize,
    /// Expected number of atoms in the DCD file.
    pub expected_atom_count: Option<usize>,
    /// File-order atom indices to retain. `None` loads every atom.
    pub atom_indices: Option<Vec<usize>>,
}

impl Default for DcdReadOptions {
    fn default() -> Self {
        Self {
            skip_no: 1,
            expected_atom_count: None,
            atom_indices: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcdTrajReader {
    reader: ObjReader,
    options: DcdReadOptions,
}

impl DcdTrajReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("dcdtraj", "DCD binary trajectory (*.dcd)", "*.dcd"),
            options: DcdReadOptions::default(),
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

    pub fn options(&self) -> &DcdReadOptions {
        &self.options
    }

    pub fn set_options(&mut self, value: DcdReadOptions) {
        self.options = value;
    }

    pub fn skip_no(&self) -> usize {
        self.options.skip_no
    }

    pub fn get_skip_no(&self) -> usize {
        self.skip_no()
    }

    pub fn set_skip_no(&mut self, value: usize) {
        self.options.skip_no = value;
    }

    pub fn create_default_obj(&self) -> TrajBlock {
        TrajBlock::default()
    }

    pub fn can_handle_content(&self, input: &[u8]) -> bool {
        sniff_dcd_content(input)
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<TrajBlock> {
        read_dcd_bytes_with_options(input, &self.options)
    }

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<TrajBlock> {
        read_dcd_file_with_options(path, &self.options)
    }
}

impl Default for DcdTrajReader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for DcdTrajReader {
    type Target = ObjReader;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}

impl std::ops::DerefMut for DcdTrajReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reader
    }
}

pub fn sniff_dcd_content(input: &[u8]) -> bool {
    input.len() >= 12
        && (decode_u32(&input[..4], Endian::Little) == 84
            || decode_u32(&input[..4], Endian::Big) == 84)
        && input.get(4..8) == Some(b"CORD".as_slice())
}

pub fn read_dcd_file(path: impl AsRef<Path>) -> Result<TrajBlock> {
    read_dcd_file_with_options(path, &DcdReadOptions::default())
}

pub fn read_dcd_file_with_options(
    path: impl AsRef<Path>,
    options: &DcdReadOptions,
) -> Result<TrajBlock> {
    let input = crate::fileio::read_bytes_maybe_compressed(path)?;
    read_dcd_bytes_with_options(&input, options)
}

pub fn read_dcd_bytes(input: &[u8]) -> Result<TrajBlock> {
    read_dcd_bytes_with_options(input, &DcdReadOptions::default())
}

pub fn read_dcd_bytes_with_options(input: &[u8], options: &DcdReadOptions) -> Result<TrajBlock> {
    if options.skip_no == 0 {
        return Err(CueMolError::invalid_argument(
            "DCD frame skip must be at least one",
        ));
    }
    let endian = detect_endian(input)?;
    let mut records = FortranRecords::new(input, endian);

    let header = records.next_record()?;
    if header.len() != 84 {
        return Err(CueMolError::parse(format!(
            "DCD invalid header length {} != 84",
            header.len()
        )));
    }
    if &header[..4] != b"CORD" {
        return Err(CueMolError::parse("DCD CORD marker is missing"));
    }
    let frame_count = decode_i32(&header[4..8], endian);
    if frame_count <= 0 {
        return Err(CueMolError::parse("DCD contains no frames"));
    }
    let has_cell = decode_i32(&header[44..48], endian) != 0;

    let _title = records.next_record()?;
    let atom_record = records.next_record()?;
    if atom_record.len() != 4 {
        return Err(CueMolError::parse(format!(
            "DCD invalid NATOM record length {} != 4",
            atom_record.len()
        )));
    }
    let atom_count = decode_i32(atom_record, endian);
    if atom_count <= 0 {
        return Err(CueMolError::parse("DCD invalid atom count"));
    }
    let atom_count = atom_count as usize;
    if let Some(expected) = options.expected_atom_count {
        if expected != atom_count {
            return Err(CueMolError::parse(format!(
                "DCD atom count {atom_count} does not match expected {expected}"
            )));
        }
    }

    let atom_indices = options
        .atom_indices
        .clone()
        .unwrap_or_else(|| (0..atom_count).collect());
    if atom_indices.iter().any(|&index| index >= atom_count) {
        return Err(CueMolError::invalid_argument(
            "DCD atom selection index out of range",
        ));
    }

    let kept_frames = (frame_count as usize).div_ceil(options.skip_no);
    let mut block = TrajBlock::new(atom_indices.len());
    block.frames.reserve(kept_frames);
    block.cells.reserve(kept_frames);

    let coordinate_bytes = atom_count
        .checked_mul(4)
        .ok_or_else(|| CueMolError::parse("DCD coordinate record size overflow"))?;
    for frame_index in 0..frame_count as usize {
        let cell = if has_cell {
            let record = records.next_record()?;
            if record.len() != 48 {
                return Err(CueMolError::parse(format!(
                    "DCD invalid cell record length {} != 48",
                    record.len()
                )));
            }
            let mut values = [0.0; 6];
            for (index, value) in values.iter_mut().enumerate() {
                *value = decode_f64(&record[index * 8..index * 8 + 8], endian);
            }
            Some(values)
        } else {
            None
        };

        let mut axes = [Vec::new(), Vec::new(), Vec::new()];
        for values in &mut axes {
            let record = records.next_record()?;
            if record.len() != coordinate_bytes {
                return Err(CueMolError::parse(format!(
                    "DCD invalid coordinate record length {} != {coordinate_bytes}",
                    record.len()
                )));
            }
            values.reserve(atom_count);
            for value in record.chunks_exact(4) {
                values.push(decode_f32(value, endian));
            }
        }

        if frame_index % options.skip_no != 0 {
            continue;
        }
        let mut coordinates = Vec::with_capacity(atom_indices.len() * 3);
        for &atom_index in &atom_indices {
            coordinates.push(axes[0][atom_index]);
            coordinates.push(axes[1][atom_index]);
            coordinates.push(axes[2][atom_index]);
        }
        block.append_frame(coordinates, cell)?;
    }
    Ok(block)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

fn detect_endian(input: &[u8]) -> Result<Endian> {
    let marker = input
        .get(..4)
        .ok_or_else(|| CueMolError::parse("DCD input is truncated"))?;
    if decode_u32(marker, Endian::Little) == 84 {
        Ok(Endian::Little)
    } else if decode_u32(marker, Endian::Big) == 84 {
        Ok(Endian::Big)
    } else {
        Err(CueMolError::parse("DCD 84-byte header record not found"))
    }
}

struct FortranRecords<'a> {
    input: &'a [u8],
    cursor: usize,
    endian: Endian,
}

impl<'a> FortranRecords<'a> {
    fn new(input: &'a [u8], endian: Endian) -> Self {
        Self {
            input,
            cursor: 0,
            endian,
        }
    }

    fn next_record(&mut self) -> Result<&'a [u8]> {
        let marker = self
            .input
            .get(self.cursor..self.cursor + 4)
            .ok_or_else(|| CueMolError::parse("DCD record marker is truncated"))?;
        let size = decode_u32(marker, self.endian) as usize;
        let payload_start = self.cursor + 4;
        let payload_end = payload_start
            .checked_add(size)
            .ok_or_else(|| CueMolError::parse("DCD record size overflow"))?;
        let payload = self
            .input
            .get(payload_start..payload_end)
            .ok_or_else(|| CueMolError::parse("DCD record payload is truncated"))?;
        let trailing = self
            .input
            .get(payload_end..payload_end + 4)
            .ok_or_else(|| CueMolError::parse("DCD trailing record marker is truncated"))?;
        let trailing_size = decode_u32(trailing, self.endian) as usize;
        if trailing_size != size {
            return Err(CueMolError::parse(format!(
                "DCD Fortran record marker mismatch {size} != {trailing_size}"
            )));
        }
        self.cursor = payload_end + 4;
        Ok(payload)
    }
}

fn decode_u32(input: &[u8], endian: Endian) -> u32 {
    let bytes: [u8; 4] = input[..4].try_into().expect("four-byte value");
    match endian {
        Endian::Little => u32::from_le_bytes(bytes),
        Endian::Big => u32::from_be_bytes(bytes),
    }
}

fn decode_i32(input: &[u8], endian: Endian) -> i32 {
    decode_u32(input, endian) as i32
}

fn decode_f32(input: &[u8], endian: Endian) -> f32 {
    f32::from_bits(decode_u32(input, endian))
}

fn decode_f64(input: &[u8], endian: Endian) -> f64 {
    let bytes: [u8; 8] = input[..8].try_into().expect("eight-byte value");
    match endian {
        Endian::Little => f64::from_le_bytes(bytes),
        Endian::Big => f64::from_be_bytes(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_molstr::Atom;

    fn atom(serial: i32) -> Atom {
        Atom {
            serial,
            name: format!("C{serial}"),
            alt_loc: None,
            res_name: "MOL".to_string(),
            chain_id: "A".to_string(),
            res_id: 1,
            ins_code: None,
            position: Vector4::new3(0.0, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "C".to_string(),
            formal_charge: None,
            int_props: Default::default(),
            real_props: Default::default(),
        }
    }

    fn encode_u32(value: u32, endian: Endian) -> [u8; 4] {
        match endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        }
    }

    fn encode_f32(value: f32, endian: Endian) -> [u8; 4] {
        encode_u32(value.to_bits(), endian)
    }

    fn encode_f64(value: f64, endian: Endian) -> [u8; 8] {
        match endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        }
    }

    fn push_record(output: &mut Vec<u8>, payload: &[u8], endian: Endian) {
        output.extend_from_slice(&encode_u32(payload.len() as u32, endian));
        output.extend_from_slice(payload);
        output.extend_from_slice(&encode_u32(payload.len() as u32, endian));
    }

    fn dcd_fixture(endian: Endian, with_cell: bool) -> Vec<u8> {
        let mut output = Vec::new();
        let mut header = vec![0_u8; 84];
        header[..4].copy_from_slice(b"CORD");
        header[4..8].copy_from_slice(&encode_u32(3, endian));
        header[44..48].copy_from_slice(&encode_u32(u32::from(with_cell), endian));
        push_record(&mut output, &header, endian);

        let mut title = Vec::new();
        title.extend_from_slice(&encode_u32(1, endian));
        let mut text = [b' '; 80];
        text[..14].copy_from_slice(b"CueMol fixture");
        title.extend_from_slice(&text);
        push_record(&mut output, &title, endian);
        push_record(&mut output, &encode_u32(3, endian), endian);

        for frame in 0..3 {
            if with_cell {
                let mut cell = Vec::new();
                for value in [10.0 + frame as f64, 0.0, 11.0, 0.0, 0.0, 12.0] {
                    cell.extend_from_slice(&encode_f64(value, endian));
                }
                push_record(&mut output, &cell, endian);
            }
            for axis in 0..3 {
                let mut values = Vec::new();
                for atom in 0..3 {
                    values.extend_from_slice(&encode_f32(
                        (frame * 100 + axis * 10 + atom) as f32,
                        endian,
                    ));
                }
                push_record(&mut output, &values, endian);
            }
        }
        output
    }

    #[test]
    fn dcd_reader_reads_cell_skips_frames_and_selects_atoms() {
        let input = dcd_fixture(Endian::Little, true);
        assert!(sniff_dcd_content(&input));
        let block = read_dcd_bytes_with_options(
            &input,
            &DcdReadOptions {
                skip_no: 2,
                expected_atom_count: Some(3),
                atom_indices: Some(vec![2, 0]),
            },
        )
        .unwrap();
        assert_eq!(block.atom_count(), 2);
        assert_eq!(block.frame_count(), 2);
        assert_eq!(block.frame(0).unwrap(), &[2.0, 12.0, 22.0, 0.0, 10.0, 20.0]);
        assert_eq!(
            block.frame(1).unwrap(),
            &[202.0, 212.0, 222.0, 200.0, 210.0, 220.0]
        );
        assert_eq!(block.cell(0), Some([10.0, 0.0, 11.0, 0.0, 0.0, 12.0]));
        assert_eq!(block.cell(1), Some([12.0, 0.0, 11.0, 0.0, 0.0, 12.0]));
    }

    #[test]
    fn dcd_reader_accepts_big_endian_without_cell() {
        let input = dcd_fixture(Endian::Big, false);
        let block = read_dcd_bytes(&input).unwrap();
        assert_eq!(block.frame_count(), 3);
        assert_eq!(block.atom_count(), 3);
        assert_eq!(block.frame(2).unwrap()[8], 222.0);
        assert_eq!(block.cell(0), None);
    }

    #[test]
    fn dcd_reader_rejects_bad_record_markers_and_atom_counts() {
        let mut input = dcd_fixture(Endian::Little, false);
        let last = input.len() - 1;
        input[last] = 9;
        assert!(read_dcd_bytes(&input).is_err());

        let input = dcd_fixture(Endian::Little, false);
        assert!(read_dcd_bytes_with_options(
            &input,
            &DcdReadOptions {
                expected_atom_count: Some(4),
                ..DcdReadOptions::default()
            }
        )
        .is_err());
    }

    #[test]
    fn trajectory_applies_frames_averages_and_reorders_blocks() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1));
        molecule.push_atom(atom(2));

        let mut first = TrajBlock::new(2);
        first
            .append_frame(vec![0.0, 0.0, 0.0, 2.0, 0.0, 0.0], None)
            .unwrap();
        first
            .append_frame(vec![2.0, 0.0, 0.0, 4.0, 0.0, 0.0], None)
            .unwrap();
        let mut second = TrajBlock::new(2);
        second
            .append_frame(vec![4.0, 0.0, 0.0, 6.0, 0.0, 0.0], None)
            .unwrap();

        let mut trajectory = Trajectory::new(molecule);
        trajectory.append(first).unwrap();
        trajectory.append(second).unwrap();
        assert_eq!(trajectory.frame_count(), 3);
        assert_eq!(trajectory.block(1).unwrap().start_index(), 2);

        trajectory.set_frame_average_size(1);
        trajectory.set_frame(1).unwrap();
        assert_eq!(trajectory.atoms()[0].position.x, 2.0);
        assert_eq!(trajectory.atoms()[1].position.x, 4.0);

        trajectory.set_frame_average_size(0);
        trajectory.move_block(1, 0).unwrap();
        assert_eq!(trajectory.block(1).unwrap().start_index(), 1);
        assert_eq!(trajectory.atoms()[0].position.x, 0.0);

        let removed = trajectory.remove_block(0).unwrap();
        assert_eq!(removed.frame_count(), 1);
        assert_eq!(trajectory.frame_count(), 2);
    }
}
