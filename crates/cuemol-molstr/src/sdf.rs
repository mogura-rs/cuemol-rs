use crate::chem::ElemSym;
use crate::molecule::{Atom, BondOrder, MolCoord};
use crate::selection::Selector;
use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_qsys::{InOutHandler, ObjWriter};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
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

#[derive(Debug, Clone, PartialEq)]
pub struct SdfWriter {
    writer: ObjWriter,
    selection: Selector,
}

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
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("sdf", "MOL/SDF Coordinates (*.mol;*.sdf)", "*.mol; *.sdf"),
            selection: Selector::All,
        }
    }

    pub fn writer(&self) -> &ObjWriter {
        &self.writer
    }

    pub fn writer_mut(&mut self) -> &mut ObjWriter {
        &mut self.writer
    }

    pub fn handler(&self) -> &InOutHandler {
        self.writer.handler()
    }

    pub fn selection(&self) -> &Selector {
        &self.selection
    }

    pub fn get_selection(&self) -> &Selector {
        self.selection()
    }

    pub fn set_selection(&mut self, value: Selector) {
        self.selection = value;
    }

    pub fn write_string(mol: &MolCoord) -> String {
        write_sdf_string(mol)
    }

    pub fn write_file(path: impl AsRef<Path>, mol: &MolCoord) -> Result<()> {
        write_sdf_file(path, mol)
    }

    pub fn write_string_configured(&self, mol: &MolCoord) -> String {
        write_sdf_string_with_selection(mol, &self.selection)
    }

    pub fn write_file_configured(&self, path: impl AsRef<Path>, mol: &MolCoord) -> Result<()> {
        fs::write(path, self.write_string_configured(mol))?;
        Ok(())
    }
}

