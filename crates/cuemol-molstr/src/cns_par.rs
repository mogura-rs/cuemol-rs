//! CNS force-field parameter/topology reader — port of C++ molstr `CnsParFile`
//! and `ParamDB`.
//!
//! CueMol ships a CNS-format parameter file (`queptl.prm`, the Engh & Huber 1991
//! protein parameters) holding the molecular-mechanics force constants and ideal
//! geometry for each atom-type combination: bonds, angles, (proper) dihedrals,
//! impropers and non-bonded Lennard-Jones terms. `CnsParFile` parses it into a
//! [`ParamDB`] dictionary keyed by atom type.
//!
//! The same reader also accepts legacy CNS topology and linkage files. `RESI`,
//! `PRES`, `LINK`, and CueMol's `PROPRESI` extension populate a
//! [`crate::toppar::TopoDB`], including linkage-patch reversal and atom property
//! definitions. CNS control directives that the reference reader ignores are
//! consumed without changing either database.

use std::collections::HashMap;

use cuemol_core::{CueMolError, Result};

use crate::toppar::{ResiPatch, ResiToppar, TopoDB};

/// Bundled Engh & Huber protein parameters (CNS format).
const QUEPTL_PRM: &str = include_str!("../data/queptl.prm");

/// Bond energy parameter: force constant `kf` and ideal length `r0` (Å).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BondParam {
    pub kf: f64,
    pub r0: f64,
}

/// Angle energy parameter: force constant `kf` and ideal angle `theta0` (degrees).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnglParam {
    pub kf: f64,
    pub theta0: f64,
}

/// Dihedral / improper energy parameter (atom types may be the `X` wildcard).
#[derive(Debug, Clone, PartialEq)]
pub struct DiheParam {
    pub types: [String; 4],
    pub kf: f64,
    pub period: i32,
    pub phase: f64,
}

/// Non-bonded Lennard-Jones parameter (`eps`/`sigma` and the 1-4 scaled pair).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonbParam {
    pub eps: f64,
    pub sigma: f64,
    pub eps14: f64,
    pub sigma14: f64,
}

