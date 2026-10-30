//! Residue topology dictionary — port of the C++ molstr `TopoDB` / `XMLTopparFile`
//! / `TopoBuilder` family.
//!
//! CueMol ships XML "toppar" dictionaries (`prot_top.xml`, `nucl_top.xml`,
//! `sugar_top.xml`, `mono_top.xml`) that define, per residue, the canonical atom
//! list and the intra-residue bonds (with bond orders), plus inter-residue *links*
//! (peptide `C-N`, phosphodiester `O3'-P`, disulfide `SG-SG`, …).  The original
//! `TopoBuilder::applyTopology()` uses these templates — instead of a pure
//! distance heuristic — to build connectivity: standard residues get exactly their
//! chemical bonds (so carbonyls become double bonds and crystal contacts are never
//! bonded), and only unknown residues (ligands, solvent) fall back to distance
//! autogeneration.
//!
//! This module bundles the four dictionaries via `include_str!`, parses them once
//! into a [`TopoDB`], and exposes [`build_topology_bonds`] which is what
//! `MolCoord::apply_topology` now drives.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::molecule::{Atom, BondOrder, MolCoord};

const PROT_TOP: &str = include_str!("../data/prot_top.xml");
const NUCL_TOP: &str = include_str!("../data/nucl_top.xml");
const SUGAR_TOP: &str = include_str!("../data/sugar_top.xml");
const MONO_TOP: &str = include_str!("../data/mono_top.xml");
// Residue-property dictionaries — their `<synonym>` entries map alternate residue
// names (Amber CYX/HID/HIE, CHARMM HSE/HSD/HSP, PDB-v3 DNA DA/DC/DG/DT, …) onto
// the canonical template id, so those residues still resolve to a topology.
const PROT_PROPS: &str = include_str!("../data/prot_props.xml");
const NUCL_PROPS: &str = include_str!("../data/nucl_props.xml");
/// Force-field parameters (per-atom-type vdW radii) — C++ `ParamDB`/default_params.xml.
const DEFAULT_PARAMS: &str = include_str!("../data/default_params.xml");

/// One atom entry of a residue template (`<atom id=".." elem=".." type=".."
/// charge=".." .../>`). `vtype` is the force-field atom TYPE (e.g. "CH1", "NH1"),
/// the key into the parameter DB for the united-atom vdW radius; `charge` is the
/// force-field partial charge.
#[derive(Debug, Clone, Default)]
pub struct TopAtom {
    /// Patch operation mode. Zero for ordinary residue topology atoms.
    pub mode: i32,
    pub name: String,
    pub elem: String,
    pub vtype: String,
    pub sig: f64,
    pub eps: f64,
    pub sig14: f64,
    pub eps14: f64,
    pub charge: f64,
}

/// A template bond between two named atoms (intra-residue or within a link).
#[derive(Debug, Clone)]
pub struct TopBond {
    /// Patch operation mode. Zero for ordinary residue topology bonds.
    pub mode: i32,
    pub a1: String,
    pub a2: String,
    pub order: BondOrder,
    /// Force constant, when supplied by a CNS parameter file.
    pub kf: f64,
    /// Ideal bond length (`value` attribute), used as the centre of the distance
    /// gate when the bond comes from a non-polymer link (e.g. disulfide).
    pub value: f64,
    /// Estimated standard deviation of the ideal length.
    pub esd: f64,
}

/// Topology modification or linkage definition.
///
/// This is the Rust counterpart of C++ `molstr::ResiPatch`. XML topology
/// `<link>` entries are represented as patches containing add-bond operations,
/// while CNS topology files can also populate atom add/delete/modify entries.
#[derive(Debug, Clone)]
pub struct ResiPatch {
    name: String,
    atoms: HashMap<String, TopAtom>,
    bonds: Vec<TopBond>,
    is_link: bool,
    prev_prefix: Option<char>,
    next_prefix: Option<char>,
    link_bond: Option<TopBond>,
    link_atom_dist: f64,
    poly_link: bool,
}

impl Default for ResiPatch {
    fn default() -> Self {
        Self {
            name: String::new(),
            atoms: HashMap::new(),
            bonds: Vec::new(),
            is_link: false,
            prev_prefix: None,
            next_prefix: None,
            link_bond: None,
            link_atom_dist: 0.0,
            poly_link: false,
        }
    }
}

impl ResiPatch {
    pub const PATCH_ADD: i32 = 1;
    pub const PATCH_DELETE: i32 = 2;
    pub const PATCH_MODIFY: i32 = 3;

    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    fn from_link(link: &LinkDef) -> Self {
        Self {
            name: link.id.clone(),
            bonds: link
                .bonds
                .iter()
                .cloned()
                .map(|mut bond| {
                    bond.mode = Self::PATCH_ADD;
                    bond
                })
                .collect(),
            poly_link: link.poly,
            ..Self::default()
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    /// Add an atom operation. Returns false when the exact atom name exists.
    pub fn add_atom(
        &mut self,
        name: impl Into<String>,
        atom_type: impl Into<String>,
        mode: i32,
    ) -> bool {
        let name = name.into();
        if self.atoms.contains_key(&name) {
            return false;
        }
        self.atoms.insert(
            name.clone(),
            TopAtom {
                mode,
                name,
                vtype: atom_type.into(),
                ..TopAtom::default()
            },
        );
        true
    }

    /// Add a bond operation and return it for parameter initialization.
    pub fn add_bond(
        &mut self,
        a1: impl Into<String>,
        a2: impl Into<String>,
        mode: i32,
    ) -> &mut TopBond {
        self.bonds.push(TopBond {
            mode,
            a1: a1.into(),
            a2: a2.into(),
            order: BondOrder::Single,
            kf: 0.0,
            value: 0.0,
            esd: 0.0,
        });
        self.bonds.last_mut().expect("bond was just appended")
    }

    pub fn atoms(&self) -> &HashMap<String, TopAtom> {
        &self.atoms
    }

    pub fn bonds(&self) -> &[TopBond] {
        &self.bonds
    }

    pub fn get_bond(&self, a1: &str, a2: &str) -> Option<&TopBond> {
        self.bonds
            .iter()
            .find(|bond| bond.a1.eq_ignore_ascii_case(a1) && bond.a2.eq_ignore_ascii_case(a2))
    }

    pub fn get_bond_mut(&mut self, a1: &str, a2: &str) -> Option<&mut TopBond> {
        self.bonds
            .iter_mut()
            .find(|bond| bond.a1.eq_ignore_ascii_case(a1) && bond.a2.eq_ignore_ascii_case(a2))
    }

    pub fn search_atom(&self, name: &str) -> Option<(i32, &str)> {
        self.atoms
            .get(name)
            .map(|atom| (atom.mode, atom.vtype.as_str()))
    }

    /// Detect the old CNS linkage-patch convention.
    ///
    /// A linkage has exactly one add-bond operation, and every atom operation
    /// begins with one of the two residue-prefix characters carried by that bond.
    pub fn check_link_obj(&mut self) -> bool {
        self.is_link = false;
        self.prev_prefix = None;
        self.next_prefix = None;
        self.link_bond = None;

        let mut added = self
            .bonds
            .iter()
            .filter(|bond| bond.mode == Self::PATCH_ADD);
        let Some(link_bond) = added.next().cloned() else {
            return false;
        };
        if added.next().is_some() {
            return false;
        }
        let (Some(prev), Some(next)) = (link_bond.a1.chars().next(), link_bond.a2.chars().next())
        else {
            return false;
        };
        if self.atoms.values().any(|atom| {
            atom.name
                .chars()
                .next()
                .is_none_or(|prefix| prefix != prev && prefix != next)
        }) {
            return false;
        }

        self.is_link = true;
        self.prev_prefix = Some(prev);
        self.next_prefix = Some(next);
        self.link_bond = Some(link_bond);
        true
    }

    pub fn is_link_obj(&self) -> bool {
        self.is_link
    }

    pub fn is_poly_link(&self) -> bool {
        self.poly_link
    }

    pub fn set_poly_link(&mut self, value: bool) {
        self.poly_link = value;
    }

    pub fn prev_prefix(&self) -> Option<char> {
        self.prev_prefix
    }

    pub fn next_prefix(&self) -> Option<char> {
        self.next_prefix
    }

    pub fn reverse(&mut self) {
        std::mem::swap(&mut self.prev_prefix, &mut self.next_prefix);
    }

    pub fn link_bond(&self) -> Option<&TopBond> {
        if self.poly_link {
            self.bonds.first()
        } else if self.is_link {
            self.link_bond.as_ref()
        } else {
            None
        }
    }

    pub fn link_dist(&self) -> f64 {
        if self.is_link {
            self.link_atom_dist
        } else {
            -1.0
        }
    }

    pub fn set_link_dist(&mut self, value: f64) {
        self.link_atom_dist = value;
    }
}

/// Residue topology template (`<resid id=".." group=".." ...>`).
#[derive(Debug, Clone, Default)]
pub struct ResiToppar {
    pub name: String,
    pub group: String,
    pub pivot_atom: String,
    pub properties: HashMap<String, String>,
    pub atoms: Vec<TopAtom>,
    pub bonds: Vec<TopBond>,
    atom_aliases: HashMap<String, String>,
    rings: Vec<Vec<String>>,
    main_chain_atoms: Option<Vec<String>>,
    side_chain_atoms: Option<Vec<String>>,
}

impl ResiToppar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Polymer residues participate in poly links; water/ion/non-polymer (and
    /// group-less) residues do not.  Mirrors `TopoBuilder::applyTopology` line
    /// "avoid to search linker data when ptop is not a polymer".
    pub fn is_polymer(&self) -> bool {
        !self.group.is_empty() && !matches!(self.group.as_str(), "water" | "ion" | "non-polymer")
    }

    /// Look up a template atom by its `id` (atom name), case-insensitive on the
    /// trimmed name (C++ `ResiToppar::getAtom`).
    pub fn get_atom(&self, name: &str) -> Option<&TopAtom> {
        let key = name.trim();
        if let Some(atom) = self
            .atoms
            .iter()
            .find(|atom| atom.name.trim().eq_ignore_ascii_case(key))
        {
            return Some(atom);
        }
        let canonical = self
            .atom_aliases
            .iter()
            .find(|(alias, _)| alias.eq_ignore_ascii_case(key))
            .map(|(_, canonical)| canonical)?;
        self.atoms
            .iter()
            .find(|atom| atom.name.eq_ignore_ascii_case(canonical))
    }

    pub fn get_atom_mut(&mut self, name: &str) -> Option<&mut TopAtom> {
        let key = name.trim();
        let canonical = self
            .atom_aliases
            .iter()
            .find(|(alias, _)| alias.eq_ignore_ascii_case(key))
            .map_or(key, |(_, canonical)| canonical.as_str())
            .to_string();
        self.atoms
            .iter_mut()
            .find(|atom| atom.name.trim().eq_ignore_ascii_case(&canonical))
    }

    pub fn get_atom_type(&self, name: &str) -> Option<&str> {
        self.get_atom(name).map(|atom| atom.vtype.as_str())
    }

    pub fn add_atom(
        &mut self,
        name: impl Into<String>,
        atom_type: impl Into<String>,
        sig: f64,
        eps: f64,
        sig14: f64,
        eps14: f64,
    ) -> bool {
        let name = name.into();
        if self.get_atom(&name).is_some() {
            return false;
        }
        self.atoms.push(TopAtom {
            name,
            vtype: atom_type.into(),
            sig,
            eps,
            sig14,
            eps14,
            ..TopAtom::default()
        });
        true
    }

    pub fn put_alias_name(&mut self, alias: impl Into<String>, canonical: impl Into<String>) {
        self.atom_aliases.insert(alias.into(), canonical.into());
    }

    pub fn add_bond(&mut self, a1: &str, a2: &str, kf: f64, value: f64) -> Option<&mut TopBond> {
        let a1 = self.get_atom(a1)?.name.clone();
        let a2 = self.get_atom(a2)?.name.clone();
        if self.get_bond(&a1, &a2).is_some() {
            return None;
        }
        self.bonds.push(TopBond {
            mode: 0,
            a1,
            a2,
            order: BondOrder::Single,
            kf,
            value,
            esd: 0.0,
        });
        self.bonds.last_mut()
    }

    pub fn get_bond(&self, a1: &str, a2: &str) -> Option<&TopBond> {
        let a1 = self.get_atom(a1)?.name.as_str();
        let a2 = self.get_atom(a2)?.name.as_str();
        self.bonds
            .iter()
            .find(|bond| (bond.a1 == a1 && bond.a2 == a2) || (bond.a1 == a2 && bond.a2 == a1))
    }

    pub fn get_bond_mut(&mut self, a1: &str, a2: &str) -> Option<&mut TopBond> {
        let a1 = self.get_atom(a1)?.name.clone();
        let a2 = self.get_atom(a2)?.name.clone();
        self.bonds
            .iter_mut()
            .find(|bond| (bond.a1 == a1 && bond.a2 == a2) || (bond.a1 == a2 && bond.a2 == a1))
    }

    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    pub fn bond_count(&self) -> usize {
        self.bonds.len()
    }

    pub fn set_property(&mut self, name: impl Into<String>, value: impl Into<String>) -> bool {
        let name = name.into();
        if self.properties.contains_key(&name) {
            return false;
        }
        self.properties.insert(name, value.into());
        true
    }

    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
    }

    pub fn residue_type(&self) -> &str {
        self.property("group").unwrap_or(&self.group)
    }

    pub fn add_pivot_atom(&mut self, name: impl Into<String>) -> bool {
        self.pivot_atom = name.into();
        true
    }

    pub fn add_ring<I, S>(&mut self, members: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let members = members.into_iter().map(Into::into).collect::<Vec<_>>();
        if !members.is_empty() {
            self.rings.push(members);
        }
    }

    pub fn rings(&self) -> &[Vec<String>] {
        &self.rings
    }

    pub fn add_side_chain_atoms<I, S>(&mut self, members: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if self.side_chain_atoms.is_none() {
            self.side_chain_atoms = Some(members.into_iter().map(Into::into).collect());
        }
    }

    pub fn add_main_chain_atoms<I, S>(&mut self, members: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if self.main_chain_atoms.is_none() {
            self.main_chain_atoms = Some(members.into_iter().map(Into::into).collect());
        }
    }

    pub fn has_side_chain_definition(&self) -> bool {
        self.side_chain_atoms.is_some()
    }

    pub fn has_main_chain_definition(&self) -> bool {
        self.main_chain_atoms.is_some()
    }

    pub fn is_side_chain_atom(&self, name: &str) -> bool {
        self.side_chain_atoms
            .as_ref()
            .is_some_and(|atoms| atoms.iter().any(|atom| atom == name))
    }

    pub fn is_main_chain_atom(&self, name: &str) -> bool {
        self.main_chain_atoms
            .as_ref()
            .is_none_or(|atoms| atoms.iter().any(|atom| atom == name))
    }
}

/// One atom-type's force-field vdW radii (C++ `param::AtomVal`). `vdwhr` is the
/// united-atom (H-absorbed) radius; `vdwr` the explicit-hydrogen radius.
#[derive(Debug, Clone, PartialEq)]
pub struct AtomParam {
    pub elem: String,
    pub hbon: String,
    pub hybrid: String,
    pub mass: f64,
    pub vdwr: f64,
    pub vdwhr: f64,
    pub ionr: f64,
    pub valency: i32,
}

impl Default for AtomParam {
    fn default() -> Self {
        Self {
            elem: String::new(),
            hbon: String::new(),
            hybrid: String::new(),
            mass: -1.0,
            vdwr: -1.0,
            vdwhr: -1.0,
            ionr: -1.0,
            valency: -1,
        }
    }
}

/// Force-field parameter dictionary keyed by atom TYPE (C++ `ParamDB`, loaded
/// from `default_params.xml`).
#[derive(Debug, Clone, Default)]
pub struct ParamDB {
    atoms: HashMap<String, AtomParam>,
}

impl ParamDB {
    /// The process-wide parameter DB parsed once from the bundled
    /// `default_params.xml`.
    pub fn global() -> &'static ParamDB {
        static DB: OnceLock<ParamDB> = OnceLock::new();
        DB.get_or_init(|| ParamDB::from_xml(DEFAULT_PARAMS))
    }

