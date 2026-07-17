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
//!   RESN=5, NAME=6, ELEM=7, BFAC=14, OCC=15, COLOR=21).
//! - Only the first coordinate set is read (C++ line 431: `getList(4)->getList(0)`;
//!   its `for i<ncset` loop is debug-only). Multi-state `.pse` keep their first
//!   state in BOTH implementations.
//! - The session `"version"` field is not branched on — C++ only LOGS it
//!   (`int ver = ...; LOG_DPRINTLN("PyMOL version %d", ver)`), using one atom
//!   layout for all versions, exactly as here.

use std::collections::BTreeMap;

use cuemol_core::{Quat, Vector4};
use cuemol_molstr::{Atom, BondOrder, MolCoord};
use cuemol_qsys::Camera;

use crate::PickleValue;

// `names` list element layout: [name, type, visible, repOn, extra, data, ...].
const NAME_TYPE: usize = 1;
const NAME_VISIBLE: usize = 2;
const NAME_EXTRA: usize = 4;
const NAME_DATA: usize = 5;
const NAME_NAME: usize = 0;
const EXEC_OBJECT: i64 = 0;
const OBJECT_MOLECULE: i64 = 1;
const OBJECT_MEASUREMENT: i64 = 4;

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
const AT_LABEL: usize = 9;
const AT_BFAC: usize = 14;
const AT_OCC: usize = 15;
const AT_VISREP: usize = 20;
const AT_COLOR: usize = 21;