/// C++ `ParamDBDetail.hpp` compatibility types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BondTag {
    pub atomi: String,
    pub atomj: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BondVal {
    pub kf: f64,
    pub r0: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnglTag {
    pub atomi: String,
    pub atomj: String,
    pub atomk: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnglVal {
    pub kf: f64,
    pub r0: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DiheTag {
    pub atomi: String,
    pub atomj: String,
    pub atomk: String,
    pub atoml: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiheVal {
    pub kf: f64,
    pub pe: f64,
    pub del: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonbVal {
    pub sig: f64,
    pub eps: f64,
    pub sig14: f64,
    pub eps14: f64,
}

/// Molecular-mechanics parameter database, port of C++ `ParamDB`.
///
/// Keys are stored in the order they appear in the file (as in the C++ `std::map`
/// dicts); the symmetry of bonds/angles/dihedrals is resolved at lookup time by
/// trying the reversed key.
#[derive(Debug, Clone, Default)]
pub struct ParamDB {
    bonds: HashMap<(String, String), BondParam>,
    angles: HashMap<(String, String, String), AnglParam>,
    dihedrals: Vec<DiheParam>,
    impropers: Vec<DiheParam>,
    nonbonded: HashMap<String, NonbParam>,
}

impl ParamDB {
    pub fn bond_count(&self) -> usize {
        self.bonds.len()
    }
    pub fn angle_count(&self) -> usize {
        self.angles.len()
    }
    pub fn dihedral_count(&self) -> usize {
        self.dihedrals.len()
    }
    pub fn improper_count(&self) -> usize {
        self.impropers.len()
    }
    pub fn nonbonded_count(&self) -> usize {
        self.nonbonded.len()
    }

    pub fn add_bond_param(
        &mut self,
        atomi: impl Into<String>,
        atomj: impl Into<String>,
        kf: f64,
        r0: f64,
    ) -> bool {
        self.bonds
            .insert((atomi.into(), atomj.into()), BondParam { kf, r0 })
            .is_none()
    }

    pub fn add_angle_param(
        &mut self,
        atomi: impl Into<String>,
        atomj: impl Into<String>,
        atomk: impl Into<String>,
        kf: f64,
        theta0: f64,
    ) -> bool {
        self.angles
            .insert(
                (atomi.into(), atomj.into(), atomk.into()),
                AnglParam { kf, theta0 },
            )
            .is_none()
    }

    pub fn add_dihedral_param(&mut self, param: DiheParam) -> bool {
        let replaced = self
            .dihedrals
            .iter()
            .position(|value| value.types == param.types)
            .map(|index| {
                self.dihedrals.remove(index);
            })
            .is_some();
        self.dihedrals.push(param);
        !replaced
    }

    pub fn add_improper_param(&mut self, param: DiheParam) -> bool {
        let replaced = self
            .impropers
            .iter()
            .position(|value| value.types == param.types)
            .map(|index| {
                self.impropers.remove(index);
            })
            .is_some();
        self.impropers.push(param);
        !replaced
    }

    pub fn add_nonbonded_param(
        &mut self,
        atom_type: impl Into<String>,
        eps: f64,
        sigma: f64,
        eps14: f64,
        sigma14: f64,
    ) -> bool {
        self.nonbonded
            .insert(
                atom_type.into(),
                NonbParam {
                    eps,
                    sigma,
                    eps14,
                    sigma14,
                },
            )
            .is_none()
    }

    /// Look up a bond parameter for atom types `a`-`b` (order-independent).
    pub fn search_bond(&self, a: &str, b: &str) -> Option<&BondParam> {
        self.bonds
            .get(&(a.to_string(), b.to_string()))
            .or_else(|| self.bonds.get(&(b.to_string(), a.to_string())))
    }

    /// Look up an angle parameter for `a`-`b`-`c` (reversible about the vertex `b`).
    pub fn search_angle(&self, a: &str, b: &str, c: &str) -> Option<&AnglParam> {
        self.angles
            .get(&(a.to_string(), b.to_string(), c.to_string()))
            .or_else(|| {
                self.angles
                    .get(&(c.to_string(), b.to_string(), a.to_string()))
            })
    }

    /// Look up a proper dihedral for `a`-`b`-`c`-`d`. Exact (wildcard-free) matches
    /// take priority over `X`-wildcard matches, and the reversed order matches too
    /// — the standard CNS convention.
    pub fn search_dihedral(&self, a: &str, b: &str, c: &str, d: &str) -> Option<&DiheParam> {
        find_dihe_key(&self.dihedrals, [a, b, c, d])
            .or_else(|| find_dihe_key(&self.dihedrals, ["X", b, c, "X"]))
    }

    /// Look up an improper using the C++ wildcard-priority sequence.
    pub fn search_improper(&self, a: &str, b: &str, c: &str, d: &str) -> Option<&DiheParam> {
        [
            [a, b, c, d],
            [a, "X", "X", d],
            ["X", b, c, d],
            ["X", b, c, "X"],
            ["X", "X", c, d],
        ]
        .into_iter()
        .find_map(|key| find_dihe_key(&self.impropers, key))
    }

    pub fn search_nonbonded(&self, atom_type: &str) -> Option<&NonbParam> {
        self.nonbonded.get(atom_type)
    }
}

/// Find an exact parameter key in either direction. A literal `X` in `key`
/// matches only a stored `X`; callers implement the reference wildcard priority
/// by trying explicit keys in order.
fn find_dihe_key<'a>(list: &'a [DiheParam], key: [&str; 4]) -> Option<&'a DiheParam> {
    let hit = |stored: &[String; 4], order: [usize; 4]| {
        (0..4).all(|i| stored[i].eq_ignore_ascii_case(key[order[i]]))
    };
    list.iter()
        .find(|param| hit(&param.types, [0, 1, 2, 3]) || hit(&param.types, [3, 2, 1, 0]))
}

/// Parse a CNS parameter file's text into a [`ParamDB`]. Non-parameter lines
/// (CNS script directives, `remarks`, blanks) are skipped; a malformed parameter
/// record is skipped rather than aborting (matching C++ warn-and-continue).
pub fn read_cns_par_str(text: &str) -> ParamDB {
    let mut db = ParamDB::default();
    for raw in text.lines() {
        // Strip comments: everything from the first '!' (which may abut a value,
        // e.g. "180.0!note").
        let line = raw.split('!').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let tok: Vec<&str> = line.split_whitespace().collect();
        let kind = tok[0].to_ascii_lowercase();
        // CNS keywords are case-insensitive and abbreviated to their first letters.
        if kind.starts_with("bond") {
            if let (Some(kf), Some(r0)) = (parse_f64(tok.get(3)), parse_f64(tok.get(4))) {
                db.bonds.insert(
                    (tok[1].to_string(), tok[2].to_string()),
                    BondParam { kf, r0 },
                );
            }
        } else if kind.starts_with("angl") {
            if let (Some(kf), Some(theta0)) = (parse_f64(tok.get(4)), parse_f64(tok.get(5))) {
                db.angles.insert(
                    (tok[1].to_string(), tok[2].to_string(), tok[3].to_string()),
                    AnglParam { kf, theta0 },
                );
            }
        } else if kind.starts_with("dihe") {
            if let Some(p) = parse_dihe(&tok) {
                db.dihedrals.push(p);
            }
        } else if kind.starts_with("impr") {
            if let Some(p) = parse_dihe(&tok) {
                db.impropers.push(p);
            }
        } else if kind.starts_with("nonb") {
            if let (Some(eps), Some(sigma)) = (parse_f64(tok.get(2)), parse_f64(tok.get(3))) {
                let eps14 = parse_f64(tok.get(4)).unwrap_or(eps);
                let sigma14 = parse_f64(tok.get(5)).unwrap_or(sigma);
                db.nonbonded.insert(
                    tok[1].to_string(),
                    NonbParam {
                        eps,
                        sigma,
                        eps14,
                        sigma14,
                    },
                );
            }
        }
    }
    db
}

/// Parse a `dihe`/`impr` record: 4 atom types, a force constant, and an optional
/// periodicity + phase (defaulting to `0`/`0.0` when absent).
fn parse_dihe(tok: &[&str]) -> Option<DiheParam> {
    if tok.len() < 6 {
        return None;
    }
    let kf = parse_f64(tok.get(5))?;
    let period = tok.get(6).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    let phase = parse_f64(tok.get(7)).unwrap_or(0.0);
    Some(DiheParam {
        types: [
            tok[1].to_string(),
            tok[2].to_string(),
            tok[3].to_string(),
            tok[4].to_string(),
        ],
        kf,
        period,
        phase,
    })
}

fn parse_f64(tok: Option<&&str>) -> Option<f64> {
    tok.and_then(|s| s.parse::<f64>().ok())
}

/// Parse a CNS parameter file from disk.
pub fn read_cns_par_file(path: impl AsRef<std::path::Path>) -> Result<ParamDB> {
    let text = std::fs::read_to_string(path.as_ref())
        .map_err(|err| CueMolError::Io(format!("CNS par read: {err}")))?;
    Ok(read_cns_par_str(&text))
}

/// The process-wide default parameter set (Engh & Huber, bundled `queptl.prm`).
pub fn builtin_param_db() -> ParamDB {
    read_cns_par_str(QUEPTL_PRM)
}

/// CNS parameter/topology reader with the C++ `attach`/`detach` lifecycle.
///
/// The attached databases are updated cumulatively, so callers can read
/// `queptl.prm`, `queptl.top`, and `queptl.lin` in that order with one reader.
#[derive(Debug, Default)]
pub struct CnsParFile<'a> {
    parameter_db: Option<&'a mut ParamDB>,
    topology_db: Option<&'a mut TopoDB>,
}

impl<'a> CnsParFile<'a> {
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

    pub fn read_str(&mut self, text: &str) -> Result<()> {
        let parameter_db = self.parameter_db.as_deref_mut().ok_or_else(|| {
            CueMolError::invalid_argument("CNS parameter database is not attached")
        })?;
        let topology_db = self.topology_db.as_deref_mut().ok_or_else(|| {
            CueMolError::invalid_argument("CNS topology database is not attached")
        })?;
        read_cns_toppar_str(text, parameter_db, topology_db)
    }

    pub fn read(&mut self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|err| CueMolError::Io(format!("CNS top/par read: {err}")))?;
        self.read_str(&text)
    }
}

/// Parse CNS parameter/topology text into existing databases.
pub fn read_cns_toppar_str(
    text: &str,
    parameter_db: &mut ParamDB,
    topology_db: &mut TopoDB,
) -> Result<()> {
    CnsParser::new(text).parse(parameter_db, topology_db)
}

/// Parse a CNS parameter/topology file into existing databases.
pub fn read_cns_toppar_file(
    path: impl AsRef<std::path::Path>,
    parameter_db: &mut ParamDB,
    topology_db: &mut TopoDB,
) -> Result<()> {
    let text = std::fs::read_to_string(path.as_ref())
        .map_err(|err| CueMolError::Io(format!("CNS top/par read: {err}")))?;
    read_cns_toppar_str(&text, parameter_db, topology_db)
}

#[derive(Debug, Clone)]
struct CnsToken {
    raw: String,
    upper: String,
    line: usize,
}

impl CnsToken {
    fn new(raw: String, line: usize) -> Self {
        let upper = raw.to_ascii_uppercase();
        Self { raw, upper, line }
    }

    fn matches(&self, keyword: &str) -> bool {
        self.upper.starts_with(keyword)
    }
}

fn tokenize_cns(text: &str) -> Vec<CnsToken> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '\n' {
            line += 1;
            index += 1;
            continue;
        }
        if ch == ' ' || ch == '=' || ch == '\t' || ch == '\r' {
            index += 1;
            continue;
        }
        if ch == '!' {
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if ch == '{' {
            index += 1;
            while index < chars.len() && chars[index] != '}' {
                if chars[index] == '\n' {
                    line += 1;
                }
                index += 1;
            }
            if index < chars.len() {
                index += 1;
            }
            continue;
        }

        let token_line = line;
        let is_literal =
            |value: char| value.is_ascii_alphanumeric() || matches!(value, '.' | '-' | '+' | '\'');
        if is_literal(ch) {
            let start = index;
            index += 1;
            while index < chars.len() && is_literal(chars[index]) {
                index += 1;
            }
            tokens.push(CnsToken::new(
                chars[start..index].iter().collect(),
                token_line,
            ));
        } else {
            tokens.push(CnsToken::new(ch.to_string(), token_line));
            index += 1;
        }
    }

    tokens
}

