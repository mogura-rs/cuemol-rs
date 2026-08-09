use crate::molecule::{Atom, BondOrder, MolCoord};
use cuemol_core::{parse_cif, CifDocument, CifLoop, CueMolError, Result, Vector4};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AtomLookupKey {
    chain_id: String,
    res_id: i32,
    ins_code: Option<char>,
    atom_name: String,
}

impl AtomLookupKey {
    fn new(chain_id: &str, res_id: i32, ins_code: Option<char>, atom_name: &str) -> Self {
        Self {
            chain_id: chain_id.to_string(),
            res_id,
            ins_code,
            atom_name: atom_name.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmcifReadOptions {
    pub load_model: bool,
    pub load_anisou: bool,
    pub load_alt_conf: bool,
    pub load_secstr: bool,
    pub auto_topo_gen: bool,
}

impl Default for MmcifReadOptions {
    fn default() -> Self {
        Self {
            load_model: false,
            load_anisou: true,
            load_alt_conf: true,
            // C++ MmcifMolReader's constructor sets m_bLoadSecstr = true,
            // overriding the qif default — so secondary structure is loaded from
            // _struct_conf / _struct_sheet_range when present.
            load_secstr: true,
            auto_topo_gen: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MmcifReader {
    options: MmcifReadOptions,
}

impl MmcifReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn options(&self) -> MmcifReadOptions {
        self.options
    }

    pub fn set_options(&mut self, options: MmcifReadOptions) {
        self.options = options;
    }

    pub fn read_str(input: &str) -> Result<MolCoord> {
        read_mmcif_str(input)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<MolCoord> {
        read_mmcif_file(path)
    }

    pub fn read_str_configured(&self, input: &str) -> Result<MolCoord> {
        read_mmcif_str_with_options(input, self.options)
    }

    pub fn read_file_configured(&self, path: impl AsRef<Path>) -> Result<MolCoord> {
        read_mmcif_file_with_options(path, self.options)
    }
}

pub fn read_mmcif_file(path: impl AsRef<Path>) -> Result<MolCoord> {
    read_mmcif_file_with_options(path, MmcifReadOptions::default())
}

pub fn read_mmcif_file_with_options(
    path: impl AsRef<Path>,
    options: MmcifReadOptions,
) -> Result<MolCoord> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_mmcif_str_with_options(&input, options)
}

pub fn read_mmcif_str(input: &str) -> Result<MolCoord> {
    read_mmcif_str_with_options(input, MmcifReadOptions::default())
}

pub fn read_mmcif_str_with_options(input: &str, options: MmcifReadOptions) -> Result<MolCoord> {
    let document = parse_cif(input)?;
    let mut mol = MolCoord::new();
    let mut atom_keys = HashMap::new();
    let mut struct_conn_rows = Vec::new();
    let mut anisou_rows = Vec::new();
    // Secondary-structure ranges: (loop, value). _struct_conf -> "helix"
    // (HELX_P rows only), _struct_sheet_range -> "sheet".
    let mut secstr_loops: Vec<(&CifLoop, &'static str)> = Vec::new();
    let mut default_model: Option<String> = None;

    for cif_loop in document.loops() {
        if cif_loop.category_is("_atom_site") {
            let columns = AtomSiteColumns::new(cif_loop);
            for row in &cif_loop.rows {
                let model = columns.value(row, columns.model).unwrap_or("1");
                if default_model.is_none() {
                    default_model = Some(model.to_string());
                }
                if !options.load_model && default_model.as_deref() != Some(model) {
                    continue;
                }
                let Some(atom) = atom_from_row_with_options(
                    &columns,
                    row,
                    options,
                    model,
                    default_model.as_deref(),
                )?
                else {
                    continue;
                };
                let key =
                    AtomLookupKey::new(&atom.chain_id, atom.res_id, atom.ins_code, &atom.name);
                if atom_keys.get(&key).is_some_and(|indices: &Vec<usize>| {
                    indices
                        .iter()
                        .any(|&index| mol.atoms()[index].alt_loc == atom.alt_loc)
                }) {
                    continue;
                }
                atom_keys
                    .entry(key)
                    .or_insert_with(Vec::new)
                    .push(mol.atom_count());
                mol.push_atom(atom);
            }
        } else if cif_loop.category_is("_struct_conn") {
            struct_conn_rows.push(cif_loop);
        } else if cif_loop.category_is("_atom_site_anisotrop") && options.load_anisou {
            anisou_rows.push(cif_loop);
        } else if cif_loop.category_is("_struct_conf") && options.load_secstr {
            secstr_loops.push((cif_loop, "helix"));
        } else if cif_loop.category_is("_struct_sheet_range") && options.load_secstr {
            secstr_loops.push((cif_loop, "sheet"));
        }
    }

    if mol.atom_count() == 0 {
        return Err(CueMolError::parse("mmCIF contains no _atom_site rows"));
    }

    // Build connectivity from the residue-topology dictionary (correct bonds +
    // orders, peptide links, distance-gated disulfides), the same path the PDB
    // reader uses; the explicit `_struct_conn` records below add any remaining
    // covalent links (and dedup against template disulfides). See `crate::toppar`.
    mol.apply_topology_with_autogen(options.auto_topo_gen);
    mol.calc_base_pair(3.7, 30.0);
    for cif_loop in anisou_rows {
        apply_anisou(cif_loop, &mut mol);
    }
    for cif_loop in struct_conn_rows {
        apply_struct_conn(cif_loop, &atom_keys, &mut mol);
    }
    if options.load_secstr {
        for (cif_loop, value) in &secstr_loops {
            apply_secstr_range(cif_loop, &mut mol, value);
        }
    } else {
        mol.calc_prot_2ndry(-500.0, false);
    }
    // Crystallographic cell + space group come as single key-value items (not a
    // loop_), so parse those separately. The symm layer resolves the space group
    // to operators from these raw values.
    if let Some(cell) = build_cell_params(&parse_single_items(&document)) {
        mol.set_cell_params(cell);
    }
    Ok(mol)
}

/// Collect single (non-loop) `_category.item value` pairs from the shared CIF
/// document.
fn parse_single_items(document: &CifDocument) -> HashMap<String, String> {
    document
        .items()
        .map(|item| (item.name.to_ascii_lowercase(), item.value.clone()))
        .collect()
}

/// Build unit-cell parameters from mmCIF `_cell` / `_symmetry` single items.
/// Requires the three cell lengths; angles default to 90°. The space group is
/// carried as its H-M name and/or Int-Tables number (resolution is the symm
/// layer's job).
fn build_cell_params(items: &HashMap<String, String>) -> Option<cuemol_core::CellParams> {
    let num = |key: &str| items.get(key).and_then(|v| v.parse::<f64>().ok());
    let a = num("_cell.length_a")?;
    let b = num("_cell.length_b")?;
    let c = num("_cell.length_c")?;
    let mut cell = cuemol_core::CellParams::new(
        a,
        b,
        c,
        num("_cell.angle_alpha").unwrap_or(90.0),
        num("_cell.angle_beta").unwrap_or(90.0),
        num("_cell.angle_gamma").unwrap_or(90.0),
    );
    if let Some(name) = items.get("_symmetry.space_group_name_h-m") {
        if name != "?" {
            cell.space_group_name = name.clone();
        }
    }
    cell.space_group_number = items
        .get("_symmetry.int_tables_number")
        .and_then(|v| v.parse::<i32>().ok());
    Some(cell)
}

/// Apply secondary-structure ranges (C++ `MmcifMolReader::apply2ndry`): set the
/// residue `secondary` property to `value` ("helix"/"sheet") for every residue
/// in each row's begin..=end auth-sequence range. For `_struct_conf` only HELX_P
/// conformation types are helices (TURN_P etc. are skipped); `_struct_sheet_range`
/// rows are all sheets. Auth columns are used (label_ as fallback) to match the
/// auth ids the `_atom_site` parser stores. Missing residues in a range are
/// silently skipped.
fn apply_secstr_range(cif_loop: &CifLoop, mol: &mut MolCoord, value: &str) {
    let val = |row: &[String], primary: &str, fallback: &str| -> Option<String> {
        value_for(&cif_loop.columns, row, primary)
            .or_else(|| value_for(&cif_loop.columns, row, fallback))
            .map(str::to_string)
    };
    for row in &cif_loop.rows {
        let code = if value == "helix" {
            // C++ accepts only the protein-helix record and maps PDB helix
            // classes 5/3 to 3-10/pi helices.
            if value_for(&cif_loop.columns, row, "conf_type_id") != Some("HELX_P") {
                continue;
            }
            match value_for(&cif_loop.columns, row, "pdbx_PDB_helix_class")
                .and_then(|class| class.parse::<i32>().ok())
                .unwrap_or(1)
            {
                5 => 'G',
                3 => 'I',
                _ => 'H',
            }
        } else {
            'E'
        };
        let (Some(chain), Some(beg), Some(end)) = (
            val(row, "beg_auth_asym_id", "beg_label_asym_id"),
            val(row, "beg_auth_seq_id", "beg_label_seq_id"),
            val(row, "end_auth_seq_id", "end_label_seq_id"),
        ) else {
            continue;
        };
        let (Ok(beg), Ok(end)) = (beg.parse::<i32>(), end.parse::<i32>()) else {
            continue;
        };
        if value == "helix" && beg >= end {
            continue;
        }
        for res_id in beg..=end {
            let mut secondary2 = code.to_string();
            if res_id == beg {
                secondary2.push('s');
            } else if res_id == end {
                secondary2.push('e');
            }
            // Missing residues (gaps) just return NotFound — ignore.
            let _ = mol.set_residue_prop_str(&chain, &res_id.to_string(), "secondary", value);
            let _ =
                mol.set_residue_prop_str(&chain, &res_id.to_string(), "secondary2", &secondary2);
        }
    }
}

fn atom_from_row_with_options(
    columns: &AtomSiteColumns,
    row: &[String],
    options: MmcifReadOptions,
    model: &str,
    default_model: Option<&str>,
) -> Result<Option<Atom>> {
    let Some(serial) = columns
        .value(row, columns.id)
        .and_then(|value| value.parse().ok())
    else {
        return Ok(None);
    };
    let name = columns
        .preferred(row, columns.auth_atom_id, columns.label_atom_id)
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return Ok(None);
    }
    let res_name = columns
        .preferred(row, columns.auth_comp_id, columns.label_comp_id)
        .unwrap_or("")
        .to_string();
    if res_name.is_empty() {
        return Ok(None);
    }
    let mut chain_id = columns
        .preferred(row, columns.auth_asym_id, columns.label_asym_id)
        .unwrap_or("")
        .to_string();
    if chain_id.is_empty() {
        return Ok(None);
    }
    let res_id = columns
        .preferred(row, columns.auth_seq_id, columns.label_seq_id)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let ins_code = columns
        .value(row, columns.ins_code)
        .and_then(|value| value.chars().next());
    let mut alt_loc = columns
        .value(row, columns.alt_id)
        .and_then(|value| value.chars().next());
    let x = parse_f64(columns.value(row, columns.x), "Cartn_x")?;
    let y = parse_f64(columns.value(row, columns.y), "Cartn_y")?;
    let z = parse_f64(columns.value(row, columns.z), "Cartn_z")?;
    let occupancy = columns
        .value(row, columns.occupancy)
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let b_factor = columns
        .value(row, columns.b_factor)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let element = columns
        .value(row, columns.element)
        .map(str::to_string)
        .unwrap_or_else(|| infer_element(&name));
    let formal_charge = columns
        .value(row, columns.formal_charge)
        .and_then(|v| v.parse().ok());

    if !options.load_alt_conf {
        match alt_loc {
            Some('A') => alt_loc = None,
            Some(_) => return Ok(None),
            None => {}
        }
    }

    if options.load_model && default_model.is_some_and(|default| default != model) {
        let model_number = model.parse::<i32>().unwrap_or(1);
        chain_id = encode_model_in_chain(&chain_id, model_number);
    }

    Ok(Some(Atom {
        serial,
        name,
        alt_loc,
        res_name,
        chain_id,
        res_id,
        ins_code,
        position: Vector4::new3(x, y, z),
        occupancy,
        b_factor,
        element,
        formal_charge,
        int_props: Default::default(),
        real_props: Default::default(),
    }))
}

#[derive(Debug, Clone, Copy)]
struct AtomSiteColumns {
    id: Option<usize>,
    auth_atom_id: Option<usize>,
    label_atom_id: Option<usize>,
    auth_comp_id: Option<usize>,
    label_comp_id: Option<usize>,
    auth_asym_id: Option<usize>,
    label_asym_id: Option<usize>,
    auth_seq_id: Option<usize>,
    label_seq_id: Option<usize>,
    ins_code: Option<usize>,
    alt_id: Option<usize>,
    x: Option<usize>,
    y: Option<usize>,
    z: Option<usize>,
    occupancy: Option<usize>,
    b_factor: Option<usize>,
    element: Option<usize>,
    formal_charge: Option<usize>,
    model: Option<usize>,
}

impl AtomSiteColumns {
    fn new(cif_loop: &CifLoop) -> Self {
        Self {
            id: cif_loop.column_index("id"),
            auth_atom_id: cif_loop.column_index("auth_atom_id"),
            label_atom_id: cif_loop.column_index("label_atom_id"),
            auth_comp_id: cif_loop.column_index("auth_comp_id"),
            label_comp_id: cif_loop.column_index("label_comp_id"),
            auth_asym_id: cif_loop.column_index("auth_asym_id"),
            label_asym_id: cif_loop.column_index("label_asym_id"),
            auth_seq_id: cif_loop.column_index("auth_seq_id"),
            label_seq_id: cif_loop.column_index("label_seq_id"),
            ins_code: cif_loop.column_index("pdbx_PDB_ins_code"),
            alt_id: cif_loop.column_index("label_alt_id"),
            x: cif_loop.column_index("Cartn_x"),
            y: cif_loop.column_index("Cartn_y"),
            z: cif_loop.column_index("Cartn_z"),
            occupancy: cif_loop.column_index("occupancy"),
            b_factor: cif_loop.column_index("B_iso_or_equiv"),
            element: cif_loop.column_index("type_symbol"),
            formal_charge: cif_loop.column_index("pdbx_formal_charge"),
            model: cif_loop.column_index("pdbx_PDB_model_num"),
        }
    }

    fn value<'a>(&self, row: &'a [String], index: Option<usize>) -> Option<&'a str> {
        index
            .and_then(|index| row.get(index))
            .map(String::as_str)
            .filter(|value| !matches!(*value, "." | "?"))
    }

    fn preferred<'a>(
        &self,
        row: &'a [String],
        primary: Option<usize>,
        fallback: Option<usize>,
    ) -> Option<&'a str> {
        self.value(row, primary)
            .or_else(|| self.value(row, fallback))
    }
}

fn value_for<'a>(columns: &[String], row: &'a [String], name: &str) -> Option<&'a str> {
    columns
        .iter()
        .position(|column| {
            column
                .rsplit_once('.')
                .is_some_and(|(_, field)| field.eq_ignore_ascii_case(name))
        })
        .and_then(|index| row.get(index).map(String::as_str))
        .filter(|value| !matches!(*value, "." | "?"))
}

