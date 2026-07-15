use crate::{parse_selector, Atom, MolCoord, ResidueKey, ResidueSummary, Selector};
use cuemol_core::Result;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct AtomIterator<'a> {
    target: &'a MolCoord,
    selection: Selector,
    indices: Vec<usize>,
    cursor: usize,
}

impl<'a> AtomIterator<'a> {
    pub fn new(target: &'a MolCoord, selection: Selector) -> Self {
        let indices = target.select_indices(&selection);
        Self {
            target,
            selection,
            indices,
            cursor: 0,
        }
    }

    pub fn all(target: &'a MolCoord) -> Self {
        Self::new(target, Selector::All)
    }

    pub fn target(&self) -> &'a MolCoord {
        self.target
    }

    pub fn get_target(&self) -> &'a MolCoord {
        self.target()
    }

    pub fn set_target(&mut self, target: &'a MolCoord) {
        *self = Self::new(target, self.selection.clone());
    }

    pub fn selection(&self) -> &Selector {
        &self.selection
    }

    pub fn get_selection(&self) -> &Selector {
        self.selection()
    }

    pub fn set_selection(&mut self, selection: Selector) {
        *self = Self::new(self.target, selection);
    }

    pub fn first(&mut self) {
        self.cursor = 0;
    }

    pub fn next(&mut self) {
        self.cursor = self.cursor.saturating_add(1);
    }

    pub fn has_more(&self) -> bool {
        self.cursor < self.indices.len()
    }

    pub fn get_id(&self) -> Option<i32> {
        self.get().map(|atom| atom.serial)
    }

    pub fn get(&self) -> Option<&'a Atom> {
        self.indices
            .get(self.cursor)
            .and_then(|index| self.target.atoms().get(*index))
    }
}

impl std::fmt::Display for AtomIterator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AtomIterator(size={}, cursor={})",
            self.indices.len(),
            self.cursor
        )
    }
}

#[derive(Debug, Clone)]
pub struct ResidIterator<'a> {
    target: &'a MolCoord,
    selection: Selector,
    residues: Vec<ResidueView<'a>>,
    cursor: usize,
}

impl<'a> ResidIterator<'a> {
    pub fn new(target: &'a MolCoord, selection: Selector) -> Self {
        let selected = target
            .select_indices(&selection)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let residues = target
            .residue_summaries()
            .into_iter()
            .filter(|summary| {
                summary
                    .atom_indices
                    .iter()
                    .any(|index| selected.contains(index))
            })
            .map(|summary| ResidueView { target, summary })
            .collect();
        Self {
            target,
            selection,
            residues,
            cursor: 0,
        }
    }

    pub fn all(target: &'a MolCoord) -> Self {
        Self::new(target, Selector::All)
    }

    pub fn target(&self) -> &'a MolCoord {
        self.target
    }

    pub fn get_target(&self) -> &'a MolCoord {
        self.target()
    }

    pub fn set_target(&mut self, target: &'a MolCoord) {
        *self = Self::new(target, self.selection.clone());
    }

    pub fn selection(&self) -> &Selector {
        &self.selection
    }

    pub fn get_selection(&self) -> &Selector {
        self.selection()
    }

    pub fn set_selection(&mut self, selection: Selector) {
        *self = Self::new(self.target, selection);
    }

    pub fn first(&mut self) {
        self.cursor = 0;
    }

    pub fn next(&mut self) {
        self.cursor = self.cursor.saturating_add(1);
    }

    pub fn has_more(&self) -> bool {
        self.cursor < self.residues.len()
    }

    pub fn get(&self) -> Option<&ResidueView<'a>> {
        self.residues.get(self.cursor)
    }
}

impl std::fmt::Display for ResidIterator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ResidIterator(size={}, cursor={})",
            self.residues.len(),
            self.cursor
        )
    }
}

#[derive(Debug, Clone)]
pub struct ResidueView<'a> {
    target: &'a MolCoord,
    summary: ResidueSummary,
}

