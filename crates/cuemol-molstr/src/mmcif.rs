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
        if cif_loop.category() == "_atom_site" {
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
        } else if cif_loop.category() == "_struct_conn" {
            struct_conn_rows.push(cif_loop);
        } else if cif_loop.category() == "_atom_site_anisotrop" && options.load_anisou {
            anisou_rows.push(cif_loop);
        } else if cif_loop.category() == "_struct_conf" && options.load_secstr {
            secstr_loops.push((cif_loop, "helix"));
        } else if cif_loop.category() == "_struct_sheet_range" && options.load_secstr {
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
    Ok(mol)
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
    fn category(&self) -> &str {
        self.columns
            .first()
            .and_then(|name| name.split_once('.').map(|(category, _)| category))
            .unwrap_or("")
    }

    fn column_index(&self, suffix: &str) -> Option<usize> {
        self.columns.iter().position(|column| {
            column
                .rsplit_once('.')
                .is_some_and(|(_, field)| field == suffix)
        })
    }
}

fn parse_loops(input: &str) -> Result<Vec<CifLoop>> {
    let lines = input.lines().collect::<Vec<_>>();
    let mut loops = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if line != "loop_" {
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
        while index < lines.len() {
            let line = lines[index].trim();
            if line.is_empty() || line == "#" {
                index += 1;
                break;
            }
            if line == "loop_" || line.starts_with('_') || line.starts_with("data_") {
                break;
            }
            let tokens = tokenize_cif_line(line)?;
            if !tokens.is_empty() {
                if tokens.len() == columns.len() {
                    rows.push(tokens);
                } else if !columns.is_empty() && tokens.len() % columns.len() == 0 {
                    for chunk in tokens.chunks(columns.len()) {
                        rows.push(chunk.to_vec());
                    }
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

fn tokenize_cif_line(line: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < line.len() {
        let ch = line[pos..].chars().next().unwrap();
        if ch.is_whitespace() {
            pos += ch.len_utf8();
            continue;
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
                    .is_some_and(|(_, field)| field == name)
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
    let element = get("type_symbol").unwrap_or("").to_string();
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
                .is_some_and(|(_, field)| field == name)
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
    let p1_chain = cif_loop.column_index("ptnr1_label_asym_id");
    let p1_seq = cif_loop.column_index("ptnr1_label_seq_id");
    let p1_atom = cif_loop.column_index("ptnr1_label_atom_id");
    let p2_chain = cif_loop.column_index("ptnr2_label_asym_id");
    let p2_seq = cif_loop.column_index("ptnr2_label_seq_id");
    let p2_atom = cif_loop.column_index("ptnr2_label_atom_id");
    let (Some(p1_chain), Some(p1_seq), Some(p1_atom), Some(p2_chain), Some(p2_seq), Some(p2_atom)) =
        (p1_chain, p1_seq, p1_atom, p2_chain, p2_seq, p2_atom)
    else {
        return;
    };
    for row in &cif_loop.rows {
        let key1 = atom_key(
            &row[p1_chain],
            row[p1_seq].parse().unwrap_or(0),
            &row[p1_atom],
        );
        let key2 = atom_key(
            &row[p2_chain],
            row[p2_seq].parse().unwrap_or(0),
            &row[p2_atom],
        );
        if let (Some(&atom1), Some(&atom2)) = (atom_keys.get(&key1), atom_keys.get(&key2)) {
            mol.add_bond(atom1, atom2, BondOrder::Single);
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
}
