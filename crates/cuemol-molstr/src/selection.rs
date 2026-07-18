use crate::molecule::{Atom, MolCoord};
use cuemol_core::{Box3, CueMolError, IntRange, RangeSet, Result, Vector4};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub enum Selector {
    All,
    None,
    Name(Vec<NamePattern>),
    Chain(Vec<NamePattern>),
    Element(Vec<NamePattern>),
    Alt(Vec<NamePattern>),
    ResidueName(Vec<NamePattern>),
    ResidueId(String),
    Aid(RangeSet),
    RProp {
        name: String,
        value: String,
    },
    /// Atom-property selection (`aprop name=value`), matching an atom's integer
    /// property — the atom-level analogue of `rprop` (C++ `SelPropNode` APROP mode).
    AProp {
        name: String,
        value: String,
    },
    Comparison {
        property: NumericProperty,
        op: CompareOp,
        value: f32,
    },
    Distance {
        op: DistanceOp,
        inner: Box<Selector>,
        distance: f32,
        molecule: Option<String>,
    },
    Unary {
        op: UnaryOp,
        inner: Box<Selector>,
    },
    Not(Box<Selector>),
    And(Box<Selector>, Box<Selector>),
    Or(Box<Selector>, Box<Selector>),
    /// A named-selection reference (`sel name`), resolved against a registry of
    /// saved selections via [`Selector::resolve_named`] (C++ `SelRefNode`).
    Ref(String),
    Hier(HierSelector),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamePattern {
    display: String,
    kind: NamePatternKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NamePatternKind {
    Empty,
    Bare(String),
    Quoted(String),
    Regex(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericProperty {
    BFactor,
    Occupancy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Less,
    Greater,
    Equal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceOp {
    Around,
    Expand,
    Neighbor,
    Extend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    ByRes,
    BySideChain,
    ByMainChain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierSelector {
    chain: String,
    residue: String,
    atom: String,
}

#[derive(Debug, Default, Clone)]
pub struct SelCommand {
    selector: Option<Selector>,
    original: String,
    pub error_msg: String,
}

impl SelCommand {
    pub const SEL_NONE: i32 = 0;
    pub const SEL_ALL: i32 = 1;
    pub const SEL_PART: i32 = 2;
    pub const SEL_UNKNOWN: i32 = 3;

    pub fn new() -> Self {
        Self {
            selector: None,
            original: String::new(),
            error_msg: String::new(),
        }
    }

    pub fn compile(&mut self, expression: &str, _flags: u32) -> bool {
        if expression.is_empty() {
            self.selector = None;
            self.original.clear();
            self.error_msg.clear();
            return true;
        }
        match parse_selector(expression) {
            Ok(selector) => {
                self.selector = Some(selector);
                self.original = expression.to_string();
                self.error_msg.clear();
                true
            }
            Err(error) => {
                self.selector = None;
                self.original.clear();
                self.error_msg = error.to_string().to_ascii_lowercase();
                false
            }
        }
    }

    pub fn compile_result(&mut self, expression: &str) -> Result<()> {
        if expression.is_empty() {
            self.selector = None;
            self.original.clear();
            self.error_msg.clear();
            return Ok(());
        }
        let selector = parse_selector(expression)?;
        self.selector = Some(selector);
        self.original = expression.to_string();
        self.error_msg.clear();
        Ok(())
    }

    pub fn selector(&self) -> Option<&Selector> {
        self.selector.as_ref()
    }

    pub fn dump_nodes(&self) -> String {
        self.selector
            .as_ref()
            .map(Selector::dump)
            .unwrap_or_else(|| "(null)".to_string())
    }

    pub fn is_empty(&self) -> bool {
        self.selector.is_none()
    }

    pub fn is_selected(&self, atom: &Atom) -> bool {
        self.selector
            .as_ref()
            .is_none_or(|selector| selector.matches(atom))
    }

    pub fn is_selected_resid(&self, residue: &crate::ResidueView<'_>) -> i32 {
        selection_status(
            self.selector.as_ref(),
            residue.molecule(),
            residue.atom_indices().iter().copied(),
        )
    }

    pub fn is_selected_chain(&self, chain: &crate::ChainView<'_>) -> i32 {
        let molecule = chain.molecule();
        let statuses = molecule
            .residue_summaries()
            .into_iter()
            .filter(|residue| residue.key.chain_id == chain.name())
            .map(|residue| {
                selection_status(
                    self.selector.as_ref(),
                    molecule,
                    residue.atom_indices.into_iter(),
                )
            });
        aggregate_status(statuses)
    }

    pub fn is_selected_mol(&self, molecule: &MolCoord) -> i32 {
        let statuses = molecule.chain_summaries().into_iter().map(|chain| {
            let residue_statuses = molecule
                .residue_summaries()
                .into_iter()
                .filter(|residue| residue.key.chain_id == chain.chain_id)
                .map(|residue| {
                    selection_status(
                        self.selector.as_ref(),
                        molecule,
                        residue.atom_indices.into_iter(),
                    )
                });
            aggregate_status(residue_statuses)
        });
        aggregate_status(statuses)
    }

    pub fn get_error_msg(&self) -> &str {
        &self.error_msg
    }

    pub fn set_error_msg(&mut self, value: impl Into<String>) {
        self.error_msg = value.into();
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        if self.selector.is_some() {
            self.original.clone()
        } else {
            String::new()
        }
    }
}

fn selection_status(
    selector: Option<&Selector>,
    molecule: &MolCoord,
    indices: impl Iterator<Item = usize>,
) -> i32 {
    let Some(selector) = selector else {
        return SelCommand::SEL_ALL;
    };
    let mut count = 0usize;
    let mut selected = 0usize;
    for index in indices {
        if let Some(atom) = molecule.atoms().get(index) {
            count += 1;
            selected += usize::from(selector.matches_in_molecule(atom, molecule));
        }
    }
    match selected {
        0 if count > 0 => SelCommand::SEL_NONE,
        value if value == count => SelCommand::SEL_ALL,
        _ => SelCommand::SEL_PART,
    }
}

fn aggregate_status(statuses: impl Iterator<Item = i32>) -> i32 {
    let statuses = statuses.collect::<Vec<_>>();
    if statuses
        .iter()
        .all(|status| *status != SelCommand::SEL_NONE)
    {
        SelCommand::SEL_ALL
    } else if statuses
        .iter()
        .all(|status| *status == SelCommand::SEL_NONE)
    {
        SelCommand::SEL_NONE
    } else {
        SelCommand::SEL_PART
    }
}

pub fn parse_selector(input: &str) -> Result<Selector> {
    let mut parser = Parser::new(input);
    let selector = parser.parse_or()?;
    parser.skip_ws();
    if parser.is_eof() {
        Ok(selector)
    } else {
        Err(CueMolError::parse(format!(
            "unexpected token near {:?}",
            parser.remaining()
        )))
    }
}

/// Parse a selection, resolving any `sel name` references against `registry`
/// (named/saved selections). Errors on an unknown or cyclic name.
pub fn parse_named_selector(input: &str, registry: &HashMap<String, Selector>) -> Result<Selector> {
    parse_selector(input)?.resolve_named(registry)
}

/// Stateful compatibility facade for C++ `molstr::SelCompiler`.
#[derive(Debug, Clone, Default)]
pub struct SelCompiler {
    error_msg: String,
}

impl SelCompiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compile(&mut self, input: &str) -> Option<Selector> {
        match parse_selector(input) {
            Ok(selector) => {
                self.error_msg.clear();
                Some(selector)
            }
            Err(error) => {
                self.error_msg = error.to_string();
                None
            }
        }
    }

    pub fn compile_result(&mut self, input: &str) -> Result<Selector> {
        match parse_selector(input) {
            Ok(selector) => {
                self.error_msg.clear();
                Ok(selector)
            }
            Err(error) => {
                self.error_msg = error.to_string();
                Err(error)
            }
        }
    }

    pub fn error_msg(&self) -> &str {
        &self.error_msg
    }

    pub fn get_error_msg(&self) -> &str {
        self.error_msg()
    }

    pub fn check_name_ref(name: &str, registry: &HashMap<String, Selector>) -> bool {
        registry.contains_key(name)
    }
}

/// Cached selection result corresponding to C++ `molstr::SelCacheData`.
#[derive(Debug, Clone, PartialEq)]
pub struct SelCacheData {
    cache_id: i32,
    molecule_id: usize,
    atom_ids: BTreeSet<i32>,
    selected_count: usize,
    bounding_box: Option<Box3>,
    center: Option<Vector4>,
    atom_count: usize,
    atoms_moved_generation: usize,
}

impl SelCacheData {
    pub fn cache_id(&self) -> i32 {
        self.cache_id
    }

    pub fn get_cache_id(&self) -> i32 {
        self.cache_id()
    }

    pub fn molecule_id(&self) -> usize {
        self.molecule_id
    }

    pub fn get_mol_id(&self) -> usize {
        self.molecule_id()
    }

    pub fn atom_ids(&self) -> &BTreeSet<i32> {
        &self.atom_ids
    }

    pub fn get_atom_id_set(&self) -> &BTreeSet<i32> {
        self.atom_ids()
    }

    pub fn selected_count(&self) -> usize {
        self.selected_count
    }

    pub fn bounding_box(&self) -> Option<Box3> {
        self.bounding_box
    }

    pub fn get_bound_box(&self) -> Option<Box3> {
        self.bounding_box()
    }

    pub fn center(&self) -> Option<Vector4> {
        self.center
    }

    pub fn get_center(&self) -> Option<Vector4> {
        self.center()
    }
}

/// Bounded selection cache corresponding to C++ `molstr::SelCacheMgr`.
#[derive(Debug, Clone)]
pub struct SelCacheMgr {
    data: BTreeMap<i32, SelCacheData>,
    tags: HashMap<String, i32>,
    next_id: i32,
    max_entries: usize,
}

impl Default for SelCacheMgr {
    fn default() -> Self {
        Self {
            data: BTreeMap::new(),
            tags: HashMap::new(),
            next_id: 0,
            max_entries: 10,
        }
    }
}

impl SelCacheMgr {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    pub fn set_max_entries(&mut self, value: usize) {
        self.max_entries = value.max(1);
        self.evict_oldest();
    }

    pub fn make_cache(&mut self, molecule: &MolCoord, selector: &Selector) -> &SelCacheData {
        let indices = molecule.select_indices(selector);
        let atom_ids = indices
            .iter()
            .map(|&index| molecule.atoms()[index].serial)
            .collect::<BTreeSet<_>>();
        let mut bounding_box = Box3::EMPTY;
        let mut sum = Vector4::ZERO;
        for &index in &indices {
            let position = molecule.atoms()[index].position;
            bounding_box.merge(position);
            sum = sum + position;
        }
        let center = (!indices.is_empty()).then(|| sum.scale(1.0 / indices.len() as f64));
        self.insert_cache(
            molecule,
            selector,
            atom_ids,
            (!indices.is_empty()).then_some(bounding_box),
            center,
        )
    }

    pub fn make_cache_by_set(
        &mut self,
        molecule: &MolCoord,
        selector: &Selector,
        atom_ids: BTreeSet<i32>,
    ) -> &SelCacheData {
        self.insert_cache(molecule, selector, atom_ids, None, None)
    }

    pub fn find_cache_data(
        &self,
        molecule: &MolCoord,
        selector: &Selector,
    ) -> Option<&SelCacheData> {
        let id = self.tags.get(&Self::cache_tag(molecule, selector))?;
        let data = self.data.get(id)?;
        (data.atom_count == molecule.atom_count()
            && data.atoms_moved_generation == molecule.atoms_moved_generation())
        .then_some(data)
    }

    pub fn find_or_make_cache_data(
        &mut self,
        molecule: &MolCoord,
        selector: &Selector,
    ) -> &SelCacheData {
        if let Some(id) = self.tags.get(&Self::cache_tag(molecule, selector)).copied() {
            let valid = self.data.get(&id).is_some_and(|data| {
                data.atom_count == molecule.atom_count()
                    && data.atoms_moved_generation == molecule.atoms_moved_generation()
            });
            if valid {
                return self.data.get(&id).expect("validated cache entry");
            }
            self.invalidate_cache(id);
        }
        self.make_cache(molecule, selector)
    }

    pub fn get_cache_entry(&self, id: i32) -> Option<&SelCacheData> {
        self.data.get(&id)
    }

    pub fn is_cache_valid(&self, id: i32) -> bool {
        id >= 0 && self.data.contains_key(&id)
    }

    pub fn invalidate_cache(&mut self, id: i32) -> bool {
        let removed = self.data.remove(&id).is_some();
        if removed {
            self.tags.retain(|_, cached_id| *cached_id != id);
        }
        removed
    }

    pub fn invalidate_molecule(&mut self, molecule: &MolCoord) -> usize {
        let molecule_id = Self::molecule_id(molecule);
        let ids = self
            .data
            .iter()
            .filter_map(|(id, data)| (data.molecule_id == molecule_id).then_some(*id))
            .collect::<Vec<_>>();
        for id in &ids {
            self.invalidate_cache(*id);
        }
        ids.len()
    }

    fn insert_cache(
        &mut self,
        molecule: &MolCoord,
        selector: &Selector,
        atom_ids: BTreeSet<i32>,
        bounding_box: Option<Box3>,
        center: Option<Vector4>,
    ) -> &SelCacheData {
        let cache_id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("selection cache exhausted");
        let data = SelCacheData {
            cache_id,
            molecule_id: Self::molecule_id(molecule),
            selected_count: atom_ids.len(),
            atom_ids,
            bounding_box,
            center,
            atom_count: molecule.atom_count(),
            atoms_moved_generation: molecule.atoms_moved_generation(),
        };
        self.data.insert(cache_id, data);
        self.tags
            .insert(Self::cache_tag(molecule, selector), cache_id);
        self.evict_oldest();
        self.data
            .get(&cache_id)
            .expect("new selection cache entry was retained")
    }

    fn evict_oldest(&mut self) {
        while self.data.len() > self.max_entries {
            let Some(id) = self.data.keys().next().copied() else {
                break;
            };
            self.invalidate_cache(id);
        }
    }

    fn cache_tag(molecule: &MolCoord, selector: &Selector) -> String {
        format!("{}:{}", Self::molecule_id(molecule), selector.dump())
    }

    fn molecule_id(molecule: &MolCoord) -> usize {
        std::ptr::from_ref(molecule).addr()
    }
}

impl Selector {
    /// Replace every `sel name` reference with the registered selector, returning
    /// a reference-free selector (C++ `SelRefNode::resolveReference`). Errors on an
    /// unknown name or a reference cycle.
    pub fn resolve_named(&self, registry: &HashMap<String, Selector>) -> Result<Selector> {
        self.resolve_named_depth(registry, 0)
    }

    fn resolve_named_depth(
        &self,
        registry: &HashMap<String, Selector>,
        depth: usize,
    ) -> Result<Selector> {
        if depth > 64 {
            return Err(CueMolError::parse("selection name reference is cyclic"));
        }
        Ok(match self {
            Selector::Ref(name) => {
                let target = registry
                    .get(name)
                    .ok_or_else(|| CueMolError::parse(format!("unknown selection name: {name}")))?;
                target.resolve_named_depth(registry, depth + 1)?
            }
            Selector::Not(inner) => {
                Selector::Not(Box::new(inner.resolve_named_depth(registry, depth + 1)?))
            }
            Selector::And(lhs, rhs) => Selector::And(
                Box::new(lhs.resolve_named_depth(registry, depth + 1)?),
                Box::new(rhs.resolve_named_depth(registry, depth + 1)?),
            ),
            Selector::Or(lhs, rhs) => Selector::Or(
                Box::new(lhs.resolve_named_depth(registry, depth + 1)?),
                Box::new(rhs.resolve_named_depth(registry, depth + 1)?),
            ),
            Selector::Unary { op, inner } => Selector::Unary {
                op: *op,
                inner: Box::new(inner.resolve_named_depth(registry, depth + 1)?),
            },
            Selector::Distance {
                op,
                inner,
                distance,
                molecule,
            } => Selector::Distance {
                op: *op,
                inner: Box::new(inner.resolve_named_depth(registry, depth + 1)?),
                distance: *distance,
                molecule: molecule.clone(),
            },
            other => other.clone(),
        })
    }

    pub fn dump(&self) -> String {
        match self {
            Selector::All => "*".to_string(),
            Selector::None => "!*".to_string(),
            Selector::Name(values) => format!("name {}", dump_name_list(values)),
            Selector::Chain(values) => format!("chain {}", dump_name_list(values)),
            Selector::Element(values) => format!("elem {}", dump_name_list(values)),
            Selector::Alt(values) => format!("alt {}", dump_name_list(values)),
            Selector::ResidueName(values) => format!("resn {}", dump_name_list(values)),
            Selector::ResidueId(value) => format!("resi {value}"),
            Selector::Aid(values) => format!("aid {values}"),
            Selector::RProp { name, value } => format!("rprop {name}={value}"),
            Selector::AProp { name, value } => format!("aprop {name}={value}"),
            Selector::Comparison {
                property,
                op,
                value,
            } => format!("{} {} {:.6}", property.dump(), op.dump(), value),
            Selector::Distance {
                op,
                inner,
                distance,
                molecule,
            } => {
                let mol = molecule
                    .as_ref()
                    .map(|name| format!(" [\"{name}\"]"))
                    .unwrap_or_default();
                format!("({}) {} {:.6}{mol}", inner.dump(), op.dump(), distance)
            }
            Selector::Unary { op, inner } => format!("{} ({})", op.dump(), inner.dump()),
            Selector::Not(inner) => format!("!({})", inner.dump()),
            Selector::And(lhs, rhs) => format!("({}) and ({})", lhs.dump(), rhs.dump()),
            Selector::Or(lhs, rhs) => format!("({}) or ({})", lhs.dump(), rhs.dump()),
            Selector::Ref(name) => format!("sel {name}"),
            Selector::Hier(value) => value.dump(),
        }
    }

    pub fn matches(&self, atom: &Atom) -> bool {
        match self {
            Selector::All => true,
            Selector::None => false,
            Selector::Name(values) => values.iter().any(|value| value.matches_name(&atom.name)),
            Selector::Chain(values) => values
                .iter()
                .any(|value| value.matches_bare(&atom.chain_id)),
            Selector::Element(values) => {
                values.iter().any(|value| value.matches_bare(&atom.element))
            }
            Selector::Alt(values) => values.iter().any(|value| {
                let alt = atom.alt_loc.map(|ch| ch.to_string()).unwrap_or_default();
                value.matches_bare(&alt)
            }),
            Selector::ResidueName(values) => values
                .iter()
                .any(|value| value.matches_bare(&atom.res_name)),
            Selector::ResidueId(value) => residue_selector_matches(value, atom),
            Selector::Aid(values) => values.contains(atom.serial),
            Selector::RProp { name, value } | Selector::AProp { name, value } => atom
                .int_prop(name)
                .is_some_and(|prop| prop.to_string() == *value),
            Selector::Comparison {
                property,
                op,
                value,
            } => op.matches(property.value(atom), *value),
            Selector::Distance { inner, .. } => inner.matches(atom),
            Selector::Unary { inner, .. } => inner.matches(atom),
            Selector::Not(inner) => !inner.matches(atom),
            Selector::And(lhs, rhs) => lhs.matches(atom) && rhs.matches(atom),
            Selector::Or(lhs, rhs) => lhs.matches(atom) || rhs.matches(atom),
            // An unresolved named reference matches nothing; resolve it first via
            // `resolve_named`.
            Selector::Ref(_) => false,
            Selector::Hier(value) => value.matches(atom),
        }
    }

    pub fn matches_in_molecule(&self, atom: &Atom, molecule: &MolCoord) -> bool {
        match self {
            Selector::RProp { name, value } => {
                molecule
                    .residue_prop_for_atom(atom, name)
                    .is_some_and(|prop| prop == value)
                    || self.matches(atom)
            }
            Selector::Distance {
                op,
                inner,
                distance,
                molecule: around_target,
            } => {
                if around_target.is_some() {
                    return false;
                }
                distance_selector_matches(molecule, atom, inner, *op, *distance)
            }
            Selector::Unary { op, inner } => match op {
                UnaryOp::ByRes => residue_contains_selected_atom(molecule, atom, inner),
                UnaryOp::ByMainChain => {
                    inner.matches_in_molecule(atom, molecule) && is_main_chain_atom(atom)
                }
                UnaryOp::BySideChain => {
                    inner.matches_in_molecule(atom, molecule) && is_side_chain_atom(atom)
                }
            },
            Selector::Not(inner) => !inner.matches_in_molecule(atom, molecule),
            Selector::And(lhs, rhs) => {
                lhs.matches_in_molecule(atom, molecule) && rhs.matches_in_molecule(atom, molecule)
            }
            Selector::Or(lhs, rhs) => {
                lhs.matches_in_molecule(atom, molecule) || rhs.matches_in_molecule(atom, molecule)
            }
            _ => self.matches(atom),
        }
    }
}

impl std::fmt::Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.dump())
    }
}

fn distance_selector_matches(
    molecule: &MolCoord,
    atom: &Atom,
    inner: &Selector,
    op: DistanceOp,
    distance: f32,
) -> bool {
    if matches!(op, DistanceOp::Neighbor | DistanceOp::Extend) {
        return graph_selector_matches(molecule, atom, inner, op, distance);
    }
    let include_inner = op == DistanceOp::Expand;
    let selected_self = inner.matches_in_molecule(atom, molecule);
    if selected_self {
        return include_inner;
    }
    let cutoff = distance.max(0.0) as f64;
    molecule
        .atoms()
        .iter()
        .filter(|other| !same_atom(atom, other))
        .filter(|other| inner.matches_in_molecule(other, molecule))
        .any(|other| (atom.position - other.position).length3d() < cutoff)
}

fn graph_selector_matches(
    molecule: &MolCoord,
    atom: &Atom,
    inner: &Selector,
    op: DistanceOp,
    distance: f32,
) -> bool {
    let Some(target_index) = atom_index(molecule, atom) else {
        return false;
    };
    let include_inner = op == DistanceOp::Extend;
    let max_depth = distance.max(0.0).ceil() as usize;
    let selected = molecule
        .atoms()
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| inner.matches_in_molecule(atom, molecule).then_some(index))
        .collect::<BTreeSet<_>>();
    if selected.contains(&target_index) {
        return include_inner;
    }
    if max_depth == 0 || selected.is_empty() {
        return false;
    }

    let mut visited = selected.clone();
    let mut queue = selected
        .into_iter()
        .map(|index| (index, 0_usize))
        .collect::<VecDeque<_>>();
    while let Some((index, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for next in bonded_neighbors(molecule, index) {
            if !visited.insert(next) {
                continue;
            }
            if next == target_index {
                return true;
            }
            queue.push_back((next, depth + 1));
        }
    }
    false
}

fn atom_index(molecule: &MolCoord, atom: &Atom) -> Option<usize> {
    molecule
        .atoms()
        .iter()
        .position(|candidate| same_atom(candidate, atom))
}

fn bonded_neighbors(molecule: &MolCoord, index: usize) -> Vec<usize> {
    molecule
        .bonds()
        .iter()
        .filter_map(|bond| {
            if bond.atom1 == index {
                Some(bond.atom2)
            } else if bond.atom2 == index {
                Some(bond.atom1)
            } else {
                None
            }
        })
        .collect()
}

fn residue_contains_selected_atom(molecule: &MolCoord, atom: &Atom, inner: &Selector) -> bool {
    molecule
        .atoms()
        .iter()
        .filter(|other| same_residue(atom, other))
        .any(|other| inner.matches_in_molecule(other, molecule))
}

fn same_atom(lhs: &Atom, rhs: &Atom) -> bool {
    lhs.serial == rhs.serial
        && lhs.chain_id == rhs.chain_id
        && lhs.res_id == rhs.res_id
        && lhs.ins_code == rhs.ins_code
        && lhs.name == rhs.name
        && lhs.alt_loc == rhs.alt_loc
}

fn same_residue(lhs: &Atom, rhs: &Atom) -> bool {
    lhs.chain_id == rhs.chain_id && lhs.res_id == rhs.res_id && lhs.ins_code == rhs.ins_code
}

fn is_main_chain_atom(atom: &Atom) -> bool {
    if let Some(topology) = crate::toppar::TopoDB::global().residue(&atom.res_name) {
        return topology.is_main_chain_atom(atom.name.trim());
    }
    matches!(
        atom.name.trim().to_ascii_uppercase().as_str(),
        "N" | "CA"
            | "C"
            | "O"
            | "OXT"
            | "P"
            | "OP1"
            | "OP2"
            | "O1P"
            | "O2P"
            | "O5'"
            | "C5'"
            | "C4'"
            | "O4'"
            | "C3'"
            | "O3'"
            | "C2'"
            | "O2'"
            | "C1'"
    )
}

fn is_side_chain_atom(atom: &Atom) -> bool {
    crate::toppar::TopoDB::global()
        .residue(&atom.res_name)
        .map_or_else(
            || !is_main_chain_atom(atom),
            |topology| topology.is_side_chain_atom(atom.name.trim()),
        )
}

impl NamePattern {
    fn bare(display: String) -> Self {
        let kind = if display == "(empty)" {
            NamePatternKind::Empty
        } else {
            NamePatternKind::Bare(display.clone())
        };
        Self { display, kind }
    }

    fn quoted(display: String, value: String) -> Self {
        Self {
            display,
            kind: NamePatternKind::Quoted(value),
        }
    }

    fn regex(display: String, value: String) -> Self {
        Self {
            display,
            kind: NamePatternKind::Regex(value),
        }
    }

    fn display(&self) -> &str {
        &self.display
    }

    fn matches_bare(&self, value: &str) -> bool {
        match &self.kind {
            NamePatternKind::Empty => value.is_empty(),
            NamePatternKind::Bare(pattern) => value.eq_ignore_ascii_case(pattern),
            NamePatternKind::Quoted(pattern) => value == pattern,
            NamePatternKind::Regex(pattern) => simple_regex_matches(pattern, value),
        }
    }

    fn matches_name(&self, value: &str) -> bool {
        self.matches_bare(value.trim())
    }
}

impl NumericProperty {
    fn dump(self) -> &'static str {
        match self {
            Self::BFactor => "bfac",
            Self::Occupancy => "occ",
        }
    }

    fn value(self, atom: &Atom) -> f32 {
        match self {
            Self::BFactor => atom.b_factor,
            Self::Occupancy => atom.occupancy,
        }
    }
}

impl CompareOp {
    fn dump(self) -> &'static str {
        match self {
            Self::Less => "<",
            Self::Greater => ">",
            Self::Equal => "=",
        }
    }

    fn matches(self, lhs: f32, rhs: f32) -> bool {
        match self {
            Self::Less => lhs < rhs,
            Self::Greater => lhs > rhs,
            Self::Equal => (lhs - rhs).abs() < f32::EPSILON,
        }
    }
}

