//! Template-based bonding parity, exercised on 1CRN (crambin).
//!
//! `MolCoord::apply_topology` (port of C++ `TopoBuilder`) builds connectivity from
//! the bundled residue-topology dictionary instead of a pure distance heuristic.
//! This test pins the observable consequences on a real protein:
//!   - canonical backbone/side-chain bonds are present,
//!   - bond *orders* come from the dictionary (a carbonyl C=O is a double bond —
//!     something a distance heuristic can never produce),
//!   - consecutive residues are joined by the peptide (poly) link,
//!   - the three disulfides are formed (here reinforced by the SSBOND records).
//!
//! Self-skips when the reference checkout is absent.

use cuemol_molstr::molecule::{Bond, BondOrder, MolCoord};
use cuemol_molstr::read_pdb_file;
use std::path::PathBuf;

fn reference_test_data(file_name: &str) -> Option<PathBuf> {
    let root = option_env!("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let path = root.join("tests/test_data").join(file_name);
    path.exists().then_some(path)
}

fn atom_index(mol: &MolCoord, res_id: i32, name: &str) -> Option<usize> {
    mol.atoms()
        .iter()
        .position(|a| a.res_id == res_id && a.name.trim().eq_ignore_ascii_case(name))
}

fn bond_between(mol: &MolCoord, a: usize, b: usize) -> Option<&Bond> {
    mol.bonds()
        .iter()
        .find(|bond| (bond.atom1 == a && bond.atom2 == b) || (bond.atom1 == b && bond.atom2 == a))
}

/// True iff residues `r1` and `r2` are joined by a bond between their named atoms.
fn residues_bonded(mol: &MolCoord, r1: i32, n1: &str, r2: i32, n2: &str) -> bool {
    match (atom_index(mol, r1, n1), atom_index(mol, r2, n2)) {
        (Some(a), Some(b)) => bond_between(mol, a, b).is_some(),
        _ => false,
    }
}

#[test]
fn topology_builds_backbone_orders_peptide_links_and_disulfides_on_1crn() {
    let Some(path) = reference_test_data("1CRN.pdb") else {
        return;
    };
    let mol = read_pdb_file(path).unwrap();

    // --- Backbone connectivity for a handful of residues (THR1, VAL2, ARG10) ---
    for res_id in [1, 2, 10] {
        let n = atom_index(&mol, res_id, "N").unwrap();
        let ca = atom_index(&mol, res_id, "CA").unwrap();
        let c = atom_index(&mol, res_id, "C").unwrap();
        let o = atom_index(&mol, res_id, "O").unwrap();
        assert!(bond_between(&mol, n, ca).is_some(), "res {res_id}: N-CA");
        assert!(bond_between(&mol, ca, c).is_some(), "res {res_id}: CA-C");
        let co = bond_between(&mol, c, o).expect("backbone C-O bond");
        // The carbonyl is a DOUBLE bond — proves the order came from the dictionary
        // template, not a distance heuristic (which yields only single bonds).
        assert_eq!(co.order, BondOrder::Double, "res {res_id}: C=O double bond");
    }

    // ARG10 side chain: the guanidinium has a C=N double bond (CZ=NH1 or CZ=NH2).
    let cz = atom_index(&mol, 10, "CZ").unwrap();
    let double_to_cz = mol
        .bonds()
        .iter()
        .any(|bond| (bond.atom1 == cz || bond.atom2 == cz) && bond.order == BondOrder::Double);
    assert!(double_to_cz, "ARG10 guanidinium C=N double bond");

    // --- Peptide (poly) links between consecutive residues ---
    for r in 1..=5 {
        assert!(
            residues_bonded(&mol, r, "C", r + 1, "N"),
            "peptide link C({r})-N({})",
            r + 1
        );
    }
    // ...and no spurious peptide bond between non-consecutive residues.
    assert!(
        !residues_bonded(&mol, 1, "C", 3, "N"),
        "no peptide bond across a residue"
    );

    // --- Disulfides: 1CRN's three SS bridges (SG-SG) ---
    for (a, b) in [(3, 40), (4, 32), (16, 26)] {
        assert!(
            residues_bonded(&mol, a, "SG", b, "SG"),
            "disulfide CYS{a}-CYS{b}"
        );
    }
    // CYS residues that are NOT bridged stay unbonded across their SG atoms.
    assert!(
        !residues_bonded(&mol, 3, "SG", 4, "SG"),
        "CYS3 and CYS4 are not a disulfide pair"
    );

    // The carbonyl oxygen has exactly one bond (to its own C) — no stray contacts.
    let o1 = atom_index(&mol, 1, "O").unwrap();
    let o1_degree = mol
        .bonds()
        .iter()
        .filter(|bond| bond.atom1 == o1 || bond.atom2 == o1)
        .count();
    assert_eq!(o1_degree, 1, "THR1 carbonyl O has a single bond");

    // C-terminal OXT (ASN46) is NOT in any residue template — it must still be
    // bonded to its backbone C by the second "bond non-bonded atoms" pass of
    // TopoBuilder::appTopoResid. (Regression guard: a template-only port leaves it
    // floating.)
    assert!(
        residues_bonded(&mol, 46, "C", 46, "OXT"),
        "C-terminal OXT bonded to backbone C"
    );

    // No heavy atom is left floating — every non-hydrogen atom has >= 1 bond.
    let degrees = {
        let mut d = vec![0usize; mol.atoms().len()];
        for bond in mol.bonds() {
            d[bond.atom1] += 1;
            d[bond.atom2] += 1;
        }
        d
    };
    for (i, atom) in mol.atoms().iter().enumerate() {
        if !atom.element.eq_ignore_ascii_case("H") {
            assert!(
                degrees[i] >= 1,
                "atom {} ({} {}{}) is floating",
                atom.name,
                atom.res_name,
                atom.res_id,
                atom.element
            );
        }
    }
}
