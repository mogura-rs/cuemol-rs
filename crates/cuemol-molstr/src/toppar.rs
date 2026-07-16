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
    pub name: String,
    pub elem: String,
    pub vtype: String,
    pub charge: f64,
}

/// A template bond between two named atoms (intra-residue or within a link).
#[derive(Debug, Clone)]
pub struct TopBond {
    pub a1: String,
    pub a2: String,
    pub order: BondOrder,
    /// Ideal bond length (`value` attribute), used as the centre of the distance
    /// gate when the bond comes from a non-polymer link (e.g. disulfide).
    pub value: f64,
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
}

impl ResiToppar {
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
        self.atoms
            .iter()
            .find(|atom| atom.name.trim().eq_ignore_ascii_case(key))
    }

    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
    }
}

/// One atom-type's force-field vdW radii (C++ `param::AtomVal`). `vdwhr` is the
/// united-atom (H-absorbed) radius; `vdwr` the explicit-hydrogen radius.
#[derive(Debug, Clone, Copy, Default)]
pub struct AtomParam {
    pub vdwr: f64,
    pub vdwhr: f64,
}

/// Force-field parameter dictionary keyed by atom TYPE (C++ `ParamDB`, loaded
/// from `default_params.xml`).
#[derive(Debug, Default)]
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
                    let num =
                        |name: &str| el.attr(name).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                    atoms.insert(
                        id,
                        AtomParam {
                            vdwr: num("vdwr"),
                            vdwhr: num("vdwhr"),
                        },
                    );
                }
            }
        }
        ParamDB { atoms }
    }

    pub fn get(&self, vtype: &str) -> Option<&AtomParam> {
        self.atoms.get(vtype)
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
    let fallback = elem_based_vdw(&atom.element);
    let Some(resi) = TopoDB::global().residue(&atom.res_name) else {
        return fallback;
    };
    let Some(top_atom) = resi.get_atom(&atom.name) else {
        return fallback;
    };
    let Some(param) = ParamDB::global().get(&top_atom.vtype) else {
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
    let resi = TopoDB::global().residue(&atom.res_name)?;
    let top_atom = resi.get_atom(&atom.name)?;
    Some(top_atom.charge)
}

/// The atom's UNITED-ATOM partial charge (C++ `TopparManager::getCharge`,
/// `bExplH=false`): the atom's own template charge PLUS the template charges of
/// its bonded hydrogens (which are absorbed into the heavy atom because explicit
/// H are dropped in united-atom mode, so their charge must be conserved here).
/// `None` for an atom with no residue template. For a hydrogen-free structure this
/// equals [`partial_charge_for_atom`] (nothing to absorb).
pub fn united_atom_partial_charge(molecule: &MolCoord, atom_index: usize) -> Option<f64> {
    let atom = molecule.atoms().get(atom_index)?;
    let mut total = partial_charge_for_atom(atom)?;
    for bond in molecule.bonds() {
        let other = if bond.atom1 == atom_index {
            bond.atom2
        } else if bond.atom2 == atom_index {
            bond.atom1
        } else {
            continue;
        };
        if let Some(neighbour) = molecule.atoms().get(other) {
            if neighbour.element.trim().eq_ignore_ascii_case("H") {
                if let Some(h_charge) = partial_charge_for_atom(neighbour) {
                    total += h_charge;
                }
            }
        }
    }
    Some(total)
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
#[derive(Debug, Default)]
pub struct TopoDB {
    residues: HashMap<String, ResiToppar>,
    links: Vec<LinkDef>,
    /// Alternate residue name (upper-cased) -> canonical template id (upper-cased).
    synonyms: HashMap<String, String>,
    /// Canonical residue id (upper-cased) -> its aromatic/saturated ring member
    /// atom-name lists (from `*_props.xml` `<rings><ring value=".."/>`).
    rings: HashMap<String, Vec<Vec<String>>>,
}

impl TopoDB {
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
                        self.residues.insert(resid.name.clone(), resid);
                    }
                }
                ("atom", _) => {
                    if let Some(resid) = cur_resid.as_mut() {
                        resid.atoms.push(TopAtom {
                            name: el.attr("id").unwrap_or_default(),
                            elem: el.attr("elem").unwrap_or_default(),
                            vtype: el.attr("type").unwrap_or_default(),
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
                        a1: el.attr("id1").unwrap_or_default(),
                        a2: el.attr("id2").unwrap_or_default(),
                        order: bond_order_from_str(el.attr("type").as_deref().unwrap_or("single")),
                        value: el.attr("value").and_then(|s| s.parse().ok()).unwrap_or(0.0),
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
        for el in TagIter::new(xml) {
            match (el.name.as_str(), el.closing) {
                ("resid", false) => {
                    cur_id = el.attr("id").map(|s| s.to_ascii_uppercase());
                    cur_rings = Vec::new();
                    cur_pivot = el.attr("pivot").unwrap_or_default();
                    cur_properties = HashMap::new();
                    for property in ["single"] {
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
                        if !cur_rings.is_empty() {
                            self.rings
                                .insert(id.clone(), std::mem::take(&mut cur_rings));
                        }
                        if let Some(residue) = self.residues.get_mut(&id) {
                            residue.pivot_atom = std::mem::take(&mut cur_pivot);
                            residue
                                .properties
                                .extend(std::mem::take(&mut cur_properties));
                        }
                    }
                    cur_rings = Vec::new();
                    cur_pivot.clear();
                    cur_properties.clear();
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
        _ => BondOrder::Single,
    }
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

// ---------------------------------------------------------------------------
// Bond construction
// ---------------------------------------------------------------------------

/// Distance tolerance (Å) around a non-polymer link's ideal length when deciding
/// whether a candidate residue pair is actually linked (e.g. which cysteines form
/// a disulfide).
const LINK_DIST_TOL: f64 = 0.6;
/// Maximum peptide/poly-link bond length (Å).  Faithful CueMol bonds consecutive
/// same-chain residues unconditionally; we additionally reject links longer than
/// this so a chain break (missing residues) does not produce a spurious long bond.
const POLY_DIST_MAX: f64 = 2.5;

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

/// Build connectivity for `mol` from the topology dictionary (see module docs).
pub fn build_topology_bonds(mol: &mut MolCoord) {
    let db = TopoDB::global();
    let summaries = mol.residue_summaries();

    // Phase 1 — intra-residue bonds: apply the residue template (correct bond
    // orders), then distance-bond any atoms the template did not cover (terminal
    // OXT, unknown ligand atoms, …). A residue with no template falls through to a
    // full per-residue distance autogeneration.
    let mut contexts: Vec<ResidueCtx> = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        let map = build_name_map(mol, &summary.atom_indices);
        let mut templated = std::collections::HashSet::new();
        let group = match db.residue(&summary.res_name) {
            Some(top) => {
                for bond in &top.bonds {
                    for (i, j) in bond_named(mol, &map, &bond.a1, &map, &bond.a2, bond.order, None)
                    {
                        templated.insert(i);
                        templated.insert(j);
                    }
                }
                top.group.clone()
            }
            None => String::new(),
        };
        bond_unbonded_atoms(mol, &summary.atom_indices, &templated);
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
        let Some(prev_top) = db.residue(&prev.res_name) else {
            continue;
        };
        if !prev_top.is_polymer() {
            continue;
        }
        for link in db.links().iter().filter(|l| l.poly) {
            if !link.targets.iter().any(|t| {
                side_matches(
                    t.group1.as_ref(),
                    t.resid1.as_ref(),
                    &prev.res_name,
                    &prev.group,
                ) && side_matches(
                    t.group2.as_ref(),
                    t.resid2.as_ref(),
                    &cur.res_name,
                    &cur.group,
                )
            }) {
                continue;
            }
            for bond in &link.bonds {
                // Borrow split: clone the small maps out so we can take &mut mol.
                let (m1, m2) = (prev.map.clone(), cur.map.clone());
                bond_named(
                    mol,
                    &m1,
                    &bond.a1,
                    &m2,
                    &bond.a2,
                    bond.order,
                    Some((0.4, POLY_DIST_MAX)),
                );
            }
        }
    }

    // Phase 3 — explicit (non-polymer) links over any matching residue pair,
    // gated by distance (which cysteines actually form a disulfide).  Each target
    // side is pre-filtered to its matching residues, so the pairing is
    // O(|side1| x |side2|) — for disulfides that is O(n_cys^2), not O(n_res^2).
    for link in db.links().iter().filter(|l| !l.poly) {
        for target in &link.targets {
            let side1: Vec<usize> = (0..contexts.len())
                .filter(|&i| {
                    side_matches(
                        target.group1.as_ref(),
                        target.resid1.as_ref(),
                        &contexts[i].res_name,
                        &contexts[i].group,
                    )
                })
                .collect();
            let side2: Vec<usize> = (0..contexts.len())
                .filter(|&j| {
                    side_matches(
                        target.group2.as_ref(),
                        target.resid2.as_ref(),
                        &contexts[j].res_name,
                        &contexts[j].group,
                    )
                })
                .collect();
            for &i in &side1 {
                for &j in &side2 {
                    if i == j {
                        continue;
                    }
                    for bond in &link.bonds {
                        let lo = (bond.value - LINK_DIST_TOL).max(0.4);
                        let hi = bond.value + LINK_DIST_TOL;
                        let (m1, m2) = (contexts[i].map.clone(), contexts[j].map.clone());
                        bond_named(
                            mol,
                            &m1,
                            &bond.a1,
                            &m2,
                            &bond.a2,
                            bond.order,
                            Some((lo, hi)),
                        );
                    }
                }
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
        // prot(21) + nucl(5) + sugar(6) + mono(12) = 44 residues.
        assert_eq!(db.residue_count(), 44);
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
        assert_eq!(ParamDB::global().atom_count(), 194);
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
        // A hydrogen-free atom is unchanged (nothing to absorb).
        let lone = mk(5, "CA", "C");
        let mut mol2 = MolCoord::new();
        mol2.push_atom(lone);
        assert!((united_atom_partial_charge(&mol2, 0).unwrap() - 0.058).abs() < 1e-9);
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
}