impl DistanceOp {
    fn dump(self) -> &'static str {
        match self {
            Self::Around => "around",
            Self::Expand => "expand",
            Self::Neighbor => "neighbor",
            Self::Extend => "extend",
        }
    }
}

impl UnaryOp {
    fn dump(self) -> &'static str {
        match self {
            Self::ByRes => "byres",
            Self::BySideChain => "bysidech",
            Self::ByMainChain => "bymainch",
        }
    }
}

impl HierSelector {
    fn dump(&self) -> String {
        format!("{}.{}.{}", self.chain, self.residue, self.atom)
    }

    fn matches(&self, atom: &Atom) -> bool {
        let chain_ok = hier_name_list_matches(&self.chain, &atom.chain_id, false);
        let residue_ok = self.residue == "*" || residue_selector_matches(&self.residue, atom);
        let atom_ok = hier_name_list_matches(&self.atom, &atom.name, true);
        chain_ok && residue_ok && atom_ok
    }
}

fn hier_name_list_matches(input: &str, value: &str, trim_value: bool) -> bool {
    if input == "*" {
        return true;
    }
    let value = if trim_value { value.trim() } else { value };
    parse_name_list(input, ListKind::Name)
        .is_ok_and(|patterns| patterns.iter().any(|pattern| pattern.matches_bare(value)))
}