    pub fn from_xml(xml: &str) -> Self {
        let mut atoms = HashMap::new();
        for el in TagIter::new(xml) {
            if el.name == "atom" && !el.closing {
                if let Some(id) = el.attr("id") {
                    let atom = atoms.entry(id).or_insert_with(AtomParam::default);
                    if let Some(value) = el.attr("elem") {
                        atom.elem = value;
                    }
                    if let Some(value) = el.attr("hbon") {
                        atom.hbon = value;
                    }
                    if let Some(value) = el.attr("hybr") {
                        atom.hybrid = value;
                    }
                    if let Some(value) = el.attr("mass").and_then(|value| value.parse().ok()) {
                        atom.mass = value;
                    }
                    if let Some(value) = el.attr("vdwr").and_then(|value| value.parse().ok()) {
                        atom.vdwr = value;
                    }
                    if let Some(value) = el.attr("vdwhr").and_then(|value| value.parse().ok()) {
                        atom.vdwhr = value;
                    }
                    if let Some(value) = el.attr("ionr").and_then(|value| value.parse().ok()) {
                        atom.ionr = value;
                    }
                    if let Some(value) = el.attr("valency").and_then(|value| value.parse().ok()) {
                        atom.valency = value;
                    }
                }
            }
        }
        ParamDB { atoms }
    }

    pub fn add_atom(&mut self, id: impl Into<String>) -> bool {
        let id = id.into();
        if self.atoms.contains_key(&id) {
            return false;
        }
        self.atoms.insert(id, AtomParam::default());
        true
    }

    pub fn get(&self, vtype: &str) -> Option<&AtomParam> {
        self.atoms.get(vtype)
    }

    pub fn get_mut(&mut self, vtype: &str) -> Option<&mut AtomParam> {
        self.atoms.get_mut(vtype)
    }

    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }
}

/// Element-based vdW radius fallback (C++ `elemBasedVdw`): the 6 common elements
/// + a 1.7 Å default. Used when an atom has no residue template / type / param.
pub fn elem_based_vdw(element: &str) -> f64 {
    match element.trim().to_ascii_uppercase().as_str() {
        "H" => 1.2,
        "C" => 1.7,
        "N" => 1.55,
        "O" => 1.52,
        "S" => 1.8,
        "P" => 1.8,
        _ => 1.7,
    }
}

/// The atom's vdW radius for surface building (C++ `TopparManager::getVdwRadius`
/// with `bExplH=false`): resolve the residue template -> atom type -> the param
/// DB's united-atom `vdwhr` (or `vdwr` when `vdwhr<=0`); fall back to the
/// element-based radius when any lookup misses (e.g. ligands/ions/waters with no
/// template). This is the larger H-absorbed radius the production C++ surface uses
/// (e.g. a backbone CH1 carbon is 1.95, not the element 1.70).
pub fn vdw_radius_for_atom(atom: &Atom) -> f64 {
    vdw_radius_for_atom_mode(atom, false)
}

/// The atom's vdW radius (C++ `TopparManager::getVdwRadius(atom, bExplH)`): with
/// `explicit_h=false` (united-atom, the surface/default-PQR mode) the larger
/// H-absorbed `vdwhr`; with `explicit_h=true` (explicit-hydrogen mode) the bare
/// `vdwr`. Element-table fallback when the residue/type/param lookup misses.
pub fn vdw_radius_for_atom_mode(atom: &Atom, explicit_h: bool) -> f64 {
    vdw_radius_for_atom_with_db(atom, explicit_h, TopoDB::global(), ParamDB::global())
}

fn vdw_radius_for_atom_with_db(
    atom: &Atom,
    explicit_h: bool,
    topology: &TopoDB,
    parameters: &ParamDB,
) -> f64 {
    let fallback = elem_based_vdw(&atom.element);
    let Some(resi) = topology.residue(&atom.res_name) else {
        return fallback;
    };
    let Some(top_atom) = resi.get_atom(&atom.name) else {
        return fallback;
    };
    let Some(param) = parameters.get(&top_atom.vtype) else {
        return fallback;
    };
    if explicit_h {
        // Explicit-hydrogen mode uses the bare vdwr.
        if param.vdwr > 0.0 {
            param.vdwr
        } else {
            fallback
        }
    } else if param.vdwhr > 0.0 {
        param.vdwhr
    } else if param.vdwr > 0.0 {
        param.vdwr
    } else {
        fallback
    }
}

/// The atom's force-field partial charge from its residue template (C++
/// `TopparManager::getCharge`, the `pTA->charge` value); `None` for an atom with
/// no template (ligand/ion/water), where C++ `getCharge` returns false and the
/// PQR writer falls back to 0. NOTE: this is the explicit-H template charge; the
/// united-atom H-absorption (heavy atom += bonded H charges) is not applied here.
pub fn partial_charge_for_atom(atom: &Atom) -> Option<f64> {
    charge_for_atom_mode(atom, true, "")
}

/// The atom's UNITED-ATOM partial charge (C++ `TopparManager::getCharge`,
/// `bExplH=false`): the atom's own template charge plus the template charges of
/// its bonded hydrogens. The hydrogens need not be present in the molecule;
/// CueMol walks the residue topology because united-atom output drops them.
pub fn united_atom_partial_charge(molecule: &MolCoord, atom_index: usize) -> Option<f64> {
    let atom = molecule.atoms().get(atom_index)?;
    charge_for_atom_mode(atom, false, "")
}

/// Resolve an atom charge using the optional force-field namespace.
///
/// This is the direct counterpart of C++ `TopparManager::getCharge`: a namespace
/// changes topology atom lookup to `namespace:atom`, while united-atom mode still
/// discovers bonded hydrogens from the unqualified topology bonds.
pub fn charge_for_atom_mode(atom: &Atom, explicit_h: bool, namespace: &str) -> Option<f64> {
    charge_for_atom_with_db(atom, explicit_h, namespace, TopoDB::global())
}

fn charge_for_atom_with_db(
    atom: &Atom,
    explicit_h: bool,
    namespace: &str,
    topology: &TopoDB,
) -> Option<f64> {
    let residue = topology.residue(&atom.res_name)?;
    let qualified_name = if namespace.is_empty() {
        atom.name.clone()
    } else {
        format!("{namespace}:{}", atom.name)
    };
    let top_atom = residue.get_atom(&qualified_name)?;
    if explicit_h {
        return Some(top_atom.charge);
    }

    let mut total = top_atom.charge;
    for bond in &residue.bonds {
        let hydrogen_name = if bond.a1 == atom.name {
            Some(bond.a2.as_str())
        } else if bond.a2 == atom.name {
            Some(bond.a1.as_str())
        } else {
            None
        };
        let Some(hydrogen_name) = hydrogen_name else {
            continue;
        };
        let Some(hydrogen) = residue.get_atom(hydrogen_name) else {
            continue;
        };
        if !hydrogen.elem.eq_ignore_ascii_case("H") {
            continue;
        }
        let qualified_hydrogen = if namespace.is_empty() {
            hydrogen_name.to_string()
        } else {
            format!("{namespace}:{hydrogen_name}")
        };
        if let Some(hydrogen) = residue.get_atom(&qualified_hydrogen) {
            total += hydrogen.charge;
        }
    }
    Some(total)
}

/// Owned topology/parameter manager corresponding to C++ `molstr::TopparManager`.
#[derive(Debug, Clone)]
pub struct TopparManager {
    parameter_db: ParamDB,
    cns_parameter_db: crate::cns_par::ParamDB,
    topology_db: TopoDB,
}

impl Default for TopparManager {
    fn default() -> Self {
        Self {
            parameter_db: ParamDB::global().clone(),
            cns_parameter_db: crate::cns_par::builtin_param_db(),
            topology_db: TopoDB::global().clone(),
        }
    }
}

impl TopparManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self) -> bool {
        *self = Self::default();
        true
    }

    pub fn parameter_db(&self) -> &ParamDB {
        &self.parameter_db
    }

    pub fn get_param_db(&self) -> &ParamDB {
        self.parameter_db()
    }

    pub fn parameter_db_mut(&mut self) -> &mut ParamDB {
        &mut self.parameter_db
    }

    /// Legacy CNS bond/angle/torsion/non-bonded parameters. CueMol2 stored
    /// these in the same manager as XML atom radii; Rust keeps their distinct
    /// schemas in separate typed dictionaries.
    pub fn cns_parameter_db(&self) -> &crate::cns_par::ParamDB {
        &self.cns_parameter_db
    }

    pub fn cns_parameter_db_mut(&mut self) -> &mut crate::cns_par::ParamDB {
        &mut self.cns_parameter_db
    }

    pub fn topology_db(&self) -> &TopoDB {
        &self.topology_db
    }

    pub fn get_topo_db(&self) -> &TopoDB {
        self.topology_db()
    }

    pub fn topology_db_mut(&mut self) -> &mut TopoDB {
        &mut self.topology_db
    }

    pub fn get_resi_toppar(&self, residue_name: &str) -> Option<&ResiToppar> {
        self.topology_db.residue(residue_name)
    }

    pub fn create_resi_toppar(&mut self, residue_name: &str) -> &mut ResiToppar {
        if !self.topology_db.contains(residue_name) {
            self.topology_db.put(ResiToppar::new(residue_name));
        }
        self.topology_db
            .residue_mut(residue_name)
            .expect("created residue topology")
    }

    /// Read one legacy CNS or native XML parameter/topology file.
    ///
    /// This mirrors C++ `TopparManager::readPrmTop`: multiple calls append to
    /// the same dictionaries, allowing parameter, topology, property, and link
    /// files to be loaded in sequence.
    pub fn read_prm_top(&mut self, path: impl AsRef<std::path::Path>, file_type: &str) -> bool {
        self.try_read_prm_top(path, file_type).is_ok()
    }

    pub fn try_read_prm_top(
        &mut self,
        path: impl AsRef<std::path::Path>,
        file_type: &str,
    ) -> cuemol_core::Result<()> {
        match file_type {
            "cns" => crate::cns_par::read_cns_toppar_file(
                path,
                &mut self.cns_parameter_db,
                &mut self.topology_db,
            ),
            "xml" => read_xml_toppar_file(path, &mut self.parameter_db, &mut self.topology_db)
                .map(|_| ()),
            other => Err(cuemol_core::CueMolError::invalid_argument(format!(
                "unknown top/par format `{other}`"
            ))),
        }
    }

    pub fn read_prm_top_str(&mut self, text: &str, file_type: &str) -> cuemol_core::Result<()> {
        match file_type {
            "cns" => crate::cns_par::read_cns_toppar_str(
                text,
                &mut self.cns_parameter_db,
                &mut self.topology_db,
            ),
            "xml" => {
                read_xml_toppar_str(text, &mut self.parameter_db, &mut self.topology_db)?;
                Ok(())
            }
            other => Err(cuemol_core::CueMolError::invalid_argument(format!(
                "unknown top/par format `{other}`"
            ))),
        }
    }

    pub fn get_vdw_radius(&self, atom: &Atom, explicit_h: bool) -> f64 {
        vdw_radius_for_atom_with_db(atom, explicit_h, &self.topology_db, &self.parameter_db)
    }

    pub fn get_charge(&self, atom: &Atom, explicit_h: bool, namespace: &str) -> Option<f64> {
        charge_for_atom_with_db(atom, explicit_h, namespace, &self.topology_db)
    }

    pub fn is_amino_acid(residue_name: &str) -> bool {
        TopoDB::global()
            .residue(residue_name)
            .is_some_and(|residue| residue.group == "prot")
    }

    pub fn is_nucl_acid(residue_name: &str) -> bool {
        TopoDB::global()
            .residue(residue_name)
            .is_some_and(|residue| residue.group == "nucl")
    }
}

