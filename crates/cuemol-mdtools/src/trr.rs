//! GROMACS TRR trajectory reader.

use crate::trajectory::TrajBlock;
use crate::xdr::XdrInStream;
use cuemol_core::{CueMolError, Result};
use cuemol_qsys::{InOutHandler, ObjReader};
use std::path::Path;

const TRR_MAGIC: i32 = 1993;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrrReadOptions {
    pub skip_no: usize,
    /// Expected atom count from the associated topology.
    pub expected_atom_count: Option<usize>,
    /// File-order atom indices to retain. `None` loads every atom.
    pub atom_indices: Option<Vec<usize>>,
}

impl Default for TrrReadOptions {
    fn default() -> Self {
        Self {
            skip_no: 1,
            expected_atom_count: None,
            atom_indices: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrrTrajReader {
    reader: ObjReader,
    options: TrrReadOptions,
}

impl TrrTrajReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("trrtraj", "GROMACS TRR trajectory (*.trr)", "*.trr"),
            options: TrrReadOptions::default(),
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

    pub fn options(&self) -> &TrrReadOptions {
        &self.options
    }

    pub fn set_options(&mut self, value: TrrReadOptions) {
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
        sniff_trr_content(input)
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<TrajBlock> {
        read_trr_bytes_with_options(input, &self.options)
    }

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<TrajBlock> {
        read_trr_file_with_options(path, &self.options)
    }
}

impl Default for TrrTrajReader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for TrrTrajReader {
    type Target = ObjReader;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}

impl std::ops::DerefMut for TrrTrajReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reader
    }
}

pub fn sniff_trr_content(input: &[u8]) -> bool {
    input
        .get(..4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(i32::from_be_bytes)
        == Some(TRR_MAGIC)
}

pub fn read_trr_file(path: impl AsRef<Path>) -> Result<TrajBlock> {
    read_trr_file_with_options(path, &TrrReadOptions::default())
}

pub fn read_trr_file_with_options(
    path: impl AsRef<Path>,
    options: &TrrReadOptions,
) -> Result<TrajBlock> {
    let input = crate::fileio::read_bytes_maybe_compressed(path)?;
    read_trr_bytes_with_options(&input, options)
}

pub fn read_trr_bytes(input: &[u8]) -> Result<TrajBlock> {
    read_trr_bytes_with_options(input, &TrrReadOptions::default())
}

pub fn read_trr_bytes_with_options(input: &[u8], options: &TrrReadOptions) -> Result<TrajBlock> {
    if options.skip_no == 0 {
        return Err(CueMolError::invalid_argument(
            "TRR frame skip must be at least one",
        ));
    }

    let mut xdr = XdrInStream::new(input);
    let mut block = None;
    let mut file_atom_count = None;
    let mut atom_indices = Vec::new();
    let mut frame_index = 0_usize;

    while let Some(magic) = xdr.read_i32_optional()? {
        if magic != TRR_MAGIC {
            return Err(CueMolError::parse("TRR invalid frame magic"));
        }

        let _version = xdr.read_gmx_string()?;
        let sizes = TrrBlockSizes::read(&mut xdr)?;
        let natoms = positive_usize(xdr.read_i32()?, "TRR atom count")?;
        let _step = xdr.read_i32()?;
        let _nre = xdr.read_i32()?;
        let precision = sizes.precision(natoms)?;

        match precision {
            TrrPrecision::Single => {
                let _time = xdr.read_f32()?;
                let _lambda = xdr.read_f32()?;
            }
            TrrPrecision::Double => {
                let _time = xdr.read_f64()?;
                let _lambda = xdr.read_f64()?;
            }
        }

        if let Some(expected) = options.expected_atom_count {
            if expected != natoms {
                return Err(CueMolError::parse(format!(
                    "TRR atom count {natoms} does not match expected {expected}"
                )));
            }
        }
        if let Some(first_count) = file_atom_count {
            if first_count != natoms {
                return Err(CueMolError::parse(
                    "TRR varying atom count is not supported",
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
                    "TRR atom selection index out of range",
                ));
            }
            block = Some(TrajBlock::new(atom_indices.len()));
        }

        // These obsolete blocks occur before the box in the TRR layout.
        xdr.skip_bytes(sizes.ir)?;
        xdr.skip_bytes(sizes.energy)?;
        let cell = if sizes.box_bytes > 0 {
            Some(xdr.read_gmx_box(precision == TrrPrecision::Double)?)
        } else {
            None
        };
        xdr.skip_bytes(sizes.virial)?;
        xdr.skip_bytes(sizes.pressure)?;
        xdr.skip_bytes(sizes.topology)?;
        xdr.skip_bytes(sizes.symmetry)?;

        let coordinates = if sizes.coordinates > 0 {
            Some(match precision {
                TrrPrecision::Single => xdr.read_f32_array(natoms * 3)?,
                TrrPrecision::Double => xdr.read_f64_array_as_f32(natoms * 3)?,
            })
        } else {
            None
        };
        xdr.skip_bytes(sizes.velocities)?;
        xdr.skip_bytes(sizes.forces)?;

        if frame_index.is_multiple_of(options.skip_no) {
            if let Some(coordinates) = coordinates {
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
                    .expect("TRR block initialized")
                    .append_frame(selected, cell)?;
            }
        }
        frame_index += 1;
    }

    block.ok_or_else(|| CueMolError::parse("TRR contains no frames"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrrPrecision {
    Single,
    Double,
}

#[derive(Debug, Clone, Copy)]
struct TrrBlockSizes {
    ir: usize,
    energy: usize,
    box_bytes: usize,
    virial: usize,
    pressure: usize,
    topology: usize,
    symmetry: usize,
    coordinates: usize,
    velocities: usize,
    forces: usize,
}

impl TrrBlockSizes {
    fn read(xdr: &mut XdrInStream<'_>) -> Result<Self> {
        Ok(Self {
            ir: block_size(xdr.read_i32()?)?,
            energy: block_size(xdr.read_i32()?)?,
            box_bytes: block_size(xdr.read_i32()?)?,
            virial: block_size(xdr.read_i32()?)?,
            pressure: block_size(xdr.read_i32()?)?,
            topology: block_size(xdr.read_i32()?)?,
            symmetry: block_size(xdr.read_i32()?)?,
            coordinates: block_size(xdr.read_i32()?)?,
            velocities: block_size(xdr.read_i32()?)?,
            forces: block_size(xdr.read_i32()?)?,
        })
    }

    fn precision(&self, natoms: usize) -> Result<TrrPrecision> {
        let coordinate_count = natoms
            .checked_mul(3)
            .ok_or_else(|| CueMolError::parse("TRR coordinate count overflow"))?;
        let scalar_bytes = if self.box_bytes > 0 {
            exact_scalar_size(self.box_bytes, 9, "box")?
        } else if self.coordinates > 0 {
            exact_scalar_size(self.coordinates, coordinate_count, "coordinates")?
        } else if self.velocities > 0 {
            exact_scalar_size(self.velocities, coordinate_count, "velocities")?
        } else if self.forces > 0 {
            exact_scalar_size(self.forces, coordinate_count, "forces")?
        } else {
            return Err(CueMolError::parse("TRR cannot determine precision"));
        };

        let precision = match scalar_bytes {
            4 => TrrPrecision::Single,
            8 => TrrPrecision::Double,
            _ => return Err(CueMolError::parse("TRR cannot determine precision")),
        };
        self.validate_known_blocks(coordinate_count, scalar_bytes)?;
        Ok(precision)
    }

    fn validate_known_blocks(&self, coordinate_count: usize, scalar_bytes: usize) -> Result<()> {
        for (label, actual, count) in [
            ("box", self.box_bytes, 9),
            ("virial", self.virial, 9),
            ("pressure", self.pressure, 9),
            ("coordinates", self.coordinates, coordinate_count),
            ("velocities", self.velocities, coordinate_count),
            ("forces", self.forces, coordinate_count),
        ] {
            if actual == 0 {
                continue;
            }
            let expected = count
                .checked_mul(scalar_bytes)
                .ok_or_else(|| CueMolError::parse("TRR block size overflow"))?;
            if actual != expected {
                return Err(CueMolError::parse(format!(
                    "TRR invalid {label} block size {actual} != {expected}"
                )));
            }
        }
        Ok(())
    }
}

fn block_size(value: i32) -> Result<usize> {
    usize::try_from(value).map_err(|_| CueMolError::parse("TRR block size is negative"))
}

fn positive_usize(value: i32, label: &str) -> Result<usize> {
    if value <= 0 {
        return Err(CueMolError::parse(format!("{label} must be positive")));
    }
    Ok(value as usize)
}

fn exact_scalar_size(bytes: usize, count: usize, label: &str) -> Result<usize> {
    if count == 0 || !bytes.is_multiple_of(count) {
        return Err(CueMolError::parse(format!(
            "TRR invalid {label} block size"
        )));
    }
    Ok(bytes / count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_i32(output: &mut Vec<u8>, value: i32) {
        output.extend_from_slice(&value.to_be_bytes());
    }

    fn push_gmx_string(output: &mut Vec<u8>, value: &str) {
        push_i32(output, value.len() as i32 + 1);
        push_i32(output, value.len() as i32);
        output.extend_from_slice(value.as_bytes());
        output.resize(output.len().next_multiple_of(4), 0);
    }

    fn push_real(output: &mut Vec<u8>, value: f64, double_precision: bool) {
        if double_precision {
            output.extend_from_slice(&value.to_be_bytes());
        } else {
            output.extend_from_slice(&(value as f32).to_be_bytes());
        }
    }

    fn push_frame(
        output: &mut Vec<u8>,
        natoms: usize,
        frame: usize,
        double_precision: bool,
        include_vectors: bool,
    ) {
        let scalar_bytes = if double_precision { 8 } else { 4 };
        let coordinate_bytes = natoms * 3 * scalar_bytes;
        push_i32(output, TRR_MAGIC);
        push_gmx_string(output, "GMX_trn_file");
        for size in [
            0,
            0,
            9 * scalar_bytes,
            9 * scalar_bytes,
            9 * scalar_bytes,
            0,
            0,
            coordinate_bytes,
            if include_vectors { coordinate_bytes } else { 0 },
            if include_vectors { coordinate_bytes } else { 0 },
        ] {
            push_i32(output, size as i32);
        }
        push_i32(output, natoms as i32);
        push_i32(output, frame as i32);
        push_i32(output, 0);
        push_real(output, frame as f64 * 0.5, double_precision);
        push_real(output, 0.0, double_precision);

        for value in [1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0] {
            push_real(output, value, double_precision);
        }
        for _ in 0..18 {
            push_real(output, 0.0, double_precision);
        }
        for atom in 0..natoms {
            for axis in 0..3 {
                push_real(
                    output,
                    (frame * 100 + atom * 10 + axis) as f64 / 10.0,
                    double_precision,
                );
            }
        }
        if include_vectors {
            for _ in 0..natoms * 6 {
                push_real(output, 1.0, double_precision);
            }
        }
    }

    fn trr_fixture(double_precision: bool) -> Vec<u8> {
        let mut output = Vec::new();
        for frame in 0..3 {
            push_frame(&mut output, 3, frame, double_precision, true);
        }
        output
    }

    #[test]
    fn reads_single_precision_frames_cells_and_skips_vectors() {
        let input = trr_fixture(false);
        assert!(sniff_trr_content(&input));
        let block = read_trr_bytes(&input).unwrap();
        assert_eq!(block.atom_count(), 3);
        assert_eq!(block.frame_count(), 3);
        assert_eq!(
            block.frame(1).unwrap(),
            &[100.0, 101.0, 102.0, 110.0, 111.0, 112.0, 120.0, 121.0, 122.0]
        );
        assert_eq!(block.cell(0), Some([10.0, 20.0, 30.0, 90.0, 90.0, 90.0]));
    }

    #[test]
    fn reads_double_precision_with_frame_and_atom_selection() {
        let input = trr_fixture(true);
        let block = read_trr_bytes_with_options(
            &input,
            &TrrReadOptions {
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
    }

    #[test]
    fn rejects_truncation_bad_magic_and_invalid_options() {
        let input = trr_fixture(false);
        assert!(read_trr_bytes(&input[..input.len() - 1]).is_err());

        let mut bad_magic = input.clone();
        bad_magic[3] = 0;
        assert!(read_trr_bytes(&bad_magic).is_err());
        assert!(read_trr_bytes_with_options(
            &input,
            &TrrReadOptions {
                skip_no: 0,
                ..TrrReadOptions::default()
            }
        )
        .is_err());
        assert!(read_trr_bytes_with_options(
            &input,
            &TrrReadOptions {
                expected_atom_count: Some(4),
                ..TrrReadOptions::default()
            }
        )
        .is_err());
    }
}