fn dump_name_list(values: &[NamePattern]) -> String {
    values
        .iter()
        .rev()
        .map(NamePattern::display)
        .collect::<Vec<_>>()
        .join(",")
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_or(&mut self) -> Result<Selector> {
        let mut selector = self.parse_and()?;
        loop {
            self.skip_ws();
            if self.consume_symbol('|') || self.consume_word("or") {
                let rhs = self.parse_and()?;
                selector = Selector::Or(Box::new(selector), Box::new(rhs));
            } else {
                break;
            }
        }
        Ok(selector)
    }

    fn parse_and(&mut self) -> Result<Selector> {
        let mut selector = self.parse_unary()?;
        loop {
            self.skip_ws();
            if self.consume_symbol('&') || self.consume_word("and") {
                let rhs = self.parse_unary()?;
                selector = Selector::And(Box::new(selector), Box::new(rhs));
            } else {
                break;
            }
        }
        Ok(selector)
    }

    fn parse_unary(&mut self) -> Result<Selector> {
        self.skip_ws();
        if self.consume_symbol('!') || self.consume_word("not") {
            let inner = self.parse_unary()?;
            return Ok(Selector::Not(Box::new(inner)));
        }
        if self.consume_word("byres") {
            let inner = self.parse_distance()?;
            return Ok(Selector::Unary {
                op: UnaryOp::ByRes,
                inner: Box::new(inner),
            });
        }
        if self.consume_word("bysidech") {
            let inner = self.parse_distance()?;
            return Ok(Selector::Unary {
                op: UnaryOp::BySideChain,
                inner: Box::new(inner),
            });
        }
        if self.consume_word("bymainch") {
            let inner = self.parse_distance()?;
            return Ok(Selector::Unary {
                op: UnaryOp::ByMainChain,
                inner: Box::new(inner),
            });
        }
        self.parse_distance()
    }

    fn parse_distance(&mut self) -> Result<Selector> {
        let mut selector = self.parse_primary()?;
        loop {
            self.skip_ws();
            let Some(op) = self.consume_distance_op() else {
                break;
            };
            let (distance, molecule) = self.parse_distance_payload()?;
            selector = Selector::Distance {
                op,
                inner: Box::new(selector),
                distance,
                molecule,
            };
        }
        Ok(selector)
    }

    fn parse_primary(&mut self) -> Result<Selector> {
        self.skip_ws();
        if self.is_eof() {
            return Err(CueMolError::parse("missing selector"));
        }
        if self.consume_symbol('(') {
            self.skip_ws();
            if self.consume_symbol(')') {
                return Err(CueMolError::parse("empty parentheses"));
            }
            let selector = self.parse_or()?;
            self.skip_ws();
            if !self.consume_symbol(')') {
                return Err(CueMolError::parse("missing closing parenthesis"));
            }
            return Ok(selector);
        }
        self.parse_leaf()
    }

    fn parse_leaf(&mut self) -> Result<Selector> {
        self.skip_ws();
        if self.remaining().starts_with("*.") {
            let raw = self.read_leaf_tail();
            return parse_hier_selector(raw.trim());
        }
        if self.consume_symbol('*') {
            return Ok(Selector::All);
        }

        if self.consume_word("all") {
            return Ok(Selector::All);
        }
        if self.consume_word("none") {
            return Ok(Selector::None);
        }

        if self.consume_keyword("chain") || self.consume_keyword("c;") {
            return Ok(Selector::Chain(parse_name_list(
                self.read_leaf_tail(),
                ListKind::Simple,
            )?));
        }
        if self.consume_keyword("elem") || self.consume_keyword("e;") {
            return Ok(Selector::Element(parse_name_list(
                self.read_leaf_tail(),
                ListKind::Simple,
            )?));
        }
        if self.consume_keyword("alt") {
            return Ok(Selector::Alt(parse_name_list(
                self.read_leaf_tail(),
                ListKind::Name,
            )?));
        }
        if self.consume_keyword("name") || self.consume_keyword("n;") {
            return Ok(Selector::Name(parse_name_list(
                self.read_leaf_tail(),
                ListKind::Name,
            )?));
        }
        if self.consume_keyword("resn") || self.consume_keyword("r;") {
            return Ok(Selector::ResidueName(parse_name_list(
                self.read_leaf_tail(),
                ListKind::Simple,
            )?));
        }
        if self.consume_keyword("aid") {
            return Ok(Selector::Aid(parse_aid_ranges(self.read_leaf_tail())?));
        }
        if self.consume_keyword("rprop") {
            return parse_rprop(self.read_leaf_tail());
        }
        if self.consume_keyword("aprop") {
            return parse_aprop(self.read_leaf_tail());
        }
        if self.consume_keyword("sel") {
            let name = self.read_leaf_tail().trim().to_string();
            if name.is_empty() {
                return Err(CueMolError::parse("missing selection name after 'sel'"));
            }
            return Ok(Selector::Ref(name));
        }
        if self.consume_keyword("resi")
            || self.consume_keyword("resid")
            || self.consume_keyword("i;")
        {
            return Ok(Selector::ResidueId(normalize_residue_id_list(
                self.read_leaf_tail(),
            )?));
        }
        if self.consume_keyword("bfac") {
            return parse_comparison(NumericProperty::BFactor, self.read_leaf_tail());
        }
        if self.consume_keyword("occ") {
            return parse_comparison(NumericProperty::Occupancy, self.read_leaf_tail());
        }

        let raw = self.read_leaf_tail();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(CueMolError::parse("missing selector"));
        }
        if trimmed == "and" || trimmed == "or" || trimmed == "not" {
            return Err(CueMolError::parse("missing operand"));
        }
        if trimmed.contains('.') {
            return parse_hier_selector(trimmed);
        }
        Err(CueMolError::undefined_reference(trimmed.to_string()))
    }

    fn consume_distance_op(&mut self) -> Option<DistanceOp> {
        if self.consume_keyword("a;") || self.consume_word("around") {
            Some(DistanceOp::Around)
        } else if self.consume_word("expand") {
            Some(DistanceOp::Expand)
        } else if self.consume_word("neighbor") {
            Some(DistanceOp::Neighbor)
        } else if self.consume_word("extend") {
            Some(DistanceOp::Extend)
        } else {
            None
        }
    }

    fn parse_distance_payload(&mut self) -> Result<(f32, Option<String>)> {
        self.skip_ws();
        let leading_molecule = if self.peek_char() == Some('[') {
            Some(self.parse_molecule_name()?)
        } else {
            None
        };
        self.skip_ws();
        let number = self.read_number_token();
        if number.is_empty() {
            return Err(CueMolError::parse("missing distance"));
        }
        if let Some(exp_index) = number.find(['e', 'E']) {
            if !number[..exp_index].contains('.') {
                return Err(CueMolError::parse(
                    "scientific notation requires decimal point",
                ));
            }
        }
        let distance = number
            .parse::<f32>()
            .map_err(|_| CueMolError::parse("invalid distance"))?;
        self.skip_ws();
        let trailing_molecule = if self.peek_char() == Some('[') {
            Some(self.parse_molecule_name()?)
        } else {
            None
        };
        if leading_molecule.is_some() && trailing_molecule.is_some() {
            return Err(CueMolError::parse("duplicate molecule name"));
        }
        Ok((distance, trailing_molecule.or(leading_molecule)))
    }

    fn read_number_token(&mut self) -> &'a str {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() || matches!(ch, '-' | '.' | 'e' | 'E') {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        &self.input[start..self.pos]
    }

    fn parse_molecule_name(&mut self) -> Result<String> {
        if !self.consume_symbol('[') {
            return Err(CueMolError::parse("missing molecule name"));
        }
        self.skip_ws();
        let start = self.pos;
        let value = if matches!(self.peek_char(), Some('\'') | Some('"')) {
            let quote = self.peek_char().unwrap();
            self.pos += quote.len_utf8();
            let value_start = self.pos;
            while let Some(ch) = self.peek_char() {
                if ch == quote {
                    let value = self.input[value_start..self.pos].to_string();
                    self.pos += quote.len_utf8();
                    self.skip_ws();
                    if !self.consume_symbol(']') {
                        return Err(CueMolError::parse("missing molecule bracket"));
                    }
                    return Ok(value);
                }
                self.pos += ch.len_utf8();
            }
            return Err(CueMolError::parse("unterminated molecule name"));
        } else {
            while let Some(ch) = self.peek_char() {
                if ch == ']' || ch.is_whitespace() {
                    break;
                }
                self.pos += ch.len_utf8();
            }
            self.input[start..self.pos].trim().to_string()
        };
        self.skip_ws();
        if value.is_empty() || value.contains('.') || value.chars().any(char::is_whitespace) {
            return Err(CueMolError::parse("invalid molecule name"));
        }
        if !self.consume_symbol(']') {
            return Err(CueMolError::parse("missing molecule bracket"));
        }
        Ok(value)
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn consume_symbol(&mut self, symbol: char) -> bool {
        if self.peek_char() == Some(symbol) {
            self.pos += symbol.len_utf8();
            true
        } else {
            false
        }
    }

    fn consume_word(&mut self, word: &str) -> bool {
        if !self.remaining().starts_with(word) {
            return false;
        }
        let end = self.pos + word.len();
        if self
            .input
            .get(end..)
            .and_then(|tail| tail.chars().next())
            .is_some_and(is_ident_continue)
        {
            return false;
        }
        self.pos = end;
        true
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        if !self.remaining().starts_with(keyword) {
            return false;
        }
        let end = self.pos + keyword.len();
        if keyword.ends_with(';') {
            self.pos = end;
            return true;
        }
        if self
            .input
            .get(end..)
            .and_then(|tail| tail.chars().next())
            .is_some_and(is_ident_continue)
        {
            return false;
        }
        self.pos = end;
        true
    }

    fn read_leaf_tail(&mut self) -> &'a str {
        let start = self.pos;
        let mut pos = self.pos;
        let mut quote: Option<char> = None;
        let mut regex = false;
        let bytes = self.input.as_bytes();
        while pos < self.input.len() {
            let ch = self.input[pos..].chars().next().unwrap();
            if let Some(quote_ch) = quote {
                pos += ch.len_utf8();
                if ch == quote_ch {
                    quote = None;
                }
                continue;
            }
            if regex {
                pos += ch.len_utf8();
                if ch == '/' {
                    regex = false;
                }
                continue;
            }
            match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    pos += ch.len_utf8();
                }
                '/' if at_token_start(bytes, start, pos) => {
                    regex = true;
                    pos += ch.len_utf8();
                }
                ')' | '&' | '|' => break,
                ch if ch.is_whitespace() => {
                    let after_ws = skip_ws_at(self.input, pos);
                    if starts_word_at(self.input, after_ws, "and")
                        || starts_word_at(self.input, after_ws, "or")
                        || starts_word_at(self.input, after_ws, "around")
                        || starts_word_at(self.input, after_ws, "expand")
                        || starts_word_at(self.input, after_ws, "neighbor")
                        || starts_word_at(self.input, after_ws, "extend")
                        || starts_keyword_at(self.input, after_ws, "a;")
                    {
                        break;
                    }
                    pos += ch.len_utf8();
                }
                _ => pos += ch.len_utf8(),
            }
        }
        self.pos = pos;
        &self.input[start..pos]
    }

    fn peek_char(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }
}