fn encode_model_in_chain(chain_id: &str, model: i32) -> String {
    if model <= 1 {
        chain_id.to_string()
    } else {
        format!("{model:02}_{chain_id}")
    }
}

fn infer_element(atom_name: &str) -> String {
    let mut chars = atom_name
        .trim_start_matches(|ch: char| ch.is_ascii_digit())
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic());
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut element = first.to_ascii_uppercase().to_string();
    if let Some(second) = chars.next().filter(char::is_ascii_lowercase) {
        element.push(second);
    }
    element
}

fn apply_anisou(cif_loop: &CifLoop, mol: &mut MolCoord) {
    let Some(id_index) = cif_loop.column_index("id") else {
        return;
    };
    let fields = [
        ("anisou_u11", "U[1][1]"),
        ("anisou_u22", "U[2][2]"),
        ("anisou_u33", "U[3][3]"),
        ("anisou_u12", "U[1][2]"),
        ("anisou_u13", "U[1][3]"),
        ("anisou_u23", "U[2][3]"),
    ];
    let field_indices = fields
        .iter()
        .filter_map(|(prop, column)| cif_loop.column_index(column).map(|index| (*prop, index)))
        .collect::<Vec<_>>();
    if field_indices.is_empty() {
        return;
    }
    let atom_indices = mol
        .atoms()
        .iter()
        .enumerate()
        .map(|(index, atom)| (atom.serial, index))
        .collect::<HashMap<_, _>>();
    for row in &cif_loop.rows {
        let serial = row
            .get(id_index)
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0);
        let Some(index) = atom_indices.get(&serial).copied() else {
            continue;
        };
        let atom = &mut mol.atoms_mut()[index];
        for (prop, index) in &field_indices {
            if let Some(value) = row.get(*index).and_then(|value| value.parse::<f64>().ok()) {
                atom.set_int_prop(*prop, (value * 10000.0).round() as i32);
            }
        }
    }
}