/// One target selector of a link (`<target group1=".." resid1=".." .../>`).
#[derive(Debug, Clone, Default)]
pub struct LinkTarget {
    pub group1: Option<String>,
    pub resid1: Option<String>,
    pub group2: Option<String>,
    pub resid2: Option<String>,
}

/// Inter-residue link (`<link id=".." poly="true">`).
#[derive(Debug, Clone, Default)]
pub struct LinkDef {
    pub id: String,
    pub poly: bool,
    pub targets: Vec<LinkTarget>,
    pub bonds: Vec<TopBond>,
}

/// The merged residue-topology dictionary.
#[derive(Debug, Clone, Default)]
pub struct TopoDB {
    residues: HashMap<String, ResiToppar>,
    links: Vec<LinkDef>,
    patches: HashMap<String, ResiPatch>,
    /// Alternate residue name (upper-cased) -> canonical template id (upper-cased).
    synonyms: HashMap<String, String>,
    /// Canonical residue id (upper-cased) -> its aromatic/saturated ring member
    /// atom-name lists (from `*_props.xml` `<rings><ring value=".."/>`).
    rings: HashMap<String, Vec<Vec<String>>>,
}

impl TopoDB {
    pub fn new() -> Self {
        Self::default()
    }

    /// The process-wide dictionary, parsed once from the bundled XML files.
    pub fn global() -> &'static TopoDB {
        static DB: OnceLock<TopoDB> = OnceLock::new();
        DB.get_or_init(|| {
            let mut db = TopoDB::default();
            for xml in [PROT_TOP, NUCL_TOP, SUGAR_TOP, MONO_TOP] {
                db.parse_xml(xml);
            }
            for xml in [PROT_PROPS, NUCL_PROPS] {
                db.parse_props(xml);
            }
            db
        })
    }

    /// Look up a residue template by name, resolving alternate names (synonyms)
    /// to their canonical residue when no direct match exists.
    pub fn residue(&self, name: &str) -> Option<&ResiToppar> {
        let key = name.trim().to_ascii_uppercase();
        if let Some(top) = self.residues.get(&key) {
            return Some(top);
        }
        let canonical = self.synonyms.get(&key)?;
        self.residues.get(canonical)
    }

    pub fn residue_mut(&mut self, name: &str) -> Option<&mut ResiToppar> {
        let key = name.trim().to_ascii_uppercase();
        let canonical = self.synonyms.get(&key).cloned().unwrap_or(key);
        self.residues.get_mut(&canonical)
    }

    pub fn put(&mut self, mut residue: ResiToppar) -> Option<ResiToppar> {
        let key = residue.name.trim().to_ascii_uppercase();
        residue.name = key.clone();
        if residue.rings.is_empty() {
            self.rings.remove(&key);
        } else {
            self.rings.insert(key.clone(), residue.rings.clone());
        }
        self.residues.insert(key, residue)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.residue(name).is_some()
    }

    pub fn remove(&mut self, name: &str) -> Option<ResiToppar> {
        let key = name.trim().to_ascii_uppercase();
        self.rings.remove(&key);
        self.residues.remove(&key)
    }

    pub fn clear(&mut self) {
        self.residues.clear();
        self.rings.clear();
    }

    pub fn put_alias_name(&mut self, alias: impl Into<String>, canonical: impl Into<String>) {
        self.synonyms.insert(
            alias.into().trim().to_ascii_uppercase(),
            canonical.into().trim().to_ascii_uppercase(),
        );
    }

    pub fn uid_decorated_name(name: &str, uid: u64) -> String {
        format!("{name}_UID{uid}")
    }

    pub fn residue_with_uid(&self, name: &str, uid: u64) -> Option<&ResiToppar> {
        self.residue(name).or_else(|| {
            let decorated = Self::uid_decorated_name(name, uid);
            self.residue(&decorated)
        })
    }

    /// Resolve an alternate residue name to its canonical id, if it is a synonym.
    pub fn canonical_name(&self, name: &str) -> Option<&str> {
        self.synonyms
            .get(&name.trim().to_ascii_uppercase())
            .map(String::as_str)
    }

    pub fn synonym_count(&self) -> usize {
        self.synonyms.len()
    }

    pub fn links(&self) -> &[LinkDef] {
        &self.links
    }

    pub fn patch(&self, name: &str) -> Option<&ResiPatch> {
        self.patches.get(name)
    }

    pub fn patch_mut(&mut self, name: &str) -> Option<&mut ResiPatch> {
        self.patches.get_mut(name)
    }

    pub fn patch_put(&mut self, patch: ResiPatch) -> Option<ResiPatch> {
        self.patches.insert(patch.name().to_string(), patch)
    }

    pub fn patch_prefix(&self, prefix: char, name: &str) -> Option<&ResiPatch> {
        self.patch(&format!("{prefix}{name}"))
    }

    pub fn patch_remove(&mut self, name: &str) -> Option<ResiPatch> {
        self.patches.remove(name)
    }

    pub fn patch_clear(&mut self) {
        self.patches.clear();
    }

    pub fn patch_count(&self) -> usize {
        self.patches.len()
    }

    pub fn put_link(&mut self, link: LinkDef) {
        self.patches
            .insert(link.id.clone(), ResiPatch::from_link(&link));
        self.links.push(link);
    }

    pub fn add_link_by_name(
        &mut self,
        prev_residue: &str,
        prev_prefix: char,
        next_residue: &str,
        next_prefix: char,
        link_name: &str,
    ) -> bool {
        let mut selected = None;
        for sign in ['+', '-'] {
            let patch_name = format!("{sign}{link_name}");
            let Some(patch) = self.patches.get_mut(&patch_name) else {
                continue;
            };
            patch.check_link_obj();
            if patch.prev_prefix() == Some(prev_prefix) && patch.next_prefix() == Some(next_prefix)
            {
                selected = Some(patch_name);
                break;
            }
        }
        let Some(patch_name) = selected else {
            return false;
        };
        let Some(patch) = self.patches.get(&patch_name) else {
            return false;
        };
        let Some(mut bond) = patch.link_bond().cloned() else {
            return false;
        };
        let strip_prefix =
            |name: &str, prefix: char| name.strip_prefix(prefix).map(ToString::to_string);
        let (prev_atom, next_atom) = if let (Some(prev), Some(next)) = (
            strip_prefix(&bond.a1, prev_prefix),
            strip_prefix(&bond.a2, next_prefix),
        ) {
            (prev, next)
        } else if let (Some(prev), Some(next)) = (
            strip_prefix(&bond.a2, prev_prefix),
            strip_prefix(&bond.a1, next_prefix),
        ) {
            (prev, next)
        } else {
            return false;
        };
        bond.a1 = prev_atom;
        bond.a2 = next_atom;
        if patch.link_dist() > 0.0 {
            bond.value = patch.link_dist();
            bond.esd = 0.0;
        }
        let residue_match =
            |name: &str| (!name.is_empty() && name != "*").then(|| name.to_ascii_uppercase());
        self.links.push(LinkDef {
            id: patch_name,
            // Legacy links are searched only between consecutive polymer residues.
            poly: true,
            targets: vec![LinkTarget {
                group1: None,
                resid1: residue_match(prev_residue),
                group2: None,
                resid2: residue_match(next_residue),
            }],
            bonds: vec![bond],
        });
        true
    }

    pub fn add_link2(
        &mut self,
        resid1: impl Into<String>,
        group1: impl Into<String>,
        resid2: impl Into<String>,
        group2: impl Into<String>,
        patch_name: &str,
    ) -> bool {
        let Some(patch) = self.patch(patch_name) else {
            return false;
        };
        let Some(bond) = patch.link_bond().cloned() else {
            return false;
        };
        let target = LinkTarget {
            group1: nonempty(group1.into()),
            resid1: nonempty(resid1.into()).map(|value| value.to_ascii_uppercase()),
            group2: nonempty(group2.into()),
            resid2: nonempty(resid2.into()).map(|value| value.to_ascii_uppercase()),
        };
        self.links.push(LinkDef {
            id: patch_name.to_string(),
            poly: patch.is_poly_link(),
            targets: vec![target],
            bonds: vec![bond],
        });
        true
    }

    pub fn link_clear(&mut self) {
        self.links.clear();
    }

    pub fn residue_count(&self) -> usize {
        self.residues.len()
    }

    /// Parse one toppar XML document, appending its residues and links.
    fn parse_xml(&mut self, xml: &str) {
        let mut cur_resid: Option<ResiToppar> = None;
        let mut cur_link: Option<LinkDef> = None;

        for el in TagIter::new(xml) {
            match (el.name.as_str(), el.closing) {
                ("resid", false) => {
                    let name = el.attr("id").unwrap_or_default().to_ascii_uppercase();
                    let group = el.attr("group").unwrap_or_default();
                    let mut properties = HashMap::new();
                    for property in ["three", "desc", "group", "single"] {
                        if let Some(value) = el.attr(property) {
                            properties.insert(property.to_string(), value);
                        }
                    }
                    for alias in [el.attr("three"), el.attr("single")].into_iter().flatten() {
                        let alias = alias.trim().to_ascii_uppercase();
                        if !alias.is_empty() && alias != name {
                            self.synonyms.entry(alias).or_insert_with(|| name.clone());
                        }
                    }
                    cur_resid = Some(ResiToppar {
                        name,
                        group,
                        properties,
                        ..Default::default()
                    });
                }
                ("resid", true) => {
                    if let Some(resid) = cur_resid.take() {
                        if !resid.rings.is_empty() {
                            self.rings.insert(resid.name.clone(), resid.rings.clone());
                        }
                        self.residues.insert(resid.name.clone(), resid);
                    }
                }
                ("atom", _) => {
                    if let Some(resid) = cur_resid.as_mut() {
                        resid.atoms.push(TopAtom {
                            mode: 0,
                            name: el.attr("id").unwrap_or_default(),
                            elem: el.attr("elem").unwrap_or_default(),
                            vtype: el.attr("type").unwrap_or_default(),
                            sig: 0.0,
                            eps: 0.0,
                            sig14: 0.0,
                            eps14: 0.0,
                            charge: el
                                .attr("charge")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0.0),
                        });
                    }
                }
                ("link", false) => {
                    cur_link = Some(LinkDef {
                        id: el.attr("id").unwrap_or_default(),
                        poly: el.attr("poly").as_deref() == Some("true"),
                        ..Default::default()
                    });
                }
                ("link", true) => {
                    if let Some(link) = cur_link.take() {
                        self.patches
                            .insert(link.id.clone(), ResiPatch::from_link(&link));
                        self.links.push(link);
                    }
                }
                ("target", _) => {
                    if let Some(link) = cur_link.as_mut() {
                        link.targets.push(LinkTarget {
                            group1: el.attr("group1"),
                            resid1: el.attr("resid1").map(|s| s.to_ascii_uppercase()),
                            group2: el.attr("group2"),
                            resid2: el.attr("resid2").map(|s| s.to_ascii_uppercase()),
                        });
                    }
                }
                ("bond", _) => {
                    let bond = TopBond {
                        mode: 0,
                        a1: el.attr("id1").unwrap_or_default(),
                        a2: el.attr("id2").unwrap_or_default(),
                        order: bond_order_from_str(el.attr("type").as_deref().unwrap_or("single")),
                        kf: 0.0,
                        value: el.attr("value").and_then(|s| s.parse().ok()).unwrap_or(0.0),
                        esd: el.attr("esd").and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    };
                    // A `<bond>` lives either inside the current link or the current
                    // residue; links are written after all residues, so the link
                    // context takes precedence when both happen to be open.
                    if let Some(link) = cur_link.as_mut() {
                        link.bonds.push(bond);
                    } else if let Some(resid) = cur_resid.as_mut() {
                        resid.bonds.push(bond);
                    }
                }
                ("ring", _) => {
                    if let (Some(resid), Some(value)) = (cur_resid.as_mut(), el.attr("value")) {
                        resid.add_ring(value.split_whitespace());
                    }
                }
                ("sidech", _) => {
                    if let (Some(resid), Some(value)) = (cur_resid.as_mut(), el.attr("value")) {
                        resid.add_side_chain_atoms(value.split_whitespace());
                    }
                }
                ("mainch", _) => {
                    if let (Some(resid), Some(value)) = (cur_resid.as_mut(), el.attr("value")) {
                        resid.add_main_chain_atoms(value.split_whitespace());
                    }
                }
                ("prop", _) => {
                    if let (Some(resid), Some(name), Some(value)) =
                        (cur_resid.as_mut(), el.attr("name"), el.attr("value"))
                    {
                        resid.set_property(name, value);
                    }
                }
                ("atomalias", _) => {
                    if let (Some(resid), Some(alias), Some(canonical)) =
                        (cur_resid.as_mut(), el.attr("value"), el.attr("cname"))
                    {
                        resid.put_alias_name(alias, canonical);
                    }
                }
                _ => {}
            }
        }
    }
}

