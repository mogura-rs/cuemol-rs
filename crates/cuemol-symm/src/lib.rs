use cuemol_core::{Matrix4, Vector4};
use cuemol_gfx::SolidColor;
use cuemol_qsys::DispListRenderer;
use std::collections::BTreeMap;

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

    pub fn from_symop_dat(input: &str) -> Self {
        let mut manager = Self {
            groups: BTreeMap::new(),
        };
        let mut current: Option<SpaceGroup> = None;
        for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if let Some(header) = line.strip_prefix('>') {
                if let Some(group) = current.take() {
                    manager.groups.insert(group.id, group);
                }
                let parts = header.split(',').map(str::trim).collect::<Vec<_>>();
                if parts.len() < 5 {
                    continue;
                }
                let Some(lattice) = LatticeSystem::from_name(parts[4]) else {
                    continue;
                };
                let Ok(id) = parts[0].parse::<i32>() else {
                    continue;
                };
                current = Some(SpaceGroup {
                    id,
                    lattice,
                    name: parts[2].to_string(),
                    canonical_name: parts[3].to_string(),
                    operators: Vec::new(),
                });
            } else if let Some(group) = current.as_mut() {
                if let Some(operator) = parse_symop_line(line) {
                    group.operators.push(operator);
                }
            }
        }
        if let Some(group) = current.take() {
            manager.groups.insert(group.id, group);
        }
        if manager.groups.is_empty() {
            manager.register_builtin_groups();
        }
        manager
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

    fn register_builtin_groups(&mut self) {
        self.groups.insert(
            1,
            SpaceGroup {
                id: 1,
                lattice: LatticeSystem::Triclinic,
                name: "P1".to_string(),
                canonical_name: "P 1".to_string(),
                operators: vec![SymmetryOperator {
                    name: "x,y,z".to_string(),
                    matrix: Matrix4::IDENTITY,
                }],
            },
        );
        self.groups.insert(
            2,
            SpaceGroup {
                id: 2,
                lattice: LatticeSystem::Triclinic,
                name: "P-1".to_string(),
                canonical_name: "P -1".to_string(),
                operators: vec![
                    SymmetryOperator {
                        name: "x,y,z".to_string(),
                        matrix: Matrix4::IDENTITY,
                    },
                    SymmetryOperator {
                        name: "-x,-y,-z".to_string(),
                        matrix: Matrix4 {
                            values: [
                                [-1.0, 0.0, 0.0, 0.0],
                                [0.0, -1.0, 0.0, 0.0],
                                [0.0, 0.0, -1.0, 0.0],
                                [0.0, 0.0, 0.0, 1.0],
                            ],
                        },
                    },
                ],
            },
        );
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
        match body {
            "x" => coeffs[0] += sign,
            "y" => coeffs[1] += sign,
            "z" => coeffs[2] += sign,
            value => offset += sign * parse_fraction(value)?,
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
