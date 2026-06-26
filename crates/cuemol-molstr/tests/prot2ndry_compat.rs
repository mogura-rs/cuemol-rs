//! DSSP secondary-structure parity, exercised on 1CRN (crambin).
//!
//! Verification scope (see also the source notes in `prot2ndry.rs`):
//! - The broad `secondary` class (helix/sheet/coil) is checked against 1CRN's
//!   deposited HELIX/SHEET records, an independent oracle. DSSP trims helix/strand
//!   termini relative to deposited assignments, which is expected.
//! - The exact residue ranges and the fine `secondary2` markers (Hs/H/He, Es/Ee,
//!   Gs/G/Ge) are pinned as a characterization of this faithful port of C++
//!   `MolCoord::calcProt2ndry`; full byte-for-byte parity with the C++ program
//!   output awaits a C++-derived oracle.
//!
//! Self-skips when the reference checkout is absent.

use cuemol_molstr::read_pdb_file;
use std::path::PathBuf;

fn reference_test_data(file_name: &str) -> Option<PathBuf> {
    let root = option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let path = root.join("tests/test_data").join(file_name);
    path.exists().then_some(path)
}

/// Per-residue (res_id, secondary, secondary2) for chain A, in residue order.
fn chain_a_secondary(path: PathBuf) -> Vec<(i32, String, String)> {
    let mol = read_pdb_file(path).unwrap();
    let mut out = Vec::new();
    for summary in mol.residue_summaries() {
        if summary.key.chain_id != "A" {
            continue;
        }
        let sindex = summary.key.script_index();
        let sec = mol
            .get_residue_prop_str("A", &sindex, "secondary")
            .unwrap()
            .unwrap_or("")
            .to_string();
        let sec2 = mol
            .get_residue_prop_str("A", &sindex, "secondary2")
            .unwrap()
            .unwrap_or("")
            .to_string();
        out.push((summary.key.res_id, sec, sec2));
    }
    out
}

/// Compact broad-SS glyph: H = helix, E = sheet, '.' = coil.
fn broad_glyph(secondary: &str) -> char {
    match secondary {
        "helix" => 'H',
        "sheet" => 'E',
        _ => '.',
    }
}

#[test]
fn dssp_assigns_1crn_secondary_structure() {
    let Some(path) = reference_test_data("1CRN.pdb") else {
        return;
    };
    let residues = chain_a_secondary(path);

    // 1CRN is a single 46-residue chain.
    assert_eq!(residues.len(), 46);
    assert_eq!(residues.first().map(|r| r.0), Some(1));
    assert_eq!(residues.last().map(|r| r.0), Some(46));

    let by_id = |id: i32| residues.iter().find(|r| r.0 == id).unwrap();

    // --- Oracle checks (vs deposited HELIX/SHEET) -------------------------
    // Deposited HELIX 1: 7-19, HELIX 2: 23-30. Helix cores must be helix.
    for id in 7..=18 {
        assert_eq!(by_id(id).1, "helix", "residue {id} expected helix");
    }
    for id in 23..=30 {
        assert_eq!(by_id(id).1, "helix", "residue {id} expected helix");
    }
    // Deposited SHEET: strand 1-4 paired antiparallel with strand 32-35. Both
    // strands must be detected as sheet (exercises bridge + linkStrands).
    for id in [2, 3] {
        assert_eq!(by_id(id).1, "sheet", "strand-1 residue {id} expected sheet");
    }
    for id in [33, 34] {
        assert_eq!(by_id(id).1, "sheet", "strand-2 residue {id} expected sheet");
    }
    // The long loop between the helices is coil.
    for id in [20, 21, 22] {
        assert_eq!(by_id(id).1, "", "loop residue {id} expected coil");
    }

    // --- Characterization (pins the faithful-port output) ----------------
    let broad: String = residues.iter().map(|r| broad_glyph(&r.1)).collect();
    assert_eq!(broad, ".EE...HHHHHHHHHHHH....HHHHHHHH..EE.......HHH..");

    // Fine markers at boundaries.
    assert_eq!(by_id(2).2, "Es");
    assert_eq!(by_id(3).2, "Ee");
    assert_eq!(by_id(7).2, "Hs");
    assert_eq!(by_id(18).2, "He");
    assert_eq!(by_id(33).2, "Es");
    assert_eq!(by_id(42).2, "Gs"); // short 3-10 helix near the C-terminus
}

