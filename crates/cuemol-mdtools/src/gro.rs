use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_molstr::{Atom, MolCoord};
use cuemol_qsys::{InOutHandler, ObjReader};
use cuemol_symm::CrystalInfo;
use std::path::Path;

const NM_TO_ANGSTROM: f64 = 10.0;
const GRO_PREFIX_LEN: usize = 20;
const GRO_DEFAULT_POS_WIDTH: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct GroFrame {
    pub molecule: MolCoord,
    pub cell: Option<CrystalInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroReader {
    reader: ObjReader,
}

impl GroReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("gro", "GROMACS Coordinates (*.gro)", "*.gro"),
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

    pub fn read_str(input: &str) -> Result<MolCoord> {
        read_gro_str(input)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<MolCoord> {
        read_gro_file(path)
    }
}

impl Default for GroReader {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_gro_file(path: impl AsRef<Path>) -> Result<MolCoord> {
    Ok(read_gro_frame_file(path)?.molecule)
}

pub fn read_gro_str(input: &str) -> Result<MolCoord> {
    Ok(read_gro_frame_str(input)?.molecule)
}

pub fn read_gro_frame_file(path: impl AsRef<Path>) -> Result<GroFrame> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_gro_frame_str(&input)
}

pub fn read_gro_frame_str(input: &str) -> Result<GroFrame> {
    let lines = input.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Err(CueMolError::parse("GRO file is empty"));
    }
    if lines.len() < 2 {
        return Err(CueMolError::parse("missing GRO atom count line"));
    }
    let atom_count = lines[1]
        .trim()
        .parse::<usize>()
        .map_err(|_| CueMolError::parse("invalid GRO atom count"))?;
    if lines.len() < 2 + atom_count {
        return Err(CueMolError::parse("GRO atom count exceeds atom lines"));
    }

    let mut molecule = MolCoord::new();
    let mut pos_width = GRO_DEFAULT_POS_WIDTH;
    for atom_index in 0..atom_count {
        let line = lines[2 + atom_index];
        if atom_index == 0 {
            pos_width = determine_position_width(line)?;
        }
        molecule.push_atom(parse_atom_line(line, atom_index, pos_width)?);
    }

    let cell = lines
        .get(2 + atom_count)
        .and_then(|line| parse_box_line(line).ok().flatten());

    Ok(GroFrame { molecule, cell })
}

pub fn sniff_gro_content(input: &str) -> cuemol_molstr::ContentVerdict {
    let mut lines = input.lines();
    let Some(_) = lines.next() else {
        return cuemol_molstr::ContentVerdict::Unknown;
    };
    let Some(count_line) = lines.next() else {
        return cuemol_molstr::ContentVerdict::Unknown;
    };
    let Ok(atom_count) = count_line.trim().parse::<isize>() else {
        return cuemol_molstr::ContentVerdict::No;
    };
    if atom_count < 0 {
        return cuemol_molstr::ContentVerdict::No;
    }
    if atom_count == 0 {
        return cuemol_molstr::ContentVerdict::Unknown;
    }
    let Some(atom_line) = lines.next() else {
        return cuemol_molstr::ContentVerdict::No;
    };
    if atom_line.len() < GRO_PREFIX_LEN + 3 * GRO_DEFAULT_POS_WIDTH {
        return cuemol_molstr::ContentVerdict::No;
    }
    for start in [20, 28, 36] {
        if atom_line
            .get(start..start + GRO_DEFAULT_POS_WIDTH)
            .unwrap_or("")
            .trim()
            .parse::<f64>()
            .is_err()
        {
            return cuemol_molstr::ContentVerdict::No;
        }
    }
    cuemol_molstr::ContentVerdict::Yes
}

fn determine_position_width(line: &str) -> Result<usize> {
    if line.len() < GRO_PREFIX_LEN + 3 * GRO_DEFAULT_POS_WIDTH {
        return Err(CueMolError::parse("GRO atom line too short"));
    }
    let rest = line.len() - GRO_PREFIX_LEN;
    if rest == 3 * GRO_DEFAULT_POS_WIDTH || rest == 6 * GRO_DEFAULT_POS_WIDTH {
        return Ok(GRO_DEFAULT_POS_WIDTH);
    }
    if rest.is_multiple_of(6) && rest / 6 >= GRO_DEFAULT_POS_WIDTH {
        return Ok(rest / 6);
    }
    if rest.is_multiple_of(3) && rest / 3 >= GRO_DEFAULT_POS_WIDTH {
        return Ok(rest / 3);
    }
    Ok(GRO_DEFAULT_POS_WIDTH)
}

