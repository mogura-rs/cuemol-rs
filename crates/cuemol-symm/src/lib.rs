use cuemol_core::{Matrix4, Vector4};
use cuemol_gfx::SolidColor;
use cuemol_qsys::DispListRenderer;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatticeSystem {
    Triclinic = 1,
    Monoclinic = 2,
    Orthorhombic = 3,
    Tetragonal = 4,
    Trigonal = 5,
    Hexagonal = 6,
    Cubic = 7,
}

impl LatticeSystem {
    pub fn name(self) -> &'static str {
        match self {
            Self::Triclinic => "TRICLINIC",
            Self::Monoclinic => "MONOCLINIC",
            Self::Orthorhombic => "ORTHORHOMBIC",
            Self::Tetragonal => "TETRAGONAL",
            Self::Trigonal => "TRIGONAL",
            Self::Hexagonal => "HEXAGONAL",
            Self::Cubic => "CUBIC",
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "TRICLINIC" => Some(Self::Triclinic),
            "MONOCLINIC" => Some(Self::Monoclinic),
            "ORTHORHOMBIC" => Some(Self::Orthorhombic),
            "TETRAGONAL" => Some(Self::Tetragonal),
            "TRIGONAL" | "RHOMBOHEDRAL" => Some(Self::Trigonal),
            "HEXAGONAL" => Some(Self::Hexagonal),
            "CUBIC" => Some(Self::Cubic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrystalInfo {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub nsg: i32,
}

impl Default for CrystalInfo {
    fn default() -> Self {
        Self::with_space_group(100.0, 100.0, 100.0, 90.0, 90.0, 90.0, 1)
    }
}

impl CrystalInfo {
    pub const TRICLINIC: i32 = 1;
    pub const MONOCLINIC: i32 = 2;
    pub const ORTHORHOMBIC: i32 = 3;
    pub const TETRAGONAL: i32 = 4;
    pub const TRIGONAL: i32 = 5;
    pub const HEXAGONAL: i32 = 6;
    pub const CUBIC: i32 = 7;

    pub fn new(a: f64, b: f64, c: f64, alpha: f64, beta: f64, gamma: f64) -> Self {
        Self::with_space_group(a, b, c, alpha, beta, gamma, 1)
    }

    pub fn with_space_group(
        a: f64,
        b: f64,
        c: f64,
        alpha: f64,
        beta: f64,
        gamma: f64,
        nsg: i32,
    ) -> Self {
        Self {
            a,
            b,
            c,
            alpha,
            beta,
            gamma,
            nsg,
        }
    }

    pub fn set_cell_dimension(
        &mut self,
        a: f64,
        b: f64,
        c: f64,
        alpha: f64,
        beta: f64,
        gamma: f64,
    ) {
        self.a = a;
        self.b = b;
        self.c = c;
        self.alpha = alpha;
        self.beta = beta;
        self.gamma = gamma;
    }

    pub fn set_a(&mut self, value: f64) {
        self.a = value;
    }

    pub fn set_b(&mut self, value: f64) {
        self.b = value;
    }

    pub fn set_c(&mut self, value: f64) {
        self.c = value;
    }

    pub fn set_alpha(&mut self, value: f64) {
        self.alpha = value;
    }

    pub fn set_beta(&mut self, value: f64) {
        self.beta = value;
    }

    pub fn beta(&self) -> f64 {
        self.beta
    }

    pub fn set_gamma(&mut self, value: f64) {
        self.gamma = value;
    }

    pub fn nsg(&self) -> i32 {
        self.nsg
    }

    pub fn set_nsg(&mut self, value: i32) {
        self.nsg = value;
    }

    pub fn space_group(&self) -> i32 {
        self.nsg
    }

    pub fn spacegroup(&self) -> i32 {
        self.space_group()
    }

    pub fn set_space_group(&mut self, value: i32) {
        self.nsg = value;
    }

    pub fn set_spacegroup(&mut self, value: i32) {
        self.set_space_group(value);
    }

    pub fn orth_mat(&self) -> Matrix4 {
        let alpha = self.alpha.to_radians();
        let beta = self.beta.to_radians();
        let gamma = self.gamma.to_radians();
        let cos_star = (beta.cos() * gamma.cos() - alpha.cos()) / (beta.sin() * gamma.sin());
        let sin_star = (1.0 - cos_star * cos_star).max(0.0).sqrt();

        let mut matrix = Matrix4::IDENTITY;
        matrix.values[0][0] = self.a;
        matrix.values[0][1] = self.b * gamma.cos();
        matrix.values[0][2] = self.c * beta.cos();
        matrix.values[1][0] = 0.0;
        matrix.values[1][1] = self.b * gamma.sin();
        matrix.values[1][2] = -self.c * beta.sin() * cos_star;
        matrix.values[2][0] = 0.0;
        matrix.values[2][1] = 0.0;
        matrix.values[2][2] = self.c * beta.sin() * sin_star;
        matrix
    }

    pub fn frac_mat(&self) -> Matrix4 {
        self.orth_mat().invert().unwrap_or(Matrix4::IDENTITY)
    }

    pub fn get_frac_mat(&self) -> Matrix4 {
        self.frac_mat()
    }

    pub fn get_orth_mat(&self) -> Matrix4 {
        self.orth_mat()
    }

    pub fn frac_to_orth(&self, pos: Vector4) -> Vector4 {
        let mut value = pos;
        self.orth_mat().xform3d(&mut value);
        value
    }

    pub fn orth_to_frac(&self, pos: Vector4) -> Vector4 {
        let mut value = pos;
        self.frac_mat().xform3d(&mut value);
        value
    }

    pub fn frac_dist(&self, lhs: Vector4, rhs: Vector4) -> f64 {
        (self.frac_to_orth(lhs) - self.frac_to_orth(rhs)).length3d()
    }

    pub fn lattice_name(&self) -> String {
        SymmOpManager::new()
            .lattice_for_space_group(self.nsg)
            .map(LatticeSystem::name)
            .unwrap_or("")
            .to_string()
    }

    pub fn lattice(&self) -> String {
        self.lattice_name()
    }

    pub fn hm_space_group_name(&self) -> String {
        SymmOpManager::new()
            .get_cname(self.nsg)
            .unwrap_or("(unknown)")
            .to_string()
    }

    pub fn hm_spacegroup(&self) -> String {
        self.hm_space_group_name()
    }

    pub fn space_group_name(&self) -> String {
        SymmOpManager::new()
            .get_name(self.nsg)
            .unwrap_or("(unknown)")
            .to_string()
    }

    pub fn sys_name_to_id(name: &str) -> i32 {
        LatticeSystem::from_name(name)
            .map(|value| value as i32)
            .unwrap_or(-1)
    }

    pub fn id_to_sys_name(id: i32) -> String {
        match id {
            Self::TRICLINIC => LatticeSystem::Triclinic.name(),
            Self::MONOCLINIC => LatticeSystem::Monoclinic.name(),
            Self::ORTHORHOMBIC => LatticeSystem::Orthorhombic.name(),
            Self::TETRAGONAL => LatticeSystem::Tetragonal.name(),
            Self::TRIGONAL => LatticeSystem::Trigonal.name(),
            Self::HEXAGONAL => LatticeSystem::Hexagonal.name(),
            Self::CUBIC => LatticeSystem::Cubic.name(),
            _ => "",
        }
        .to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymmetryOperator {
    pub name: String,
    pub matrix: Matrix4,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpaceGroup {
    pub id: i32,
    pub lattice: LatticeSystem,
    pub name: String,
    pub canonical_name: String,
    pub operators: Vec<SymmetryOperator>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymmOpManager {
    groups: BTreeMap<i32, SpaceGroup>,
}

impl Default for SymmOpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SymmOpManager {
    pub fn new() -> Self {
        let mut manager = Self {
            groups: BTreeMap::new(),
        };
        manager.register_builtin_groups();
        manager
    }

    /// Parse a symmetry-operator database in the `symop.dat` format (the same
    /// file the C++ `SymOpDB::loadSymLibFile` reads). Each space group is a
    /// `>id,nasu,name,cname,SYSTEM` header followed by operators; every operator
    /// is a symbolic label line (e.g. `-X,Y+1/2,-Z`) and the 3 numeric rows that
    /// encode its matrix. The numeric rows are authoritative (mirrors C++
    /// `readSymOp`); the symbolic label is kept as the operator name.
    pub fn from_symop_dat(input: &str) -> Self {
        Self {
            groups: parse_symop_dat(input),
        }
    }

    pub fn get_sg_names_json(&self, lattice: &str) -> String {
        let Some(lattice) = LatticeSystem::from_name(lattice) else {
            return "[]".to_string();
        };
        let body = self
            .groups
            .values()
            .filter(|group| group.lattice == lattice)
            .map(|group| {
                format!(
                    "{{\"cname\":\"{}\",\"id\":{}}}",
                    json_escape(&group.canonical_name),
                    group.id
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{body}]")
    }

    pub fn get_cname(&self, nsg: i32) -> Option<&str> {
        self.groups
            .get(&nsg)
            .map(|group| group.canonical_name.as_str())
    }

    pub fn get_c_name(&self, nsg: i32) -> Option<&str> {
        self.get_cname(nsg)
    }

    pub fn get_name(&self, nsg: i32) -> Option<&str> {
        self.groups.get(&nsg).map(|group| group.name.as_str())
    }

    pub fn lattice_for_space_group(&self, nsg: i32) -> Option<LatticeSystem> {
        self.groups.get(&nsg).map(|group| group.lattice)
    }

    pub fn symmetry_operators(&self, nsg: i32) -> Option<&[SymmetryOperator]> {
        self.groups
            .get(&nsg)
            .map(|group| group.operators.as_slice())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn change_xtal_info(
        &self,
        _object_id: u64,
        a: f64,
        b: f64,
        c: f64,
        alpha: f64,
        beta: f64,
        gamma: f64,
        nsg: i32,
    ) -> CrystalInfo {
        CrystalInfo::with_space_group(a, b, c, alpha, beta, gamma, nsg)
    }

    /// Load the full built-in space-group table (all 230 groups plus alternate
    /// settings) from the vendored `symop.dat`. Replaces the former hand-coded
    /// P1/P-1-only stub, so crystallographic operations on non-triclinic cells
    /// now get their real symmetry operators instead of falling back to P1.
    fn register_builtin_groups(&mut self) {
        self.groups = builtin_symop_table().clone();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymmRenderer {
    maxop: i32,
    extent: f64,
    unitcell: bool,
    autoupdate: bool,
    center: Vector4,
    operators: Vec<SymmetryOperator>,
    update_required: bool,
}

impl Default for SymmRenderer {
    fn default() -> Self {
        Self {
            maxop: 1000,
            extent: 50.0,
            unitcell: true,
            autoupdate: false,
            center: Vector4::ZERO,
            operators: Vec::new(),
            update_required: true,
        }
    }
}

impl SymmRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "*symm"
    }

    pub fn maxop(&self) -> i32 {
        self.maxop
    }

    pub fn set_maxop(&mut self, value: i32) {
        self.maxop = value;
    }

    pub fn extent(&self) -> f64 {
        self.extent
    }

    pub fn set_extent(&mut self, value: f64) {
        self.extent = value;
        self.update_required = true;
    }

    pub fn unitcell(&self) -> bool {
        self.unitcell
    }

    pub fn set_unitcell(&mut self, value: bool) {
        self.unitcell = value;
        self.update_required = true;
    }

    pub fn autoupdate(&self) -> bool {
        self.autoupdate
    }

    pub fn set_autoupdate(&mut self, value: bool) {
        self.autoupdate = value;
        self.update_required = true;
    }

    pub fn center(&self) -> Vector4 {
        self.center
    }

    pub fn set_center(&mut self, value: Vector4) {
        self.center = value;
        self.update_required = true;
    }

    pub fn is_hit_test_supported(&self) -> bool {
        true
    }

    pub fn update_required(&self) -> bool {
        self.update_required
    }

    pub fn clear(&mut self) {
        self.operators.clear();
        self.update_required = true;
    }

    pub fn set_operators(&mut self, operators: Vec<SymmetryOperator>) {
        self.operators = operators;
        self.update_required = false;
    }

    pub fn operators(&self) -> &[SymmetryOperator] {
        &self.operators
    }

    pub fn get_xform_matrix(&self, symid: usize) -> Matrix4 {
        self.operators
            .get(symid)
            .map(|operator| operator.matrix)
            .unwrap_or(Matrix4::IDENTITY)
    }

    /// Generate the per-unit-cell symmetry-mate transforms for a molecule
    /// centred at `center_cart` (Cartesian) in `crystal`, looking the space
    /// group's operators up in `manager`, and store them as this renderer's
    /// operators. Returns the number generated. Port of C++
    /// `SymmRenderer::genByCell`; needs the full symop table (Increment 4) to
    /// produce real packing for any non-P1 group.
    pub fn generate_by_cell(
        &mut self,
        center_cart: Vector4,
        crystal: &CrystalInfo,
        manager: &SymmOpManager,
    ) -> usize {
        let operators = manager
            .symmetry_operators(crystal.space_group())
            .unwrap_or(&[]);
        let mates = symm_mates_by_cell(center_cart, crystal, operators);
        let count = mates.len();
        self.set_operators(mates);
        count
    }

    /// Generate all symmetry mates within the renderer's `extent` of its view
    /// `center`, for a molecule centred at `mol_center_cart`. Stores them as the
    /// renderer's operators and returns the count. Port of C++
    /// `SymmRenderer::genByExtent` (respects `maxop`).
    pub fn generate_by_extent(
        &mut self,
        mol_center_cart: Vector4,
        crystal: &CrystalInfo,
        manager: &SymmOpManager,
    ) -> usize {
        let operators = manager
            .symmetry_operators(crystal.space_group())
            .unwrap_or(&[]);
        let mates = symm_mates_by_extent(
            mol_center_cart,
            self.center,
            self.extent,
            self.maxop.max(0) as usize,
            crystal,
            operators,
        );
        let count = mates.len();
        self.set_operators(mates);
        count
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitCellRenderer {
    disp_list: DispListRenderer,
    width: f64,
    color: SolidColor,
    labelcolor: SolidColor,
}

impl Default for UnitCellRenderer {
    fn default() -> Self {
        Self {
            disp_list: DispListRenderer::new(),
            width: 1.0,
            color: SolidColor::from_rgb(1.0, 1.0, 0.0),
            labelcolor: SolidColor::from_rgb(1.0, 0.0, 0.0),
        }
    }
}

impl UnitCellRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "*unitcell"
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
        self.disp_list.invalidate_display_cache();
    }

    pub fn color(&self) -> &SolidColor {
        &self.color
    }

    pub fn set_color(&mut self, value: SolidColor) {
        self.color = value;
        self.disp_list.invalidate_display_cache();
    }

    pub fn labelcolor(&self) -> &SolidColor {
        &self.labelcolor
    }

    pub fn set_labelcolor(&mut self, value: SolidColor) {
        self.labelcolor = value;
        self.disp_list.invalidate_display_cache();
    }

    pub fn is_hit_test_supported(&self) -> bool {
        false
    }

    pub fn center_for_crystal(&self, crystal: &CrystalInfo) -> Vector4 {
        crystal.frac_to_orth(Vector4::new3(0.5, 0.5, 0.5))
    }

    pub fn unit_cell_edges(&self, crystal: &CrystalInfo) -> Vec<(Vector4, Vector4)> {
        let vertices = [
            Vector4::new3(0.0, 0.0, 0.0),
            Vector4::new3(1.0, 0.0, 0.0),
            Vector4::new3(1.0, 1.0, 0.0),
            Vector4::new3(0.0, 1.0, 0.0),
            Vector4::new3(0.0, 0.0, 1.0),
            Vector4::new3(1.0, 0.0, 1.0),
            Vector4::new3(1.0, 1.0, 1.0),
            Vector4::new3(0.0, 1.0, 1.0),
        ]
        .map(|vertex| crystal.frac_to_orth(vertex));
        [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ]
        .into_iter()
        .map(|(lhs, rhs)| (vertices[lhs], vertices[rhs]))
        .collect()
    }

    pub fn display_cache_valid(&self) -> bool {
        self.disp_list.is_display_cache_valid()
    }

    pub fn mark_display_cache_valid(&mut self) {
        self.disp_list.mark_display_cache_valid();
    }
}

/// Build the per-unit-cell symmetry-mate transforms (in Cartesian space) for a
/// molecule centred at `center_cart`, given the crystal and its fractional
/// symmetry `operators`. Port of C++ `SymmRenderer::genByCell`: for each
/// operator, the transformed centre of mass is shifted back into the home unit
/// cell, so the Cartesian transform is `orth * trans * op_frac * frac` where
/// `frac`/`orth` are the (cart->frac)/(frac->cart) matrices and `trans` is the
/// integer cell shift `-floor(op . center_frac)`.
pub fn symm_mates_by_cell(
    center_cart: Vector4,
    crystal: &CrystalInfo,
    operators: &[SymmetryOperator],
) -> Vec<SymmetryOperator> {
    let frac = crystal.frac_mat();
    let orth = crystal.orth_mat();
    let raw = crystal.orth_to_frac(center_cart);
    // Treat the centre as a point (w = 1) so an operator's fractional
    // translation is applied; `Vector4::new3` (e.g. atom positions) yields w = 0,
    // which would otherwise drop the translation in `transform_point`.
    let center_frac = Vector4::new4(raw.x, raw.y, raw.z, 1.0);

    operators
        .iter()
        .map(|op| {
            let ftmp = op.matrix.transform_point(center_frac);
            let trans = Matrix4::make_trans_mat(Vector4::new3(
                -ftmp.x.floor(),
                -ftmp.y.floor(),
                -ftmp.z.floor(),
            ));
            let matrix = orth.mul_mat(trans).mul_mat(op.matrix).mul_mat(frac);
            SymmetryOperator {
                name: op.name.clone(),
                matrix,
            }
        })
        .collect()
}

/// Generate every symmetry mate whose centre of mass lies within `extent` of
/// `view_center_cart`, expanding over unit-cell translations. Port of C++
/// `SymmRenderer::genByExtent`/`searchX`/`searchY`/`searchZ`/`checkPos`: for
/// each operator a base transform is built (operator + the cell shift that pulls
/// its mate into the home cell), then integer cell translations are flood-
/// searched outward from the view cell, keeping the Cartesian transform of every
/// mate within `extent`. The identity (the molecule itself) is skipped but still
/// drives the shell search; at most `max_ops` mates are returned.
pub fn symm_mates_by_extent(
    mol_center_cart: Vector4,
    view_center_cart: Vector4,
    extent: f64,
    max_ops: usize,
    crystal: &CrystalInfo,
    operators: &[SymmetryOperator],
) -> Vec<SymmetryOperator> {
    let frac = crystal.frac_mat();
    let orth = crystal.orth_mat();
    let fmol_raw = crystal.orth_to_frac(mol_center_cart);
    let nc = Vector4::new3(fmol_raw.x.floor(), fmol_raw.y.floor(), fmol_raw.z.floor());
    let nv_raw = crystal.orth_to_frac(view_center_cart);
    let nv = Vector4::new3(nv_raw.x.floor(), nv_raw.y.floor(), nv_raw.z.floor());
    // mol centre reduced into the home cell (w = 1 so operator translations apply)
    let fmol_reduced = Vector4::new4(fmol_raw.x - nc.x, fmol_raw.y - nc.y, fmol_raw.z - nc.z, 1.0);

    let mut search = ExtentSearch {
        orth,
        base: Matrix4::IDENTITY,
        name: String::new(),
        mol_center: Vector4::new4(mol_center_cart.x, mol_center_cart.y, mol_center_cart.z, 1.0),
        view_center: view_center_cart,
        extent,
        max_ops,
        out: Vec::new(),
        terminated: false,
    };

    for op in operators {
        if search.terminated {
            break;
        }
        let fmolx = op.matrix.transform_point(fmol_reduced);
        let xtr = Vector4::new3(-fmolx.x.floor(), -fmolx.y.floor(), -fmolx.z.floor());
        let neg_nc = Vector4::new3(-nc.x, -nc.y, -nc.z);
        search.base = Matrix4::make_trans_mat(xtr)
            .mul_mat(op.matrix)
            .mul_mat(Matrix4::make_trans_mat(neg_nc))
            .mul_mat(frac);
        search.name = op.name.clone();
        search.search_x(nv);
    }

    search.out
}

/// Recursive cell-translation flood search state for `symm_mates_by_extent`.
struct ExtentSearch {
    orth: Matrix4,
    base: Matrix4,
    name: String,
    mol_center: Vector4,
    view_center: Vector4,
    extent: f64,
    max_ops: usize,
    out: Vec<SymmetryOperator>,
    terminated: bool,
}

impl ExtentSearch {
    /// Test one integer cell translation; add its mate if within `extent` and
    /// not the identity. Returns whether the translated centre is within extent.
    fn check_pos(&mut self, trn: Vector4) -> bool {
        let mat = self
            .orth
            .mul_mat(Matrix4::make_trans_mat(trn))
            .mul_mat(self.base);
        let fx = mat.transform_point(self.mol_center);
        if (fx - self.view_center).length3d() > self.extent {
            return false;
        }
        if matrix_is_identity(&mat, 1.0e-4) {
            return true; // the molecule itself: keep searching, don't add
        }
        if self.out.len() < self.max_ops {
            self.out.push(SymmetryOperator {
                name: self.name.clone(),
                matrix: mat,
            });
        } else {
            self.terminated = true;
        }
        true
    }

    fn search_z(&mut self, trn: Vector4) -> i32 {
        let mut nok = 0;
        let mut iz = 0;
        loop {
            let mut dnok = 0;
            let plus = Vector4::new3(trn.x, trn.y, trn.z + iz as f64);
            if self.check_pos(plus) {
                dnok += 1;
            }
            if self.terminated {
                return 0;
            }
            if iz != 0 {
                let minus = Vector4::new3(trn.x, trn.y, trn.z - iz as f64);
                if self.check_pos(minus) {
                    dnok += 1;
                }
                if self.terminated {
                    return 0;
                }
            }
            nok += dnok;
            if dnok == 0 && iz >= 2 {
                break;
            }
            iz += 1;
        }
        nok
    }

    fn search_y(&mut self, trn: Vector4) -> i32 {
        let mut nok = 0;
        let mut iy = 0;
        loop {
            let mut dnok = 0;
            dnok += self.search_z(Vector4::new3(trn.x, trn.y + iy as f64, trn.z));
            if self.terminated {
                return 0;
            }
            if iy != 0 {
                dnok += self.search_z(Vector4::new3(trn.x, trn.y - iy as f64, trn.z));
                if self.terminated {
                    return 0;
                }
            }
            nok += dnok;
            if dnok == 0 && iy >= 2 {
                break;
            }
            iy += 1;
        }
        nok
    }

    fn search_x(&mut self, trn: Vector4) -> i32 {
        let mut nok = 0;
        let mut ix = 0;
        loop {
            let mut dnok = 0;
            dnok += self.search_y(Vector4::new3(trn.x + ix as f64, trn.y, trn.z));
            if self.terminated {
                return 0;
            }
            if ix != 0 {
                dnok += self.search_y(Vector4::new3(trn.x - ix as f64, trn.y, trn.z));
                if self.terminated {
                    return 0;
                }
            }
            nok += dnok;
            if dnok == 0 && ix >= 2 {
                break;
            }
            ix += 1;
        }
        nok
    }
}

/// True when `mat` equals the identity within `eps` (C++ `Matrix4D::isIdent`).
fn matrix_is_identity(mat: &Matrix4, eps: f64) -> bool {
    for i in 0..4 {
        for j in 0..4 {
            let expect = if i == j { 1.0 } else { 0.0 };
            if (mat.values[i][j] - expect).abs() > eps {
                return false;
            }
        }
    }
    true
}

/// The vendored full space-group table, parsed once. Mirrors the C++
/// `SymOpDB` singleton, which loads `symop.dat` lazily and caches it.
fn builtin_symop_table() -> &'static BTreeMap<i32, SpaceGroup> {
    static TABLE: OnceLock<BTreeMap<i32, SpaceGroup>> = OnceLock::new();
    TABLE.get_or_init(|| parse_symop_dat(include_str!("../data/symop.dat")))
}

/// Parse the whole `symop.dat` body into a space-group table. Port of C++
/// `SymOpDB::loadSymLibFile`: `>` lines start a group; every other non-empty
/// line is an operator label, immediately followed by 3 numeric matrix rows.
/// The numeric rows are authoritative; if a label is *not* followed by numeric
/// rows (symbolic-only input), the label itself is parsed symbolically.
fn parse_symop_dat(input: &str) -> BTreeMap<i32, SpaceGroup> {
    let lines: Vec<&str> = input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    let mut groups = BTreeMap::new();
    let mut current: Option<SpaceGroup> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(header) = line.strip_prefix('>') {
            if let Some(group) = current.take() {
                // First definition wins on duplicate id (C++ std::map::insert).
                groups.entry(group.id).or_insert(group);
            }
            current = parse_sg_header(header);
            i += 1;
        } else if let Some(group) = current.as_mut() {
            // Operator: label line, then (usually) 3 numeric rows for its matrix.
            if let Some(matrix) = try_parse_numeric_rows(&lines, i + 1) {
                group.operators.push(SymmetryOperator {
                    name: line.to_string(),
                    matrix,
                });
                i += 4;
            } else {
                if let Some(operator) = parse_symop_line(line) {
                    group.operators.push(operator);
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    if let Some(group) = current.take() {
        groups.entry(group.id).or_insert(group);
    }
    groups
}

/// Parse a `>id,nasu,name,cname,SYSTEM` header (the leading `>` already stripped).
fn parse_sg_header(header: &str) -> Option<SpaceGroup> {
    let parts = header.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() < 5 {
        return None;
    }
    let id = parts[0].parse::<i32>().ok()?;
    let lattice = LatticeSystem::from_name(parts[4])?;
    Some(SpaceGroup {
        id,
        lattice,
        name: parts[2].to_string(),
        canonical_name: parts[3].to_string(),
        operators: Vec::new(),
    })
}

/// Build a `Matrix4` from the 3 numeric rows at `lines[start..start+3]`, each
/// `a,b,c,trans_num,trans_den` (mirrors C++ `readSymOp`: translation = num/den).
/// Returns `None` unless all three are well-formed 5-integer rows — so operator
/// labels (3 fields) and `>` headers (whose first field isn't an integer) are
/// never mistaken for matrix rows.
fn try_parse_numeric_rows(lines: &[&str], start: usize) -> Option<Matrix4> {
    if start + 3 > lines.len() {
        return None;
    }
    let mut matrix = Matrix4::IDENTITY;
    for row in 0..3 {
        let fields = lines[start + row]
            .split(',')
            .map(str::trim)
            .collect::<Vec<_>>();
        if fields.len() != 5 {
            return None;
        }
        let mut nums = [0i32; 5];
        for (slot, field) in nums.iter_mut().zip(fields.iter()) {
            *slot = field.parse::<i32>().ok()?;
        }
        if nums[4] == 0 {
            return None;
        }
        matrix.values[row][0] = nums[0] as f64;
        matrix.values[row][1] = nums[1] as f64;
        matrix.values[row][2] = nums[2] as f64;
        matrix.values[row][3] = nums[3] as f64 / nums[4] as f64;
    }
    Some(matrix)
}

fn parse_symop_line(line: &str) -> Option<SymmetryOperator> {
    let name = line.trim().to_string();
    let parts = name.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let mut matrix = Matrix4::IDENTITY;
    for (row, expr) in parts.iter().enumerate() {
        let (coeffs, offset) = parse_symop_expr(expr)?;
        matrix.values[row][0] = coeffs[0];
        matrix.values[row][1] = coeffs[1];
        matrix.values[row][2] = coeffs[2];
        matrix.values[row][3] = offset;
    }
    Some(SymmetryOperator { name, matrix })
}

fn parse_symop_expr(expr: &str) -> Option<([f64; 3], f64)> {
    let normalized = expr.replace('-', "+-");
    let mut coeffs = [0.0; 3];
    let mut offset = 0.0;
    for raw in normalized.split('+') {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        let sign = if token.starts_with('-') { -1.0 } else { 1.0 };
        let body = token.trim_start_matches('-');
        // Axis match is case-insensitive (symop.dat uses upper-case X/Y/Z, while
        // some callers pass lower-case); fractions read the original body.
        match body.to_ascii_lowercase().as_str() {
            "x" => coeffs[0] += sign,
            "y" => coeffs[1] += sign,
            "z" => coeffs[2] += sign,
            _ => offset += sign * parse_fraction(body)?,
        }
    }
    Some((coeffs, offset))
}

fn parse_fraction(value: &str) -> Option<f64> {
    if let Some((num, den)) = value.split_once('/') {
        let num = num.trim().parse::<f64>().ok()?;
        let den = den.trim().parse::<f64>().ok()?;
        if den.abs() < 1.0e-12 {
            None
        } else {
            Some(num / den)
        }
    } else {
        value.parse::<f64>().ok()
    }
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystal_info_defaults_and_transforms_match_reference() {
        let crystal = CrystalInfo::default();
        assert_eq!(crystal.a, 100.0);
        assert_eq!(crystal.b, 100.0);
        assert_eq!(crystal.c, 100.0);
        assert_eq!(crystal.alpha, 90.0);
        assert_eq!(crystal.beta, 90.0);
        assert_eq!(crystal.gamma, 90.0);
        assert_eq!(crystal.nsg, 1);
        assert_eq!(crystal.lattice_name(), "TRICLINIC");
        assert_eq!(crystal.hm_space_group_name(), "P 1");
        assert_eq!(crystal.space_group_name(), "P1");

        let orth = crystal.frac_to_orth(Vector4::new3(0.5, 0.25, 0.1));
        assert_eq!(orth, Vector4::new3(50.0, 25.0, 10.0));
        let frac = crystal.orth_to_frac(orth);
        assert!((frac.x - 0.5).abs() < 1.0e-12);
        assert!((frac.y - 0.25).abs() < 1.0e-12);
        assert!((frac.z - 0.1).abs() < 1.0e-12);
        assert_eq!(
            crystal.frac_dist(Vector4::ZERO, Vector4::new3(1.0, 0.0, 0.0)),
            100.0
        );
        assert_eq!(
            CrystalInfo::sys_name_to_id("rhombohedral"),
            CrystalInfo::TRIGONAL
        );
        assert_eq!(CrystalInfo::id_to_sys_name(CrystalInfo::CUBIC), "CUBIC");
    }

    #[test]
    fn symop_manager_builtin_and_parser_cover_reference_shape() {
        let manager = SymmOpManager::new();
        assert_eq!(manager.get_cname(1), Some("P 1"));
        assert_eq!(manager.get_name(2), Some("P-1"));
        assert!(manager.get_sg_names_json("TRICLINIC").contains("\"id\":1"));
        assert_eq!(manager.symmetry_operators(1).unwrap().len(), 1);
        assert_eq!(manager.symmetry_operators(2).unwrap().len(), 2);

        let parsed =
            SymmOpManager::from_symop_dat(">4,2,P21,P 1 21 1,MONOCLINIC\nx,y,z\n-x,y+1/2,-z\n");
        assert_eq!(parsed.get_cname(4), Some("P 1 21 1"));
        let ops = parsed.symmetry_operators(4).unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[1].matrix.values[0][0], -1.0);
        assert_eq!(ops[1].matrix.values[1][3], 0.5);

        let changed = parsed.change_xtal_info(10, 10.0, 20.0, 30.0, 90.0, 90.0, 120.0, 4);
        assert_eq!(changed.nsg, 4);
        assert_eq!(changed.gamma, 120.0);
    }

    // ---- Full space-group table (vendored symop.dat) ----

    fn rot3(op: &SymmetryOperator) -> [[f64; 3]; 3] {
        let m = op.matrix.values;
        [
            [m[0][0], m[0][1], m[0][2]],
            [m[1][0], m[1][1], m[1][2]],
            [m[2][0], m[2][1], m[2][2]],
        ]
    }

    fn det3(r: [[f64; 3]; 3]) -> f64 {
        r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
            - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
            + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
    }

    fn mul3(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
        let mut out = [[0.0; 3]; 3];
        for (i, orow) in out.iter_mut().enumerate() {
            for (j, ocell) in orow.iter_mut().enumerate() {
                *ocell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        out
    }

    #[test]
    fn builtin_table_loads_full_symop_database() {
        let manager = SymmOpManager::new();
        // symop.dat carries the 230 standard groups plus alternate settings.
        assert_eq!(manager.groups.len(), 266);
        // Every standard space-group number must resolve.
        for nsg in 1..=230 {
            assert!(
                manager.symmetry_operators(nsg).is_some(),
                "space group {nsg} missing"
            );
        }
        // Operator multiplicities from the International Tables.
        let nops = |nsg| manager.symmetry_operators(nsg).unwrap().len();
        assert_eq!(nops(1), 1); // P1
        assert_eq!(nops(2), 2); // P-1
        assert_eq!(nops(4), 2); // P21
        assert_eq!(nops(19), 4); // P212121 (most common protein group)
        assert_eq!(nops(96), 8); // P43212
        assert_eq!(nops(5), 4); // C2 (C-centred doubles the primitive 2)
    }

    #[test]
    fn builtin_table_operators_are_valid_crystallographic_symmetries() {
        let manager = SymmOpManager::new();
        for group in manager.groups.values() {
            for op in &group.operators {
                // Rotation part is a proper/improper symmetry: det = +/-1.
                let det = det3(rot3(op));
                assert!(
                    (det.abs() - 1.0).abs() < 1.0e-9,
                    "sg {} op {:?}: |det| = {det}",
                    group.id,
                    op.name
                );
                // Fractional translations are multiples of 1/12 and lie in [0,1).
                for row in 0..3 {
                    let t = op.matrix.values[row][3];
                    assert!((0.0..1.0).contains(&t), "sg {} trans {t}", group.id);
                    let twelfths = t * 12.0;
                    assert!(
                        (twelfths - twelfths.round()).abs() < 1.0e-9,
                        "sg {} trans {t} not a multiple of 1/12",
                        group.id
                    );
                }
            }
        }
    }

    #[test]
    fn builtin_table_numeric_matrix_matches_symbolic_label() {
        // The numeric rows are authoritative, but they must agree with the
        // human-readable label parsed independently (self-consistency oracle).
        let manager = SymmOpManager::new();
        for &nsg in &[1, 2, 4, 5, 19, 96, 152, 198] {
            for op in manager.symmetry_operators(nsg).unwrap() {
                let from_label = parse_symop_line(&op.name)
                    .unwrap_or_else(|| panic!("sg {nsg}: unparsable label {:?}", op.name));
                for row in 0..3 {
                    for col in 0..4 {
                        assert!(
                            (op.matrix.values[row][col] - from_label.matrix.values[row][col]).abs()
                                < 1.0e-9,
                            "sg {nsg} op {:?}: numeric vs symbolic mismatch at [{row}][{col}]",
                            op.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn builtin_table_point_group_is_closed() {
        // The set of rotation parts of a space group forms its point group and
        // must be closed under multiplication. Check P212121 (222) and P43212.
        let manager = SymmOpManager::new();
        for &nsg in &[19, 96] {
            let rots: Vec<[[f64; 3]; 3]> = manager
                .symmetry_operators(nsg)
                .unwrap()
                .iter()
                .map(rot3)
                .collect();
            for a in &rots {
                for b in &rots {
                    let prod = mul3(*a, *b);
                    assert!(
                        rots.iter().any(|r| {
                            (0..3).all(|i| (0..3).all(|j| (r[i][j] - prod[i][j]).abs() < 1.0e-9))
                        }),
                        "sg {nsg}: point group not closed"
                    );
                }
            }
        }
    }

    // ---- Crystal symmetry-mate generation (genByCell) ----

    fn ortho_crystal(sg: i32) -> CrystalInfo {
        CrystalInfo::with_space_group(20.0, 24.0, 28.0, 90.0, 90.0, 90.0, sg)
    }

    fn assert_in_home_cell(frac: Vector4, ctx: &str) {
        for comp in [frac.x, frac.y, frac.z] {
            assert!(
                (-1.0e-9..1.0).contains(&comp),
                "{ctx}: fractional {comp} not in [0,1)"
            );
        }
    }

    #[test]
    fn crystal_orth_and_frac_matrices_are_inverses() {
        let crystal = ortho_crystal(1);
        let prod = crystal.orth_mat().mul_mat(crystal.frac_mat());
        for i in 0..4 {
            for j in 0..4 {
                let expect = if i == j { 1.0 } else { 0.0 };
                assert!((prod.values[i][j] - expect).abs() < 1.0e-9);
            }
        }
    }

    #[test]
    fn symm_mates_by_cell_p1_is_identity() {
        let crystal = ortho_crystal(1);
        let manager = SymmOpManager::new();
        let ops = manager.symmetry_operators(1).unwrap();
        // Centre inside the unit cell (frac 0.5, 0.5, 0.5).
        let mates = symm_mates_by_cell(Vector4::new3(10.0, 12.0, 14.0), &crystal, ops);
        assert_eq!(mates.len(), 1);
        for i in 0..4 {
            for j in 0..4 {
                let expect = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (mates[0].matrix.values[i][j] - expect).abs() < 1.0e-9,
                    "P1 mate not identity at [{i}][{j}]"
                );
            }
        }
    }

    #[test]
    fn symm_mates_by_cell_place_centers_in_home_cell() {
        // The defining property of genByCell: every generated mate's centre of
        // mass lands inside the home unit cell [0,1), and the mate count equals
        // the space-group multiplicity.
        let manager = SymmOpManager::new();
        // General fractional centre (0.3, 0.4, 0.7) in Cartesian.
        let center = Vector4::new3(0.3 * 20.0, 0.4 * 24.0, 0.7 * 28.0);
        for &(sg, expected) in &[(1, 1), (2, 2), (4, 2), (19, 4), (96, 8)] {
            let crystal = ortho_crystal(sg);
            let ops = manager.symmetry_operators(sg).unwrap();
            let mates = symm_mates_by_cell(center, &crystal, ops);
            assert_eq!(mates.len(), expected, "sg {sg} mate count");
            let center_pt = Vector4::new4(center.x, center.y, center.z, 1.0);
            for mate in &mates {
                let moved = mate.matrix.transform_point(center_pt);
                assert_in_home_cell(crystal.orth_to_frac(moved), &format!("sg {sg}"));
            }
        }
    }

    #[test]
    fn symm_renderer_generate_by_cell_stores_packing_transforms() {
        let crystal = ortho_crystal(19); // P212121
        let manager = SymmOpManager::new();
        let mut renderer = SymmRenderer::new();
        let center = Vector4::new3(5.0, 6.0, 7.0);
        let count = renderer.generate_by_cell(center, &crystal, &manager);
        assert_eq!(count, 4);
        assert_eq!(renderer.operators().len(), 4);
        let center_pt = Vector4::new4(center.x, center.y, center.z, 1.0);
        let moved = renderer.get_xform_matrix(0).transform_point(center_pt);
        assert_in_home_cell(crystal.orth_to_frac(moved), "renderer mate 0");
    }

    // ---- Crystal symmetry-mate generation within an extent (genByExtent) ----

    #[test]
    fn symm_mates_by_extent_p1_finds_face_neighbours() {
        // P1 cubic 10A: the cell-translation lattice. With the molecule and view
        // centred together, an extent of 12 reaches only the 6 face neighbours
        // (distance 10); the 12 edge neighbours (distance ~14.1) are excluded.
        let crystal = CrystalInfo::with_space_group(10.0, 10.0, 10.0, 90.0, 90.0, 90.0, 1);
        let manager = SymmOpManager::new();
        let ops = manager.symmetry_operators(1).unwrap();
        let center = Vector4::new4(5.0, 5.0, 5.0, 1.0);

        let mates = symm_mates_by_extent(center, center, 12.0, 1000, &crystal, ops);
        assert_eq!(mates.len(), 6, "P1 extent 12 should give 6 face neighbours");

        // Every mate's centre is within the extent of the view centre.
        for mate in &mates {
            let moved = mate.matrix.transform_point(center);
            assert!((moved - center).length3d() <= 12.0 + 1.0e-6);
        }

        // Smaller than the nearest neighbour -> only the (skipped) identity.
        assert_eq!(
            symm_mates_by_extent(center, center, 5.0, 1000, &crystal, ops).len(),
            0
        );
        // Larger extent reaches strictly more mates.
        assert!(symm_mates_by_extent(center, center, 15.0, 1000, &crystal, ops).len() > 6);
    }

    #[test]
    fn symm_mates_by_extent_respects_max_ops() {
        let crystal = CrystalInfo::with_space_group(10.0, 10.0, 10.0, 90.0, 90.0, 90.0, 1);
        let manager = SymmOpManager::new();
        let ops = manager.symmetry_operators(1).unwrap();
        let center = Vector4::new4(5.0, 5.0, 5.0, 1.0);
        let mates = symm_mates_by_extent(center, center, 30.0, 4, &crystal, ops);
        assert_eq!(mates.len(), 4, "max_ops must cap the result");
    }

    #[test]
    fn symm_renderer_generate_by_extent_uses_extent_and_center() {
        let crystal = CrystalInfo::with_space_group(10.0, 10.0, 10.0, 90.0, 90.0, 90.0, 19);
        let manager = SymmOpManager::new();
        let mut renderer = SymmRenderer::new();
        renderer.set_center(Vector4::new4(5.0, 5.0, 5.0, 1.0));
        renderer.set_extent(8.0);
        let count =
            renderer.generate_by_extent(Vector4::new4(5.0, 5.0, 5.0, 1.0), &crystal, &manager);
        assert_eq!(renderer.operators().len(), count);
        // Every stored mate lies within the extent of the view centre.
        for i in 0..count {
            let moved = renderer
                .get_xform_matrix(i)
                .transform_point(Vector4::new4(5.0, 5.0, 5.0, 1.0));
            assert!((moved - renderer.center()).length3d() <= 8.0 + 1.0e-6);
        }
    }

    #[test]
    fn symm_renderer_defaults_and_operator_access_match_qif() {
        let mut renderer = SymmRenderer::new();
        assert_eq!(renderer.type_name(), "*symm");
        assert_eq!(renderer.maxop(), 1000);
        assert_eq!(renderer.extent(), 50.0);
        assert!(renderer.unitcell());
        assert!(!renderer.autoupdate());
        assert_eq!(renderer.center(), Vector4::ZERO);
        assert!(renderer.is_hit_test_supported());
        assert!(renderer.update_required());

        renderer.set_extent(25.0);
        renderer.set_unitcell(false);
        renderer.set_autoupdate(true);
        renderer.set_center(Vector4::new3(1.0, 2.0, 3.0));
        assert!(renderer.update_required());
        renderer.set_operators(vec![SymmetryOperator {
            name: "x,y,z".to_string(),
            matrix: Matrix4::IDENTITY,
        }]);
        assert!(!renderer.update_required());
        assert_eq!(renderer.get_xform_matrix(0), Matrix4::IDENTITY);
        assert_eq!(renderer.get_xform_matrix(99), Matrix4::IDENTITY);
        renderer.clear();
        assert!(renderer.operators().is_empty());
    }

    #[test]
    fn unit_cell_renderer_defaults_edges_and_cache_match_qif() {
        let crystal = CrystalInfo::new(10.0, 20.0, 30.0, 90.0, 90.0, 90.0);
        let mut renderer = UnitCellRenderer::new();
        assert_eq!(renderer.type_name(), "*unitcell");
        assert_eq!(renderer.width(), 1.0);
        assert_eq!(
            renderer.color().rgba8(),
            SolidColor::from_rgb(1.0, 1.0, 0.0).rgba8()
        );
        assert_eq!(
            renderer.labelcolor().rgba8(),
            SolidColor::from_rgb(1.0, 0.0, 0.0).rgba8()
        );
        assert!(!renderer.is_hit_test_supported());
        let center = renderer.center_for_crystal(&crystal);
        assert!((center.x - 5.0).abs() < 1.0e-12);
        assert!((center.y - 10.0).abs() < 1.0e-12);
        assert!((center.z - 15.0).abs() < 1.0e-12);
        assert_eq!(renderer.unit_cell_edges(&crystal).len(), 12);

        renderer.mark_display_cache_valid();
        renderer.set_width(2.0);
        assert!(!renderer.display_cache_valid());
    }
}
