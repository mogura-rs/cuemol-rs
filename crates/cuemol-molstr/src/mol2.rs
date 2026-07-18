use crate::molecule::{Atom, BondOrder, MolCoord};
use cuemol_core::{CueMolError, Result, Vector4};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mol2ReadOptions {
    pub load_compound_id: i32,
    pub load_as_chain: bool,
    pub chain_name: String,
    pub residue_index: i32,
}

impl Default for Mol2ReadOptions {
    fn default() -> Self {
        Self {
            load_compound_id: -1,
            load_as_chain: true,
            chain_name: "A".to_string(),
            residue_index: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mol2Reader {
    options: Mol2ReadOptions,
}

impl Mol2Reader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn options(&self) -> &Mol2ReadOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: Mol2ReadOptions) {
        self.options = options;
    }

    pub fn read_str(input: &str) -> Result<MolCoord> {
        read_mol2_str(input)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<MolCoord> {
        read_mol2_file(path)
    }

    pub fn read_str_configured(&self, input: &str) -> Result<MolCoord> {
        read_mol2_str_with_options(input, &self.options)
    }

    pub fn read_file_configured(&self, path: impl AsRef<Path>) -> Result<MolCoord> {
        read_mol2_file_with_options(path, &self.options)
    }
}

pub fn read_mol2_file(path: impl AsRef<Path>) -> Result<MolCoord> {
    read_mol2_file_with_options(path, &Mol2ReadOptions::default())
}

pub fn read_mol2_file_with_options(
    path: impl AsRef<Path>,
    options: &Mol2ReadOptions,
) -> Result<MolCoord> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_mol2_str_with_options(&input, options)
}

pub fn read_mol2_str(input: &str) -> Result<MolCoord> {
    read_mol2_str_with_options(input, &Mol2ReadOptions::default())
}

pub fn read_mol2_str_with_options(input: &str, options: &Mol2ReadOptions) -> Result<MolCoord> {
    let lines = input.lines().collect::<Vec<_>>();
    let mut index = 0;
    let mut molecule = MolCoord::new();
    let mut compound_id = 0_i32;
    while let Some(marker) = find_section(&lines, index, "@<TRIPOS>MOLECULE") {
        index = marker + 1;
        let skip = options.load_compound_id >= 0 && options.load_compound_id != compound_id;
        read_one_mol2(
            &lines,
            &mut index,
            compound_id,
            options,
            skip,
            &mut molecule,
        )?;
        compound_id += 1;
    }
    if molecule.atom_count() == 0 {
        Err(CueMolError::parse("MOL2 contains no atoms"))
    } else {
        Ok(molecule)
    }
}

fn read_one_mol2(
    lines: &[&str],
    index: &mut usize,
    compound_id: i32,
    options: &Mol2ReadOptions,
    skip: bool,
    molecule: &mut MolCoord,
) -> Result<()> {
    let compound_name = lines
        .get(*index)
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .unwrap_or("_");
    *index += 1;

    let counts = lines
        .get(*index)
        .ok_or_else(|| CueMolError::parse("missing MOL2 counts line"))?
        .split_whitespace()
        .collect::<Vec<_>>();
    *index += 1;
    let atom_count = parse_usize(counts.first().copied(), "MOL2 atom count")?;
    let bond_count = counts
        .get(1)
        .map(|value| parse_usize(Some(*value), "MOL2 bond count"))
        .transpose()?
        .unwrap_or(0);

    let mol_type = lines.get(*index).map(|line| line.trim()).unwrap_or("");
    let apply_biopolymer_names = matches!(mol_type, "PROTEIN" | "NUCLEIC_ACID");
    *index += 1;

    let atom_marker = find_section(lines, *index, "@<TRIPOS>ATOM")
        .ok_or_else(|| CueMolError::parse("MOL2 ATOM section not found"))?;
    *index = atom_marker + 1;

    let atom_offset = molecule.atom_count();
    let mut atom_map = HashMap::<i32, usize>::new();
    let mut name_counts = BTreeMap::<String, usize>::new();
    let mut previous_residue = i32::MIN;

    for _ in 0..atom_count {
        let line = lines
            .get(*index)
            .ok_or_else(|| CueMolError::parse("missing MOL2 atom line"))?;
        *index += 1;
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 8 {
            return Err(CueMolError::parse("invalid MOL2 atom record"));
        }

        let source_id = parse_i32(Some(fields[0]), "MOL2 atom id")?;
        let mut atom_name = fields[1].to_string();
        let x = parse_f64(Some(fields[2]), "MOL2 x coordinate")?;
        let y = parse_f64(Some(fields[3]), "MOL2 y coordinate")?;
        let z = parse_f64(Some(fields[4]), "MOL2 z coordinate")?;
        let atom_type = fields[5];
        let mut residue_id = parse_i32(Some(fields[6]), "MOL2 residue id")?;
        let mut residue_name = fields[7].to_string();

        if residue_name == "<0>" {
            residue_name = compound_name.to_string();
        }
        if apply_biopolymer_names {
            if let Some((name, parsed_residue_id)) = split_biopolymer_residue(&residue_name) {
                residue_name = name;
                residue_id = parsed_residue_id;
            }
            if residue_id != previous_residue {
                name_counts.clear();
            }
        }

        let count = name_counts.entry(atom_name.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            atom_name = format!("{}{}", count, atom_name);
        }

        let element = element_from_mol2_type(atom_type)?;
        if !skip {
            let chain_id = if options.load_as_chain {
                mol2_chain_name(&options.chain_name, compound_id)
            } else {
                options.chain_name.clone()
            };
            let res_id = if options.load_as_chain {
                residue_id
            } else {
                options.residue_index + compound_id
            };
            molecule.push_atom(Atom {
                serial: (atom_offset + atom_map.len() + 1) as i32,
                name: atom_name,
                alt_loc: None,
                res_name: residue_name,
                chain_id,
                res_id,
                ins_code: None,
                position: Vector4::new3(x, y, z),
                occupancy: 1.0,
                b_factor: 0.0,
                element,
                formal_charge: None,
                int_props: Default::default(),
                real_props: Default::default(),
            });
            atom_map.insert(source_id, atom_offset + atom_map.len());
        }
        previous_residue = residue_id;
    }

    if bond_count == 0 || skip {
        if !skip {
            finish_mol2_topology(molecule, atom_offset, apply_biopolymer_names, mol_type);
        }
        return Ok(());
    }

    let Some(bond_marker) = find_section(lines, *index, "@<TRIPOS>BOND") else {
        finish_mol2_topology(molecule, atom_offset, apply_biopolymer_names, mol_type);
        return Ok(());
    };
    *index = bond_marker + 1;
    for _ in 0..bond_count {
        let line = lines
            .get(*index)
            .ok_or_else(|| CueMolError::parse("missing MOL2 bond line"))?;
        *index += 1;
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 4 {
            return Err(CueMolError::parse("invalid MOL2 bond record"));
        }
        let source_atom1 = parse_i32(Some(fields[1]), "MOL2 bond atom1")?;
        let source_atom2 = parse_i32(Some(fields[2]), "MOL2 bond atom2")?;
        let atom1 = *atom_map
            .get(&source_atom1)
            .ok_or_else(|| CueMolError::parse("MOL2 bond atom1 not found"))?;
        let atom2 = *atom_map
            .get(&source_atom2)
            .ok_or_else(|| CueMolError::parse("MOL2 bond atom2 not found"))?;
        molecule.add_bond(atom1, atom2, mol2_bond_order(fields[3]));
        molecule.mark_bond_persistent(atom1, atom2);
    }

    finish_mol2_topology(molecule, atom_offset, apply_biopolymer_names, mol_type);
    Ok(())
}

fn finish_mol2_topology(
    molecule: &mut MolCoord,
    atom_offset: usize,
    apply_biopolymer_topology: bool,
    molecule_type: &str,
) {
    if apply_biopolymer_topology {
        molecule.apply_topology();
        if molecule_type == "PROTEIN" {
            molecule.calc_prot_2ndry(-500.0, false);
        } else if molecule_type == "NUCLEIC_ACID" {
            molecule.calc_base_pair(3.7, 30.0);
        }
        return;
    }

    let residue_keys = molecule.atoms()[atom_offset..]
        .iter()
        .map(|atom| (atom.chain_id.clone(), atom.res_id))
        .collect::<std::collections::BTreeSet<_>>();
    for (chain_id, residue_id) in residue_keys {
        let _ =
            molecule.set_residue_prop_str(&chain_id, &residue_id.to_string(), "noautogen", "true");
    }
}

fn find_section(lines: &[&str], start: usize, marker: &str) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, line)| (line.trim_end() == marker).then_some(index))
}