const REP_STICKS: usize = 0;
const REP_SPHERES: usize = 1;
const REP_SURFACE: usize = 2;
const REP_CARTOON: usize = 5;
const REP_LINES: usize = 7;
const SETTING_FIELD_OF_VIEW: i64 = 152;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PseRepresentation {
    Sticks,
    Spheres,
    Cartoon,
    Lines,
    Surface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseMeasurementKind {
    Distance,
    Angle,
    Torsion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PseMeasurement {
    kind: PseMeasurementKind,
    positions: Vec<Vector4>,
}

impl PseMeasurement {
    pub fn kind(&self) -> PseMeasurementKind {
        self.kind
    }

    pub fn positions(&self) -> &[Vector4] {
        &self.positions
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PseMeasurementRecord {
    name: String,
    visible: bool,
    molecule_index: Option<usize>,
    measurements: Vec<PseMeasurement>,
}

impl PseMeasurementRecord {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn molecule_index(&self) -> Option<usize> {
        self.molecule_index
    }

    pub fn measurements(&self) -> &[PseMeasurement] {
        &self.measurements
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseAtomLabel {
    atom_index: usize,
    text: String,
}

impl PseAtomLabel {
    pub fn atom_index(&self) -> usize {
        self.atom_index
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PseMoleculeRecord {
    name: String,
    visible: bool,
    molecule: MolCoord,
    representations: BTreeMap<PseRepresentation, Vec<usize>>,
    labels: Vec<PseAtomLabel>,
}

impl PseMoleculeRecord {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn molecule(&self) -> &MolCoord {
        &self.molecule
    }

    pub fn into_molecule(self) -> MolCoord {
        self.molecule
    }

    pub fn atom_indices(&self, representation: PseRepresentation) -> &[usize] {
        self.representations
            .get(&representation)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn labels(&self) -> &[PseAtomLabel] {
        &self.labels
    }
}

/// Extract every molecule object from a parsed `.pse` session tree.
pub fn extract_pse_molecules(tree: &PickleValue) -> Vec<MolCoord> {
    extract_pse_molecule_records(tree)
        .into_iter()
        .map(PseMoleculeRecord::into_molecule)
        .collect()
}

/// Extract molecule names and coordinates from a parsed PyMOL session.
pub fn extract_pse_named_molecules(tree: &PickleValue) -> Vec<(String, MolCoord)> {
    extract_pse_molecule_records(tree)
        .into_iter()
        .map(|record| (record.name, record.molecule))
        .collect()
}

/// Extract molecule state, including per-atom PyMOL representations and labels.
pub fn extract_pse_molecule_records(tree: &PickleValue) -> Vec<PseMoleculeRecord> {
    let mut molecules = Vec::new();
    let Some(names) = tree.get("names").and_then(PickleValue::as_list) else {
        return molecules;
    };
    for element in names {
        // `None` placeholders and non-molecule executive objects are skipped.
        if element.index(NAME_TYPE).and_then(PickleValue::as_i64) != Some(EXEC_OBJECT)
            || element.index(NAME_EXTRA).and_then(PickleValue::as_i64) != Some(OBJECT_MOLECULE)
        {
            continue;
        }
        let name = element
            .index(NAME_NAME)
            .and_then(PickleValue::as_str)
            .unwrap_or("molecule")
            .to_string();
        let visible = element.index(NAME_VISIBLE).is_none_or(pickle_truthy);
        if let Some(data) = element.index(NAME_DATA).and_then(PickleValue::as_list) {
            if let Some(extracted) = extract_one(data) {
                molecules.push(PseMoleculeRecord {
                    name,
                    visible,
                    molecule: extracted.molecule,
                    representations: extracted.representations,
                    labels: extracted.labels,
                });
            }
        }
    }
    molecules
}

/// Extract PyMOL distance, angle, and torsion objects as fixed-coordinate
/// AtomIntr definitions. Measurements target the nearest preceding molecule;
/// when none exists CueMol creates a dummy molecule to host the renderer.
pub fn extract_pse_measurement_records(tree: &PickleValue) -> Vec<PseMeasurementRecord> {
    let mut records = Vec::new();
    let Some(names) = tree.get("names").and_then(PickleValue::as_list) else {
        return records;
    };
    let mut molecule_count = 0_usize;
    let mut previous_molecule = None;
    for element in names {
        if element.index(NAME_TYPE).and_then(PickleValue::as_i64) != Some(EXEC_OBJECT) {
            continue;
        }
        match element.index(NAME_EXTRA).and_then(PickleValue::as_i64) {
            Some(OBJECT_MOLECULE) => {
                if element
                    .index(NAME_DATA)
                    .and_then(PickleValue::as_list)
                    .and_then(extract_one)
                    .is_some()
                {
                    previous_molecule = Some(molecule_count);
                    molecule_count += 1;
                }
            }
            Some(OBJECT_MEASUREMENT) => {
                let Some(data) = element.index(NAME_DATA).and_then(PickleValue::as_list) else {
                    continue;
                };
                let measurements = extract_measurements(data);
                if measurements.is_empty() {
                    continue;
                }
                records.push(PseMeasurementRecord {
                    name: element
                        .index(NAME_NAME)
                        .and_then(PickleValue::as_str)
                        .unwrap_or("measurement")
                        .to_string(),
                    visible: element.index(NAME_VISIBLE).is_none_or(pickle_truthy),
                    molecule_index: previous_molecule,
                    measurements,
                });
            }
            _ => {}
        }
    }
    records
}

fn extract_measurements(data: &[PickleValue]) -> Vec<PseMeasurement> {
    let Some(measurement) = data
        .get(2)
        .and_then(PickleValue::as_list)
        .and_then(|values| values.first())
        .and_then(PickleValue::as_list)
    else {
        return Vec::new();
    };
    let (kind, coordinate_index, coordinate_count) = if measurement
        .get(1)
        .is_some_and(|value| value.as_list().is_some())
    {
        (PseMeasurementKind::Distance, 1, 2)
    } else if measurement
        .get(4)
        .is_some_and(|value| value.as_list().is_some())
    {
        (PseMeasurementKind::Angle, 4, 3)
    } else if measurement
        .get(6)
        .is_some_and(|value| value.as_list().is_some())
    {
        (PseMeasurementKind::Torsion, 6, 4)
    } else {
        return Vec::new();
    };
    let Some(coordinates) = measurement
        .get(coordinate_index)
        .and_then(PickleValue::as_list)
    else {
        return Vec::new();
    };
    coordinates
        .chunks_exact(3 * coordinate_count)
        .filter_map(|chunk| {
            let positions = chunk
                .chunks_exact(3)
                .map(|xyz| {
                    Some(Vector4::new3(
                        xyz[0].as_f64()?,
                        xyz[1].as_f64()?,
                        xyz[2].as_f64()?,
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(PseMeasurement { kind, positions })
        })
        .collect()
}

/// Extract the PyMOL view as CueMol's `__current` camera.
pub fn extract_pse_camera(tree: &PickleValue) -> Option<Camera> {
    let view = tree.get("view")?.as_list()?;
    if view.len() < 23 {
        return None;
    }
    let number = |index: usize| view.get(index).and_then(PickleValue::as_f64);
    let matrix = [
        [number(0)?, number(4)?, number(8)?],
        [number(1)?, number(5)?, number(9)?],
        [number(2)?, number(6)?, number(10)?],
    ];
    let rotation = quat_from_rotation(matrix)?;
    let view_z = number(18)?;
    let center = Vector4::new3(number(19)?, number(20)?, number(21)?);
    let slab_depth = number(22)?;
    let field_of_view = pse_setting(tree, SETTING_FIELD_OF_VIEW)
        .and_then(PickleValue::as_f64)
        .unwrap_or(20.0);
    let zoom = -view_z * field_of_view.to_radians().tan();
    if !zoom.is_finite() {
        return None;
    }

    let mut camera = Camera::new();
    camera.set_name("__current");
    camera.set_center(center);
    camera.set_rot_quat(rotation);
    camera.set_slab_depth(slab_depth);
    camera.set_zoom(zoom);
    Some(camera)
}

fn pse_setting(tree: &PickleValue, id: i64) -> Option<&PickleValue> {
    tree.get("settings")?.as_list()?.iter().find_map(|setting| {
        (setting.index(0)?.as_i64()? == id)
            .then(|| setting.index(2))
            .flatten()
    })
}

fn quat_from_rotation(matrix: [[f64; 3]; 3]) -> Option<Quat> {
    let trace = matrix[0][0] + matrix[1][1] + matrix[2][2] + 1.0;
    let quat = if trace >= 1.0 {
        let scale = 0.5 / trace.sqrt();
        Quat::new(
            (matrix[1][2] - matrix[2][1]) * scale,
            (matrix[2][0] - matrix[0][2]) * scale,
            (matrix[0][1] - matrix[1][0]) * scale,
            0.25 / scale,
        )
    } else if matrix[0][0] > matrix[1][1].max(matrix[2][2]) {
        let root = (matrix[0][0] - matrix[1][1] - matrix[2][2] + 1.0).sqrt();
        if root == 0.0 {
            return None;
        }
        let scale = 0.5 / root;
        Quat::new(
            root * 0.5,
            (matrix[0][1] + matrix[1][0]) * scale,
            (matrix[2][0] + matrix[0][2]) * scale,
            (matrix[1][2] - matrix[2][1]) * scale,
        )
    } else if matrix[1][1] >= matrix[2][2] {
        let root = (matrix[1][1] - matrix[2][2] - matrix[0][0] + 1.0).sqrt();
        if root == 0.0 {
            return None;
        }
        let scale = 0.5 / root;
        Quat::new(
            (matrix[0][1] + matrix[1][0]) * scale,
            root * 0.5,
            (matrix[1][2] + matrix[2][1]) * scale,
            (matrix[2][0] - matrix[0][2]) * scale,
        )
    } else {
        let root = (matrix[2][2] - matrix[0][0] - matrix[1][1] + 1.0).sqrt();
        if root == 0.0 {
            return None;
        }
        let scale = 0.5 / root;
        Quat::new(
            (matrix[2][0] + matrix[0][2]) * scale,
            (matrix[1][2] + matrix[2][1]) * scale,
            root * 0.5,
            (matrix[0][1] - matrix[1][0]) * scale,
        )
    };
    quat.normalize().ok()
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

struct ExtractedPseMolecule {
    molecule: MolCoord,
    representations: BTreeMap<PseRepresentation, Vec<usize>>,
    labels: Vec<PseAtomLabel>,
}

fn extract_one(data: &[PickleValue]) -> Option<ExtractedPseMolecule> {
    let natoms = data.get(DATA_NATOMS)?.as_i64()? as usize;
    let coordset = data.get(DATA_COORDSETS)?.index(0)?;
    let coords = coordset.index(CS_COORDS)?.as_list()?;
    let atoms = data.get(DATA_ATOMS)?.as_list()?;

    // C++ appends atoms in coordinate-set order, using idxToAtm to choose the
    // matching atom record. Preserve that order and retain a map for bonds.
    let idx_to_atm = coordset.index(CS_IDXTOATM).and_then(PickleValue::as_list);
    let ncoord = idx_to_atm.map(<[_]>::len).unwrap_or(natoms);
    let mut atom_id_map = BTreeMap::new();
    let mut mol = MolCoord::new();
    let mut representations = BTreeMap::<PseRepresentation, Vec<usize>>::new();
    let mut labels = Vec::new();
    for i in 0..ncoord {
        let aid = match idx_to_atm {
            Some(map) => map.get(i)?.as_i64()? as usize,
            None => i,
        };
        let a = atoms.get(aid)?.as_list()?;
        let x = coords.get(3 * i)?.as_f64()?;
        let y = coords.get(3 * i + 1)?.as_f64()?;
        let z = coords.get(3 * i + 2)?.as_f64()?;
        let field_str = |idx: usize| a.get(idx).and_then(PickleValue::as_str).unwrap_or("");
        let chain = match field_str(AT_CHAIN) {
            "" => "_".to_string(),
            value => value.to_string(),
        };
        let resn = field_str(AT_RESN).to_string();
        let name = field_str(AT_NAME).to_string();
        let element = field_str(AT_ELEM).to_string();
        let (res_id, ins_code) = parse_resi(field_str(AT_RESI));
        let b_factor = a.get(AT_BFAC).and_then(PickleValue::as_f64).unwrap_or(0.0) as f32;
        let occupancy = a.get(AT_OCC).and_then(PickleValue::as_f64).unwrap_or(1.0) as f32;
        let mut int_props = BTreeMap::new();
        if let Some(color_index) = a.get(AT_COLOR).and_then(PickleValue::as_i64) {
            int_props.insert("pymol_color".to_string(), color_index as i32);
            if let Some(color) = crate::pse_colors::pse_color(color_index) {
                int_props.insert("col".to_string(), color as i32);
            }
        }
        let atom = Atom {
            serial: 0,
            name,
            alt_loc: None,
            res_name: resn.clone(),
            chain_id: chain.clone(),
            res_id,
            ins_code,
            position: Vector4::new3(x, y, z),
            occupancy,
            b_factor,
            element,
            formal_charge: None,
            int_props,
        };
        mol.append_atom1(atom, chain, res_id, resn);
        let atom_index = mol.atom_count() - 1;
        atom_id_map.insert(aid, atom_index);
        let visrep = a.get(AT_VISREP);
        for (representation, rep_index) in [
            (PseRepresentation::Sticks, REP_STICKS),
            (PseRepresentation::Spheres, REP_SPHERES),
            (PseRepresentation::Cartoon, REP_CARTOON),
            (PseRepresentation::Lines, REP_LINES),
            (PseRepresentation::Surface, REP_SURFACE),
        ] {
            if visrep.is_some_and(|value| representation_enabled(value, rep_index)) {
                representations
                    .entry(representation)
                    .or_default()
                    .push(atom_index);
            }
        }
        if let Some(text) = a
            .get(AT_LABEL)
            .and_then(PickleValue::as_str)
            .filter(|text| !text.is_empty())
        {
            labels.push(PseAtomLabel {
                atom_index,
                text: text.to_string(),
            });
        }
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
            if let (Some(&atom1), Some(&atom2)) = (
                atom_id_map.get(&(a1 as usize)),
                atom_id_map.get(&(a2 as usize)),
            ) {
                mol.add_bond(atom1, atom2, BondOrder::from_sdf(order as i32));
            }
        }
    }
    mol.apply_topology();
    mol.calc_prot_2ndry(-500.0, false);
    mol.calc_base_pair(3.7, 30.0);
    Some(ExtractedPseMolecule {
        molecule: mol,
        representations,
        labels,
    })
}

fn pickle_truthy(value: &PickleValue) -> bool {
    match value {
        PickleValue::Bool(value) => *value,
        PickleValue::Int(value) => *value != 0,
        PickleValue::Float(value) => *value != 0.0,
        PickleValue::None => false,
        _ => true,
    }
}

fn representation_enabled(value: &PickleValue, representation: usize) -> bool {
    if let Some(values) = value.as_list() {
        return values.get(representation).is_some_and(pickle_truthy);
    }
    value
        .as_i64()
        .is_some_and(|mask| mask & (1_i64 << representation) != 0)
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
            let mut f = vec![PickleValue::Int(0); 22];
            f[AT_CHAIN] = PickleValue::Str("A".into());
            f[AT_RESI] = PickleValue::Str("1".into());
            f[AT_RESN] = PickleValue::Str("ALA".into());
            f[AT_NAME] = PickleValue::Str(name.into());
            f[AT_ELEM] = PickleValue::Str("C".into());
            f[AT_LABEL] = PickleValue::Str(format!("label-{name}"));
            f[AT_BFAC] = PickleValue::Float(0.0);
            f[AT_OCC] = PickleValue::Float(1.0);
            f[AT_VISREP] = PickleValue::Int((1 << REP_STICKS) | (1 << REP_CARTOON));
            f[AT_COLOR] = PickleValue::Int(2);
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
        name_elem[NAME_TYPE] = PickleValue::Int(EXEC_OBJECT);
        name_elem[NAME_VISIBLE] = PickleValue::Int(1);
        name_elem[NAME_EXTRA] = PickleValue::Int(OBJECT_MOLECULE);
        name_elem[NAME_DATA] = PickleValue::List(data);
        PickleValue::Dict(vec![(
            PickleValue::Str("names".into()),
            PickleValue::List(vec![PickleValue::None, PickleValue::List(name_elem)]),
        )])
    }

    #[test]
    fn idx_to_atm_routes_coordinates_to_the_right_atom() {
        // Permutation [2,0,1]: coord index 0 -> atom 2, 1 -> atom 0, 2 -> atom 1.
        // The C++ importer appends in coordinate order while routing identities.
        let mols = extract_pse_molecules(&molecule_tree(vec![2, 0, 1]));
        assert_eq!(mols.len(), 1);
        let atoms = mols[0].atoms();
        assert_eq!(atoms[0].name, "A2");
        assert_eq!(atoms[0].position.x, 10.0);
        assert_eq!(atoms[1].name, "A0");
        assert_eq!(atoms[1].position.x, 20.0);
        assert_eq!(atoms[2].name, "A1");
        assert_eq!(atoms[2].position.x, 30.0);
        assert_eq!(atoms[0].int_prop("pymol_color"), Some(2));
        assert_eq!(atoms[0].int_prop("col"), Some(0xff00_00ff_u32 as i32));
    }

    #[test]
    fn identity_idx_to_atm_keeps_coordinate_order() {
        let mols = extract_pse_molecules(&molecule_tree(vec![0, 1, 2]));
        let atoms = mols[0].atoms();
        assert!((atoms[0].position.x - 10.0).abs() < 1e-6);
        assert!((atoms[1].position.x - 20.0).abs() < 1e-6);
        assert!((atoms[2].position.x - 30.0).abs() < 1e-6);
    }

    #[test]
    fn molecule_records_preserve_visibility_representations_and_labels() {
        let records = extract_pse_molecule_records(&molecule_tree(vec![2, 0, 1]));
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert!(record.is_visible());
        assert_eq!(record.atom_indices(PseRepresentation::Sticks), &[0, 1, 2]);
        assert_eq!(record.atom_indices(PseRepresentation::Cartoon), &[0, 1, 2]);
        assert!(record.atom_indices(PseRepresentation::Spheres).is_empty());
        assert_eq!(record.labels()[0].atom_index(), 0);
        assert_eq!(record.labels()[0].text(), "label-A2");
    }

    #[test]
    fn measurement_records_preserve_fixed_coordinates_and_target_molecule() {
        let mut tree = molecule_tree(vec![0, 1, 2]);
        let mut measurement = vec![PickleValue::None; 9];
        measurement[1] = PickleValue::List(
            [0.0, 0.0, 0.0, 3.0, 4.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 6.0]
                .into_iter()
                .map(PickleValue::Float)
                .collect(),
        );
        let mut data = vec![PickleValue::None; 3];
        data[2] = PickleValue::List(vec![PickleValue::List(measurement)]);
        let mut name_element = vec![PickleValue::None; 6];
        name_element[NAME_NAME] = PickleValue::Str("distances".to_string());
        name_element[NAME_TYPE] = PickleValue::Int(EXEC_OBJECT);
        name_element[NAME_VISIBLE] = PickleValue::Int(0);
        name_element[NAME_EXTRA] = PickleValue::Int(OBJECT_MEASUREMENT);
        name_element[NAME_DATA] = PickleValue::List(data);
        if let PickleValue::Dict(entries) = &mut tree {
            let names = entries
                .iter_mut()
                .find_map(|(key, value)| (key.as_str() == Some("names")).then_some(value))
                .unwrap();
            if let PickleValue::List(names) = names {
                names.push(PickleValue::List(name_element));
            }
        }

        let records = extract_pse_measurement_records(&tree);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name(), "distances");
        assert!(!records[0].is_visible());
        assert_eq!(records[0].molecule_index(), Some(0));
        assert_eq!(records[0].measurements().len(), 2);
        assert_eq!(
            records[0].measurements()[0].kind(),
            PseMeasurementKind::Distance
        );
        assert_eq!(
            records[0].measurements()[0].positions(),
            &[Vector4::new3(0.0, 0.0, 0.0), Vector4::new3(3.0, 4.0, 0.0)]
        );
    }

    #[test]
    fn measurement_parser_recognizes_angles_and_torsions() {
        for (index, count, expected) in [
            (4, 3, PseMeasurementKind::Angle),
            (6, 4, PseMeasurementKind::Torsion),
        ] {
            let mut measurement = vec![PickleValue::None; 9];
            measurement[index] = PickleValue::List(
                (0..count * 3)
                    .map(|value| PickleValue::Float(f64::from(value)))
                    .collect(),
            );
            let mut data = vec![PickleValue::None; 3];
            data[2] = PickleValue::List(vec![PickleValue::List(measurement)]);
            let parsed = extract_measurements(&data);
            assert_eq!(parsed.len(), 1);
            assert_eq!(parsed[0].kind(), expected);
            assert_eq!(parsed[0].positions().len(), count as usize);
        }
    }

    #[test]
    fn extracts_reference_view_camera_and_field_of_view() {
        let mut view = vec![PickleValue::Float(0.0); 24];
        view[0] = PickleValue::Float(1.0);
        view[5] = PickleValue::Float(1.0);
        view[10] = PickleValue::Float(1.0);
        view[18] = PickleValue::Float(-10.0);
        view[19] = PickleValue::Float(1.0);
        view[20] = PickleValue::Float(2.0);
        view[21] = PickleValue::Float(3.0);
        view[22] = PickleValue::Float(40.0);
        let setting = PickleValue::List(vec![
            PickleValue::Int(SETTING_FIELD_OF_VIEW),
            PickleValue::None,
            PickleValue::Float(30.0),
        ]);
        let tree = PickleValue::Dict(vec![
            (
                PickleValue::Str("view".to_string()),
                PickleValue::List(view),
            ),
            (
                PickleValue::Str("settings".to_string()),
                PickleValue::List(vec![setting]),
            ),
        ]);

        let camera = extract_pse_camera(&tree).unwrap();
        assert_eq!(camera.name(), "__current");
        assert_eq!(camera.center(), Vector4::new3(1.0, 2.0, 3.0));
        assert_eq!(camera.rotation(), Quat::IDENTITY);
        assert!((camera.zoom() - 10.0 * 30.0_f64.to_radians().tan()).abs() < 1.0e-9);
        assert_eq!(camera.slab_depth(), 40.0);
    }
}