#[derive(Debug)]
struct CnsParser {
    tokens: Vec<CnsToken>,
    index: usize,
}

impl CnsParser {
    fn new(text: &str) -> Self {
        Self {
            tokens: tokenize_cns(text),
            index: 0,
        }
    }

    fn parse(&mut self, parameter_db: &mut ParamDB, topology_db: &mut TopoDB) -> Result<()> {
        while let Some(token) = self.next() {
            if token.matches("REMARK") {
                self.skip_line(token.line);
            } else if token.matches("SET")
                || token.matches("NBON")
                || token.matches("AUTOGEN")
                || token.matches("FIRST")
                || token.matches("LAST")
            {
                self.skip_to_end()?;
            } else if token.matches("CHECKVER") {
                self.required("CHECKVERSION value")?;
            } else if token.matches("BOND") {
                self.parse_bond_parameter(parameter_db)?;
            } else if token.matches("ANGL") {
                self.parse_angle_parameter(parameter_db)?;
            } else if token.matches("DIHE") {
                self.parse_dihedral_parameter(parameter_db, false)?;
            } else if token.matches("IMPR") {
                self.parse_dihedral_parameter(parameter_db, true)?;
            } else if token.matches("NONB") {
                self.parse_nonbonded_parameter(parameter_db)?;
            } else if token.matches("MASS") {
                self.required("MASS atom type")?;
                self.number("MASS value")?;
            } else if token.matches("RESI") {
                self.parse_residue(parameter_db, topology_db)?;
            } else if token.matches("PRES") {
                self.parse_patch(parameter_db, topology_db)?;
            } else if token.matches("LINK") {
                self.parse_link(topology_db)?;
            } else if token.matches("PROP") {
                self.parse_residue_properties(topology_db)?;
            }
        }
        Ok(())
    }

