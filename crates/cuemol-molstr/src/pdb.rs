use crate::molecule::{Atom, MolCoord};
use crate::selection::Selector;
use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_qsys::{InOutHandler, ObjReader, ObjWriter};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdbReader {
    reader: ObjReader,
    load_model: bool,
    load_anisou: bool,
    load_alt_conf: bool,
    build_secondary: bool,
    load_seg_id: bool,
    auto_topo_gen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfPdbReader {
    reader: PdbReader,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdbWriter {
    writer: ObjWriter,
    selection: Selector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdbReadOptions {
    pub load_model: bool,
    pub load_anisou: bool,
    pub load_alt_conf: bool,
    pub build_secondary: bool,
    pub load_seg_id: bool,
    pub auto_topo_gen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SsBondRecord {
    chain1: String,
    res_id1: i32,
    ins_code1: Option<char>,
    chain2: String,
    res_id2: i32,
    ins_code2: Option<char>,
}

impl Default for PdbReadOptions {
    fn default() -> Self {
        Self {
            load_model: false,
            load_anisou: true,
            load_alt_conf: true,
            build_secondary: true,
            load_seg_id: false,
            auto_topo_gen: true,
        }
    }
}

impl PdbReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new(
                "pdb",
                "PDB Coordinates (*.pdb;*.ent;*.pdb.gz)",
                "*.pdb; *.ent; *.pdb.gz",
            ),
            load_model: false,
            load_anisou: true,
            load_alt_conf: true,
            build_secondary: true,
            load_seg_id: false,
            auto_topo_gen: true,
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

    pub fn load_model(&self) -> bool {
        self.load_model
    }

    pub fn set_load_model(&mut self, value: bool) {
        self.load_model = value;
    }

    pub fn load_anisou(&self) -> bool {
        self.load_anisou
    }

    pub fn set_load_anisou(&mut self, value: bool) {
        self.load_anisou = value;
    }

    pub fn load_alt_conf(&self) -> bool {
        self.load_alt_conf
    }

    pub fn set_load_alt_conf(&mut self, value: bool) {
        self.load_alt_conf = value;
    }

    pub fn build_secondary(&self) -> bool {
        self.build_secondary
    }

    pub fn build2ndry(&self) -> bool {
        self.build_secondary()
    }

    pub fn build_2_ndry(&self) -> bool {
        self.build_secondary()
    }

    pub fn set_build_secondary(&mut self, value: bool) {
        self.build_secondary = value;
    }

    pub fn set_build2ndry(&mut self, value: bool) {
        self.set_build_secondary(value);
    }

    pub fn set_build_2_ndry(&mut self, value: bool) {
        self.set_build_secondary(value);
    }

    pub fn load_seg_id(&self) -> bool {
        self.load_seg_id
    }

    pub fn loadsegid(&self) -> bool {
        self.load_seg_id()
    }

    pub fn set_load_seg_id(&mut self, value: bool) {
        self.load_seg_id = value;
    }

    pub fn set_loadsegid(&mut self, value: bool) {
        self.set_load_seg_id(value);
    }

    pub fn auto_topo_gen(&self) -> bool {
        self.auto_topo_gen
    }

    pub fn set_auto_topo_gen(&mut self, value: bool) {
        self.auto_topo_gen = value;
    }

    pub fn read_str(input: &str) -> Result<MolCoord> {
        read_pdb_str(input)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<MolCoord> {
        read_pdb_file(path)
    }

    pub fn options(&self) -> PdbReadOptions {
        PdbReadOptions {
            load_model: self.load_model,
            load_anisou: self.load_anisou,
            load_alt_conf: self.load_alt_conf,
            build_secondary: self.build_secondary,
            load_seg_id: self.load_seg_id,
            auto_topo_gen: self.auto_topo_gen,
        }
    }

    pub fn read_str_configured(&self, input: &str) -> Result<MolCoord> {
        read_pdb_str_with_options(input, self.options())
    }

    pub fn read_file_configured(&self, path: impl AsRef<Path>) -> Result<MolCoord> {
        read_pdb_file_with_options(path, self.options())
    }
}

impl Default for PdbReader {
    fn default() -> Self {
        Self::new()
    }
}

impl QdfPdbReader {
    pub fn new() -> Self {
        let mut reader = PdbReader::new();
        reader.reader = ObjReader::new("qdfpdb", "QDF-PDB (*.qdfpdb)", "*.qdfpdb");
        reader.set_build_secondary(false);
        Self { reader }
    }

    pub fn reader(&self) -> &ObjReader {
        self.reader.reader()
    }

    pub fn reader_mut(&mut self) -> &mut ObjReader {
        self.reader.reader_mut()
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn options(&self) -> PdbReadOptions {
        self.reader.options()
    }

    pub fn build_secondary(&self) -> bool {
        self.reader.build_secondary()
    }

    pub fn build2ndry(&self) -> bool {
        self.build_secondary()
    }

    pub fn set_build_secondary(&mut self, value: bool) {
        self.reader.set_build_secondary(value);
    }

    pub fn read_str(input: &str) -> Result<MolCoord> {
        read_qdfpdb_str(input)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<MolCoord> {
        read_qdfpdb_file(path)
    }
}

impl Default for QdfPdbReader {
    fn default() -> Self {
        Self::new()
    }
}

impl PdbWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("pdb", "PDB coordinates (*.pdb)", "*.pdb"),
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

    pub fn set_selection(&mut self, value: Selector) {
        self.selection = value;
    }

    pub fn write_string(mol: &MolCoord) -> String {
        write_pdb_string(mol)
    }

    pub fn write_file(path: impl AsRef<Path>, mol: &MolCoord) -> Result<()> {
        write_pdb_file(path, mol)
    }
}

impl Default for PdbWriter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_pdb_file(path: impl AsRef<Path>) -> Result<MolCoord> {
    read_pdb_file_with_options(path, PdbReadOptions::default())
}

pub fn read_qdfpdb_file(path: impl AsRef<Path>) -> Result<MolCoord> {
    read_pdb_file_with_options(path, QdfPdbReader::new().options())
}

pub fn read_pdb_file_with_options(
    path: impl AsRef<Path>,
    options: PdbReadOptions,
) -> Result<MolCoord> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_pdb_str_with_options(&input, options)
}

pub fn write_pdb_file(path: impl AsRef<Path>, mol: &MolCoord) -> Result<()> {
    fs::write(path, write_pdb_string(mol))?;
    Ok(())
}

pub fn read_pdb_str(input: &str) -> Result<MolCoord> {
    read_pdb_str_with_options(input, PdbReadOptions::default())
}

pub fn read_qdfpdb_str(input: &str) -> Result<MolCoord> {
    read_pdb_str_with_options(input, QdfPdbReader::new().options())
}

pub fn read_pdb_str_with_options(input: &str, options: PdbReadOptions) -> Result<MolCoord> {
    let mut mol = MolCoord::new();
    let mut current_model = None;
    let mut default_model = None;
    let mut ssbonds = Vec::new();
    let mut secondary_ranges = Vec::new();
    for (line_number, line) in input.lines().enumerate() {
        if line.starts_with("ATOM  ") || line.starts_with("HETATM") {
            if default_model.is_none() {
                default_model = Some(current_model);
            }
            if !options.load_model && Some(current_model) != default_model {
                continue;
            }
            if let Some(atom) = parse_atom_line_with_options(
                line,
                line_number + 1,
                options,
                current_model,
                default_model,
            )? {
                mol.push_atom(atom);
            }
        } else if line.starts_with("ANISOU") && options.load_anisou {
            apply_anisou_line(&mut mol, line, line_number + 1)?;
        } else if line.starts_with("MODEL ") {
            current_model = parse_model_number(line);
        } else if line.starts_with("ENDMDL") {
            current_model = None;
        } else if line.starts_with("SSBOND") {
            ssbonds.push(parse_ssbond_line(line, line_number + 1)?);
        } else if line.starts_with("HELIX ") {
            if let Some(range) = parse_helix_range(line) {
                secondary_ranges.push(range);
            }
        } else if line.starts_with("SHEET ") {
            if let Some(range) = parse_sheet_range(line) {
                secondary_ranges.push(range);
            }
        }
    }
    if mol.atom_count() == 0 {
        return Err(CueMolError::parse("PDB contains no ATOM/HETATM records"));
    }
    if options.auto_topo_gen {
        mol.apply_topology();
    }
    apply_ssbond_records(&mut mol, &ssbonds);
    if options.build_secondary {
        apply_secondary_ranges(&mut mol, &secondary_ranges);
    }
    Ok(mol)
}

pub fn write_pdb_string(mol: &MolCoord) -> String {
    let mut output = String::new();
    output.push_str("REMARK   PDB File Generated by CueMol2\n");
    write_ssbond_records(&mut output, mol);

    let mut chains = mol.atoms().iter().enumerate().fold(
        BTreeMap::<String, Vec<(usize, &Atom)>>::new(),
        |mut chains, entry| {
            chains
                .entry(entry.1.chain_id.clone())
                .or_default()
                .push(entry);
            chains
        },
    );
    let empty_chain = chains.remove("");
    let underscore_chain = chains.remove("_");

    let mut serial = 1usize;
    for atoms in chains
        .values()
        .chain(underscore_chain.iter())
        .chain(empty_chain.iter())
    {
        write_chain_atoms(&mut output, atoms, &mut serial);
    }
    output.push_str("END\n");
    output
}

fn write_chain_atoms(output: &mut String, atoms: &[(usize, &Atom)], serial: &mut usize) {
    let mut residues = BTreeMap::<(i32, Option<char>), Vec<(usize, &Atom)>>::new();
    for (index, atom) in atoms {
        residues
            .entry((atom.res_id, atom.ins_code))
            .or_default()
            .push((*index, *atom));
    }

    for residue_atoms in residues.values_mut() {
        residue_atoms.sort_by_key(|(index, _)| *index);
        for (_, atom) in residue_atoms {
            let pdb_serial = *serial;
            output.push_str(&format_atom_line(pdb_serial, atom));
            if let Some(line) = format_anisou_line(pdb_serial, atom) {
                output.push_str(&line);
            }
            *serial += 1;
        }
    }
    output.push_str("TER\n");
    *serial += 1;
}

fn format_anisou_line(serial: usize, atom: &Atom) -> Option<String> {
    let values = [
        atom.int_prop("anisou_u11")?,
        atom.int_prop("anisou_u22")?,
        atom.int_prop("anisou_u33")?,
        atom.int_prop("anisou_u12")?,
        atom.int_prop("anisou_u13")?,
        atom.int_prop("anisou_u23")?,
    ];
    Some(format!(
        "ANISOU{serial:>5} {}{}{:>3} {}{:>4}{} {:>7}{:>7}{:>7}{:>7}{:>7}{:>7}      {:>2}{:>2}\n",
        format_atom_name(atom),
        atom.alt_loc.unwrap_or(' '),
        truncate_ascii(&atom.res_name, 3),
        format_chain_id(&atom.chain_id),
        atom.res_id,
        atom.ins_code.unwrap_or(' '),
        values[0],
        values[1],
        values[2],
        values[3],
        values[4],
        values[5],
        truncate_ascii(&atom.element, 2),
        format_charge(atom.formal_charge),
    ))
}

fn format_atom_line(serial: usize, atom: &Atom) -> String {
    format!(
        "ATOM  {serial:>5} {}{}{:>3} {}{:>4}{}   {:>8.3}{:>8.3}{:>8.3}{:>6.2}{:>6.2}          {:>2}{:>2}\n",
        format_atom_name(atom),
        atom.alt_loc.unwrap_or(' '),
        truncate_ascii(&atom.res_name, 3),
        format_chain_id(&atom.chain_id),
        atom.res_id,
        atom.ins_code.unwrap_or(' '),
        atom.position.x,
        atom.position.y,
        atom.position.z,
        atom.occupancy,
        atom.b_factor,
        truncate_ascii(&atom.element, 2),
        format_charge(atom.formal_charge),
    )
}

fn format_atom_name(atom: &Atom) -> String {
    let atom_name = atom.name.trim();
    let element = atom.element.trim().to_ascii_uppercase();
    if atom_name.len() >= 4 || element.is_empty() {
        return format!("{atom_name:>4}");
    }

    let atom_upper = atom_name.to_ascii_uppercase();
    if let Some(position) = atom_upper.find(&element) {
        let end = position + element.len();
        let element_part = &atom_name[..end.min(atom_name.len())];
        let modifier = atom_name.get(end..).unwrap_or("");
        if modifier.len() <= 2 {
            return format!("{element_part:>2}{modifier:<2}");
        }
    }

    format!("{atom_name:>4}")
}

fn format_chain_id(chain_id: &str) -> char {
    if chain_id == "_" {
        ' '
    } else {
        chain_id.chars().next().unwrap_or(' ')
    }
}

fn format_charge(charge: Option<i32>) -> String {
    match charge {
        Some(value) if value > 0 => format!("{}+", value.min(9)),
        Some(value) if value < 0 => format!("{}-", value.abs().min(9)),
        _ => "  ".to_string(),
    }
}

fn write_ssbond_records(output: &mut String, mol: &MolCoord) {
    let mut index = 1;
    for bond in mol.bonds() {
        let Some(atom1) = mol.atoms().get(bond.atom1) else {
            continue;
        };
        let Some(atom2) = mol.atoms().get(bond.atom2) else {
            continue;
        };
        if !is_cys_sg(atom1) || !is_cys_sg(atom2) {
            continue;
        }
        output.push_str(&format!(
            "SSBOND {index:>3} CYS {}{:>4}{}   CYS {}{:>4}{}\n",
            format_chain_id(&atom1.chain_id),
            atom1.res_id,
            atom1.ins_code.unwrap_or(' '),
            format_chain_id(&atom2.chain_id),
            atom2.res_id,
            atom2.ins_code.unwrap_or(' '),
        ));
        index += 1;
    }
}

fn truncate_ascii(input: &str, limit: usize) -> &str {
    input.get(..input.len().min(limit)).unwrap_or(input)
}

fn parse_atom_line_with_options(
    line: &str,
    line_number: usize,
    options: PdbReadOptions,
    current_model: Option<i32>,
    default_model: Option<Option<i32>>,
) -> Result<Option<Atom>> {
    let serial = parse_i32(field(line, 6, 11), "atom serial", line_number)?;
    let raw_name = field(line, 12, 16).trim().to_string();
    let mut alt_loc = single_char(field(line, 16, 17));
    let res_name = field(line, 17, 20).trim().to_string();
    let mut chain_id = if options.load_seg_id {
        field(line, 72, 76).trim().to_string()
    } else {
        field(line, 21, 22).trim().to_string()
    };
    let res_id = parse_i32(field(line, 22, 26), "residue id", line_number)?;
    let ins_code = single_char(field(line, 26, 27));
    let x = parse_f64(field(line, 30, 38), "x", line_number)?;
    let y = parse_f64(field(line, 38, 46), "y", line_number)?;
    let z = parse_f64(field(line, 46, 54), "z", line_number)?;
    let occupancy = parse_optional_f32(field(line, 54, 60)).unwrap_or(1.0);
    let b_factor = parse_optional_f32(field(line, 60, 66)).unwrap_or(0.0);
    let element_field = field(line, 76, 78).trim();
    let element = if element_field.is_empty() {
        infer_element(&raw_name)
    } else {
        element_field.to_string()
    };

    if !options.load_alt_conf {
        match alt_loc {
            Some('A') => alt_loc = None,
            Some(_) => return Ok(None),
            None => {}
        }
    }

    if options.load_model && default_model.is_some_and(|model| model != current_model) {
        if let Some(model) = current_model {
            chain_id = encode_model_in_chain(&chain_id, model);
        }
    }

    Ok(Some(Atom {
        serial,
        name: raw_name,
        alt_loc,
        res_name,
        chain_id,
        res_id,
        ins_code,
        position: Vector4::new3(x, y, z),
        occupancy,
        b_factor,
        element,
        formal_charge: None,
        int_props: Default::default(),
    }))
}

fn parse_model_number(line: &str) -> Option<i32> {
    line.get(6..)
        .unwrap_or("")
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<i32>().ok())
}

fn parse_ssbond_line(line: &str, line_number: usize) -> Result<SsBondRecord> {
    if let Some(record) = parse_fixed_ssbond_line(line) {
        return Ok(record);
    }

    let parts = line.split_whitespace().collect::<Vec<_>>();
    if parts.len() >= 8 {
        return Ok(SsBondRecord {
            chain1: parts[3].to_string(),
            res_id1: parts[4].parse().map_err(|_| {
                CueMolError::parse(format!("invalid SSBOND residue on PDB line {line_number}"))
            })?,
            ins_code1: None,
            chain2: parts[6].to_string(),
            res_id2: parts[7].parse().map_err(|_| {
                CueMolError::parse(format!("invalid SSBOND residue on PDB line {line_number}"))
            })?,
            ins_code2: None,
        });
    }

    Err(CueMolError::parse(format!(
        "invalid SSBOND record on PDB line {line_number}"
    )))
}

fn parse_fixed_ssbond_line(line: &str) -> Option<SsBondRecord> {
    Some(SsBondRecord {
        chain1: field(line, 15, 16).trim().to_string(),
        res_id1: field(line, 17, 21).trim().parse().ok()?,
        ins_code1: single_char(field(line, 21, 22)),
        chain2: field(line, 29, 30).trim().to_string(),
        res_id2: field(line, 31, 35).trim().parse().ok()?,
        ins_code2: single_char(field(line, 35, 36)),
    })
}

fn apply_ssbond_records(mol: &mut MolCoord, records: &[SsBondRecord]) {
    for record in records {
        let Some(atom1) =
            find_residue_atom(mol, &record.chain1, record.res_id1, record.ins_code1, "SG")
        else {
            continue;
        };
        let Some(atom2) =
            find_residue_atom(mol, &record.chain2, record.res_id2, record.ins_code2, "SG")
        else {
            continue;
        };
        mol.add_bond(atom1, atom2, crate::molecule::BondOrder::Single);
    }
}

fn find_residue_atom(
    mol: &MolCoord,
    chain_id: &str,
    res_id: i32,
    ins_code: Option<char>,
    atom_name: &str,
) -> Option<usize> {
    mol.atoms().iter().position(|atom| {
        atom.chain_id == chain_id
            && atom.res_id == res_id
            && atom.ins_code == ins_code
            && atom.matches_name_case_insensitive(atom_name)
    })
}

fn is_cys_sg(atom: &Atom) -> bool {
    atom.matches_residue_name_case_insensitive("CYS") && atom.matches_name_case_insensitive("SG")
}

fn encode_model_in_chain(chain_id: &str, model: i32) -> String {
    if model <= 1 {
        chain_id.to_string()
    } else {
        format!("{model:02}_{chain_id}")
    }
}

fn apply_anisou_line(mol: &mut MolCoord, line: &str, line_number: usize) -> Result<()> {
    let serial = parse_i32(field(line, 6, 11), "anisou atom serial", line_number)?;
    let values = [
        ("anisou_u11", field(line, 28, 35)),
        ("anisou_u22", field(line, 35, 42)),
        ("anisou_u33", field(line, 42, 49)),
        ("anisou_u12", field(line, 49, 56)),
        ("anisou_u13", field(line, 56, 63)),
        ("anisou_u23", field(line, 63, 70)),
    ];
    let parsed = values
        .into_iter()
        .map(|(name, value)| parse_i32(value, name, line_number).map(|parsed| (name, parsed)))
        .collect::<Result<Vec<_>>>()?;
    if let Some(atom) = mol
        .atoms_mut()
        .iter_mut()
        .find(|atom| atom.serial == serial)
    {
        for (name, value) in parsed {
            atom.set_int_prop(name, value);
        }
    }
    Ok(())
}

fn field(line: &str, start: usize, end: usize) -> &str {
    line.get(start..end).unwrap_or("")
}

/// A protein secondary-structure span from a HELIX or SHEET record.
#[derive(Debug, Clone)]
struct SecondaryRange {
    kind: &'static str,
    chain: String,
    start: i32,
    end: i32,
}

/// Parse a HELIX record. Columns (1-indexed): initChainID 20, initSeqNum 22-25,
/// endChainID 32, endSeqNum 34-37.
fn parse_helix_range(line: &str) -> Option<SecondaryRange> {
    let chain = field(line, 19, 20).trim();
    let start = field(line, 21, 25).trim().parse::<i32>().ok()?;
    let end = field(line, 33, 37).trim().parse::<i32>().ok()?;
    Some(SecondaryRange {
        kind: "helix",
        chain: chain.to_string(),
        start,
        end,
    })
}

/// Parse a SHEET record. Columns (1-indexed): initChainID 22, initSeqNum 23-26,
/// endChainID 33, endSeqNum 34-37.
fn parse_sheet_range(line: &str) -> Option<SecondaryRange> {
    let chain = field(line, 21, 22).trim();
    let start = field(line, 22, 26).trim().parse::<i32>().ok()?;
    let end = field(line, 33, 37).trim().parse::<i32>().ok()?;
    Some(SecondaryRange {
        kind: "sheet",
        chain: chain.to_string(),
        start,
        end,
    })
}

/// Apply parsed HELIX/SHEET spans as the residue `secondary` property
/// ("helix"/"sheet"). Mirrors the original `PDBFileReader::apply2ndry`. Residues
/// that are absent (gaps, insertion codes) are skipped silently.
fn apply_secondary_ranges(mol: &mut MolCoord, ranges: &[SecondaryRange]) {
    for range in ranges {
        if range.end < range.start {
            continue;
        }
        for res_id in range.start..=range.end {
            let _ =
                mol.set_residue_prop_str(&range.chain, &res_id.to_string(), "secondary", range.kind);
        }
    }
}

fn single_char(value: &str) -> Option<char> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.chars().next()
    }
}

fn parse_i32(value: &str, name: &str, line_number: usize) -> Result<i32> {
    value.trim().parse::<i32>().map_err(|_| {
        CueMolError::parse(format!(
            "invalid {name} on PDB line {line_number}: {value:?}"
        ))
    })
}

fn parse_f64(value: &str, name: &str, line_number: usize) -> Result<f64> {
    value.trim().parse::<f64>().map_err(|_| {
        CueMolError::parse(format!(
            "invalid {name} coordinate on PDB line {line_number}: {value:?}"
        ))
    })
}

fn parse_optional_f32(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse::<f32>().ok()
    }
}

