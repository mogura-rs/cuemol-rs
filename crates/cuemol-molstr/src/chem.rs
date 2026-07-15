use cuemol_core::{CueMolError, Matrix4, Result, Vector4};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const INVALID_UID: u64 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElemSym(u8);

#[allow(non_upper_case_globals)]
impl ElemSym {
    pub const H: Self = Self(1);
    pub const He: Self = Self(2);
    pub const Li: Self = Self(3);
    pub const Be: Self = Self(4);
    pub const B: Self = Self(5);
    pub const C: Self = Self(6);
    pub const N: Self = Self(7);
    pub const O: Self = Self(8);
    pub const F: Self = Self(9);
    pub const Ne: Self = Self(10);
    pub const Na: Self = Self(11);
    pub const Mg: Self = Self(12);
    pub const Al: Self = Self(13);
    pub const Si: Self = Self(14);
    pub const P: Self = Self(15);
    pub const S: Self = Self(16);
    pub const Cl: Self = Self(17);
    pub const Ar: Self = Self(18);
    pub const K: Self = Self(19);
    pub const Ca: Self = Self(20);
    pub const Sc: Self = Self(21);
    pub const Ti: Self = Self(22);
    pub const V: Self = Self(23);
    pub const Cr: Self = Self(24);
    pub const Mn: Self = Self(25);
    pub const Fe: Self = Self(26);
    pub const Co: Self = Self(27);
    pub const Ni: Self = Self(28);
    pub const Cu: Self = Self(29);
    pub const Zn: Self = Self(30);
    pub const Ga: Self = Self(31);
    pub const Ge: Self = Self(32);
    pub const As: Self = Self(33);
    pub const Se: Self = Self(34);
    pub const Br: Self = Self(35);
    pub const Kr: Self = Self(36);
    pub const Rb: Self = Self(37);
    pub const Sr: Self = Self(38);
    pub const Y: Self = Self(39);
    pub const Zr: Self = Self(40);
    pub const Nb: Self = Self(41);
    pub const Mo: Self = Self(42);
    pub const Tc: Self = Self(43);
    pub const Ru: Self = Self(44);
    pub const Rh: Self = Self(45);
    pub const Pd: Self = Self(46);
    pub const Ag: Self = Self(47);
    pub const Cd: Self = Self(48);
    pub const In: Self = Self(49);
    pub const Sn: Self = Self(50);
    pub const Sb: Self = Self(51);
    pub const Te: Self = Self(52);
    pub const I: Self = Self(53);
    pub const Xe: Self = Self(54);
    pub const Cs: Self = Self(55);
    pub const Ba: Self = Self(56);
    pub const La: Self = Self(57);
    pub const Ce: Self = Self(58);
    pub const Pr: Self = Self(59);
    pub const Nd: Self = Self(60);
    pub const Pm: Self = Self(61);
    pub const Sm: Self = Self(62);
    pub const Eu: Self = Self(63);
    pub const Gd: Self = Self(64);
    pub const Tb: Self = Self(65);
    pub const Dy: Self = Self(66);
    pub const Ho: Self = Self(67);
    pub const Er: Self = Self(68);
    pub const Tm: Self = Self(69);
    pub const Yb: Self = Self(70);
    pub const Lu: Self = Self(71);
    pub const Hf: Self = Self(72);
    pub const Ta: Self = Self(73);
    pub const W: Self = Self(74);
    pub const Re: Self = Self(75);
    pub const Os: Self = Self(76);
    pub const Ir: Self = Self(77);
    pub const Pt: Self = Self(78);
    pub const Au: Self = Self(79);
    pub const Hg: Self = Self(80);
    pub const Tl: Self = Self(81);
    pub const Pb: Self = Self(82);
    pub const Bi: Self = Self(83);
    pub const Po: Self = Self(84);
    pub const At: Self = Self(85);
    pub const Rn: Self = Self(86);
    pub const Fr: Self = Self(87);
    pub const Ra: Self = Self(88);
    pub const Ac: Self = Self(89);
    pub const Th: Self = Self(90);
    pub const Pa: Self = Self(91);
    pub const U: Self = Self(92);
    pub const Np: Self = Self(93);
    pub const Pu: Self = Self(94);
    pub const XX: Self = Self(255);

