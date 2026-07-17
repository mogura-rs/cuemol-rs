use crate::molecule::{Atom, BondOrder, MolCoord};
use cuemol_core::{CueMolError, Result, Vector4};
use std::collections::HashMap;
use std::path::Path;

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
    let loops = parse_loops(input)?;
    let mut mol = MolCoord::new();
    let mut atom_keys = HashMap::new();
    let mut struct_conn_rows = Vec::new();
    let mut anisou_rows = Vec::new();
    // Secondary-structure ranges: (loop, value). _struct_conf -> "helix"
    // (HELX_* rows only), _struct_sheet_range -> "sheet".
    let mut secstr_loops: Vec<(CifLoop, &'static str)> = Vec::new();
    let mut default_model: Option<String> = None;

    for cif_loop in loops {
        if cif_loop.category_is("_atom_site") {
            for row in &cif_loop.rows {
                let model = value_for(&cif_loop.columns, row, "pdbx_PDB_model_num")
                    .unwrap_or("1")
                    .to_string();
                if default_model.is_none() {
                    default_model = Some(model.clone());
                }
                if !options.load_model && default_model.as_deref() != Some(model.as_str()) {
                    continue;
                }
                let Some(atom) = atom_from_row_with_options(
                    &cif_loop.columns,
                    row,
                    options,
                    &model,
                    default_model.as_deref(),
                )?
                else {
                    continue;
                };
                let key = atom_key(&atom.chain_id, atom.res_id, &atom.name);
                atom_keys.insert(key, mol.atom_count());
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
    mol.apply_topology();
    for cif_loop in anisou_rows {
        apply_anisou(&cif_loop, &mut mol);
    }
    for cif_loop in struct_conn_rows {
        apply_struct_conn(&cif_loop, &atom_keys, &mut mol);
    }
    for (cif_loop, value) in &secstr_loops {
        apply_secstr_range(cif_loop, &mut mol, value);
    }
    // Crystallographic cell + space group come as single key-value items (not a
    // loop_), so parse those separately. The symm layer resolves the space group
    // to operators from these raw values.
    if let Some(cell) = build_cell_params(&parse_single_items(input)) {
        mol.set_cell_params(cell);
    }
    Ok(mol)
}

/// Collect single (non-loop) `_category.item value` pairs. A loop column header
/// is a lone `_`-token line (one token); a key-value item has the value on the
/// same line (>= 2 tokens); data rows do not start with `_`.
fn parse_single_items(input: &str) -> HashMap<String, String> {
    let mut items = HashMap::new();
    for line in cif_content_lines(input) {
        let line = line.trim();
        if !line.starts_with('_') {
            continue;
        }
        if let Ok(tokens) = tokenize_cif_line(line) {
            if tokens.len() >= 2 {
                items.insert(tokens[0].to_ascii_lowercase(), tokens[1].clone());
            }
        }
    }
    items
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
/// in each row's begin..=end auth-sequence range. For `_struct_conf` only HELX_*
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
        // _struct_conf carries non-helix conformation types too; keep only HELX_*.
        if value == "helix" {
            let conf = value_for(&cif_loop.columns, row, "conf_type_id").unwrap_or("");
            if !conf.starts_with("HELX") {
                continue;
            }
        }
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
        for res_id in beg..=end {
            // Missing residues (gaps) just return NotFound — ignore.
            let _ = mol.set_residue_prop_str(&chain, &res_id.to_string(), "secondary", value);
        }
    }
}

#[derive(Debug, Clone)]
struct CifLoop {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl CifLoop {
    fn category_is(&self, expected: &str) -> bool {
        self.columns
            .first()
            .and_then(|name| name.split_once('.').map(|(category, _)| category))
            .is_some_and(|category| category.eq_ignore_ascii_case(expected))
    }

    fn column_index(&self, suffix: &str) -> Option<usize> {
        self.columns.iter().position(|column| {
            column
                .rsplit_once('.')
                .is_some_and(|(_, field)| field.eq_ignore_ascii_case(suffix))
        })
    }
}

fn parse_loops(input: &str) -> Result<Vec<CifLoop>> {
    let lines = cif_content_lines(input);
    let mut loops = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if !line.eq_ignore_ascii_case("loop_") {
            index += 1;
            continue;
        }
        index += 1;
        let mut columns = Vec::new();
        while index < lines.len() {
            let line = lines[index].trim();
            if line.starts_with('_') {
                columns.push(line.to_string());
                index += 1;
            } else {
                break;
            }
        }
        let mut rows = Vec::new();
        let mut pending = Vec::new();
        while index < lines.len() {
            let line = lines[index].trim();
            if line.is_empty() {
                index += 1;
                continue;
            }
            if line == "#" {
                index += 1;
                break;
            }
            if line.eq_ignore_ascii_case("loop_")
                || line.starts_with('_')
                || starts_with_ignore_ascii_case(line, "data_")
            {
                break;
            }
            let tokens = tokenize_cif_line(line)?;
            pending.extend(tokens);
            if !columns.is_empty() {
                while pending.len() >= columns.len() {
                    rows.push(pending.drain(..columns.len()).collect());
                }
            }
            index += 1;
        }
        if !columns.is_empty() {
            loops.push(CifLoop { columns, rows });
        }
    }
    Ok(loops)
}

fn cif_content_lines(input: &str) -> Vec<&str> {
    let mut in_text_block = false;
    input
        .lines()
        .filter(|line| {
            if line.starts_with(';') {
                in_text_block = !in_text_block;
                return false;
            }
            !in_text_block
        })
        .collect()
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

fn tokenize_cif_line(line: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < line.len() {
        let ch = line[pos..].chars().next().unwrap();
        if ch.is_whitespace() {
            pos += ch.len_utf8();
            continue;
        }
        if ch == '#' {
            break;
        }
        if ch == '\'' || ch == '"' {
            let quote = ch;
            pos += ch.len_utf8();
            let start = pos;
            while pos < line.len() {
                let next = line[pos..].chars().next().unwrap();
                if next == quote {
                    tokens.push(line[start..pos].to_string());
                    pos += quote.len_utf8();
                    break;
                }
                pos += next.len_utf8();
            }
            if pos >= line.len() && !line.ends_with(quote) {
                return Err(CueMolError::parse("unterminated mmCIF quote"));
            }
            continue;
        }
        let start = pos;
        while pos < line.len() {
            let next = line[pos..].chars().next().unwrap();
            if next.is_whitespace() {
                break;
            }
            pos += next.len_utf8();
        }
        tokens.push(line[start..pos].to_string());
    }
    Ok(tokens)
}

fn atom_from_row_with_options(
    columns: &[String],
    row: &[String],
    options: MmcifReadOptions,
    model: &str,
    default_model: Option<&str>,
) -> Result<Option<Atom>> {
    let get = |name: &str| -> Option<&str> {
        columns
            .iter()
            .position(|column| {
                column
                    .rsplit_once('.')
                    .is_some_and(|(_, field)| field.eq_ignore_ascii_case(name))
            })
            .and_then(|index| row.get(index).map(String::as_str))
            .filter(|value| !matches!(*value, "." | "?"))
    };
    let serial = get("id").and_then(|v| v.parse().ok()).unwrap_or(0);
    let name = get("auth_atom_id")
        .or_else(|| get("label_atom_id"))
        .unwrap_or("")
        .to_string();
    let res_name = get("auth_comp_id")
        .or_else(|| get("label_comp_id"))
        .unwrap_or("")
        .to_string();
    let mut chain_id = get("auth_asym_id")
        .or_else(|| get("label_asym_id"))
        .unwrap_or("")
        .to_string();
    let res_id = get("auth_seq_id")
        .or_else(|| get("label_seq_id"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let ins_code = get("pdbx_PDB_ins_code").and_then(|value| value.chars().next());
    let mut alt_loc = get("label_alt_id").and_then(|value| value.chars().next());
    let x = parse_f64(get("Cartn_x"), "Cartn_x")?;
    let y = parse_f64(get("Cartn_y"), "Cartn_y")?;
    let z = parse_f64(get("Cartn_z"), "Cartn_z")?;
    let occupancy = get("occupancy").and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let b_factor = get("B_iso_or_equiv")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let element = get("type_symbol")
        .map(str::to_string)
        .unwrap_or_else(|| infer_element(&name));
    let formal_charge = get("pdbx_formal_charge").and_then(|v| v.parse().ok());

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
    }))
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
    for row in &cif_loop.rows {
        let serial = row
            .get(id_index)
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0);
        let Some(atom) = mol
            .atoms_mut()
            .iter_mut()
            .find(|atom| atom.serial == serial)
        else {
            continue;
        };
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

fn apply_struct_conn(cif_loop: &CifLoop, atom_keys: &HashMap<String, usize>, mol: &mut MolCoord) {
    // Resolve a `_struct_conn` partner's (chain, seq, atom) with the SAME
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
    let partner_key = |row: &[String], n: u8| -> Option<String> {
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
        Some(atom_key(&chain, seq.parse().unwrap_or(0), &atom))
    };
    for row in &cif_loop.rows {
        if let (Some(key1), Some(key2)) = (partner_key(row, 1), partner_key(row, 2)) {
            if let (Some(&atom1), Some(&atom2)) = (atom_keys.get(&key1), atom_keys.get(&key2)) {
                mol.add_bond(atom1, atom2, BondOrder::Single);
            }
        }
    }
}

fn atom_key(chain: &str, res_id: i32, atom_name: &str) -> String {
    format!("{chain}:{res_id}:{atom_name}")
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
        let pair = (bond.atom1.min(bond.atom2), bond.atom1.max(bond.atom2));
        assert_eq!(pair, (0, 1), "the bond connects CA (atom0) and FE (atom1)");
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
}