/// Broad-SS glyph string for chain A after computing `calc_prot_2ndry2` with the
/// given gap-fill threshold (records-off load avoids the auto DSSP pass).
fn broad_after_con_helices(path: &std::path::Path, dhangl: f64) -> String {
    use cuemol_molstr::{pdb::PdbReadOptions, read_pdb_file_with_options};
    let opts = PdbReadOptions {
        build_secondary: false,
        ..PdbReadOptions::default()
    };
    let mut mol = read_pdb_file_with_options(path, opts).unwrap();
    mol.calc_prot_2ndry2(false, dhangl);
    mol.residue_summaries()
        .iter()
        .filter(|s| s.key.chain_id == "A")
        .map(|s| {
            broad_glyph(
                mol.get_residue_prop_str("A", &s.key.script_index(), "secondary")
                    .unwrap()
                    .unwrap_or(""),
            )
        })
        .collect()
}

#[test]
fn calc_prot_2ndry2_gap_fill_merges_helices() {
    let Some(path) = reference_test_data("1CRN.pdb") else {
        return;
    };

    // At the default 60° threshold the loop between the two alpha-helices is too
    // bent to fill: the result matches the plain (non-gap-filled) assignment.
    assert_eq!(
        broad_after_con_helices(&path, 60.0),
        ".EE...HHHHHHHHHHHH....HHHHHHHH..EE.......HHH.."
    );

    // With a permissive threshold, con_helices bridges helix-1 (7-18) and
    // helix-2 (23-30) across the 19-22 gap (now all helix) — but the E-guard
    // stops it from merging across the 33-34 strand into the 42-44 3-10 helix.
    assert_eq!(
        broad_after_con_helices(&path, 360.0),
        ".EE...HHHHHHHHHHHHHHHHHHHHHHHH..EE.......HHH.."
    );
}

#[test]
fn prot2ndry_set_edits_and_reapplies_structure() {
    use cuemol_molstr::Prot2ndrySet;
    let Some(path) = reference_test_data("1CRN.pdb") else {
        return;
    };
    let mut mol = read_pdb_file(path).unwrap();

    // Snapshot the DSSP assignment, then force the 19-22 loop (between the two
    // alpha-helices) to alpha. The two helices and the gap fuse into one 7-30 run.
    let mut ss = Prot2ndrySet::create(&mol);
    ss.set_type(2, "A", 19, 22);
    ss.apply_to(&mut mol);

    let sec = |id: i32| {
        mol.get_residue_prop_str("A", &id.to_string(), "secondary")
            .unwrap()
            .unwrap_or("")
            .to_string()
    };
    let sec2 = |id: i32| {
        mol.get_residue_prop_str("A", &id.to_string(), "secondary2")
            .unwrap()
            .unwrap_or("")
            .to_string()
    };

    // The formerly-coil loop is now helix, fused into a single 7-30 run.
    for id in 19..=22 {
        assert_eq!(sec(id), "helix", "residue {id} should now be helix");
    }
    for id in 7..=30 {
        assert_eq!(
            sec(id),
            "helix",
            "residue {id} should be in the fused helix"
        );
    }
    // Start/end markers track the new run boundaries (start XOR end).
    assert_eq!(sec2(7), "Hs");
    assert_eq!(sec2(18), "H"); // was the old helix-1 end "He", now interior
    assert_eq!(sec2(30), "He");

    // Untouched elements are preserved: the beta-sheet and the 3-10 helix.
    assert_eq!(sec(2), "sheet");
    assert_eq!(sec(33), "sheet");
    assert_eq!(sec(42), "helix"); // 3-10 helix, unchanged
    assert_eq!(sec2(42), "Gs");
}
