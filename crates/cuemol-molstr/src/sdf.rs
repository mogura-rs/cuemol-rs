use crate::molecule::{Atom, BondOrder, MolCoord};
use cuemol_core::{CueMolError, Result, Vector4};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdfReadOptions {
    pub load_compound_id: i32,
    pub load_as_chain: bool,
    pub chain_name: String,
    pub residue_index: i32,
}

impl Default for SdfReadOptions {
    fn default() -> Self {
        Self {
            load_compound_id: -1,
            load_as_chain: false,
            chain_name: "A".to_string(),
            residue_index: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SdfReader {
    options: SdfReadOptions,
}
pub struct SdfWriter;

impl SdfReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn options(&self) -> &SdfReadOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: SdfReadOptions) {
        self.options = options;
    }

    pub fn read_str(input: &str) -> Result<MolCoord> {
        read_sdf_str(input)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<MolCoord> {
        read_sdf_file(path)
    }

    pub fn read_str_configured(&self, input: &str) -> Result<MolCoord> {
        read_sdf_str_with_options(input, &self.options)
    }

    pub fn read_file_configured(&self, path: impl AsRef<Path>) -> Result<MolCoord> {
        read_sdf_file_with_options(path, &self.options)
    }
}

impl SdfWriter {
    pub fn write_string(mol: &MolCoord) -> String {
        write_sdf_string(mol)
    }
}

pub fn read_sdf_file(path: impl AsRef<Path>) -> Result<MolCoord> {
    read_sdf_file_with_options(path, &SdfReadOptions::default())
}

pub fn read_sdf_file_with_options(
    path: impl AsRef<Path>,
    options: &SdfReadOptions,
) -> Result<MolCoord> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_sdf_str_with_options(&input, options)
}

pub fn read_sdf_str(input: &str) -> Result<MolCoord> {
    read_sdf_str_with_options(input, &SdfReadOptions::default())
}

pub fn read_sdf_str_with_options(input: &str, options: &SdfReadOptions) -> Result<MolCoord> {
    let lines = input.lines().collect::<Vec<_>>();
    let mut index = 0;
    let mut mol = MolCoord::new();
    let mut compound_id = 0_i32;
    while index < lines.len() {
        if lines[index].trim().is_empty() && index + 1 >= lines.len() {
            break;
        }
        if index + 3 >= lines.len() {
            break;
        }
        index += 3;
        let counts = lines[index];
        index += 1;
        let atom_count = parse_counts_int(counts, 0, 3, "atom count")?;
        let bond_count = parse_counts_int(counts, 3, 6, "bond count")?;
        if !counts.get(33..39).unwrap_or("").trim().eq("V2000") {
            return Err(CueMolError::parse("unsupported SDF version"));
        }

        let skip = options.load_compound_id >= 0 && options.load_compound_id != compound_id;
        let mut atom_map = Vec::new();
        for _ in 0..atom_count {
            let line = lines
                .get(index)
                .ok_or_else(|| CueMolError::parse("missing SDF atom line"))?;
            index += 1;
            let x = parse_sdf_f64(line, 0, 10)?;
            let y = parse_sdf_f64(line, 10, 20)?;
            let z = parse_sdf_f64(line, 20, 30)?;
            let element = line.get(31..34).unwrap_or("").trim().to_string();
            if skip {
                atom_map.push(None);
            } else {
                let new_index = mol.atom_count();
                let chain_id = if options.load_as_chain {
                    sdf_chain_name(&options.chain_name, compound_id)
                } else {
                    options.chain_name.clone()
                };
                let res_id = if options.load_as_chain {
                    options.residue_index
                } else {
                    options.residue_index + compound_id
                };
                mol.push_atom(Atom {
                    serial: (new_index + 1) as i32,
                    name: element.clone(),
                    alt_loc: None,
                    res_name: "MOL".to_string(),
                    chain_id,
                    res_id,
                    ins_code: None,
                    position: Vector4::new3(x, y, z),
                    occupancy: 1.0,
                    b_factor: 0.0,
                    element,
                    formal_charge: None,
                    int_props: Default::default(),
                });
                atom_map.push(Some(new_index));
            }
        }
        for _ in 0..bond_count {
            let line = lines
                .get(index)
                .ok_or_else(|| CueMolError::parse("missing SDF bond line"))?;
            index += 1;
            let atom1 = parse_counts_int(line, 0, 3, "bond atom1")? - 1;
            let atom2 = parse_counts_int(line, 3, 6, "bond atom2")? - 1;
            let order = parse_counts_int(line, 6, 9, "bond order")?;
            // The MDL bond-stereo field (cols 10-12) is optional; default 0.
            let stereo = line
                .get(9..12)
                .and_then(|s| s.trim().parse::<i32>().ok())
                .unwrap_or(0);
            if !skip {
                let Some(Some(atom1)) = atom_map.get(atom1) else {
                    continue;
                };
                let Some(Some(atom2)) = atom_map.get(atom2) else {
                    continue;
                };
                mol.add_bond_stereo(*atom1, *atom2, BondOrder::from_sdf(order as i32), stereo);
            }
        }
        while index < lines.len() {
            let line = lines[index];
            index += 1;
            if line.starts_with("M  CHG") {
                read_charge_line(line, &atom_map, &mut mol)?;
            }
            if line.starts_with("$$$$") {
                break;
            }
            if line.starts_with("M  END") && index >= lines.len() {
                break;
            }
        }
        compound_id += 1;
    }
    if mol.atom_count() == 0 {
        Err(CueMolError::parse("SDF contains no atoms"))
    } else {
        Ok(mol)
    }
}

pub fn write_sdf_string(mol: &MolCoord) -> String {
    let mut output = String::new();
    output.push_str("SDFile Generated by CueMol2\n");
    output.push_str("     RDKit          3D\n");
    output.push('\n');
    output.push_str(&format!(
        "{:>3}{:>3}  0  0  0  0  0  0  0  0999 V2000\n",
        mol.atom_count(),
        mol.bond_count()
    ));
    for atom in mol.atoms() {
        output.push_str(&format!(
            "{:>10.4}{:>10.4}{:>10.4} {:<3} 0  0  0  0  0  0  0  0  0  0  0  0\n",
            atom.position.x, atom.position.y, atom.position.z, atom.element
        ));
    }
    for bond in mol.bonds() {
        output.push_str(&format!(
            "{:>3}{:>3}{:>3}  0\n",
            bond.atom1 + 1,
            bond.atom2 + 1,
            bond.order.sdf_code()
        ));
    }
    let charges = mol
        .atoms()
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| atom.formal_charge.map(|charge| (index + 1, charge)))
        .collect::<Vec<_>>();
    for chunk in charges.chunks(8) {
        output.push_str(&format!("M  CHG{:>3}", chunk.len()));
        for (index, charge) in chunk {
            output.push_str(&format!("{index:>4}{charge:>4}"));
        }
        output.push('\n');
    }
    output.push_str("M  END\n$$$$\n");
    output
}