impl<'a> ResidueView<'a> {
    pub fn molecule(&self) -> &'a MolCoord {
        self.target
    }

    pub fn key(&self) -> &ResidueKey {
        &self.summary.key
    }

    pub fn name(&self) -> &str {
        &self.summary.res_name
    }

    pub fn chain_name(&self) -> &str {
        &self.summary.key.chain_id
    }

    pub fn sindex(&self) -> String {
        self.summary.key.script_index()
    }

    pub fn atoms(&self) -> Vec<&'a Atom> {
        self.summary
            .atom_indices
            .iter()
            .filter_map(|index| self.target.atoms().get(*index))
            .collect()
    }

    /// The molecule atom indices that make up this residue (used to test a
    /// per-renderer `sel="..."` draw restriction at residue granularity).
    pub fn atom_indices(&self) -> &[usize] {
        &self.summary.atom_indices
    }

    pub fn atoms_json(&self) -> String {
        let body = self
            .atoms()
            .into_iter()
            .map(|atom| atom.serial.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("[{body}]")
    }
}

impl std::fmt::Display for ResidueView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.summary.key.chain_id,
            self.summary.res_name,
            self.summary.key.script_index()
        )
    }
}

#[derive(Debug, Clone)]
pub struct ChainIterator<'a> {
    target: &'a MolCoord,
    selection: Selector,
    chains: Vec<ChainView<'a>>,
    cursor: usize,
}

impl<'a> ChainIterator<'a> {
    pub fn new(target: &'a MolCoord, selection: Selector) -> Self {
        let selected = target
            .select_indices(&selection)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let chains = target
            .chain_summaries()
            .into_iter()
            .filter(|summary| {
                target.atoms().iter().enumerate().any(|(index, atom)| {
                    selected.contains(&index) && atom.chain_id == summary.chain_id
                })
            })
            .map(|summary| ChainView {
                target,
                chain_id: summary.chain_id,
            })
            .collect();
        Self {
            target,
            selection,
            chains,
            cursor: 0,
        }
    }

    pub fn all(target: &'a MolCoord) -> Self {
        Self::new(target, Selector::All)
    }

    pub fn target(&self) -> &'a MolCoord {
        self.target
    }

    pub fn get_target(&self) -> &'a MolCoord {
        self.target()
    }

    pub fn set_target(&mut self, target: &'a MolCoord) {
        *self = Self::new(target, self.selection.clone());
    }

    pub fn selection(&self) -> &Selector {
        &self.selection
    }

    pub fn get_selection(&self) -> &Selector {
        self.selection()
    }

    pub fn set_selection(&mut self, selection: Selector) {
        *self = Self::new(self.target, selection);
    }

    pub fn first(&mut self) {
        self.cursor = 0;
    }

    pub fn next(&mut self) {
        self.cursor = self.cursor.saturating_add(1);
    }

    pub fn has_more(&self) -> bool {
        self.cursor < self.chains.len()
    }

    pub fn get(&self) -> Option<&ChainView<'a>> {
        self.chains.get(self.cursor)
    }
}

impl std::fmt::Display for ChainIterator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ChainIterator(size={}, cursor={})",
            self.chains.len(),
            self.cursor
        )
    }
}

#[derive(Debug, Clone)]
pub struct ChainView<'a> {
    target: &'a MolCoord,
    chain_id: String,
}

impl<'a> ChainView<'a> {
    pub fn molecule(&self) -> &'a MolCoord {
        self.target
    }

    pub fn name(&self) -> &str {
        &self.chain_id
    }

    pub fn atoms(&self) -> Vec<&'a Atom> {
        self.target.chain_atoms(&self.chain_id)
    }

    pub fn residue_keys(&self) -> Vec<ResidueKey> {
        self.target
            .residue_summaries()
            .into_iter()
            .filter(|summary| summary.key.chain_id == self.chain_id)
            .map(|summary| summary.key)
            .collect()
    }

    pub fn resids_json(&self) -> String {
        let body = self
            .residue_keys()
            .iter()
            .map(|key| format!("\"{}\"", key.script_index()))
            .collect::<Vec<_>>()
            .join(",");
        format!("[{body}]")
    }
}

