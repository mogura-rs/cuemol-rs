use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_molstr::{Atom, BondOrder, MolCoord};
use cuemol_qsys::{InOutHandler, ObjReader};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsfReader {
    reader: ObjReader,
}

impl PsfReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("psf", "CHARMM/NAMD PSF topology (*.psf)", "*.psf"),
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
        read_psf_str(input)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<MolCoord> {
        read_psf_file(path)
    }
}

impl Default for PsfReader {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_psf_file(path: impl AsRef<Path>) -> Result<MolCoord> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_psf_str(&input)
}

pub fn read_psf_str(input: &str) -> Result<MolCoord> {
    let lines = input.lines().collect::<Vec<_>>();
    let first = lines
        .iter()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| CueMolError::parse("empty PSF file"))?;
    if !first.trim_start().starts_with("PSF") {
        return Err(CueMolError::parse("missing PSF header"));
    }

    let (atom_count_line, atom_count) = find_section_count(&lines, "NATOM")?;
    let mut molecule = MolCoord::new();
    let mut serial_to_index = BTreeMap::new();
    let mut line_index = atom_count_line + 1;
    let mut atom_index = 0;
    while atom_index < atom_count {
        let line = lines
            .get(line_index)
            .ok_or_else(|| CueMolError::parse("PSF atom block is truncated"))?;
        line_index += 1;
        if line.trim().is_empty() {
            continue;
        }
        let atom = parse_atom_line(line, atom_index)?;
        serial_to_index.insert(atom.serial, atom_index);
        molecule.push_atom(atom);
        atom_index += 1;
    }

    if let Ok((bond_count_line, bond_count)) = find_section_count(&lines, "NBOND") {
        add_psf_bonds(
            &lines,
            bond_count_line,
            bond_count,
            &serial_to_index,
            &mut molecule,
        )?;
    }

    Ok(molecule)
}

pub fn sniff_psf_content(input: &str) -> cuemol_molstr::ContentVerdict {
    let Some(first) = input.lines().find(|line| !line.trim().is_empty()) else {
        return cuemol_molstr::ContentVerdict::Unknown;
    };
    if !first.trim_start().starts_with("PSF") {
        return cuemol_molstr::ContentVerdict::No;
    }
    for line in input.lines() {
        if line
            .split_once('!')
            .map(|(_, marker)| {
                marker
                    .trim_start()
                    .to_ascii_uppercase()
                    .starts_with("NATOM")
            })
            .unwrap_or(false)
        {
            return cuemol_molstr::ContentVerdict::Yes;
        }
    }
    cuemol_molstr::ContentVerdict::Unknown
}

fn find_section_count(lines: &[&str], marker: &str) -> Result<(usize, usize)> {
    for (index, line) in lines.iter().enumerate() {
        let Some((prefix, comment)) = line.split_once('!') else {
            continue;
        };
        if !comment
            .trim_start()
            .to_ascii_uppercase()
            .starts_with(marker)
        {
            continue;
        }
        let count = prefix
            .split_whitespace()
            .next()
            .ok_or_else(|| CueMolError::parse(format!("missing PSF {marker} count")))?
            .parse::<usize>()
            .map_err(|_| CueMolError::parse(format!("invalid PSF {marker} count")))?;
        return Ok((index, count));
    }
    Err(CueMolError::parse(format!("missing PSF {marker} section")))
}

fn parse_atom_line(line: &str, atom_index: usize) -> Result<Atom> {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    if tokens.len() < 8 {
        return Err(CueMolError::parse("PSF atom line is too short"));
    }
    let serial = tokens[0]
        .parse::<i32>()
        .map_err(|_| CueMolError::parse("invalid PSF atom serial"))?;
    let res_id = tokens[2]
        .parse::<i32>()
        .map_err(|_| CueMolError::parse("invalid PSF residue id"))?;
    tokens[6]
        .parse::<f64>()
        .map_err(|_| CueMolError::parse("invalid PSF atom charge"))?;
    let mass = tokens[7]
        .parse::<f64>()
        .map_err(|_| CueMolError::parse("invalid PSF atom mass"))?;

    Ok(Atom {
        serial: if serial > 0 {
            serial
        } else {
            (atom_index + 1) as i32
        },
        name: tokens[4].to_string(),
        alt_loc: None,
        res_name: tokens[3].to_string(),
        chain_id: empty_as(tokens[1], "A").to_string(),
        res_id,
        ins_code: None,
        position: Vector4::ZERO,
        occupancy: 1.0,
        b_factor: 0.0,
        element: mass_symbol(mass).unwrap_or("XX").to_string(),
        formal_charge: None,
        int_props: Default::default(),
    })
}

fn add_psf_bonds(
    lines: &[&str],
    count_line: usize,
    bond_count: usize,
    serial_to_index: &BTreeMap<i32, usize>,
    molecule: &mut MolCoord,
) -> Result<()> {
    let required_values = bond_count * 2;
    if required_values == 0 {
        return Ok(());
    }

    let mut values = Vec::with_capacity(required_values);
    for line in lines.iter().skip(count_line + 1) {
        if values.len() >= required_values {
            break;
        }
        if is_next_psf_section(line) {
            break;
        }
        let data = line.split_once('!').map(|(data, _)| data).unwrap_or(line);
        for token in data.split_whitespace() {
            if values.len() == required_values {
                break;
            }
            values.push(
                token
                    .parse::<i32>()
                    .map_err(|_| CueMolError::parse("invalid PSF bond atom serial"))?,
            );
        }
    }

    if values.len() != required_values {
        return Err(CueMolError::parse("PSF bond block is truncated"));
    }

    for pair in values.chunks_exact(2) {
        let atom1 = serial_to_index
            .get(&pair[0])
            .copied()
            .ok_or_else(|| CueMolError::parse("PSF bond references unknown atom"))?;
        let atom2 = serial_to_index
            .get(&pair[1])
            .copied()
            .ok_or_else(|| CueMolError::parse("PSF bond references unknown atom"))?;
        molecule.add_bond(atom1, atom2, BondOrder::Single);
    }
    Ok(())
}

fn is_next_psf_section(line: &str) -> bool {
    let Some((prefix, comment)) = line.split_once('!') else {
        return false;
    };
    prefix.split_whitespace().next().is_some()
        && comment
            .trim_start()
            .chars()
            .next()
            .map(|ch| ch == 'N' || ch == 'n')
            .unwrap_or(false)
}

fn mass_symbol(mass: f64) -> Option<&'static str> {
    Some(match () {
        _ if near_mass(mass, 1.0080) => "H",
        _ if near_mass(mass, 12.0110) => "C",
        _ if near_mass(mass, 14.0070) => "N",
        _ if near_mass(mass, 15.9994) => "O",
        _ if near_mass(mass, 18.998) => "F",
        _ if near_mass(mass, 22.9898) => "Na",
        _ if near_mass(mass, 24.305) => "Mg",
        _ if near_mass(mass, 30.9740) => "P",
        _ if near_mass(mass, 32.0600) => "S",
        _ if near_mass(mass, 35.4500) => "Cl",
        _ if near_mass(mass, 39.102000) => "K",
        _ if near_mass(mass, 40.08) => "Ca",
        _ if near_mass(mass, 55.847) => "Fe",
        _ if near_mass(mass, 65.37) => "Zn",
        _ => return None,
    })
}

fn near_mass(value: f64, target: f64) -> bool {
    target - 0.1 < value && value < target + 0.1
}

fn empty_as<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}