#[derive(Debug, Clone, Copy)]
enum ListKind {
    Simple,
    Name,
}

fn parse_name_list(input: &str, kind: ListKind) -> Result<Vec<NamePattern>> {
    let input = input.trim();
    if input.is_empty() {
        return Err(CueMolError::parse("missing selector argument"));
    }
    split_quoted_list(input)?
        .into_iter()
        .map(|part| parse_name_pattern(part.trim(), kind))
        .collect()
}

fn parse_name_pattern(input: &str, kind: ListKind) -> Result<NamePattern> {
    if input.is_empty() {
        return Err(CueMolError::parse("empty selector list item"));
    }

    if let Some(value) = parse_quoted(input)? {
        return Ok(NamePattern::quoted(input.to_string(), value));
    }

    if input.starts_with('/') {
        if input.len() < 2 || !input.ends_with('/') {
            return Err(CueMolError::parse("unterminated regular expression"));
        }
        return Ok(NamePattern::regex(
            input.to_string(),
            input[1..input.len() - 1].to_string(),
        ));
    }

    let normalized = match kind {
        ListKind::Name => normalize_name_token(input)?,
        ListKind::Simple => {
            if input.chars().any(char::is_whitespace) {
                return Err(CueMolError::parse("unexpected whitespace in selector item"));
            }
            input.to_string()
        }
    };
    Ok(NamePattern::bare(normalized))
}