    fn parse_bond_parameter(&mut self, parameter_db: &mut ParamDB) -> Result<()> {
        let a = self.upper("BOND atom type 1")?;
        let b = self.upper("BOND atom type 2")?;
        let kf = self.number("BOND force constant")?;
        let r0 = self.number("BOND equilibrium length")?;
        parameter_db.add_bond_param(a, b, kf, r0);
        Ok(())
    }

    fn parse_angle_parameter(&mut self, parameter_db: &mut ParamDB) -> Result<()> {
        let a = self.upper("ANGLE atom type 1")?;
        let b = self.upper("ANGLE atom type 2")?;
        let c = self.upper("ANGLE atom type 3")?;
        let kf = self.number("ANGLE force constant")?;
        let theta0 = self.number("ANGLE equilibrium angle")?;
        parameter_db.add_angle_param(a, b, c, kf, theta0);
        Ok(())
    }

    fn parse_dihedral_parameter(
        &mut self,
        parameter_db: &mut ParamDB,
        improper: bool,
    ) -> Result<()> {
        let types = [
            self.upper("DIHEDRAL atom type 1")?,
            self.upper("DIHEDRAL atom type 2")?,
            self.upper("DIHEDRAL atom type 3")?,
            self.upper("DIHEDRAL atom type 4")?,
        ];
        let kf = self.number("DIHEDRAL force constant")?;
        let period_token = self.required("DIHEDRAL periodicity")?;
        let period = period_token.raw.parse::<i32>().map_err(|_| {
            self.parse_error(
                period_token.line,
                format!("invalid DIHEDRAL periodicity `{}`", period_token.raw),
            )
        })?;
        if period < 0 {
            return Err(self.parse_error(
                period_token.line,
                "DIHEDRAL periodicity must not be negative",
            ));
        }
        let phase = self.number("DIHEDRAL phase")?;
        let parameter = DiheParam {
            types,
            kf,
            period,
            phase,
        };
        if improper {
            parameter_db.add_improper_param(parameter);
        } else {
            parameter_db.add_dihedral_param(parameter);
        }
        Ok(())
    }

    fn parse_nonbonded_parameter(&mut self, parameter_db: &mut ParamDB) -> Result<()> {
        let atom_type = self.upper("NONBONDED atom type")?;
        let eps = self.number("NONBONDED epsilon")?;
        let sigma = self.number("NONBONDED sigma")?;
        let eps14 = self.number("NONBONDED 1-4 epsilon")?;
        let sigma14 = self.number("NONBONDED 1-4 sigma")?;
        parameter_db.add_nonbonded_param(atom_type, eps, sigma, eps14, sigma14);
        Ok(())
    }

