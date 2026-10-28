use crate::selection::Selector;
use crate::SolidColoring;
use cuemol_core::{ByteArray, CueMolError, Matrix3, Matrix4, Quat, Result, Vector4};
use cuemol_gfx::SolidColor;
use cuemol_qsys::View;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct Atom {
    pub serial: i32,
    pub name: String,
    pub alt_loc: Option<char>,
    pub res_name: String,
    pub chain_id: String,
    pub res_id: i32,
    pub ins_code: Option<char>,
    pub position: Vector4,
    pub occupancy: f32,
    pub b_factor: f32,
    pub element: String,
    pub formal_charge: Option<i32>,
    pub int_props: BTreeMap<String, i32>,
    pub real_props: BTreeMap<String, f64>,
}

impl Atom {
    pub fn format_msg(&self) -> String {
        let mut residue_id = self.res_id.to_string();
        if let Some(ins_code) = self.ins_code {
            residue_id.push(ins_code);
        }
        let mut message = format!(
            "{} {} {} {}",
            self.chain_id, self.res_name, residue_id, self.name
        );
        if let Some(alt_loc) = self.alt_loc {
            message.push(':');
            message.push(alt_loc);
        }
        message
    }

    pub fn matches_name_case_insensitive(&self, value: &str) -> bool {
        self.name.trim().eq_ignore_ascii_case(value)
    }

    pub fn matches_chain_case_insensitive(&self, value: &str) -> bool {
        self.chain_id.eq_ignore_ascii_case(value)
    }

    pub fn matches_element_case_insensitive(&self, value: &str) -> bool {
        self.element.eq_ignore_ascii_case(value)
    }

    pub fn matches_residue_name_case_insensitive(&self, value: &str) -> bool {
        self.res_name.eq_ignore_ascii_case(value)
    }

    pub fn int_prop(&self, name: &str) -> Option<i32> {
        self.int_props.get(name).copied()
    }

    pub fn set_int_prop(&mut self, name: impl Into<String>, value: i32) {
        let name = name.into();
        self.real_props.remove(&name);
        self.int_props.insert(name, value);
    }

    pub fn real_prop(&self, name: &str) -> Option<f64> {
        self.real_props.get(name).copied()
    }

    pub fn set_real_prop(&mut self, name: impl Into<String>, value: f64) {
        let name = name.into();
        self.int_props.remove(&name);
        self.real_props.insert(name, value);
    }

