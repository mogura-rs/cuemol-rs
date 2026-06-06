use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_molstr::MolCoord;
use cuemol_qsys::{InOutHandler, ObjReader};
use std::path::Path;

use crate::psf::{read_psf_file, read_psf_str};

const MAX_NATOMS: i32 = 10_000_000;
const HEADER_LEN: usize = 4;
const COORD_SIZE: usize = 8;
const COORDS_PER_ATOM: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct NamdCoorFrame {
    pub positions: Vec<Vector4>,
    pub byte_swapped: bool,
}

impl NamdCoorFrame {
    pub fn atom_count(&self) -> usize {
        self.positions.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamdCoorLoad {
    pub molecule: MolCoord,
    pub frame: NamdCoorFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamdCoorReader {
    reader: ObjReader,
}

impl NamdCoorReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("namdcoor", "NAMD Coordinates (*.coor)", "*.coor"),
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

    pub fn topology_path(&self) -> &str {
        self.reader.path_for("topo")
    }

    pub fn set_topology_path(&mut self, path: impl Into<String>) {
        self.reader.set_sub_path("topo", path);
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<NamdCoorFrame> {
        read_namd_coor_file(path)
    }

    pub fn read_bytes(bytes: &[u8]) -> Result<NamdCoorFrame> {
        read_namd_coor_bytes(bytes)
    }

    pub fn read_with_psf_files(
        coor_path: impl AsRef<Path>,
        psf_path: impl AsRef<Path>,
    ) -> Result<NamdCoorLoad> {
        read_namd_coor_with_psf_files(coor_path, psf_path)
    }
}

impl Default for NamdCoorReader {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_namd_coor_file(path: impl AsRef<Path>) -> Result<NamdCoorFrame> {
    let bytes = crate::fileio::read_bytes_maybe_compressed(path)?;
    read_namd_coor_bytes(&bytes)
}

pub fn read_namd_coor_with_psf_files(
    coor_path: impl AsRef<Path>,
    psf_path: impl AsRef<Path>,
) -> Result<NamdCoorLoad> {
    let bytes = crate::fileio::read_bytes_maybe_compressed(coor_path)?;
    let molecule = read_psf_file(psf_path)?;
    read_namd_coor_with_psf_molecule(bytes.as_slice(), molecule)
}

pub fn read_namd_coor_with_psf_bytes(coor_bytes: &[u8], psf_input: &str) -> Result<NamdCoorLoad> {
    let molecule = read_psf_str(psf_input)?;
    read_namd_coor_with_psf_molecule(coor_bytes, molecule)
}

pub fn read_namd_coor_bytes(bytes: &[u8]) -> Result<NamdCoorFrame> {
    if bytes.len() < HEADER_LEN {
        return Err(CueMolError::parse("NAMD coor file is too short"));
    }

    if let Some(frame) = try_read_namd_coor(bytes, false)? {
        return Ok(frame);
    }
    if let Some(frame) = try_read_namd_coor(bytes, true)? {
        return Ok(frame);
    }

    Err(CueMolError::parse("invalid NAMD coor atom count or size"))
}

pub fn apply_namd_coor_frame(molecule: &mut MolCoord, frame: &NamdCoorFrame) -> Result<()> {
    if molecule.atom_count() != frame.positions.len() {
        return Err(CueMolError::parse(format!(
            "molecule/coor atom count mismatch: {} != {}",
            molecule.atom_count(),
            frame.positions.len()
        )));
    }

    for (atom, position) in molecule.atoms_mut().iter_mut().zip(&frame.positions) {
        atom.position = *position;
    }
    Ok(())
}

pub fn apply_namd_coor_file(
    molecule: &mut MolCoord,
    path: impl AsRef<Path>,
) -> Result<NamdCoorFrame> {
    let frame = read_namd_coor_file(path)?;
    apply_namd_coor_frame(molecule, &frame)?;
    Ok(frame)
}

pub fn apply_namd_coor_bytes(molecule: &mut MolCoord, bytes: &[u8]) -> Result<NamdCoorFrame> {
    let frame = read_namd_coor_bytes(bytes)?;
    apply_namd_coor_frame(molecule, &frame)?;
    Ok(frame)
}

fn read_namd_coor_with_psf_molecule(
    coor_bytes: &[u8],
    mut molecule: MolCoord,
) -> Result<NamdCoorLoad> {
    let frame = apply_namd_coor_bytes(&mut molecule, coor_bytes)?;
    if molecule.bond_count() == 0 {
        molecule.apply_topology();
    }
    Ok(NamdCoorLoad { molecule, frame })
}

fn try_read_namd_coor(bytes: &[u8], big_endian: bool) -> Result<Option<NamdCoorFrame>> {
    let natoms = read_i32(bytes, 0, big_endian);
    let Some(expected_len) = expected_len(natoms) else {
        return Ok(None);
    };
    if bytes.len() < expected_len {
        return Ok(None);
    }

    let mut positions = Vec::with_capacity(natoms as usize);
    let mut offset = HEADER_LEN;
    for _ in 0..natoms {
        let x = read_f64(bytes, offset, big_endian);
        let y = read_f64(bytes, offset + COORD_SIZE, big_endian);
        let z = read_f64(bytes, offset + 2 * COORD_SIZE, big_endian);
        if !x.is_finite() || !y.is_finite() || !z.is_finite() {
            return Err(CueMolError::parse("invalid NAMD coor coordinate"));
        }
        positions.push(Vector4::new3(x, y, z));
        offset += COORDS_PER_ATOM * COORD_SIZE;
    }

    Ok(Some(NamdCoorFrame {
        positions,
        byte_swapped: big_endian,
    }))
}

fn expected_len(natoms: i32) -> Option<usize> {
    if !(1..=MAX_NATOMS).contains(&natoms) {
        return None;
    }
    let natoms = natoms as usize;
    HEADER_LEN.checked_add(natoms.checked_mul(COORDS_PER_ATOM * COORD_SIZE)?)
}

fn read_i32(bytes: &[u8], offset: usize, big_endian: bool) -> i32 {
    let field = bytes[offset..offset + 4].try_into().unwrap();
    if big_endian {
        i32::from_be_bytes(field)
    } else {
        i32::from_le_bytes(field)
    }
}

fn read_f64(bytes: &[u8], offset: usize, big_endian: bool) -> f64 {
    let field = bytes[offset..offset + 8].try_into().unwrap();
    if big_endian {
        f64::from_be_bytes(field)
    } else {
        f64::from_le_bytes(field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_namd_coor_positions() {
        let frame = read_namd_coor_bytes(&namd_bytes(&[[1.0, -2.0, 3.0], [4.0, 5.0, -6.0]], false))
            .unwrap();

        assert_eq!(frame.atom_count(), 2);
        assert!(!frame.byte_swapped);
        assert_eq!(frame.positions[0], Vector4::new3(1.0, -2.0, 3.0));
        assert_eq!(frame.positions[1], Vector4::new3(4.0, 5.0, -6.0));
    }

    #[test]
    fn reads_swapped_namd_coor_positions() {
        let frame = read_namd_coor_bytes(&namd_bytes(&[[1.0, -2.0, 3.0]], true)).unwrap();

        assert_eq!(frame.atom_count(), 1);
        assert!(frame.byte_swapped);
        assert_eq!(frame.positions[0], Vector4::new3(1.0, -2.0, 3.0));
    }

    fn namd_bytes(coords: &[[f64; 3]], swapped: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        let natoms = coords.len() as i32;
        if swapped {
            bytes.extend_from_slice(&natoms.to_be_bytes());
            for [x, y, z] in coords {
                bytes.extend_from_slice(&x.to_be_bytes());
                bytes.extend_from_slice(&y.to_be_bytes());
                bytes.extend_from_slice(&z.to_be_bytes());
            }
        } else {
            bytes.extend_from_slice(&natoms.to_le_bytes());
            for [x, y, z] in coords {
                bytes.extend_from_slice(&x.to_le_bytes());
                bytes.extend_from_slice(&y.to_le_bytes());
                bytes.extend_from_slice(&z.to_le_bytes());
            }
        }
        bytes
    }
}