    fn parse_residue(&mut self, parameter_db: &ParamDB, topology_db: &mut TopoDB) -> Result<()> {
        let name = self.upper("RESIDUE name")?;
        let mut residue = ResiToppar::new(&name);

        loop {
            let token = self.required("RESIDUE record or END")?;
            if token.matches("END") {
                break;
            }
            if token.matches("GROU") {
                continue;
            }
            if token.matches("ATOM") {
                let (atom_name, atom_type) = self.parse_topology_atom()?;
                let nonbonded = parameter_db.search_nonbonded(&atom_type).copied();
                let (sig, eps, sig14, eps14) = nonbonded
                    .map(|value| (value.sigma, value.eps, value.sigma14, value.eps14))
                    .unwrap_or_default();
                residue.add_atom(atom_name, atom_type, sig, eps, sig14, eps14);
                continue;
            }
            if token.matches("BOND") {
                let a = self.upper("RESIDUE BOND atom 1")?;
                let b = self.upper("RESIDUE BOND atom 2")?;
                let parameter = residue
                    .get_atom_type(&a)
                    .zip(residue.get_atom_type(&b))
                    .and_then(|(a_type, b_type)| parameter_db.search_bond(a_type, b_type))
                    .copied();
                let (kf, r0) = parameter
                    .map(|value| (value.kf, value.r0))
                    .unwrap_or((-1.0, 0.0));
                residue.add_bond(&a, &b, kf, r0);
                continue;
            }
            if token.matches("DIHE") || token.matches("IMPR") {
                self.consume_names(4, "RESIDUE torsion")?;
            }
        }

        topology_db.put(residue);
        Ok(())
    }

    fn parse_topology_atom(&mut self) -> Result<(String, String)> {
        let name = self.upper("ATOM name")?;
        let mut atom_type = String::new();
        loop {
            let token = self.required("ATOM attribute or END")?;
            if token.matches("END") {
                break;
            }
            if token.matches("TYPE") {
                atom_type = self.upper("ATOM TYPE value")?;
            }
        }
        Ok((name, atom_type))
    }

    fn parse_patch(&mut self, _parameter_db: &ParamDB, topology_db: &mut TopoDB) -> Result<()> {
        let name = self.upper("PRESIDUE name")?;
        let mut patch = ResiPatch::new(&name);
        let mut mode = -1;

        loop {
            let token = self.required("PRESIDUE record or END")?;
            if token.matches("END") {
                break;
            }
            if token.matches("GROU") {
                mode = -1;
                continue;
            }
            if token.matches("ADD") {
                mode = ResiPatch::PATCH_ADD;
                continue;
            }
            if token.matches("DELE") {
                mode = ResiPatch::PATCH_DELETE;
                continue;
            }
            if token.matches("MODI") {
                mode = ResiPatch::PATCH_MODIFY;
                continue;
            }
            if token.matches("ATOM") {
                let (atom_name, atom_type) = self.parse_topology_atom()?;
                if !atom_name.is_empty() && !atom_type.is_empty() {
                    patch.add_atom(atom_name, atom_type, mode);
                }
                continue;
            }
            if token.matches("BOND") {
                let a = self.upper("PRESIDUE BOND atom 1")?;
                let b = self.upper("PRESIDUE BOND atom 2")?;
                patch.add_bond(a, b, mode);
                continue;
            }
            if token.matches("ANGL") {
                self.consume_names(3, "PRESIDUE ANGLE")?;
                continue;
            }
            if token.matches("DIHE") || token.matches("IMPR") {
                self.consume_names(4, "PRESIDUE torsion")?;
                continue;
            }
            if token.matches("LINKDIST") {
                let distance = self.number("LINKDIST value")?;
                if distance > 0.0 {
                    patch.set_link_dist(distance);
                }
            }
        }

        if patch.check_link_obj() {
            let mut forward = patch;
            forward.set_name(format!("+{name}"));
            let mut reverse = forward.clone();
            reverse.reverse();
            reverse.set_name(format!("-{name}"));
            topology_db.patch_put(forward);
            topology_db.patch_put(reverse);
        } else {
            topology_db.patch_put(patch);
        }
        Ok(())
    }

    fn parse_link(&mut self, topology_db: &mut TopoDB) -> Result<()> {
        let patch_name = self.upper("LINK patch name")?;
        self.expect("HEAD", "LINK HEAD")?;
        let previous_prefix = self.prefix("LINK HEAD prefix")?;
        let previous_residue = self.upper("LINK HEAD residue")?;
        self.expect("TAIL", "LINK TAIL")?;
        let next_prefix = self.prefix("LINK TAIL prefix")?;
        let next_residue = self.upper("LINK TAIL residue")?;
        self.expect("END", "LINK END")?;
        if !topology_db.add_link_by_name(
            &previous_residue,
            previous_prefix,
            &next_residue,
            next_prefix,
            &patch_name,
        ) {
            return Err(self.parse_error(
                self.current_line(),
                format!("LINK references unavailable patch `{patch_name}`"),
            ));
        }
        Ok(())
    }