    pub fn remove_prop(&mut self, name: &str) -> bool {
        self.int_props.remove(name).is_some() | self.real_props.remove(name).is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BondOrder {
    Single,
    Double,
    Triple,
    Aromatic,
    Other(i32),
}

impl BondOrder {
    pub fn from_sdf(value: i32) -> Self {
        match value {
            1 => Self::Single,
            2 => Self::Double,
            3 => Self::Triple,
            4 => Self::Aromatic,
            value => Self::Other(value),
        }
    }

    pub fn sdf_code(self) -> i32 {
        match self {
            Self::Single => 1,
            Self::Double => 2,
            Self::Triple => 3,
            Self::Aromatic => 4,
            Self::Other(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bond {
    pub atom1: usize,
    pub atom2: usize,
    pub order: BondOrder,
    /// MDL/SDF bond stereo flag: 0 = none, 1 = up (wedge, fat end at atom2),
    /// 6 = down (hashed). Preserved for stereochemistry-aware rendering/export.
    pub stereo: i32,
    /// Whether this bond was explicitly DEPOSITED (a PDB LINK / SSBOND record),
    /// as opposed to inferred by the topology/distance builder (C++
    /// `MolBond::isPersist`). The PDB writer emits LINK records only for
    /// persistent, non-disulfide bonds, so topology bonds are not echoed back.
    pub persist: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResidueKey {
    pub chain_id: String,
    pub res_id: i32,
    pub ins_code: Option<char>,
}

impl ResidueKey {
    pub fn new(chain_id: impl Into<String>, res_id: i32, ins_code: Option<char>) -> Self {
        Self {
            chain_id: chain_id.into(),
            res_id,
            ins_code,
        }
    }

    pub fn script_index(&self) -> String {
        format_residue_index(self.res_id, self.ins_code)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResidueSummary {
    pub key: ResidueKey,
    pub res_name: String,
    pub atom_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainSummary {
    pub chain_id: String,
    pub residue_keys: Vec<ResidueKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolCoord {
    atoms: Vec<Atom>,
    bonds: Vec<Bond>,
    residue_props: BTreeMap<ResidueKey, BTreeMap<String, String>>,
    selection: Selector,
    selection_text: String,
    coloring: SolidColoring,
    default_color: SolidColor,
    atoms_moved_generation: usize,
    cell_params: Option<cuemol_core::CellParams>,
}

impl Default for MolCoord {
    fn default() -> Self {
        Self::new()
    }
}

impl MolCoord {
    pub fn new() -> Self {
        Self {
            atoms: Vec::new(),
            bonds: Vec::new(),
            residue_props: BTreeMap::new(),
            selection: Selector::All,
            selection_text: String::new(),
            coloring: SolidColoring::new(),
            default_color: SolidColor::from_rgb(1.0, 1.0, 1.0),
            atoms_moved_generation: 0,
            cell_params: None,
        }
    }

    pub fn with_atoms(atoms: Vec<Atom>) -> Self {
        Self {
            atoms,
            bonds: Vec::new(),
            residue_props: BTreeMap::new(),
            selection: Selector::All,
            selection_text: String::new(),
            coloring: SolidColoring::new(),
            default_color: SolidColor::from_rgb(1.0, 1.0, 1.0),
            atoms_moved_generation: 0,
            cell_params: None,
        }
    }

    pub fn selection(&self) -> &Selector {
        &self.selection
    }

    pub fn get_selection(&self) -> &Selector {
        self.selection()
    }

    pub fn sel(&self) -> &Selector {
        self.selection()
    }

    pub fn selection_text(&self) -> &str {
        &self.selection_text
    }

    pub fn coloring(&self) -> &SolidColoring {
        &self.coloring
    }

    pub fn get_col_schm(&self) -> &SolidColoring {
        self.coloring()
    }

    pub fn set_col_schm(&mut self, value: SolidColoring) {
        self.coloring = value;
    }

    pub fn set_coloring(&mut self, value: SolidColoring) {
        self.set_col_schm(value);
    }

    pub fn default_color(&self) -> &SolidColor {
        &self.default_color
    }

    pub fn get_default_color(&self) -> &SolidColor {
        self.default_color()
    }

    pub fn defaultcolor(&self) -> &SolidColor {
        self.default_color()
    }

    pub fn set_default_color(&mut self, value: SolidColor) {
        self.default_color = value;
    }

    pub fn set_defaultcolor(&mut self, value: SolidColor) {
        self.set_default_color(value);
    }

    pub fn set_selection(&mut self, selection: Selector) {
        self.selection_text = selection.to_string();
        self.selection = selection;
    }

    pub fn set_sel(&mut self, selection: Selector) {
        self.set_selection(selection);
    }

    pub fn set_selection_text(&mut self, value: impl Into<String>) -> Result<()> {
        let value = value.into();
        let selection = if value.is_empty() {
            Selector::All
        } else {
            crate::parse_selector(&value)?
        };
        self.selection = selection;
        self.selection_text = value;
        Ok(())
    }

    /// Crystallographic unit-cell parameters (from PDB CRYST1 or mmCIF
    /// `_cell`/`_symmetry`), if the source file supplied them.
    pub fn cell_params(&self) -> Option<&cuemol_core::CellParams> {
        self.cell_params.as_ref()
    }

    pub fn set_cell_params(&mut self, cell: cuemol_core::CellParams) {
        self.cell_params = Some(cell);
    }

    pub fn atom_by_serial(&self, serial: i32) -> Option<&Atom> {
        self.atoms.iter().find(|atom| atom.serial == serial)
    }

    pub fn get_atom_by_id(&self, serial: i32) -> Option<&Atom> {
        self.atom_by_serial(serial)
    }

    pub fn atom_mut_by_serial(&mut self, serial: i32) -> Option<&mut Atom> {
        self.atoms.iter_mut().find(|atom| atom.serial == serial)
    }

    pub fn atom_by_spec(
        &self,
        chain: &str,
        sindex: &str,
        atom_name: &str,
    ) -> Result<Option<&Atom>> {
        let (res_id, ins_code) = parse_residue_index(sindex)?;
        Ok(self.atoms.iter().find(|atom| {
            atom.chain_id == chain
                && atom.res_id == res_id
                && atom.ins_code == ins_code
                && atom.name.trim().eq_ignore_ascii_case(atom_name)
        }))
    }

    pub fn atom_by_spec_with_alt(
        &self,
        chain: &str,
        sindex: &str,
        atom_name: &str,
        alt_loc: Option<char>,
    ) -> Result<Option<&Atom>> {
        let (res_id, ins_code) = parse_residue_index(sindex)?;
        let mut candidates = self.atoms.iter().filter(|atom| {
            atom.chain_id == chain
                && atom.res_id == res_id
                && atom.ins_code == ins_code
                && atom.name == atom_name
        });
        Ok(match alt_loc {
            Some(alt_loc) => candidates.find(|atom| atom.alt_loc == Some(alt_loc)),
            None => {
                let candidates = candidates.collect::<Vec<_>>();
                candidates
                    .iter()
                    .find(|atom| atom.alt_loc.is_none())
                    .copied()
                    .or_else(|| candidates.first().copied())
            }
        })
    }

    pub fn get_atom(&self, chain: &str, sindex: &str, atom_name: &str) -> Result<Option<&Atom>> {
        self.atom_by_spec(chain, sindex, atom_name)
    }

    pub fn to_str_aid(&self, serial: i32) -> String {
        let Some(atom) = self.atom_by_serial(serial) else {
            return String::new();
        };
        let mut value = format!(
            "{}.{}.{}",
            atom.chain_id,
            format_residue_index(atom.res_id, atom.ins_code),
            atom.name
        );
        if let Some(alt_loc) = atom.alt_loc {
            value.push(':');
            value.push(alt_loc);
        }
        value
    }

    pub fn from_str_aid(&self, value: &str) -> i32 {
        let mut parts = value.split('.');
        let (Some(chain), Some(sindex), Some(atom_spec), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return -1;
        };
        if chain.is_empty() || atom_spec.is_empty() {
            return -1;
        }
        let (atom_name, alt_loc) = match atom_spec.split_once(':') {
            Some((name, alt)) if !name.is_empty() && !alt.is_empty() && !alt.contains(':') => {
                (name, alt.chars().next())
            }
            Some(_) => return -1,
            None => (atom_spec, None),
        };
        self.atom_by_spec_with_alt(chain, sindex, atom_name, alt_loc)
            .ok()
            .flatten()
            .map_or(-1, |atom| atom.serial)
    }

    pub fn residue_atoms(&self, chain: &str, sindex: &str) -> Result<Vec<&Atom>> {
        let (res_id, ins_code) = parse_residue_index(sindex)?;
        Ok(self
            .atoms
            .iter()
            .filter(|atom| {
                atom.chain_id == chain && atom.res_id == res_id && atom.ins_code == ins_code
            })
            .collect())
    }

    pub fn get_residue(&self, chain: &str, sindex: &str) -> Result<Option<ResidueSummary>> {
        let atoms = self.residue_atoms(chain, sindex)?;
        let Some(first) = atoms.first() else {
            return Ok(None);
        };
        let (res_id, ins_code) = parse_residue_index(sindex)?;
        let atom_indices = self
            .atoms
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| {
                (atom.chain_id == chain && atom.res_id == res_id && atom.ins_code == ins_code)
                    .then_some(index)
            })
            .collect();
        Ok(Some(ResidueSummary {
            key: ResidueKey::new(chain, res_id, ins_code),
            res_name: first.res_name.clone(),
            atom_indices,
        }))
    }

    pub fn chain_atoms(&self, chain: &str) -> Vec<&Atom> {
        self.atoms
            .iter()
            .filter(|atom| atom.chain_id == chain)
            .collect()
    }

    pub fn get_chain(&self, chain: &str) -> Option<ChainSummary> {
        self.chain_summaries()
            .into_iter()
            .find(|summary| summary.chain_id == chain)
    }

    pub fn push_atom(&mut self, atom: Atom) {
        self.atoms.push(atom);
    }

    pub fn append_atom1(
        &mut self,
        mut atom: Atom,
        chain: impl Into<String>,
        resid: i32,
        resn: impl Into<String>,
    ) -> i32 {
        let chain = chain.into();
        let resn = resn.into();
        if atom.serial <= 0 || self.atom_by_serial(atom.serial).is_some() {
            atom.serial = self.next_serial();
        }
        atom.chain_id = chain.clone();
        atom.res_id = resid;
        if resn.is_empty() {
            if let Some(existing) = self
                .atoms
                .iter()
                .find(|existing| existing.chain_id == chain && existing.res_id == resid)
            {
                atom.res_name = existing.res_name.clone();
            }
        } else {
            atom.res_name = resn;
        }
        if atom.chain_id.is_empty()
            || atom.name.is_empty()
            || self.atoms.iter().any(|existing| {
                existing.chain_id == atom.chain_id
                    && existing.res_id == atom.res_id
                    && existing.ins_code == atom.ins_code
                    && existing.name == atom.name
                    && existing.alt_loc == atom.alt_loc
            })
        {
            return -1;
        }
        let serial = atom.serial;
        self.push_atom(atom);
        serial
    }

    pub fn append_atom_1(
        &mut self,
        atom: Atom,
        chain: impl Into<String>,
        resid: i32,
        resn: impl Into<String>,
    ) -> i32 {
        self.append_atom1(atom, chain, resid, resn)
    }

    pub fn atoms(&self) -> &[Atom] {
        &self.atoms
    }

    pub fn atoms_mut(&mut self) -> &mut [Atom] {
        &mut self.atoms
    }

    pub fn add_bond(&mut self, atom1: usize, atom2: usize, order: BondOrder) {
        self.add_bond_stereo(atom1, atom2, order, 0);
    }

    pub fn make_bond(&mut self, serial1: i32, serial2: i32, persist: bool) -> bool {
        let Some(atom1) = self.atoms.iter().position(|atom| atom.serial == serial1) else {
            return false;
        };
        let Some(atom2) = self.atoms.iter().position(|atom| atom.serial == serial2) else {
            return false;
        };
        if self.bonds.iter().any(|bond| {
            (bond.atom1 == atom1 && bond.atom2 == atom2)
                || (bond.atom1 == atom2 && bond.atom2 == atom1)
        }) {
            if persist {
                self.mark_bond_persistent(atom1, atom2);
            }
            return true;
        }
        self.add_bond(atom1, atom2, BondOrder::Single);
        if persist {
            self.mark_bond_persistent(atom1, atom2);
        }
        true
    }

    pub fn remove_bond(&mut self, serial1: i32, serial2: i32) -> bool {
        let Some(atom1) = self.atoms.iter().position(|atom| atom.serial == serial1) else {
            return false;
        };
        let Some(atom2) = self.atoms.iter().position(|atom| atom.serial == serial2) else {
            return false;
        };
        self.remove_bond_between(atom1, atom2)
    }

    /// Add a bond carrying an MDL/SDF stereo flag (0 none / 1 up-wedge / 6 down).
    /// Deduplicates against an existing bond between the same atoms.
    pub fn add_bond_stereo(&mut self, atom1: usize, atom2: usize, order: BondOrder, stereo: i32) {
        if atom1 != atom2 && atom1 < self.atoms.len() && atom2 < self.atoms.len() {
            if self.bonds.iter().any(|bond| {
                (bond.atom1 == atom1 && bond.atom2 == atom2)
                    || (bond.atom1 == atom2 && bond.atom2 == atom1)
            }) {
                return;
            }
            self.bonds.push(Bond {
                atom1,
                atom2,
                order,
                stereo,
                persist: false,
            });
        }
    }

    /// Mark the bond between `atom1` and `atom2` (either direction) as a
    /// deposited/persistent bond (C++ `MolBond::setPersist`). Used by the PDB
    /// LINK-record reader so the writer can echo those links back. Returns
    /// whether a matching bond was found.
    pub fn mark_bond_persistent(&mut self, atom1: usize, atom2: usize) -> bool {
        for bond in &mut self.bonds {
            if (bond.atom1 == atom1 && bond.atom2 == atom2)
                || (bond.atom1 == atom2 && bond.atom2 == atom1)
            {
                bond.persist = true;
                return true;
            }
        }
        false
    }

    /// Remove the bond between two atom indices without rebuilding the
    /// molecule, preserving residue properties, selection, coloring, and cell
    /// metadata.
    pub fn remove_bond_between(&mut self, atom1: usize, atom2: usize) -> bool {
        let before = self.bonds.len();
        self.bonds.retain(|bond| {
            !((bond.atom1 == atom1 && bond.atom2 == atom2)
                || (bond.atom1 == atom2 && bond.atom2 == atom1))
        });
        self.bonds.len() != before
    }

    pub fn remove_nonpersistent_bonds(&mut self) -> usize {
        let before = self.bonds.len();
        self.bonds.retain(|bond| bond.persist);
        before - self.bonds.len()
    }

    pub fn bonds(&self) -> &[Bond] {
        &self.bonds
    }

    pub fn bond_count(&self) -> usize {
        self.bonds.len()
    }

    pub fn get_bond_size(&self) -> usize {
        self.bond_count()
    }

    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    pub fn get_atom_size(&self) -> usize {
        self.atom_count()
    }

    pub fn atom_sel_count(&self, selector: &Selector) -> usize {
        self.select_indices(selector).len()
    }

    pub fn get_atom_sel_size(&self, selector: &Selector) -> usize {
        self.atom_sel_count(selector)
    }

    pub fn chain_ids(&self) -> Vec<String> {
        self.atoms
            .iter()
            .map(|atom| atom.chain_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn chain_count(&self) -> usize {
        self.chain_ids().len()
    }

    pub fn get_chain_size(&self) -> usize {
        self.chain_count()
    }

    pub fn residue_count(&self) -> usize {
        self.atoms
            .iter()
            .map(|atom| (atom.chain_id.clone(), atom.res_id, atom.ins_code))
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// Returns residue summaries ordered by [`ResidueKey`].
    pub fn residue_summaries(&self) -> Vec<ResidueSummary> {
        let mut residues = BTreeMap::<ResidueKey, ResidueSummary>::new();
        for (index, atom) in self.atoms.iter().enumerate() {
            let key = ResidueKey::new(atom.chain_id.clone(), atom.res_id, atom.ins_code);
            residues
                .entry(key.clone())
                .and_modify(|summary| summary.atom_indices.push(index))
                .or_insert_with(|| ResidueSummary {
                    key,
                    res_name: atom.res_name.clone(),
                    atom_indices: vec![index],
                });
        }
        residues.into_values().collect()
    }

    pub fn chain_summaries(&self) -> Vec<ChainSummary> {
        let mut chains = BTreeMap::<String, BTreeSet<ResidueKey>>::new();
        for atom in &self.atoms {
            chains
                .entry(atom.chain_id.clone())
                .or_default()
                .insert(ResidueKey::new(
                    atom.chain_id.clone(),
                    atom.res_id,
                    atom.ins_code,
                ));
        }
        chains
            .into_iter()
            .map(|(chain_id, residues)| ChainSummary {
                chain_id,
                residue_keys: residues.into_iter().collect(),
            })
            .collect()
    }

    pub fn chains_json(&self) -> String {
        string_array_json(&self.chain_ids())
    }

    pub fn get_chains_json(&self) -> String {
        self.chains_json()
    }

    pub fn select_indices(&self, selector: &Selector) -> Vec<usize> {
        self.atoms
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| selector.matches_in_molecule(atom, self).then_some(index))
            .collect()
    }

    pub fn set_residue_prop_str(
        &mut self,
        chain: &str,
        sindex: &str,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<()> {
        let (res_id, ins_code) = parse_residue_index(sindex)?;
        let key = ResidueKey::new(chain, res_id, ins_code);
        if !self.atoms.iter().any(|atom| residue_key(atom) == key) {
            return Err(CueMolError::NotFound(format!(
                "residue {} {}",
                chain, sindex
            )));
        }
        self.residue_props
            .entry(key)
            .or_default()
            .insert(name.into(), value.into());
        Ok(())
    }

    pub(crate) fn replace_secondary_structure(
        &mut self,
        assignments: impl IntoIterator<Item = (ResidueKey, Option<&'static str>, String)>,
    ) {
        self.residue_props.retain(|_, props| {
            props.remove("secondary");
            props.remove("secondary2");
            !props.is_empty()
        });
        for (key, secondary, marker) in assignments {
            let props = self.residue_props.entry(key).or_default();
            if let Some(secondary) = secondary {
                props.insert("secondary".into(), secondary.into());
            }
            props.insert("secondary2".into(), marker);
        }
    }

    pub fn get_residue_prop_str(
        &self,
        chain: &str,
        sindex: &str,
        name: &str,
    ) -> Result<Option<&str>> {
        let (res_id, ins_code) = parse_residue_index(sindex)?;
        Ok(self
            .residue_props
            .get(&ResidueKey::new(chain, res_id, ins_code))
            .and_then(|props| props.get(name))
            .map(String::as_str))
    }

    pub fn remove_residue_prop_str(
        &mut self,
        chain: &str,
        sindex: &str,
        name: &str,
    ) -> Result<bool> {
        let (res_id, ins_code) = parse_residue_index(sindex)?;
        let key = ResidueKey::new(chain, res_id, ins_code);
        let Some(props) = self.residue_props.get_mut(&key) else {
            return Ok(false);
        };
        let removed = props.remove(name).is_some();
        if props.is_empty() {
            self.residue_props.remove(&key);
        }
        Ok(removed)
    }

    pub fn residue_prop_for_atom(&self, atom: &Atom, name: &str) -> Option<&str> {
        self.residue_props
            .get(&residue_key(atom))
            .and_then(|props| props.get(name))
            .map(String::as_str)
    }

    /// Relabel residues by a per-atom destination-key function: `dest` returns
    /// `Some(new_key)` for an atom that moves to a new (chain, resid, ins) residue,
    /// or `None` to keep its current residue. Returns `false` WITHOUT mutating if
    /// the relabel would MERGE two distinct residues — two atoms with different
    /// ORIGINAL residue keys mapping to the same destination key — mirroring C++
    /// `MolAnlManager::changeChainName`/`shiftResIndex`, which abort (throw) rather
    /// than silently collapse two residues into one (the flat `MolCoord` groups by
    /// `(chain,resid,ins)`, so an unchecked collision silently merges them).
    ///
    /// On success it rewrites each moved atom's chain/resid/ins and moves the
    /// residue properties of any residue FULLY vacated to a single new key (a split
    /// residue, whose old key still holds atoms, keeps its props) — the C++
    /// "the same `MolResidue` object moves, carrying its props" behaviour.
    pub fn relabel_residue_keys(&mut self, dest: impl Fn(&Atom) -> Option<ResidueKey>) -> bool {
        // Destination key per atom (its current key when the atom does not move).
        let dests: Vec<ResidueKey> = self
            .atoms
            .iter()
            .map(|atom| dest(atom).unwrap_or_else(|| residue_key(atom)))
            .collect();
        // Collision guard: a destination key must not receive atoms from two
        // different original residues.
        let mut origin: BTreeMap<&ResidueKey, ResidueKey> = BTreeMap::new();
        for (atom, new_key) in self.atoms.iter().zip(&dests) {
            let old_key = residue_key(atom);
            match origin.get(new_key) {
                Some(existing) if *existing != old_key => return false,
                _ => {
                    origin.insert(new_key, old_key);
                }
            }
        }
        // Residues fully vacated to a single new key -> move their props.
        let mut orig_dests: BTreeMap<ResidueKey, BTreeSet<ResidueKey>> = BTreeMap::new();
        for (atom, new_key) in self.atoms.iter().zip(&dests) {
            orig_dests
                .entry(residue_key(atom))
                .or_default()
                .insert(new_key.clone());
        }
        let prop_moves: Vec<(ResidueKey, ResidueKey)> = orig_dests
            .iter()
            .filter_map(|(old, news)| match news.iter().next() {
                Some(new) if news.len() == 1 && new != old => Some((old.clone(), new.clone())),
                _ => None,
            })
            .collect();
        // Apply atom mutations.
        for (atom, new_key) in self.atoms.iter_mut().zip(&dests) {
            atom.chain_id = new_key.chain_id.clone();
            atom.res_id = new_key.res_id;
            atom.ins_code = new_key.ins_code;
        }
        // Re-key props: pull from every old key first, then insert at the new keys,
        // so a lockstep shift (1->2, 2->3) does not clobber a not-yet-moved entry.
        let mut moved = Vec::new();
        for (old, new) in &prop_moves {
            if let Some(props) = self.residue_props.remove(old) {
                moved.push((new.clone(), props));
            }
        }
        for (new, props) in moved {
            self.residue_props.insert(new, props);
        }
        true
    }

    /// All residue string properties, keyed by residue. Used by the QDF molecule
    /// serializer to carry per-residue props (e.g. DSSP `secondary`) through a
    /// save/load round-trip.
    pub fn residue_props(&self) -> &BTreeMap<ResidueKey, BTreeMap<String, String>> {
        &self.residue_props
    }

    /// Replace the entire residue-property table (used to restore a snapshot, e.g.
    /// for undo of a secondary-structure edit).
    pub fn set_all_residue_props(&mut self, props: BTreeMap<ResidueKey, BTreeMap<String, String>>) {
        self.residue_props = props;
    }

    /// Assign protein secondary structure (Kabsch-Sander DSSP) into the
    /// `secondary`/`secondary2` residue properties. Mirrors C++
    /// `MolCoord::calcProt2ndry(hb_high, bIgnoreBulge)`.
    pub fn calc_prot_2ndry(&mut self, hb_high: f64, ignore_bulge: bool) {
        crate::prot2ndry::calc_prot_2ndry(self, hb_high, ignore_bulge);
    }

    /// As [`Self::calc_prot_2ndry`], then merge helices across a short
    /// low-curvature gap. Mirrors C++ `MolCoord::calcProt2ndry2(bIgnoreBulge,
    /// dhangl1)`.
    pub fn calc_prot_2ndry2(&mut self, ignore_bulge: bool, dhangl: f64) {
        crate::prot2ndry::calc_prot_2ndry2(self, ignore_bulge, dhangl);
    }

    pub fn calc_prot2ndry2(&mut self, ignore_bulge: bool, dhangl: f64) {
        self.calc_prot_2ndry2(ignore_bulge, dhangl);
    }

    pub fn center_pos(&self, selector: Option<&Selector>) -> Option<Vector4> {
        let indices = selected_or_all_indices(self, selector);
        if indices.is_empty() {
            return None;
        }
        let mut center = Vector4::ZERO;
        for index in &indices {
            center = center + self.atoms[*index].position;
        }
        Some(center / indices.len() as f64)
    }

    pub fn get_center_pos(&self, selected: bool) -> Option<Vector4> {
        let selector = selected.then_some(&self.selection);
        self.center_pos(selector)
    }

    pub fn bound_box_min(&self, selector: Option<&Selector>) -> Option<Vector4> {
        self.bounding_box_for(selector).map(|(min, _)| min)
    }

    pub fn get_bound_box_min(&self, selected: bool) -> Option<Vector4> {
        let selector = selected.then_some(&self.selection);
        self.bound_box_min(selector)
    }

    pub fn bound_box_max(&self, selector: Option<&Selector>) -> Option<Vector4> {
        self.bounding_box_for(selector).map(|(_, max)| max)
    }

    pub fn get_bound_box_max(&self, selected: bool) -> Option<Vector4> {
        let selector = selected.then_some(&self.selection);
        self.bound_box_max(selector)
    }

    pub fn xform_by_mat(&mut self, matrix: Matrix4, selector: Option<&Selector>) -> usize {
        let indices = selected_or_all_indices(self, selector);
        for index in &indices {
            matrix.xform3d(&mut self.atoms[*index].position);
        }
        indices.len()
    }

    /// Transform atoms and record the before/after coordinates for undo.
    pub fn xform_by_mat_with_undo(
        &mut self,
        matrix: Matrix4,
        selector: Option<&Selector>,
        undo: &mut crate::mol_undo::MolUndoManager,
    ) -> usize {
        let mut edit = crate::mol_undo::MolXformEditInfo::new();
        edit.save_before_pos_selected(self, selector);
        let count = self.xform_by_mat(matrix, selector);
        edit.save_after_pos_selected(self, selector);
        undo.record(Box::new(edit));
        count
    }

    pub fn fit_view(&self, view: &mut View, selected: bool) -> bool {
        let selector = selected.then_some(&self.selection);
        self.fit_view2(view, selector)
    }

    pub fn fit_view2(&self, view: &mut View, selector: Option<&Selector>) -> bool {
        self.fit_view_with_xform(view, selector, Matrix4::IDENTITY)
    }

    pub fn fit_view_with_xform(
        &self,
        view: &mut View,
        selector: Option<&Selector>,
        transform: Matrix4,
    ) -> bool {
        let indices = selected_or_all_indices(self, selector);
        view.fit_positions(indices.into_iter().map(|index| {
            let mut position = self.atoms[index].position;
            transform.xform3d(&mut position);
            position
        }))
    }

    pub fn orient_view(&self, view: &mut View, selector: Option<&Selector>) -> bool {
        self.orient_view_with_xform(view, selector, Matrix4::IDENTITY)
    }

    pub fn orient_view_with_xform(
        &self,
        view: &mut View,
        selector: Option<&Selector>,
        transform: Matrix4,
    ) -> bool {
        let indices = selected_or_all_indices(self, selector);
        if indices.is_empty() {
            return false;
        }
        let position = |index: usize| {
            let mut position = self.atoms[index].position;
            transform.xform3d(&mut position);
            position
        };
        let center = indices
            .iter()
            .fold(Vector4::ZERO, |sum, index| sum + position(*index))
            .scale(1.0 / indices.len() as f64);
        let mut moment = Matrix3::ZERO;
        for index in indices {
            let d = position(index) - center;
            let d = [d.x, d.y, d.z];
            for i in 0..3 {
                for j in 0..3 {
                    moment.values[i][j] += d[i] * d[j];
                }
            }
        }
        let mut vectors = Matrix3::IDENTITY;
        let mut values = Vector4::ZERO;
        if moment.diag(&mut vectors, &mut values) {
            let values = [values.x, values.y, values.z];
            let mut order = [0, 1, 2];
            order.sort_by(|a, b| values[*b].total_cmp(&values[*a]));
            let column = |i| {
                Vector4::new3(
                    vectors.values[0][i],
                    vectors.values[1][i],
                    vectors.values[2][i],
                )
            };
            let first = column(order[0]);
            let second = column(order[1]);
            let third = first.cross(second);
            let rotation = Matrix3::new([
                [first.x, first.y, first.z],
                [second.x, second.y, second.z],
                [third.x, third.y, third.z],
            ]);
            if let Ok(quaternion) = Quat::from_rot_matrix(rotation) {
                view.set_rot_quat(quaternion);
            }
        }
        self.fit_view_with_xform(view, selector, transform)
    }

    pub fn fit_view_2(&self, view: &mut View, selector: Option<&Selector>) -> bool {
        self.fit_view2(view, selector)
    }

    pub fn fire_atoms_moved(&mut self) {
        self.atoms_moved_generation = self.atoms_moved_generation.saturating_add(1);
    }

    pub fn atoms_moved_generation(&self) -> usize {
        self.atoms_moved_generation
    }

    /// Generate connectivity from the residue-topology dictionary.
    ///
    /// Standard residues get their canonical template bonds (with correct bond
    /// orders), consecutive polymer residues are joined by the matching poly link
    /// (peptide / phosphodiester), explicit links (disulfide) are formed between
    /// distance-matching residue pairs, and unknown residues (ligands, solvent)
    /// fall back to per-residue distance autogeneration.  Port of
    /// `TopoBuilder::applyTopology`; see [`crate::toppar`]. Existing
    /// non-persistent bonds are rebuilt, while deposited persistent links remain.
    pub fn apply_topology(&mut self) {
        crate::toppar::build_topology_bonds(self);
    }

    pub fn apply_topology_with_autogen(&mut self, auto_generate: bool) {
        crate::toppar::build_topology_bonds_with_db_options(
            self,
            crate::toppar::TopoDB::global(),
            auto_generate,
        );
    }

    pub fn apply_topology_with_db(&mut self, db: &crate::toppar::TopoDB) {
        crate::toppar::build_topology_bonds_with_db(self, db);
    }

    pub fn apply_topology_with_db_options(
        &mut self,
        db: &crate::toppar::TopoDB,
        auto_generate: bool,
    ) {
        crate::toppar::build_topology_bonds_with_db_options(self, db, auto_generate);
    }

    pub fn calc_base_pair(&mut self, cutoff: f64, tilt: f64) -> usize {
        let cutoff = cutoff.max(0.0);
        let tilt_cos = (90.0_f64 - tilt.clamp(0.0, 90.0)).to_radians().cos();
        let nucleotide_residues = self
            .residue_summaries()
            .into_iter()
            .filter_map(|summary| {
                let base_type = self
                    .get_residue_prop_str(
                        &summary.key.chain_id,
                        &summary.key.script_index(),
                        "basetype",
                    )
                    .ok()
                    .flatten()
                    .map(str::to_string)
                    .or_else(|| {
                        crate::toppar::TopoDB::global()
                            .residue_property(&summary.res_name, "basetype")
                            .map(str::to_string)
                    })?;
                if !matches!(base_type.as_str(), "pur" | "pyr") {
                    return None;
                }
                Some((summary, base_type))
            })
            .collect::<Vec<_>>();

        for (summary, _) in &nucleotide_residues {
            let _ = self.remove_residue_prop_str(
                &summary.key.chain_id,
                &summary.key.script_index(),
                "basepair",
            );
        }

        let residues = nucleotide_residues
            .into_iter()
            .filter_map(|(summary, base_type)| {
                let atom_position = |name: &str| {
                    summary
                        .atom_indices
                        .iter()
                        .find(|&&index| self.atoms[index].matches_name_case_insensitive(name))
                        .map(|&index| self.atoms[index].position)
                };
                let normal_names = if base_type == "pur" {
                    ["C4", "C5", "C8"]
                } else {
                    ["C2", "C4", "C6"]
                };
                let p1 = atom_position(normal_names[0])?;
                let p2 = atom_position(normal_names[1])?;
                let p3 = atom_position(normal_names[2])?;
                let normal = (p2 - p1).cross(p3 - p1).normalize3d().ok()?;
                Some((summary.key, summary.atom_indices.clone(), normal))
            })
            .collect::<Vec<_>>();

        for atom in &mut self.atoms {
            atom.int_props.remove("base_pair");
        }

        const EDGE_NAMES: [&str; 8] = ["N1", "N2", "O6", "N6", "N3", "N4", "O2", "O4"];
        let mut paired = BTreeSet::new();
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for i in 0..residues.len() {
            if paired.contains(&i) {
                continue;
            }
            let mut partner_counts: BTreeMap<usize, usize> = BTreeMap::new();
            for &atom_index in &residues[i].1 {
                let atom = &self.atoms[atom_index];
                if !EDGE_NAMES
                    .iter()
                    .any(|name| atom.matches_name_case_insensitive(name))
                {
                    continue;
                }
                let mut nearest: Option<(usize, f64)> = None;
                for (j, residue) in residues.iter().enumerate() {
                    if i == j || paired.contains(&j) {
                        continue;
                    }
                    for &other_index in &residue.1 {
                        let other = &self.atoms[other_index];
                        if !is_wc_hbond_pair(&atom.name, &other.name) {
                            continue;
                        }
                        let delta = atom.position - other.position;
                        let distance = delta.length3d();
                        if distance <= 1.0e-12 || distance > cutoff {
                            continue;
                        }
                        let plane_ok =
                            |normal: Vector4| (normal.dot3d(delta) / distance).abs() <= tilt_cos;
                        if !plane_ok(residues[i].2) || !plane_ok(residue.2) {
                            continue;
                        }
                        let distance_sq = delta.sqlen3d();
                        if nearest.is_none_or(|(_, best)| distance_sq < best) {
                            nearest = Some((j, distance_sq));
                        }
                    }
                }
                if let Some((j, _)) = nearest {
                    *partner_counts.entry(j).or_default() += 1;
                }
            }

            let partner = partner_counts
                .into_iter()
                .filter(|(_, count)| (2..=3).contains(count))
                .max_by_key(|(_, count)| *count)
                .map(|(j, _)| j);
            if let Some(j) = partner {
                paired.insert(i);
                paired.insert(j);
                pairs.push((i, j));
            }
        }

        for (i, j) in &pairs {
            let lhs = &residues[*i].0;
            let rhs = &residues[*j].0;
            let lhs_resid = lhs.res_id;
            let rhs_resid = rhs.res_id;
            for index in &residues[*i].1 {
                self.atoms[*index].set_int_prop("base_pair", rhs_resid);
            }
            for index in &residues[*j].1 {
                self.atoms[*index].set_int_prop("base_pair", lhs_resid);
            }
            let _ = self.set_residue_prop_str(
                &lhs.chain_id,
                &lhs.script_index(),
                "basepair",
                format!("{}.{}", rhs.chain_id, rhs.script_index()),
            );
            let _ = self.set_residue_prop_str(
                &rhs.chain_id,
                &rhs.script_index(),
                "basepair",
                format!("{}.{}", lhs.chain_id, lhs.script_index()),
            );
        }
        pairs.len()
    }

    pub fn delete_selected_atoms(&mut self, selector: &Selector) -> usize {
        let selected = self
            .select_indices(selector)
            .into_iter()
            .collect::<BTreeSet<_>>();
        self.delete_atom_indices(&selected)
    }

    pub(crate) fn delete_atom_keys(&mut self, keys: &BTreeSet<AtomKey>) -> usize {
        let selected = self
            .atoms
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| keys.contains(&atom_key(atom)).then_some(index))
            .collect::<BTreeSet<_>>();
        self.delete_atom_indices(&selected)
    }

    fn delete_atom_indices(&mut self, selected: &BTreeSet<usize>) -> usize {
        if selected.is_empty() {
            return 0;
        }

        let mut remap = vec![None; self.atoms.len()];
        let mut atoms = Vec::with_capacity(self.atoms.len() - selected.len());
        for (old_index, atom) in self.atoms.iter().enumerate() {
            if selected.contains(&old_index) {
                continue;
            }
            remap[old_index] = Some(atoms.len());
            atoms.push(atom.clone());
        }

        let bonds = self
            .bonds
            .iter()
            .filter_map(|bond| {
                let atom1 = remap.get(bond.atom1).copied().flatten()?;
                let atom2 = remap.get(bond.atom2).copied().flatten()?;
                Some(Bond {
                    atom1,
                    atom2,
                    order: bond.order,
                    stereo: bond.stereo,
                    persist: bond.persist,
                })
            })
            .collect();

        self.atoms = atoms;
        self.bonds = bonds;
        let live_residues = self.atoms.iter().map(residue_key).collect::<BTreeSet<_>>();
        self.residue_props
            .retain(|key, _| live_residues.contains(key));
        selected.len()
    }

    pub fn remove_atom(&mut self, serial: i32) -> bool {
        let Some(index) = self.atoms.iter().position(|atom| atom.serial == serial) else {
            return false;
        };
        self.delete_atom_indices(&BTreeSet::from([index])) == 1
    }

    pub fn remove_chain(&mut self, chain: &str) -> bool {
        let selected = self
            .atoms
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| (atom.chain_id == chain).then_some(index))
            .collect::<BTreeSet<_>>();
        if selected.is_empty() {
            return false;
        }
        self.delete_atom_indices(&selected);
        true
    }

    pub fn change_chain_name(
        &mut self,
        old_chain: &str,
        sindex: &str,
        new_chain: &str,
    ) -> Result<bool> {
        let (res_id, ins_code) = parse_residue_index(sindex)?;
        let old_key = ResidueKey::new(old_chain, res_id, ins_code);
        let new_key = ResidueKey::new(new_chain, res_id, ins_code);
        if !self.atoms.iter().any(|atom| residue_key(atom) == old_key) {
            return Ok(false);
        }
        if old_key != new_key && self.atoms.iter().any(|atom| residue_key(atom) == new_key) {
            return Ok(false);
        }
        for atom in &mut self.atoms {
            if residue_key(atom) == old_key {
                atom.chain_id = new_chain.to_string();
            }
        }
        if let Some(props) = self.residue_props.remove(&old_key) {
            self.residue_props.insert(new_key, props);
        }
        Ok(true)
    }

    pub fn change_res_index(
        &mut self,
        chain: &str,
        old_sindex: &str,
        new_sindex: &str,
    ) -> Result<bool> {
        let (old_res_id, old_ins_code) = parse_residue_index(old_sindex)?;
        let (new_res_id, new_ins_code) = parse_residue_index(new_sindex)?;
        let old_key = ResidueKey::new(chain, old_res_id, old_ins_code);
        let new_key = ResidueKey::new(chain, new_res_id, new_ins_code);
        if !self.atoms.iter().any(|atom| residue_key(atom) == old_key) {
            return Ok(false);
        }
        if old_key != new_key && self.atoms.iter().any(|atom| residue_key(atom) == new_key) {
            return Ok(false);
        }
        for atom in &mut self.atoms {
            if residue_key(atom) == old_key {
                atom.res_id = new_res_id;
                atom.ins_code = new_ins_code;
            }
        }
        if let Some(props) = self.residue_props.remove(&old_key) {
            self.residue_props.insert(new_key, props);
        }
        Ok(true)
    }

    pub fn delete_atoms(&mut self, selector: &Selector) -> bool {
        self.delete_selected_atoms(selector);
        true
    }

    pub fn copy_selected_atoms_from(
        &mut self,
        source: &MolCoord,
        selector: &Selector,
    ) -> Result<usize> {
        let selected = source
            .select_indices(selector)
            .into_iter()
            .collect::<BTreeSet<_>>();
        if selected.is_empty() {
            return Ok(0);
        }

        let existing_residues = self.atoms.iter().map(residue_key).collect::<BTreeSet<_>>();
        let existing_atoms = self.atoms.iter().map(atom_key).collect::<BTreeSet<_>>();
        for index in &selected {
            let atom = &source.atoms[*index];
            if existing_residues.contains(&residue_key(atom)) {
                return Err(CueMolError::invalid_argument(format!(
                    "residue collision while copying atoms: {} {}",
                    atom.chain_id, atom.res_id
                )));
            }
            if existing_atoms.contains(&atom_key(atom)) {
                return Err(CueMolError::invalid_argument(format!(
                    "atom collision while copying atoms: {} {} {}",
                    atom.chain_id, atom.res_id, atom.name
                )));
            }
        }

        let mut remap = BTreeMap::new();
        for index in &selected {
            let new_index = self.atoms.len();
            self.atoms.push(source.atoms[*index].clone());
            remap.insert(*index, new_index);
        }
        for bond in &source.bonds {
            if let (Some(atom1), Some(atom2)) = (remap.get(&bond.atom1), remap.get(&bond.atom2)) {
                self.add_bond_stereo(*atom1, *atom2, bond.order, bond.stereo);
                if bond.persist {
                    self.mark_bond_persistent(*atom1, *atom2);
                }
            }
        }
        for index in &selected {
            let key = residue_key(&source.atoms[*index]);
            if let Some(props) = source.residue_props.get(&key) {
                self.residue_props
                    .entry(key)
                    .or_default()
                    .extend(props.clone());
            }
        }

        Ok(selected.len())
    }

    pub fn copy_atoms(&mut self, source: &MolCoord, selector: &Selector) -> bool {
        self.copy_selected_atoms_from(source, selector).is_ok()
    }

    pub fn crd_array(&self) -> ByteArray {
        let values = self
            .atoms
            .iter()
            .flat_map(|atom| {
                [
                    atom.position.x as f32,
                    atom.position.y as f32,
                    atom.position.z as f32,
                ]
            })
            .collect::<Vec<_>>();
        ByteArray::copy_from_f32(&values)
    }

    pub fn get_crd_array(&self) -> ByteArray {
        self.crd_array()
    }

    pub fn prop_array(&self, propname: &str) -> ByteArray {
        let values = self
            .atoms
            .iter()
            .map(|atom| match propname {
                "bfac" => atom.b_factor,
                "occ" => atom.occupancy,
                _ => 0.0,
            })
            .collect::<Vec<_>>();
        ByteArray::copy_from_f32(&values)
    }

    pub fn get_prop_array(&self, propname: &str) -> ByteArray {
        self.prop_array(propname)
    }

    pub fn sel_array(&self, selector: &Selector) -> ByteArray {
        let values = self
            .select_indices(selector)
            .into_iter()
            .map(|index| index as i32)
            .collect::<Vec<_>>();
        ByteArray::copy_from_i32(&values)
    }

    pub fn get_sel_array(&self, selector: &Selector) -> ByteArray {
        self.sel_array(selector)
    }

    pub fn chain_name_candidates(&self) -> Vec<String> {
        self.chain_ids()
    }

    pub fn residue_name_candidates(&self) -> Vec<String> {
        self.atoms
            .iter()
            .map(|atom| atom.res_name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn atom_name_candidates(&self) -> Vec<String> {
        self.atoms
            .iter()
            .map(|atom| atom.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn element_name_candidates(&self) -> Vec<String> {
        self.atoms
            .iter()
            .map(|atom| atom.element.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn chain_name_candidates_json(&self) -> String {
        string_array_json(&self.chain_name_candidates())
    }

    pub fn get_chain_name_cands_json(&self) -> String {
        self.chain_name_candidates_json()
    }

    pub fn residue_name_candidates_json(&self) -> String {
        string_array_json(&self.residue_name_candidates())
    }

    pub fn get_resid_name_cands_json(&self) -> String {
        self.residue_name_candidates_json()
    }

    pub fn atom_name_candidates_json(&self) -> String {
        string_array_json(&self.atom_name_candidates())
    }

    pub fn get_atom_name_cands_json(&self) -> String {
        self.atom_name_candidates_json()
    }

    pub fn element_name_candidates_json(&self) -> String {
        string_array_json(&self.element_name_candidates())
    }

    pub fn get_elem_name_cands_json(&self) -> String {
        self.element_name_candidates_json()
    }

    pub fn get_resids_json(&self) -> String {
        let entries = self
            .residue_summaries()
            .into_iter()
            .map(|residue| {
                format!(
                    "{{\"chain\":\"{}\",\"sindex\":\"{}\",\"name\":\"{}\"}}",
                    escape_json(&residue.key.chain_id),
                    escape_json(&residue.key.script_index()),
                    escape_json(&residue.res_name)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub fn get_atoms_json(&self) -> String {
        let entries = self
            .atoms
            .iter()
            .map(|atom| {
                format!(
                    "{{\"id\":{},\"name\":\"{}\",\"chain\":\"{}\",\"resid\":{},\"elem\":\"{}\"}}",
                    atom.serial,
                    escape_json(&atom.name),
                    escape_json(&atom.chain_id),
                    atom.res_id,
                    escape_json(&atom.element)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub fn get_atom_prop_int(&self, serial: i32, name: &str) -> Option<i32> {
        self.atom_by_serial(serial)?.int_prop(name)
    }

    pub fn get_atom_prop_real(&self, serial: i32, name: &str) -> Option<f64> {
        let atom = self.atom_by_serial(serial)?;
        match name {
            "bfac" => Some(f64::from(atom.b_factor)),
            "occ" => Some(f64::from(atom.occupancy)),
            _ => None,
        }
    }

    pub fn get_atom_alt_conf(&self, serial: i32) -> Option<char> {
        self.atom_by_serial(serial)?.alt_loc
    }

    pub fn is_selected(&self, serial: i32, selector: &Selector) -> bool {
        self.atom_by_serial(serial)
            .is_some_and(|atom| selector.matches(atom))
    }

    pub fn is_selected_mol(&self, selector: &Selector) -> bool {
        self.atoms.iter().any(|atom| selector.matches(atom))
    }

    pub fn is_selected_chain(&self, chain: &str, selector: &Selector) -> bool {
        self.atoms
            .iter()
            .any(|atom| atom.chain_id == chain && selector.matches(atom))
    }

    pub fn is_selected_resid(
        &self,
        chain: &str,
        sindex: &str,
        selector: &Selector,
    ) -> Result<bool> {
        let (res_id, ins_code) = parse_residue_index(sindex)?;
        Ok(self.atoms.iter().any(|atom| {
            atom.chain_id == chain
                && atom.res_id == res_id
                && atom.ins_code == ins_code
                && selector.matches(atom)
        }))
    }

    pub fn bounding_box(&self) -> Option<(Vector4, Vector4)> {
        let first = self.atoms.first()?;
        let mut min = first.position;
        let mut max = first.position;
        for atom in &self.atoms[1..] {
            let pos = atom.position;
            min.x = min.x.min(pos.x);
            min.y = min.y.min(pos.y);
            min.z = min.z.min(pos.z);
            max.x = max.x.max(pos.x);
            max.y = max.y.max(pos.y);
            max.z = max.z.max(pos.z);
        }
        Some((min, max))
    }

    pub fn bounding_box_for(&self, selector: Option<&Selector>) -> Option<(Vector4, Vector4)> {
        let indices = selected_or_all_indices(self, selector);
        let first = *indices.first()?;
        let mut min = self.atoms[first].position;
        let mut max = self.atoms[first].position;
        for index in indices.into_iter().skip(1) {
            let pos = self.atoms[index].position;
            min.x = min.x.min(pos.x);
            min.y = min.y.min(pos.y);
            min.z = min.z.min(pos.z);
            max.x = max.x.max(pos.x);
            max.y = max.y.max(pos.y);
            max.z = max.z.max(pos.z);
        }
        Some((min, max))
    }

    pub fn encode_model_in_chain(chain_name: &str, model: i32) -> Result<String> {
        if model <= 0 {
            return Err(CueMolError::invalid_argument("invalid model number"));
        }
        if model == 1 {
            Ok(chain_name.to_string())
        } else {
            Ok(format!("{model:02}_{chain_name}"))
        }
    }

    pub fn decode_model_from_chain(value: &str) -> Option<(String, i32)> {
        let (model, chain) = value.split_once('_')?;
        if chain.is_empty() {
            return None;
        }
        Some((chain.to_string(), model.parse::<i32>().ok()?))
    }

    fn next_serial(&self) -> i32 {
        self.atoms.iter().map(|atom| atom.serial).max().unwrap_or(0) + 1
    }
}

fn string_array_json(values: &[String]) -> String {
    let entries = values
        .iter()
        .map(|value| format!("\"{}\"", escape_json(value)))
        .collect::<Vec<_>>();
    format!("[{}]", entries.join(","))
}

fn escape_json(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn selected_or_all_indices(molecule: &MolCoord, selector: Option<&Selector>) -> Vec<usize> {
    match selector {
        Some(selector) => molecule.select_indices(selector),
        None => (0..molecule.atoms.len()).collect(),
    }
}

fn parse_residue_index(value: &str) -> Result<(i32, Option<char>)> {
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
    Ok((number, suffix))
}

fn format_residue_index(number: i32, ins_code: Option<char>) -> String {
    match ins_code {
        Some(ins_code) => format!("{number}{ins_code}"),
        None => number.to_string(),
    }
}

fn residue_key(atom: &Atom) -> ResidueKey {
    ResidueKey::new(atom.chain_id.clone(), atom.res_id, atom.ins_code)
}

pub(crate) type AtomKey = (String, i32, Option<char>, String, Option<char>);

pub(crate) fn atom_key(atom: &Atom) -> AtomKey {
    (
        atom.chain_id.clone(),
        atom.res_id,
        atom.ins_code,
        atom.name.clone(),
        atom.alt_loc,
    )
}

fn is_wc_hbond_pair(lhs: &str, rhs: &str) -> bool {
    let lhs = lhs.trim().to_ascii_uppercase();
    let rhs = rhs.trim().to_ascii_uppercase();
    let pair = |a: &str, b: &str| (lhs == a && rhs == b) || (lhs == b && rhs == a);
    pair("N4", "O6")
        || pair("N3", "N1")
        || pair("O2", "N2")
        || pair("O4", "N6")
        || pair("N3", "O6")
        || pair("O2", "N1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_selector;

    fn atom(serial: i32, chain: &str, resid: i32, name: &str, x: f64) -> Atom {
        Atom {
            serial,
            name: name.to_string(),
            alt_loc: None,
            res_name: "ALA".to_string(),
            chain_id: chain.to_string(),
            res_id: resid,
            ins_code: None,
            position: Vector4::new3(x, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "C".to_string(),
            formal_charge: None,
            int_props: BTreeMap::new(),
            real_props: BTreeMap::new(),
        }
    }

    #[test]
    fn atom_format_msg_matches_cuemol_hit_text() {
        let mut atom = atom(1, "A", 10, "CA", 0.0);
        atom.ins_code = Some('B');
        atom.alt_loc = Some('C');
        assert_eq!(atom.format_msg(), "A ALA 10B CA:C");
    }

    #[test]
    fn molcoord_qif_accessors_and_edit_helpers_work() {
        let mut mol = MolCoord::new();
        assert_eq!(mol.defaultcolor(), mol.default_color());
        let default_color = SolidColor::from_rgb(0.25, 0.5, 0.75);
        mol.set_defaultcolor(default_color.clone());
        assert_eq!(mol.default_color(), &default_color);
        mol.set_sel(parse_selector("chain A").unwrap());
        assert_eq!(mol.sel(), &parse_selector("chain A").unwrap());
        mol.set_coloring(SolidColoring::new());
        assert_eq!(mol.coloring().type_name(), "SolidColoring");
        mol.push_atom(atom(1, "A", 1, "N", 0.0));
        mol.push_atom(atom(2, "A", 1, "CA", 2.0));
        mol.push_atom(atom(3, "B", 2, "CA", 4.0));

        assert_eq!(mol.atom_by_serial(2).unwrap().name, "CA");
        assert_eq!(mol.atom_by_spec("A", "1", "CA").unwrap().unwrap().serial, 2);
        assert_eq!(mol.residue_atoms("A", "1").unwrap().len(), 2);
        assert_eq!(mol.chain_atoms("B").len(), 1);
        assert_eq!(mol.atom_sel_count(&parse_selector("name CA").unwrap()), 2);
        assert_eq!(mol.chains_json(), "[\"A\",\"B\"]");
        assert_eq!(mol.residue_summaries().len(), 2);
        assert_eq!(mol.chain_summaries().len(), 2);
        assert_eq!(mol.center_pos(None).unwrap(), Vector4::new3(2.0, 0.0, 0.0));
        assert_eq!(
            mol.center_pos(Some(&parse_selector("chain A").unwrap()))
                .unwrap(),
            Vector4::new3(1.0, 0.0, 0.0)
        );

        let mut matrix = Matrix4::IDENTITY;
        matrix.set_at(1, 4, 10.0).unwrap();
        assert_eq!(
            mol.xform_by_mat(matrix, Some(&parse_selector("chain B").unwrap())),
            1
        );
        assert_eq!(mol.atom_by_serial(3).unwrap().position.x, 14.0);

        let serial = mol.append_atom1(atom(0, "", 0, "C", 5.0), "A", 1, "");
        assert_eq!(serial, 4);
        assert_eq!(mol.atom_by_serial(4).unwrap().res_name, "ALA");
        assert_eq!(mol.append_atom1(atom(0, "", 0, "C", 6.0), "A", 1, ""), -1);
        assert_eq!(mol.atom_count(), 4);
        assert_eq!(MolCoord::encode_model_in_chain("A", 1).unwrap(), "A");
        assert_eq!(MolCoord::encode_model_in_chain("A", 12).unwrap(), "12_A");
        assert_eq!(
            MolCoord::decode_model_from_chain("12_A"),
            Some(("A".to_string(), 12))
        );
        assert_eq!(mol.atoms_moved_generation(), 0);
        mol.fire_atoms_moved();
        assert_eq!(mol.atoms_moved_generation(), 1);
    }

    #[test]
    fn persistent_atom_ids_round_trip_with_insertion_and_alt_ids() {
        let mut selected = atom(11, "A", 1, "CA", 0.0);
        selected.ins_code = Some('I');
        selected.alt_loc = Some('B');
        let mol = MolCoord::with_atoms(vec![selected]);

        assert_eq!(mol.to_str_aid(11), "A.1I.CA:B");
        assert_eq!(mol.from_str_aid("A.1I.CA:B"), 11);
        assert_eq!(mol.from_str_aid("A.1I.CA:A"), -1);
        assert_eq!(mol.from_str_aid("invalid"), -1);
        assert_eq!(mol.to_str_aid(999), "");
    }

    #[test]
    fn direct_atom_chain_and_residue_edits_preserve_model_consistency() {
        let mut mol = MolCoord::new();
        mol.push_atom(atom(1, "A", 1, "N", 0.0));
        mol.push_atom(atom(2, "B", 2, "CA", 2.0));
        mol.add_bond(0, 1, BondOrder::Single);
        mol.set_residue_prop_str("A", "1", "secondary", "helix")
            .unwrap();

        assert!(mol.change_chain_name("A", "1", "Z").unwrap());
        assert_eq!(mol.atoms()[0].chain_id, "Z");
        assert_eq!(
            mol.get_residue_prop_str("Z", "1", "secondary").unwrap(),
            Some("helix")
        );
        assert!(mol.change_res_index("Z", "1", "3I").unwrap());
        assert_eq!(
            (mol.atoms()[0].res_id, mol.atoms()[0].ins_code),
            (3, Some('I'))
        );
        assert_eq!(
            mol.get_residue_prop_str("Z", "3I", "secondary").unwrap(),
            Some("helix")
        );

        assert!(mol.remove_atom(1));
        assert_eq!(mol.atom_count(), 1);
        assert_eq!(mol.bond_count(), 0);
        assert_eq!(
            mol.get_residue_prop_str("Z", "3I", "secondary").unwrap(),
            None
        );
        assert!(mol.remove_chain("B"));
        assert!(mol.is_empty());
        assert!(!mol.remove_chain("missing"));
    }

    #[test]
    fn molcoord_fit_view_matches_rotated_bounding_box_contract() {
        let mut first = atom(1, "A", 1, "N", 0.0);
        first.position = Vector4::new3(0.0, 0.0, 0.0);
        let mut second = atom(2, "A", 1, "CA", 30.0);
        second.position = Vector4::new3(30.0, 10.0, 40.0);
        let mut mol = MolCoord::with_atoms(vec![first, second]);
        mol.set_selection(parse_selector("name CA").unwrap());
        assert_eq!(
            mol.get_center_pos(true),
            Some(Vector4::new3(30.0, 10.0, 40.0))
        );

        let mut view = View::new(cuemol_core::Uid::new(1), cuemol_core::Uid::new(2));
        view.set_view_size(200, 100);
        assert!(mol.fit_view(&mut view, false));
        assert_eq!(view.view_center(), Vector4::new3(15.0, 5.0, 20.0));
        assert!((view.zoom() - 21.0).abs() < 1.0e-12);
        assert!((view.slab_depth() - 56.0).abs() < 1.0e-12);
    }

    #[test]
    fn molcoord_residue_properties_drive_rprop_selection() {
        let mut mol = MolCoord::new();
        mol.push_atom(atom(1, "A", 1, "N", 0.0));
        mol.push_atom(atom(2, "A", 1, "CA", 1.0));
        mol.push_atom(atom(3, "A", 2, "CA", 2.0));
        mol.set_residue_prop_str("A", "1", "secondary", "helix")
            .unwrap();

        let selector = parse_selector("rprop secondary=helix").unwrap();
        assert_eq!(mol.select_indices(&selector), vec![0, 1]);
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "secondary").unwrap(),
            Some("helix")
        );

        let mut copied = MolCoord::new();
        copied
            .copy_selected_atoms_from(&mol, &parse_selector("resi 1").unwrap())
            .unwrap();
        assert_eq!(copied.select_indices(&selector), vec![0, 1]);

        assert_eq!(
            mol.delete_selected_atoms(&parse_selector("resi 1").unwrap()),
            2
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "secondary").unwrap(),
            None
        );
    }

    #[test]
    fn molcoord_selection_evaluates_distance_and_unary_ops() {
        let mut mol = MolCoord::new();
        mol.push_atom(atom(1, "A", 1, "N", 0.0));
        mol.push_atom(atom(2, "A", 1, "CA", 2.0));
        mol.push_atom(atom(3, "A", 1, "CB", 2.5));
        mol.push_atom(atom(4, "A", 2, "O", 10.0));
        mol.add_bond(0, 1, BondOrder::Single);
        mol.add_bond(1, 2, BondOrder::Single);

        assert_eq!(
            mol.select_indices(&parse_selector("name N around 3").unwrap()),
            vec![1, 2]
        );
        assert_eq!(
            mol.select_indices(&parse_selector("name N expand 3").unwrap()),
            vec![0, 1, 2]
        );
        assert_eq!(
            mol.select_indices(&parse_selector("byres name CA").unwrap()),
            vec![0, 1, 2]
        );
        assert_eq!(
            mol.select_indices(&parse_selector("bymainch resi 1").unwrap()),
            vec![0, 1]
        );
        assert_eq!(
            mol.select_indices(&parse_selector("bysidech resi 1").unwrap()),
            vec![1, 2]
        );
        assert_eq!(
            mol.select_indices(&parse_selector("name N neighbor 1").unwrap()),
            vec![1]
        );
        assert_eq!(
            mol.select_indices(&parse_selector("name N neighbor 2").unwrap()),
            vec![1, 2]
        );
        assert_eq!(
            mol.select_indices(&parse_selector("name N extend 1").unwrap()),
            vec![0, 1]
        );
    }

    #[test]
    fn molcoord_apply_topology_adds_missing_near_bonds() {
        // A residue with no dictionary template (a ligand) falls back to
        // per-residue distance autogeneration: C1-C2 (1.4 Å) bonds, C3 (5 Å) does
        // not.  Atom names C1/C2/C3 are not real ALA atoms, so the topology path
        // would correctly add nothing — this exercises the autogen fallback.
        let mut mol = MolCoord::new();
        for (serial, name, x) in [(1, "C1", 0.0), (2, "C2", 1.4), (3, "C3", 5.0)] {
            let mut a = atom(serial, "A", 1, name, x);
            a.res_name = "LIG".to_string();
            mol.push_atom(a);
        }

        mol.apply_topology();
        mol.apply_topology();

        assert_eq!(mol.bond_count(), 1);
        assert_eq!(mol.bonds()[0].atom1, 0);
        assert_eq!(mol.bonds()[0].atom2, 1);
    }

    #[test]
    fn molcoord_calc_base_pair_marks_near_nucleotide_residues() {
        let mut mol = MolCoord::new();
        let mut serial = 1;
        let mut push = |chain: &str, resid: i32, res_name: &str, name: &str, x: f64, y: f64| {
            let mut value = atom(serial, chain, resid, name, x);
            serial += 1;
            value.res_name = res_name.to_string();
            value.position.y = y;
            mol.push_atom(value);
        };
        for (name, x, y) in [
            ("C4", 0.0, 0.0),
            ("C5", 1.0, 0.0),
            ("C8", 0.0, 1.0),
            ("N6", 0.0, 0.0),
            ("N1", 0.0, 1.0),
        ] {
            push("A", 1, "A", name, x, y);
        }
        for (name, x, y) in [
            ("C2", 3.0, 0.0),
            ("C4", 4.0, 0.0),
            ("C6", 3.0, 1.0),
            ("O4", 3.0, 0.0),
            ("N3", 3.0, 1.0),
        ] {
            push("B", 2, "T", name, x, y);
        }
        let mut far = atom(serial, "C", 3, "N1", 10.0);
        far.res_name = "G".to_string();
        mol.push_atom(far);

        assert_eq!(mol.calc_base_pair(3.7, 30.0), 1);

        assert_eq!(mol.atoms()[0].int_prop("base_pair"), Some(2));
        assert_eq!(mol.atoms()[5].int_prop("base_pair"), Some(1));
        assert_eq!(mol.atoms()[10].int_prop("base_pair"), None);
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "basepair").unwrap(),
            Some("B.2")
        );
        assert_eq!(
            mol.get_residue_prop_str("B", "2", "basepair").unwrap(),
            Some("A.1")
        );
    }

    #[test]
    fn molcoord_calc_base_pair_clears_stale_property_without_base_normal() {
        let mut mol = MolCoord::new();
        let mut value = atom(1, "A", 1, "N1", 0.0);
        value.res_name = "A".to_string();
        mol.push_atom(value);
        mol.set_residue_prop_str("A", "1", "basepair", "B.2")
            .unwrap();

        assert_eq!(mol.calc_base_pair(3.7, 30.0), 0);
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "basepair").unwrap(),
            None
        );
    }

    #[test]
    fn molcoord_preserves_empty_and_explicit_all_selection_text() {
        let mut mol = MolCoord::new();
        assert_eq!(mol.selection(), &Selector::All);
        assert_eq!(mol.selection_text(), "");

        mol.set_selection_text("*").unwrap();
        assert_eq!(mol.selection(), &Selector::All);
        assert_eq!(mol.selection_text(), "*");

        mol.set_selection_text("").unwrap();
        assert_eq!(mol.selection(), &Selector::All);
        assert_eq!(mol.selection_text(), "");
    }
}
