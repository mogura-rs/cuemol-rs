//! PyMOL `.pse` molecule extraction: traverse the pickle session tree (produced
//! by the pickle VM) into [`MolCoord`] molecules. Ports the ChemPy object
//! traversal of C++ `importers::PSEFileReader`.
//!
//! The version-specific magic indices below were read directly from a real
//! PyMOL-3 `.pse` (`tests/fixtures/mol3.pse`, which is the verification oracle —
//! a known 3-atom molecule). So this is verified against genuine PyMOL output,
//! not a synthetic guess.
//!
//! C++ PARITY (verified against `PSEFileReader.cpp`, NOT limitations of this
//! port — the reference behaves identically):
//! - Atom field indices match the C++ `AT_*` constants exactly (CHAIN=1, RESI=3,
//!   RESN=5, NAME=6, ELEM=7, BFAC=14, OCC=15).
//! - Only the first coordinate set is read (C++ line 431: `getList(4)->getList(0)`;
//!   its `for i<ncset` loop is debug-only). Multi-state `.pse` keep their first
//!   state in BOTH implementations.
//! - The session `"version"` field is not branched on — C++ only LOGS it
//!   (`int ver = ...; LOG_DPRINTLN("PyMOL version %d", ver)`), using one atom
//!   layout for all versions, exactly as here.

use std::collections::BTreeMap;

use cuemol_core::Vector4;
use cuemol_molstr::{Atom, BondOrder, MolCoord};

use crate::PickleValue;

// `names` list element layout: [name, type, visible, repOn, extra, data, ...].
const NAME_TYPE: usize = 1;
const NAME_DATA: usize = 5;
const OBJ_TYPE_MOLECULE: i64 = 0;

// Molecule `data` block layout.
const DATA_NATOMS: usize = 3;
const DATA_COORDSETS: usize = 4;
const DATA_BONDS: usize = 6;
const DATA_ATOMS: usize = 7;
// Coordset layout (C++ CT_* constants): coordset[2] = flat [x,y,z,...] in
// COORD-index order; coordset[3] = idxToAtm, mapping each coord index to its
// atom id (a permutation — the atom list and coordinate array can differ in
// order after a session is edited, so coords MUST be routed through it).
const CS_COORDS: usize = 2;
const CS_IDXTOATM: usize = 3;

// Atom-record (ChemPy) field indices.
const AT_CHAIN: usize = 1;
const AT_RESI: usize = 3;
const AT_RESN: usize = 5;
const AT_NAME: usize = 6;
const AT_ELEM: usize = 7;
const AT_BFAC: usize = 14;
const AT_OCC: usize = 15;

/// Extract every molecule object from a parsed `.pse` session tree.
pub fn extract_pse_molecules(tree: &PickleValue) -> Vec<MolCoord> {
    let mut molecules = Vec::new();
    let Some(names) = tree.get("names").and_then(PickleValue::as_list) else {
        return molecules;
    };
    for element in names {
        // `None` placeholders and non-molecule objects (type != 0) are skipped.
        if element.index(NAME_TYPE).and_then(PickleValue::as_i64) != Some(OBJ_TYPE_MOLECULE) {
            continue;
        }
        if let Some(data) = element.index(NAME_DATA).and_then(PickleValue::as_list) {
            if let Some(molecule) = extract_one(data) {
                molecules.push(molecule);
            }
        }
    }
    molecules
}

/// Split a PyMOL residue identifier (`"100"`, `"100A"`) into number + insertion.
fn parse_resi(resi: &str) -> (i32, Option<char>) {
    let resi = resi.trim();
    match resi.find(|c: char| !c.is_ascii_digit() && c != '-') {
        Some(0) => (0, resi.chars().next()),
        Some(idx) => (resi[..idx].parse().unwrap_or(0), resi[idx..].chars().next()),
        None => (resi.parse().unwrap_or(0), None),
    }
}

