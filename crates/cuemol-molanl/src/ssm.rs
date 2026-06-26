//! Native SSM-style structure superposition — a functional reimplementation of
//! the vendored CCP4 `ssmlib` that C++ `MolAnlManager::superposeSSM` uses.
//!
//! Unlike LSQ (which needs a pre-supplied 1:1 atom correspondence), SSM aligns
//! two structures WITHOUT one: it matches secondary-structure elements (SSEs) by
//! geometry, derives a transform from the matched SSEs, then refines it at the
//! residue level. This is a NATIVE reimplementation of that FUNCTIONALITY — the
//! same native-vs-vendored approach the project takes for the grid SES vs BALL —
//! NOT a byte-parity port of ssmlib's graph-isomorphism internals.
//!
//! Verification uses the order-independent "shuffle oracle": aligning a structure
//! to a rotated + atom-SHUFFLED copy of itself must recover the correspondence
//! and the inverse transform (RMSD ~ 0) despite the scrambled atom order — which
//! a mere index-zip LSQ could never do.

use crate::{rigid_fit_matrix, rmsd_for_pairs};
use cuemol_core::{CueMolError, Matrix4, Result, Vector4};
use cuemol_molstr::{MolCoord, ResidueKey, Selector};
use std::collections::BTreeSet;

/// Minimum consecutive residues to count as an SSE.
const MIN_SSE_LEN: usize = 3;
/// A CA is "matched" to a reference CA within this distance (Å) after transform.
const MATCH_CUTOFF: f64 = 3.0;
/// Tolerance (Å) when matching SSE-centroid pairwise distances during seeding.
const DIST_TOL: f64 = 2.0;
/// ICP refinement iteration cap.
const MAX_ICP_ITERS: usize = 12;
/// Upper bound on seed triples evaluated (bounds the O(n³·m³) search).
const MAX_SEEDS: usize = 20_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SsType {
    Helix,
    Sheet,
    Coil,
}

struct Ca {
    pos: Vector4,
    ss: SsType,
    chain: String,
    resid: i32,
}

struct Sse {
    /// First member CA — together with `end` these break the coplanarity of bare
    /// centroids so a 3-SSE fit is not reflection-ambiguous.
    start: Vector4,
    centroid: Vector4,
    /// Last member CA.
    end: Vector4,
    /// Number of member CAs — used (with type + inter-centroid distances) to
    /// disambiguate the SSE correspondence.
    len: usize,
    ss: SsType,
}

fn apply(m: &Matrix4, p: Vector4) -> Vector4 {
    let t = m.transform_point(Vector4::new4(p.x, p.y, p.z, 1.0));
    Vector4::new3(t.x, t.y, t.z)
}

fn dist(a: Vector4, b: Vector4) -> f64 {
    (a - b).length3d()
}

fn centroid_of<I: Iterator<Item = Vector4>>(it: I) -> Vector4 {
    let mut sum = Vector4::new3(0.0, 0.0, 0.0);
    let mut n = 0.0;
    for p in it {
        sum = sum + p;
        n += 1.0;
    }
    if n == 0.0 {
        sum
    } else {
        sum.scale(1.0 / n)
    }
}

/// Selected CA atoms in chain/residue order, tagged with secondary structure.
fn extract_cas(mol: &MolCoord, selector: &Selector) -> Vec<Ca> {
    let selected: BTreeSet<usize> = mol.select_indices(selector).into_iter().collect();
    let mut cas = Vec::new();
    for (i, atom) in mol.atoms().iter().enumerate() {
        if !selected.contains(&i) || atom.name != "CA" {
            continue;
        }
        let sindex = ResidueKey::new(&atom.chain_id, atom.res_id, atom.ins_code).script_index();
        let ss = match mol
            .get_residue_prop_str(&atom.chain_id, &sindex, "secondary")
            .ok()
            .flatten()
        {
            Some(s) if s.eq_ignore_ascii_case("helix") => SsType::Helix,
            Some(s) if s.eq_ignore_ascii_case("sheet") || s.eq_ignore_ascii_case("strand") => {
                SsType::Sheet
            }
            _ => SsType::Coil,
        };
        cas.push(Ca {
            pos: atom.position,
            ss,
            chain: atom.chain_id.clone(),
            resid: atom.res_id,
        });
    }
    cas
}