fn parse_usize(value: Option<&str>, name: &str) -> Result<usize> {
    value
        .ok_or_else(|| CueMolError::parse(format!("missing {name}")))?
        .parse::<usize>()
        .map_err(|_| CueMolError::parse(format!("invalid {name}")))
}

fn parse_i32(value: Option<&str>, name: &str) -> Result<i32> {
    value
        .ok_or_else(|| CueMolError::parse(format!("missing {name}")))?
        .parse::<i32>()
        .map_err(|_| CueMolError::parse(format!("invalid {name}")))
}

fn parse_f64(value: Option<&str>, name: &str) -> Result<f64> {
    value
        .ok_or_else(|| CueMolError::parse(format!("missing {name}")))?
        .parse::<f64>()
        .map_err(|_| CueMolError::parse(format!("invalid {name}")))
}

fn element_from_mol2_type(atom_type: &str) -> Result<String> {
    let raw = atom_type
        .split('.')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CueMolError::parse("invalid MOL2 atom type"))?;
    let mut chars = raw.chars();
    let Some(first) = chars.next() else {
        return Err(CueMolError::parse("invalid MOL2 atom type"));
    };
    let mut element = String::new();
    element.push(first.to_ascii_uppercase());
    if let Some(second) = chars.next() {
        if second.is_ascii_lowercase() {
            element.push(second);
        }
    }
    Ok(element)
}