    pub fn from_name(name: &str) -> Self {
        let name = name.trim();
        ELEMENT_SYMBOLS
            .iter()
            .position(|symbol| !symbol.is_empty() && symbol.eq_ignore_ascii_case(name))
            .map_or(Self::XX, |number| Self(number as u8))
    }

    pub fn name(self) -> &'static str {
        ELEMENT_SYMBOLS
            .get(usize::from(self.0))
            .copied()
            .filter(|symbol| !symbol.is_empty())
            .unwrap_or("Unknown")
    }

    pub fn atomic_number(self) -> Option<u8> {
        (1..=94).contains(&self.0).then_some(self.0)
    }
}

const ELEMENT_SYMBOLS: [&str; 95] = [
    "", "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S",
    "Cl", "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge",
    "As", "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd",
    "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd",
    "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg",
    "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ResidIndex {
    pub number: i32,
    pub ins_code: Option<char>,
}

impl ResidIndex {
    pub fn new(number: i32) -> Self {
        Self {
            number,
            ins_code: None,
        }
    }

    pub fn with_ins_code(number: i32, ins_code: char) -> Self {
        Self {
            number,
            ins_code: Some(ins_code),
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            return Err(CueMolError::invalid_argument("empty residue index"));
        }
        let mut digits = String::new();
        let mut suffix = None;
        for ch in value.chars() {
            if ch.is_ascii_digit() || (digits.is_empty() && (ch == '-' || ch == '+')) {
                digits.push(ch);
            } else {
                suffix = Some(ch);
                break;
            }
        }
        let number = digits
            .parse::<i32>()
            .map_err(|_| CueMolError::invalid_argument("invalid residue index"))?;
        Ok(Self {
            number,
            ins_code: suffix,
        })
    }

