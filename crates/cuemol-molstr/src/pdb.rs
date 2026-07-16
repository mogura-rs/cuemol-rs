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

impl std::ops::Deref for PdbReader {
    type Target = ObjReader;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}

impl std::ops::DerefMut for PdbReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reader
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

impl std::ops::Deref for QdfPdbReader {
    type Target = PdbReader;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}

impl std::ops::DerefMut for QdfPdbReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reader
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

    pub fn get_selection(&self) -> &Selector {
        self.selection()
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

impl std::ops::Deref for PdbWriter {
    type Target = ObjWriter;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl std::ops::DerefMut for PdbWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
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
    let mut links = Vec::new();
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
        } else if line.starts_with("LINK  ") || line.starts_with("LINKR ") {
            if let Some(record) = parse_link_line(line) {
                links.push(record);
            }
        } else if line.starts_with("HELIX ") {
            if let Some(range) = parse_helix_range(line) {
                secondary_ranges.push(range);
            }
        } else if line.starts_with("SHEET ") {
            if let Some(range) = parse_sheet_range(line) {
                secondary_ranges.push(range);
            }
        } else if line.starts_with("CRYST1") {
            if let Some(cell) = parse_cryst1_line(line) {
                mol.set_cell_params(cell);
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
    apply_link_records(&mut mol, &links);
    // C++ PDBFileReader: when build2ndry is set (the default), secondary
    // structure is *computed* by DSSP and the deposited HELIX/SHEET records are
    // ignored; only when it is off are those records applied verbatim.
    if options.build_secondary {
        mol.calc_prot_2ndry(-500.0, false);
    } else {
        apply_secondary_ranges(&mut mol, &secondary_ranges);
    }
    mol.calc_base_pair(3.7, 30.0);
    Ok(mol)
}

pub fn write_pdb_string(mol: &MolCoord) -> String {
    let mut output = String::new();
    output.push_str("REMARK   PDB File Generated by CueMol2\n");
    write_cryst1_record(&mut output, mol);
    write_ssbond_records(&mut output, mol);
    write_links_records(&mut output, mol);
    write_secstr_records(&mut output, mol);

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

/// Write the CRYST1 unit-cell record (C++ `PDBCryst1Handler::write`,
/// PDBCryst1Handler.cpp:117-133). Emitted only when the molecule carries cell
/// params (mirrors C++ `if (pci.isnull()) return false;`) — the writer-side
/// counterpart of `parse_cryst1_line`, so a `.pdb`'s crystal info survives a
/// load -> write -> reload round-trip. Fixed columns: a/b/c `%9.3f`, alpha/beta/
/// gamma `%7.2f`, then ` %-10s` for the H-M space-group name + trailing 4 spaces
/// (C++ omits the optional Z value at column 67).
fn write_cryst1_record(output: &mut String, mol: &MolCoord) {
    let Some(cell) = mol.cell_params() else {
        return;
    };
    output.push_str(&format!(
        "CRYST1{:9.3}{:9.3}{:9.3}{:7.2}{:7.2}{:7.2} {:<10}    \n",
        cell.a, cell.b, cell.c, cell.alpha, cell.beta, cell.gamma, cell.space_group_name,
    ));
}

/// Write HELIX and SHEET records from the per-residue `secondary2` property
/// (C++ `PDBFileWriter::writeSecstr`, PDBFileWriter.cpp:493-622). `secondary2`
/// encodes the DSSP class + a segment marker: "H"/"G"/"I" (alpha / 3-10 / pi
/// helix) or "E" (strand), with a trailing "s" (segment start) / "e" (segment
/// end); an interior residue carries just the bare class char. Each start..end
/// run becomes one record; the helix type code is 1/5/3 for H/G/I (the PDB
/// convention C++ uses). A single-residue helix ("Hse") is treated as a start
/// with no matching end and dropped — faithful to C++'s `pfx = sec.substr(1,1)`.
/// Write LINK records for deposited inter-residue bonds (C++ `PDBFileWriter::
/// writeLinks`, PDBFileWriter.cpp:427-491): persistent bonds (created from a LINK
/// record) that are not disulfides (those are written as SSBOND). The columns
/// follow the STANDARD PDB LINK layout — exactly what `parse_link_line` (and C++
/// `readLink`, PDBFileReader.cpp:814-838) parse — rather than C++ `writeLinks`'s
/// own layout, which omits the altLoc column and lands one short of `readLink`'s
/// fields (so it does not round-trip even in C++). Standard columns keep the LINK
/// re-readable, which is the whole point of writing it back.
fn write_links_records(output: &mut String, mol: &MolCoord) {
    for bond in mol.bonds() {
        if !bond.persist {
            continue;
        }
        let (Some(atom1), Some(atom2)) = (mol.atoms().get(bond.atom1), mol.atoms().get(bond.atom2))
        else {
            continue;
        };
        // Disulfides go in SSBOND, not LINK (C++ skips CYS-SG..CYS-SG).
        if is_cys_sg(atom1) && is_cys_sg(atom2) {
            continue;
        }
        output.push_str(&format!(
            "LINK        {} {:>3} {}{:>4}{}               {} {:>3} {}{:>4}{}\n",
            format_atom_name(atom1),
            truncate_ascii(&atom1.res_name, 3),
            format_chain_id(&atom1.chain_id),
            atom1.res_id,
            atom1.ins_code.unwrap_or(' '),
            format_atom_name(atom2),
            truncate_ascii(&atom2.res_name, 3),
            format_chain_id(&atom2.chain_id),
            atom2.res_id,
            atom2.ins_code.unwrap_or(' '),
        ));
    }
}

fn write_secstr_records(output: &mut String, mol: &MolCoord) {
    // The residue fields a HELIX/SHEET record needs from its start/end residue.
    struct SecResidue {
        res_name: String,
        chain_id: String,
        res_id: i32,
        ins_code: char,
    }
    let residues = mol.residue_summaries();
    let sec2 = |summary: &crate::ResidueSummary| -> Option<String> {
        mol.get_residue_prop_str(
            &summary.key.chain_id,
            &summary.key.script_index(),
            "secondary2",
        )
        .ok()
        .flatten()
        .map(str::to_string)
    };
    let res_of = |summary: &crate::ResidueSummary| SecResidue {
        res_name: truncate_ascii(&summary.res_name, 3).to_string(),
        chain_id: truncate_ascii(&summary.key.chain_id, 1).to_string(),
        res_id: summary.key.res_id,
        ins_code: summary.key.ins_code.unwrap_or(' '),
    };

    // HELIX records (DSSP classes H/G/I), pairing each "s" start to its "e" end.
    let mut nhx = 1;
    let mut start: Option<SecResidue> = None;
    for summary in &residues {
        let Some(sec) = sec2(summary) else { continue };
        let first = sec.chars().next();
        if !matches!(first, Some('H' | 'G' | 'I')) {
            continue;
        }
        match sec.chars().nth(1) {
            Some('s') => start = Some(res_of(summary)),
            Some('e') => {
                if let Some(start_res) = start.take() {
                    let end_res = res_of(summary);
                    // Type from the (same-class) end residue, mirroring C++.
                    let ntype = match first {
                        Some('G') => 5,
                        Some('I') => 3,
                        _ => 1,
                    };
                    output.push_str(&format!(
                        "HELIX  {:>3} {:>3} {:>3} {:>1} {:>4}{} {:>3} {:>1} {:>4}{}{:>2}\n",
                        nhx,
                        nhx,
                        start_res.res_name,
                        start_res.chain_id,
                        start_res.res_id,
                        start_res.ins_code,
                        end_res.res_name,
                        end_res.chain_id,
                        end_res.res_id,
                        end_res.ins_code,
                        ntype,
                    ));
                    nhx += 1;
                }
            }
            _ => {}
        }
    }

    // SHEET records (DSSP class E). Single strands per record (C++ writes 1).
    let mut nsh = 1;
    let mut start: Option<SecResidue> = None;
    for summary in &residues {
        let Some(sec) = sec2(summary) else { continue };
        if !sec.starts_with('E') {
            continue;
        }
        match sec.chars().nth(1) {
            Some('s') => start = Some(res_of(summary)),
            Some('e') => {
                if let Some(start_res) = start.take() {
                    let end_res = res_of(summary);
                    output.push_str(&format!(
                        "SHEET  {:>3} {:>3} 1 {:>3} {:>1}{:>4}{} {:>3} {:>1}{:>4}{} 0\n",
                        nsh,
                        nsh,
                        start_res.res_name,
                        start_res.chain_id,
                        start_res.res_id,
                        start_res.ins_code,
                        end_res.res_name,
                        end_res.chain_id,
                        end_res.res_id,
                        end_res.ins_code,
                    ));
                    nsh += 1;
                }
            }
            _ => {}
        }
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

/// A LINK record: an explicit covalent bond between two named atoms (C++
/// `PDBFileReader::readLink`). Used for non-disulfide linkages (metal
/// coordination, cross-links, modified residues) the topology dictionary misses.
#[derive(Debug, Clone)]
struct LinkRecord {
    name1: String,
    chain1: String,
    res_id1: i32,
    ins_code1: Option<char>,
    name2: String,
    chain2: String,
    res_id2: i32,
    ins_code2: Option<char>,
}

/// Parse a fixed-column PDB LINK record. Columns (1-indexed): name1 13-16,
/// chainID1 22, resSeq1 23-26, iCode1 27; name2 43-46, chainID2 52, resSeq2
/// 53-56, iCode2 57.
fn parse_link_line(line: &str) -> Option<LinkRecord> {
    let name1 = field(line, 12, 16).trim().to_string();
    let name2 = field(line, 42, 46).trim().to_string();
    if name1.is_empty() || name2.is_empty() {
        return None;
    }
    Some(LinkRecord {
        name1,
        chain1: field(line, 21, 22).trim().to_string(),
        res_id1: field(line, 22, 26).trim().parse().ok()?,
        ins_code1: single_char(field(line, 26, 27)),
        name2,
        chain2: field(line, 51, 52).trim().to_string(),
        res_id2: field(line, 52, 56).trim().parse().ok()?,
        ins_code2: single_char(field(line, 56, 57)),
    })
}

fn apply_link_records(mol: &mut MolCoord, records: &[LinkRecord]) {
    for record in records {
        let Some(atom1) = find_residue_atom(
            mol,
            &record.chain1,
            record.res_id1,
            record.ins_code1,
            &record.name1,
        ) else {
            continue;
        };
        let Some(atom2) = find_residue_atom(
            mol,
            &record.chain2,
            record.res_id2,
            record.ins_code2,
            &record.name2,
        ) else {
            continue;
        };
        // add_bond dedups, so a LINK duplicating a template/SSBOND bond is a no-op.
        mol.add_bond(atom1, atom2, crate::molecule::BondOrder::Single);
        // Mark it deposited (C++ `isPersist`) so the writer echoes it as a LINK
        // record — even if a template bond already existed between these atoms.
        mol.mark_bond_persistent(atom1, atom2);
    }
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

/// Parse a PDB `CRYST1` record into unit-cell parameters (C++ `PDBCryst1Handler`).
/// Columns (1-indexed): a 7-15, b 16-24, c 25-33, alpha 34-40, beta 41-47,
/// gamma 48-54, space-group H-M name 56-66. The H-M name is carried verbatim; the
/// symm layer resolves it to operators, exactly as the mmCIF `_cell`/`_symmetry`
/// path does (so a PDB and its mmCIF twin yield the same cell + space group).
fn parse_cryst1_line(line: &str) -> Option<cuemol_core::CellParams> {
    // Clamp the end to the line length so a CRYST1 that is not space-padded to
    // column 66 still yields its trailing space-group field.
    let col = |start: usize, end: usize| -> &str {
        let end = end.min(line.len());
        if start >= end {
            return "";
        }
        line.get(start..end).unwrap_or("").trim()
    };
    let num = |start, end| col(start, end).parse::<f64>().ok();
    let a = num(6, 15)?;
    let b = num(15, 24)?;
    let c = num(24, 33)?;
    let mut cell = cuemol_core::CellParams::new(
        a,
        b,
        c,
        num(33, 40).unwrap_or(90.0),
        num(40, 47).unwrap_or(90.0),
        num(47, 54).unwrap_or(90.0),
    );
    let sgroup = col(55, 66);
    if !sgroup.is_empty() {
        cell.space_group_name = sgroup.to_string();
    }
    Some(cell)
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
            let _ = mol.set_residue_prop_str(
                &range.chain,
                &res_id.to_string(),
                "secondary",
                range.kind,
            );
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
    fn parse_cryst1_reads_cell_and_space_group() {
        // The real 1CRN.pdb CRYST1 record (C++ PDBCryst1Handler parses it; the Rust
        // reader used to drop CRYST1 entirely while the mmCIF reader kept _cell).
        let line =
            "CRYST1   40.960   18.650   22.520  90.00  90.77  90.00 P 1 21 1      2          ";
        let cell = parse_cryst1_line(line).expect("CRYST1 parses");
        assert!((cell.a - 40.960).abs() < 1e-6);
        assert!((cell.b - 18.650).abs() < 1e-6);
        assert!((cell.c - 22.520).abs() < 1e-6);
        assert!((cell.alpha - 90.0).abs() < 1e-6);
        assert!((cell.beta - 90.77).abs() < 1e-6);
        assert!((cell.gamma - 90.0).abs() < 1e-6);
        assert_eq!(cell.space_group_name, "P 1 21 1");
        // And it flows through the reader onto the molecule's cell params.
        let pdb = format!(
            "{line}\nATOM      1  N   THR A   1      17.047  14.099   3.625  1.00 13.79           N\n"
        );
        let mol = read_pdb_str(&pdb).unwrap();
        let parsed = mol
            .cell_params()
            .expect("PDB CRYST1 -> cell params on the molecule");
        assert_eq!(parsed.space_group_name, "P 1 21 1");
        assert!((parsed.a - 40.960).abs() < 1e-6);
    }

    #[test]
    fn write_pdb_round_trips_the_cryst1_cell() {
        // The writer-side counterpart of parse_cryst1_line (C++ PDBCryst1Handler::
        // write). The reader populated cell_params but the writer dropped it, so a
        // .pdb's crystal info vanished on load -> write -> reload. Independent
        // oracle: the cell values come from the real 1CRN CRYST1 (known a=40.96
        // etc.), not the writer's own output — and the round-trip asserts they
        // survive byte-for-byte through the new writer.
        let line =
            "CRYST1   40.960   18.650   22.520  90.00  90.77  90.00 P 1 21 1      2          ";
        let pdb = format!(
            "{line}\nATOM      1  N   THR A   1      17.047  14.099   3.625  1.00 13.79           N\n"
        );
        let mol = read_pdb_str(&pdb).unwrap();
        assert!(mol.cell_params().is_some(), "reader populates cell params");

        let written = write_pdb_string(&mol);
        // The writer emits a CRYST1 with the cell + space group (C++ format:
        // a/b/c %9.3f, angles %7.2f, ` %-10s`).
        let cryst1 = written
            .lines()
            .find(|line| line.starts_with("CRYST1"))
            .expect("writer emits a CRYST1 record");
        assert_eq!(
            cryst1, "CRYST1   40.960   18.650   22.520  90.00  90.77  90.00 P 1 21 1      ",
            "CRYST1 must match the C++ PDBCryst1Handler::write column layout"
        );

        // Round-trip: re-reading the written PDB preserves the cell exactly.
        let reloaded = read_pdb_str(&written).unwrap();
        let cell = reloaded
            .cell_params()
            .expect("cell params survive the write -> reload round-trip");
        assert!((cell.a - 40.960).abs() < 1e-6, "a = {}", cell.a);
        assert!((cell.b - 18.650).abs() < 1e-6, "b = {}", cell.b);
        assert!((cell.c - 22.520).abs() < 1e-6, "c = {}", cell.c);
        assert!((cell.alpha - 90.0).abs() < 1e-6);
        assert!((cell.beta - 90.77).abs() < 1e-6);
        assert!((cell.gamma - 90.0).abs() < 1e-6);
        assert_eq!(cell.space_group_name, "P 1 21 1");

        // A molecule with no cell (e.g. a raw model) emits no CRYST1.
        let plain = read_pdb_str(
            "ATOM      1  N   THR A   1      17.047  14.099   3.625  1.00 13.79           N\n",
        )
        .unwrap();
        assert!(plain.cell_params().is_none());
        assert!(
            !write_pdb_string(&plain).contains("CRYST1"),
            "no cell params -> no CRYST1 (C++ returns false)"
        );
    }

    #[test]
    fn write_pdb_emits_helix_and_sheet_from_secondary2() {
        // C++ PDBFileWriter::writeSecstr reads the per-residue `secondary2`
        // ("Hs"/"H"/"He" runs etc.) and emits one HELIX/SHEET record per segment.
        // The Rust writer dropped them. Oracle: set secondary2 directly (isolating
        // the writer from DSSP) and assert the records' semantic content.
        let mut pdb = String::new();
        for resid in 1..=6 {
            let x = (resid - 1) as f64 * 3.8;
            pdb.push_str(&format!(
                "ATOM  {resid:>5}  CA  ALA A{resid:>4}    {x:>8.3}{:>8.3}{:>8.3}  1.00  0.00\n",
                0.0, 0.0
            ));
        }
        let mut mol = read_pdb_str(&pdb).unwrap();
        // A 3-residue alpha helix (1-3) and a 2-residue strand (5-6). Setting
        // secondary2 directly overwrites whatever DSSP assigned to the CA chain.
        for (resid, sec2) in [(1, "Hs"), (2, "H"), (3, "He"), (5, "Es"), (6, "Ee")] {
            mol.set_residue_prop_str("A", &resid.to_string(), "secondary2", sec2)
                .unwrap();
        }

        let written = write_pdb_string(&mol);
        let helix: Vec<&str> = written
            .lines()
            .filter(|line| line.starts_with("HELIX"))
            .collect();
        let sheet: Vec<&str> = written
            .lines()
            .filter(|line| line.starts_with("SHEET"))
            .collect();
        assert_eq!(helix.len(), 1, "exactly one HELIX record: {helix:?}");
        assert_eq!(sheet.len(), 1, "exactly one SHEET record: {sheet:?}");

        // Semantic content (robust to exact spacing): serial, id, start res/chain/
        // id, end res/chain/id, type. HELIX 1-3 type 1 (alpha); SHEET 5-6 dir 0.
        let htok: Vec<&str> = helix[0].split_whitespace().collect();
        assert_eq!(
            htok,
            vec!["HELIX", "1", "1", "ALA", "A", "1", "ALA", "A", "3", "1"],
            "HELIX tokens"
        );
        let stok: Vec<&str> = sheet[0].split_whitespace().collect();
        assert_eq!(
            stok,
            vec!["SHEET", "1", "1", "1", "ALA", "A", "5", "ALA", "A", "6", "0"],
            "SHEET tokens"
        );
        // Pin the C++ column layout for the HELIX record (catches a format drift).
        assert_eq!(helix[0], "HELIX    1   1 ALA A    1  ALA A    3  1");
    }

    #[test]
    fn write_pdb_emits_secstr_for_a_real_protein() {
        // End-to-end on 1CRN: the default reader runs DSSP -> secondary2, so the
        // writer must surface HELIX/SHEET records (1CRN is helical + has a sheet).
        let root = std::path::PathBuf::from(
            option_env!("CUEMOL2_REF_DIR")
                .unwrap_or(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../cuemol2")),
        )
        .join("tests/test_data");
        let fixture = root.join("1CRN.pdb");
        if !fixture.exists() {
            return; // reference fixtures absent -> self-skip (compat-test convention)
        }
        let mol = read_pdb_file(fixture).unwrap();
        let written = write_pdb_string(&mol);
        let helices = written.lines().filter(|l| l.starts_with("HELIX")).count();
        let sheets = written.lines().filter(|l| l.starts_with("SHEET")).count();
        assert!(helices > 0, "1CRN is helical -> at least one HELIX record");
        assert!(
            sheets > 0,
            "1CRN has a beta sheet -> at least one SHEET record"
        );
    }

    #[test]
    fn parse_cryst1_handles_short_unpadded_record() {
        // A CRYST1 not space-padded to column 80 must still yield its space group.
        let line = "CRYST1   40.960   18.650   22.520  90.00  90.77  90.00 P 1 21 1";
        let cell = parse_cryst1_line(line).expect("short CRYST1 parses");
        assert_eq!(cell.space_group_name, "P 1 21 1");
    }

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

    const LINK_LINE: &str = concat!(
        "LINK  ",
        "      ",
        " O1 ",
        " ",
        "LIG",
        " ",
        "A",
        "   1",
        " ",
        "               ",
        " C3 ",
        " ",
        "LIG",
        " ",
        "A",
        "   2"
    );

    #[test]
    fn parse_link_line_extracts_both_atom_endpoints() {
        let rec = parse_link_line(LINK_LINE).expect("LINK parse");
        assert_eq!(rec.name1, "O1");
        assert_eq!(rec.chain1, "A");
        assert_eq!(rec.res_id1, 1);
        assert_eq!(rec.name2, "C3");
        assert_eq!(rec.chain2, "A");
        assert_eq!(rec.res_id2, 2);
    }

    #[test]
    fn link_record_creates_a_covalent_bond_topology_misses() {
        // Two atoms 5 A apart in different non-polymer LIG residues: the topology
        // dictionary bonds neither (unknown residue, too far, not a polymer
        // link). Only the LINK record bonds them.
        let atoms = "\
ATOM      1  O1  LIG A   1       0.000   0.000   0.000  1.00  0.00           O
ATOM      2  C3  LIG A   2       5.000   0.000   0.000  1.00  0.00           C
";
        assert_eq!(read_pdb_str(atoms).unwrap().bond_count(), 0);

        let with_link = format!("{atoms}{LINK_LINE}\n");
        let mol = read_pdb_str(&with_link).unwrap();
        assert_eq!(
            mol.bond_count(),
            1,
            "LINK record should add exactly one covalent bond"
        );
    }

    #[test]
    fn write_pdb_round_trips_a_deposited_link() {
        // The writer-side counterpart of the LINK reader (C++ writeLinks). A LINK
        // bond is `persist` and must be echoed as a LINK record; a topology bond
        // (not persist) must NOT be. Oracle: read a LINK'd structure, write, and
        // re-read to confirm the inter-residue bond survives (a topology bond
        // would NOT, since the two LIG residues are 5 A apart / untemplated).
        let atoms = "\
ATOM      1  O1  LIG A   1       0.000   0.000   0.000  1.00  0.00           O
ATOM      2  C3  LIG A   2       5.000   0.000   0.000  1.00  0.00           C
";
        let with_link = format!("{atoms}{LINK_LINE}\n");
        let mol = read_pdb_str(&with_link).unwrap();
        assert_eq!(mol.bond_count(), 1, "the LINK bond is read");

        let written = write_pdb_string(&mol);
        let links: Vec<&str> = written.lines().filter(|l| l.starts_with("LINK")).collect();
        assert_eq!(links.len(), 1, "exactly one LINK record written: {links:?}");
        // Standard columns -> the reader's own LINK parser recovers both endpoints.
        let rec = parse_link_line(links[0]).expect("written LINK re-parses");
        assert_eq!((rec.name1.as_str(), rec.res_id1), ("O1", 1));
        assert_eq!((rec.name2.as_str(), rec.res_id2), ("C3", 2));

        // Full round-trip: the bond survives load -> write -> reload.
        let reloaded = read_pdb_str(&written).unwrap();
        assert_eq!(
            reloaded.bond_count(),
            1,
            "the LINK bond survives the round-trip"
        );

        // A structure whose only bonds are from topology emits NO LINK record
        // (those bonds are not `persist`). 1-residue ALA: CA-only -> no bonds, but
        // a real templated residue would bond by topology, never persist.
        let templated = "\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N
ATOM      2  CA  ALA A   1       1.458   0.000   0.000  1.00  0.00           C
ATOM      3  C   ALA A   1       2.009   1.420   0.000  1.00  0.00           C
ATOM      4  O   ALA A   1       1.251   2.390   0.000  1.00  0.00           O
";
        let tmol = read_pdb_str(templated).unwrap();
        assert!(tmol.bond_count() > 0, "topology bonds the ALA backbone");
        assert!(
            !write_pdb_string(&tmol).contains("\nLINK"),
            "topology bonds are not persistent -> no LINK records"
        );
    }

    #[test]
    fn applies_deposited_helix_sheet_records_when_build_secondary_off() {
        // With build_secondary OFF, the reader applies the deposited HELIX/SHEET
        // records verbatim (C++ PDBFileReader::apply2ndry path).
        let pdb = "\
HELIX    1  H1 ALA A    1  ALA A    3  1
SHEET    1   A 1 ALA A   5  ALA A   6  0
ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00           C
ATOM      2  CA  ALA A   3       3.000   0.000   0.000  1.00  0.00           C
ATOM      3  CA  ALA A   5       6.000   0.000   0.000  1.00  0.00           C
ATOM      4  CA  ALA A   6       9.000   0.000   0.000  1.00  0.00           C
";
        let options = PdbReadOptions {
            build_secondary: false,
            ..PdbReadOptions::default()
        };
        let mol = read_pdb_str_with_options(pdb, options).unwrap();
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
    fn ignores_deposited_records_and_computes_dssp_by_default() {
        // With build_secondary ON (the default, matching C++ build2ndry=true),
        // deposited HELIX records are IGNORED in favor of computed DSSP. These
        // CA-only residues have no full backbone, so DSSP assigns nothing — and
        // crucially the reader does NOT fall back to the deposited HELIX record.
        let pdb = "\
HELIX    1  H1 ALA A    1  ALA A    3  1
ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00           C
ATOM      2  CA  ALA A   3       3.000   0.000   0.000  1.00  0.00           C
";
        let mol = read_pdb_str(pdb).unwrap();
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "secondary").unwrap(),
            None
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "3", "secondary").unwrap(),
            None
        );
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
