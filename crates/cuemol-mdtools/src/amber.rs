use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_molstr::{Atom, BondOrder, MolCoord};
use cuemol_qsys::{InOutHandler, ObjReader};
use cuemol_symm::CrystalInfo;
use std::collections::BTreeMap;
use std::path::Path;

const DEFAULT_CHAIN: &str = "A";
const RESTART_WIDTH: usize = 12;
const MDCRD_WIDTH: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct AmberRestart {
    pub positions: Vec<Vector4>,
    pub cell: Option<CrystalInfo>,
    pub has_velocities: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmberMdcrdFrame {
    pub positions: Vec<Vector4>,
    pub cell: Option<CrystalInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmberMdcrdTrajectory {
    pub frames: Vec<AmberMdcrdFrame>,
    pub has_box: bool,
}

impl AmberMdcrdTrajectory {
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn atom_count(&self) -> usize {
        self.frames.first().map_or(0, |frame| frame.positions.len())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmberPrmtopLoad {
    pub molecule: MolCoord,
    pub cell: Option<CrystalInfo>,
}

pub struct AmberCrdReader;

impl AmberCrdReader {
    pub fn read_str(input: &str) -> Result<AmberRestart> {
        read_amber_restart_str(input)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<AmberRestart> {
        read_amber_restart_file(path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmberPrmtopReader {
    reader: ObjReader,
}

impl AmberPrmtopReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new(
                "amberprm",
                "AMBER topology (*.prmtop;*.parm7;*.top)",
                "*.prmtop;*.parm7;*.top",
            ),
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
        read_amber_prmtop_str(input)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<MolCoord> {
        read_amber_prmtop_file(path)
    }
}

impl Default for AmberPrmtopReader {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_amber_restart_file(path: impl AsRef<Path>) -> Result<AmberRestart> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_amber_restart_str(&input)
}

pub fn read_amber_mdcrd_file(
    path: impl AsRef<Path>,
    natom: usize,
    has_box: bool,
) -> Result<AmberMdcrdTrajectory> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_amber_mdcrd_str(&input, natom, has_box)
}

pub fn read_amber_mdcrd_str(
    input: &str,
    natom: usize,
    has_box: bool,
) -> Result<AmberMdcrdTrajectory> {
    if natom == 0 {
        return Err(CueMolError::parse("non-positive AMBER mdcrd NATOM"));
    }
    let mut lines = input.lines();
    lines
        .next()
        .ok_or_else(|| CueMolError::parse("empty AMBER mdcrd file"))?;

    let values = parse_mdcrd_reals(lines)?;
    let frame_value_count = natom
        .checked_mul(3)
        .and_then(|count| count.checked_add(if has_box { 3 } else { 0 }))
        .ok_or_else(|| CueMolError::parse("AMBER mdcrd frame size overflow"))?;
    if frame_value_count == 0 || values.len() % frame_value_count != 0 {
        return Err(CueMolError::parse("AMBER mdcrd frame value count mismatch"));
    }

    let mut frames = Vec::new();
    for frame_values in values.chunks_exact(frame_value_count) {
        let mut positions = Vec::with_capacity(natom);
        for atom_index in 0..natom {
            positions.push(Vector4::new3(
                frame_values[atom_index * 3],
                frame_values[atom_index * 3 + 1],
                frame_values[atom_index * 3 + 2],
            ));
        }
        let cell = if has_box {
            let offset = natom * 3;
            Some(CrystalInfo::new(
                frame_values[offset],
                frame_values[offset + 1],
                frame_values[offset + 2],
                90.0,
                90.0,
                90.0,
            ))
        } else {
            None
        };
        frames.push(AmberMdcrdFrame { positions, cell });
    }
    if frames.is_empty() {
        return Err(CueMolError::parse("AMBER mdcrd contains no frames"));
    }

    Ok(AmberMdcrdTrajectory { frames, has_box })
}

pub fn read_amber_restart_str(input: &str) -> Result<AmberRestart> {
    let mut lines = input.lines();
    lines
        .next()
        .ok_or_else(|| CueMolError::parse("empty AMBER restart file"))?;
    let natom_line = lines
        .next()
        .ok_or_else(|| CueMolError::parse("missing AMBER restart NATOM line"))?;
    let natom = natom_line
        .split_whitespace()
        .next()
        .ok_or_else(|| CueMolError::parse("empty AMBER restart NATOM line"))?
        .parse::<usize>()
        .map_err(|_| CueMolError::parse("invalid AMBER restart NATOM"))?;
    if natom == 0 {
        return Err(CueMolError::parse("non-positive AMBER restart NATOM"));
    }

    let reals = parse_restart_reals(lines)?;
    let coord_count = natom * 3;
    if reals.len() < coord_count {
        return Err(CueMolError::parse(
            "AMBER restart coordinate block is truncated",
        ));
    }
    let positions = (0..natom)
        .map(|index| Vector4::new3(reals[index * 3], reals[index * 3 + 1], reals[index * 3 + 2]))
        .collect::<Vec<_>>();

    let extra = reals.len() - coord_count;
    let has_velocities = extra == coord_count || extra == coord_count + 6;
    let has_box = extra == 6 || extra == coord_count + 6;
    let cell = if has_box {
        let offset = coord_count + if has_velocities { coord_count } else { 0 };
        Some(CrystalInfo::new(
            reals[offset],
            reals[offset + 1],
            reals[offset + 2],
            reals[offset + 3],
            reals[offset + 4],
            reals[offset + 5],
        ))
    } else {
        None
    };

    Ok(AmberRestart {
        positions,
        cell,
        has_velocities,
    })
}

pub fn apply_amber_restart(molecule: &mut MolCoord, restart: &AmberRestart) -> Result<()> {
    if molecule.atom_count() != restart.positions.len() {
        return Err(CueMolError::parse(format!(
            "AMBER restart NATOM ({}) does not match molecule NATOM ({})",
            restart.positions.len(),
            molecule.atom_count()
        )));
    }
    for (atom, position) in molecule.atoms_mut().iter_mut().zip(&restart.positions) {
        atom.position = *position;
    }
    Ok(())
}

pub fn apply_amber_restart_str(molecule: &mut MolCoord, input: &str) -> Result<AmberRestart> {
    let restart = read_amber_restart_str(input)?;
    apply_amber_restart(molecule, &restart)?;
    Ok(restart)
}

pub fn apply_amber_mdcrd_frame(molecule: &mut MolCoord, frame: &AmberMdcrdFrame) -> Result<()> {
    if molecule.atom_count() != frame.positions.len() {
        return Err(CueMolError::parse(format!(
            "AMBER mdcrd NATOM ({}) does not match molecule NATOM ({})",
            frame.positions.len(),
            molecule.atom_count()
        )));
    }
    for (atom, position) in molecule.atoms_mut().iter_mut().zip(&frame.positions) {
        atom.position = *position;
    }
    Ok(())
}

pub fn read_amber_prmtop_file(path: impl AsRef<Path>) -> Result<MolCoord> {
    let input = crate::fileio::read_text_maybe_compressed(path)?;
    read_amber_prmtop_str(&input)
}

pub fn read_amber_prmtop_with_restart_files(
    prmtop_path: impl AsRef<Path>,
    restart_path: impl AsRef<Path>,
) -> Result<AmberPrmtopLoad> {
    let prmtop = crate::fileio::read_text_maybe_compressed(prmtop_path)?;
    let restart = crate::fileio::read_text_maybe_compressed(restart_path)?;
    read_amber_prmtop_with_restart_str(&prmtop, Some(&restart))
}

pub fn read_amber_prmtop_str(input: &str) -> Result<MolCoord> {
    Ok(read_amber_prmtop_with_restart_str(input, None)?.molecule)
}

pub fn read_amber_prmtop_with_restart_str(
    prmtop_input: &str,
    restart_input: Option<&str>,
) -> Result<AmberPrmtopLoad> {
    let sections = parse_prmtop_sections(prmtop_input)?;
    let pointers = parse_required_ints(&sections, "POINTERS", 31)?;
    if pointers.len() < 12 {
        return Err(CueMolError::parse("AMBER POINTERS section too short"));
    }
    let natom = positive_count(pointers[0], "invalid AMBER NATOM")?;
    let nbonh = pointers.get(2).copied().unwrap_or(0).max(0) as usize;
    let mbona = pointers.get(3).copied().unwrap_or(0).max(0) as usize;
    let nres = positive_count(pointers[11], "invalid AMBER NRES")?;

    let atom_names = parse_required_strings(&sections, "ATOM_NAME", natom)?;
    let residue_labels = parse_required_strings(&sections, "RESIDUE_LABEL", nres)?;
    let residue_pointers = parse_required_ints(&sections, "RESIDUE_POINTER", nres)?;
    let atomic_numbers = parse_optional_ints(&sections, "ATOMIC_NUMBER", natom)?;
    let masses = parse_optional_reals(&sections, "MASS", natom)?;
    let amber_types = parse_optional_strings(&sections, "AMBER_ATOM_TYPE", natom)?;

    let atom_to_res = build_atom_residue_map(natom, &residue_pointers)?;
    let mut molecule = MolCoord::new();
    for atom_index in 0..natom {
        let residue_index = atom_to_res[atom_index];
        let atomic_number = atomic_numbers
            .as_ref()
            .and_then(|values| values.get(atom_index))
            .copied()
            .unwrap_or(0);
        let mass = masses
            .as_ref()
            .and_then(|values| values.get(atom_index))
            .copied()
            .unwrap_or(0.0);
        let amber_type = amber_types
            .as_ref()
            .and_then(|values| values.get(atom_index))
            .map(String::as_str)
            .unwrap_or("");
        molecule.push_atom(Atom {
            serial: (atom_index + 1) as i32,
            name: atom_names[atom_index].clone(),
            alt_loc: None,
            res_name: residue_labels[residue_index].clone(),
            chain_id: DEFAULT_CHAIN.to_string(),
            res_id: (residue_index + 1) as i32,
            ins_code: None,
            position: Vector4::new3(0.0, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: resolve_amber_element(atomic_number, mass, amber_type),
            formal_charge: None,
            int_props: Default::default(),
        });
    }

    add_amber_bonds(&sections, "BONDS_INC_HYDROGEN", nbonh, &mut molecule)?;
    add_amber_bonds(&sections, "BONDS_WITHOUT_HYDROGEN", mbona, &mut molecule)?;

    let cell = if let Some(restart_input) = restart_input {
        let restart = apply_amber_restart_str(&mut molecule, restart_input)?;
        restart.cell
    } else {
        None
    };

    Ok(AmberPrmtopLoad { molecule, cell })
}

fn parse_restart_reals<'a>(lines: impl Iterator<Item = &'a str>) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    for line in lines {
        let stripped = line.trim_end_matches(['\r', '\n']);
        let mut pos = 0;
        while pos + RESTART_WIDTH <= stripped.len() {
            let token = fixed_ascii(stripped, pos, pos + RESTART_WIDTH)
                .ok_or_else(|| CueMolError::parse("invalid AMBER restart line"))?
                .trim();
            if token.is_empty() {
                break;
            }
            values.push(parse_fortran_real(token, "invalid AMBER restart real")?);
            pos += RESTART_WIDTH;
        }
        if pos < stripped.len() {
            for token in stripped[pos..].split_whitespace() {
                values.push(parse_fortran_real(
                    token,
                    "invalid AMBER restart tail real",
                )?);
            }
        }
    }
    Ok(values)
}

fn parse_mdcrd_reals<'a>(lines: impl Iterator<Item = &'a str>) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    for line in lines {
        let stripped = line.trim_end_matches(['\r', '\n']);
        let mut pos = 0;
        while pos + MDCRD_WIDTH <= stripped.len() {
            let token = fixed_ascii(stripped, pos, pos + MDCRD_WIDTH)
                .ok_or_else(|| CueMolError::parse("invalid AMBER mdcrd line"))?
                .trim();
            if !token.is_empty() {
                values.push(parse_fortran_real(token, "invalid AMBER mdcrd real")?);
            }
            pos += MDCRD_WIDTH;
        }
        if pos < stripped.len() {
            for token in stripped[pos..].split_whitespace() {
                values.push(parse_fortran_real(token, "invalid AMBER mdcrd tail real")?);
            }
        }
    }
    Ok(values)
}

#[derive(Debug, Clone)]
struct PrmtopSection {
    format: FortFormat,
    lines: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct FortFormat {
    width: usize,
    kind: char,
}

fn parse_prmtop_sections(input: &str) -> Result<BTreeMap<String, PrmtopSection>> {
    let mut lines = input.lines().map(str::trim_end).peekable();
    let first = lines
        .by_ref()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| CueMolError::parse("empty AMBER prmtop file"))?;
    if !first.trim_start().starts_with("%VERSION") && !first.trim_start().starts_with("%FLAG") {
        return Err(CueMolError::parse(
            "Pre-Amber-7 label-less prmtop is not supported",
        ));
    }

    let mut pending_flag = first
        .trim_start()
        .strip_prefix("%FLAG")
        .map(str::trim)
        .map(ToString::to_string);
    let mut sections = BTreeMap::new();
    loop {
        let flag = if let Some(flag) = pending_flag.take() {
            flag
        } else {
            let Some(line) = lines
                .by_ref()
                .find(|line| line.trim_start().starts_with("%FLAG"))
            else {
                break;
            };
            line.trim_start()
                .strip_prefix("%FLAG")
                .unwrap()
                .trim()
                .to_string()
        };

        let format_line = lines
            .by_ref()
            .find(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty() && !trimmed.starts_with("%COMMENT")
            })
            .ok_or_else(|| CueMolError::parse("missing AMBER prmtop FORMAT line"))?;
        if !format_line.trim_start().starts_with("%FORMAT") {
            return Err(CueMolError::parse("expected AMBER prmtop FORMAT line"));
        }
        let format = parse_fort_format(format_line)?;
        let mut data = Vec::new();
        while let Some(line) = lines.peek().copied() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("%FLAG") {
                break;
            }
            if trimmed.starts_with("%VERSION") || trimmed.starts_with("%COMMENT") {
                lines.next();
                continue;
            }
            data.push(line.to_string());
            lines.next();
        }
        sections.insert(
            flag,
            PrmtopSection {
                format,
                lines: data,
            },
        );
    }
    Ok(sections)
}

fn parse_fort_format(line: &str) -> Result<FortFormat> {
    let start = line
        .find('(')
        .ok_or_else(|| CueMolError::parse("malformed AMBER FORMAT"))?;
    let end = line
        .rfind(')')
        .ok_or_else(|| CueMolError::parse("malformed AMBER FORMAT"))?;
    let spec = &line[start + 1..end];
    let mut chars = spec.trim().chars().peekable();
    while matches!(chars.peek(), Some(ch) if ch.is_ascii_digit()) {
        chars.next();
    }
    let kind = chars
        .next()
        .ok_or_else(|| CueMolError::parse("missing AMBER FORMAT kind"))?
        .to_ascii_uppercase();
    let width = chars
        .by_ref()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse::<usize>()
        .map_err(|_| CueMolError::parse("invalid AMBER FORMAT width"))?;
    if width == 0 || !matches!(kind, 'A' | 'I' | 'E' | 'F') {
        return Err(CueMolError::parse("unsupported AMBER FORMAT"));
    }
    Ok(FortFormat { width, kind })
}

fn parse_required_ints(
    sections: &BTreeMap<String, PrmtopSection>,
    flag: &str,
    count: usize,
) -> Result<Vec<i32>> {
    let section = sections
        .get(flag)
        .ok_or_else(|| CueMolError::parse(format!("missing AMBER {flag} section")))?;
    parse_ints(section, count)
}

fn parse_optional_ints(
    sections: &BTreeMap<String, PrmtopSection>,
    flag: &str,
    count: usize,
) -> Result<Option<Vec<i32>>> {
    sections
        .get(flag)
        .map(|section| parse_ints(section, count))
        .transpose()
}

fn parse_optional_reals(
    sections: &BTreeMap<String, PrmtopSection>,
    flag: &str,
    count: usize,
) -> Result<Option<Vec<f64>>> {
    sections
        .get(flag)
        .map(|section| parse_reals(section, count))
        .transpose()
}

fn parse_required_strings(
    sections: &BTreeMap<String, PrmtopSection>,
    flag: &str,
    count: usize,
) -> Result<Vec<String>> {
    let section = sections
        .get(flag)
        .ok_or_else(|| CueMolError::parse(format!("missing AMBER {flag} section")))?;
    parse_strings(section, count)
}

fn parse_optional_strings(
    sections: &BTreeMap<String, PrmtopSection>,
    flag: &str,
    count: usize,
) -> Result<Option<Vec<String>>> {
    sections
        .get(flag)
        .map(|section| parse_strings(section, count))
        .transpose()
}

fn parse_ints(section: &PrmtopSection, count: usize) -> Result<Vec<i32>> {
    if section.format.kind != 'I' {
        return Err(CueMolError::parse("AMBER section is not integer formatted"));
    }
    parse_fixed_tokens(section, count)?
        .into_iter()
        .map(|token| {
            token
                .parse::<i32>()
                .map_err(|_| CueMolError::parse("invalid AMBER integer"))
        })
        .collect()
}

fn parse_reals(section: &PrmtopSection, count: usize) -> Result<Vec<f64>> {
    if !matches!(section.format.kind, 'E' | 'F') {
        return Err(CueMolError::parse("AMBER section is not real formatted"));
    }
    parse_fixed_tokens(section, count)?
        .into_iter()
        .map(|token| parse_fortran_real(&token, "invalid AMBER real"))
        .collect()
}

fn parse_strings(section: &PrmtopSection, count: usize) -> Result<Vec<String>> {
    if section.format.kind != 'A' {
        return Err(CueMolError::parse("AMBER section is not string formatted"));
    }
    parse_fixed_tokens(section, count)
}

fn parse_fixed_tokens(section: &PrmtopSection, count: usize) -> Result<Vec<String>> {
    let mut tokens = Vec::with_capacity(count);
    for line in &section.lines {
        let mut pos = 0;
        while pos < line.len() && tokens.len() < count {
            let end = (pos + section.format.width).min(line.len());
            let token = fixed_ascii(line, pos, end)
                .ok_or_else(|| CueMolError::parse("invalid AMBER fixed-width field"))?
                .trim()
                .to_string();
            if !token.is_empty() || section.format.kind == 'A' {
                tokens.push(token);
            }
            pos += section.format.width;
        }
        if tokens.len() == count {
            break;
        }
    }
    if tokens.len() != count {
        return Err(CueMolError::parse("AMBER section record count mismatch"));
    }
    Ok(tokens)
}

fn positive_count(value: i32, message: &'static str) -> Result<usize> {
    if value <= 0 {
        Err(CueMolError::parse(message))
    } else {
        Ok(value as usize)
    }
}

fn build_atom_residue_map(natom: usize, residue_pointers: &[i32]) -> Result<Vec<usize>> {
    let mut atom_to_res = vec![0_usize; natom];
    for residue_index in 0..residue_pointers.len() {
        let begin = residue_pointers[residue_index] - 1;
        let end = residue_pointers
            .get(residue_index + 1)
            .map(|value| value - 1)
            .unwrap_or(natom as i32);
        if begin < 0 || end <= begin || end as usize > natom {
            return Err(CueMolError::parse("inconsistent AMBER RESIDUE_POINTER"));
        }
        for value in atom_to_res
            .iter_mut()
            .take(end as usize)
            .skip(begin as usize)
        {
            *value = residue_index;
        }
    }
    Ok(atom_to_res)
}

fn add_amber_bonds(
    sections: &BTreeMap<String, PrmtopSection>,
    flag: &str,
    count: usize,
    molecule: &mut MolCoord,
) -> Result<()> {
    if count == 0 {
        return Ok(());
    }
    let ints = parse_required_ints(sections, flag, count * 3)?;
    for bond_index in 0..count {
        let atom1 = ints[bond_index * 3] / 3;
        let atom2 = ints[bond_index * 3 + 1] / 3;
        if atom1 >= 0 && atom2 >= 0 {
            molecule.add_bond(atom1 as usize, atom2 as usize, BondOrder::Single);
        }
    }
    Ok(())
}

fn resolve_amber_element(atomic_number: i32, mass: f64, amber_type: &str) -> String {
    if let Some(symbol) = atomic_number_symbol(atomic_number) {
        return symbol.to_string();
    }
    if let Some(symbol) = mass_symbol(mass) {
        return symbol.to_string();
    }
    amber_type
        .chars()
        .find(|ch| ch.is_ascii_alphabetic())
        .map(|ch| ch.to_ascii_uppercase().to_string())
        .unwrap_or_default()
}

fn atomic_number_symbol(value: i32) -> Option<&'static str> {
    usize::try_from(value)
        .ok()
        .and_then(|number| ELEMENT_SYMBOLS.get(number).copied())
        .filter(|symbol| !symbol.is_empty())
}