impl TopoDB {
    /// Parse a residue-property XML, including aliases, pivot atoms, rings, and
    /// arbitrary string properties such as the nucleic-acid `basetype`.
    fn parse_props(&mut self, xml: &str) {
        let mut cur_id: Option<String> = None;
        let mut cur_rings: Vec<Vec<String>> = Vec::new();
        let mut cur_pivot = String::new();
        let mut cur_properties: HashMap<String, String> = HashMap::new();
        let mut cur_group = String::new();
        let mut cur_main_chain: Option<Vec<String>> = None;
        let mut cur_side_chain: Option<Vec<String>> = None;
        let mut cur_atom_aliases: Vec<(String, String)> = Vec::new();
        for el in TagIter::new(xml) {
            match (el.name.as_str(), el.closing) {
                ("resid", false) => {
                    cur_id = el.attr("id").map(|s| s.to_ascii_uppercase());
                    cur_rings = Vec::new();
                    cur_pivot = el.attr("pivot").unwrap_or_default();
                    cur_properties = HashMap::new();
                    cur_group = el.attr("group").unwrap_or_default();
                    cur_main_chain = None;
                    cur_side_chain = None;
                    cur_atom_aliases = Vec::new();
                    for property in ["three", "desc", "group", "single"] {
                        if let Some(value) = el.attr(property) {
                            cur_properties.insert(property.to_string(), value.clone());
                            if let Some(id) = cur_id.as_ref() {
                                let alias = value.trim().to_ascii_uppercase();
                                if !alias.is_empty() && alias != *id {
                                    self.synonyms.entry(alias).or_insert_with(|| id.clone());
                                }
                            }
                        }
                    }
                }
                ("resid", true) => {
                    if let Some(id) = cur_id.take() {
                        let residue =
                            self.residues
                                .entry(id.clone())
                                .or_insert_with(|| ResiToppar {
                                    name: id.clone(),
                                    group: cur_group.clone(),
                                    ..ResiToppar::default()
                                });
                        if residue.group.is_empty() && !cur_group.is_empty() {
                            residue.group = cur_group.clone();
                        }
                        if !cur_pivot.is_empty() {
                            residue.pivot_atom = std::mem::take(&mut cur_pivot);
                        }
                        for (name, value) in std::mem::take(&mut cur_properties) {
                            residue.properties.entry(name).or_insert(value);
                        }
                        if !cur_rings.is_empty() {
                            residue.rings.extend(cur_rings.clone());
                            self.rings
                                .insert(id.clone(), std::mem::take(&mut cur_rings));
                        }
                        if let Some(atoms) = cur_main_chain.take() {
                            residue.add_main_chain_atoms(atoms);
                        }
                        if let Some(atoms) = cur_side_chain.take() {
                            residue.add_side_chain_atoms(atoms);
                        }
                        for (alias, canonical) in std::mem::take(&mut cur_atom_aliases) {
                            residue.put_alias_name(alias, canonical);
                        }
                    }
                    cur_rings = Vec::new();
                    cur_pivot.clear();
                    cur_properties.clear();
                    cur_group.clear();
                    cur_main_chain = None;
                    cur_side_chain = None;
                    cur_atom_aliases.clear();
                }
                ("synonym", _) => {
                    if let (Some(id), Some(alt)) = (cur_id.as_ref(), el.attr("value")) {
                        let alt = alt.trim().to_ascii_uppercase();
                        if !alt.is_empty() {
                            self.synonyms.entry(alt).or_insert_with(|| id.clone());
                        }
                    }
                }
                ("ring", _) if cur_id.is_some() => {
                    if let Some(value) = el.attr("value") {
                        let members: Vec<String> =
                            value.split_whitespace().map(str::to_string).collect();
                        if members.len() >= 3 {
                            cur_rings.push(members);
                        }
                    }
                }
                ("sidech", _) if cur_id.is_some() => {
                    if let Some(value) = el.attr("value") {
                        cur_side_chain =
                            Some(value.split_whitespace().map(str::to_string).collect());
                    }
                }
                ("mainch", _) if cur_id.is_some() => {
                    if let Some(value) = el.attr("value") {
                        cur_main_chain =
                            Some(value.split_whitespace().map(str::to_string).collect());
                    }
                }
                ("atomalias", _) if cur_id.is_some() => {
                    if let (Some(alias), Some(canonical)) = (el.attr("value"), el.attr("cname")) {
                        cur_atom_aliases.push((alias, canonical));
                    }
                }
                ("prop", _) if cur_id.is_some() => {
                    if let (Some(name), Some(value)) = (el.attr("name"), el.attr("value")) {
                        cur_properties.insert(name, value);
                    }
                }
                _ => {}
            }
        }
    }

    /// The ring-atom-name lists for a residue (resolving synonyms), empty if the
    /// residue has no ring definition.
    pub fn rings(&self, name: &str) -> &[Vec<String>] {
        let key = name.trim().to_ascii_uppercase();
        if let Some(rings) = self.rings.get(&key) {
            return rings;
        }
        if let Some(canonical) = self.synonyms.get(&key) {
            if let Some(rings) = self.rings.get(canonical) {
                return rings;
            }
        }
        &[]
    }

    pub fn pivot_atom(&self, name: &str) -> Option<&str> {
        self.residue(name)
            .map(|residue| residue.pivot_atom.as_str())
            .filter(|value| !value.is_empty())
    }

    pub fn residue_property(&self, residue_name: &str, property: &str) -> Option<&str> {
        self.residue(residue_name)?.property(property)
    }
}

fn bond_order_from_str(value: &str) -> BondOrder {
    match value.trim().to_ascii_lowercase().as_str() {
        "double" => BondOrder::Double,
        "triple" => BondOrder::Triple,
        "aromatic" | "arom" => BondOrder::Aromatic,
        "deloc" => BondOrder::Other(15),
        _ => BondOrder::Single,
    }
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

// ---------------------------------------------------------------------------
// Minimal XML tag tokenizer.
//
// The toppar files are flat, regular, attribute-only XML; a full DOM parser would
// be overkill (and add a dependency).  `TagIter` yields one [`Tag`] per element
// start/end, skipping comments and the `<?xml?>` declaration.
// ---------------------------------------------------------------------------

struct Tag {
    name: String,
    attrs: Vec<(String, String)>,
    /// `true` for a closing tag `</name>`.
    closing: bool,
    /// `true` for an empty-element tag `<name/>`.
    self_closing: bool,
}

impl Tag {
    fn attr(&self, key: &str) -> Option<String> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }
}

struct TagIter<'a> {
    rest: &'a str,
}

impl<'a> TagIter<'a> {
    fn new(xml: &'a str) -> Self {
        TagIter { rest: xml }
    }
}

impl Iterator for TagIter<'_> {
    type Item = Tag;

    fn next(&mut self) -> Option<Tag> {
        loop {
            let lt = self.rest.find('<')?;
            self.rest = &self.rest[lt + 1..];

            if let Some(after) = self.rest.strip_prefix("!--") {
                // Comment: skip to "-->".
                let end = after.find("-->").map(|e| e + 3).unwrap_or(after.len());
                self.rest = &after[end..];
                continue;
            }
            if self.rest.starts_with('?') || self.rest.starts_with('!') {
                // Declaration / DOCTYPE: skip to '>'.
                let end = self
                    .rest
                    .find('>')
                    .map(|e| e + 1)
                    .unwrap_or(self.rest.len());
                self.rest = &self.rest[end..];
                continue;
            }

            let gt = self.rest.find('>')?;
            let inner = self.rest[..gt].trim();
            self.rest = &self.rest[gt + 1..];

            let closing = inner.starts_with('/');
            let self_closing = !closing && inner.ends_with('/');
            let inner = inner.trim_start_matches('/').trim_end_matches('/').trim();
            if inner.is_empty() {
                continue;
            }
            let mut chars = inner.splitn(2, char::is_whitespace);
            let name = chars.next().unwrap_or("").to_string();
            let attrs = parse_attrs(chars.next().unwrap_or(""));
            return Some(Tag {
                name,
                attrs,
                closing,
                self_closing,
            });
        }
    }
}

/// Extract `key="value"` pairs from a tag's attribute string.
fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].trim();
        rest = rest[eq + 1..].trim_start();
        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        rest = &rest[1..];
        let Some(end) = rest.find(quote) else { break };
        let value = &rest[..end];
        if !key.is_empty() {
            out.push((key.to_string(), value.to_string()));
        }
        rest = &rest[end + 1..];
    }
    out
}

/// Counts produced by one native XML top/par read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XmlTopparReadStats {
    pub residues: usize,
    pub links: usize,
}

/// Native XML parameter/topology reader with the C++ `attach`/`detach`
/// lifecycle.
#[derive(Debug, Default)]
pub struct XmlTopparFile<'a> {
    parameter_db: Option<&'a mut ParamDB>,
    topology_db: Option<&'a mut TopoDB>,
}

impl<'a> XmlTopparFile<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(&mut self, parameter_db: &'a mut ParamDB, topology_db: &'a mut TopoDB) {
        self.parameter_db = Some(parameter_db);
        self.topology_db = Some(topology_db);
    }

    pub fn detach(&mut self) {
        self.parameter_db = None;
        self.topology_db = None;
    }

    pub fn is_attached(&self) -> bool {
        self.parameter_db.is_some() && self.topology_db.is_some()
    }

    pub fn read_str(&mut self, xml: &str) -> cuemol_core::Result<XmlTopparReadStats> {
        let parameter_db = self.parameter_db.as_deref_mut().ok_or_else(|| {
            cuemol_core::CueMolError::invalid_argument("XML parameter database is not attached")
        })?;
        let topology_db = self.topology_db.as_deref_mut().ok_or_else(|| {
            cuemol_core::CueMolError::invalid_argument("XML topology database is not attached")
        })?;
        read_xml_toppar_str(xml, parameter_db, topology_db)
    }

    pub fn read(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> cuemol_core::Result<XmlTopparReadStats> {
        let xml = std::fs::read_to_string(path.as_ref())
            .map_err(|error| cuemol_core::CueMolError::Io(format!("XML top/par read: {error}")))?;
        self.read_str(&xml)
    }
}

/// Parse native CueMol top/par XML into existing parameter and topology
/// dictionaries.
pub fn read_xml_toppar_str(
    xml: &str,
    parameter_db: &mut ParamDB,
    topology_db: &mut TopoDB,
) -> cuemol_core::Result<XmlTopparReadStats> {
    XmlTopparParser::new(parameter_db, topology_db).parse(xml)
}

/// Parse one native CueMol top/par XML file.
pub fn read_xml_toppar_file(
    path: impl AsRef<std::path::Path>,
    parameter_db: &mut ParamDB,
    topology_db: &mut TopoDB,
) -> cuemol_core::Result<XmlTopparReadStats> {
    let xml = std::fs::read_to_string(path.as_ref())
        .map_err(|error| cuemol_core::CueMolError::Io(format!("XML top/par read: {error}")))?;
    read_xml_toppar_str(&xml, parameter_db, topology_db)
}

struct XmlTopparParser<'a> {
    parameter_db: &'a mut ParamDB,
    topology_db: &'a mut TopoDB,
    stack: Vec<String>,
    current_residue: Option<ResiToppar>,
    current_namespace: String,
    current_link: Option<LinkDef>,
    stats: XmlTopparReadStats,
}

impl<'a> XmlTopparParser<'a> {
    fn new(parameter_db: &'a mut ParamDB, topology_db: &'a mut TopoDB) -> Self {
        Self {
            parameter_db,
            topology_db,
            stack: Vec::new(),
            current_residue: None,
            current_namespace: String::new(),
            current_link: None,
            stats: XmlTopparReadStats::default(),
        }
    }

    fn parse(mut self, xml: &str) -> cuemol_core::Result<XmlTopparReadStats> {
        for tag in TagIter::new(xml) {
            let name = tag.name.to_ascii_lowercase();
            if tag.closing {
                self.end_element(&name)?;
                continue;
            }

            self.stack.push(name.clone());
            self.start_element(&tag)?;
            if tag.self_closing {
                self.end_element(&name)?;
            }
        }

        if !self.stack.is_empty() || self.current_residue.is_some() || self.current_link.is_some() {
            return Err(cuemol_core::CueMolError::parse(
                "unterminated XML top/par element",
            ));
        }
        Ok(self.stats)
    }

