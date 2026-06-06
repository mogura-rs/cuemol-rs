use cuemol_core::{CueMolError, Matrix4, Result, Uid, Vector4};
use cuemol_molstr::{
    parse_selector, Atom, BondOrder, MolCoord, MolSceneManager, ResidueKey, Selector,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MolAnlManager;

impl MolAnlManager {
    pub fn new() -> Self {
        Self
    }

    pub fn centroid(&self, molecule: &MolCoord) -> Option<Vector4> {
        centroid(molecule)
    }

    pub fn selected_centroid(&self, molecule: &MolCoord, selector: &Selector) -> Option<Vector4> {
        selected_centroid(molecule, selector)
    }

    pub fn calc_rmsd(
        &self,
        reference: &MolCoord,
        reference_selector: &Selector,
        moving: &MolCoord,
        moving_selector: &Selector,
    ) -> Result<f64> {
        let pairs =
            selected_position_pairs(reference, reference_selector, moving, moving_selector)?;
        Ok(rmsd_for_pairs(&pairs, Matrix4::IDENTITY))
    }

    pub fn superpose_lsq_matrix(
        &self,
        reference: &MolCoord,
        reference_selector: &Selector,
        moving: &MolCoord,
        moving_selector: &Selector,
    ) -> Result<Matrix4> {
        let pairs =
            selected_position_pairs(reference, reference_selector, moving, moving_selector)?;
        rigid_fit_matrix(&pairs)
    }

    pub fn superpose_lsq1(
        &self,
        reference: &MolCoord,
        reference_selector: &Selector,
        moving: &MolCoord,
        moving_selector: &Selector,
        _use_prop: bool,
    ) -> Result<Matrix4> {
        self.superpose_lsq_matrix(reference, reference_selector, moving, moving_selector)
    }

    pub fn superpose_lsq_1(
        &self,
        reference: &MolCoord,
        reference_selector: &Selector,
        moving: &MolCoord,
        moving_selector: &Selector,
        use_prop: bool,
    ) -> Result<Matrix4> {
        self.superpose_lsq1(
            reference,
            reference_selector,
            moving,
            moving_selector,
            use_prop,
        )
    }

    pub fn superpose_ssm1(
        &self,
        reference: &MolCoord,
        reference_selector: &Selector,
        moving: &MolCoord,
        moving_selector: &Selector,
        _use_prop: bool,
    ) -> Result<Matrix4> {
        self.superpose_lsq_matrix(reference, reference_selector, moving, moving_selector)
    }

    pub fn superpose_ssm_1(
        &self,
        reference: &MolCoord,
        reference_selector: &Selector,
        moving: &MolCoord,
        moving_selector: &Selector,
        use_prop: bool,
    ) -> Result<Matrix4> {
        self.superpose_ssm1(
            reference,
            reference_selector,
            moving,
            moving_selector,
            use_prop,
        )
    }

    pub fn superpose_ssm2(
        &self,
        reference_id: u64,
        reference_selection: &str,
        moving_id: u64,
        moving_selection: &str,
    ) -> Result<()> {
        validate_superpose_ids(reference_id, moving_id)?;
        parse_selector(reference_selection).map_err(|err| CueMolError::parse(err.to_string()))?;
        parse_selector(moving_selection).map_err(|err| CueMolError::parse(err.to_string()))?;
        Err(CueMolError::invalid_argument(
            "superpose_ssm2 requires a MolSceneManager; use superpose_ssm2_in_manager",
        ))
    }

    pub fn superpose_ssm2_in_manager(
        &self,
        scene_manager: &mut MolSceneManager,
        reference_id: u64,
        reference_selection: &str,
        moving_id: u64,
        moving_selection: &str,
    ) -> Result<Matrix4> {
        validate_superpose_ids(reference_id, moving_id)?;
        let reference_uid = Uid::new(reference_id);
        let moving_uid = Uid::new(moving_id);
        let reference_selector = parse_selector(reference_selection)
            .map_err(|err| CueMolError::parse(err.to_string()))?;
        let moving_selector =
            parse_selector(moving_selection).map_err(|err| CueMolError::parse(err.to_string()))?;
        let reference = scene_manager
            .object(reference_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("reference object {reference_id}")))?
            .mol()
            .clone();
        let moving = scene_manager
            .object(moving_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("moving object {moving_id}")))?
            .mol()
            .clone();
        let matrix = self.superpose_ssm1(
            &reference,
            &reference_selector,
            &moving,
            &moving_selector,
            false,
        )?;
        let moved_count = scene_manager
            .object_mut(moving_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("moving object {moving_id}")))?
            .mol_mut()
            .xform_by_mat(matrix, Some(&moving_selector));
        if moved_count == 0 {
            return Err(CueMolError::invalid_argument("no moving atoms selected"));
        }
        Ok(matrix)
    }

    pub fn superpose_ssm_2(
        &self,
        reference_id: u64,
        reference_selection: &str,
        moving_id: u64,
        moving_selection: &str,
    ) -> Result<()> {
        self.superpose_ssm2(
            reference_id,
            reference_selection,
            moving_id,
            moving_selection,
        )
    }

    pub fn superpose_ssm_rmsd(
        &self,
        reference: &MolCoord,
        reference_selector: &Selector,
        moving: &MolCoord,
        moving_selector: &Selector,
        _show_log: bool,
    ) -> Result<f64> {
        let pairs =
            selected_position_pairs(reference, reference_selector, moving, moving_selector)?;
        let matrix = rigid_fit_matrix(&pairs)?;
        Ok(rmsd_for_pairs(&pairs, matrix))
    }

    pub fn copy_atoms(
        &self,
        target: &mut MolCoord,
        source: &MolCoord,
        selector: &Selector,
    ) -> Result<bool> {
        target
            .copy_selected_atoms_from(source, selector)
            .map(|count| count > 0)
    }

    pub fn delete_atoms(&self, molecule: &mut MolCoord, selector: &Selector) -> bool {
        molecule.delete_selected_atoms(selector) > 0
    }

    pub fn change_chain_name(
        &self,
        molecule: &mut MolCoord,
        selector: &Selector,
        name: &str,
    ) -> bool {
        let mut changed = false;
        for atom in molecule.atoms_mut() {
            if selector.matches(atom) {
                atom.chain_id = name.to_string();
                changed = true;
            }
        }
        changed
    }

    pub fn shift_res_index(
        &self,
        molecule: &mut MolCoord,
        selector: &Selector,
        shift_selected: bool,
        value: i32,
    ) -> bool {
        let mut changed = false;
        for atom in molecule.atoms_mut() {
            if selector.matches(atom) == shift_selected {
                atom.res_id += value;
                changed = true;
            }
        }
        changed
    }

    pub fn renum_res_index(
        &self,
        molecule: &mut MolCoord,
        selector: &Selector,
        shift_selected: bool,
        start: i32,
    ) -> bool {
        let mut changed = false;
        let mut current_key: Option<(String, i32, Option<char>)> = None;
        let mut next_resid = start;
        for atom in molecule.atoms_mut() {
            if selector.matches(atom) != shift_selected {
                continue;
            }
            let key = (atom.chain_id.clone(), atom.res_id, atom.ins_code);
            if current_key.as_ref() != Some(&key) {
                current_key = Some(key);
                next_resid += 1;
            }
            atom.res_id = next_resid - 1;
            changed = true;
        }
        changed
    }

    pub fn make_bond(&self, molecule: &mut MolCoord, aid1: i32, aid2: i32) -> bool {
        let Some(index1) = atom_index_by_serial(molecule, aid1) else {
            return false;
        };
        let Some(index2) = atom_index_by_serial(molecule, aid2) else {
            return false;
        };
        let before = molecule.bond_count();
        molecule.add_bond(index1, index2, BondOrder::Single);
        molecule.bond_count() != before
    }

    pub fn remove_bond(&self, molecule: &mut MolCoord, aid1: i32, aid2: i32) -> bool {
        remove_bond_by_serial(molecule, aid1, aid2)
    }

    pub fn get_nostd_bonds_json(&self, molecule: &MolCoord) -> String {
        bond_pairs_json(
            molecule
                .bonds()
                .iter()
                .filter_map(|bond| {
                    let aid1 = molecule.atoms().get(bond.atom1)?.serial;
                    let aid2 = molecule.atoms().get(bond.atom2)?.serial;
                    Some(sorted_pair(aid1, aid2))
                })
                .collect::<BTreeSet<_>>()
                .into_iter(),
        )
    }

    pub fn calc_atom_contact_json(
        &self,
        molecule: &MolCoord,
        selector: &Selector,
        r_min: f64,
        r_max: f64,
        hbond: bool,
        max_count: usize,
    ) -> String {
        contact_json_same_molecule(
            molecule,
            selector,
            &Selector::All,
            r_min,
            r_max,
            hbond,
            max_count,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn calc_atom_contact2_json(
        &self,
        molecule: &MolCoord,
        selector1: &Selector,
        selector2: &Selector,
        r_min: f64,
        r_max: f64,
        hbond: bool,
        max_count: usize,
    ) -> String {
        contact_json_same_molecule(
            molecule, selector1, selector2, r_min, r_max, hbond, max_count,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn calc_atom_contact_2_json(
        &self,
        molecule: &MolCoord,
        selector1: &Selector,
        selector2: &Selector,
        r_min: f64,
        r_max: f64,
        hbond: bool,
        max_count: usize,
    ) -> String {
        self.calc_atom_contact2_json(
            molecule, selector1, selector2, r_min, r_max, hbond, max_count,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn calc_atom_contact3_json(
        &self,
        molecule1: &MolCoord,
        selector1: &Selector,
        molecule2: &MolCoord,
        selector2: &Selector,
        r_min: f64,
        r_max: f64,
        hbond: bool,
        max_count: usize,
    ) -> String {
        if std::ptr::eq(molecule1, molecule2) {
            contact_json_same_molecule(
                molecule1, selector1, selector2, r_min, r_max, hbond, max_count,
            )
        } else {
            contact_json_two_molecules(
                molecule1, selector1, molecule2, selector2, r_min, r_max, hbond, max_count,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn calc_atom_contact_3_json(
        &self,
        molecule1: &MolCoord,
        selector1: &Selector,
        molecule2: &MolCoord,
        selector2: &Selector,
        r_min: f64,
        r_max: f64,
        hbond: bool,
        max_count: usize,
    ) -> String {
        self.calc_atom_contact3_json(
            molecule1, selector1, molecule2, selector2, r_min, r_max, hbond, max_count,
        )
    }

    pub fn calc_prot_2ndry2(&self, molecule: &mut MolCoord, ignore_breaks: bool, angle: f64) {
        let summaries = molecule.residue_summaries();
        let mut protein_residues = BTreeSet::new();
        let mut chain_ca = BTreeMap::<String, Vec<ResidueCa>>::new();

        for summary in &summaries {
            if !is_amino_acid(&summary.res_name)
                && !has_backbone_ca(molecule, &summary.atom_indices)
            {
                continue;
            }
            protein_residues.insert(summary.key.clone());
            if let Some(position) = ca_position(molecule, &summary.atom_indices) {
                chain_ca
                    .entry(summary.key.chain_id.clone())
                    .or_default()
                    .push(ResidueCa {
                        key: summary.key.clone(),
                        position,
                    });
            }
        }

        for atom in molecule.atoms_mut() {
            let key = ResidueKey::new(atom.chain_id.clone(), atom.res_id, atom.ins_code);
            if protein_residues.contains(&key) {
                atom.set_int_prop("secondary", 0);
            }
        }

        let max_angle = if angle > 0.01 { angle.abs() } else { 60.0 };
        let mut helix_residues = BTreeSet::new();
        for residues in chain_ca.values_mut() {
            residues.sort_by_key(|residue| (residue.key.res_id, residue.key.ins_code));
            for window in residues.windows(4) {
                if !ignore_breaks && !is_consecutive_ca_window(window) {
                    continue;
                }
                let Some(torsion) = ca_torsion_degrees(
                    window[0].position,
                    window[1].position,
                    window[2].position,
                    window[3].position,
                ) else {
                    continue;
                };
                if torsion.abs() <= max_angle {
                    for residue in window {
                        helix_residues.insert(residue.key.clone());
                    }
                }
            }
        }

        for atom in molecule.atoms_mut() {
            let key = ResidueKey::new(atom.chain_id.clone(), atom.res_id, atom.ins_code);
            if helix_residues.contains(&key) {
                atom.set_int_prop("secondary", 2);
            }
        }
    }

    pub fn calc_prot_2_ndry_2(&self, molecule: &mut MolCoord, ignore_breaks: bool, angle: f64) {
        self.calc_prot_2ndry2(molecule, ignore_breaks, angle);
    }

    pub fn set_prot_2ndry(
        &self,
        molecule: &mut MolCoord,
        selector: &Selector,
        secondary_type: i32,
    ) {
        for atom in molecule.atoms_mut() {
            if selector.matches(atom) {
                atom.set_int_prop("secondary", secondary_type);
            }
        }
    }

    pub fn set_prot_2_ndry(
        &self,
        molecule: &mut MolCoord,
        selector: &Selector,
        secondary_type: i32,
    ) {
        self.set_prot_2ndry(molecule, selector, secondary_type);
    }
}

pub fn centroid(molecule: &MolCoord) -> Option<Vector4> {
    let count = molecule.atom_count();
    if count == 0 {
        return None;
    }
    let sum = molecule
        .atoms()
        .iter()
        .fold(Vector4::ZERO, |acc, atom| acc.add_vec(atom.position));
    Some(sum.scale(1.0 / count as f64))
}

pub fn selected_centroid(molecule: &MolCoord, selector: &Selector) -> Option<Vector4> {
    let indices = molecule.select_indices(selector);
    if indices.is_empty() {
        return None;
    }
    let sum = indices.iter().fold(Vector4::ZERO, |acc, index| {
        acc.add_vec(molecule.atoms()[*index].position)
    });
    Some(sum.scale(1.0 / indices.len() as f64))
}

#[derive(Debug, Clone, PartialEq)]
struct ResidueCa {
    key: ResidueKey,
    position: Vector4,
}

fn is_amino_acid(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_uppercase().as_str(),
        "ALA"
            | "ARG"
            | "ASN"
            | "ASP"
            | "CYS"
            | "GLN"
            | "GLU"
            | "GLY"
            | "HIS"
            | "ILE"
            | "LEU"
            | "LYS"
            | "MET"
            | "PHE"
            | "PRO"
            | "SER"
            | "THR"
            | "TRP"
            | "TYR"
            | "VAL"
            | "MSE"
    )
}

fn has_backbone_ca(molecule: &MolCoord, atom_indices: &[usize]) -> bool {
    ca_position(molecule, atom_indices).is_some()
}

fn ca_position(molecule: &MolCoord, atom_indices: &[usize]) -> Option<Vector4> {
    atom_indices.iter().find_map(|index| {
        let atom = molecule.atoms().get(*index)?;
        atom.matches_name_case_insensitive("CA")
            .then_some(atom.position)
    })
}

fn is_consecutive_ca_window(window: &[ResidueCa]) -> bool {
    window.windows(2).all(|pair| {
        pair[0].key.chain_id == pair[1].key.chain_id && pair[1].key.res_id == pair[0].key.res_id + 1
    })
}

fn ca_torsion_degrees(a: Vector4, b: Vector4, c: Vector4, d: Vector4) -> Option<f64> {
    let b1 = b - a;
    let b2 = c - b;
    let b3 = d - c;
    let n1 = b1.cross(b2).normalize3d().ok()?;
    let n2 = b2.cross(b3).normalize3d().ok()?;
    let m1 = n1.cross(b2.normalize3d().ok()?);
    Some(m1.dot3d(n2).atan2(n1.dot3d(n2)).to_degrees())
}

fn selected_position_pairs(
    reference: &MolCoord,
    reference_selector: &Selector,
    moving: &MolCoord,
    moving_selector: &Selector,
) -> Result<Vec<(Vector4, Vector4)>> {
    let ref_indices = reference.select_indices(reference_selector);
    let mov_indices = moving.select_indices(moving_selector);
    if ref_indices.is_empty() {
        return Err(CueMolError::invalid_argument("no reference atoms selected"));
    }
    if ref_indices.len() != mov_indices.len() {
        return Err(CueMolError::invalid_argument(format!(
            "selection size mismatch: {} != {}",
            ref_indices.len(),
            mov_indices.len()
        )));
    }
    Ok(ref_indices
        .into_iter()
        .zip(mov_indices)
        .map(|(ref_index, mov_index)| {
            (
                reference.atoms()[ref_index].position,
                moving.atoms()[mov_index].position,
            )
        })
        .collect())
}

fn validate_superpose_ids(reference_id: u64, moving_id: u64) -> Result<()> {
    if reference_id == 0 {
        return Err(CueMolError::invalid_argument("reference object id is zero"));
    }
    if moving_id == 0 {
        return Err(CueMolError::invalid_argument("moving object id is zero"));
    }
    Ok(())
}

fn rigid_fit_matrix(pairs: &[(Vector4, Vector4)]) -> Result<Matrix4> {
    if pairs.is_empty() {
        return Err(CueMolError::invalid_argument("no atom pairs"));
    }
    let ref_center = center_of_positions(pairs.iter().map(|(reference, _)| *reference));
    let mov_center = center_of_positions(pairs.iter().map(|(_, moving)| *moving));
    if pairs.len() == 1 {
        return Ok(Matrix4::make_trans_mat(ref_center - mov_center));
    }

    let mut covariance = [[0.0_f64; 3]; 3];
    for (reference, moving) in pairs {
        let r = *reference - ref_center;
        let m = *moving - mov_center;
        let mv = [m.x, m.y, m.z];
        let rv = [r.x, r.y, r.z];
        for i in 0..3 {
            for (j, value) in rv.iter().enumerate() {
                covariance[i][j] += mv[i] * value;
            }
        }
    }

    let quat = dominant_quaternion(covariance);
    let mut matrix = Matrix4::make_rot_mat(quat);
    let moved_center =
        matrix.transform_point(Vector4::new4(mov_center.x, mov_center.y, mov_center.z, 1.0));
    matrix
        .set_trans_part(ref_center - Vector4::new3(moved_center.x, moved_center.y, moved_center.z));
    Ok(matrix)
}

fn dominant_quaternion(s: [[f64; 3]; 3]) -> cuemol_core::Quat {
    let trace = s[0][0] + s[1][1] + s[2][2];
    let n = [
        [
            trace,
            s[1][2] - s[2][1],
            s[2][0] - s[0][2],
            s[0][1] - s[1][0],
        ],
        [
            s[1][2] - s[2][1],
            s[0][0] - s[1][1] - s[2][2],
            s[0][1] + s[1][0],
            s[2][0] + s[0][2],
        ],
        [
            s[2][0] - s[0][2],
            s[0][1] + s[1][0],
            -s[0][0] + s[1][1] - s[2][2],
            s[1][2] + s[2][1],
        ],
        [
            s[0][1] - s[1][0],
            s[2][0] + s[0][2],
            s[1][2] + s[2][1],
            -s[0][0] - s[1][1] + s[2][2],
        ],
    ];

    let mut q = [1.0, 0.0, 0.0, 0.0];
    for _ in 0..64 {
        let mut next = [0.0; 4];
        for i in 0..4 {
            next[i] = (0..4).map(|j| n[i][j] * q[j]).sum();
        }
        let norm = next.iter().map(|value| value * value).sum::<f64>().sqrt();
        if norm <= 1.0e-15 {
            break;
        }
        for value in &mut next {
            *value /= norm;
        }
        q = next;
    }
    cuemol_core::Quat::from_scalar_vector(q[0], q[1], q[2], q[3])
}

fn center_of_positions(values: impl Iterator<Item = Vector4>) -> Vector4 {
    let mut count = 0_usize;
    let mut sum = Vector4::ZERO;
    for value in values {
        count += 1;
        sum = sum.add_vec(value);
    }
    sum.scale(1.0 / count as f64)
}

fn rmsd_for_pairs(pairs: &[(Vector4, Vector4)], matrix: Matrix4) -> f64 {
    let sum = pairs
        .iter()
        .map(|(reference, moving)| {
            let transformed =
                matrix.transform_point(Vector4::new4(moving.x, moving.y, moving.z, 1.0));
            let delta = *reference - Vector4::new3(transformed.x, transformed.y, transformed.z);
            delta.length3d().powi(2)
        })
        .sum::<f64>();
    (sum / pairs.len() as f64).sqrt()
}

fn contact_json_same_molecule(
    molecule: &MolCoord,
    selector1: &Selector,
    selector2: &Selector,
    r_min: f64,
    r_max: f64,
    hbond: bool,
    max_count: usize,
) -> String {
    let bonded = molecule
        .bonds()
        .iter()
        .filter_map(|bond| {
            let aid1 = molecule.atoms().get(bond.atom1)?.serial;
            let aid2 = molecule.atoms().get(bond.atom2)?.serial;
            Some(sorted_pair(aid1, aid2))
        })
        .collect::<BTreeSet<_>>();
    let mut hits = BTreeSet::new();
    let atoms = molecule.atoms();
    for (index1, atom1) in atoms.iter().enumerate() {
        if !selector1.matches(atom1) || excluded_contact_atom(atom1, hbond) {
            continue;
        }
        for atom2 in atoms.iter().skip(index1 + 1) {
            if !selector2.matches(atom2) || excluded_contact_atom(atom2, hbond) {
                continue;
            }
            if same_residue(atom1, atom2) {
                continue;
            }
            let pair = sorted_pair(atom1.serial, atom2.serial);
            if bonded.contains(&pair) {
                continue;
            }
            let distance = (atom1.position - atom2.position).length3d();
            if distance >= r_min && distance <= r_max {
                hits.insert(pair);
            }
        }
    }
    bond_pairs_json(hits.into_iter().take(max_count))
}

#[allow(clippy::too_many_arguments)]
fn contact_json_two_molecules(
    molecule1: &MolCoord,
    selector1: &Selector,
    molecule2: &MolCoord,
    selector2: &Selector,
    r_min: f64,
    r_max: f64,
    hbond: bool,
    max_count: usize,
) -> String {
    let mut hits = BTreeSet::new();
    for atom1 in molecule1.atoms() {
        if !selector1.matches(atom1) || excluded_contact_atom(atom1, hbond) {
            continue;
        }
        for atom2 in molecule2.atoms() {
            if !selector2.matches(atom2) || excluded_contact_atom(atom2, hbond) {
                continue;
            }
            let distance = (atom1.position - atom2.position).length3d();
            if distance >= r_min && distance <= r_max {
                hits.insert((atom1.serial, atom2.serial));
            }
        }
    }
    bond_pairs_json(hits.into_iter().take(max_count))
}

fn excluded_contact_atom(atom: &Atom, hbond: bool) -> bool {
    let element = atom.element.trim();
    element.eq_ignore_ascii_case("H") || (hbond && element.eq_ignore_ascii_case("C"))
}

fn same_residue(lhs: &Atom, rhs: &Atom) -> bool {
    lhs.chain_id == rhs.chain_id && lhs.res_id == rhs.res_id && lhs.ins_code == rhs.ins_code
}

fn sorted_pair(aid1: i32, aid2: i32) -> (i32, i32) {
    if aid1 <= aid2 {
        (aid1, aid2)
    } else {
        (aid2, aid1)
    }
}

fn bond_pairs_json(pairs: impl Iterator<Item = (i32, i32)>) -> String {
    let entries = pairs
        .map(|(aid1, aid2)| format!("[{aid1},{aid2}]"))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        String::new()
    } else {
        format!("[{}]", entries.join(","))
    }
}

fn atom_index_by_serial(molecule: &MolCoord, serial: i32) -> Option<usize> {
    molecule
        .atoms()
        .iter()
        .position(|atom| atom.serial == serial)
}

fn remove_bond_by_serial(molecule: &mut MolCoord, aid1: i32, aid2: i32) -> bool {
    let Some(index1) = atom_index_by_serial(molecule, aid1) else {
        return false;
    };
    let Some(index2) = atom_index_by_serial(molecule, aid2) else {
        return false;
    };

    let mut rebuilt = MolCoord::with_atoms(molecule.atoms().to_vec());
    let mut removed = false;
    for bond in molecule.bonds() {
        let is_target = (bond.atom1 == index1 && bond.atom2 == index2)
            || (bond.atom1 == index2 && bond.atom2 == index1);
        if is_target {
            removed = true;
        } else {
            rebuilt.add_bond(bond.atom1, bond.atom2, bond.order);
        }
    }
    if removed {
        *molecule = rebuilt;
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(serial: i32, name: &str, chain: &str, res_id: i32, element: &str, x: f64) -> Atom {
        Atom {
            serial,
            name: name.to_string(),
            alt_loc: None,
            res_name: "ALA".to_string(),
            chain_id: chain.to_string(),
            res_id,
            ins_code: None,
            position: Vector4::new3(x, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: element.to_string(),
            formal_charge: None,
            int_props: Default::default(),
        }
    }

    #[test]
    fn manager_centroid_rmsd_and_lsq_translation_match_reference_shape() {
        let mut reference = MolCoord::new();
        reference.push_atom(atom(1, "N", "A", 1, "N", 1.0));
        reference.push_atom(atom(2, "CA", "A", 1, "C", 2.0));
        let mut moving = MolCoord::new();
        moving.push_atom(atom(1, "N", "A", 1, "N", 3.0));
        moving.push_atom(atom(2, "CA", "A", 1, "C", 4.0));

        let manager = MolAnlManager::new();
        assert_eq!(
            manager.centroid(&reference),
            Some(Vector4::new3(1.5, 0.0, 0.0))
        );
        let matrix = manager
            .superpose_lsq_matrix(&reference, &Selector::All, &moving, &Selector::All)
            .unwrap();
        assert_eq!(matrix.get_trans_part(), Vector4::new3(-2.0, 0.0, 0.0));
        assert!(
            manager
                .superpose_ssm_rmsd(&reference, &Selector::All, &moving, &Selector::All, false)
                .unwrap()
                < 1.0e-8
        );
    }

    #[test]
    fn manager_superpose_ssm2_in_manager_transforms_moving_object() {
        let mut reference = MolCoord::new();
        reference.push_atom(atom(1, "N", "A", 1, "N", 1.0));
        reference.push_atom(atom(2, "CA", "A", 1, "C", 2.0));
        let mut moving = MolCoord::new();
        moving.push_atom(atom(1, "N", "A", 1, "N", 3.0));
        moving.push_atom(atom(2, "CA", "A", 1, "C", 4.0));

        let mut scene_manager = MolSceneManager::new();
        let scene_uid = scene_manager.new_scene("scene");
        let scene = scene_manager.scene_mut(scene_uid).unwrap();
        let reference_uid = scene.add_molecule("ref", reference);
        let moving_uid = scene.add_molecule("mov", moving);

        let matrix = MolAnlManager::new()
            .superpose_ssm2_in_manager(
                &mut scene_manager,
                reference_uid.get(),
                "all",
                moving_uid.get(),
                "all",
            )
            .unwrap();

        assert_eq!(matrix.get_trans_part(), Vector4::new3(-2.0, 0.0, 0.0));
        let moved = scene_manager.object(moving_uid).unwrap().mol();
        assert_eq!(moved.atoms()[0].position, Vector4::new3(1.0, 0.0, 0.0));
        assert_eq!(moved.atoms()[1].position, Vector4::new3(2.0, 0.0, 0.0));
    }

    #[test]
    fn manager_edit_methods_delegate_to_molecule_model() {
        let manager = MolAnlManager::new();
        let mut target = MolCoord::new();
        target.push_atom(atom(1, "N", "A", 1, "N", 0.0));
        let mut source = MolCoord::new();
        source.push_atom(atom(2, "CA", "B", 2, "C", 1.0));

        assert!(manager
            .copy_atoms(&mut target, &source, &Selector::All)
            .unwrap());
        assert_eq!(target.atom_count(), 2);
        assert!(manager.change_chain_name(&mut target, &Selector::All, "Z"));
        assert!(target.atoms().iter().all(|atom| atom.chain_id == "Z"));
        assert!(manager.shift_res_index(&mut target, &Selector::All, true, 10));
        assert_eq!(target.atoms()[0].res_id, 11);
        assert!(manager.renum_res_index(&mut target, &Selector::All, true, 1));
        assert_eq!(target.atoms()[0].res_id, 1);
        assert!(manager.delete_atoms(&mut target, &Selector::All));
        assert_eq!(target.atom_count(), 0);
    }

    #[test]
    fn manager_bond_and_contact_json_match_reference_shape() {
        let manager = MolAnlManager::new();
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1, "N", "A", 1, "N", 0.0));
        molecule.push_atom(atom(2, "O", "A", 2, "O", 2.5));
        molecule.push_atom(atom(3, "C", "A", 3, "C", 4.5));
        molecule.push_atom(atom(4, "H", "A", 4, "H", 4.7));

        assert_eq!(
            manager.calc_atom_contact_json(&molecule, &Selector::All, 1.0, 3.0, false, 10),
            "[[1,2],[2,3]]"
        );
        assert_eq!(
            manager.calc_atom_contact_json(&molecule, &Selector::All, 1.0, 3.0, true, 10),
            "[[1,2]]"
        );
        assert!(manager.make_bond(&mut molecule, 1, 2));
        assert_eq!(manager.get_nostd_bonds_json(&molecule), "[[1,2]]");
        assert_eq!(
            manager.calc_atom_contact_json(&molecule, &Selector::All, 1.0, 3.0, false, 10),
            "[[2,3]]"
        );
        assert!(manager.remove_bond(&mut molecule, 1, 2));
        assert_eq!(manager.get_nostd_bonds_json(&molecule), "");
    }

    #[test]
    fn manager_secondary_structure_sets_atom_property() {
        let manager = MolAnlManager::new();
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1, "CA", "A", 1, "C", 0.0));
        manager.set_prot_2ndry(&mut molecule, &Selector::All, 2);
        assert_eq!(molecule.atoms()[0].int_prop("secondary"), Some(2));
    }

    #[test]
    fn manager_calc_secondary_marks_ca_torsion_helix_window() {
        let manager = MolAnlManager::new();
        let mut molecule = MolCoord::new();
        let mut a1 = atom(1, "CA", "A", 1, "C", 0.0);
        a1.position = Vector4::new3(0.0, 0.0, 0.0);
        let mut a2 = atom(2, "CA", "A", 2, "C", 0.0);
        a2.position = Vector4::new3(1.0, 0.0, 0.0);
        let mut a3 = atom(3, "CA", "A", 3, "C", 0.0);
        a3.position = Vector4::new3(1.0, 1.0, 0.0);
        let mut a4 = atom(4, "CA", "A", 4, "C", 0.0);
        a4.position = Vector4::new3(0.0, 2.0, 0.0);
        molecule.push_atom(a1);
        molecule.push_atom(a2);
        molecule.push_atom(a3);
        molecule.push_atom(a4);

        manager.calc_prot_2ndry2(&mut molecule, false, 60.0);

        assert!(molecule
            .atoms()
            .iter()
            .all(|atom| atom.int_prop("secondary") == Some(2)));
    }
}