    fn parse_residue_properties(&mut self, topology_db: &mut TopoDB) -> Result<()> {
        let requested_name = self.upper("PROPRESIDUE name")?;
        let mut residue = topology_db
            .residue(&requested_name)
            .cloned()
            .unwrap_or_else(|| ResiToppar::new(&requested_name));
        let canonical_name = residue.name.clone();
        let mut aliases = Vec::new();

        loop {
            let token = self.required("PROPRESIDUE record or END")?;
            if token.matches("END") {
                break;
            }
            if token.matches("PIVOT") {
                residue.add_pivot_atom(self.upper("PIVOT atom")?);
                continue;
            }
            if token.matches("RING") {
                residue.add_ring(self.names_until_end("RING")?);
                continue;
            }
            if token.matches("SIDECH") {
                residue.add_side_chain_atoms(self.names_until_end("SIDECH")?);
                continue;
            }
            if token.matches("MAINCH") {
                residue.add_main_chain_atoms(self.names_until_end("MAINCH")?);
                continue;
            }
            if token.matches("ALIAS") {
                aliases.push(self.upper("ALIAS name")?);
                continue;
            }
            if token.matches("PROP") {
                let key = self.raw("PROP key")?.to_ascii_lowercase();
                let value = self.raw("PROP value")?.to_ascii_lowercase();
                residue.set_property(key, value);
            }
        }

        topology_db.put(residue);
        for alias in aliases {
            topology_db.put_alias_name(alias, &canonical_name);
        }
        Ok(())
    }

    fn names_until_end(&mut self, context: &str) -> Result<Vec<String>> {
        let mut names = Vec::new();
        loop {
            let token = self.required(&format!("{context} atom or END"))?;
            if token.matches("END") {
                return Ok(names);
            }
            names.push(token.upper);
        }
    }

    fn consume_names(&mut self, count: usize, context: &str) -> Result<()> {
        for _ in 0..count {
            self.required(context)?;
        }
        Ok(())
    }

    fn skip_to_end(&mut self) -> Result<()> {
        loop {
            let token = self.required("END")?;
            if token.matches("END") {
                return Ok(());
            }
        }
    }

    fn skip_line(&mut self, line: usize) {
        while self
            .tokens
            .get(self.index)
            .is_some_and(|token| token.line == line)
        {
            self.index += 1;
        }
    }

    fn expect(&mut self, keyword: &str, context: &str) -> Result<()> {
        let token = self.required(context)?;
        if token.matches(keyword) {
            Ok(())
        } else {
            Err(self.parse_error(
                token.line,
                format!("expected {keyword}, got `{}`", token.raw),
            ))
        }
    }

    fn upper(&mut self, context: &str) -> Result<String> {
        Ok(self.required(context)?.upper)
    }

    fn raw(&mut self, context: &str) -> Result<String> {
        Ok(self.required(context)?.raw)
    }

    fn prefix(&mut self, context: &str) -> Result<char> {
        let token = self.required(context)?;
        token
            .upper
            .chars()
            .next()
            .ok_or_else(|| self.parse_error(token.line, format!("empty {context}")))
    }

    fn number(&mut self, context: &str) -> Result<f64> {
        let token = self.required(context)?;
        token
            .raw
            .parse::<f64>()
            .map_err(|_| self.parse_error(token.line, format!("invalid {context} `{}`", token.raw)))
    }

    fn required(&mut self, context: &str) -> Result<CnsToken> {
        self.next().ok_or_else(|| {
            self.parse_error(
                self.current_line(),
                format!("unexpected end of input while reading {context}"),
            )
        })
    }

    fn next(&mut self) -> Option<CnsToken> {
        let token = self.tokens.get(self.index)?.clone();
        self.index += 1;
        Some(token)
    }

    fn current_line(&self) -> usize {
        self.tokens
            .get(self.index.saturating_sub(1))
            .map_or(1, |token| token.line)
    }