impl std::fmt::Display for ChainView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.chain_id)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResidRangeSet {
    residues: BTreeSet<ResidueKey>,
}

impl ResidRangeSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.residues.clear();
    }

    pub fn from_selection(&mut self, molecule: &MolCoord, selection: &Selector) {
        self.clear();
        self.append(molecule, selection);
    }

    pub fn from_sel(&mut self, molecule: &MolCoord, selection: &Selector) {
        self.from_selection(molecule, selection);
    }

    pub fn append(&mut self, molecule: &MolCoord, selection: &Selector) {
        for index in molecule.select_indices(selection) {
            let atom = &molecule.atoms()[index];
            self.residues.insert(ResidueKey::new(
                atom.chain_id.clone(),
                atom.res_id,
                atom.ins_code,
            ));
        }
    }

    pub fn remove(&mut self, molecule: &MolCoord, selection: &Selector) {
        for index in molecule.select_indices(selection) {
            let atom = &molecule.atoms()[index];
            self.residues.remove(&ResidueKey::new(
                atom.chain_id.clone(),
                atom.res_id,
                atom.ins_code,
            ));
        }
    }

    pub fn contains_key(&self, key: &ResidueKey) -> bool {
        self.residues.contains(key)
    }

    pub fn contains_atom(&self, atom: &Atom) -> bool {
        self.residues.contains(&ResidueKey::new(
            atom.chain_id.clone(),
            atom.res_id,
            atom.ins_code,
        ))
    }

    pub fn contains(&self, residue: &crate::MolResidue) -> bool {
        self.residues.contains(&ResidueKey::new(
            residue.chain_name().to_string(),
            residue.index().number,
            residue.index().ins_code,
        ))
    }

    pub fn residues(&self) -> &BTreeSet<ResidueKey> {
        &self.residues
    }

    pub fn to_selection(&self) -> Result<Selector> {
        if self.residues.is_empty() {
            return Ok(Selector::None);
        }
        parse_selector(&self.to_string())
    }

    pub fn to_sel(&self, _molecule: &MolCoord) -> Result<Selector> {
        self.to_selection()
    }
}

impl std::fmt::Display for ResidRangeSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut chains = BTreeMap::<&str, Vec<&ResidueKey>>::new();
        for residue in &self.residues {
            chains
                .entry(residue.chain_id.as_str())
                .or_default()
                .push(residue);
        }
        let body = chains
            .into_iter()
            .map(|(chain, residues)| format!("'{chain}'.{}.*", format_residue_ranges(&residues)))
            .collect::<Vec<_>>()
            .join("|");
        f.write_str(&body)
    }
}

fn format_residue_ranges(residues: &[&ResidueKey]) -> String {
    let Some(first) = residues.first().copied() else {
        return String::new();
    };
    let mut ranges = Vec::new();
    let mut start = first;
    let mut end = first;
    for residue in residues.iter().copied().skip(1) {
        if residue.ins_code == end.ins_code && residue.res_id == end.res_id + 1 {
            end = residue;
        } else {
            ranges.push(format_residue_range(start, end));
            start = residue;
            end = residue;
        }
    }
    ranges.push(format_residue_range(start, end));
    ranges.join(",")
}

