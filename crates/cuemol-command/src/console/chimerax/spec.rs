use super::{colors, Model};
use cuemol_desktop_core::DesktopAppState;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Error {
    pub message: String,
    pub pos: usize,
}
impl Error {
    pub(super) fn new(message: impl Into<String>, pos: usize) -> Self {
        Self {
            message: message.into(),
            pos,
        }
    }
    pub fn display(&self, input: &str) -> String {
        format!(
            "{input}\n{}^ {}",
            " ".repeat(self.pos.min(input.len())),
            self.message
        )
    }
}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Selector(String, usize),
    Hier {
        models: Option<Vec<(Option<u32>, Option<u32>)>>,
        tests: Vec<Test>,
        chains: Vec<Chain>,
    },
    Zone {
        term: Box<Expr>,
        level: char,
        outside: bool,
        distance: f64,
        pos: usize,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Part {
    start: String,
    end: Option<String>,
    pos: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Test {
    name: String,
    op: Option<String>,
    value: Option<String>,
    negated: bool,
    pos: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Residues {
    parts: Option<Vec<Part>>,
    tests: Vec<Test>,
    atoms: Option<Vec<Part>>,
    atom_tests: Vec<Test>,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Chain {
    parts: Option<Vec<Part>>,
    tests: Vec<Test>,
    residues: Vec<Residues>,
}

const BACKBONE: &str =
    "((protein) and (name N,CA,C,O,OXT)) or ((nucleic) and (name P,OP1,OP2,O5*,C5*,C4*,C3*,O3*))";
const SELECTORS: &[(&str, &str)] = &[
    ("all", "*"),
    ("protein", "protein"),
    ("nucleic", "nucleic"),
    ("nucleic-acid", "nucleic"),
    ("ligand", "ligand"),
    ("solvent", "water"),
    ("backbone", BACKBONE),
    ("mainchain", BACKBONE),
    ("sidechain", "(protein) and not (name N,CA,C,O,OXT)"),
    ("sideonly", "(protein) and not (name N,CA,C,O,OXT)"),
    ("helix", "helix"),
    ("strand", "sheet"),
    ("coil", "coil"),
];
const UNSUPPORTED: &[&str] = &[
    "ions",
    "main",
    "min-backbone",
    "ribose",
    "pbonds",
    "pbondatoms",
    "hbonds",
    "hbondatoms",
    "sel-residues",
    "template-mismatch",
];
pub(super) fn selector_names(names: &BTreeSet<String>) -> Vec<String> {
    SELECTORS
        .iter()
        .map(|(name, _)| (*name).into())
        .chain(std::iter::once("sel".into()))
        .chain(names.iter().cloned())
        .chain(colors::tables().elements.keys().cloned())
        .collect()
}
fn is_selector(name: &str, names: &BTreeSet<String>) -> bool {
    name == "sel"
        || SELECTORS.iter().any(|(n, _)| *n == name)
        || UNSUPPORTED.contains(&name)
        || colors::tables().elements.contains_key(name)
        || names.contains(name)
}

struct Reader<'a> {
    text: &'a str,
    pos: usize,
    names: &'a BTreeSet<String>,
    depth: usize,
}
impl Reader<'_> {
    fn rest(&self) -> &str {
        &self.text[self.pos..]
    }
    fn take(&mut self, token: &str) -> bool {
        if self.rest().starts_with(token) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }
    fn space(&mut self) {
        self.pos = self.text.len() - self.rest().trim_start().len();
    }
    fn error(&self, message: impl Into<String>) -> Error {
        Error::new(message, self.pos)
    }
    fn zone(&self) -> bool {
        let b = self.rest().as_bytes();
        b.len() >= 2 && b"@:/#".contains(&b[0]) && b"<>".contains(&b[1])
    }
    fn model_prefix(text: &str) -> Option<usize> {
        let n = text.bytes().take_while(u8::is_ascii_digit).count();
        (n > 0 && n <= 5 && !text.as_bytes().get(n).is_some_and(u8::is_ascii_hexdigit)).then_some(n)
    }
    fn starts(&self) -> bool {
        if self.zone() {
            return false;
        }
        let s = self.rest();
        match s.chars().next() {
            Some('(' | '~' | '/' | ':' | '@') => true,
            Some('#') => {
                s.starts_with("#!")
                    || s.starts_with("##")
                    || s.starts_with("#*")
                    || Self::model_prefix(&s[1..]).is_some()
            }
            _ => re!(r"^[A-Za-z_][-+A-Za-z0-9_]*")
                .find(s)
                .is_some_and(|word| {
                    let after = &s[word.end()..];
                    is_selector(word.as_str(), self.names)
                        && (after.is_empty()
                            || after
                                .chars()
                                .next()
                                .is_some_and(|c| c.is_whitespace() || "&|()".contains(c))
                            || re!(r"^[@:/#][<>]").is_match(after))
                }),
        }
    }
    fn spec(&mut self) -> Result<Expr> {
        self.depth += 1;
        if self.depth > 256 {
            return Err(self.error("atom specification is nested too deeply"));
        }
        let left = self.union()?;
        let save = self.pos;
        self.space();
        let expr = if self.rest().starts_with('&') || self.rest().starts_with('|') {
            let and = self.take("&");
            if !and {
                self.take("|");
            }
            self.space();
            if !self.starts() {
                return Err(self.error(format!(
                    "a specification is expected after \"{}\"",
                    if and { "&" } else { "|" }
                )));
            }
            let right = self.spec()?;
            if and {
                Expr::And(Box::new(left), Box::new(right))
            } else {
                Expr::Or(Box::new(left), Box::new(right))
            }
        } else {
            self.pos = save;
            left
        };
        self.depth -= 1;
        Ok(expr)
    }
    fn union(&mut self) -> Result<Expr> {
        let mut result = self.term()?;
        loop {
            let save = self.pos;
            self.space();
            if self.pos == save || !self.starts() {
                self.pos = save;
                return Ok(result);
            }
            result = Expr::Or(Box::new(result), Box::new(self.term()?));
        }
    }
    fn term(&mut self) -> Result<Expr> {
        self.depth += 1;
        if self.depth > 256 {
            return Err(self.error("atom specification is nested too deeply"));
        }
        let expr = if self.take("(") {
            self.space();
            let inner = self.spec()?;
            self.space();
            if !self.take(")") {
                return Err(self.error("\")\" is missing"));
            }
            self.zone_after(inner)?
        } else if self.take("~") {
            self.space();
            if !self.starts() {
                return Err(self.error("a specification is expected after \"~\""));
            }
            Expr::Not(Box::new(self.term()?))
        } else if self
            .rest()
            .chars()
            .next()
            .is_some_and(|c| "#/:@".contains(c))
        {
            let item = self.hier()?;
            self.zone_after(item)?
        } else if let Some(word) = re!(r"^[A-Za-z_][-+A-Za-z0-9_]*").find(self.rest()) {
            let pos = self.pos;
            let name = word.as_str().to_string();
            if !is_selector(&name, self.names) {
                return Err(self.error("a specification is expected here"));
            }
            self.pos += word.end();
            self.zone_after(Expr::Selector(name, pos))?
        } else {
            return Err(self.error("a specification is expected here"));
        };
        self.depth -= 1;
        Ok(expr)
    }
    fn zone_after(&mut self, mut term: Expr) -> Result<Expr> {
        loop {
            let save = self.pos;
            self.space();
            let Some(m) = re!(r"^([@:/#])([<>])\s*([0-9]*\.?[0-9]+)").captures(self.rest()) else {
                self.pos = save;
                return Ok(term);
            };
            let pos = self.pos;
            let level = m[1].chars().next().unwrap();
            let outside = &m[2] == ">";
            let distance = m[3]
                .parse::<f64>()
                .map_err(|_| self.error("invalid zone distance"))?;
            if !distance.is_finite() {
                return Err(self.error("zone distance must be finite"));
            }
            self.pos += m[0].len();
            term = Expr::Zone {
                term: Box::new(term),
                level,
                outside,
                distance,
                pos,
            };
        }
    }
    fn tests(&mut self) -> Result<Vec<Test>> {
        let mut tests = Vec::new();
        loop {
            let pos = self.pos;
            let negated = self.take("^");
            let name = re!(r"^[A-Za-z_][A-Za-z0-9_]*")
                .find(self.rest())
                .ok_or_else(|| self.error("an attribute name is expected"))?;
            let name = name.as_str().to_string();
            self.pos += name.len();
            let mut op = None;
            let mut value = None;
            if !negated {
                if let Some(m) = re!(r"^(>=|<=|!==|!=|==|<>|>|<|=)").find(self.rest()) {
                    op = Some(m.as_str().to_string());
                    self.pos += m.end();
                    let m = re!(r#"^\"([^\"]*)\"|^'([^']*)'|^([^,#/:@\s]+)"#)
                        .captures(self.rest())
                        .ok_or_else(|| self.error("an attribute value is expected"))?;
                    value = Some(
                        m.get(1)
                            .or(m.get(2))
                            .or(m.get(3))
                            .unwrap()
                            .as_str()
                            .to_string(),
                    );
                    self.pos += m[0].len();
                }
            }
            tests.push(Test {
                name,
                op,
                value,
                negated,
                pos,
            });
            if !self.take(",") {
                return Ok(tests);
            }
        }
    }
    fn hier(&mut self) -> Result<Expr> {
        let mut models = None;
        let mut tests = Vec::new();
        if self.rest().starts_with('#') && !self.zone() {
            if self.rest().starts_with("#!") {
                return Err(self.error("\"#!\" is not supported yet"));
            }
            if !self.rest().starts_with("##") {
                self.pos += 1;
                let mut values = Vec::new();
                loop {
                    let first = self.model_number()?;
                    let last = if self.take("-") {
                        self.model_number()?
                    } else {
                        first
                    };
                    values.push((first, last));
                    if !self.take(",") {
                        break;
                    }
                }
                models = Some(values);
                if re!(r"^\.[\d*]").is_match(self.rest()) {
                    return Err(self.error("a sub-model is not supported yet"));
                }
            }
            if self.take("##") {
                tests = self.tests()?;
            }
        }
        let mut chains = Vec::new();
        if self.rest().starts_with([':', '@']) && !self.zone() {
            chains.push(Chain {
                parts: None,
                tests: Vec::new(),
                residues: self.residues()?,
            });
        }
        while self.rest().starts_with('/') && !self.zone() {
            let parts = if self.rest().starts_with("//") {
                None
            } else {
                self.pos += 1;
                Some(self.parts("chain")?)
            };
            let tests = if self.take("//") {
                self.tests()?
            } else {
                Vec::new()
            };
            chains.push(Chain {
                parts,
                tests,
                residues: self.residues()?,
            });
        }
        Ok(Expr::Hier {
            models,
            tests,
            chains,
        })
    }
    fn model_number(&mut self) -> Result<Option<u32>> {
        if self.take("*") {
            return Ok(None);
        }
        let n = Self::model_prefix(self.rest())
            .ok_or_else(|| self.error("a model number is expected"))?;
        let value = self.rest()[..n].parse().unwrap();
        self.pos += n;
        Ok(Some(value))
    }
    fn residues(&mut self) -> Result<Vec<Residues>> {
        let mut groups = Vec::new();
        while self.rest().starts_with([':', '@']) && !self.zone() {
            let mut parts = None;
            let mut tests = Vec::new();
            if self.rest().starts_with(':') {
                if !self.rest().starts_with("::") {
                    self.pos += 1;
                    parts = Some(self.parts("residue")?);
                }
                if self.take("::") {
                    tests = self.tests()?;
                }
            }
            let mut atoms = None;
            let mut atom_tests = Vec::new();
            if self.rest().starts_with('@') && !self.zone() {
                if !self.rest().starts_with("@@") {
                    self.pos += 1;
                    atoms = Some(self.parts("atom")?);
                    if let Some(part) = atoms.as_ref().unwrap().iter().find(|p| p.end.is_some()) {
                        return Err(Error::new("an atom name range is not allowed", part.pos));
                    }
                }
                if self.take("@@") {
                    atom_tests = self.tests()?;
                }
            }
            groups.push(Residues {
                parts,
                tests,
                atoms,
                atom_tests,
            });
        }
        Ok(groups)
    }
    fn parts(&mut self, what: &str) -> Result<Vec<Part>> {
        let mut parts = Vec::new();
        loop {
            let pos = self.pos;
            let part = re!(r#"^-?[A-Za-z0-9_'\"*?\[\]\\]+"#)
                .find(self.rest())
                .ok_or_else(|| self.error(format!("a {what} is expected")))?;
            let start = part.as_str().to_string();
            self.pos += part.end();
            let end = if re!(r#"^-[-A-Za-z0-9_'\"\\]"#).is_match(self.rest()) {
                self.pos += 1;
                let part = re!(r#"^[-A-Za-z0-9_'\"\\]+"#)
                    .find(self.rest())
                    .ok_or_else(|| self.error(format!("the end of a {what} range is expected")))?;
                let end = part.as_str().to_string();
                self.pos += part.end();
                Some(end)
            } else {
                None
            };
            parts.push(Part { start, end, pos });
            if !self.take(",") {
                return Ok(parts);
            }
        }
    }
}

pub(super) fn parse(input: &str, names: &BTreeSet<String>) -> Result<Option<(Expr, usize)>> {
    let mut reader = Reader {
        text: input,
        pos: 0,
        names,
        depth: 0,
    };
    reader.space();
    if !reader.starts() {
        return Ok(None);
    }
    let expr = reader.spec()?;
    Ok(Some((expr, reader.pos)))
}

pub(super) fn union(parts: impl IntoIterator<Item = String>) -> String {
    let parts = parts
        .into_iter()
        .filter(|part| part != "none")
        .collect::<Vec<_>>();
    if parts.iter().any(|part| part == "*") {
        "*".into()
    } else if parts.is_empty() {
        "none".into()
    } else if parts.len() == 1 {
        parts.into_iter().next().unwrap()
    } else {
        parts
            .iter()
            .map(|part| format!("({part})"))
            .collect::<Vec<_>>()
            .join(" or ")
    }
}
pub(super) fn intersect(parts: impl IntoIterator<Item = String>) -> String {
    let parts = parts
        .into_iter()
        .filter(|part| part != "*")
        .collect::<Vec<_>>();
    if parts.iter().any(|part| part == "none") {
        "none".into()
    } else if parts.is_empty() {
        "*".into()
    } else if parts.len() == 1 {
        parts.into_iter().next().unwrap()
    } else {
        parts
            .iter()
            .map(|part| format!("({part})"))
            .collect::<Vec<_>>()
            .join(" and ")
    }
}
pub(super) fn negate(expr: &str) -> String {
    match expr {
        "*" => "none".into(),
        "none" => "*".into(),
        _ => format!("not ({expr})"),
    }
}
pub(super) fn pattern_regex(raw: &str, ignore_case: bool, pos: usize) -> Result<String> {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '*' => out.push_str(".*"),
            '?' => out.push('.'),
            '[' => {
                out.push('[');
                let mut first = true;
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == ']' {
                        out.push(']');
                        closed = true;
                        break;
                    }
                    if first && c == '!' {
                        out.push('^');
                    } else {
                        if "\\/".contains(c) {
                            out.push('\\');
                        }
                        out.push(c);
                    }
                    first = false;
                }
                if !closed {
                    return Err(Error::new("\"]\" is missing", pos));
                }
            }
            '\\' => {
                out.push('\\');
                if let Some(c) = chars.next() {
                    out.push(c);
                }
            }
            _ => {
                if ".+^${}()|/".contains(ch) {
                    out.push('\\');
                }
                out.push(ch);
            }
        }
    }
    Ok(format!(
        "/{}^{out}$/",
        if ignore_case { "(?i)" } else { "" }
    ))
}
fn name_token(raw: &str, ignore_case: bool, pos: usize) -> Result<String> {
    if raw.contains('"') {
        return Err(Error::new("a quoted name is not supported yet", pos));
    }
    if raw.contains(['*', '?', '[']) || !ignore_case {
        pattern_regex(raw, ignore_case, pos)
    } else {
        Ok(raw.into())
    }
}
fn names_expr(keyword: &str, parts: &[Part], what: &str) -> Result<String> {
    if parts
        .iter()
        .any(|part| part.start == "*" && part.end.is_none())
    {
        return Ok("*".into());
    }
    let mut names = Vec::new();
    for part in parts {
        if part.end.is_some() {
            return Err(Error::new(
                format!("a {what} range is not supported"),
                part.pos,
            ));
        }
        names.push(name_token(&part.start, true, part.pos)?);
    }
    Ok(format!("{keyword} {}", names.join(",")))
}
fn residue_id(raw: &str) -> Option<String> {
    if raw.eq_ignore_ascii_case("start") {
        return Some("-99999".into());
    }
    if raw.eq_ignore_ascii_case("end") {
        return Some("99999".into());
    }
    let m = re!(r"^([-+]?\d+)([A-Za-z]?)$").captures(raw)?;
    Some(format!("{}{}", m[1].parse::<i64>().ok()?, &m[2]))
}
fn residue_expr(part: &Part) -> Result<String> {
    if part.start == "*" && part.end.is_none() {
        return Ok("*".into());
    }
    if let Some(first) = residue_id(&part.start) {
        if part.end.is_none() {
            return Ok(format!("resi {first}"));
        }
        if let Some(last) = part.end.as_deref().and_then(residue_id) {
            return Ok(format!("resi {first}:{last}"));
        }
    }
    if let Some(end) = &part.end {
        return Err(Error::new(
            format!("\"{}-{end}\" is not a residue number range", part.start),
            part.pos,
        ));
    }
    Ok(format!("resn {}", name_token(&part.start, true, part.pos)?))
}
fn numeric_test(keyword: &str, test: &Test) -> Result<String> {
    let value = test
        .value
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .ok_or_else(|| Error::new(format!("{} takes a number", test.name), test.pos))?;
    Ok(match test.op.as_deref() {
        Some(">" | "<") => format!("{keyword} {} {value}", test.op.as_deref().unwrap()),
        Some("=" | "==") => format!("{keyword} = {value}"),
        Some(">=") => format!("not ({keyword} < {value})"),
        Some("<=") => format!("not ({keyword} > {value})"),
        _ => format!("not ({keyword} = {value})"),
    })
}
fn name_test(keyword: &str, test: &Test) -> Result<String> {
    let value = test
        .value
        .as_deref()
        .ok_or_else(|| Error::new(format!("{} takes a value", test.name), test.pos))?;
    let expr = format!(
        "{keyword} {}",
        name_token(
            value,
            !matches!(test.op.as_deref(), Some("==" | "!==")),
            test.pos
        )?
    );
    match test.op.as_deref() {
        Some("=" | "==") => Ok(expr),
        Some("!=" | "!==" | "<>") => Ok(negate(&expr)),
        _ => Err(Error::new(
            format!(
                "{} cannot be compared with {}",
                test.name,
                test.op.as_deref().unwrap_or("")
            ),
            test.pos,
        )),
    }
}
fn test_expr(test: &Test, level: &str) -> Result<String> {
    let bad = || {
        Error::new(
            format!("the attribute \"{}\" is not supported yet", test.name),
            test.pos,
        )
    };
    if test.op.is_none() {
        let value = match (level, test.name.as_str()) {
            ("residue", "is_helix") => "helix",
            ("residue", "is_strand") => "sheet",
            _ => return Err(bad()),
        };
        return Ok(if test.negated {
            negate(value)
        } else {
            value.into()
        });
    }
    match (level, test.name.as_str()) {
        ("atom", "bfactor") => numeric_test("bfac", test),
        ("atom", "occupancy") => numeric_test("occ", test),
        ("atom", "name") => name_test("name", test),
        ("atom", "element") => name_test("elem", test),
        ("residue", "name") => name_test("resn", test),
        ("residue" | "chain", "chain_id") => name_test("chain", test),
        ("residue", "number") => {
            let value = test
                .value
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or_else(|| Error::new("number takes a whole number", test.pos))?;
            Ok(match test.op.as_deref() {
                Some(">=") => format!("resi {value}:99999"),
                Some(">") => format!("resi {}:99999", value.saturating_add(1)),
                Some("<=") => format!("resi -99999:{value}"),
                Some("<") => format!("resi -99999:{}", value.saturating_sub(1)),
                Some("=" | "==") => format!("resi {value}"),
                _ => format!("not (resi {value})"),
            })
        }
        ("residue", "polymer_type" | "ss_type") => {
            let table = if test.name == "polymer_type" {
                ["not (protein or nucleic)", "protein", "nucleic"]
            } else {
                ["coil", "helix", "sheet"]
            };
            let value = test
                .value
                .as_deref()
                .and_then(|v| v.parse::<usize>().ok())
                .and_then(|i| table.get(i))
                .ok_or_else(|| Error::new(format!("{} is one of 0, 1, 2", test.name), test.pos))?;
            match test.op.as_deref() {
                Some("=" | "==") => Ok((*value).into()),
                Some("!=" | "!==" | "<>") => Ok(negate(value)),
                _ => Err(bad()),
            }
        }
        _ => Err(bad()),
    }
}
fn tests_expr(tests: &[Test], level: &str) -> Result<String> {
    Ok(intersect(
        tests
            .iter()
            .map(|test| test_expr(test, level))
            .collect::<Result<Vec<_>>>()?,
    ))
}
fn chain_expr(chain: &Chain) -> Result<String> {
    let mut residues = Vec::new();
    for group in &chain.residues {
        residues.push(intersect([
            group
                .parts
                .as_ref()
                .map(|parts| {
                    parts
                        .iter()
                        .map(residue_expr)
                        .collect::<Result<Vec<_>>>()
                        .map(union)
                })
                .transpose()?
                .unwrap_or_else(|| "*".into()),
            tests_expr(&group.tests, "residue")?,
            group
                .atoms
                .as_ref()
                .map(|parts| names_expr("name", parts, "atom name"))
                .transpose()?
                .unwrap_or_else(|| "*".into()),
            tests_expr(&group.atom_tests, "atom")?,
        ]));
    }
    Ok(intersect([
        chain
            .parts
            .as_ref()
            .map(|parts| names_expr("chain", parts, "chain"))
            .transpose()?
            .unwrap_or_else(|| "*".into()),
        tests_expr(&chain.tests, "chain")?,
        if residues.is_empty() {
            "*".into()
        } else {
            union(residues)
        },
    ]))
}

pub(super) struct Context<'a> {
    pub state: &'a DesktopAppState,
    pub models: &'a [Model],
    pub names: &'a BTreeSet<String>,
}
impl Context<'_> {
    fn hits(&self, model: &Model, expr: &str) -> Result<Vec<usize>> {
        let Some(object) = self.state.scene_manager().object(model.id) else {
            return Ok(Vec::new());
        };
        let selector = self
            .state
            .parse_object_selector(model.id, expr)
            .map_err(|error| Error::new(error.to_string(), 0))?;
        Ok(object.mol().select_indices(&selector))
    }
    pub fn emit(&self, expr: &Expr, model: &Model) -> Result<String> {
        match expr {
            Expr::And(left, right) => Ok(intersect([
                self.emit(left, model)?,
                self.emit(right, model)?,
            ])),
            Expr::Or(left, right) => Ok(union([self.emit(left, model)?, self.emit(right, model)?])),
            Expr::Not(term) => Ok(negate(&self.emit(term, model)?)),
            Expr::Selector(name, pos) => {
                if name == "sel" {
                    return Ok(self
                        .state
                        .molecule_selection_text(model.id)
                        .ok()
                        .filter(|v| !v.trim().is_empty())
                        .unwrap_or("none")
                        .into());
                }
                if let Some((_, expr)) = SELECTORS.iter().find(|(n, _)| n == name) {
                    return Ok((*expr).into());
                }
                if colors::tables().elements.contains_key(name) {
                    return Ok(format!("elem {name}"));
                }
                if self.names.contains(name) {
                    return Ok(name.clone());
                }
                Err(Error::new(
                    format!("the selector \"{name}\" is not supported yet"),
                    *pos,
                ))
            }
            Expr::Hier {
                models,
                tests,
                chains,
            } => {
                if models.as_ref().is_some_and(|ranges| {
                    !ranges.iter().any(|(first, last)| {
                        first.is_none_or(|v| model.number >= v)
                            && last.is_none_or(|v| model.number <= v)
                    })
                }) {
                    return Ok("none".into());
                }
                for test in tests {
                    if test.name != "name" || test.value.is_none() {
                        return Err(Error::new(
                            format!("the model attribute \"{}\" is not supported yet", test.name),
                            test.pos,
                        ));
                    }
                    let exact = matches!(test.op.as_deref(), Some("==" | "!=="));
                    let pattern = pattern_regex(test.value.as_deref().unwrap(), !exact, test.pos)?;
                    let regex =
                        regex::Regex::new(&pattern[1..pattern.len() - 1].replace("\\/", "/"))
                            .map_err(|error| Error::new(error.to_string(), test.pos))?;
                    let hit = regex.is_match(&model.name);
                    let pass = match test.op.as_deref() {
                        Some("=" | "==") => hit,
                        Some("!=" | "!==" | "<>") => !hit,
                        _ => return Err(Error::new("name comparison is not supported", test.pos)),
                    };
                    if !pass {
                        return Ok("none".into());
                    }
                }
                if chains.is_empty() {
                    Ok("*".into())
                } else {
                    chains
                        .iter()
                        .map(chain_expr)
                        .collect::<Result<Vec<_>>>()
                        .map(union)
                }
            }
            Expr::Zone {
                term,
                level,
                outside,
                distance,
                pos,
            } => {
                let mut parts = Vec::new();
                for other in self.models.iter().filter(|m| m.molecule) {
                    let selection = self.emit(term, other)?;
                    if selection == "none" {
                        continue;
                    }
                    parts.push(if other.id == model.id {
                        format!("({selection}) expand {distance}")
                    } else {
                        format!(
                            "({selection}) around {distance} [{}]",
                            serde_json::to_string(&other.name).unwrap()
                        )
                    });
                }
                let near = union(parts);
                let result = match level {
                    '@' => near,
                    ':' => {
                        if near == "none" || near == "*" {
                            near
                        } else {
                            format!("byres ({near})")
                        }
                    }
                    '/' => {
                        let mut chains = BTreeSet::new();
                        if let Some(object) = self.state.scene_manager().object(model.id) {
                            for index in self.hits(model, &near)? {
                                chains.insert(object.mol().atoms()[index].chain_id.clone());
                            }
                        }
                        if chains.is_empty() {
                            "none".into()
                        } else {
                            format!(
                                "chain {}",
                                chains
                                    .iter()
                                    .map(|name| name_token(name, true, *pos))
                                    .collect::<Result<Vec<_>>>()?
                                    .join(",")
                            )
                        }
                    }
                    '#' => {
                        if self.hits(model, &near)?.is_empty() {
                            "none".into()
                        } else {
                            "*".into()
                        }
                    }
                    _ => unreachable!(),
                };
                Ok(if *outside { negate(&result) } else { result })
            }
        }
    }
}