    pub fn script_string(self) -> String {
        match self.ins_code {
            Some(code) => format!("{}{}", self.number, code),
            None => self.number.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MolBond {
    atom1: i32,
    atom2: i32,
    persist: bool,
    bond_type: i32,
}

impl Default for MolBond {
    fn default() -> Self {
        Self {
            atom1: -1,
            atom2: -1,
            persist: false,
            bond_type: Self::SINGLE,
        }
    }
}

impl MolBond {
    pub const SINGLE: i32 = 10;
    pub const DELOC: i32 = 15;
    pub const DOUBLE: i32 = 20;
    pub const TRIPLE: i32 = 30;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn atom1(&self) -> i32 {
        self.atom1
    }

    pub fn set_atom1(&mut self, value: i32) {
        self.atom1 = value;
    }

    pub fn atom2(&self) -> i32 {
        self.atom2
    }

    pub fn set_atom2(&mut self, value: i32) {
        self.atom2 = value;
    }

    pub fn is_persist(&self) -> bool {
        self.persist
    }

    pub fn set_persist(&mut self, value: bool) {
        self.persist = value;
    }

    pub fn bond_type(&self) -> i32 {
        self.bond_type
    }

    pub fn set_bond_type(&mut self, value: i32) {
        self.bond_type = value;
    }

    fn other_atom(&self, atom_id: i32) -> Option<i32> {
        if self.atom1 == atom_id {
            Some(self.atom2)
        } else if self.atom2 == atom_id {
            Some(self.atom1)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AtomProp {
    Int(i32),
    Real(f64),
    Str(String),
}

#[derive(Debug, PartialEq)]
pub struct MolAtom {
    parent_uid: u64,
    id: i32,
    name: String,
    chain_name: String,
    res_name: String,
    res_index: ResidIndex,
    bfac: f64,
    occ: f64,
    conf_id: Option<char>,
    cname: String,
    element: ElemSym,
    raw_pos: Vector4,
    xform: Option<Matrix4>,
    bonds: Vec<MolBond>,
    anisou: Option<[[f64; 3]; 3]>,
    props: BTreeMap<String, AtomProp>,
}

impl Clone for MolAtom {
    fn clone(&self) -> Self {
        Self {
            parent_uid: INVALID_UID,
            id: self.id,
            name: self.name.clone(),
            chain_name: self.chain_name.clone(),
            res_name: self.res_name.clone(),
            res_index: self.res_index,
            bfac: self.bfac,
            occ: self.occ,
            conf_id: self.conf_id,
            cname: self.cname.clone(),
            element: self.element,
            raw_pos: self.raw_pos,
            xform: self.xform,
            bonds: self.bonds.clone(),
            anisou: self.anisou,
            props: self.props.clone(),
        }
    }
}

impl Default for MolAtom {
    fn default() -> Self {
        Self {
            parent_uid: INVALID_UID,
            id: -1,
            name: String::new(),
            chain_name: String::new(),
            res_name: String::new(),
            res_index: ResidIndex::default(),
            bfac: 1.0,
            occ: 1.0,
            conf_id: None,
            cname: String::new(),
            element: ElemSym::XX,
            raw_pos: Vector4::ZERO,
            xform: None,
            bonds: Vec::new(),
            anisou: None,
            props: BTreeMap::new(),
        }
    }
}

impl MolAtom {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parent_uid(&self) -> u64 {
        self.parent_uid
    }

    pub fn set_parent_uid(&mut self, value: u64) {
        self.parent_uid = value;
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn get_id(&self) -> i32 {
        self.id()
    }

    pub fn set_id(&mut self, value: i32) {
        self.id = value;
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    pub fn chain_name(&self) -> &str {
        &self.chain_name
    }

    pub fn get_chain_name(&self) -> &str {
        self.chain_name()
    }

    pub fn set_chain_name(&mut self, value: impl Into<String>) {
        self.chain_name = value.into();
    }

    pub fn res_name(&self) -> &str {
        &self.res_name
    }

    pub fn get_res_name(&self) -> &str {
        self.res_name()
    }

    pub fn resid_name(&self) -> &str {
        self.res_name()
    }

    pub fn set_res_name(&mut self, value: impl Into<String>) {
        self.res_name = value.into();
    }

    pub fn set_resid_name(&mut self, value: impl Into<String>) {
        self.set_res_name(value);
    }

    pub fn res_index(&self) -> ResidIndex {
        self.res_index
    }

    pub fn resid_index(&self) -> String {
        self.res_index.script_string()
    }

    pub fn get_res_index_scr(&self) -> String {
        self.resid_index()
    }

    pub fn set_res_index(&mut self, value: ResidIndex) {
        self.res_index = value;
    }

    pub fn bfac(&self) -> f64 {
        self.bfac
    }

    pub fn get_bfac(&self) -> f64 {
        self.bfac()
    }

    pub fn set_bfac(&mut self, value: f64) {
        self.bfac = value;
    }

    pub fn occ(&self) -> f64 {
        self.occ
    }

    pub fn get_occ(&self) -> f64 {
        self.occ()
    }

    pub fn set_occ(&mut self, value: f64) {
        self.occ = value;
    }

    pub fn conf_id(&self) -> Option<char> {
        self.conf_id
    }

    pub fn confid(&self) -> String {
        self.conf_id
            .map(|value| value.to_string())
            .unwrap_or_default()
    }

    pub fn get_conf_id_scr(&self) -> String {
        self.confid()
    }

    pub fn set_conf_id(&mut self, value: Option<char>) {
        self.conf_id = value;
    }

    pub fn set_confid(&mut self, value: impl AsRef<str>) {
        self.conf_id = value.as_ref().chars().next();
    }

    pub fn set_conf_id_scr(&mut self, value: impl AsRef<str>) {
        self.set_confid(value);
    }

    pub fn get_u(&self, i: usize, j: usize) -> f64 {
        self.u(i, j).unwrap_or(0.0)
    }

    pub fn cname(&self) -> &str {
        &self.cname
    }

    pub fn set_cname(&mut self, value: impl Into<String>) {
        self.cname = value.into();
    }

    pub fn element(&self) -> ElemSym {
        self.element
    }

    pub fn set_element(&mut self, value: ElemSym) {
        self.element = value;
    }

    pub fn set_element_name(&mut self, value: &str) {
        self.element = ElemSym::from_name(value);
    }

    pub fn element_name(&self) -> &'static str {
        self.element.name()
    }

    pub fn get_element_name(&self) -> &'static str {
        self.element_name()
    }

    pub fn set_pos(&mut self, value: Vector4) -> Result<()> {
        if self.xform.is_some() {
            return Err(CueMolError::invalid_argument(
                "cannot set transformed atom position",
            ));
        }
        self.raw_pos = value;
        Ok(())
    }

    pub fn pos(&self) -> Vector4 {
        let mut pos = self.raw_pos;
        if let Some(xform) = self.xform {
            xform.xform3d(&mut pos);
        }
        pos
    }

    pub fn get_pos(&self) -> Vector4 {
        self.pos()
    }

    pub fn get_pos_scr(&self) -> Vector4 {
        self.pos()
    }

    pub fn set_pos_scr(&mut self, value: Vector4) -> Result<()> {
        self.set_pos(value)
    }

    pub fn set_raw_pos(&mut self, value: Vector4) {
        self.raw_pos = value;
    }

    pub fn raw_pos(&self) -> Vector4 {
        self.raw_pos
    }

    pub fn set_xform_matrix(&mut self, value: Matrix4) {
        self.xform = Some(value);
    }

    pub fn reset_xform_matrix(&mut self) {
        self.xform = None;
    }

    pub fn add_bond(&mut self, bond: &MolBond) -> bool {
        let Some(other) = bond.other_atom(self.id) else {
            return false;
        };
        if self.is_bonded(other) {
            return false;
        }
        self.bonds.push(bond.clone());
        true
    }

    pub fn remove_bond(&mut self, bond: &MolBond) -> bool {
        let before = self.bonds.len();
        self.bonds.retain(|existing| existing != bond);
        self.bonds.len() != before
    }

    pub fn bond_count(&self) -> usize {
        self.bonds.len()
    }

    pub fn is_bonded(&self, atom_id: i32) -> bool {
        self.bonds
            .iter()
            .any(|bond| bond.other_atom(self.id) == Some(atom_id))
    }

    pub fn bond(&self, atom_id: i32) -> Option<&MolBond> {
        self.bonds
            .iter()
            .find(|bond| bond.other_atom(self.id) == Some(atom_id))
    }

    pub fn has_anisou(&self) -> bool {
        self.anisou.is_some()
    }

    pub fn set_u(&mut self, row: usize, col: usize, value: f64) -> Result<()> {
        if row >= 3 || col >= 3 {
            return Err(CueMolError::invalid_argument("anisou index out of range"));
        }
        let matrix = self.anisou.get_or_insert([[0.0; 3]; 3]);
        matrix[row][col] = value;
        matrix[col][row] = value;
        Ok(())
    }

    pub fn u(&self, row: usize, col: usize) -> Result<f64> {
        if row >= 3 || col >= 3 {
            return Err(CueMolError::invalid_argument("anisou index out of range"));
        }
        Ok(self.anisou.unwrap_or([[0.0; 3]; 3])[row][col])
    }

    pub fn format_msg(&self) -> String {
        let mut message = format!(
            "{} {} {} {}",
            self.chain_name,
            self.res_name,
            self.res_index.script_string(),
            self.name
        );
        if let Some(conf_id) = self.conf_id {
            message.push(':');
            message.push(conf_id);
        }
        message
    }

    pub fn set_atom_prop_int(&mut self, name: impl Into<String>, value: i32) {
        self.props.insert(name.into(), AtomProp::Int(value));
    }

    pub fn set_atom_prop_real(&mut self, name: impl Into<String>, value: f64) {
        self.props.insert(name.into(), AtomProp::Real(value));
    }

    pub fn set_atom_prop_str(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.props.insert(name.into(), AtomProp::Str(value.into()));
    }

    pub fn has_atom_prop(&self, name: &str) -> bool {
        self.props.contains_key(name)
    }

    pub fn atom_prop_int(&self, name: &str) -> Option<i32> {
        match self.props.get(name) {
            Some(AtomProp::Int(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn get_atom_prop_int(&self, name: &str) -> Option<i32> {
        self.atom_prop_int(name)
    }

    pub fn atom_prop_real(&self, name: &str) -> Option<f64> {
        match self.props.get(name) {
            Some(AtomProp::Real(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn get_atom_prop_real(&self, name: &str) -> Option<f64> {
        self.atom_prop_real(name)
    }

    pub fn atom_prop_str(&self, name: &str) -> Option<&str> {
        match self.props.get(name) {
            Some(AtomProp::Str(value)) => Some(value),
            _ => None,
        }
    }

    pub fn get_prop_type_name(&self, name: &str) -> String {
        match self.props.get(name) {
            Some(AtomProp::Int(_)) => "integer",
            Some(AtomProp::Real(_)) => "real",
            Some(AtomProp::Str(_)) => "string",
            None => "",
        }
        .to_string()
    }

    pub fn remove_atom_prop(&mut self, name: &str) -> bool {
        self.props.remove(name).is_some()
    }

    pub fn dihedral(&self, atom2: &Self, atom3: &Self, atom4: &Self) -> Result<f64> {
        Vector4::dihedral(self.pos(), atom2.pos(), atom3.pos(), atom4.pos())
    }
}

impl fmt::Display for MolAtom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_msg())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MolResidue {
    parent_uid: u64,
    name: String,
    res_type: String,
    chain_name: String,
    index: ResidIndex,
    scr_index: i32,
    str_index: String,
    atoms: Vec<MolAtom>,
    props: BTreeMap<String, String>,
    pivot_atom_name: String,
    link_next_index: Option<ResidIndex>,
}

impl MolResidue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parent_uid(&self) -> u64 {
        self.parent_uid
    }

    pub fn set_parent_uid(&mut self, value: u64) {
        self.parent_uid = value;
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    pub fn res_type(&self) -> &str {
        &self.res_type
    }

    pub fn get_type(&self) -> &str {
        self.res_type()
    }

    pub fn set_type(&mut self, value: impl Into<String>) {
        self.set_res_type(value);
    }

    pub fn set_res_type(&mut self, value: impl Into<String>) {
        self.res_type = value.into();
    }

    pub fn chain_name(&self) -> &str {
        &self.chain_name
    }

    pub fn get_chain_name(&self) -> &str {
        self.chain_name()
    }

    pub fn set_chain_name(&mut self, value: impl Into<String>) {
        self.chain_name = value.into();
    }

    pub fn index(&self) -> ResidIndex {
        self.index
    }

    pub fn set_index(&mut self, value: ResidIndex) {
        self.index = value;
        self.scr_index = value.number;
        self.str_index = value.script_string();
    }

    pub fn atom_size(&self) -> usize {
        self.atoms.len()
    }

    pub fn append_atom(&mut self, mut atom: MolAtom) -> bool {
        if self
            .atoms
            .iter()
            .any(|existing| existing.name == atom.name && existing.conf_id == atom.conf_id)
        {
            return false;
        }
        atom.set_parent_uid(self.parent_uid);
        atom.set_chain_name(self.chain_name.clone());
        atom.set_res_index(self.index);
        self.atoms.push(atom);
        true
    }

    pub fn remove_atom(&mut self, name: &str, conf_id: Option<char>) -> bool {
        let before = self.atoms.len();
        self.atoms
            .retain(|atom| !(atom.name == name && atom.conf_id == conf_id));
        self.atoms.len() != before
    }

    pub fn atom_id(&self, name: &str, conf_id: Option<char>) -> Option<i32> {
        self.atoms
            .iter()
            .find(|atom| atom.name == name && atom.conf_id == conf_id)
            .map(MolAtom::id)
    }

    pub fn alt_confs(&self, name: &str) -> BTreeSet<char> {
        self.atoms
            .iter()
            .filter(|atom| atom.name == name)
            .filter_map(|atom| atom.conf_id)
            .collect()
    }

    pub fn atoms(&self) -> &[MolAtom] {
        &self.atoms
    }

    pub fn set_scr_index(&mut self, value: i32) {
        self.index = ResidIndex::new(value);
        self.scr_index = value;
        self.str_index = self.index.script_string();
    }

    pub fn scr_index(&self) -> i32 {
        self.index.number
    }

    pub fn set_str_index(&mut self, value: impl Into<String>) {
        let value = value.into();
        if let Ok(index) = ResidIndex::parse(&value) {
            self.index = index;
            self.scr_index = index.number;
            self.str_index = index.script_string();
        }
    }

    pub fn str_index(&self) -> String {
        self.index.script_string()
    }

    pub fn get_str_index(&self) -> String {
        self.str_index()
    }

    pub fn get_scr_index(&self) -> i32 {
        self.scr_index()
    }

    pub fn get_atoms_json(&self) -> String {
        let entries = self
            .atoms
            .iter()
            .map(|atom| {
                format!(
                    "{{\"name\":\"{}\",\"id\":{},\"elem\":\"{}\"}}",
                    json_escape(atom.name()),
                    atom.id(),
                    json_escape(atom.element_name())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub fn get_atom(&self, name: &str) -> Option<&MolAtom> {
        self.atoms
            .iter()
            .find(|atom| atom.name() == name && atom.conf_id().is_none())
            .or_else(|| self.atoms.iter().find(|atom| atom.name() == name))
    }

    pub fn get_atom_alt_conf(&self, name: &str, conf_id: &str) -> Option<&MolAtom> {
        let conf_id = conf_id.chars().next();
        self.atoms
            .iter()
            .find(|atom| atom.name() == name && atom.conf_id() == conf_id)
    }

    pub fn set_prop_str(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.props.insert(key.into(), value.into());
    }

    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.props.get(key).map(String::as_str)
    }

    pub fn remove_prop_str(&mut self, key: &str) -> bool {
        self.props.remove(key).is_some()
    }

    pub fn res_prop_names(&self) -> BTreeSet<String> {
        self.props.keys().cloned().collect()
    }

    pub fn pivot_atom_name(&self) -> &str {
        &self.pivot_atom_name
    }

    pub fn get_pivot_pos(&self) -> Vector4 {
        if let Some(position) = self
            .atoms
            .iter()
            .find(|atom| atom.name() == self.pivot_atom_name())
            .map(MolAtom::pos)
        {
            return position;
        }
        if self.atoms.is_empty() {
            return Vector4::ZERO;
        }
        self.atoms
            .iter()
            .fold(Vector4::ZERO, |sum, atom| sum + atom.pos())
            / self.atoms.len() as f64
    }

    pub fn set_pivot_atom_name(&mut self, value: impl Into<String>) {
        self.pivot_atom_name = value.into();
    }

    pub fn set_link_next_index(&mut self, value: Option<ResidIndex>) {
        self.link_next_index = value;
    }

    pub fn is_linked_to_index(&self, value: ResidIndex) -> bool {
        self.link_next_index == Some(value)
    }
}

impl fmt::Display for MolResidue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.chain_name,
            self.index.script_string(),
            self.name
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MolChain {
    parent_uid: u64,
    name: String,
    residues: Vec<MolResidue>,
}

impl MolChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    pub fn parent_uid(&self) -> u64 {
        self.parent_uid
    }

    pub fn set_parent_uid(&mut self, value: u64) {
        self.parent_uid = value;
    }

    pub fn size(&self) -> usize {
        self.residues.len()
    }

    pub fn append_residue(&mut self, mut residue: MolResidue) -> bool {
        if self
            .residues
            .iter()
            .any(|existing| existing.index == residue.index)
        {
            return false;
        }
        residue.set_chain_name(self.name.clone());
        residue.set_parent_uid(self.parent_uid);
        self.residues.push(residue);
        true
    }

    pub fn remove_residue(&mut self, index: ResidIndex) -> bool {
        let before = self.residues.len();
        self.residues.retain(|residue| residue.index != index);
        self.residues.len() != before
    }

    pub fn residue(&self, index: ResidIndex) -> Option<&MolResidue> {
        self.residues.iter().find(|residue| residue.index == index)
    }

    pub fn residue_by_str(&self, index: &str) -> Result<Option<&MolResidue>> {
        let index = ResidIndex::parse(index)?;
        Ok(self.residue(index))
    }

    pub fn get_residue(&self, index: &str) -> Result<Option<&MolResidue>> {
        self.residue_by_str(index)
    }

    pub fn get_resids_json(&self) -> String {
        let entries = self
            .residues_in_index_order()
            .into_iter()
            .map(|residue| {
                format!(
                    "{{\"name\":\"{}\",\"single\":\"{}\",\"sel\":false,\"index\":\"{}\"}}",
                    json_escape(residue.name()),
                    json_escape(residue.prop_str("single").unwrap_or("")),
                    json_escape(&residue.index().script_string())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub fn residues(&self) -> &[MolResidue] {
        &self.residues
    }

    pub fn residues_in_index_order(&self) -> Vec<&MolResidue> {
        let mut residues = self.residues.iter().collect::<Vec<_>>();
        residues.sort_by_key(|residue| residue.index);
        residues
    }
}

impl fmt::Display for MolChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn molbond_defaults_and_accessors_match_reference() {
        assert_eq!(ElemSym::from_name("ca"), ElemSym::Ca);
        assert_eq!(ElemSym::Ca.name(), "Ca");
        assert_eq!(ElemSym::Pu.atomic_number(), Some(94));
        assert_eq!(ElemSym::XX.name(), "Unknown");
        let mut bond = MolBond::new();
        assert_eq!(bond.atom1(), -1);
        assert_eq!(bond.atom2(), -1);
        assert!(!bond.is_persist());
        assert_eq!(bond.bond_type(), MolBond::SINGLE);

        bond.set_atom1(5);
        bond.set_atom2(7);
        bond.set_persist(true);
        bond.set_bond_type(MolBond::DOUBLE);
        assert_eq!(bond.atom1(), 5);
        assert_eq!(bond.atom2(), 7);
        assert!(bond.is_persist());
        assert_eq!(bond.bond_type(), MolBond::DOUBLE);
    }

    #[test]
    fn molatom_properties_transform_bonds_and_props_match_reference() {
        let mut atom = MolAtom::new();
        assert_eq!(atom.parent_uid(), INVALID_UID);
        assert_eq!(atom.id(), -1);
        assert_eq!(atom.element(), ElemSym::XX);
        assert_eq!(atom.bfac(), 1.0);
        assert_eq!(atom.occ(), 1.0);
        assert_eq!(atom.conf_id(), None);
        assert_eq!(atom.bond_count(), 0);
        assert!(!atom.has_anisou());

        atom.set_id(0);
        atom.set_name("CA");
        atom.set_chain_name("A");
        atom.set_res_name("ALA");
        atom.set_res_index(ResidIndex::new(10));
        atom.set_bfac(25.5);
        atom.set_occ(0.5);
        atom.set_conf_id(Some('B'));
        atom.set_cname("CA");
        atom.set_element_name("N");
        atom.set_pos(Vector4::new3(1.0, 2.0, 3.0)).unwrap();

        assert_eq!(atom.name(), "CA");
        assert_eq!(atom.chain_name(), "A");
        assert_eq!(atom.res_name(), "ALA");
        assert_eq!(atom.res_index(), ResidIndex::new(10));
        assert_eq!(atom.element(), ElemSym::N);
        assert_eq!(atom.element_name(), "N");

        let mut xform = Matrix4::IDENTITY;
        xform.set_at(1, 4, 10.0).unwrap();
        xform.set_at(2, 4, 20.0).unwrap();
        xform.set_at(3, 4, 30.0).unwrap();
        atom.set_xform_matrix(xform);
        assert_eq!(atom.pos(), Vector4::new3(11.0, 22.0, 33.0));
        assert!(atom.set_pos(Vector4::ZERO).is_err());
        atom.reset_xform_matrix();
        assert_eq!(atom.pos(), Vector4::new3(1.0, 2.0, 3.0));

        let mut bond = MolBond::new();
        bond.set_atom1(0);
        bond.set_atom2(5);
        assert!(atom.add_bond(&bond));
        assert!(!atom.add_bond(&bond));
        assert!(atom.is_bonded(5));
        assert!(atom.bond(5).is_some());
        assert!(atom.remove_bond(&bond));

        atom.set_u(0, 1, 0.05).unwrap();
        assert!(atom.has_anisou());
        assert_eq!(atom.u(1, 0).unwrap(), 0.05);
        assert_eq!(atom.get_u(0, 1), 0.05);

        atom.set_atom_prop_int("myInt", 42);
        atom.set_atom_prop_real("myReal", 2.75);
        atom.set_atom_prop_str("myStr", "hello");
        assert_eq!(atom.atom_prop_int("myInt"), Some(42));
        assert_eq!(atom.atom_prop_real("myReal"), Some(2.75));
        assert_eq!(atom.atom_prop_str("myStr"), Some("hello"));
        assert!(atom.remove_atom_prop("myInt"));
        assert_eq!(atom.format_msg(), "A ALA 10 CA:B");
        assert_eq!(atom.to_string(), atom.format_msg());

        atom.set_parent_uid(999);
        let copied = atom.clone();
        assert_eq!(copied.parent_uid(), INVALID_UID);
        assert_eq!(copied.name(), atom.name());
        assert!(copied.has_anisou());
    }

    #[test]
    fn molresidue_and_molchain_operations_match_reference() {
        let mut residue = MolResidue::new();
        residue.set_name("ALA");
        residue.set_res_type("protein");
        residue.set_index(ResidIndex::with_ins_code(10, 'A'));
        residue.set_chain_name("A");
        residue.set_scr_index(7);
        residue.set_str_index("10A");
        residue.set_pivot_atom_name("CA");
        assert_eq!(residue.name(), "ALA");
        assert_eq!(residue.res_type(), "protein");
        assert_eq!(residue.scr_index(), 10);
        assert_eq!(residue.str_index(), "10A");
        assert_eq!(residue.pivot_atom_name(), "CA");
        residue.set_link_next_index(Some(ResidIndex::new(11)));
        assert!(residue.is_linked_to_index(ResidIndex::new(11)));
        assert!(!residue.is_linked_to_index(ResidIndex::new(12)));

        let mut atom = MolAtom::new();
        atom.set_id(1);
        atom.set_name("CA");
        atom.set_conf_id(Some('A'));
        assert!(residue.append_atom(atom));
        assert_eq!(residue.atom_size(), 1);
        assert_eq!(residue.atom_id("CA", Some('A')), Some(1));
        assert_eq!(residue.get_atom_alt_conf("CA", "A").unwrap().id(), 1);
        assert_eq!(
            residue.get_atoms_json(),
            "[{\"name\":\"CA\",\"id\":1,\"elem\":\"Unknown\"}]"
        );
        assert_eq!(residue.alt_confs("CA").len(), 1);
        residue.set_prop_str("color", "red");
        assert_eq!(residue.prop_str("color"), Some("red"));
        assert_eq!(residue.res_prop_names().len(), 1);
        assert!(residue.remove_prop_str("color"));
        assert!(residue.remove_atom("CA", Some('A')));

        let mut chain = MolChain::new();
        chain.set_name("B");
        chain.set_parent_uid(123);
        let mut residue1 = MolResidue::new();
        residue1.set_index(ResidIndex::new(3));
        let mut residue2 = MolResidue::new();
        residue2.set_index(ResidIndex::new(1));
        assert!(chain.append_residue(residue1));
        assert!(chain.append_residue(residue2));
        assert_eq!(chain.size(), 2);
        assert_eq!(chain.residues()[0].index(), ResidIndex::new(3));
        assert_eq!(
            chain.residues_in_index_order()[0].index(),
            ResidIndex::new(1)
        );
        assert!(chain.residue_by_str("1").unwrap().is_some());
        assert!(chain.get_residue("1").unwrap().is_some());
        assert_eq!(
            chain.get_resids_json(),
            "[{\"name\":\"\",\"single\":\"\",\"sel\":false,\"index\":\"1\"},{\"name\":\"\",\"single\":\"\",\"sel\":false,\"index\":\"3\"}]"
        );
        assert!(chain.remove_residue(ResidIndex::new(3)));
    }
}