fn parse_atom_line(line: &str, atom_index: usize, pos_width: usize) -> Result<Atom> {
    let required = GRO_PREFIX_LEN + 3 * pos_width;
    if line.len() < required {
        return Err(CueMolError::parse("GRO atom line too short"));
    }
    let res_id = field(line, 0, 5)
        .trim()
        .parse::<i32>()
        .map_err(|_| CueMolError::parse("invalid GRO residue id"))?;
    let res_name = empty_as(field(line, 5, 10).trim(), "UNK");
    let atom_name = field(line, 10, 15).trim();
    if atom_name.is_empty() {
        return Err(CueMolError::parse("missing GRO atom name"));
    }
    let x = parse_position(line, 0, pos_width)?;
    let y = parse_position(line, 1, pos_width)?;
    let z = parse_position(line, 2, pos_width)?;
    Ok(Atom {
        serial: (atom_index + 1) as i32,
        name: atom_name.to_string(),
        alt_loc: None,
        res_name: res_name.to_string(),
        chain_id: "A".to_string(),
        res_id,
        ins_code: None,
        position: Vector4::new3(x * NM_TO_ANGSTROM, y * NM_TO_ANGSTROM, z * NM_TO_ANGSTROM),
        occupancy: 1.0,
        b_factor: 0.0,
        element: guess_element(atom_name),
        formal_charge: None,
        int_props: Default::default(),
    })
}

fn parse_position(line: &str, index: usize, width: usize) -> Result<f64> {
    let start = GRO_PREFIX_LEN + index * width;
    field(line, start, start + width)
        .trim()
        .parse::<f64>()
        .map_err(|_| CueMolError::parse("invalid GRO coordinate"))
}

fn parse_box_line(line: &str) -> Result<Option<CrystalInfo>> {
    let values = line
        .split_whitespace()
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| CueMolError::parse("invalid GRO box value"))
        })
        .collect::<Result<Vec<_>>>()?;
    if values.len() == 3 {
        return Ok(Some(CrystalInfo::new(
            values[0] * NM_TO_ANGSTROM,
            values[1] * NM_TO_ANGSTROM,
            values[2] * NM_TO_ANGSTROM,
            90.0,
            90.0,
            90.0,
        )));
    }
    if values.len() != 9 {
        return Ok(None);
    }

    let v1 = [values[0], values[3], values[4]];
    let v2 = [values[5], values[1], values[6]];
    let v3 = [values[7], values[8], values[2]];
    let a = length(v1);
    let b = length(v2);
    let c = length(v3);
    if a == 0.0 || b == 0.0 || c == 0.0 {
        return Ok(None);
    }
    Ok(Some(CrystalInfo::new(
        a * NM_TO_ANGSTROM,
        b * NM_TO_ANGSTROM,
        c * NM_TO_ANGSTROM,
        angle_deg(v2, v3),
        angle_deg(v1, v3),
        angle_deg(v1, v2),
    )))
}

fn length(value: [f64; 3]) -> f64 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

fn dot(lhs: [f64; 3], rhs: [f64; 3]) -> f64 {
    lhs[0] * rhs[0] + lhs[1] * rhs[1] + lhs[2] * rhs[2]
}

fn angle_deg(lhs: [f64; 3], rhs: [f64; 3]) -> f64 {
    (dot(lhs, rhs) / (length(lhs) * length(rhs)))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

fn field(line: &str, start: usize, end: usize) -> &str {
    line.get(start..end).unwrap_or("")
}

fn empty_as<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() {
        fallback
    } else {
        value
    }
}

fn guess_element(atom_name: &str) -> String {
    let trimmed = atom_name.trim_start_matches(|ch: char| ch.is_ascii_digit());
    if trimmed.is_empty() {
        return String::new();
    }
    let mut chars = trimmed.chars();
    let first = chars.next().unwrap().to_ascii_uppercase();
    if let Some(second) = chars.next() {
        let mut two = String::new();
        two.push(first);
        two.push(second.to_ascii_lowercase());
        if matches!(two.as_str(), "Ca" | "Mg" | "Na" | "Cl" | "Fe" | "Zn") {
            return two;
        }
    }
    first.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_gro_positions_in_angstroms() {
        let input = "\
water
    3
    1SOL     OW    1   0.100   0.200   0.300
    1SOL    HW1    2   0.110   0.200   0.300
    1SOL    HW2    3   0.100   0.210   0.300
   2.00000   2.00000   2.00000
";
        let frame = read_gro_frame_str(input).unwrap();
        assert_eq!(frame.molecule.atom_count(), 3);
        assert_eq!(frame.molecule.atoms()[0].name, "OW");
        assert_eq!(frame.molecule.atoms()[0].position.x, 1.0);
        assert_eq!(frame.molecule.atoms()[0].position.y, 2.0);
        assert_eq!(frame.molecule.atoms()[0].position.z, 3.0);
        assert_eq!(frame.cell.unwrap().a, 20.0);
    }
}
