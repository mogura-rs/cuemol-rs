use crate::tmalign;
use cuemol_core::{CueMolError, Matrix4, Result};
use cuemol_molstr::{MolCoord, Selector, TopparManager};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct UsAlignResult {
    pub matrix: Matrix4,
    pub tm_reference: f64,
    pub tm_moving: f64,
    pub rmsd: f64,
    pub aligned_residues: usize,
    pub sequence_identity: f64,
    pub nucleic: bool,
}

pub fn superpose_usalign(
    reference: &MolCoord,
    reference_selection: &Selector,
    moving: &MolCoord,
    moving_selection: &Selector,
    pivot: &str,
) -> Result<UsAlignResult> {
    let automatic = pivot.is_empty() || pivot.eq_ignore_ascii_case("auto");
    let (rx, rn) = pivot_arrays(reference, reference_selection, pivot)?;
    let (mx, mn) = pivot_arrays(moving, moving_selection, pivot)?;
    let (matrix, scores) = align_arrays(&mx, &mn, &rx, &rn, if automatic { "C3'" } else { pivot })?;
    let mut transform = Matrix4::IDENTITY;
    for row in 0..3 {
        for col in 0..4 {
            transform.set_at(row + 1, col + 1, matrix[row * 4 + col])?;
        }
    }
    Ok(UsAlignResult {
        matrix: transform,
        tm_reference: scores[0],
        tm_moving: scores[1],
        rmsd: scores[2],
        aligned_residues: scores[3] as usize,
        sequence_identity: scores[4],
        nucleic: scores[5] != 0.,
    })
}

/// Pivot coordinates and residue name/nucleic-flag records for US-align.
pub(crate) fn pivot_arrays(
    mol: &MolCoord,
    sel: &Selector,
    pivot: &str,
) -> Result<(Vec<f64>, Vec<u8>)> {
    let automatic = pivot.is_empty() || pivot.eq_ignore_ascii_case("auto");
    let mut residues = BTreeMap::new();
    for atom in mol.atoms() {
        let entry = residues
            .entry((atom.chain_id.as_str(), atom.res_id, atom.ins_code))
            .or_insert((false, false));
        entry.0 |= atom.name == "CA";
        entry.1 |= matches!(atom.name.as_str(), "C3'" | "C4'");
    }
    let mut seen = BTreeSet::new();
    let mut xyz = Vec::new();
    let mut names = Vec::new();
    for atom in mol.atoms() {
        if !matches!(atom.alt_loc, None | Some(' ') | Some('A'))
            || !sel.matches_in_molecule(atom, mol)
        {
            continue;
        }
        let key = (atom.chain_id.as_str(), atom.res_id, atom.ins_code);
        let (has_ca, has_sugar) = residues[&key];
        let nucleic = TopparManager::is_nucl_acid(&atom.res_name) || (!has_ca && has_sugar);
        let atom_name = if automatic {
            if nucleic {
                "C3'"
            } else {
                "CA"
            }
        } else {
            pivot
        };
        if atom.name != atom_name || !seen.insert(key) {
            continue;
        }
        if ![atom.position.x, atom.position.y, atom.position.z]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(CueMolError::invalid_argument("non-finite atom position"));
        }
        xyz.extend([atom.position.x, atom.position.y, atom.position.z]);
        let mut name = [b' '; 4];
        let bytes = atom.res_name.as_bytes();
        let count = bytes.len().min(3);
        name[3 - count..3].copy_from_slice(&bytes[..count]);
        name[3] = u8::from(nucleic);
        names.extend(name);
    }
    if xyz.len() < 9 || xyz.len() / 3 > i32::MAX as usize {
        return Err(CueMolError::invalid_argument(
            "US-align needs at least three selected pivot residues",
        ));
    }
    Ok((xyz, names))
}