impl Default for SdfWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for SdfWriter {
    type Target = ObjWriter;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl std::ops::DerefMut for SdfWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
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
        let compound_name = lines[index].trim().to_string();
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
        let mut element_counts = BTreeMap::<String, usize>::new();
        for _ in 0..atom_count {
            let line = lines
                .get(index)
                .ok_or_else(|| CueMolError::parse("missing SDF atom line"))?;
            index += 1;
            let x = parse_sdf_f64(line, 0, 10)?;
            let y = parse_sdf_f64(line, 10, 20)?;
            let z = parse_sdf_f64(line, 20, 30)?;
            let raw_element = line.get(31..34).unwrap_or("").trim();
            let element_id = ElemSym::from_name(raw_element);
            let element = if element_id == ElemSym::XX {
                raw_element.to_string()
            } else {
                element_id.name().to_string()
            };
            let element_count = element_counts.entry(element.clone()).or_default();
            *element_count += 1;
            let atom_name = format!("{element}{element_count}");
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
                    name: atom_name,
                    alt_loc: None,
                    res_name: compound_name.clone(),
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
                let order = if order >= 4 {
                    BondOrder::Aromatic
                } else {
                    BondOrder::from_sdf(order as i32)
                };
                mol.add_bond_stereo(*atom1, *atom2, order, stereo);
                mol.mark_bond_persistent(*atom1, *atom2);
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
        if !skip {
            if let Some(atom_index) = atom_map.iter().flatten().next().copied() {
                let atom = &mol.atoms()[atom_index];
                let chain_id = atom.chain_id.clone();
                let residue_index = atom.res_id.to_string();
                let _ = mol.set_residue_prop_str(&chain_id, &residue_index, "noautogen", "true");
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

pub fn write_sdf_file(path: impl AsRef<Path>, mol: &MolCoord) -> Result<()> {
    fs::write(path, write_sdf_string(mol))?;
    Ok(())
}

pub fn write_sdf_string(mol: &MolCoord) -> String {
    write_sdf_string_with_selection(mol, &Selector::All)
}

/// Write one V2000 record per residue, matching `SDFMolWriter::writeResidue`.
/// Bonds crossing residue boundaries are intentionally omitted because an SDF
/// record cannot refer to atoms in another record.
pub fn write_sdf_string_with_selection(mol: &MolCoord, selection: &Selector) -> String {
    let mut output = String::new();
    for residue in mol.residue_summaries() {
        let atom_indices = residue
            .atom_indices
            .into_iter()
            .filter(|&index| selection.matches(&mol.atoms()[index]))
            .collect::<Vec<_>>();
        if atom_indices.is_empty() {
            continue;
        }
        write_sdf_residue(&mut output, mol, &atom_indices);
    }
    output
}

fn write_sdf_residue(output: &mut String, mol: &MolCoord, atom_indices: &[usize]) {
    let selected = atom_indices.iter().copied().collect::<BTreeSet<_>>();
    let bonds = mol
        .bonds()
        .iter()
        .filter(|bond| selected.contains(&bond.atom1) && selected.contains(&bond.atom2))
        .collect::<Vec<_>>();

    output.push_str("SDFile Generated by CueMol2\n");
    output.push_str("     RDKit          3D\n");
    output.push('\n');
    output.push_str(&format!(
        "{:>3}{:>3}  0  0  0  0  0  0  0  0999 V2000\n",
        atom_indices.len(),
        bonds.len()
    ));

    let mut local_indices = BTreeMap::new();
    for (local_index, &atom_index) in atom_indices.iter().enumerate() {
        let atom = &mol.atoms()[atom_index];
        local_indices.insert(atom_index, local_index + 1);
        let element = atom.element.chars().take(3).collect::<String>();
        output.push_str(&format!(
            "{:>10.4}{:>10.4}{:>10.4}{:>3}  0  0  0  0  0  0  0  0  0  0  0  0\n",
            atom.position.x, atom.position.y, atom.position.z, element
        ));
    }

    for bond in bonds {
        let atom1 = local_indices[&bond.atom1];
        let atom2 = local_indices[&bond.atom2];
        let order = match bond.order {
            BondOrder::Single => 1,
            BondOrder::Double => 2,
            BondOrder::Triple => 3,
            BondOrder::Aromatic => 4,
            BondOrder::Other(_) => 8,
        };
        output.push_str(&format!("{:>3}{:>3}{:>3}  0\n", atom1, atom2, order));
    }

    let charges = atom_indices
        .iter()
        .filter_map(|&atom_index| {
            mol.atoms()[atom_index]
                .formal_charge
                .map(|charge| (local_indices[&atom_index], charge))
        })
        .collect::<Vec<_>>();
    for chunk in charges.chunks(8) {
        output.push_str(&format!("M  CHG{:>3}", chunk.len()));
        for (index, charge) in chunk {
            output.push_str(&format!("{index:>4}{charge:>4}"));
        }
        output.push('\n');
    }
    output.push_str("M  END\n$$$$\n");
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
    use crate::parse_selector;

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
        assert_eq!(mol.atoms()[0].res_name, "first");
        assert_eq!(mol.atoms()[0].name, "C1");
        assert_eq!(mol.atoms()[1].res_id, 2);
        assert_eq!(mol.atoms()[1].res_name, "second");
        assert_eq!(mol.atoms()[1].name, "N1");
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "noautogen").unwrap(),
            Some("true")
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "2", "noautogen").unwrap(),
            Some("true")
        );

        let options = SdfReadOptions {
            load_compound_id: 1,
            load_as_chain: true,
            chain_name: "Z".to_string(),
            residue_index: 5,
        };
        let mol = read_sdf_str_with_options(TWO_COMPOUNDS, &options).unwrap();
        assert_eq!(mol.atom_count(), 1);
        assert_eq!(mol.atoms()[0].element, "N");
        assert_eq!(mol.atoms()[0].res_name, "second");
        assert_eq!(mol.atoms()[0].name, "N1");
        assert_eq!(mol.atoms()[0].chain_id, "Z1");
        assert_eq!(mol.atoms()[0].res_id, 5);
    }

    #[test]
    fn sdf_reader_numbers_repeated_elements_within_each_compound() {
        let input = concat!(
            "ligand\n",
            "  CueMol\n",
            "\n",
            "  2  0  0  0  0  0  0  0  0  0999 V2000\n",
            "    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0\n",
            "    1.0000    0.0000    0.0000 c   0  0  0  0  0  0  0  0  0  0  0  0\n",
            "M  END\n",
            "$$$$\n",
        );
        let mol = read_sdf_str(input).unwrap();
        assert_eq!(mol.atoms()[0].name, "C1");
        assert_eq!(mol.atoms()[1].name, "C2");
        assert_eq!(mol.atoms()[1].element, "C");
        assert!(mol.atoms().iter().all(|atom| atom.res_name == "ligand"));
    }