fn parse_quoted(input: &str) -> Result<Option<String>> {
    let Some(quote) = input.chars().next().filter(|ch| *ch == '\'' || *ch == '"') else {
        return Ok(None);
    };
    if !input.ends_with(quote) || input.len() < 2 {
        return Err(CueMolError::parse("unterminated quoted string"));
    }
    Ok(Some(input[1..input.len() - 1].to_string()))
}

fn normalize_name_token(input: &str) -> Result<String> {
    let compact = input
        .split(':')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(":");
    if compact == "null" {
        return Ok("(empty)".to_string());
    }
    if compact.is_empty() {
        return Err(CueMolError::parse("empty name"));
    }
    if compact.contains("::") {
        return Err(CueMolError::parse("invalid colon syntax"));
    }
    if compact.chars().any(char::is_whitespace) {
        return Err(CueMolError::parse("unexpected whitespace in atom name"));
    }
    if is_insertion_residue_token(&compact)
        || compact.parse::<f32>().is_ok() && compact.contains('.')
    {
        return Err(CueMolError::parse("invalid bare atom name"));
    }
    let mut chars = compact.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphanumeric() || first == '_') {
        return Err(CueMolError::parse("invalid atom name start"));
    }
    if !chars
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '\'' || ch == '*' || ch == ':')
    {
        return Err(CueMolError::parse("invalid atom name character"));
    }
    Ok(compact)
}