/// Group consecutive same-chain, same-SS, consecutive-residue CAs (helix/sheet
/// only, length >= MIN_SSE_LEN) into SSEs represented by their centroid + type.
fn extract_sses(cas: &[Ca]) -> Vec<Sse> {
    let mut sses = Vec::new();
    let mut i = 0;
    while i < cas.len() {
        let ss = cas[i].ss;
        if ss == SsType::Coil {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < cas.len()
            && cas[j].ss == ss
            && cas[j].chain == cas[i].chain
            && cas[j].resid == cas[j - 1].resid + 1
        {
            j += 1;
        }
        if j - i >= MIN_SSE_LEN {
            sses.push(Sse {
                start: cas[i].pos,
                centroid: centroid_of(cas[i..j].iter().map(|c| c.pos)),
                end: cas[j - 1].pos,
                len: j - i,
                ss,
            });
        }
        i = j;
    }
    sses
}

/// Count moving CAs that land within `MATCH_CUTOFF` of some reference CA after
/// applying `m` (the alignment score).
fn count_inliers(m: &Matrix4, ref_cas: &[Ca], mov_cas: &[Ca]) -> usize {
    mov_cas
        .iter()
        .filter(|mc| {
            let p = apply(m, mc.pos);
            ref_cas
                .iter()
                .any(|rc| dist(rc.pos, p) <= MATCH_CUTOFF)
        })
        .count()
}

/// Pairs `(reference_pos, moving_original_pos)` formed by mapping each moving CA
/// (after `m`) to its nearest reference CA within `MATCH_CUTOFF`.
fn build_pairs(m: &Matrix4, ref_cas: &[Ca], mov_cas: &[Ca]) -> Vec<(Vector4, Vector4)> {
    let mut pairs = Vec::new();
    for mc in mov_cas {
        let p = apply(m, mc.pos);
        let mut best: Option<(f64, Vector4)> = None;
        for rc in ref_cas {
            let d = dist(rc.pos, p);
            if d <= MATCH_CUTOFF && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best = Some((d, rc.pos));
            }
        }
        if let Some((_, ref_pos)) = best {
            pairs.push((ref_pos, mc.pos));
        }
    }
    pairs
}

/// Two SSEs may correspond if they share a secondary-structure type and have a
/// near-equal length (member count within 1).
fn sse_compatible(a: &Sse, b: &Sse) -> bool {
    a.ss == b.ss && a.len.abs_diff(b.len) <= 1
}