fn extract_one(data: &[PickleValue]) -> Option<MolCoord> {
    let natoms = data.get(DATA_NATOMS)?.as_i64()? as usize;
    let coordset = data.get(DATA_COORDSETS)?.index(0)?;
    let coords = coordset.index(CS_COORDS)?.as_list()?;
    let atoms = data.get(DATA_ATOMS)?.as_list()?;

    // Route each coord block to its atom via idxToAtm (coord index i -> atom id),
    // so reordered sessions keep coordinates attached to the right atom. When the
    // map is absent we fall back to identity (coord i -> atom i).
    let mut positions = vec![Vector4::new3(0.0, 0.0, 0.0); natoms];
    let idx_to_atm = coordset.index(CS_IDXTOATM).and_then(PickleValue::as_list);
    let ncoord = idx_to_atm.map(<[_]>::len).unwrap_or(natoms);
    for i in 0..ncoord {
        let aid = match idx_to_atm {
            Some(map) => map.get(i)?.as_i64()? as usize,
            None => i,
        };
        let x = coords.get(3 * i)?.as_f64()?;
        let y = coords.get(3 * i + 1)?.as_f64()?;
        let z = coords.get(3 * i + 2)?.as_f64()?;
        if let Some(slot) = positions.get_mut(aid) {
            *slot = Vector4::new3(x, y, z);
        }
    }

    let mut mol = MolCoord::new();
    for (atom_pv, &position) in atoms.iter().zip(&positions) {
        let a = atom_pv.as_list()?;
        let field_str = |idx: usize| a.get(idx).and_then(PickleValue::as_str).unwrap_or("");
        let chain = field_str(AT_CHAIN).to_string();
        let resn = field_str(AT_RESN).to_string();
        let name = field_str(AT_NAME).to_string();
        let element = field_str(AT_ELEM).to_string();
        let (res_id, ins_code) = parse_resi(field_str(AT_RESI));
        let b_factor = a.get(AT_BFAC).and_then(PickleValue::as_f64).unwrap_or(0.0) as f32;
        let occupancy = a.get(AT_OCC).and_then(PickleValue::as_f64).unwrap_or(1.0) as f32;
        let atom = Atom {
            serial: 0,
            name,
            alt_loc: None,
            res_name: resn.clone(),
            chain_id: chain.clone(),
            res_id,
            ins_code,
            position,
            occupancy,
            b_factor,
            element,
            formal_charge: None,
            int_props: BTreeMap::new(),
        };
        mol.append_atom1(atom, chain, res_id, resn);
    }

    // Bonds: data[6] = list of [aid1, aid2, order, ...].
    if let Some(bonds) = data.get(DATA_BONDS).and_then(PickleValue::as_list) {
        for bond in bonds {
            let Some(fields) = bond.as_list() else {
                continue;
            };
            let (Some(a1), Some(a2)) = (
                fields.first().and_then(PickleValue::as_i64),
                fields.get(1).and_then(PickleValue::as_i64),
            ) else {
                continue;
            };
            let order = fields.get(2).and_then(PickleValue::as_i64).unwrap_or(1);
            if (a1 as usize) < mol.atom_count() && (a2 as usize) < mol.atom_count() {
                mol.add_bond(a1 as usize, a2 as usize, BondOrder::from_sdf(order as i32));
            }
        }
    }
    Some(mol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_pickle;

    #[test]
    fn parse_resi_splits_number_and_insertion() {
        assert_eq!(parse_resi("100"), (100, None));
        assert_eq!(parse_resi("100A"), (100, Some('A')));
        assert_eq!(parse_resi("-5"), (-5, None));
    }

    #[test]
    fn extracts_no_molecules_from_an_empty_tree() {
        let tree = parse_pickle(b"}.").unwrap(); // empty dict + STOP
        assert!(extract_pse_molecules(&tree).is_empty());
    }

    // Build a 3-atom molecule session tree with an explicit idxToAtm permutation.
    fn molecule_tree(idx_to_atm: Vec<i64>) -> PickleValue {
        let atom = |name: &str| {
            let mut f = vec![PickleValue::Int(0); 16];
            f[AT_CHAIN] = PickleValue::Str("A".into());
            f[AT_RESI] = PickleValue::Str("1".into());
            f[AT_RESN] = PickleValue::Str("ALA".into());
            f[AT_NAME] = PickleValue::Str(name.into());
            f[AT_ELEM] = PickleValue::Str("C".into());
            f[AT_BFAC] = PickleValue::Float(0.0);
            f[AT_OCC] = PickleValue::Float(1.0);
            PickleValue::List(f)
        };
        // coords in COORD-index order: index 0 -> x=10, 1 -> 20, 2 -> 30.
        let coords = PickleValue::List(
            [10.0, 0.0, 0.0, 20.0, 0.0, 0.0, 30.0, 0.0, 0.0]
                .iter()
                .map(|v| PickleValue::Float(*v))
                .collect(),
        );
        let mut coordset = vec![PickleValue::None; 6];
        coordset[CS_COORDS] = coords;
        coordset[CS_IDXTOATM] =
            PickleValue::List(idx_to_atm.into_iter().map(PickleValue::Int).collect());
        let mut data = vec![PickleValue::None; 8];
        data[DATA_NATOMS] = PickleValue::Int(3);
        data[DATA_COORDSETS] = PickleValue::List(vec![PickleValue::List(coordset)]);
        data[DATA_BONDS] = PickleValue::List(vec![]);
        data[DATA_ATOMS] = PickleValue::List(vec![atom("A0"), atom("A1"), atom("A2")]);
        let mut name_elem = vec![PickleValue::None; 6];
        name_elem[NAME_TYPE] = PickleValue::Int(OBJ_TYPE_MOLECULE);
        name_elem[NAME_DATA] = PickleValue::List(data);
        PickleValue::Dict(vec![(
            PickleValue::Str("names".into()),
            PickleValue::List(vec![PickleValue::None, PickleValue::List(name_elem)]),
        )])
    }

    #[test]
    fn idx_to_atm_routes_coordinates_to_the_right_atom() {
        // Permutation [2,0,1]: coord index 0 -> atom 2, 1 -> atom 0, 2 -> atom 1.
        // So atom A0 must get coord index 1 (x=20), A1 index 2 (x=30), A2 index 0 (x=10).
        let mols = extract_pse_molecules(&molecule_tree(vec![2, 0, 1]));
        assert_eq!(mols.len(), 1);
        let atoms = mols[0].atoms();
        assert_eq!(atoms[0].name, "A0");
        assert!((atoms[0].position.x - 20.0).abs() < 1e-6, "A0 x={}", atoms[0].position.x);
        assert!((atoms[1].position.x - 30.0).abs() < 1e-6, "A1 x={}", atoms[1].position.x);
        assert!((atoms[2].position.x - 10.0).abs() < 1e-6, "A2 x={}", atoms[2].position.x);
    }

    #[test]
    fn identity_idx_to_atm_keeps_coordinate_order() {
        let mols = extract_pse_molecules(&molecule_tree(vec![0, 1, 2]));
        let atoms = mols[0].atoms();
        assert!((atoms[0].position.x - 10.0).abs() < 1e-6);
        assert!((atoms[1].position.x - 20.0).abs() < 1e-6);
        assert!((atoms[2].position.x - 30.0).abs() < 1e-6);
    }
}
