use crate::molecule::{Atom, MolCoord};
use cuemol_core::{CueMolError, IntRange, RangeSet, Result};
use std::collections::{BTreeSet, VecDeque};

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
    pub error_msg: String,
}

impl SelCommand {
    pub fn new() -> Self {
        Self {
            selector: None,
            error_msg: String::new(),
        }
    }

    pub fn compile(&mut self, expression: &str, _flags: u32) -> bool {
        match parse_selector(expression) {
            Ok(selector) => {
                self.selector = Some(selector);
                self.error_msg.clear();
                true
            }
            Err(error) => {
                self.selector = None;
                self.error_msg = error.to_string().to_ascii_lowercase();
                false
            }
        }
    }

    pub fn compile_result(&mut self, expression: &str) -> Result<()> {
        let selector = parse_selector(expression)?;
        self.selector = Some(selector);
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

impl Selector {
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
            Selector::RProp { name, value } => atom
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
    !is_main_chain_atom(atom)
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
        let chain_ok = self.chain == "*"
            || self
                .chain
                .split(',')
                .any(|v| v.eq_ignore_ascii_case(&atom.chain_id));
        let residue_ok = self.residue == "*" || residue_selector_matches(&self.residue, atom);
        let atom_ok = self.atom == "*"
            || self
                .atom
                .split(',')
                .any(|v| atom.matches_name_case_insensitive(v));
        chain_ok && residue_ok && atom_ok
    }
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
    if let Some(rest) = pattern.strip_prefix('^') {
        if let Some(prefix) = rest.strip_suffix('$') {
            value == prefix
        } else if rest == "[A-Z]" {
            value
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
        } else {
            value.starts_with(rest)
        }
    } else {
        value.contains(pattern)
    }
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
        .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b',')
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
    }
}