    fn start_element(&mut self, tag: &Tag) -> cuemol_core::Result<()> {
        if self.path_is(&["topology", "resid"]) {
            self.start_residue(tag)?;
        } else if self.path_is(&["topology", "resid", "atoms"]) {
            self.current_namespace = tag.attr("ns").unwrap_or_default();
        } else if self.path_is(&["resid", "atoms", "atom"]) {
            self.read_topology_atom(tag)?;
        } else if self.path_is(&["resid", "bonds", "bond"]) {
            self.read_topology_bond(tag)?;
        } else if self.path_is(&["resid", "synonyms", "synonym"]) {
            let alias = self.required_attr(tag, "value", "residue synonym")?;
            let canonical = self.current_residue()?.name.clone();
            self.topology_db.put_alias_name(alias, canonical);
        } else if self.path_is(&["resid", "synonyms", "atom"])
            || self.path_is(&["resid", "synonyms", "atomalias"])
        {
            let alias = self.required_attr(tag, "value", "atom synonym")?;
            let canonical = self.required_attr(tag, "cname", "atom synonym")?;
            self.current_residue()?.put_alias_name(alias, canonical);
        } else if self.path_is(&["resid", "rings", "ring"]) {
            let members = self
                .required_attr(tag, "value", "residue ring")?
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>();
            self.current_residue()?.add_ring(members);
        } else if self.path_is(&["topology", "resid", "sidech"]) {
            let members = self
                .required_attr(tag, "value", "residue side chain")?
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>();
            self.current_residue()?.add_side_chain_atoms(members);
        } else if self.path_is(&["topology", "resid", "mainch"]) {
            let members = self
                .required_attr(tag, "value", "residue main chain")?
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>();
            self.current_residue()?.add_main_chain_atoms(members);
        } else if self.path_is(&["topology", "resid", "prop"]) {
            let name = self.required_attr(tag, "name", "residue property")?;
            let value = self.required_attr(tag, "value", "residue property")?;
            self.current_residue()?.set_property(name, value);
        } else if self.path_is(&["links", "link"]) {
            self.start_link(tag)?;
        } else if self.path_is(&["link", "targets", "target"]) {
            self.read_link_target(tag)?;
        } else if self.path_is(&["link", "bonds", "bond"]) {
            self.read_link_bond(tag)?;
        } else if self.path_is(&["params", "atoms", "atom"]) {
            self.read_parameter_atom(tag)?;
        }
        Ok(())
    }

    fn end_element(&mut self, name: &str) -> cuemol_core::Result<()> {
        let Some(open_name) = self.stack.last() else {
            return Err(cuemol_core::CueMolError::parse(format!(
                "unexpected XML closing element `{name}`"
            )));
        };
        if open_name != name {
            return Err(cuemol_core::CueMolError::parse(format!(
                "mismatched XML closing element `{name}` for `{open_name}`"
            )));
        }

        if self.path_is(&["topology", "resid", "atoms"]) {
            self.current_namespace.clear();
        } else if self.path_is(&["topology", "resid"]) {
            let residue = self.current_residue.take().ok_or_else(|| {
                cuemol_core::CueMolError::parse("residue end without a current residue")
            })?;
            self.topology_db.put(residue);
            self.stats.residues += 1;
        } else if self.path_is(&["links", "link"]) {
            let link = self.current_link.take().ok_or_else(|| {
                cuemol_core::CueMolError::parse("link end without a current link")
            })?;
            self.topology_db
                .links
                .retain(|existing| existing.id != link.id);
            self.topology_db.patches.remove(&link.id);
            self.topology_db.put_link(link);
            self.stats.links += 1;
        }
        self.stack.pop();
        Ok(())
    }

    fn start_residue(&mut self, tag: &Tag) -> cuemol_core::Result<()> {
        if self.current_residue.is_some() {
            return Err(cuemol_core::CueMolError::parse(
                "nested XML residue definition",
            ));
        }
        let name = self
            .required_attr(tag, "id", "residue")?
            .trim()
            .to_ascii_uppercase();
        let mut residue = self
            .topology_db
            .residues
            .get(&name)
            .cloned()
            .unwrap_or_else(|| ResiToppar::new(&name));
        residue.name = name.clone();

        for property in ["three", "desc", "group", "single"] {
            if let Some(value) = tag.attr(property) {
                residue
                    .properties
                    .entry(property.to_string())
                    .or_insert_with(|| value.clone());
                if property == "group" && residue.group.is_empty() {
                    residue.group = value.clone();
                }
                if matches!(property, "three" | "single") {
                    let alias = value.trim().to_ascii_uppercase();
                    if !alias.is_empty() && alias != name {
                        self.topology_db
                            .synonyms
                            .entry(alias)
                            .or_insert_with(|| name.clone());
                    }
                }
            }
        }
        if let Some(pivot) = tag.attr("pivot") {
            residue.add_pivot_atom(pivot);
        }
        self.current_residue = Some(residue);
        Ok(())
    }

    fn read_topology_atom(&mut self, tag: &Tag) -> cuemol_core::Result<()> {
        let id = self.required_attr(tag, "id", "topology atom")?;
        let name = if self.current_namespace.is_empty() {
            id
        } else {
            format!("{}:{id}", self.current_namespace)
        };
        let residue = self.current_residue()?;
        if residue.get_atom(&name).is_none() {
            residue.add_atom(&name, "", 0.0, 0.0, 0.0, 0.0);
        }
        let atom = residue.get_atom_mut(&name).ok_or_else(|| {
            cuemol_core::CueMolError::parse(format!("cannot create topology atom `{name}`"))
        })?;
        if let Some(value) = tag.attr("elem") {
            atom.elem = value;
        }
        if let Some(value) = tag.attr("type") {
            atom.vtype = value;
        }
        if let Some(value) = tag.attr("charge").and_then(|value| value.parse().ok()) {
            atom.charge = value;
        }
        Ok(())
    }

    fn read_topology_bond(&mut self, tag: &Tag) -> cuemol_core::Result<()> {
        let a1 = self.required_attr(tag, "id1", "topology bond")?;
        let a2 = self.required_attr(tag, "id2", "topology bond")?;
        let residue = self.current_residue()?;
        if residue.get_bond(&a1, &a2).is_none() && residue.add_bond(&a1, &a2, 0.0, 0.0).is_none() {
            return Err(cuemol_core::CueMolError::undefined_reference(format!(
                "XML topology bond `{a1}`-`{a2}`"
            )));
        }
        let bond = residue.get_bond_mut(&a1, &a2).ok_or_else(|| {
            cuemol_core::CueMolError::parse(format!(
                "cannot create XML topology bond `{a1}`-`{a2}`"
            ))
        })?;
        if let Some(value) = tag.attr("type") {
            bond.order = bond_order_from_str(&value);
        }
        if let Some(value) = tag.attr("value").and_then(|value| value.parse().ok()) {
            bond.value = value;
        }
        if let Some(value) = tag.attr("esd").and_then(|value| value.parse().ok()) {
            bond.esd = value;
        }
        Ok(())
    }

    fn start_link(&mut self, tag: &Tag) -> cuemol_core::Result<()> {
        if self.current_link.is_some() {
            return Err(cuemol_core::CueMolError::parse(
                "nested XML link definition",
            ));
        }
        self.current_link = Some(LinkDef {
            id: self.required_attr(tag, "id", "link")?,
            poly: tag
                .attr("poly")
                .is_some_and(|value| value.eq_ignore_ascii_case("true")),
            ..LinkDef::default()
        });
        Ok(())
    }

    fn read_link_target(&mut self, tag: &Tag) -> cuemol_core::Result<()> {
        let target = LinkTarget {
            group1: tag.attr("group1"),
            resid1: tag.attr("resid1").map(|value| value.to_ascii_uppercase()),
            group2: tag.attr("group2"),
            resid2: tag.attr("resid2").map(|value| value.to_ascii_uppercase()),
        };
        self.current_link()?.targets.push(target);
        Ok(())
    }

    fn read_link_bond(&mut self, tag: &Tag) -> cuemol_core::Result<()> {
        let a1 = self.required_attr(tag, "id1", "link bond")?;
        let a2 = self.required_attr(tag, "id2", "link bond")?;
        self.current_link()?.bonds.push(TopBond {
            mode: ResiPatch::PATCH_ADD,
            a1,
            a2,
            order: tag
                .attr("type")
                .as_deref()
                .map(bond_order_from_str)
                .unwrap_or(BondOrder::Single),
            kf: 0.0,
            value: tag
                .attr("value")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0.0),
            esd: tag
                .attr("esd")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0.0),
        });
        Ok(())
    }

    fn read_parameter_atom(&mut self, tag: &Tag) -> cuemol_core::Result<()> {
        let id = self.required_attr(tag, "id", "parameter atom")?;
        self.parameter_db.atoms.entry(id.clone()).or_default();
        let atom = self
            .parameter_db
            .atoms
            .get_mut(&id)
            .expect("parameter atom was inserted");
        if let Some(value) = tag.attr("elem") {
            atom.elem = value;
        }
        if let Some(value) = tag.attr("hbon") {
            atom.hbon = value;
        }
        if let Some(value) = tag.attr("hybr") {
            atom.hybrid = value;
        }
        for (name, target) in [
            ("mass", &mut atom.mass),
            ("vdwr", &mut atom.vdwr),
            ("vdwhr", &mut atom.vdwhr),
            ("ionr", &mut atom.ionr),
        ] {
            if let Some(value) = tag.attr(name) {
                *target = value.parse().map_err(|_| {
                    cuemol_core::CueMolError::parse(format!(
                        "invalid parameter atom {name} `{value}`"
                    ))
                })?;
            }
        }
        if let Some(value) = tag.attr("valency") {
            atom.valency = value.parse().map_err(|_| {
                cuemol_core::CueMolError::parse(format!("invalid parameter atom valency `{value}`"))
            })?;
        }
        Ok(())
    }

    fn current_residue(&mut self) -> cuemol_core::Result<&mut ResiToppar> {
        self.current_residue
            .as_mut()
            .ok_or_else(|| cuemol_core::CueMolError::parse("XML residue child outside a residue"))
    }

    fn current_link(&mut self) -> cuemol_core::Result<&mut LinkDef> {
        self.current_link
            .as_mut()
            .ok_or_else(|| cuemol_core::CueMolError::parse("XML link child outside a link"))
    }

    fn required_attr(&self, tag: &Tag, name: &str, context: &str) -> cuemol_core::Result<String> {
        tag.attr(name).ok_or_else(|| {
            cuemol_core::CueMolError::parse(format!("XML {context} is missing `{name}` attribute"))
        })
    }

    fn path_is(&self, suffix: &[&str]) -> bool {
        self.stack.len() >= suffix.len()
            && self.stack[self.stack.len() - suffix.len()..]
                .iter()
                .map(String::as_str)
                .eq(suffix.iter().copied())
    }
}

// ---------------------------------------------------------------------------
// Bond construction
// ---------------------------------------------------------------------------

/// C++ `TopoDB::findLinkImpl2` accepts bonds up to ideal length plus ten ESDs.
const LINK_ESD_TOLERANCE: f64 = 10.0;

/// Per-residue map: upper-cased atom name -> the global atom indices carrying it
/// (more than one when alternate conformations are present).
type NameMap = HashMap<String, Vec<usize>>;

fn build_name_map(mol: &MolCoord, indices: &[usize]) -> NameMap {
    let mut map: NameMap = HashMap::new();
    for &i in indices {
        let name = mol.atoms()[i].name.trim().to_ascii_uppercase();
        map.entry(name).or_default().push(i);
    }
    map
}

fn alt_compatible(a: Option<char>, b: Option<char>) -> bool {
    a.is_none() || b.is_none() || a == b
}

fn distance(mol: &MolCoord, i: usize, j: usize) -> f64 {
    let a = mol.atoms()[i].position;
    let b = mol.atoms()[j].position;
    (a - b).length3d()
}

/// Add bonds between every alt-conf-compatible atom pair named `(a1, a2)` drawn
/// from `map1`/`map2`.  `gate` optionally restricts the bond length to `[lo, hi]`.
/// Returns the atom-index pairs actually bonded (`add_bond` dedups, so an
/// already-present bond is not reported).
fn bond_named(
    mol: &mut MolCoord,
    map1: &NameMap,
    a1: &str,
    map2: &NameMap,
    a2: &str,
    order: BondOrder,
    gate: Option<(f64, f64)>,
) -> Vec<(usize, usize)> {
    let a1 = a1.trim().to_ascii_uppercase();
    let a2 = a2.trim().to_ascii_uppercase();
    let (Some(list1), Some(list2)) = (map1.get(&a1), map2.get(&a2)) else {
        return Vec::new();
    };
    let mut added = Vec::new();
    for &i in list1 {
        for &j in list2 {
            if i == j {
                continue;
            }
            if !alt_compatible(mol.atoms()[i].alt_loc, mol.atoms()[j].alt_loc) {
                continue;
            }
            if let Some((lo, hi)) = gate {
                let d = distance(mol, i, j);
                if d < lo || d > hi {
                    continue;
                }
            }
            let before = mol.bond_count();
            mol.add_bond(i, j, order);
            if mol.bond_count() > before {
                added.push((i, j));
            }
        }
    }
    added
}

/// Per-class covalent bond-length cutoffs, ported verbatim from C++
/// `TopoBuilder::TopoBuilder` (`m_distmat`).  Atom classes (see [`atom_class`]):
/// 0 = H, 1 = period-2 light (B-F), 2 = period-3 (Si-Cl), 3 = heavier.
const DIST_MAT: [[f64; 4]; 4] = [
    [0.9, 1.2, 1.4, 1.4],
    [1.2, 1.6, 2.3, 2.3],
    [1.4, 2.3, 2.3, 2.3],
    [1.4, 2.3, 2.3, 2.3],
];