    fn parse_error(&self, line: usize, message: impl Into<String>) -> CueMolError {
        CueMolError::parse(format!("CNS line {line}: {}", message.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_parameters_from_bundled_queptl_prm() {
        let db = builtin_param_db();

        // Bond C-N (peptide): kf 2312.500, r0 1.341 — and order-independent.
        let cn = db.search_bond("C", "N").expect("bond C-N");
        assert!((cn.kf - 2312.500).abs() < 1e-3);
        assert!((cn.r0 - 1.341).abs() < 1e-6);
        assert_eq!(db.search_bond("N", "C"), db.search_bond("C", "N"));

        // Angle C5W-CW-CW: kf 1349.600, theta0 107.200 — reversible.
        let angle = db.search_angle("C5W", "CW", "CW").expect("angle");
        assert!((angle.kf - 1349.600).abs() < 1e-3);
        assert!((angle.theta0 - 107.200).abs() < 1e-3);
        assert_eq!(
            db.search_angle("CW", "CW", "C5W"),
            db.search_angle("C5W", "CW", "CW")
        );

        // Proper dihedral CH1E-C-N-CH1E: kf 1250.0, period 2, phase 180.
        let dihe = db.search_dihedral("CH1E", "C", "N", "CH1E").expect("dihe");
        assert!((dihe.kf - 1250.0).abs() < 1e-3);
        assert_eq!(dihe.period, 2);
        assert!((dihe.phase - 180.0).abs() < 1e-3);

        // Wildcard dihedral: X-C-CH1E-X matches any 1st/4th type.
        let wild = db
            .search_dihedral("CH3E", "C", "CH1E", "NH1")
            .expect("wildcard dihe");
        assert_eq!(wild.period, 3);

        // Improper C-C-CR1E-CH2E: kf 750.0.
        let impr = db.search_improper("C", "C", "CR1E", "CH2E").expect("impr");
        assert!((impr.kf - 750.0).abs() < 1e-3);

        // Non-bonded H: eps 0.0498, sigma 1.4254.
        let nb = db.search_nonbonded("H").expect("nonbonded H");
        assert!((nb.eps - 0.0498).abs() < 1e-6);
        assert!((nb.sigma - 1.4254).abs() < 1e-6);

        // Sanity on bulk counts (the file has 175 bond / 320 angle records, some
        // duplicated keys collapse in the map).
        assert!(db.bond_count() > 100, "bonds: {}", db.bond_count());
        assert!(db.angle_count() > 200, "angles: {}", db.angle_count());
        assert!(db.dihedral_count() > 50);
        assert!(db.improper_count() > 50);
        assert!(db.nonbonded_count() > 20);
    }

    #[test]
    fn skips_script_directives_and_comments() {
        let text = "\
remarks this is a header
set echo=false end
bond CA CB 1000.0 1.5 ! a comment with ! marks
angle CA CB CG 500.0 110.0
! a full comment line
nbonds something end
";
        let db = read_cns_par_str(text);
        // Only the bond and angle records are parameters; remarks/set/nbonds/the
        // full-comment line are skipped.
        assert_eq!(db.bond_count(), 1);
        assert_eq!(db.angle_count(), 1);
        let b = db.search_bond("CA", "CB").unwrap();
        assert!((b.r0 - 1.5).abs() < 1e-9);
        // The `! ... ! ...` comment after the values is stripped, not parsed.
        assert!((b.kf - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn mutation_api_replaces_parameters_and_keeps_symmetric_lookup() {
        let mut db = ParamDB::default();
        assert!(db.add_bond_param("A", "B", 1.0, 2.0));
        assert!(!db.add_bond_param("A", "B", 3.0, 4.0));
        assert_eq!(db.search_bond("B", "A").unwrap().kf, 3.0);

        assert!(db.add_angle_param("A", "B", "C", 5.0, 120.0));
        assert_eq!(db.search_angle("C", "B", "A").unwrap().theta0, 120.0);
        assert!(db.add_nonbonded_param("A", 0.1, 1.1, 0.05, 1.0));
        assert!(!db.add_nonbonded_param("A", 0.2, 1.2, 0.1, 1.1));
        assert_eq!(db.search_nonbonded("A").unwrap().eps, 0.2);

        let wildcard = DiheParam {
            types: ["X".into(), "B".into(), "C".into(), "X".into()],
            kf: 6.0,
            period: 3,
            phase: 180.0,
        };
        assert!(db.add_dihedral_param(wildcard.clone()));
        assert!(!db.add_dihedral_param(DiheParam {
            kf: 7.0,
            ..wildcard
        }));
        assert_eq!(db.search_dihedral("A", "B", "C", "D").unwrap().kf, 7.0);
    }

    #[test]
    fn improper_lookup_uses_reference_wildcard_priority() {
        let mut db = ParamDB::default();
        for (types, kf) in [
            (["X", "B", "C", "X"], 1.0),
            (["A", "X", "X", "D"], 2.0),
            (["A", "B", "C", "D"], 3.0),
        ] {
            db.add_improper_param(DiheParam {
                types: types.map(str::to_string),
                kf,
                period: 0,
                phase: 0.0,
            });
        }
        assert_eq!(db.search_improper("A", "B", "C", "D").unwrap().kf, 3.0);
        assert_eq!(db.search_improper("A", "Q", "R", "D").unwrap().kf, 2.0);
        assert_eq!(db.search_improper("Q", "B", "C", "R").unwrap().kf, 1.0);

        let tag = BondTag {
            atomi: "A".to_string(),
            atomj: "B".to_string(),
        };
        let value = BondVal { kf: 3.0, r0: 1.5 };
        assert_eq!(tag.atomi, "A");
        assert_eq!(value.r0, 1.5);
    }

    #[test]
    fn parses_legacy_residue_patch_link_and_properties() {
        let text = "\
remarks complete legacy topology sample
nonbonded NH1 0.20 1.50 0.10 1.40
nonbonded C 0.30 1.70 0.15 1.60
bond NH1 C 900.0 1.33

residue ALA
  group
  atom N type=NH1 charge=-0.35 end
  atom C type=C exclusion=( N ) end
  bond N C
end

presidue PEPT
  add bond -C +N
  add angle -CA -C +N
  add dihedral -N -CA -C +N
  linkdist 1.33
end

link PEPT head - ALA tail + GLY end

propresidue ALA
  pivot N
  ring N C end
  sidechain C end
  mainchain N C end
  prop type prot
  alias ALX
end
";
        let mut parameters = ParamDB::default();
        let mut topology = TopoDB::new();
        read_cns_toppar_str(text, &mut parameters, &mut topology).unwrap();

        let alanine = topology.residue("ALA").unwrap();
        assert_eq!(alanine.atom_count(), 2);
        let nitrogen = alanine.get_atom("N").unwrap();
        assert_eq!(nitrogen.vtype, "NH1");
        assert_eq!(nitrogen.sig, 1.50);
        assert_eq!(nitrogen.eps, 0.20);
        let bond = alanine.get_bond("N", "C").unwrap();
        assert_eq!(bond.kf, 900.0);
        assert_eq!(bond.value, 1.33);
        assert_eq!(alanine.pivot_atom, "N");
        assert_eq!(alanine.rings(), &[vec!["N".to_string(), "C".to_string()]]);
        assert!(alanine.is_side_chain_atom("C"));
        assert!(alanine.is_main_chain_atom("N"));
        assert_eq!(alanine.property("type"), Some("prot"));
        assert_eq!(topology.canonical_name("ALX"), Some("ALA"));

        let forward = topology.patch("+PEPT").unwrap();
        assert!(forward.is_link_obj());
        assert_eq!(forward.prev_prefix(), Some('-'));
        assert_eq!(forward.next_prefix(), Some('+'));
        assert_eq!(forward.link_dist(), 1.33);
        let reverse = topology.patch("-PEPT").unwrap();
        assert_eq!(reverse.prev_prefix(), Some('+'));
        assert_eq!(reverse.next_prefix(), Some('-'));

        let link = topology.links().last().unwrap();
        assert_eq!(link.id, "+PEPT");
        assert_eq!(link.targets[0].resid1.as_deref(), Some("ALA"));
        assert_eq!(link.targets[0].resid2.as_deref(), Some("GLY"));
        assert_eq!(link.bonds[0].a1, "C");
        assert_eq!(link.bonds[0].a2, "N");
        assert_eq!(link.bonds[0].value, 1.33);
    }

    #[test]
    fn attached_reader_accumulates_parameter_and_topology_inputs() {
        let mut parameters = ParamDB::default();
        let mut topology = TopoDB::new();
        {
            let mut reader = CnsParFile::new();
            assert!(!reader.is_attached());
            reader.attach(&mut parameters, &mut topology);
            reader
                .read_str("nonb C 0.2 1.7 0.1 1.6\nbond C C 10.0 1.5\n")
                .unwrap();
            reader
                .read_str("resi LIG atom C1 type=C end atom C2 type=C end bond C1 C2 end")
                .unwrap();
            reader.detach();
            assert!(!reader.is_attached());
        }

        assert_eq!(parameters.nonbonded_count(), 1);
        assert_eq!(topology.residue("lig").unwrap().bond_count(), 1);
    }

    #[test]
    fn full_reader_parses_bundled_parameter_file() {
        let mut parameters = ParamDB::default();
        let mut topology = TopoDB::new();
        read_cns_toppar_str(QUEPTL_PRM, &mut parameters, &mut topology).unwrap();
        assert!(parameters.bond_count() > 100);
        assert!(parameters.angle_count() > 200);
        assert!(parameters.dihedral_count() > 50);
        assert!(parameters.improper_count() > 50);
        assert!(parameters.nonbonded_count() > 20);
        assert_eq!(topology.links().len(), 0);
    }

    #[test]
    fn full_reader_rejects_truncated_recognized_record() {
        let mut parameters = ParamDB::default();
        let mut topology = TopoDB::new();
        let error = read_cns_toppar_str("resi BAD atom C type=C", &mut parameters, &mut topology)
            .unwrap_err();
        assert!(error.message().contains("ATOM attribute or END"));
    }
}