fn split_quoted_list(input: &str) -> Result<Vec<&str>> {
    let mut values = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut regex = false;
    let mut pos = 0;
    while pos < input.len() {
        let ch = input[pos..].chars().next().unwrap();
        if let Some(quote_ch) = quote {
            pos += ch.len_utf8();
            if ch == quote_ch {
                quote = None;
            }
            continue;
        }
        if regex {
            pos += ch.len_utf8();
            if ch == '/' {
                regex = false;
            }
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                pos += ch.len_utf8();
            }
            '/' if input[start..pos].trim().is_empty() => {
                regex = true;
                pos += ch.len_utf8();
            }
            ',' => {
                let value = input[start..pos].trim();
                if value.is_empty() {
                    return Err(CueMolError::parse("empty selector list item"));
                }
                values.push(value);
                start = pos + 1;
                pos += 1;
            }
            _ => pos += ch.len_utf8(),
        }
    }
    if quote.is_some() {
        return Err(CueMolError::parse("unterminated quoted string"));
    }
    if regex {
        return Err(CueMolError::parse("unterminated regular expression"));
    }
    let value = input[start..].trim();
    if value.is_empty() {
        return Err(CueMolError::parse("empty selector list item"));
    }
    values.push(value);
    Ok(values)
}

fn parse_aid_ranges(input: &str) -> Result<RangeSet> {
    let input = input.trim();
    if input.is_empty() {
        return Err(CueMolError::parse("missing atom id"));
    }
    let mut ranges = RangeSet::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(CueMolError::parse("empty atom id item"));
        }
        if part.matches(':').count() > 1 {
            return Err(CueMolError::parse("invalid atom id range"));
        }
        if let Some((start, end)) = part.split_once(':') {
            let start = parse_aid_endpoint(start)?;
            let end = parse_aid_endpoint(end)?;
            ranges.insert_range(IntRange::new(start, end));
        } else {
            ranges.insert_value(parse_aid_endpoint(part)?);
        }
    }
    if ranges.is_empty() {
        Err(CueMolError::parse("empty atom id selector"))
    } else {
        Ok(ranges)
    }
}

fn parse_aid_endpoint(input: &str) -> Result<i32> {
    let input = input.trim();
    if input.is_empty() {
        return Err(CueMolError::parse("missing atom id endpoint"));
    }
    let mut chars = input.chars().peekable();
    if chars.peek() == Some(&'-') {
        chars.next();
    }
    let mut digit_count = 0;
    while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
        digit_count += 1;
        chars.next();
    }
    if digit_count == 0 {
        return Err(CueMolError::parse("atom id must be numeric"));
    }
    let number_end = input
        .char_indices()
        .take_while(|(_, ch)| *ch == '-' || ch.is_ascii_digit())
        .last()
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let suffix = &input[number_end..];
    if suffix.len() > 1 || suffix.chars().any(|ch| !ch.is_ascii_alphabetic()) {
        return Err(CueMolError::parse("invalid atom id insertion code"));
    }
    input[..number_end]
        .parse::<i32>()
        .map_err(|_| CueMolError::parse("invalid atom id"))
}

fn normalize_residue_id_list(input: &str) -> Result<String> {
    let input = input.trim();
    if input.is_empty() {
        return Err(CueMolError::parse("missing residue id"));
    }
    if input.contains("::")
        || input.ends_with(':')
        || input.starts_with(':')
        || input.ends_with(',')
    {
        return Err(CueMolError::parse("invalid residue id range"));
    }
    let normalized = input
        .split(',')
        .map(|part| part.split(':').map(str::trim).collect::<Vec<_>>().join(":"))
        .collect::<Vec<_>>()
        .join(",");
    for item in normalized.split([',', ':']) {
        if !is_residue_id_token(item) {
            return Err(CueMolError::parse("invalid residue id"));
        }
    }
    Ok(compress_insertion_residue_list(&normalized))
}

fn compress_insertion_residue_list(input: &str) -> String {
    let parts = input.split(',').collect::<Vec<_>>();
    if parts.len() < 2 || parts.iter().any(|part| part.contains(':')) {
        return input.to_string();
    }
    let parsed = parts
        .iter()
        .map(|part| split_residue_insert(part))
        .collect::<Option<Vec<_>>>();
    let Some(parsed) = parsed else {
        return input.to_string();
    };
    let number = parsed[0].0;
    if !parsed
        .iter()
        .all(|(num, code)| *num == number && code.is_some())
    {
        return input.to_string();
    }
    let codes = parsed
        .iter()
        .map(|(_, code)| code.unwrap())
        .collect::<Vec<_>>();
    if codes
        .windows(2)
        .all(|pair| (pair[1] as u32) == (pair[0] as u32) + 1)
    {
        format!("{number}{}:{number}{}", codes[0], codes[codes.len() - 1])
    } else {
        input.to_string()
    }
}