/// Seed transform: search SSE-centroid triples for a geometry-consistent
/// correspondence (matching types + preserved pairwise distances), build a
/// candidate transform from each, and keep the one with the most CA inliers.
fn seed_transform(ref_sses: &[Sse], mov_sses: &[Sse], ref_cas: &[Ca], mov_cas: &[Ca]) -> Option<Matrix4> {
    let n = ref_sses.len();
    let m = mov_sses.len();
    let mut best: Option<(usize, Matrix4)> = None;
    let mut tried = 0usize;

    for a in 0..n {
        for b in (a + 1)..n {
            for c in (b + 1)..n {
                let rab = dist(ref_sses[a].centroid, ref_sses[b].centroid);
                let rac = dist(ref_sses[a].centroid, ref_sses[c].centroid);
                let rbc = dist(ref_sses[b].centroid, ref_sses[c].centroid);
                for i in 0..m {
                    if !sse_compatible(&mov_sses[i], &ref_sses[a]) {
                        continue;
                    }
                    for j in 0..m {
                        if j == i || !sse_compatible(&mov_sses[j], &ref_sses[b]) {
                            continue;
                        }
                        if (dist(mov_sses[i].centroid, mov_sses[j].centroid) - rab).abs() > DIST_TOL
                        {
                            continue;
                        }
                        for k in 0..m {
                            if k == i || k == j || !sse_compatible(&mov_sses[k], &ref_sses[c]) {
                                continue;
                            }
                            if (dist(mov_sses[i].centroid, mov_sses[k].centroid) - rac).abs()
                                > DIST_TOL
                                || (dist(mov_sses[j].centroid, mov_sses[k].centroid) - rbc).abs()
                                    > DIST_TOL
                            {
                                continue;
                            }
                            // Fit on start/centroid/end of each matched SSE (9
                            // non-coplanar points) so the rotation is fully
                            // determined — bare centroids are coplanar and admit
                            // a reflected solution.
                            let pairs = [
                                (ref_sses[a].start, mov_sses[i].start),
                                (ref_sses[a].centroid, mov_sses[i].centroid),
                                (ref_sses[a].end, mov_sses[i].end),
                                (ref_sses[b].start, mov_sses[j].start),
                                (ref_sses[b].centroid, mov_sses[j].centroid),
                                (ref_sses[b].end, mov_sses[j].end),
                                (ref_sses[c].start, mov_sses[k].start),
                                (ref_sses[c].centroid, mov_sses[k].centroid),
                                (ref_sses[c].end, mov_sses[k].end),
                            ];
                            if let Ok(matrix) = rigid_fit_matrix(&pairs) {
                                let score = count_inliers(&matrix, ref_cas, mov_cas);
                                if best.as_ref().is_none_or(|(s, _)| score > *s) {
                                    best = Some((score, matrix));
                                }
                            }
                            tried += 1;
                            if tried >= MAX_SEEDS {
                                return best.map(|(_, mx)| mx);
                            }
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, mx)| mx)
}

/// Iterative closest-point refinement: re-pair CAs under the current transform
/// and refit until the matched-pair RMSD stops improving.
fn icp_refine(seed: Matrix4, ref_cas: &[Ca], mov_cas: &[Ca]) -> (Matrix4, Vec<(Vector4, Vector4)>) {
    let mut matrix = seed;
    let mut pairs = build_pairs(&matrix, ref_cas, mov_cas);
    let mut prev_rmsd = if pairs.is_empty() {
        f64::INFINITY
    } else {
        rmsd_for_pairs(&pairs, matrix)
    };
    for _ in 0..MAX_ICP_ITERS {
        let candidate = build_pairs(&matrix, ref_cas, mov_cas);
        if candidate.len() < 3 {
            break;
        }
        let Ok(next) = rigid_fit_matrix(&candidate) else {
            break;
        };
        let rmsd = rmsd_for_pairs(&candidate, next);
        matrix = next;
        pairs = candidate;
        if (prev_rmsd - rmsd).abs() < 1.0e-9 {
            break;
        }
        prev_rmsd = rmsd;
    }
    (matrix, pairs)
}

/// Align `moving` onto `reference` by secondary-structure matching. Returns the
/// transform (moving → reference), the matched-CA RMSD, and the number of fitted
/// residues. Errs when either structure lacks >= 3 SSEs or no consistent SSE
/// correspondence is found (the caller may then fall back to LSQ).
pub(crate) fn superpose_ssm_native(
    reference: &MolCoord,
    reference_selector: &Selector,
    moving: &MolCoord,
    moving_selector: &Selector,
) -> Result<(Matrix4, f64, usize)> {
    let ref_cas = extract_cas(reference, reference_selector);
    let mov_cas = extract_cas(moving, moving_selector);
    if ref_cas.len() < MIN_SSE_LEN || mov_cas.len() < MIN_SSE_LEN {
        return Err(CueMolError::invalid_argument("SSM: too few CA atoms"));
    }
    let ref_sses = extract_sses(&ref_cas);
    let mov_sses = extract_sses(&mov_cas);
    if ref_sses.len() < 3 || mov_sses.len() < 3 {
        return Err(CueMolError::invalid_argument(
            "SSM: need at least 3 secondary-structure elements in each structure",
        ));
    }
    let seed = seed_transform(&ref_sses, &mov_sses, &ref_cas, &mov_cas)
        .ok_or_else(|| CueMolError::invalid_argument("SSM: no consistent SSE correspondence"))?;
    let (matrix, pairs) = icp_refine(seed, &ref_cas, &mov_cas);
    if pairs.is_empty() {
        return Err(CueMolError::invalid_argument(
            "SSM: no matched residues after refinement",
        ));
    }
    let rmsd = rmsd_for_pairs(&pairs, matrix);
    Ok((matrix, rmsd, pairs.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_molstr::Atom;

    fn ca_atom(serial: i32, chain: &str, resid: i32, x: f64, y: f64, z: f64) -> Atom {
        Atom {
            serial,
            name: "CA".to_string(),
            alt_loc: None,
            res_name: "ALA".to_string(),
            chain_id: chain.to_string(),
            res_id: resid,
            ins_code: None,
            position: Vector4::new3(x, y, z),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "C".to_string(),
            formal_charge: None,
            int_props: Default::default(),
        }
    }

    /// Helix anchor origins (well-separated, non-collinear) and DISTINCT lengths
    /// so the SSE correspondence is unambiguous (type + length + geometry).
    const HELIX_ORIGINS: [(f64, f64, f64); 3] =
        [(0.0, 0.0, 0.0), (12.0, 1.0, 0.0), (3.0, 11.0, 2.0)];
    const HELIX_LENGTHS: [usize; 3] = [4, 6, 8];

    /// Build a structure of three helices of distinct lengths, with the
    /// `secondary="helix"` prop set on every residue.
    fn three_helix_structure() -> MolCoord {
        let mut mol = MolCoord::new();
        let mut serial = 1;
        for (h, (&(ox, oy, oz), &len)) in
            HELIX_ORIGINS.iter().zip(HELIX_LENGTHS.iter()).enumerate()
        {
            for r in 0..len {
                let resid = (h as i32) * 100 + r as i32 + 1;
                mol.push_atom(ca_atom(
                    serial,
                    "A",
                    resid,
                    ox + 0.3 * r as f64,
                    oy + 0.2 * r as f64,
                    oz + 1.5 * r as f64,
                ));
                serial += 1;
            }
        }
        set_all_helix(&mut mol);
        mol
    }

    fn set_all_helix(mol: &mut MolCoord) {
        for atom in mol.atoms().to_vec() {
            let sindex = ResidueKey::new("A", atom.res_id, None).script_index();
            mol.set_residue_prop_str("A", &sindex, "secondary", "helix")
                .unwrap();
        }
    }

    /// Rotate (about z by `ang`) + translate, and emit the three helix blocks in
    /// a PERMUTED order with fresh, gap-separated residue numbers. Each block
    /// stays internally residue-ordered (so SSEs are still extracted), but the
    /// SSE order differs from the reference — so a correct alignment must match
    /// SSEs by GEOMETRY, not by index. Geometrically identical to `mol`.
    fn rotated_reordered(mol: &MolCoord, ang: f64, tx: f64, ty: f64, tz: f64) -> MolCoord {
        let atoms: Vec<Atom> = mol.atoms().to_vec();
        // Split into the three helix blocks by their (distinct) lengths.
        let mut blocks: Vec<Vec<Atom>> = Vec::new();
        let mut idx = 0;
        for &len in &HELIX_LENGTHS {
            blocks.push(atoms[idx..idx + len].to_vec());
            idx += len;
        }
        let order = [2usize, 0, 1]; // permuted block order
        let (s, c) = ang.sin_cos();
        let mut out = MolCoord::new();
        let mut serial = 1;
        for (emit, &bi) in order.iter().enumerate() {
            // Gap-separated residue base per emitted block so blocks don't merge.
            let base = emit as i32 * 100 + 1;
            for (r, a) in blocks[bi].iter().enumerate() {
                let p = a.position;
                let rx = c * p.x - s * p.y + tx;
                let ry = s * p.x + c * p.y + ty;
                let rz = p.z + tz;
                out.push_atom(ca_atom(serial, "A", base + r as i32, rx, ry, rz));
                serial += 1;
            }
        }
        set_all_helix(&mut out);
        out
    }

    #[test]
    fn ssm_recovers_transform_from_a_rotated_shuffled_copy() {
        // The shuffle oracle: align a structure to a rotated + atom-reordered
        // copy of itself. SSM must recover the correspondence (sequence/order
        // independent) and the inverse transform => RMSD ~ 0.
        let reference = three_helix_structure();
        let moving = rotated_reordered(&reference, 0.7, 5.0, -3.0, 2.0);

        let (matrix, rmsd, nfit) =
            superpose_ssm_native(&reference, &Selector::All, &moving, &Selector::All)
                .expect("SSM should align the structures");
        assert!(rmsd < 1.0e-6, "rmsd={rmsd}");
        assert_eq!(nfit, reference.atoms().len(), "all residues fitted");

        // Applying the transform maps a moving CA onto its (geometric)
        // reference partner — independent of the scrambled order.
        let mv0 = moving.atoms()[0].position;
        let mapped = apply(&matrix, mv0);
        assert!(
            reference
                .atoms()
                .iter()
                .any(|a| dist(a.position, mapped) < 1.0e-6),
            "transformed moving atom lands on a reference atom"
        );
    }

    #[test]
    fn rigid_fit_recovers_rotation_and_translation() {
        // Regression guard for the Horn-quaternion fix: a real rotation (not just
        // translation, which the LSQ tests covered) must be recovered exactly.
        // 4 non-coplanar points, rotate about z by 0.7 + translate, fit.
        let pts = [
            Vector4::new3(0.0, 0.0, 0.0),
            Vector4::new3(5.0, 0.0, 0.0),
            Vector4::new3(0.0, 4.0, 0.0),
            Vector4::new3(1.0, 1.0, 6.0),
        ];
        let (s, c) = 0.7_f64.sin_cos();
        let pairs: Vec<(Vector4, Vector4)> = pts
            .iter()
            .map(|p| {
                let mov = Vector4::new3(c * p.x - s * p.y + 5.0, s * p.x + c * p.y - 3.0, p.z + 2.0);
                (*p, mov) // (reference=original, moving=rotated)
            })
            .collect();
        let m = rigid_fit_matrix(&pairs).unwrap();
        for (refp, movp) in &pairs {
            let got = apply(&m, *movp);
            assert!(dist(*refp, got) < 1.0e-6, "fit off: {:?} vs {:?}", refp, got);
        }
    }

    #[test]
    fn ssm_errors_without_enough_secondary_structure() {
        // Two bare CAs, no SSEs -> SSM cannot run (caller falls back to LSQ).
        let mut mol = MolCoord::new();
        mol.push_atom(ca_atom(1, "A", 1, 0.0, 0.0, 0.0));
        mol.push_atom(ca_atom(2, "A", 2, 1.0, 0.0, 0.0));
        let res = superpose_ssm_native(&mol, &Selector::All, &mol, &Selector::All);
        assert!(res.is_err());
    }
}