fn format_residue_range(start: &ResidueKey, end: &ResidueKey) -> String {
    if start.res_id == end.res_id && start.ins_code == end.ins_code {
        start.script_index()
    } else {
        format!("{}:{}", start.script_index(), end.script_index())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::Vector4;
    use std::collections::BTreeMap;

    fn atom(serial: i32, chain: &str, resid: i32, name: &str) -> Atom {
        Atom {
            serial,
            name: name.to_string(),
            alt_loc: None,
            res_name: "ALA".to_string(),
            chain_id: chain.to_string(),
            res_id: resid,
            ins_code: None,
            position: Vector4::new3(serial as f64, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "C".to_string(),
            formal_charge: None,
            int_props: BTreeMap::new(),
        }
    }

    fn molecule() -> MolCoord {
        let mut mol = MolCoord::new();
        mol.push_atom(atom(1, "A", 1, "N"));
        mol.push_atom(atom(2, "A", 1, "CA"));
        mol.push_atom(atom(3, "B", 2, "CA"));
        mol
    }

    #[test]
    fn atom_iterator_matches_qif_cursor_flow() {
        let mol = molecule();
        let mut iter = AtomIterator::new(&mol, parse_selector("name CA").unwrap());

        assert!(iter.has_more());
        assert_eq!(iter.get_id(), Some(2));
        iter.next();
        assert_eq!(iter.get_id(), Some(3));
        iter.next();
        assert!(!iter.has_more());
        iter.first();
        assert_eq!(iter.get().unwrap().serial, 2);
        iter.set_selection(parse_selector("chain B").unwrap());
        assert_eq!(iter.get_id(), Some(3));
        assert_eq!(iter.get_target().atom_count(), 3);
        assert!(iter.to_string().contains("AtomIterator"));
    }

    #[test]
    fn residue_and_chain_iterators_return_summaries() {
        let mol = molecule();
        let mut residues = ResidIterator::new(&mol, parse_selector("chain A").unwrap());
        assert!(residues.has_more());
        let residue = residues.get().unwrap();
        assert_eq!(residue.chain_name(), "A");
        assert_eq!(residue.sindex(), "1");
        assert_eq!(residue.atoms().len(), 2);
        assert_eq!(residue.atoms_json(), "[1,2]");
        residues.next();
        assert!(!residues.has_more());
        residues.set_selection(parse_selector("chain B").unwrap());
        assert_eq!(residues.get().unwrap().chain_name(), "B");

        let mut chains = ChainIterator::all(&mol);
        assert_eq!(chains.get().unwrap().name(), "A");
        assert_eq!(chains.get().unwrap().resids_json(), "[\"1\"]");
        chains.next();
        assert_eq!(chains.get().unwrap().name(), "B");
        chains.set_selection(parse_selector("chain A").unwrap());
        assert_eq!(chains.get().unwrap().name(), "A");
    }

    #[test]
    fn residue_range_set_roundtrips_selection() {
        let mol = molecule();
        let mut ranges = ResidRangeSet::new();
        ranges.from_selection(&mol, &parse_selector("chain A").unwrap());

        assert_eq!(ranges.residues().len(), 1);
        assert!(ranges.contains_atom(&mol.atoms()[0]));
        assert!(!ranges.contains_atom(&mol.atoms()[2]));
        assert_eq!(ranges.to_string(), "'A'.1.*");
        assert!(ranges.to_selection().unwrap().matches(&mol.atoms()[0]));

        ranges.append(&mol, &parse_selector("chain B").unwrap());
        assert_eq!(ranges.residues().len(), 2);
        assert_eq!(ranges.to_string(), "'A'.1.*|'B'.2.*");
        ranges.remove(&mol, &parse_selector("chain A").unwrap());
        assert_eq!(ranges.residues().len(), 1);
        ranges.clear();
        assert!(matches!(ranges.to_selection().unwrap(), Selector::None));
    }

    #[test]
    fn residue_range_format_compresses_reference_intervals() {
        let keys = [
            ResidueKey::new("A", 1, None),
            ResidueKey::new("A", 2, None),
            ResidueKey::new("A", 4, None),
            ResidueKey::new("A", 5, Some('A')),
            ResidueKey::new("A", 6, Some('A')),
        ];
        let refs = keys.iter().collect::<Vec<_>>();
        assert_eq!(format_residue_ranges(&refs), "1:2,4,5A:6A");
    }
}