const ELEMENT_SYMBOLS: [&str; 119] = [
    "", "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S",
    "Cl", "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge",
    "As", "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd",
    "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd",
    "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg",
    "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm",
    "Bk", "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn",
    "Nh", "Fl", "Mc", "Lv", "Ts", "Og",
];

fn mass_symbol(mass: f64) -> Option<&'static str> {
    let near = |target: f64| (target - 0.5..target + 0.5).contains(&mass);
    Some(match () {
        _ if near(1.008) => "H",
        _ if near(12.011) => "C",
        _ if near(14.007) => "N",
        _ if near(15.999) => "O",
        _ if near(18.998) => "F",
        _ if near(22.990) => "Na",
        _ if near(24.305) => "Mg",
        _ if near(30.974) => "P",
        _ if near(32.06) => "S",
        _ if near(35.45) => "Cl",
        _ if near(39.098) => "K",
        _ if near(40.078) => "Ca",
        _ if near(55.845) => "Fe",
        _ if near(65.38) => "Zn",
        _ if near(79.904) => "Br",
        _ if near(126.904) => "I",
        _ => return None,
    })
}

fn parse_fortran_real(token: &str, message: &'static str) -> Result<f64> {
    let normalized = token.replace(['D', 'd'], "E");
    normalized
        .parse::<f64>()
        .map_err(|_| CueMolError::parse(message))
}

fn fixed_ascii(input: &str, start: usize, end: usize) -> Option<&str> {
    std::str::from_utf8(input.as_bytes().get(start..end)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_number_symbols_cover_the_periodic_table() {
        assert_eq!(atomic_number_symbol(1), Some("H"));
        assert_eq!(atomic_number_symbol(14), Some("Si"));
        assert_eq!(atomic_number_symbol(79), Some("Au"));
        assert_eq!(atomic_number_symbol(118), Some("Og"));
        assert_eq!(atomic_number_symbol(0), None);
        assert_eq!(atomic_number_symbol(119), None);
    }
}