    #[test]
    fn sdf_reader_maps_extended_bond_types_to_delocalized() {
        let input = concat!(
            "ligand\n",
            "  CueMol\n",
            "\n",
            "  2  1  0  0  0  0  0  0  0  0999 V2000\n",
            "    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0\n",
            "    1.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0\n",
            "  1  2  8  0\n",
            "M  END\n",
            "$$$$\n",
        );
        let mol = read_sdf_str(input).unwrap();
        assert_eq!(mol.bonds()[0].order, BondOrder::Aromatic);
    }

    #[test]
    fn sdf_writer_emits_one_record_per_residue_with_local_indices() {
        let mut mol = MolCoord::new();
        mol.push_atom(test_atom(10, "C1", "C", 1, 0.0, None));
        mol.push_atom(test_atom(20, "O1", "O", 1, 1.0, Some(1)));
        mol.push_atom(test_atom(30, "N1", "N", 2, 2.0, Some(-1)));
        mol.push_atom(test_atom(40, "C2", "C", 2, 3.0, None));
        mol.add_bond(0, 1, BondOrder::Double);
        mol.add_bond(2, 3, BondOrder::Other(99));
        mol.add_bond(1, 2, BondOrder::Single);

        let output = write_sdf_string(&mol);
        assert_eq!(output.matches("$$$$\n").count(), 2);
        let records = output.split("$$$$\n").collect::<Vec<_>>();
        assert_eq!(
            records[0].lines().nth(3),
            Some("  2  1  0  0  0  0  0  0  0  0999 V2000")
        );
        assert!(records[0].contains("  1  2  2  0\n"));
        assert!(records[0].contains("M  CHG  1   2   1\n"));
        assert_eq!(
            records[1].lines().nth(3),
            Some("  2  1  0  0  0  0  0  0  0  0999 V2000")
        );
        assert!(records[1].contains("  1  2  8  0\n"));
        assert!(records[1].contains("M  CHG  1   1  -1\n"));

        let roundtrip = read_sdf_str(&output).unwrap();
        assert_eq!(roundtrip.atom_count(), 4);
        assert_eq!(roundtrip.bond_count(), 2);
        assert_eq!(roundtrip.atoms()[1].formal_charge, Some(1));
        assert_eq!(roundtrip.atoms()[2].formal_charge, Some(-1));
        assert_eq!(roundtrip.bonds()[1].order, BondOrder::Aromatic);
        assert!(roundtrip.bonds().iter().all(|bond| bond.persist));
    }

    #[test]
    fn configured_sdf_writer_applies_selection_and_exposes_handler_metadata() {
        let mut mol = MolCoord::new();
        mol.push_atom(test_atom(1, "C1", "C", 1, 0.0, None));
        mol.push_atom(test_atom(2, "C2", "C", 2, 1.0, None));

        let mut writer = SdfWriter::new();
        assert_eq!(writer.handler().name(), "sdf");
        assert_eq!(
            writer.handler().type_descr(),
            "MOL/SDF Coordinates (*.mol;*.sdf)"
        );
        writer.set_selection(parse_selector("resi 2").unwrap());

        let output = writer.write_string_configured(&mol);
        assert_eq!(output.matches("$$$$\n").count(), 1);
        assert_eq!(
            output.lines().nth(3),
            Some("  1  0  0  0  0  0  0  0  0  0999 V2000")
        );
        assert!(output.contains("    1.0000    0.0000    0.0000  C"));
    }

    fn test_atom(
        serial: i32,
        name: &str,
        element: &str,
        res_id: i32,
        x: f64,
        formal_charge: Option<i32>,
    ) -> Atom {
        Atom {
            serial,
            name: name.to_string(),
            alt_loc: None,
            res_name: "LIG".to_string(),
            chain_id: "A".to_string(),
            res_id,
            ins_code: None,
            position: Vector4::new3(x, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: element.to_string(),
            formal_charge,
            int_props: BTreeMap::new(),
            real_props: BTreeMap::new(),
        }
    }
}