/// Atom bond-distance class, port of C++ `getAtomClass` (ordered by atomic
/// number): H; B..F; Si..Cl; other light elements (<= Cl) that never autobond
/// (`-1`); everything heavier than Cl.
fn atom_class(element: &str) -> i32 {
    match element.trim().to_ascii_uppercase().as_str() {
        "H" | "D" => 0,
        "B" | "C" | "N" | "O" | "F" => 1,
        "SI" | "P" | "S" | "CL" => 2,
        "" | "HE" | "LI" | "BE" | "NE" | "NA" | "MG" | "AL" => -1,
        _ => 3,
    }
}

/// Distance-based bond test between two atoms, port of `TopoBuilder::chkBondDist`.
fn chk_bond_dist(mol: &MolCoord, i: usize, j: usize) -> bool {
    let d = distance(mol, i, j);
    if d > DIST_MAT[3][3] {
        return false;
    }
    let a1 = atom_class(&mol.atoms()[i].element);
    let a2 = atom_class(&mol.atoms()[j].element);
    if a1 < 0 || a2 < 0 {
        return false;
    }
    d < DIST_MAT[a1 as usize][a2 as usize]
}

/// Distance-bond every atom in `indices` that is not already bonded by the
/// template (`templated`) to the other same-residue atoms — the second pass of
/// `TopoBuilder::appTopoResid` ("bond non-bonded atoms"), which is also what adds
/// terminal atoms (e.g. C-terminal `OXT`) that no residue template contains.  For
/// a residue with no template, `templated` is empty, so this becomes the full
/// per-residue distance autogeneration.
fn bond_unbonded_atoms(
    mol: &mut MolCoord,
    indices: &[usize],
    templated: &std::collections::HashSet<usize>,
) {
    for &i in indices {
        if templated.contains(&i) {
            continue;
        }
        for &j in indices {
            if i == j || !alt_compatible(mol.atoms()[i].alt_loc, mol.atoms()[j].alt_loc) {
                continue;
            }
            if chk_bond_dist(mol, i, j) {
                mol.add_bond(i, j, BondOrder::Single);
            }
        }
    }
}

/// Does residue `name`/`group` satisfy one side of a link target?
fn side_matches(
    group: Option<&String>,
    resid: Option<&String>,
    res_name: &str,
    res_group: &str,
) -> bool {
    if let Some(want) = resid {
        if !res_name.eq_ignore_ascii_case(want) {
            return false;
        }
    }
    if let Some(want) = group {
        if res_group != want {
            return false;
        }
    }
    true
}

/// A residue prepared for bonding: its key fields plus a name->indices map.
struct ResidueCtx {
    res_name: String,
    group: String,
    chain_id: String,
    map: NameMap,
}

fn target_score(target: &LinkTarget, prev: &ResidueCtx, next: &ResidueCtx) -> Option<i32> {
    if !side_matches(
        target.group1.as_ref(),
        target.resid1.as_ref(),
        &prev.res_name,
        &prev.group,
    ) || !side_matches(
        target.group2.as_ref(),
        target.resid2.as_ref(),
        &next.res_name,
        &next.group,
    ) {
        return None;
    }

    let side_score = |resid: &Option<String>, group: &Option<String>| {
        (if resid.is_some() { 3 } else { 1 }) + if group.is_some() { 2 } else { 1 }
    };
    Some(side_score(&target.resid1, &target.group1) + side_score(&target.resid2, &target.group2))
}

fn link_bond_upper_bound(bond: &TopBond) -> f64 {
    bond.value + bond.esd * LINK_ESD_TOLERANCE
}

fn link_bond_matches(mol: &MolCoord, prev: &ResidueCtx, next: &ResidueCtx, bond: &TopBond) -> bool {
    let a1 = bond.a1.trim().to_ascii_uppercase();
    let a2 = bond.a2.trim().to_ascii_uppercase();
    let (Some(atoms1), Some(atoms2)) = (prev.map.get(&a1), next.map.get(&a2)) else {
        return false;
    };
    let upper = link_bond_upper_bound(bond);
    atoms1.iter().any(|&i| {
        atoms2.iter().any(|&j| {
            alt_compatible(mol.atoms()[i].alt_loc, mol.atoms()[j].alt_loc)
                && distance(mol, i, j) <= upper
        })
    })
}

/// Select one link using the specificity scoring and distance test from C++
/// `TopoDB::findLinkImpl2`. Equal-scoring definitions retain XML order.
fn best_link<'a>(
    db: &'a TopoDB,
    mol: &MolCoord,
    prev: &ResidueCtx,
    next: &ResidueCtx,
    poly: bool,
) -> Option<&'a LinkDef> {
    let mut best = None;
    let mut best_score = 0;
    for link in db.links().iter().filter(|link| link.poly == poly) {
        let Some(bond) = link.bonds.first() else {
            continue;
        };
        if !link_bond_matches(mol, prev, next, bond) {
            continue;
        }
        for target in &link.targets {
            let Some(score) = target_score(target, prev, next) else {
                continue;
            };
            if score > best_score {
                best_score = score;
                best = Some(link);
            }
        }
    }
    best
}

/// Build connectivity for `mol` from the topology dictionary (see module docs).
pub fn build_topology_bonds(mol: &mut MolCoord) {
    build_topology_bonds_with_db_options(mol, TopoDB::global(), true);
}

pub fn build_topology_bonds_with_db(mol: &mut MolCoord, db: &TopoDB) {
    build_topology_bonds_with_db_options(mol, db, true);
}