fn split_biopolymer_residue(input: &str) -> Option<(String, i32)> {
    if input.len() <= 3 {
        return None;
    }
    let residue_id = input.get(3..)?.parse::<i32>().ok()?;
    Some((input.get(..3)?.to_string(), residue_id))
}

fn mol2_bond_order(value: &str) -> BondOrder {
    match value {
        "1" => BondOrder::Single,
        "2" => BondOrder::Double,
        "3" => BondOrder::Triple,
        "ar" | "am" => BondOrder::Aromatic,
        other => other
            .parse::<i32>()
            .map(BondOrder::Other)
            .unwrap_or(BondOrder::Other(0)),
    }
}

fn mol2_chain_name(base: &str, compound_id: i32) -> String {
    if compound_id == 0 {
        base.to_string()
    } else {
        format!("{base}{compound_id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_minimal_mol2_atoms_and_bonds() {
        let input = "\
@<TRIPOS>MOLECULE
ligand
3 2 1 0 0
SMALL
USER_CHARGES
@<TRIPOS>ATOM
1 N 0.0 0.0 0.0 N.am 1 LIG -0.6
2 CA 1.0 0.0 0.0 C.3 1 LIG 0.2
3 O 1.0 1.0 0.0 O.2 1 LIG -0.4
@<TRIPOS>BOND
1 1 2 1
2 2 3 am
";
        let molecule = read_mol2_str(input).unwrap();
        assert_eq!(molecule.atom_count(), 3);
        assert_eq!(molecule.bond_count(), 2);
        assert_eq!(molecule.atoms()[0].element, "N");
        assert_eq!(molecule.atoms()[1].name, "CA");
        assert_eq!(molecule.bonds()[1].order, BondOrder::Aromatic);
        assert!(molecule.bonds().iter().all(|bond| bond.persist));
        assert_eq!(
            molecule
                .get_residue_prop_str("A", "1", "noautogen")
                .unwrap(),
            Some("true")
        );
    }

    #[test]
    fn mol2_reader_options_select_compound_and_chain_residue_mode() {
        let input = "\
@<TRIPOS>MOLECULE
first
1 0 0 0 0
SMALL
USER_CHARGES
@<TRIPOS>ATOM
1 C1 0.0 0.0 0.0 C.3 1 LIG 0.0
@<TRIPOS>MOLECULE
second
1 0 0 0 0
SMALL
USER_CHARGES
@<TRIPOS>ATOM
1 N1 1.0 0.0 0.0 N.3 1 LIG 0.0
";
        let mol = read_mol2_str(input).unwrap();
        assert_eq!(mol.atom_count(), 2);
        assert_eq!(mol.atoms()[0].chain_id, "A");
        assert_eq!(mol.atoms()[1].chain_id, "A1");

        let options = Mol2ReadOptions {
            load_compound_id: 1,
            load_as_chain: false,
            chain_name: "Z".to_string(),
            residue_index: 10,
        };
        let mol = read_mol2_str_with_options(input, &options).unwrap();
        assert_eq!(mol.atom_count(), 1);
        assert_eq!(mol.atoms()[0].name, "N1");
        assert_eq!(mol.atoms()[0].chain_id, "Z");
        assert_eq!(mol.atoms()[0].res_id, 11);
        assert_eq!(
            mol.get_residue_prop_str("Z", "11", "noautogen").unwrap(),
            Some("true")
        );
    }
}
