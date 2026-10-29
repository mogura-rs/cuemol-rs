//! Crystallographic unit-cell parameters carried on a molecule (from PDB CRYST1
//! or mmCIF `_cell`/`_symmetry`). Kept as plain data in `cuemol-core` so the
//! foundation-layer `MolCoord` (in `cuemol-molstr`) can hold it without
//! depending on the higher `cuemol-symm` crate; the symm layer builds its
//! `CrystalInfo` from these raw values.

/// Unit cell lengths (Å), angles (degrees), and the space group, as parsed from
/// a coordinate file. The space group may be given by its Int-Tables number, its
/// Hermann–Mauguin name, or both — name → number resolution is the symm layer's
/// job (it owns the space-group table).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CellParams {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    /// Hermann–Mauguin space-group name (e.g. "P 1 21 1"), empty if absent.
    pub space_group_name: String,
    /// International Tables space-group number, if the file supplied it.
    pub space_group_number: Option<i32>,
}

impl CellParams {
    /// Build cell parameters with only the geometry; the space group is unset.
    pub fn new(a: f64, b: f64, c: f64, alpha: f64, beta: f64, gamma: f64) -> Self {
        Self {
            a,
            b,
            c,
            alpha,
            beta,
            gamma,
            space_group_name: String::new(),
            space_group_number: None,
        }
    }
}