pub fn build_topology_bonds_with_db_options(mol: &mut MolCoord, db: &TopoDB, auto_generate: bool) {
    mol.remove_nonpersistent_bonds();
    let summaries = mol.residue_summaries();

    // Phase 1 — intra-residue bonds: apply the residue template (correct bond
    // orders), then distance-bond any atoms the template did not cover (terminal
    // OXT, unknown ligand atoms, …). A residue with no template falls through to a
    // full per-residue distance autogeneration.
    let mut contexts: Vec<ResidueCtx> = Vec::with_capacity(summaries.len());
    let mut pivot_names: HashMap<&str, std::sync::Arc<str>> = HashMap::new();
    for summary in &summaries {
        let map = build_name_map(mol, &summary.atom_indices);
        let mut templated = std::collections::HashSet::new();
        let top = db.residue(&summary.res_name);
        let has_topology = top.is_some_and(|value| value.atom_count() > 0);
        let no_autogen = mol
            .get_residue_prop_str(
                &summary.key.chain_id,
                &summary.key.script_index(),
                "noautogen",
            )
            .ok()
            .flatten()
            .is_some_and(|value| value.eq_ignore_ascii_case("true"));
        let allow_autogen = auto_generate && !no_autogen;
        let group = match top {
            Some(top) if top.atom_count() > 0 => {
                if !top.pivot_atom.is_empty() {
                    let name = pivot_names
                        .entry(&top.pivot_atom)
                        .or_insert_with(|| std::sync::Arc::from(top.pivot_atom.as_str()));
                    mol.set_topology_pivot(&summary.key, name.clone());
                }
                for bond in &top.bonds {
                    for (i, j) in bond_named(mol, &map, &bond.a1, &map, &bond.a2, bond.order, None)
                    {
                        templated.insert(i);
                        templated.insert(j);
                    }
                }
                let residue_type = top.residue_type();
                if !residue_type.is_empty() {
                    let _ = mol.set_residue_prop_str(
                        &summary.key.chain_id,
                        &summary.key.script_index(),
                        "type",
                        residue_type,
                    );
                }
                top.group.clone()
            }
            Some(top) if allow_autogen => top.group.clone(),
            Some(_) => String::new(),
            None => String::new(),
        };
        if has_topology || allow_autogen {
            bond_unbonded_atoms(mol, &summary.atom_indices, &templated);
        }
        contexts.push(ResidueCtx {
            res_name: summary.res_name.clone(),
            group,
            chain_id: summary.key.chain_id.clone(),
            map,
        });
    }

    // Phase 2 — polymer links between consecutive same-chain residues
    // (peptide C-N, phosphodiester O3'-P, …). `summaries` is sorted by
    // (chain, res_id, ins_code), so adjacency here is sequence adjacency.
    for w in 1..contexts.len() {
        let (prev, cur) = (&contexts[w - 1], &contexts[w]);
        if prev.chain_id != cur.chain_id {
            continue;
        }
        if prev.group.is_empty() || matches!(prev.group.as_str(), "water" | "ion" | "non-polymer") {
            continue;
        }
        if let Some(link) = best_link(db, mol, prev, cur, true) {
            for bond in &link.bonds {
                let (m1, m2) = (prev.map.clone(), cur.map.clone());
                bond_named(
                    mol,
                    &m1,
                    &bond.a1,
                    &m2,
                    &bond.a2,
                    bond.order,
                    Some((0.0, link_bond_upper_bound(bond))),
                );
            }
        }
    }

    // Phase 3 — explicit non-polymer links over matching residue pairs. Restrict
    // each side to residues accepted by at least one target before forming the
    // Cartesian product. Typical proteins therefore test only CYS/MPR residues
    // for disulfides instead of every residue against every other residue.
    let side_candidates = |first: bool| {
        contexts
            .iter()
            .enumerate()
            .filter_map(|(index, context)| {
                db.links()
                    .iter()
                    .filter(|link| !link.poly)
                    .flat_map(|link| &link.targets)
                    .any(|target| {
                        if first {
                            side_matches(
                                target.group1.as_ref(),
                                target.resid1.as_ref(),
                                &context.res_name,
                                &context.group,
                            )
                        } else {
                            side_matches(
                                target.group2.as_ref(),
                                target.resid2.as_ref(),
                                &context.res_name,
                                &context.group,
                            )
                        }
                    })
                    .then_some(index)
            })
            .collect::<Vec<_>>()
    };
    let first_candidates = side_candidates(true);
    let second_candidates = side_candidates(false);
    for &i in &first_candidates {
        for &j in &second_candidates {
            if i == j {
                continue;
            }
            let Some(link) = best_link(db, mol, &contexts[i], &contexts[j], false) else {
                continue;
            };
            for bond in &link.bonds {
                let (m1, m2) = (contexts[i].map.clone(), contexts[j].map.clone());
                bond_named(
                    mol,
                    &m1,
                    &bond.a1,
                    &m2,
                    &bond.a2,
                    bond.order,
                    Some((0.0, link_bond_upper_bound(bond))),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_loads_all_bundled_residues_and_links() {
        let db = TopoDB::global();
        // The topology files define 44 residues; the property dictionaries add
        // modified nucleotide entries, matching XMLTopparFile's merge behavior.
        assert!(db.residue_count() > 44);
        // ALA template: 9 intra bonds, the C=O carbonyl is a double bond.
        let ala = db.residue("ALA").expect("ALA template");
        assert_eq!(ala.group, "prot");
        assert!(ala.is_polymer());
        let co = ala
            .bonds
            .iter()
            .find(|b| (b.a1 == "C" && b.a2 == "O") || (b.a1 == "O" && b.a2 == "C"))
            .expect("ALA C-O bond");
        assert_eq!(co.order, BondOrder::Double);
        // Links include the peptide bond and the disulfide.
        assert!(db.links().iter().any(|l| l.id == "trans-pept" && l.poly));
        let ss = db
            .links()
            .iter()
            .find(|l| l.id == "disulfide")
            .expect("disulfide link");
        assert!(!ss.poly);
        assert!((ss.bonds[0].value - 2.031).abs() < 1e-6);
        assert!((ss.bonds[0].esd - 0.020).abs() < 1e-6);

        // XML links are also exposed through the C++-compatible patch table.
        assert_eq!(db.patch_count(), db.links().len());
        let peptide = db.patch("trans-pept").expect("peptide patch");
        assert!(peptide.is_poly_link());
        assert_eq!(peptide.link_bond().unwrap().mode, ResiPatch::PATCH_ADD);
        assert!((peptide.link_bond().unwrap().esd - 0.014).abs() < 1e-6);
    }

    #[test]
    fn residue_toppar_mutation_and_alias_api_matches_reference() {
        let mut top = ResiToppar {
            name: "TST".to_string(),
            group: "ligand".to_string(),
            ..ResiToppar::default()
        };
        assert!(top.add_atom("A", "CT", 1.0, 2.0, 3.0, 4.0));
        assert!(top.add_atom("B", "NT", 0.0, 0.0, 0.0, 0.0));
        assert!(!top.add_atom("A", "duplicate", 0.0, 0.0, 0.0, 0.0));
        top.put_alias_name("A1", "A");
        assert_eq!(top.get_atom_type("A1"), Some("CT"));

        let bond = top.add_bond("A1", "B", 6000.0, 1.5).unwrap();
        bond.order = BondOrder::Double;
        assert!(top.add_bond("B", "A", 0.0, 0.0).is_none());
        assert_eq!(top.atom_count(), 2);
        assert_eq!(top.bond_count(), 1);
        assert_eq!(top.get_bond("B", "A1").unwrap().order, BondOrder::Double);

        assert!(top.set_property("desc", "test residue"));
        assert!(!top.set_property("desc", "replacement"));
        assert_eq!(top.property("desc"), Some("test residue"));
        assert!(top.add_pivot_atom("A"));
        top.add_ring(["A", "B", "C"]);
        top.add_main_chain_atoms(["A"]);
        top.add_side_chain_atoms(["A", "B"]);
        assert_eq!(top.rings()[0], ["A", "B", "C"]);
        assert!(top.is_main_chain_atom("A"));
        assert!(!top.is_main_chain_atom("B"));
        assert!(top.is_side_chain_atom("B"));
    }

    #[test]
    fn topology_db_mutation_and_custom_application_match_reference() {
        let mut top = ResiToppar::new("tst");
        top.group = "non-polymer".to_string();
        assert!(top.add_atom("A", "CT", 0.0, 0.0, 0.0, 0.0));
        assert!(top.add_atom("B", "CT", 0.0, 0.0, 0.0, 0.0));
        top.add_bond("A", "B", 0.0, 1.5).unwrap();

        let mut db = TopoDB::new();
        assert!(db.put(top).is_none());
        db.put_alias_name("alt", "tst");
        assert!(db.contains("ALT"));
        assert_eq!(db.residue("tst").unwrap().name, "TST");
        assert_eq!(db.residue_mut("alt").unwrap().group, "non-polymer");

        let mut decorated = ResiToppar::new(TopoDB::uid_decorated_name("LIG", 7));
        decorated.group = "non-polymer".to_string();
        db.put(decorated);
        assert_eq!(db.residue_with_uid("LIG", 7).unwrap().name, "LIG_UID7");

        let mut molecule = MolCoord::new();
        for (serial, name, x) in [(1, "A", 0.0), (2, "B", 10.0), (3, "C", 20.0)] {
            let mut atom = sg_atom(serial, 1, x);
            atom.name = name.to_string();
            atom.res_name = "TST".to_string();
            atom.element = "C".to_string();
            molecule.push_atom(atom);
        }
        molecule.add_bond(0, 2, BondOrder::Single);
        molecule.apply_topology_with_db(&db);
        assert_eq!(molecule.bond_count(), 1);
        assert_eq!(
            molecule.get_residue_prop_str("A", "1", "type").unwrap(),
            Some("non-polymer")
        );
        assert_eq!(
            (molecule.bonds()[0].atom1, molecule.bonds()[0].atom2),
            (0, 1)
        );

        molecule.add_bond(0, 2, BondOrder::Single);
        assert!(molecule.mark_bond_persistent(0, 2));
        molecule.apply_topology_with_db(&db);
        assert_eq!(molecule.bond_count(), 2);

        assert!(db.remove("TST").is_some());
        assert!(!db.contains("TST"));
        db.clear();
        assert_eq!(db.residue_count(), 0);
    }

    #[test]
    fn topology_application_honors_noautogen_for_unknown_residues() {
        let mut molecule = MolCoord::new();
        for (serial, name, x) in [(1, "C1", 0.0), (2, "C2", 1.4)] {
            let mut atom = sg_atom(serial, 1, x);
            atom.name = name.to_string();
            atom.res_name = "LIG".to_string();
            atom.element = "C".to_string();
            molecule.push_atom(atom);
        }
        molecule
            .set_residue_prop_str("A", "1", "noautogen", "true")
            .unwrap();
        molecule.apply_topology_with_db(&TopoDB::new());
        assert_eq!(molecule.bond_count(), 0);
    }

    #[test]
    fn topology_db_patch_and_link_mutation_api_matches_reference() {
        let mut patch = ResiPatch::new("join");
        patch.set_poly_link(true);
        let bond = patch.add_bond("A", "B", ResiPatch::PATCH_ADD);
        bond.value = 2.0;
        bond.esd = 0.1;

        let mut db = TopoDB::new();
        assert!(db.patch_put(patch).is_none());
        assert!(db.patch("join").is_some());
        assert!(db.add_link2("ONE", "", "TWO", "", "join"));
        assert_eq!(db.links().len(), 1);
        assert_eq!(db.links()[0].targets[0].resid1.as_deref(), Some("ONE"));
        db.link_clear();
        assert!(db.links().is_empty());
        assert!(db.patch_remove("join").is_some());
        db.patch_clear();
        assert_eq!(db.patch_count(), 0);
    }

    #[test]
    fn legacy_named_link_registration_resolves_patch_prefixes() {
        let mut patch = ResiPatch::new("+legacy");
        patch.add_bond("+C", "-N", ResiPatch::PATCH_ADD).value = 1.4;
        patch.set_link_dist(1.8);

        let mut db = TopoDB::new();
        db.patch_put(patch);
        assert!(db.add_link_by_name("ALA", '+', "GLY", '-', "legacy"));
        assert_eq!(db.links().len(), 1);
        let link = &db.links()[0];
        assert!(link.poly);
        assert_eq!(link.targets[0].resid1.as_deref(), Some("ALA"));
        assert_eq!(link.targets[0].resid2.as_deref(), Some("GLY"));
        assert_eq!(link.bonds[0].a1, "C");
        assert_eq!(link.bonds[0].a2, "N");
        assert!((link.bonds[0].value - 1.8).abs() < 1e-9);
        assert!(!db.add_link_by_name("ALA", '-', "GLY", '+', "missing"));
    }

    #[test]
    fn residue_patch_supports_cns_link_detection_and_deep_clone() {
        let mut patch = ResiPatch::new("+peptide");
        assert!(patch.add_atom("+C", "C", ResiPatch::PATCH_MODIFY));
        assert!(patch.add_atom("-N", "N", ResiPatch::PATCH_MODIFY));
        assert!(!patch.add_atom("+C", "duplicate", ResiPatch::PATCH_ADD));
        let bond = patch.add_bond("+C", "-N", ResiPatch::PATCH_ADD);
        bond.value = 1.329;
        bond.esd = 0.014;
        patch.set_link_dist(1.8);

        assert!(patch.check_link_obj());
        assert!(patch.is_link_obj());
        assert_eq!(patch.prev_prefix(), Some('+'));
        assert_eq!(patch.next_prefix(), Some('-'));
        assert_eq!(
            patch.search_atom("+C"),
            Some((ResiPatch::PATCH_MODIFY, "C"))
        );
        assert!(patch.get_bond("-N", "+C").is_none());
        assert!((patch.link_bond().unwrap().value - 1.329).abs() < 1e-9);
        assert!((patch.link_dist() - 1.8).abs() < 1e-9);

        let mut reversed = patch.clone();
        reversed.reverse();
        reversed.get_bond_mut("+C", "-N").unwrap().value = 2.0;
        assert_eq!(reversed.prev_prefix(), Some('-'));
        assert_eq!(reversed.next_prefix(), Some('+'));
        assert!((patch.get_bond("+C", "-N").unwrap().value - 1.329).abs() < 1e-9);
    }

    #[test]
    fn residue_synonyms_resolve_to_canonical_templates() {
        let db = TopoDB::global();
        assert!(db.synonym_count() >= 20);

        // Amber/CHARMM protein name variants resolve to the standard residue.
        assert_eq!(db.canonical_name("CYX"), Some("CYS")); // disulfide cysteine
        assert_eq!(db.canonical_name("HID"), Some("HIS")); // Amber histidine
        assert_eq!(db.canonical_name("HSE"), Some("HIS")); // CHARMM histidine
                                                           // residue() returns the canonical template for a synonym name.
        assert!(std::ptr::eq(
            db.residue("CYX").unwrap(),
            db.residue("CYS").unwrap()
        ));

        // PDB-v3 DNA names resolve to the nucleic templates (ids Ar/Cr/Gr/Td).
        for (alt, canon) in [("DA", "AR"), ("DC", "CR"), ("DG", "GR")] {
            assert_eq!(db.canonical_name(alt), Some(canon), "{alt}");
            assert!(db.residue(alt).is_some(), "{alt} template");
        }

        // A non-synonym, non-template name still resolves to nothing.
        assert!(db.residue("ZZZ").is_none());
    }

    /// A residue named with an Amber synonym (CYX) still gets its template bonds.
    #[test]
    fn synonym_named_residue_gets_template_bonds() {
        let mut mol = MolCoord::new();
        for (serial, name, x, y, z) in [
            (1, "N", 0.0, 0.0, 0.0),
            (2, "CA", 1.46, 0.0, 0.0),
            (3, "CB", 2.0, 1.4, 0.0),
            (4, "SG", 3.5, 1.5, 0.0),
            (5, "C", 2.0, -1.2, 0.0),
            (6, "O", 1.5, -2.3, 0.0),
        ] {
            let mut a = sg_atom(serial, 1, x);
            a.name = name.to_string();
            a.res_name = "CYX".to_string(); // Amber disulfide cysteine
            a.element = match name {
                "N" => "N",
                "O" => "O",
                "SG" => "S",
                _ => "C",
            }
            .to_string();
            a.position = Vector4::new3(x, y, z);
            mol.push_atom(a);
        }
        build_topology_bonds(&mut mol);
        // CYS template bonds N-CA, CA-CB, CB-SG, CA-C, C-O are all present despite
        // the residue being named CYX.
        assert!(mol.bond_count() >= 5, "got {} bonds", mol.bond_count());
    }

    #[test]
    fn vdw_radius_and_charge_resolve_through_topology_and_param_db() {
        // C++ TopparManager::getVdwRadius(false)/getCharge: resn+aname -> type ->
        // default_params.xml vdwhr / prot_top.xml charge. The expected values are
        // transcribed from the C++ data files (the oracle, NOT Rust's output).
        let mk = |name: &str, resn: &str, elem: &str| -> Atom {
            let mut a = sg_atom(1, 1, 0.0);
            a.name = name.to_string();
            a.res_name = resn.to_string();
            a.element = elem.to_string();
            a
        };
        // United-atom (H-absorbed) radii: a backbone CH1 carbon is 1.95, NOT the
        // element radius 1.70; ALA C->1.75, O->1.52, N(NH1)->1.60.
        assert!((vdw_radius_for_atom(&mk("CA", "ALA", "C")) - 1.95).abs() < 1e-9);
        assert!((vdw_radius_for_atom(&mk("C", "ALA", "C")) - 1.75).abs() < 1e-9);
        assert!((vdw_radius_for_atom(&mk("O", "ALA", "O")) - 1.52).abs() < 1e-9);
        assert!((vdw_radius_for_atom(&mk("N", "ALA", "N")) - 1.60).abs() < 1e-9);
        // Explicit-hydrogen mode uses the bare vdwr (CH1 = 1.70), not vdwhr (1.95).
        assert!((vdw_radius_for_atom_mode(&mk("CA", "ALA", "C"), true) - 1.70).abs() < 1e-9);
        assert!((vdw_radius_for_atom_mode(&mk("CA", "ALA", "C"), false) - 1.95).abs() < 1e-9);
        // A non-template residue (ligand) falls back to the element-based radius.
        assert!((vdw_radius_for_atom(&mk("C1", "LIG", "C")) - 1.70).abs() < 1e-9);
        // Partial charges from the prot_top.xml ALA template.
        assert!((partial_charge_for_atom(&mk("CA", "ALA", "C")).unwrap() - 0.058).abs() < 1e-9);
        assert!((partial_charge_for_atom(&mk("N", "ALA", "N")).unwrap() + 0.204).abs() < 1e-9);
        assert_eq!(partial_charge_for_atom(&mk("C1", "LIG", "C")), None);
        // default_params.xml lists 195 <atom> entries; one id repeats, so 194 unique.
        let params = ParamDB::global();
        assert_eq!(params.atom_count(), 194);
        let ch1 = params.get("CH1").unwrap();
        assert_eq!(ch1.elem, "C");
        assert_eq!(ch1.hbon, "N");
        assert_eq!(ch1.hybrid, "sp3");
        assert!((ch1.mass - 12.01150).abs() < 1e-9);
        assert_eq!(ch1.valency, 4);
        let nitrogen = params.get("N").unwrap();
        assert!((nitrogen.ionr - 1.32).abs() < 1e-9);
    }

    #[test]
    fn toppar_manager_facade_owns_mutable_dictionary_copies() {
        let mut manager = TopparManager::new();
        assert!(TopparManager::is_amino_acid("ALA"));
        assert!(TopparManager::is_nucl_acid("A"));
        assert!(manager.get_resi_toppar("ALA").is_some());

        let custom = manager.create_resi_toppar("ZZZ");
        custom.group = "non-polymer".to_string();
        assert_eq!(
            manager
                .get_resi_toppar("ZZZ")
                .map(|residue| residue.group.as_str()),
            Some("non-polymer")
        );

        let mut atom = sg_atom(1, 1, 0.0);
        atom.name = "CA".to_string();
        atom.res_name = "ALA".to_string();
        atom.element = "C".to_string();
        assert!((manager.get_vdw_radius(&atom, false) - 1.95).abs() < 1.0e-9);
        assert!((manager.get_charge(&atom, false, "").unwrap() - 0.104).abs() < 1.0e-9);
    }

    #[test]
    fn xml_toppar_reader_merges_all_native_sections() {
        let first = r#"
<toppar>
  <params ns="test">
    <atoms>
      <atom id="CX" elem="C" mass="12.0" vdwr="1.7" vdwhr="1.9" valency="4"/>
    </atoms>
  </params>
  <topology>
    <resid id="TST" three="TST" group="prot" pivot="A">
      <atoms>
        <atom id="A" elem="C" type="CX" charge="0.2"/>
        <atom id="B" elem="N" type="NX" charge="-0.2"/>
      </atoms>
      <atoms ns="pqr">
        <atom id="A" charge="0.4"/>
      </atoms>
      <bonds>
        <bond id1="A" id2="B" type="deloc" value="1.4" esd="0.03"/>
      </bonds>
      <synonyms>
        <synonym value="ALT"/>
        <atom value="OLD" cname="A"/>
      </synonyms>
    </resid>
  </topology>
  <links>
    <link id="test-link" poly="true">
      <targets><target resid1="TST" group2="prot"/></targets>
      <bonds><bond id1="A" id2="B" type="double" value="1.2" esd="0.02"/></bonds>
    </link>
  </links>
</toppar>
"#;
        let properties = r#"
<toppar><topology>
  <resid id="TST" single="Z">
    <rings><ring value="A B C"/></rings>
    <sidech value="A B"/>
    <mainch value="A"/>
    <prop name="kind" value="demo"/>
  </resid>
</topology></toppar>
"#;
        let mut parameters = ParamDB::default();
        let mut topology = TopoDB::new();
        let stats = read_xml_toppar_str(first, &mut parameters, &mut topology).unwrap();
        assert_eq!(
            stats,
            XmlTopparReadStats {
                residues: 1,
                links: 1
            }
        );
        let stats = read_xml_toppar_str(properties, &mut parameters, &mut topology).unwrap();
        assert_eq!(stats.residues, 1);

        assert_eq!(parameters.atom_count(), 1);
        assert_eq!(parameters.get("CX").unwrap().vdwhr, 1.9);
        let residue = topology.residue("TST").unwrap();
        assert_eq!(residue.atom_count(), 3);
        assert_eq!(residue.get_atom("OLD").unwrap().name, "A");
        assert_eq!(residue.get_atom("pqr:A").unwrap().charge, 0.4);
        assert_eq!(
            residue.get_bond("A", "B").unwrap().order,
            BondOrder::Other(15)
        );
        assert_eq!(residue.property("kind"), Some("demo"));
        assert!(residue.is_side_chain_atom("B"));
        assert!(residue.is_main_chain_atom("A"));
        assert_eq!(topology.canonical_name("ALT"), Some("TST"));
        assert_eq!(topology.canonical_name("Z"), Some("TST"));
        assert_eq!(topology.links()[0].bonds[0].order, BondOrder::Double);
        assert!(topology.patch("test-link").unwrap().is_poly_link());
    }

    #[test]
    fn xml_toppar_reader_loads_bundled_documents_without_replacing_residues() {
        let mut parameters = ParamDB::default();
        let mut topology = TopoDB::new();
        read_xml_toppar_str(DEFAULT_PARAMS, &mut parameters, &mut topology).unwrap();
        for xml in [PROT_TOP, NUCL_TOP, SUGAR_TOP, MONO_TOP] {
            read_xml_toppar_str(xml, &mut parameters, &mut topology).unwrap();
        }
        for xml in [PROT_PROPS, NUCL_PROPS] {
            read_xml_toppar_str(xml, &mut parameters, &mut topology).unwrap();
        }

        assert_eq!(parameters.atom_count(), ParamDB::global().atom_count());
        assert_eq!(topology.residue_count(), TopoDB::global().residue_count());
        assert_eq!(topology.links().len(), TopoDB::global().links().len());
        assert_eq!(topology.residue("ALA").unwrap().atom_count(), 10);
        assert!(topology.residue("ALA").unwrap().is_side_chain_atom("CB"));
        assert_eq!(
            topology
                .residue("PO4")
                .unwrap()
                .get_bond("P", "O1")
                .unwrap()
                .order,
            BondOrder::Other(15)
        );
    }

    #[test]
    fn xml_toppar_attached_reader_and_manager_match_reference_flow() {
        let xml = r#"<toppar><topology><resid id="ZZZ"><atoms>
          <atom id="A" elem="C" type="C"/>
        </atoms></resid></topology></toppar>"#;
        let mut parameters = ParamDB::default();
        let mut topology = TopoDB::new();
        {
            let mut reader = XmlTopparFile::new();
            reader.attach(&mut parameters, &mut topology);
            assert!(reader.is_attached());
            assert_eq!(reader.read_str(xml).unwrap().residues, 1);
            reader.detach();
            assert!(!reader.is_attached());
        }
        assert!(topology.contains("ZZZ"));

        let mut manager = TopparManager::new();
        manager.read_prm_top_str(xml, "xml").unwrap();
        manager
            .read_prm_top_str(
                "nonb ZX 0.1 1.0 0.05 0.9 resi ZC atom A type=ZX end end",
                "cns",
            )
            .unwrap();
        assert!(manager.get_resi_toppar("ZZZ").is_some());
        assert!(manager.get_resi_toppar("ZC").is_some());
        assert!(manager.cns_parameter_db().search_nonbonded("ZX").is_some());
        assert!(!manager.read_prm_top("missing.top", "unknown"));
    }

    #[test]
    fn xml_toppar_reader_rejects_invalid_parameter_values() {
        let mut parameters = ParamDB::default();
        let mut topology = TopoDB::new();
        let error = read_xml_toppar_str(
            r#"<toppar><params><atoms><atom id="X" mass="bad"/></atoms></params></toppar>"#,
            &mut parameters,
            &mut topology,
        )
        .unwrap_err();
        assert!(error.message().contains("mass"));
    }

    #[test]
    fn atom_param_db_add_and_duplicate_update_match_reference() {
        let mut db = ParamDB::default();
        assert!(db.add_atom("X"));
        assert!(!db.add_atom("X"));
        let defaults = db.get("X").unwrap();
        assert_eq!(defaults.mass, -1.0);
        assert_eq!(defaults.vdwr, -1.0);
        assert_eq!(defaults.valency, -1);
        db.get_mut("X").unwrap().elem = "Xe".to_string();
        assert_eq!(db.get("X").unwrap().elem, "Xe");

        let merged = ParamDB::from_xml(
            r#"<params><atom id="X" elem="C" mass="12"/><atom id="X" vdwr="1.7"/></params>"#,
        );
        let atom = merged.get("X").unwrap();
        assert_eq!(atom.elem, "C");
        assert_eq!(atom.mass, 12.0);
        assert_eq!(atom.vdwr, 1.7);
    }

    #[test]
    fn united_atom_charge_absorbs_bonded_hydrogens() {
        // C++ getCharge(bExplH=false): a heavy atom absorbs its bonded H charges
        // (the H are dropped in united-atom mode). ALA CB is CH3 (-0.120) bonded to
        // HB1/HB2/HB3 (HCH3, 0.040 each), so its united-atom charge is exactly 0.0,
        // while its explicit-H charge stays -0.120. Values from prot_top.xml.
        let mk = |serial: i32, name: &str, elem: &str| -> Atom {
            let mut a = sg_atom(serial, 1, 0.0);
            a.name = name.to_string();
            a.res_name = "ALA".to_string();
            a.element = elem.to_string();
            a
        };
        let mut mol = MolCoord::new();
        mol.push_atom(mk(1, "CB", "C")); // index 0
        mol.push_atom(mk(2, "HB1", "H"));
        mol.push_atom(mk(3, "HB2", "H"));
        mol.push_atom(mk(4, "HB3", "H"));
        for h in 1..=3 {
            mol.add_bond(0, h, BondOrder::Single);
        }
        assert!((partial_charge_for_atom(&mol.atoms()[0]).unwrap() + 0.120).abs() < 1e-9);
        assert!(
            united_atom_partial_charge(&mol, 0).unwrap().abs() < 1e-9,
            "CB (-0.120) + 3x HB (0.040) absorbed = 0.0, got {}",
            united_atom_partial_charge(&mol, 0).unwrap()
        );
        // The topology hydrogen is absorbed even when it is absent in the model.
        let lone = mk(5, "CA", "C");
        let mut mol2 = MolCoord::new();
        mol2.push_atom(lone);
        assert!((united_atom_partial_charge(&mol2, 0).unwrap() - 0.104).abs() < 1e-9);
    }

    #[test]
    fn water_and_ion_are_not_polymers() {
        let db = TopoDB::global();
        if let Some(hoh) = db.residue("HOH") {
            assert!(!hoh.is_polymer());
        }
    }

    #[test]
    fn residue_rings_parsed_from_props_xml() {
        let db = TopoDB::global();
        // PHE: a single 6-membered aromatic ring (CG CD1 CE1 CZ CE2 CD2).
        let phe = db.rings("PHE");
        assert_eq!(phe.len(), 1);
        assert_eq!(phe[0].len(), 6);
        assert_eq!(phe[0][0], "CG");
        assert_eq!(phe[0][3], "CZ");
        // HIS: a 5-membered imidazole ring.
        assert_eq!(db.rings("HIS")[0].len(), 5);
        // A non-ring residue (or unknown) has none.
        assert!(db.rings("ALA").is_empty());
        assert!(db.rings("ZZZ").is_empty());
        // Ring-only definitions embedded in mono_top.xml are retained too.
        assert_eq!(db.rings("ATP").len(), 3);
    }

    #[test]
    fn residue_main_and_side_chain_tables_are_loaded() {
        let db = TopoDB::global();
        let ala = db.residue("ALA").unwrap();
        assert!(ala.has_main_chain_definition());
        assert!(ala.has_side_chain_definition());
        assert!(ala.is_main_chain_atom("CA"));
        assert!(!ala.is_main_chain_atom("CB"));
        // CueMol includes CA as the side-chain anchor.
        assert!(ala.is_side_chain_atom("CA"));
        assert!(ala.is_side_chain_atom("CB"));
        assert!(!ala.is_side_chain_atom("N"));

        // Property-only modified nucleotides are registered like XMLTopparFile.
        let h2u = db.residue("H2U").expect("modified nucleotide properties");
        assert_eq!(h2u.group, "nucl");
        assert!(h2u.is_main_chain_atom("P"));
        assert!(h2u.is_side_chain_atom("N1"));
    }

    #[test]
    fn nucleotide_properties_and_single_letter_aliases_are_parsed() {
        let db = TopoDB::global();
        assert_eq!(db.canonical_name("A"), Some("AR"));
        let adenine = db.residue("A").expect("single-letter nucleotide alias");
        assert_eq!(adenine.name, "AR");
        assert_eq!(db.pivot_atom("DA"), Some("P"));
        assert_eq!(db.residue_property("ADE", "basetype"), Some("pur"));
        assert_eq!(db.residue_property("DT", "basetype"), Some("pyr"));
    }

    #[test]
    fn parse_attrs_handles_quotes_and_spacing() {
        let attrs = parse_attrs(r#"id="N"  elem="N" type="NH1" charge="-0.204""#);
        assert_eq!(attrs.len(), 4);
        assert_eq!(attrs[0], ("id".to_string(), "N".to_string()));
        assert_eq!(attrs[3], ("charge".to_string(), "-0.204".to_string()));
    }

    use crate::molecule::Atom;
    use cuemol_core::Vector4;

    fn sg_atom(serial: i32, res_id: i32, x: f64) -> Atom {
        Atom {
            serial,
            name: "SG".to_string(),
            alt_loc: None,
            res_name: "CYS".to_string(),
            chain_id: "A".to_string(),
            res_id,
            ins_code: None,
            position: Vector4::new3(x, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "S".to_string(),
            formal_charge: None,
            int_props: Default::default(),
            real_props: Default::default(),
        }
    }

    /// The disulfide link forms an SG-SG bond between two CYS only when the sulfurs
    /// are within bonding distance — independent of any PDB SSBOND record.
    #[test]
    fn disulfide_link_is_distance_gated() {
        // SG-SG at the ideal 2.03 Å -> bonded.
        let mut bonded = MolCoord::new();
        bonded.push_atom(sg_atom(1, 3, 0.0));
        bonded.push_atom(sg_atom(2, 40, 2.03));
        build_topology_bonds(&mut bonded);
        assert_eq!(bonded.bond_count(), 1, "near cysteines disulfide-bond");

        // SG-SG 4 Å apart (reduced cysteines) -> no bond.
        let mut apart = MolCoord::new();
        apart.push_atom(sg_atom(1, 3, 0.0));
        apart.push_atom(sg_atom(2, 40, 4.0));
        build_topology_bonds(&mut apart);
        assert_eq!(apart.bond_count(), 0, "distant cysteines do not bond");
    }

    #[test]
    fn competing_polymer_links_choose_first_best_match() {
        let sugar_atom = |serial: i32, res_id: i32, name: &str, x: f64| {
            let mut atom = sg_atom(serial, res_id, x);
            atom.name = name.to_string();
            atom.res_name = "NAG".to_string();
            atom.element = if name.starts_with('O') { "O" } else { "C" }.to_string();
            atom
        };

        // Both ALPHA1-2 (O2-C1) and ALPHA1-3 (O3-C1) pass their 10-ESD
        // distance gates. C++ retains XML order when target scores tie, so only
        // the first definition is applied.
        let mut mol = MolCoord::new();
        mol.push_atom(sugar_atom(1, 1, "O2", 1.4));
        mol.push_atom(sugar_atom(2, 1, "O3", -1.4));
        mol.push_atom(sugar_atom(3, 2, "C1", 0.0));
        build_topology_bonds(&mut mol);

        assert!(mol.bonds().iter().any(|bond| {
            (bond.atom1 == 0 && bond.atom2 == 2) || (bond.atom1 == 2 && bond.atom2 == 0)
        }));
        assert!(!mol.bonds().iter().any(|bond| {
            (bond.atom1 == 1 && bond.atom2 == 2) || (bond.atom1 == 2 && bond.atom2 == 1)
        }));
    }
}