fn split_residue_insert(input: &str) -> Option<(i32, Option<char>)> {
    let mut end = 0;
    for (index, ch) in input.char_indices() {
        if index == 0 && ch == '-' || ch.is_ascii_digit() {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let number = input[..end].parse().ok()?;
    let suffix = &input[end..];
    match suffix.chars().collect::<Vec<_>>().as_slice() {
        [] => Some((number, None)),
        [code] if code.is_ascii_alphabetic() => Some((number, Some(*code))),
        _ => None,
    }
}

fn parse_comparison(property: NumericProperty, input: &str) -> Result<Selector> {
    let input = input.trim();
    let (op, rest) = if let Some(rest) = input.strip_prefix('<') {
        (CompareOp::Less, rest)
    } else if let Some(rest) = input.strip_prefix('>') {
        (CompareOp::Greater, rest)
    } else if let Some(rest) = input.strip_prefix('=') {
        (CompareOp::Equal, rest)
    } else {
        return Err(CueMolError::parse("missing comparison operator"));
    };
    let value_text = rest.trim();
    if value_text.is_empty() {
        return Err(CueMolError::parse("missing comparison value"));
    }
    if value_text.starts_with(['<', '>', '=', '!']) {
        return Err(CueMolError::parse("invalid comparison operator"));
    }
    if let Some(exp_index) = value_text.find(['e', 'E']) {
        if !value_text[..exp_index].contains('.') {
            return Err(CueMolError::parse(
                "scientific notation requires decimal point",
            ));
        }
    }
    let value = value_text
        .parse::<f32>()
        .map_err(|_| CueMolError::parse("invalid comparison value"))?;
    Ok(Selector::Comparison {
        property,
        op,
        value,
    })
}

/// Parse `aprop name=value` — an atom integer-property match.
fn parse_aprop(input: &str) -> Result<Selector> {
    let input = input.trim();
    let Some((name, value)) = input.split_once('=') else {
        return Err(CueMolError::parse("missing atom property value"));
    };
    if value.trim_start().starts_with('=') {
        return Err(CueMolError::parse("invalid atom property operator"));
    }
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() {
        return Err(CueMolError::parse("missing atom property name"));
    }
    if value.is_empty() {
        return Err(CueMolError::parse("missing atom property value"));
    }
    Ok(Selector::AProp {
        name: name.to_string(),
        value: value.to_string(),
    })
}

fn parse_rprop(input: &str) -> Result<Selector> {
    let input = input.trim();
    let Some((name, value)) = input.split_once('=') else {
        return Err(CueMolError::parse("missing residue property value"));
    };
    if value.trim_start().starts_with('=') {
        return Err(CueMolError::parse("invalid residue property operator"));
    }
    let name = normalize_rprop_name(name.trim())?;
    let value = normalize_rprop_value(value.trim())?;
    Ok(Selector::RProp { name, value })
}

fn normalize_rprop_name(input: &str) -> Result<String> {
    if input.is_empty() {
        return Err(CueMolError::parse("missing residue property name"));
    }
    if input.starts_with(['"', '\'']) {
        return Err(CueMolError::parse("invalid residue property name"));
    }
    normalize_name_token(input)
}

fn normalize_rprop_value(input: &str) -> Result<String> {
    if input.is_empty() {
        return Err(CueMolError::parse("missing residue property value"));
    }
    if let Some(value) = parse_quoted(input)? {
        return Ok(value);
    }
    if input.starts_with('/') {
        if input.len() < 2 || !input.ends_with('/') {
            return Err(CueMolError::parse("unterminated regular expression"));
        }
        return Ok(input[1..input.len() - 1].to_string());
    }
    if input.parse::<i32>().is_ok() {
        return Ok(input.to_string());
    }
    if looks_numeric(input) && (input.contains('.') || input.contains('e') || input.contains('E')) {
        let value = input
            .parse::<f64>()
            .map_err(|_| CueMolError::parse("invalid residue property value"))?;
        return Ok(format!("{value:.6}"));
    }
    normalize_name_token(input)
}

fn parse_hier_selector(input: &str) -> Result<Selector> {
    let parts = split_hier_parts(input)?;
    if parts.len() != 3 {
        return Err(CueMolError::parse("invalid hierarchical selector"));
    }
    let chain = normalize_hier_name_list(parts[0])?;
    let residue = if parts[1].trim() == "*" {
        "*".to_string()
    } else {
        normalize_residue_id_list(parts[1])?
    };
    let atom = normalize_hier_name_list(parts[2])?;
    Ok(Selector::Hier(HierSelector {
        chain,
        residue,
        atom,
    }))
}

fn normalize_hier_name_list(input: &str) -> Result<String> {
    let input = input.trim();
    if input == "*" {
        return Ok("*".to_string());
    }
    Ok(dump_name_list(&parse_name_list(input, ListKind::Name)?))
}

fn split_hier_parts(input: &str) -> Result<Vec<&str>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut regex = false;
    let mut pos = 0;
    while pos < input.len() {
        let ch = input[pos..].chars().next().unwrap();
        if let Some(quote_ch) = quote {
            pos += ch.len_utf8();
            if ch == quote_ch {
                quote = None;
            }
            continue;
        }
        if regex {
            pos += ch.len_utf8();
            if ch == '/' {
                regex = false;
            }
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                pos += ch.len_utf8();
            }
            '/' if input[start..pos].trim().is_empty()
                || input.as_bytes().get(pos.saturating_sub(1)) == Some(&b',') =>
            {
                regex = true;
                pos += ch.len_utf8();
            }
            '.' => {
                let value = input[start..pos].trim();
                if value.is_empty() {
                    return Err(CueMolError::parse("empty hierarchical component"));
                }
                parts.push(value);
                start = pos + 1;
                pos += 1;
            }
            _ => pos += ch.len_utf8(),
        }
    }
    if quote.is_some() {
        return Err(CueMolError::parse("unterminated quoted string"));
    }
    if regex {
        return Err(CueMolError::parse("unterminated regular expression"));
    }
    let value = input[start..].trim();
    if value.is_empty() {
        return Err(CueMolError::parse("empty hierarchical component"));
    }
    parts.push(value);
    Ok(parts)
}

fn residue_selector_matches(selector: &str, atom: &Atom) -> bool {
    selector.split(',').any(|part| {
        if let Some((start, end)) = part.split_once(':') {
            parse_residue_number(start)
                .zip(parse_residue_number(end))
                .is_some_and(|(start, end)| start <= atom.res_id && atom.res_id <= end)
        } else {
            parse_residue_number(part) == Some(atom.res_id)
        }
    })
}

fn parse_residue_number(input: &str) -> Option<i32> {
    let mut end = 0;
    for (index, ch) in input.char_indices() {
        if index == 0 && ch == '-' || ch.is_ascii_digit() {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    input.get(..end)?.parse().ok()
}

fn is_residue_id_token(input: &str) -> bool {
    if input.is_empty() {
        return false;
    }
    let mut chars = input.chars().peekable();
    if chars.peek() == Some(&'-') {
        chars.next();
    }
    let mut digits = 0;
    while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
        chars.next();
        digits += 1;
    }
    if digits == 0 {
        return false;
    }
    let suffix = chars.collect::<String>();
    suffix.len() <= 1 && suffix.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn looks_numeric(input: &str) -> bool {
    input
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit() || ch == '-' || ch == '.')
}

fn is_insertion_residue_token(input: &str) -> bool {
    let mut chars = input.chars().peekable();
    let mut digit_count = 0;
    while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
        chars.next();
        digit_count += 1;
    }
    digit_count > 0 && chars.clone().count() == 1 && chars.all(|ch| ch.is_ascii_alphabetic())
}

