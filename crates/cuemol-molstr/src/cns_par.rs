//! CNS force-field parameter file reader — port of C++ molstr `CnsParFile` +
//! `ParamDB`.
//!
//! CueMol ships a CNS-format parameter file (`queptl.prm`, the Engh & Huber 1991
//! protein parameters) holding the molecular-mechanics force constants and ideal
//! geometry for each atom-type combination: bonds, angles, (proper) dihedrals,
//! impropers and non-bonded Lennard-Jones terms. `CnsParFile` parses it into a
//! [`ParamDB`] dictionary keyed by atom type.
//!
//! Scope (honest): this ports the *parameter-record* parsing — the data that
//! `queptl.prm` actually contains — and the [`ParamDB`] store + symmetric lookup.
//! It does NOT interpret the full CNS scripting grammar (`set`/`eval`/`remarks`/…
//! lines are skipped, as they carry no parameters), and nothing downstream yet
//! consumes the force constants (CueMol feeds them to energy minimisation, which
//! is not part of this viewer-focused port). Verification is therefore a *parser*
//! oracle: the bundled `queptl.prm`'s known values must parse back exactly.

use std::collections::HashMap;

use cuemol_core::{CueMolError, Result};

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

/// Molecular-mechanics parameter database, port of C++ `ParamDB`.
///
/// Keys are stored in the order they appear in the file (as in the C++ `std::map`
/// dicts); the symmetry of bonds/angles/dihedrals is resolved at lookup time by
/// trying the reversed key.
#[derive(Debug, Default)]
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
        search_dihe_list(&self.dihedrals, [a, b, c, d])
    }

    /// Look up an improper for `a`-`b`-`c`-`d` (same matching rules as dihedrals).
    pub fn search_improper(&self, a: &str, b: &str, c: &str, d: &str) -> Option<&DiheParam> {
        search_dihe_list(&self.impropers, [a, b, c, d])
    }

    pub fn search_nonbonded(&self, atom_type: &str) -> Option<&NonbParam> {
        self.nonbonded.get(atom_type)
    }
}

/// Does a stored dihedral/improper type tuple match the query (forward or
/// reversed), treating `X` as a wildcard?
fn dihe_matches(stored: &[String; 4], query: [&str; 4]) -> bool {
    let hit = |order: [usize; 4]| {
        (0..4).all(|i| stored[i] == "X" || stored[i].eq_ignore_ascii_case(query[order[i]]))
    };
    hit([0, 1, 2, 3]) || hit([3, 2, 1, 0])
}

fn search_dihe_list<'a>(list: &'a [DiheParam], query: [&str; 4]) -> Option<&'a DiheParam> {
    // Prefer a fully-specified (wildcard-free) match before falling back to one
    // that uses `X`.
    list.iter()
        .find(|p| !p.types.contains(&"X".to_string()) && dihe_matches(&p.types, query))
        .or_else(|| list.iter().find(|p| dihe_matches(&p.types, query)))
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
}
