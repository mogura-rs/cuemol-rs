use cuemol_core::{CueMolError, Matrix4, Result, Uid, Vector4};
use cuemol_molstr::mol_undo::{MolUndoManager, ResiduePropsEdit};
use cuemol_molstr::{
    parse_selector, Atom, BondOrder, MolCoord, MolSceneManager, Prot2ndrySet, ResidueKey, Selector,
};
use std::collections::{BTreeMap, BTreeSet};

mod ssm;

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

    /// SSM (Secondary Structure Matching) superposition: aligns `moving` onto
    /// `reference` by matching secondary-structure elements — NO pre-supplied
    /// atom correspondence needed (unlike LSQ). Native reimplementation of the
    /// vendored CCP4 ssmlib's functionality (see `ssm`). Falls back to pre-paired
    /// LSQ when the structures lack enough secondary structure for SSM (e.g.
    /// tiny / SS-less selections of equal size), preserving the LSQ contract
    /// there.
    pub fn superpose_ssm1(
        &self,
        reference: &MolCoord,
        reference_selector: &Selector,
        moving: &MolCoord,
        moving_selector: &Selector,
        _use_prop: bool,
    ) -> Result<Matrix4> {
        match ssm::superpose_ssm_native(reference, reference_selector, moving, moving_selector) {
            Ok((matrix, _rmsd, _nfit)) => Ok(matrix),
            Err(_) => {
                self.superpose_lsq_matrix(reference, reference_selector, moving, moving_selector)
            }
        }
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
        // Move every selected atom's residue to chain `name`; the collision guard
        // in `relabel_residue_keys` rejects a merge into an existing residue.
        molecule.relabel_residue_keys(|atom| {
            selector
                .matches(atom)
                .then(|| ResidueKey::new(name, atom.res_id, atom.ins_code))
        })
    }

    pub fn shift_res_index(
        &self,
        molecule: &mut MolCoord,
        selector: &Selector,
        shift_selected: bool,
        value: i32,
    ) -> bool {
        molecule.relabel_residue_keys(|atom| {
            (selector.matches(atom) == shift_selected)
                .then(|| ResidueKey::new(atom.chain_id.clone(), atom.res_id + value, atom.ins_code))
        })
    }

    pub fn renum_res_index(
        &self,
        molecule: &mut MolCoord,
        selector: &Selector,
        shift_selected: bool,
        start: i32,
    ) -> bool {
        // Sequential renumber depends on residue ORDER, so precompute each affected
        // residue's new resid (in atom order) before delegating the relabel.
        let mut mapping: BTreeMap<ResidueKey, i32> = BTreeMap::new();
        let mut next = start;
        for atom in molecule.atoms() {
            if selector.matches(atom) == shift_selected {
                let key = ResidueKey::new(atom.chain_id.clone(), atom.res_id, atom.ins_code);
                if !mapping.contains_key(&key) {
                    mapping.insert(key, next);
                    next += 1;
                }
            }
        }
        molecule.relabel_residue_keys(|atom| {
            let key = ResidueKey::new(atom.chain_id.clone(), atom.res_id, atom.ins_code);
            mapping
                .get(&key)
                .map(|&resid| ResidueKey::new(atom.chain_id.clone(), resid, atom.ins_code))
        })
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
        // Delegate to the molstr Kabsch-Sander DSSP assignment. Mirrors C++
        // `MolAnlManager::calcProt2ndry2`, which calls `MolCoord::calcProt2ndry2`:
        // `ignore_breaks` is the bulge flag, `angle` the helix gap-fill Cα-torsion
        // criterion. Results land in the `secondary`/`secondary2` residue
        // properties (the form the renderers consume).
        molecule.calc_prot_2ndry2(ignore_breaks, angle);
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
        // Mirror C++ MolAnlManager::setProt2ndry: snapshot the current assignment,
        // override the selected residues to the requested type, and rewrite the
        // residue string properties the renderers read. Type codes: 1=sheet,
        // 2=alpha, 3=3-10, 4=pi, other=coil.
        let selected_atoms: BTreeSet<usize> =
            molecule.select_indices(selector).into_iter().collect();
        let selected: Vec<(String, i32)> = molecule
            .residue_summaries()
            .iter()
            .filter(|s| s.atom_indices.iter().any(|i| selected_atoms.contains(i)))
            .map(|s| (s.key.chain_id.clone(), s.key.res_id))
            .collect();

        let mut secset = Prot2ndrySet::create(molecule);
        for (chain, rid) in &selected {
            secset.set_type(secondary_type, chain, *rid, *rid);
        }
        secset.apply_to(molecule);
    }

    /// Like [`Self::set_prot_2ndry`], but records the edit with `undo` so it can be
    /// reverted. Port of C++ `MolAnlManager::setProt2ndry`, which wraps the edit in
    /// a `Prot2ndryEditInfo` registered with the UndoManager (saveBefore/saveAfter).
    pub fn set_prot_2ndry_with_undo(
        &self,
        molecule: &mut MolCoord,
        selector: &Selector,
        secondary_type: i32,
        undo: &mut MolUndoManager,
    ) {
        let before = molecule.residue_props().clone();
        self.set_prot_2ndry(molecule, selector, secondary_type);
        let after = molecule.residue_props().clone();
        undo.record(Box::new(ResiduePropsEdit::new(
            before,
            after,
            "set secondary structure",
        )));
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

pub(crate) fn rigid_fit_matrix(pairs: &[(Vector4, Vector4)]) -> Result<Matrix4> {
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

    // Horn's N matrix has trace 0, so it always has both positive and negative
    // eigenvalues; the optimal rotation is the eigenvector of the LARGEST
    // (algebraic) eigenvalue, but plain power iteration converges to the largest
    // *magnitude* one — which may be the negative extreme, giving a wrong
    // rotation. Shift the diagonal by a Gershgorin bound so every eigenvalue
    // becomes non-negative; then largest-magnitude == largest-algebraic, and the
    // eigenvector (hence the quaternion) is unchanged.
    let shift = (0..4)
        .map(|i| (0..4).map(|j| n[i][j].abs()).sum::<f64>())
        .fold(0.0_f64, f64::max);
    let mut m = n;
    for (i, row) in m.iter_mut().enumerate() {
        row[i] += shift;
    }

    let mut q = [1.0, 0.0, 0.0, 0.0];
    for _ in 0..512 {
        let mut next = [0.0; 4];
        for i in 0..4 {
            next[i] = (0..4).map(|j| m[i][j] * q[j]).sum();
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
    // Horn's eigenvector quaternion `q` encodes the rotation R with ref ≈ R·mov.
    // `Quat::to_rot_matrix` builds the TRANSPOSE of the standard rotation matrix
    // (it is applied as Rᵀ·v by `transform_point`), so feed it the CONJUGATE
    // (negated vector part) to make `transform_point` apply R·mov, not Rᵀ·mov.
    cuemol_core::Quat::from_scalar_vector(q[0], -q[1], -q[2], -q[3])
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

pub(crate) fn rmsd_for_pairs(pairs: &[(Vector4, Vector4)], matrix: Matrix4) -> f64 {
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
    fn change_chain_name_aborts_on_residue_collision_instead_of_merging() {
        // C++ MolAnlManager::changeChainName (MolAnlManager.cpp:785) throws if a
        // renamed residue would collide with an existing one; the Rust path used to
        // mutate unconditionally, so the flat (chain,resid)-keyed model SILENTLY
        // merged the two residues. Oracle: residue_count.
        let manager = MolAnlManager::new();
        let mut mol = MolCoord::new();
        mol.push_atom(atom(1, "CA", "A", 1, "C", 0.0)); // chain A resid 1 (unselected)
        mol.push_atom(atom(2, "CA", "B", 1, "C", 1.0)); // chain B resid 1 (selected)
        assert_eq!(mol.residue_count(), 2);
        let sel = parse_selector("chain B").unwrap();
        // Renaming chain B resid 1 -> chain A collides with the existing A resid 1.
        assert!(
            !manager.change_chain_name(&mut mol, &sel, "A"),
            "a collision must abort the rename"
        );
        assert_eq!(mol.residue_count(), 2, "no silent residue merge");
        assert_eq!(mol.atoms()[1].chain_id, "B", "the molecule is left untouched");
    }

    #[test]
    fn change_chain_name_migrates_residue_props_to_the_new_key() {
        // C++ moves the same MolResidue object, so its property map follows the
        // rename (MolCoord.cpp:661). The flat model must re-key residue_props from
        // (A,1) to (Z,1); the old code stranded them under the old key.
        let manager = MolAnlManager::new();
        let mut mol = MolCoord::new();
        mol.push_atom(atom(1, "CA", "A", 1, "C", 0.0));
        mol.set_residue_prop_str("A", "1", "secondary", "Hs").unwrap();
        let sel = parse_selector("chain A").unwrap();
        assert!(manager.change_chain_name(&mut mol, &sel, "Z"));
        assert_eq!(
            mol.get_residue_prop_str("Z", "1", "secondary").unwrap(),
            Some("Hs"),
            "props must follow the residue to its new chain"
        );
        assert_eq!(
            mol.get_residue_prop_str("A", "1", "secondary").unwrap(),
            None,
            "no props stranded under the old key"
        );
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
    fn manager_set_secondary_writes_residue_string_prop() {
        // set_prot_2ndry overrides the selected residues' secondary structure and
        // writes the residue *string* props the renderers read (not the legacy
        // per-atom int prop). Type 2 = alpha helix; a lone residue is a 1-run "Hs".
        let manager = MolAnlManager::new();
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1, "CA", "A", 1, "C", 0.0));
        manager.set_prot_2ndry(&mut molecule, &Selector::All, 2);
        assert_eq!(
            molecule
                .get_residue_prop_str("A", "1", "secondary")
                .unwrap(),
            Some("helix")
        );
        assert_eq!(
            molecule
                .get_residue_prop_str("A", "1", "secondary2")
                .unwrap(),
            Some("Hs")
        );
        assert!(molecule
            .atoms()
            .iter()
            .all(|a| a.int_prop("secondary").is_none()));
    }

    #[test]
    fn set_prot_2ndry_with_undo_reverts_and_replays() {
        // The undo-aware variant records the secondary-structure edit so it can be
        // reverted (C++ Prot2ndryEditInfo) — edit -> undo restores the prior state
        // -> redo re-applies it.
        let manager = MolAnlManager::new();
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom(1, "CA", "A", 1, "C", 0.0));
        let mut undo = MolUndoManager::new();

        manager.set_prot_2ndry_with_undo(&mut molecule, &Selector::All, 2, &mut undo);
        assert_eq!(
            molecule.get_residue_prop_str("A", "1", "secondary").unwrap(),
            Some("helix")
        );
        assert!(undo.is_undoable());

        assert!(undo.undo(&mut molecule));
        // Before the edit there was no secondary assignment for the residue.
        assert_eq!(
            molecule.get_residue_prop_str("A", "1", "secondary").unwrap(),
            None
        );

        assert!(undo.redo(&mut molecule));
        assert_eq!(
            molecule.get_residue_prop_str("A", "1", "secondary").unwrap(),
            Some("helix")
        );
    }

    #[test]
    fn manager_calc_secondary_delegates_to_dssp() {
        // calc_prot_2ndry2 now delegates to the molstr DSSP, which writes the
        // residue *string* properties (secondary/secondary2) the renderers read,
        // not the legacy per-atom int property. A single complete backbone
        // residue is enough to drive the delegation (it resolves to coil).
        let manager = MolAnlManager::new();
        let mut molecule = MolCoord::new();
        for (serial, name) in [(1, "N"), (2, "CA"), (3, "C"), (4, "O")] {
            let mut a = atom(serial, name, "A", 1, "C", 0.0);
            a.position = Vector4::new3(serial as f64, 0.0, 0.0);
            molecule.push_atom(a);
        }

        manager.calc_prot_2ndry2(&mut molecule, false, 60.0);

        // String-property convention is populated for the backbone residue...
        assert_eq!(
            molecule
                .get_residue_prop_str("A", "1", "secondary2")
                .unwrap(),
            Some("")
        );
        // ...and the legacy int property is no longer used.
        assert!(molecule
            .atoms()
            .iter()
            .all(|atom| atom.int_prop("secondary").is_none()));
    }
}