fn simple_regex_matches(pattern: &str, value: &str) -> bool {
    regex::Regex::new(pattern).is_ok_and(|expression| expression.is_match(value))
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn skip_ws_at(input: &str, mut pos: usize) -> usize {
    while pos < input.len() {
        let ch = input[pos..].chars().next().unwrap();
        if ch.is_whitespace() {
            pos += ch.len_utf8();
        } else {
            break;
        }
    }
    pos
}

fn starts_word_at(input: &str, pos: usize, word: &str) -> bool {
    input.get(pos..).is_some_and(|tail| {
        tail.starts_with(word)
            && tail
                .get(word.len()..)
                .and_then(|rest| rest.chars().next())
                .is_none_or(|ch| !is_ident_continue(ch))
    })
}

fn starts_keyword_at(input: &str, pos: usize, keyword: &str) -> bool {
    input
        .get(pos..)
        .is_some_and(|tail| tail.starts_with(keyword))
}

fn at_token_start(bytes: &[u8], start: usize, pos: usize) -> bool {
    if pos == start {
        return true;
    }
    bytes
        .get(pos.saturating_sub(1))
        .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(*byte, b',' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdb::read_pdb_str;

    #[test]
    fn dumps_all_none_chain_elem_like_cuemol() {
        let mut cmd = SelCommand::new();
        for (input, expected) in [
            ("*", "*"),
            ("all", "*"),
            ("none", "!*"),
            ("chain A,B,C", "chain C,B,A"),
            ("c; A", "chain A"),
            ("elem C,N,O", "elem O,N,C"),
            ("e;C", "elem C"),
            ("name CA,N", "name N,CA"),
            ("n; CA", "name CA"),
            ("resn ALA,VAL", "resn VAL,ALA"),
            ("aid 1,2,3,4,5", "aid 1:5"),
            ("bfac>50", "bfac > 50.000000"),
            ("occ < 0.5", "occ < 0.500000"),
        ] {
            assert!(cmd.compile(input, 0), "{input}: {}", cmd.error_msg);
            assert_eq!(cmd.dump_nodes(), expected, "{input}");
        }
    }

    #[test]
    fn handles_logical_precedence_and_parentheses() {
        let mut cmd = SelCommand::new();
        for (input, expected) in [
            ("not name CA and name N", "(!(name CA)) and (name N)"),
            (
                "name CA or name N and name O",
                "(name CA) or ((name N) and (name O))",
            ),
            (
                "(name CA or name N) and name O",
                "((name CA) or (name N)) and (name O)",
            ),
            (
                "all and not chain A or none",
                "((*) and (!(chain A))) or (!*)",
            ),
        ] {
            assert!(cmd.compile(input, 0), "{input}: {}", cmd.error_msg);
            assert_eq!(cmd.dump_nodes(), expected, "{input}");
        }
    }

    #[test]
    fn rejects_representative_invalid_selectors() {
        let mut cmd = SelCommand::new();
        for input in [
            "chain",
            "chain ,A",
            "chain A,",
            "CHAIN A",
            "elem",
            "name \"CA",
            "aid",
            "aid 123:",
            "bfac>=50",
            "and name CA",
            "name CA and",
            "()",
        ] {
            assert!(!cmd.compile(input, 0), "{input}");
            assert_eq!(cmd.dump_nodes(), "(null)");
        }
    }

    #[test]
    fn evaluates_selection_against_pdb_atoms() {
        let pdb = "\
ATOM      1  N   THR A   1      17.047  14.099   3.625  1.00 13.79           N
ATOM      2  CA  THR A   1      16.967  12.784   4.338  1.00 10.80           C
ATOM      3  C   THR B   2      15.685  12.755   5.133  1.00  9.19           C
";
        let mol = read_pdb_str(pdb).unwrap();
        let selector = parse_selector("chain A and elem C").unwrap();
        let selected = mol.select_indices(&selector);
        assert_eq!(selected, vec![1]);
        assert_eq!(
            mol.select_indices(&parse_selector("'A'.1.'CA'").unwrap()),
            vec![1]
        );
        assert_eq!(
            mol.select_indices(&parse_selector("/^A$/.1./^N|CA$/").unwrap()),
            vec![0, 1]
        );

        let mut command = SelCommand::new();
        assert!(command.is_empty());
        assert!(command.is_selected(&mol.atoms()[0]));
        assert_eq!(command.is_selected_mol(&mol), SelCommand::SEL_ALL);
        assert!(command.compile("name CA", 0));
        assert_eq!(command.to_string(), "name CA");
        assert!(!command.is_selected(&mol.atoms()[0]));
        assert!(command.is_selected(&mol.atoms()[1]));
        assert_eq!(command.is_selected_mol(&mol), SelCommand::SEL_PART);
        let residues = crate::ResidIterator::all(&mol);
        assert_eq!(
            command.is_selected_resid(residues.get().unwrap()),
            SelCommand::SEL_PART
        );
        let chains = crate::ChainIterator::all(&mol);
        assert_eq!(
            command.is_selected_chain(chains.get().unwrap()),
            SelCommand::SEL_ALL
        );
        assert!(command.compile("", 0));
        assert!(command.is_empty());
        assert_eq!(command.to_string(), "");
    }

    #[test]
    fn evaluates_aprop_atom_property_selection() {
        // `aprop` selects on an atom's integer property (here the ANISOU U11
        // component), the atom-level analogue of `rprop`. Atom 0 carries ANISOU;
        // atom 1 does not.
        let pdb = "\
ATOM      1  N   THR A   1      17.047  14.099   3.625  1.00 13.79           N
ANISOU    1  N   THR A   1     1111   2222   3333    444    555    666       N
ATOM      2  CA  THR A   1      16.967  12.784   4.338  1.00 10.80           C
";
        let mol = read_pdb_str(pdb).unwrap();
        let sel = parse_selector("aprop anisou_u11=1111").unwrap();
        assert_eq!(sel.dump(), "aprop anisou_u11=1111");
        assert_eq!(mol.select_indices(&sel), vec![0]);
        // A non-matching value (and the atom that lacks the prop) select nothing.
        assert!(mol
            .select_indices(&parse_selector("aprop anisou_u11=9999").unwrap())
            .is_empty());
        // `aprop` composes with the boolean operators like any other leaf.
        assert_eq!(
            mol.select_indices(&parse_selector("aprop anisou_u11=1111 and name N").unwrap()),
            vec![0]
        );
        // A missing value is a parse error.
        assert!(parse_selector("aprop anisou_u11").is_err());
    }

    #[test]
    fn main_and_side_chain_selection_uses_topology_tables() {
        let pdb = "\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 10.00           C
ATOM      3  CB  ALA A   1       1.450   1.500   0.000  1.00 10.00           C
";
        let mol = read_pdb_str(pdb).unwrap();
        assert_eq!(
            mol.select_indices(&parse_selector("bymainch *").unwrap()),
            vec![0, 1]
        );
        // CueMol's residue table includes CA in both lists as the side-chain anchor.
        assert_eq!(
            mol.select_indices(&parse_selector("bysidech *").unwrap()),
            vec![1, 2]
        );
    }

    #[test]
    fn resolves_named_sel_references() {
        let pdb = "\
ATOM      1  N   THR A   1      17.047  14.099   3.625  1.00 13.79           N
ATOM      2  CA  THR A   1      16.967  12.784   4.338  1.00 10.80           C
ATOM      3  C   THR B   2      15.685  12.755   5.133  1.00  9.19           C
";
        let mol = read_pdb_str(pdb).unwrap();
        let mut registry = HashMap::new();
        registry.insert("chainA".to_string(), parse_selector("chain A").unwrap());
        registry.insert("carbons".to_string(), parse_selector("elem C").unwrap());

        // `sel name` references a saved selection.
        let sel = parse_named_selector("sel chainA", &registry).unwrap();
        assert_eq!(mol.select_indices(&sel), vec![0, 1]);

        // It composes with operators: chain-A carbons = atom 1 (CA).
        let sel = parse_named_selector("sel chainA and sel carbons", &registry).unwrap();
        assert_eq!(mol.select_indices(&sel), vec![1]);

        // The unresolved reference dumps as `sel name`.
        assert_eq!(parse_selector("sel chainA").unwrap().dump(), "sel chainA");

        // An unknown name is an error.
        assert!(parse_named_selector("sel nope", &registry).is_err());

        // A reference cycle is detected, not looped forever.
        let mut cyclic = HashMap::new();
        cyclic.insert("a".to_string(), parse_selector("sel b").unwrap());
        cyclic.insert("b".to_string(), parse_selector("sel a").unwrap());
        assert!(parse_named_selector("sel a", &cyclic).is_err());
    }

    #[test]
    fn compiler_and_cache_facades_match_reference_flow() {
        let mut compiler = SelCompiler::new();
        let selector = compiler.compile("name CA").unwrap();
        assert!(compiler.error_msg().is_empty());
        assert!(compiler.compile("name \"").is_none());
        assert!(!compiler.error_msg().is_empty());

        let pdb = "\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       2.000   0.000   0.000  1.00 10.00           C
";
        let mut molecule = read_pdb_str(pdb).unwrap();
        let mut cache = SelCacheMgr::new();
        let entry = cache.make_cache(&molecule, &selector);
        let first_id = entry.cache_id();
        assert_eq!(entry.atom_ids(), &BTreeSet::from([2]));
        assert_eq!(entry.selected_count(), 1);
        assert_eq!(entry.center(), Some(Vector4::new3(2.0, 0.0, 0.0)));
        assert!(cache.find_cache_data(&molecule, &selector).is_some());

        molecule.fire_atoms_moved();
        assert!(cache.find_cache_data(&molecule, &selector).is_none());
        let replacement_id = cache
            .find_or_make_cache_data(&molecule, &selector)
            .cache_id();
        assert_ne!(replacement_id, first_id);
        assert!(!cache.is_cache_valid(first_id));
    }
}