fn parse_f64(value: Option<&str>, name: &str) -> Result<f64> {
    value
        .ok_or_else(|| CueMolError::parse(format!("missing mmCIF {name}")))?
        .parse::<f64>()
        .map_err(|_| CueMolError::parse(format!("invalid mmCIF {name}")))
}

fn apply_struct_conn(
    cif_loop: &CifLoop,
    atom_keys: &HashMap<AtomLookupKey, Vec<usize>>,
    mol: &mut MolCoord,
) {
    // Resolve a `_struct_conn` partner's (chain, seq, insertion, atom, alt) with the SAME
    // auth-preferred-then-label scheme the atoms were stored under (`get_atom`:
    // auth_* `||` label_*), mirroring C++ `applyLink`'s `getAsymID`/`getResidIndex`/
    // `getAtomID` (MmcifMolReader.cpp:636-645). A label-only lookup misses every
    // atom whose `auth_seq_id != label_seq_id` (ligands/metals/glycans, where
    // `label_seq_id` is `.`), silently dropping the covalent bond.
    let pick = |row: &[String], auth: &str, label: &str| -> Option<String> {
        let read = |name: &str| -> Option<String> {
            cif_loop
                .column_index(name)
                .and_then(|i| row.get(i))
                .filter(|v| !matches!(v.as_str(), "." | "?" | ""))
                .cloned()
        };
        read(auth).or_else(|| read(label))
    };
    let read = |row: &[String], name: &str| -> Option<String> {
        cif_loop
            .column_index(name)
            .and_then(|i| row.get(i))
            .filter(|v| !matches!(v.as_str(), "." | "?" | ""))
            .cloned()
    };
    let partner_key = |row: &[String], n: u8| -> Option<(AtomLookupKey, Option<char>)> {
        let chain = pick(
            row,
            &format!("ptnr{n}_auth_asym_id"),
            &format!("ptnr{n}_label_asym_id"),
        )?;
        let seq = pick(
            row,
            &format!("ptnr{n}_auth_seq_id"),
            &format!("ptnr{n}_label_seq_id"),
        )?;
        let atom = pick(
            row,
            &format!("ptnr{n}_auth_atom_id"),
            &format!("ptnr{n}_label_atom_id"),
        )?;
        let ins_code =
            read(row, &format!("pdbx_ptnr{n}_PDB_ins_code")).and_then(|value| value.chars().next());
        let alt_loc =
            read(row, &format!("pdbx_ptnr{n}_label_alt_id")).and_then(|value| value.chars().next());
        Some((
            AtomLookupKey::new(&chain, seq.parse().unwrap_or(0), ins_code, &atom),
            alt_loc,
        ))
    };
    for row in &cif_loop.rows {
        let connection_type = value_for(&cif_loop.columns, row, "conn_type_id").unwrap_or("");
        if !connection_type.eq_ignore_ascii_case("covale")
            && !connection_type.eq_ignore_ascii_case("disulf")
        {
            continue;
        }
        if let (Some((key1, alt1)), Some((key2, alt2))) = (partner_key(row, 1), partner_key(row, 2))
        {
            let resolve = |key: &AtomLookupKey, alt: Option<char>| {
                atom_keys.get(key).and_then(|indices| {
                    alt.map_or_else(
                        || {
                            indices
                                .iter()
                                .copied()
                                .find(|&index| mol.atoms()[index].alt_loc.is_none())
                                .or_else(|| indices.first().copied())
                        },
                        |alt| {
                            indices
                                .iter()
                                .copied()
                                .find(|&index| mol.atoms()[index].alt_loc == Some(alt))
                        },
                    )
                })
            };
            if let (Some(atom1), Some(atom2)) = (resolve(&key1, alt1), resolve(&key2, alt2)) {
                mol.add_bond(atom1, atom2, BondOrder::Single);
                mol.mark_bond_persistent(atom1, atom2);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmcif_reader_options_filter_models_altloc_and_read_anisou() {
        let cif = "\
data_test
loop_
_atom_site.id
_atom_site.auth_atom_id
_atom_site.auth_comp_id
_atom_site.auth_asym_id
_atom_site.auth_seq_id
_atom_site.pdbx_PDB_ins_code
_atom_site.label_alt_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.type_symbol
_atom_site.pdbx_formal_charge
_atom_site.pdbx_PDB_model_num
1 CA ALA A 1 ? A 0.0 0.0 0.0 1.0 10.0 C 0 1
2 CA ALA A 1 ? B 1.0 0.0 0.0 1.0 10.0 C 0 1
3 N GLY A 2 ? . 2.0 0.0 0.0 1.0 10.0 N 0 2
#
loop_
_atom_site_anisotrop.id
_atom_site_anisotrop.U[1][1]
_atom_site_anisotrop.U[2][2]
_atom_site_anisotrop.U[3][3]
_atom_site_anisotrop.U[1][2]
_atom_site_anisotrop.U[1][3]
_atom_site_anisotrop.U[2][3]
1 0.1111 0.2222 0.3333 0.0444 0.0555 0.0666
#
";
        let mol = read_mmcif_str(cif).unwrap();
        assert_eq!(mol.atom_count(), 2);
        assert_eq!(mol.atoms()[0].alt_loc, Some('A'));
        assert_eq!(mol.atoms()[1].alt_loc, Some('B'));
        assert_eq!(mol.atoms()[0].int_prop("anisou_u11"), Some(1111));

        let mut options = MmcifReadOptions {
            load_alt_conf: false,
            ..MmcifReadOptions::default()
        };
        let mol = read_mmcif_str_with_options(cif, options).unwrap();
        assert_eq!(mol.atom_count(), 1);
        assert_eq!(mol.atoms()[0].serial, 1);
        assert_eq!(mol.atoms()[0].alt_loc, None);

        options.load_model = true;
        let mol = read_mmcif_str_with_options(cif, options).unwrap();
        assert_eq!(mol.atom_count(), 2);
        assert_eq!(mol.atoms()[1].serial, 3);
        assert_eq!(mol.atoms()[1].chain_id, "02_A");

        options.load_anisou = false;
        let mol = read_mmcif_str_with_options(cif, options).unwrap();
        assert_eq!(mol.atoms()[0].int_prop("anisou_u11"), None);
    }

    #[test]
    fn deposited_secondary_structure_sets_dssp_codes_and_markers() {
        let cif = "\
data_test
loop_
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
1 C CA ALA A 1 0.0 0.0 0.0
2 C CA ALA A 2 10.0 0.0 0.0
3 C CA ALA A 3 20.0 0.0 0.0
4 C CA ALA A 4 30.0 0.0 0.0
#
loop_
_struct_conf.conf_type_id
_struct_conf.beg_label_asym_id
_struct_conf.beg_label_seq_id
_struct_conf.end_label_seq_id
_struct_conf.pdbx_PDB_helix_class
HELX_P A 1 2 5
TURN_P A 3 4 .
#
loop_
_struct_sheet_range.beg_label_asym_id
_struct_sheet_range.beg_label_seq_id
_struct_sheet_range.end_label_seq_id
A 3 4
#
";
        let mol = read_mmcif_str(cif).unwrap();
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "secondary2").unwrap(),
            Some("Gs")
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "2", "secondary2").unwrap(),
            Some("Ge")
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "3", "secondary2").unwrap(),
            Some("Es")
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "4", "secondary2").unwrap(),
            Some("Ee")
        );
    }

    #[test]
    fn struct_conn_resolves_partners_by_auth_ids_like_the_atoms() {
        // A `_struct_conn` covale to a metal whose auth_seq_id (100) differs from
        // its label_seq_id (`.`). Atoms are stored under auth ids, so a label-only
        // lookup would search "B:0:FE" and miss — dropping the bond. The fix
        // resolves partners auth-first (C++ applyLink, MmcifMolReader.cpp:636), so
        // the covale bond is created. FE is placed 50 A away so no distance/topology
        // bond can mask the result: the ONLY path to a CA-FE bond is the covale.
        let cif = "\
data_test
loop_
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.auth_atom_id
_atom_site.auth_comp_id
_atom_site.auth_asym_id
_atom_site.auth_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
1 C CA ALA A 1 CA ALA A 1 0.0 0.0 0.0
2 FE FE FE2 B . FE FE2 A 100 50.0 0.0 0.0
#
loop_
_struct_conn.conn_type_id
_struct_conn.ptnr1_label_asym_id
_struct_conn.ptnr1_label_seq_id
_struct_conn.ptnr1_label_atom_id
_struct_conn.ptnr1_auth_asym_id
_struct_conn.ptnr1_auth_seq_id
_struct_conn.ptnr1_auth_atom_id
_struct_conn.ptnr2_label_asym_id
_struct_conn.ptnr2_label_seq_id
_struct_conn.ptnr2_label_atom_id
_struct_conn.ptnr2_auth_asym_id
_struct_conn.ptnr2_auth_seq_id
_struct_conn.ptnr2_auth_atom_id
covale A 1 CA A 1 CA B . FE A 100 FE
#
";
        let mol = read_mmcif_str(cif).unwrap();
        assert_eq!(mol.atom_count(), 2);
        // The FE atom is stored under its AUTH seq id (100), not label (`.`->0).
        assert_eq!(mol.atoms()[1].res_id, 100, "FE stored under auth_seq_id");
        // The covale bond exists — only resolvable via auth-preferred partner keys.
        assert_eq!(mol.bonds().len(), 1, "the covale bond must be created");
        let bond = &mol.bonds()[0];
        assert!(bond.persist);
        let pair = (bond.atom1.min(bond.atom2), bond.atom1.max(bond.atom2));
        assert_eq!(pair, (0, 1), "the bond connects CA (atom0) and FE (atom1)");
    }

    #[test]
    fn struct_conn_resolves_insertion_codes_and_alternate_conformers() {
        let cif = "\
data_test
loop_
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.pdbx_PDB_ins_code
_atom_site.label_alt_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
1 C C1 LIG A 10 B A 0.0 0.0 0.0
2 C C1 LIG A 10 B B 30.0 0.0 0.0
3 C C1 LIG A 10 A B 60.0 0.0 0.0
4 FE FE FE2 A 20 ? . 90.0 0.0 0.0
#
loop_
_struct_conn.conn_type_id
_struct_conn.ptnr1_label_asym_id
_struct_conn.ptnr1_label_seq_id
_struct_conn.ptnr1_label_atom_id
_struct_conn.pdbx_ptnr1_PDB_ins_code
_struct_conn.pdbx_ptnr1_label_alt_id
_struct_conn.ptnr2_label_asym_id
_struct_conn.ptnr2_label_seq_id
_struct_conn.ptnr2_label_atom_id
_struct_conn.pdbx_ptnr2_PDB_ins_code
_struct_conn.pdbx_ptnr2_label_alt_id
covale A 10 C1 B B A 20 FE ? .
#
";
        let mol = read_mmcif_str(cif).unwrap();
        assert_eq!(mol.bond_count(), 1);
        let bond = &mol.bonds()[0];
        assert!(bond.persist);
        let pair = (bond.atom1.min(bond.atom2), bond.atom1.max(bond.atom2));
        assert_eq!(pair, (1, 3));
    }

    #[test]
    fn mmcif_disabling_autogen_keeps_known_topology_and_ignores_non_covalent_conn() {
        let cif = "\
data_test
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
ATOM 1 N N ALA A 1 0.0 0.0 0.0
ATOM 2 C CA ALA A 1 10.0 0.0 0.0
#
loop_
_struct_conn.conn_type_id
_struct_conn.ptnr1_label_asym_id
_struct_conn.ptnr1_label_seq_id
_struct_conn.ptnr1_label_atom_id
_struct_conn.ptnr2_label_asym_id
_struct_conn.ptnr2_label_seq_id
_struct_conn.ptnr2_label_atom_id
hydrog A 1 N A 1 CA
#
";
        let options = MmcifReadOptions {
            auto_topo_gen: false,
            ..MmcifReadOptions::default()
        };
        let mol = read_mmcif_str_with_options(cif, options).unwrap();
        assert_eq!(mol.bond_count(), 1);
        assert!(!mol.bonds()[0].persist);
    }

    #[test]
    fn minimal_atom_site_defaults_optional_fields_and_guesses_elements() {
        let cif = "\
data_test
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
ATOM 1 N ALA A 1 0.0 0.0 0.0
ATOM 2 CB GLY A 2 1.5 0.0 0.0
";
        let mol = read_mmcif_str(cif).unwrap();
        assert_eq!(mol.atom_count(), 2);
        assert_eq!(mol.atoms()[0].res_id, 1);
        assert_eq!(mol.atoms()[1].res_id, 2);
        assert_eq!(mol.atoms()[0].occupancy, 1.0);
        assert_eq!(mol.atoms()[0].b_factor, 0.0);
        assert_eq!(mol.atoms()[0].element, "N");
        assert_eq!(mol.atoms()[1].element, "C");
    }

    #[test]
    fn large_atom_site_with_shuffled_columns_keeps_every_row() {
        const ATOM_COUNT: usize = 2_048;
        let mut cif = String::from(
            "data_large\n\
             loop_\n\
             _atom_site.Cartn_z\n\
             _atom_site.label_seq_id\n\
             _atom_site.label_comp_id\n\
             _atom_site.id\n\
             _atom_site.Cartn_x\n\
             _atom_site.label_atom_id\n\
             _atom_site.label_asym_id\n\
             _atom_site.Cartn_y\n",
        );
        for index in 0..ATOM_COUNT {
            let serial = index + 1;
            cif.push_str(&format!("0.0 {serial} ALA {serial} {index}.0 CA A 1.0\n"));
        }
        cif.push_str("#\n");

        let molecule = read_mmcif_str_with_options(
            &cif,
            MmcifReadOptions {
                auto_topo_gen: false,
                ..MmcifReadOptions::default()
            },
        )
        .unwrap();
        assert_eq!(molecule.atom_count(), ATOM_COUNT);
        assert_eq!(molecule.atoms()[0].serial, 1);
        assert_eq!(molecule.atoms()[ATOM_COUNT - 1].serial, ATOM_COUNT as i32);
        assert_eq!(
            molecule.atoms()[ATOM_COUNT - 1].position,
            Vector4::new3((ATOM_COUNT - 1) as f64, 1.0, 0.0)
        );
    }

    #[test]
    fn invalid_and_duplicate_atom_site_rows_are_skipped() {
        let cif = "\
data_test
loop_
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.label_alt_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
1 C C1 LIG A 1 . 0.0 0.0 0.0
2 C C1 LIG A 1 . 1.0 0.0 0.0
bad C C2 LIG A 1 . 2.0 0.0 0.0
3 C C3 LIG ? 1 . 3.0 0.0 0.0
#
";
        let mol = read_mmcif_str(cif).unwrap();
        assert_eq!(mol.atom_count(), 1);
        assert_eq!(mol.atoms()[0].name, "C1");
    }

    #[test]
    fn cif_tags_are_case_insensitive_and_inline_comments_are_ignored() {
        let cif = "\
data_one
loop_
_ATOM_SITE.GROUP_PDB
_ATOM_SITE.ID
_ATOM_SITE.TYPE_SYMBOL
_ATOM_SITE.LABEL_ATOM_ID
_ATOM_SITE.LABEL_COMP_ID
_ATOM_SITE.LABEL_ASYM_ID
_ATOM_SITE.LABEL_SEQ_ID
_ATOM_SITE.CARTN_X
_ATOM_SITE.CARTN_Y
_ATOM_SITE.CARTN_Z
ATOM 1 N N ALA A 1 0.0 0.0 0.0 # trailing comment
#
data_two
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
ATOM 2 C CA GLY B 2
3.8 0.0 0.0
";
        let mol = read_mmcif_str(cif).unwrap();
        assert_eq!(mol.atom_count(), 2);
        assert_eq!(mol.atoms()[0].name, "N");
        assert_eq!(mol.atoms()[1].chain_id, "B");
        assert_eq!(mol.atoms()[1].position.x, 3.8);
    }

    #[test]
    fn semicolon_text_blocks_do_not_create_spurious_loops() {
        let cif = "\
data_test
_struct.title
;Long title
loop_
_atom_site.fake prose
;
loop_
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
1 N N ALA A 1 0.0 0.0 0.0
";
        let mol = read_mmcif_str(cif).unwrap();
        assert_eq!(mol.atom_count(), 1);
        assert_eq!(mol.atoms()[0].name, "N");
    }

    #[test]
    fn single_cell_items_accept_separate_lines_and_quoted_values() {
        let cif = "\
data_test
_cell.length_a
10.0
_cell.length_b 11.0
_cell.length_c 12.0
_cell.angle_alpha 90.0
_cell.angle_beta 91.0
_cell.angle_gamma 92.0
_symmetry.space_group_name_H-M 'P 1'
loop_
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
1 C CA ALA A 1 0.0 0.0 0.0
";
        let mol = read_mmcif_str(cif).unwrap();
        let cell = mol.cell_params().unwrap();
        assert_eq!((cell.a, cell.b, cell.c), (10.0, 11.0, 12.0));
        assert_eq!((cell.alpha, cell.beta, cell.gamma), (90.0, 91.0, 92.0));
        assert_eq!(cell.space_group_name, "P 1");
    }
}