fn sdf_chain_name(base: &str, compound_id: i32) -> String {
    if compound_id == 0 {
        base.to_string()
    } else {
        format!("{base}{compound_id}")
    }
}

fn read_charge_line(line: &str, atom_map: &[Option<usize>], mol: &mut MolCoord) -> Result<()> {
    let count = parse_counts_int(line, 6, 9, "charge count")?;
    for index in 0..count {
        let atom_start = 9 + index * 8;
        let charge_start = atom_start + 4;
        let atom_index = parse_counts_int(line, atom_start, atom_start + 4, "charge atom")?;
        let charge = parse_counts_i32(line, charge_start, charge_start + 4, "charge value")?;
        let Some(mapped_index) = atom_map.get(atom_index - 1).copied().flatten() else {
            continue;
        };
        if let Some(atom) = mol.atoms_mut().get_mut(mapped_index) {
            atom.formal_charge = Some(charge);
        }
    }
    Ok(())
}

fn parse_counts_int(line: &str, start: usize, end: usize, name: &str) -> Result<usize> {
    line.get(start..end)
        .unwrap_or("")
        .trim()
        .parse::<usize>()
        .map_err(|_| CueMolError::parse(format!("invalid SDF {name}")))
}

fn parse_counts_i32(line: &str, start: usize, end: usize, name: &str) -> Result<i32> {
    line.get(start..end)
        .unwrap_or("")
        .trim()
        .parse::<i32>()
        .map_err(|_| CueMolError::parse(format!("invalid SDF {name}")))
}

fn parse_sdf_f64(line: &str, start: usize, end: usize) -> Result<f64> {
    line.get(start..end)
        .unwrap_or("")
        .trim()
        .parse::<f64>()
        .map_err(|_| CueMolError::parse("invalid SDF coordinate"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_COMPOUNDS: &str = "\
first
  CueMol

  1  0  0  0  0  0  0  0  0  0999 V2000
    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
M  END
$$$$
second
  CueMol

  1  0  0  0  0  0  0  0  0  0999 V2000
    1.0000    0.0000    0.0000 N   0  0  0  0  0  0  0  0  0  0  0  0
M  END
$$$$
";

    #[test]
    fn sdf_reader_options_select_compound_and_chain_mode() {
        let mol = read_sdf_str(TWO_COMPOUNDS).unwrap();
        assert_eq!(mol.atom_count(), 2);
        assert_eq!(mol.atoms()[0].chain_id, "A");
        assert_eq!(mol.atoms()[0].res_id, 1);
        assert_eq!(mol.atoms()[1].res_id, 2);

        let options = SdfReadOptions {
            load_compound_id: 1,
            load_as_chain: true,
            chain_name: "Z".to_string(),
            residue_index: 5,
        };
        let mol = read_sdf_str_with_options(TWO_COMPOUNDS, &options).unwrap();
        assert_eq!(mol.atom_count(), 1);
        assert_eq!(mol.atoms()[0].element, "N");
        assert_eq!(mol.atoms()[0].chain_id, "Z1");
        assert_eq!(mol.atoms()[0].res_id, 5);
    }
}