/// Aligns packed pivot arrays; returns the 3x4 row-major transform of the
/// moving structure and (TM-score by reference, TM-score by moving, RMSD,
/// aligned residues, sequence identity, nucleic flag).
pub(crate) fn align_arrays(
    mx: &[f64],
    mn: &[u8],
    rx: &[f64],
    rn: &[u8],
    pivot: &str,
) -> Result<([f64; 12], [f64; 6])> {
    if pivot.contains('\0') {
        return Err(CueMolError::invalid_argument("invalid pivot atom"));
    }
    if mn.len() < mx.len() / 3 * 4 || rn.len() < rx.len() / 3 * 4 {
        return Err(CueMolError::invalid_argument(
            "US-align residue records do not match the coordinates",
        ));
    }
    let result = tmalign::align(
        &tmalign::Chain::from_records(mx, mn),
        &tmalign::Chain::from_records(rx, rn),
        &tmalign::pad_atom_name(pivot),
    )
    .map_err(|message| CueMolError::invalid_argument(format!("US-align failed: {message}")))?;
    let mut matrix = [0.; 12];
    for row in 0..3 {
        matrix[row * 4..row * 4 + 3].copy_from_slice(&result.u[row]);
        matrix[row * 4 + 3] = result.t[row];
    }
    let scores = [
        result.tm_reference,
        result.tm_moving,
        result.rmsd,
        f64::from(result.aligned),
        result.sequence_identity,
        if result.nucleic { 1.0 } else { 0.0 },
    ];
    Ok((matrix, scores))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::Vector4;
    #[test]
    fn mixed_pivots_skip_altlocs_and_report_sequence_statistics() {
        let template = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n",
        )
        .unwrap()
        .atoms()[0]
            .clone();
        let mut reference = MolCoord::new();
        for i in 0..8 {
            let mut atom = template.clone();
            atom.serial = i + 1;
            atom.res_id = i + 1;
            if i >= 3 {
                atom.res_name = if i < 5 { "DG" } else { "M6A" }.into();
                atom.name = "C3'".into();
            }
            let t = i as f64;
            atom.position = Vector4::new3((t * 0.8).cos() * 4., (t * 0.8).sin() * 4., t * 1.5);
            reference.push_atom(atom);
        }
        let mut moving = reference.clone();
        for atom in moving.atoms_mut() {
            let p = atom.position;
            atom.position = Vector4::new3(-p.y + 8., p.x - 3., p.z + 2.);
        }
        let mut alternate = moving.atoms()[0].clone();
        alternate.serial = 100;
        alternate.alt_loc = Some('B');
        alternate.position = Vector4::new3(100., 100., 100.);
        moving.push_atom(alternate);
        let mut duplicate = moving.atoms()[1].clone();
        duplicate.serial = 101;
        duplicate.alt_loc = Some('A');
        duplicate.position = Vector4::new3(-100., 100., 0.);
        moving.push_atom(duplicate);
        let result =
            superpose_usalign(&reference, &Selector::All, &moving, &Selector::All, "AUTO").unwrap();
        assert_eq!(result.aligned_residues, 8);
        assert!(result.nucleic);
        assert!((result.sequence_identity - 1.).abs() < 1e-12);
        assert!((result.tm_reference - 1.).abs() < 1e-12 && (result.tm_moving - 1.).abs() < 1e-12);
        assert!(result.rmsd < 1e-6);
        for (r, m) in reference.atoms().iter().zip(moving.atoms()) {
            let p =
                result
                    .matrix
                    .mulvec(Vector4::new4(m.position.x, m.position.y, m.position.z, 1.));
            assert!(
                (p.x - r.position.x).abs() < 1e-6
                    && (p.y - r.position.y).abs() < 1e-6
                    && (p.z - r.position.z).abs() < 1e-6
            );
        }
        let explicit =
            superpose_usalign(&reference, &Selector::All, &moving, &Selector::All, "C3'").unwrap();
        assert_eq!(explicit.aligned_residues, 5);
        assert!(explicit.nucleic);
    }
}