fn infer_element(atom_name: &str) -> String {
    let trimmed = atom_name
        .trim_start_matches(|ch: char| ch.is_ascii_digit())
        .trim();
    let mut chars = trimmed.chars().filter(|ch| ch.is_ascii_alphabetic());
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut element = String::new();
    element.push(first.to_ascii_uppercase());
    if let Some(second) = chars.next() {
        if second.is_ascii_lowercase() {
            element.push(second);
        }
    }
    element
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_qsys::InOutHandler;

    #[test]
    fn pdb_reader_writer_state_matches_qif_defaults() {
        let reader = PdbReader::new();
        assert_eq!(reader.handler().name(), "pdb");
        assert_eq!(
            reader.handler().type_descr(),
            "PDB Coordinates (*.pdb;*.ent;*.pdb.gz)"
        );
        assert_eq!(reader.handler().file_ext(), "*.pdb; *.ent; *.pdb.gz");
        assert_eq!(reader.handler().category(), InOutHandler::IOH_CAT_OBJREADER);
        assert!(!reader.load_model());
        assert!(reader.load_anisou());
        assert!(reader.load_alt_conf());
        assert!(reader.build_secondary());
        assert!(!reader.load_seg_id());
        assert!(reader.auto_topo_gen());

        let qdfpdb = QdfPdbReader::new();
        assert_eq!(qdfpdb.handler().name(), "qdfpdb");
        assert_eq!(qdfpdb.handler().type_descr(), "QDF-PDB (*.qdfpdb)");
        assert_eq!(qdfpdb.handler().file_ext(), "*.qdfpdb");
        assert_eq!(qdfpdb.handler().category(), InOutHandler::IOH_CAT_OBJREADER);
        assert!(!qdfpdb.build_secondary());

        let writer = PdbWriter::new();
        assert_eq!(writer.handler().name(), "pdb");
        assert_eq!(writer.handler().type_descr(), "PDB coordinates (*.pdb)");
        assert_eq!(writer.handler().file_ext(), "*.pdb");
        assert_eq!(writer.handler().category(), InOutHandler::IOH_CAT_OBJWRITER);
        assert_eq!(writer.selection(), &Selector::All);
    }

    #[test]
    fn reads_helix_and_sheet_records_into_residue_secondary() {
        let pdb = "\
HELIX    1  H1 ALA A    1  ALA A    3  1
SHEET    1   A 1 ALA A   5  ALA A   6  0
ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00           C
ATOM      2  CA  ALA A   3       3.000   0.000   0.000  1.00  0.00           C
ATOM      3  CA  ALA A   5       6.000   0.000   0.000  1.00  0.00           C
ATOM      4  CA  ALA A   6       9.000   0.000   0.000  1.00  0.00           C
";
        let mol = read_pdb_str(pdb).unwrap();
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "secondary").unwrap(),
            Some("helix")
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "3", "secondary").unwrap(),
            Some("helix")
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "5", "secondary").unwrap(),
            Some("sheet")
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "6", "secondary").unwrap(),
            Some("sheet")
        );
    }

    #[test]
    fn skips_secondary_when_build_secondary_disabled() {
        let pdb = "\
HELIX    1  H1 ALA A    1  ALA A    3  1
ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00           C
ATOM      2  CA  ALA A   3       3.000   0.000   0.000  1.00  0.00           C
";
        let options = PdbReadOptions {
            build_secondary: false,
            ..PdbReadOptions::default()
        };
        let mol = read_pdb_str_with_options(pdb, options).unwrap();
        assert_eq!(mol.get_residue_prop_str("A", "1", "secondary").unwrap(), None);
    }

    #[test]
    fn reads_minimal_pdb_atom_records() {
        let pdb = "\
ATOM      1  N   THR A   1      17.047  14.099   3.625  1.00 13.79           N
ATOM      2  CA  THR A   1      16.967  12.784   4.338  1.00 10.80           C
ATOM      3  C   THR A   1      15.685  12.755   5.133  1.00  9.19           C
";
        let mol = read_pdb_str(pdb).unwrap();
        assert_eq!(mol.atom_count(), 3);
        assert_eq!(mol.chain_ids(), vec!["A"]);
        assert_eq!(mol.residue_count(), 1);
        assert_eq!(mol.atoms()[1].name, "CA");
        assert_eq!(mol.atoms()[1].element, "C");
        assert_eq!(mol.bond_count(), 2);

        let qdfpdb = read_qdfpdb_str(pdb).unwrap();
        assert_eq!(qdfpdb.atom_count(), 3);
        assert_eq!(qdfpdb.atoms()[0].name, "N");
    }

    #[test]
    fn pdb_reader_auto_topology_option_controls_bond_generation() {
        let pdb = "\
ATOM      1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C
ATOM      2  C2  LIG A   1       1.400   0.000   0.000  1.00 10.00           C
ATOM      3  C3  LIG A   1       5.000   0.000   0.000  1.00 10.00           C
";
        let mol = read_pdb_str(pdb).unwrap();
        assert_eq!(mol.bond_count(), 1);

        let options = PdbReadOptions {
            auto_topo_gen: false,
            ..PdbReadOptions::default()
        };
        let mol = read_pdb_str_with_options(pdb, options).unwrap();
        assert_eq!(mol.bond_count(), 0);
    }

    #[test]
    fn pdb_reader_options_filter_models_altloc_and_read_anisou() {
        let pdb = "\
MODEL        1
ATOM      1  CA AALA A   1       0.000   0.000   0.000  1.00 10.00           C
ANISOU    1  CA AALA A   1     1111   2222   3333    444    555    666       C
ATOM      2  CA BALA A   1       1.000   0.000   0.000  1.00 10.00           C
ENDMDL
MODEL        2
ATOM      3  N   GLY A   2       2.000   0.000   0.000  1.00 10.00           N
ENDMDL
";
        let mol = read_pdb_str(pdb).unwrap();
        assert_eq!(mol.atom_count(), 2);
        assert_eq!(mol.atoms()[0].alt_loc, Some('A'));
        assert_eq!(mol.atoms()[1].alt_loc, Some('B'));
        assert_eq!(mol.atoms()[0].int_prop("anisou_u11"), Some(1111));

        let mut options = PdbReadOptions {
            load_alt_conf: false,
            ..PdbReadOptions::default()
        };
        let mol = read_pdb_str_with_options(pdb, options).unwrap();
        assert_eq!(mol.atom_count(), 1);
        assert_eq!(mol.atoms()[0].serial, 1);
        assert_eq!(mol.atoms()[0].alt_loc, None);

        options.load_model = true;
        let mol = read_pdb_str_with_options(pdb, options).unwrap();
        assert_eq!(mol.atom_count(), 2);
        assert_eq!(mol.atoms()[1].serial, 3);
        assert_eq!(mol.atoms()[1].chain_id, "02_A");

        options.load_anisou = false;
        let mol = read_pdb_str_with_options(pdb, options).unwrap();
        assert_eq!(mol.atoms()[0].int_prop("anisou_u11"), None);
    }

    #[test]
    fn pdb_writer_preserves_anisou_records_after_atoms() {
        let pdb = "\
ATOM      1  CA AALA A   1       0.000   0.000   0.000  1.00 10.00           C
ANISOU    1  CA AALA A   1     1111   2222   3333    444    555    666       C
";
        let mol = read_pdb_str(pdb).unwrap();
        let output = write_pdb_string(&mol);
        let lines = output.lines().collect::<Vec<_>>();
        assert!(lines[1].starts_with("ATOM      1  CA AALA A   1"));
        assert!(lines[2].starts_with("ANISOU    1  CA AALA A   1"));
        assert!(lines[2].contains("1111"));
        assert!(lines[2].contains("2222"));
        assert!(lines[2].contains("3333"));
        assert!(lines[2].contains("444"));
        assert!(lines[2].contains("555"));
        assert!(lines[2].contains("666"));

        let roundtrip = read_pdb_str(&output).unwrap();
        assert_eq!(roundtrip.atoms()[0].int_prop("anisou_u23"), Some(666));
    }

    #[test]
    fn pdb_reader_writer_roundtrips_ssbond_sg_linkage() {
        let pdb = "\
SSBOND   1 CYS A   1    CYS B   2
ATOM      1  SG  CYS A   1       0.000   0.000   0.000  1.00 10.00           S
ATOM      2  SG  CYS B   2       8.000   0.000   0.000  1.00 10.00           S
";
        let options = PdbReadOptions {
            auto_topo_gen: false,
            ..PdbReadOptions::default()
        };
        let mol = read_pdb_str_with_options(pdb, options).unwrap();
        assert_eq!(mol.bond_count(), 1);
        assert_eq!(mol.bonds()[0].atom1, 0);
        assert_eq!(mol.bonds()[0].atom2, 1);

        let output = write_pdb_string(&mol);
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines[1], "SSBOND   1 CYS A   1    CYS B   2 ");
        let roundtrip = read_pdb_str_with_options(&output, options).unwrap();
        assert_eq!(roundtrip.bond_count(), 1);
    }

    #[test]
    fn writes_minimal_pdb_atom_records() {
        let mol = read_pdb_str(
            "\
ATOM      1  N   THR A   1      17.047  14.099   3.625  1.00 13.79           N
ATOM      2  CA  THR A   1      16.967  12.784   4.338  1.00 10.80           C
",
        )
        .unwrap();
        let output = write_pdb_string(&mol);
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines[0], "REMARK   PDB File Generated by CueMol2");
        assert!(lines[1].starts_with("ATOM      1  N   THR A   1"));
        assert!(lines[2].starts_with("ATOM      2  CA  THR A   1"));
        assert_eq!(lines[3], "TER");
        assert_eq!(lines[4], "END");
    }
}
