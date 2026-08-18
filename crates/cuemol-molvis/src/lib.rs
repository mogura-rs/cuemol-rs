use cuemol_core::Vector4;
use cuemol_gfx::{ColorSpec, SolidColor};
use cuemol_molstr::{
    parse_selector, Atom, BfacColoring, Bond, BondOrder, MolCoord, MolRenderer, MolScene,
    ResidIterator, ResidueView,
};
use cuemol_qsys::{DrawObj, StyleManager};
use cuemol_render::{
    AtomIntrCapType, AtomIntrRenderMode, AtomIntrRenderOptions, MeshVertex, RenderAtom, RenderBond,
    RenderMesh, RenderSceneSnapshot,
};
use std::sync::OnceLock;

pub type CPK2Renderer = Cpk2Renderer;
pub type CPKRenderer = CpkRenderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoleculeRendererKind {
    Simple,
    Cpk,
    BallStick,
    Ribbon,
    Tube,
    AtomInteraction,
    AnIsoU,
    Disorder,
    NucleicAcid,
    Cartoon,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DistPickDrawObj {
    draw_obj: DrawObj,
    color: SolidColor,
    width: f64,
    positions: Vec<Vector4>,
}

impl Default for DistPickDrawObj {
    fn default() -> Self {
        Self {
            draw_obj: DrawObj::new(),
            color: SolidColor::from_rgba(1.0, 1.0, 1.0, 0.5),
            width: 4.0,
            positions: Vec::new(),
        }
    }
}

impl DistPickDrawObj {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "DistPickDrawObj"
    }

    pub fn is_enabled(&self) -> bool {
        self.draw_obj.is_enabled()
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.draw_obj.set_enabled(value);
        if !value {
            self.positions.clear();
        }
    }

    pub fn color(&self) -> &SolidColor {
        &self.color
    }

    pub fn set_color(&mut self, value: SolidColor) {
        self.color = value;
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
    }

    pub fn positions(&self) -> &[Vector4] {
        &self.positions
    }

    pub fn clear(&mut self) {
        self.positions.clear();
    }

    pub fn append_position(&mut self, position: Vector4) {
        self.positions.push(position);
    }

    pub fn append_atom(&mut self, molecule: &MolCoord, atom_id: i32) -> bool {
        let Some(atom) = molecule.atoms().iter().find(|atom| atom.serial == atom_id) else {
            return false;
        };
        self.append_position(atom.position);
        true
    }

    pub fn append(&mut self, molecule: &MolCoord, atom_id: i32) -> bool {
        self.append_atom(molecule, atom_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VdwRadii {
    h: f64,
    c: f64,
    n: f64,
    o: f64,
    s: f64,
    p: f64,
    x: f64,
}

impl Default for VdwRadii {
    fn default() -> Self {
        Self {
            h: 1.2,
            c: 1.7,
            n: 1.55,
            o: 1.52,
            s: 1.8,
            p: 1.8,
            x: 1.7,
        }
    }
}

impl VdwRadii {
    pub fn h(&self) -> f64 {
        self.h
    }

    pub fn set_h(&mut self, value: f64) {
        self.h = value;
    }

    pub fn c(&self) -> f64 {
        self.c
    }

    pub fn set_c(&mut self, value: f64) {
        self.c = value;
    }

    pub fn n(&self) -> f64 {
        self.n
    }

    pub fn set_n(&mut self, value: f64) {
        self.n = value;
    }

    pub fn o(&self) -> f64 {
        self.o
    }

    pub fn set_o(&mut self, value: f64) {
        self.o = value;
    }

    pub fn s(&self) -> f64 {
        self.s
    }

    pub fn set_s(&mut self, value: f64) {
        self.s = value;
    }

    pub fn p(&self) -> f64 {
        self.p
    }

    pub fn set_p(&mut self, value: f64) {
        self.p = value;
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn set_x(&mut self, value: f64) {
        self.x = value;
    }

    pub fn radius_for_element(&self, element: &str) -> f64 {
        match element.trim().to_ascii_uppercase().as_str() {
            "H" => self.h,
            "C" => self.c,
            "N" => self.n,
            "O" => self.o,
            "S" => self.s,
            "P" => self.p,
            _ => self.x,
        }
    }

    pub fn set_radius_for_element(&mut self, element: &str, value: f64) {
        match element.trim().to_ascii_uppercase().as_str() {
            "H" => self.h = value,
            "C" => self.c = value,
            "N" => self.n = value,
            "O" => self.o = value,
            "S" => self.s = value,
            "P" => self.p = value,
            _ => self.x = value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpkRenderer {
    vdw_radii: VdwRadii,
    detail: i32,
}

impl Default for CpkRenderer {
    fn default() -> Self {
        Self {
            vdw_radii: VdwRadii::default(),
            detail: 3,
        }
    }
}

impl CpkRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "cpk"
    }

    pub fn is_rend_bond(&self) -> bool {
        false
    }

    pub fn detail(&self) -> i32 {
        self.detail
    }

    pub fn set_detail(&mut self, value: i32) {
        self.detail = value;
    }

    pub fn vdw_radii(&self) -> VdwRadii {
        self.vdw_radii
    }

    pub fn vdw_radii_mut(&mut self) -> &mut VdwRadii {
        &mut self.vdw_radii
    }

    pub fn vdw_radius(&self, element: &str) -> f64 {
        self.vdw_radii.radius_for_element(element)
    }

    pub fn vdwr_h(&self) -> f64 {
        self.vdw_radii.h()
    }

    pub fn set_vdwr_h(&mut self, value: f64) {
        self.vdw_radii.set_h(value);
    }

    pub fn vdwr_c(&self) -> f64 {
        self.vdw_radii.c()
    }

    pub fn set_vdwr_c(&mut self, value: f64) {
        self.vdw_radii.set_c(value);
    }

    pub fn vdwr_n(&self) -> f64 {
        self.vdw_radii.n()
    }

    pub fn set_vdwr_n(&mut self, value: f64) {
        self.vdw_radii.set_n(value);
    }

    pub fn vdwr_o(&self) -> f64 {
        self.vdw_radii.o()
    }

    pub fn set_vdwr_o(&mut self, value: f64) {
        self.vdw_radii.set_o(value);
    }

    pub fn vdwr_s(&self) -> f64 {
        self.vdw_radii.s()
    }

    pub fn set_vdwr_s(&mut self, value: f64) {
        self.vdw_radii.set_s(value);
    }

    pub fn vdwr_p(&self) -> f64 {
        self.vdw_radii.p()
    }

    pub fn set_vdwr_p(&mut self, value: f64) {
        self.vdw_radii.set_p(value);
    }

    pub fn vdwr_x(&self) -> f64 {
        self.vdw_radii.x()
    }

    pub fn set_vdwr_x(&mut self, value: f64) {
        self.vdw_radii.set_x(value);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cpk2Renderer {
    cpk: CpkRenderer,
    gl_render_mode: i32,
    use_shader: bool,
    check_shader_ok: bool,
    sphere_gpu_prim_valid: bool,
}

impl Default for Cpk2Renderer {
    fn default() -> Self {
        let mut cpk = CpkRenderer::default();
        cpk.set_detail(10);
        Self {
            cpk,
            gl_render_mode: Self::REND_DEFAULT,
            use_shader: false,
            check_shader_ok: false,
            sphere_gpu_prim_valid: false,
        }
    }
}

impl Cpk2Renderer {
    pub const REND_DEFAULT: i32 = 0;
    pub const REND_SHADER: i32 = 1;
    pub const REND_VBO: i32 = 0;
    pub const REND_GLU: i32 = 0;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "cpk"
    }

    pub fn is_rend_bond(&self) -> bool {
        false
    }

    pub fn cpk(&self) -> &CpkRenderer {
        &self.cpk
    }

    pub fn cpk_mut(&mut self) -> &mut CpkRenderer {
        &mut self.cpk
    }

    pub fn detail(&self) -> i32 {
        self.cpk.detail()
    }

    pub fn set_detail(&mut self, value: i32) {
        self.cpk.set_detail(value);
    }

    pub fn vdw_radius(&self, element: &str) -> f64 {
        self.cpk.vdw_radius(element)
    }

    pub fn vdwr_h(&self) -> f64 {
        self.cpk.vdwr_h()
    }

    pub fn set_vdwr_h(&mut self, value: f64) {
        self.cpk.set_vdwr_h(value);
    }

    pub fn vdwr_c(&self) -> f64 {
        self.cpk.vdwr_c()
    }

    pub fn set_vdwr_c(&mut self, value: f64) {
        self.cpk.set_vdwr_c(value);
    }

    pub fn vdwr_n(&self) -> f64 {
        self.cpk.vdwr_n()
    }

    pub fn set_vdwr_n(&mut self, value: f64) {
        self.cpk.set_vdwr_n(value);
    }

    pub fn vdwr_o(&self) -> f64 {
        self.cpk.vdwr_o()
    }

    pub fn set_vdwr_o(&mut self, value: f64) {
        self.cpk.set_vdwr_o(value);
    }

    pub fn vdwr_s(&self) -> f64 {
        self.cpk.vdwr_s()
    }

    pub fn set_vdwr_s(&mut self, value: f64) {
        self.cpk.set_vdwr_s(value);
    }

    pub fn vdwr_p(&self) -> f64 {
        self.cpk.vdwr_p()
    }

    pub fn set_vdwr_p(&mut self, value: f64) {
        self.cpk.set_vdwr_p(value);
    }

    pub fn vdwr_x(&self) -> f64 {
        self.cpk.vdwr_x()
    }

    pub fn set_vdwr_x(&mut self, value: f64) {
        self.cpk.set_vdwr_x(value);
    }

    pub fn gl_render_mode(&self) -> i32 {
        self.gl_render_mode
    }

    pub fn get_gl_render_mode(&self) -> i32 {
        self.gl_render_mode()
    }

    pub fn glrender_mode(&self) -> i32 {
        self.gl_render_mode()
    }

    pub fn set_gl_render_mode(&mut self, value: i32) {
        self.gl_render_mode = value;
    }

    pub fn set_glrender_mode(&mut self, value: i32) {
        self.set_gl_render_mode(value);
    }

    pub fn use_shader(&self) -> bool {
        self.use_shader
    }

    pub fn set_use_shader(&mut self, value: bool) {
        self.use_shader = value;
    }

    pub fn check_shader_ok(&self) -> bool {
        self.check_shader_ok
    }

    pub fn set_check_shader_ok(&mut self, value: bool) {
        self.check_shader_ok = value;
    }

    pub fn sphere_gpu_prim_valid(&self) -> bool {
        self.sphere_gpu_prim_valid
    }

    pub fn set_sphere_gpu_prim_valid(&mut self, value: bool) {
        self.sphere_gpu_prim_valid = value;
    }

    pub fn invalidate_display_cache(&mut self) {
        self.sphere_gpu_prim_valid = false;
        self.check_shader_ok = false;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BallStickRenderer {
    bondw: f64,
    sphr: f64,
    detail: i32,
    ring: bool,
    thickness: f64,
    ring_color: SolidColor,
    valbond_mode: i32,
    gl_render_mode: i32,
    draw_ring_only: bool,
    use_shader: bool,
    check_shader_ok: bool,
    sphere_gpu_prim_valid: bool,
}

impl Default for BallStickRenderer {
    fn default() -> Self {
        Self {
            bondw: 0.2,
            sphr: 0.3,
            detail: 3,
            ring: false,
            thickness: 0.2,
            ring_color: SolidColor::from_rgb(1.0, 1.0, 0.75),
            valbond_mode: Self::VBMODE_OFF,
            gl_render_mode: Self::REND_DEFAULT,
            draw_ring_only: false,
            use_shader: false,
            check_shader_ok: false,
            sphere_gpu_prim_valid: false,
        }
    }
}

impl BallStickRenderer {
    pub const VBMODE_OFF: i32 = 0;
    pub const VBMODE_TYPE1: i32 = 1;
    pub const REND_DEFAULT: i32 = 0;
    pub const REND_SHADER: i32 = 1;
    pub const REND_VBO: i32 = 2;
    pub const REND_GLU: i32 = 3;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "ballstick"
    }

    pub fn is_rend_bond(&self) -> bool {
        !self.draw_ring_only
    }

    pub fn bondw(&self) -> f64 {
        self.bondw
    }

    pub fn set_bondw(&mut self, value: f64) {
        self.bondw = value;
    }

    pub fn sphr(&self) -> f64 {
        self.sphr
    }

    pub fn set_sphr(&mut self, value: f64) {
        self.sphr = value;
    }

    pub fn detail(&self) -> i32 {
        self.detail
    }

    pub fn set_detail(&mut self, value: i32) {
        self.detail = value;
    }

    pub fn ring(&self) -> bool {
        self.ring
    }

    pub fn set_ring(&mut self, value: bool) {
        self.ring = value;
    }

    pub fn thickness(&self) -> f64 {
        self.thickness
    }

    pub fn set_thickness(&mut self, value: f64) {
        self.thickness = value;
    }

    pub fn ring_color(&self) -> &SolidColor {
        &self.ring_color
    }

    pub fn ringcolor(&self) -> &SolidColor {
        self.ring_color()
    }

    pub fn set_ring_color(&mut self, value: SolidColor) {
        self.ring_color = value;
    }

    pub fn set_ringcolor(&mut self, value: SolidColor) {
        self.set_ring_color(value);
    }

    pub fn valbond_mode(&self) -> i32 {
        self.valbond_mode
    }

    pub fn get_vb_mode(&self) -> i32 {
        self.valbond_mode()
    }

    pub fn valbond(&self) -> i32 {
        self.valbond_mode()
    }

    pub fn set_valbond_mode(&mut self, value: i32) {
        self.valbond_mode = value;
    }

    pub fn set_vb_mode(&mut self, value: i32) {
        self.set_valbond_mode(value);
    }

    pub fn set_valbond(&mut self, value: i32) {
        self.set_valbond_mode(value);
    }

    pub fn gl_render_mode(&self) -> i32 {
        self.gl_render_mode
    }

    pub fn get_gl_render_mode(&self) -> i32 {
        self.gl_render_mode()
    }

    pub fn glrender_mode(&self) -> i32 {
        self.gl_render_mode()
    }

    pub fn set_gl_render_mode(&mut self, value: i32) {
        self.gl_render_mode = value;
    }

    pub fn set_glrender_mode(&mut self, value: i32) {
        self.set_gl_render_mode(value);
    }

    pub fn draw_ring_only(&self) -> bool {
        self.draw_ring_only
    }

    pub fn set_draw_ring_only(&mut self, value: bool) {
        self.draw_ring_only = value;
    }

    pub fn use_shader(&self) -> bool {
        self.use_shader
    }

    pub fn set_use_shader(&mut self, value: bool) {
        self.use_shader = value;
    }

    pub fn check_shader_ok(&self) -> bool {
        self.check_shader_ok
    }

    pub fn set_check_shader_ok(&mut self, value: bool) {
        self.check_shader_ok = value;
    }

    pub fn sphere_gpu_prim_valid(&self) -> bool {
        self.sphere_gpu_prim_valid
    }

    pub fn set_sphere_gpu_prim_valid(&mut self, value: bool) {
        self.sphere_gpu_prim_valid = value;
    }

    pub fn invalidate_display_cache(&mut self) {
        self.sphere_gpu_prim_valid = false;
        self.check_shader_ok = false;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnIsoURenderer {
    ball_stick: BallStickRenderer,
    drawdisc: bool,
    discscale: f64,
    discthick: f64,
    maxverts: i32,
}

impl Default for AnIsoURenderer {
    fn default() -> Self {
        Self {
            ball_stick: BallStickRenderer::default(),
            drawdisc: true,
            discscale: 1.1,
            discthick: 0.1,
            maxverts: 1_000_000,
        }
    }
}

impl AnIsoURenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "anisou"
    }

    pub fn ball_stick(&self) -> &BallStickRenderer {
        &self.ball_stick
    }

    pub fn ball_stick_mut(&mut self) -> &mut BallStickRenderer {
        &mut self.ball_stick
    }

    pub fn get_bondw(&self) -> f64 {
        self.ball_stick.bondw()
    }

    pub fn set_bondw(&mut self, value: f64) {
        self.ball_stick.set_bondw(value);
    }

    pub fn get_sphr(&self) -> f64 {
        self.ball_stick.sphr()
    }

    pub fn set_sphr(&mut self, value: f64) {
        self.ball_stick.set_sphr(value);
    }

    pub fn drawdisc(&self) -> bool {
        self.drawdisc
    }

    pub fn set_drawdisc(&mut self, value: bool) {
        self.drawdisc = value;
    }

    pub fn discscale(&self) -> f64 {
        self.discscale
    }

    pub fn set_discscale(&mut self, value: f64) {
        self.discscale = value;
    }

    pub fn discthick(&self) -> f64 {
        self.discthick
    }

    pub fn set_discthick(&mut self, value: f64) {
        self.discthick = value;
    }

    pub fn maxverts(&self) -> i32 {
        self.maxverts
    }

    pub fn get_glu_vert_max(&self) -> i32 {
        self.maxverts()
    }

    pub fn set_maxverts(&mut self, value: i32) {
        self.maxverts = value;
    }

    pub fn set_glu_vert_max(&mut self, value: i32) {
        self.set_maxverts(value);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomIntrElem {
    pub molecule_id: i32,
    pub molecule_name: String,
    pub mode: AtomIntrElemMode,
    pub selection: Option<String>,
    pub atom_id: i32,
    pub string_atom_id: String,
    pub position: Vector4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum AtomIntrElemMode {
    Selection = 0,
    AtomId = 1,
    Position = 2,
}

impl Default for AtomIntrElem {
    fn default() -> Self {
        Self {
            molecule_id: -1,
            molecule_name: String::new(),
            mode: AtomIntrElemMode::Position,
            selection: None,
            atom_id: -1,
            string_atom_id: String::new(),
            position: Vector4::ZERO,
        }
    }
}

impl AtomIntrElem {
    pub const AI_SEL: i32 = 0;
    pub const AI_AID: i32 = 1;
    pub const AI_POS: i32 = 2;

    pub fn set_molecule_name(&mut self, value: impl Into<String>) {
        self.molecule_id = -1;
        self.molecule_name = value.into();
    }

    pub fn set_selection(&mut self, value: impl Into<String>) {
        self.selection = Some(value.into());
        self.mode = AtomIntrElemMode::Selection;
    }

    pub fn set_atom_id(&mut self, value: i32) {
        self.atom_id = value;
        self.mode = AtomIntrElemMode::AtomId;
    }

    pub fn set_position(&mut self, value: Vector4) {
        self.position = value;
        self.mode = AtomIntrElemMode::Position;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomIntrData {
    pub mode: i32,
    pub elements: [AtomIntrElem; 4],
    pub label_cache_id: i32,
}

impl Default for AtomIntrData {
    fn default() -> Self {
        Self {
            mode: 0,
            elements: std::array::from_fn(|_| AtomIntrElem::default()),
            label_cache_id: -1,
        }
    }
}

impl AtomIntrData {
    pub fn distance_by_selection(
        molecule_id1: i32,
        selection1: impl Into<String>,
        molecule_id2: i32,
        selection2: impl Into<String>,
    ) -> Self {
        let mut value = Self {
            mode: 1,
            ..Self::default()
        };
        value.elements[0].set_selection(selection1);
        value.elements[0].molecule_id = molecule_id1;
        value.elements[1].set_selection(selection2);
        value.elements[1].molecule_id = molecule_id2;
        value
    }

    pub fn distance_by_id(
        molecule_id1: i32,
        atom_id1: i32,
        molecule_id2: i32,
        atom_id2: i32,
    ) -> Self {
        Self::by_ids(1, &[(molecule_id1, atom_id1), (molecule_id2, atom_id2)])
    }

    pub fn angle_by_id(values: [(i32, i32); 3]) -> Self {
        Self::by_ids(2, &values)
    }

    pub fn torsion_by_id(values: [(i32, i32); 4]) -> Self {
        Self::by_ids(3, &values)
    }

    pub fn kind(&self) -> Option<AtomIntrDefKind> {
        match self.mode {
            1 => Some(AtomIntrDefKind::Distance),
            2 => Some(AtomIntrDefKind::Angle),
            3 => Some(AtomIntrDefKind::Torsion),
            _ => None,
        }
    }

    fn by_ids(mode: i32, values: &[(i32, i32)]) -> Self {
        let mut result = Self {
            mode,
            ..Self::default()
        };
        for (element, (molecule_id, atom_id)) in
            result.elements.iter_mut().zip(values.iter().copied())
        {
            element.set_atom_id(atom_id);
            element.molecule_id = molecule_id;
        }
        result
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomIntrRenderer {
    client_mol_id: i32,
    mode: i32,
    showlabel: bool,
    detail: i32,
    stipples: [f64; 6],
    width: f64,
    color: SolidColor,
    start_cap_type: i32,
    end_cap_type: i32,
    arrow_height: f64,
    arrow_width: f64,
    font_size: f64,
    font_name: String,
    font_style: String,
    font_weight: String,
    defs: Vec<AtomIntrDef>,
}

impl Default for AtomIntrRenderer {
    fn default() -> Self {
        Self {
            client_mol_id: 0,
            mode: Self::AIR_FANCY,
            showlabel: false,
            detail: 5,
            stipples: [1.0, 1.0, -1.0, -1.0, -1.0, -1.0],
            width: 0.1,
            color: SolidColor::from_rgb(1.0, 1.0, 0.0),
            start_cap_type: Self::END_SPHERE,
            end_cap_type: Self::END_SPHERE,
            arrow_height: 1.0,
            arrow_width: 2.0,
            font_size: 12.0,
            font_name: "sans-serif".to_string(),
            font_style: "normal".to_string(),
            font_weight: "normal".to_string(),
            defs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomIntrDef {
    id: i32,
    kind: AtomIntrDefKind,
    selections: Vec<String>,
    atom_ids: Vec<i32>,
    mol_ids: Vec<i32>,
    positions: Vec<Vector4>,
    show_message: bool,
    active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomIntrDefKind {
    Distance,
    Angle,
    Torsion,
}

impl AtomIntrDefKind {
    fn mode(self) -> i32 {
        match self {
            Self::Distance => 1,
            Self::Angle => 2,
            Self::Torsion => 3,
        }
    }

    fn point_count(self) -> usize {
        self.mode() as usize + 1
    }
}

impl AtomIntrDef {
    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn kind(&self) -> AtomIntrDefKind {
        self.kind
    }

    pub fn selections(&self) -> &[String] {
        &self.selections
    }

    pub fn atom_ids(&self) -> &[i32] {
        &self.atom_ids
    }

    pub fn mol_ids(&self) -> &[i32] {
        &self.mol_ids
    }

    pub fn positions(&self) -> &[Vector4] {
        &self.positions
    }

    pub fn show_message(&self) -> bool {
        self.show_message
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn to_data(&self) -> AtomIntrData {
        if !self.active {
            return AtomIntrData::default();
        }
        let mut data = AtomIntrData {
            mode: match self.kind {
                AtomIntrDefKind::Distance => 1,
                AtomIntrDefKind::Angle => 2,
                AtomIntrDefKind::Torsion => 3,
            },
            ..AtomIntrData::default()
        };
        let count = data.mode as usize + 1;
        for index in 0..count {
            let molecule_id = self.mol_ids.get(index).copied().unwrap_or(-1);
            let element = &mut data.elements[index];
            element.molecule_id = molecule_id;
            if let Some(selection) = self.selections.get(index) {
                element.set_selection(selection.clone());
            } else if let Some(atom_id) = self.atom_ids.get(index).copied() {
                element.set_atom_id(atom_id);
            } else if let Some(position) = self.positions.get(index).copied() {
                element.set_position(position);
            }
        }
        data
    }
}

impl AtomIntrRenderer {
    pub const END_FLAT: i32 = 0;
    pub const END_SPHERE: i32 = 1;
    pub const END_ARROW: i32 = 2;
    pub const AIR_SIMPLE: i32 = 0;
    pub const AIR_FANCY: i32 = 1;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn client_mol_id(&self) -> i32 {
        self.client_mol_id
    }

    pub fn set_client_mol_id(&mut self, value: i32) {
        self.client_mol_id = value;
    }

    pub fn type_name(&self) -> &'static str {
        "atomintr"
    }

    pub fn mode(&self) -> i32 {
        self.mode
    }

    pub fn get_mode(&self) -> i32 {
        self.mode()
    }

    pub fn set_mode(&mut self, value: i32) {
        self.mode = value;
    }

    pub fn showlabel(&self) -> bool {
        self.showlabel
    }

    pub fn is_show_label(&self) -> bool {
        self.showlabel()
    }

    pub fn set_showlabel(&mut self, value: bool) {
        self.showlabel = value;
    }

    pub fn set_show_label(&mut self, value: bool) {
        self.set_showlabel(value);
    }

    pub fn detail(&self) -> i32 {
        self.detail
    }

    pub fn get_detail(&self) -> i32 {
        self.detail()
    }

    pub fn set_detail(&mut self, value: i32) {
        self.detail = value;
    }

    pub fn stipple(&self, index: usize) -> Option<f64> {
        self.stipples.get(index).copied()
    }

    pub fn set_stipple(&mut self, index: usize, value: f64) -> bool {
        match self.stipples.get_mut(index) {
            Some(slot) => {
                *slot = value;
                true
            }
            None => false,
        }
    }

    pub fn stipples(&self) -> [f64; 6] {
        self.stipples
    }

    pub fn stipple0(&self) -> f64 {
        self.stipples[0]
    }

    pub fn get_stipple0(&self) -> f64 {
        self.stipple0()
    }

    pub fn stipple_0(&self) -> f64 {
        self.stipple0()
    }

    pub fn set_stipple0(&mut self, value: f64) {
        self.stipples[0] = value;
    }

    pub fn set_stipple_0(&mut self, value: f64) {
        self.set_stipple0(value);
    }

    pub fn stipple1(&self) -> f64 {
        self.stipples[1]
    }

    pub fn get_stipple1(&self) -> f64 {
        self.stipple1()
    }

    pub fn stipple_1(&self) -> f64 {
        self.stipple1()
    }

    pub fn set_stipple1(&mut self, value: f64) {
        self.stipples[1] = value;
    }

    pub fn set_stipple_1(&mut self, value: f64) {
        self.set_stipple1(value);
    }

    pub fn stipple2(&self) -> f64 {
        self.stipples[2]
    }

    pub fn get_stipple2(&self) -> f64 {
        self.stipple2()
    }

    pub fn stipple_2(&self) -> f64 {
        self.stipple2()
    }

    pub fn set_stipple2(&mut self, value: f64) {
        self.stipples[2] = value;
    }

    pub fn set_stipple_2(&mut self, value: f64) {
        self.set_stipple2(value);
    }

    pub fn stipple3(&self) -> f64 {
        self.stipples[3]
    }

    pub fn get_stipple3(&self) -> f64 {
        self.stipple3()
    }

    pub fn stipple_3(&self) -> f64 {
        self.stipple3()
    }

    pub fn set_stipple3(&mut self, value: f64) {
        self.stipples[3] = value;
    }

    pub fn set_stipple_3(&mut self, value: f64) {
        self.set_stipple3(value);
    }

    pub fn stipple4(&self) -> f64 {
        self.stipples[4]
    }

    pub fn get_stipple4(&self) -> f64 {
        self.stipple4()
    }

    pub fn stipple_4(&self) -> f64 {
        self.stipple4()
    }

    pub fn set_stipple4(&mut self, value: f64) {
        self.stipples[4] = value;
    }

    pub fn set_stipple_4(&mut self, value: f64) {
        self.set_stipple4(value);
    }

    pub fn stipple5(&self) -> f64 {
        self.stipples[5]
    }

    pub fn get_stipple5(&self) -> f64 {
        self.stipple5()
    }

    pub fn stipple_5(&self) -> f64 {
        self.stipple5()
    }

    pub fn set_stipple5(&mut self, value: f64) {
        self.stipples[5] = value;
    }

    pub fn set_stipple_5(&mut self, value: f64) {
        self.set_stipple5(value);
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn get_width(&self) -> f64 {
        self.width()
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
    }

    pub fn color(&self) -> &SolidColor {
        &self.color
    }

    pub fn set_color(&mut self, value: SolidColor) {
        self.color = value;
    }

    pub fn start_cap_type(&self) -> i32 {
        self.start_cap_type
    }

    pub fn get_start_cap_type(&self) -> i32 {
        self.start_cap_type()
    }

    pub fn start_captype(&self) -> i32 {
        self.start_cap_type()
    }

    pub fn captype_start(&self) -> i32 {
        self.start_cap_type()
    }

    pub fn set_start_cap_type(&mut self, value: i32) {
        self.start_cap_type = value;
    }

    pub fn set_start_captype(&mut self, value: i32) {
        self.set_start_cap_type(value);
    }

    pub fn set_captype_start(&mut self, value: i32) {
        self.set_start_cap_type(value);
    }

    pub fn end_cap_type(&self) -> i32 {
        self.end_cap_type
    }

    pub fn get_end_cap_type(&self) -> i32 {
        self.end_cap_type()
    }

    pub fn end_captype(&self) -> i32 {
        self.end_cap_type()
    }

    pub fn captype_end(&self) -> i32 {
        self.end_cap_type()
    }

    pub fn set_end_cap_type(&mut self, value: i32) {
        self.end_cap_type = value;
    }

    pub fn set_end_captype(&mut self, value: i32) {
        self.set_end_cap_type(value);
    }

    pub fn set_captype_end(&mut self, value: i32) {
        self.set_end_cap_type(value);
    }

    pub fn arrow_height(&self) -> f64 {
        self.arrow_height
    }

    pub fn get_arrow_height(&self) -> f64 {
        self.arrow_height()
    }

    pub fn arrowheight(&self) -> f64 {
        self.arrow_height()
    }

    pub fn set_arrow_height(&mut self, value: f64) {
        self.arrow_height = value;
    }

    pub fn set_arrowheight(&mut self, value: f64) {
        self.set_arrow_height(value);
    }

    pub fn arrow_width(&self) -> f64 {
        self.arrow_width
    }

    pub fn get_arrow_width(&self) -> f64 {
        self.arrow_width()
    }

    pub fn arrowwidth(&self) -> f64 {
        self.arrow_width()
    }

    pub fn set_arrow_width(&mut self, value: f64) {
        self.arrow_width = value;
    }

    pub fn set_arrowwidth(&mut self, value: f64) {
        self.set_arrow_width(value);
    }

    pub fn font_size(&self) -> f64 {
        self.font_size
    }

    pub fn set_font_size(&mut self, value: f64) {
        self.font_size = value;
    }

    pub fn font_name(&self) -> &str {
        &self.font_name
    }

    pub fn set_font_name(&mut self, value: impl Into<String>) {
        self.font_name = value.into();
    }

    pub fn font_style(&self) -> &str {
        &self.font_style
    }

    pub fn set_font_style(&mut self, value: impl Into<String>) {
        self.font_style = value.into();
    }

    pub fn font_weight(&self) -> &str {
        &self.font_weight
    }

    pub fn set_font_weight(&mut self, value: impl Into<String>) {
        self.font_weight = value.into();
    }

    pub fn defs(&self) -> &[AtomIntrDef] {
        &self.defs
    }

    /// Reference points of a definition: its fixed positions if any, else the
    /// positions of its atoms looked up by id in `mol`.
    fn def_points(&self, def: &AtomIntrDef, mol: &MolCoord) -> Option<Vec<Vector4>> {
        if !def.active {
            return None;
        }
        if !def.positions.is_empty() {
            return Some(def.positions.clone());
        }
        if !def.selections.is_empty() {
            let mut points = Vec::with_capacity(def.selections.len());
            for selection in &def.selections {
                let selector = parse_selector(selection).ok()?;
                let selected = mol
                    .atoms()
                    .iter()
                    .filter(|atom| selector.matches_in_molecule(atom, mol))
                    .collect::<Vec<_>>();
                if selected.is_empty() {
                    return None;
                }
                let count = selected.len() as f64;
                points.push(Vector4::new3(
                    selected.iter().map(|atom| atom.position.x).sum::<f64>() / count,
                    selected.iter().map(|atom| atom.position.y).sum::<f64>() / count,
                    selected.iter().map(|atom| atom.position.z).sum::<f64>() / count,
                ));
            }
            return Some(points);
        }
        let mut points = Vec::with_capacity(def.atom_ids.len());
        for &aid in &def.atom_ids {
            points.push(mol.get_atom_by_id(aid)?.position);
        }
        Some(points)
    }

    /// The connecting line segments for every measurement definition (C++
    /// `AtomIntrRenderer` draws these between the measured atoms): one segment
    /// for a Distance, two for an Angle (the two arms), three for a Torsion.
    /// The arc indicator and the numeric text label are NOT included here — the
    /// label is GPU/font text with no headless oracle; the line geometry is the
    /// verifiable part. Unresolvable definitions are skipped.
    pub fn measurement_lines(&self, mol: &MolCoord) -> Vec<(Vector4, Vector4)> {
        let mut lines = Vec::new();
        for def in &self.defs {
            if let Some(points) = self.def_points(def, mol) {
                for seg in points.windows(2) {
                    lines.push((seg[0], seg[1]));
                }
            }
        }
        lines
    }

    /// Arc polylines for Angle measurements (C++ `AtomIntrRenderer` draws an arc
    /// at the vertex indicating the angle). Each arc sweeps from the first arm to
    /// the second at `radius_frac` of the shorter arm length, in `segments`+1
    /// points. Distance/Torsion defs produce no arc. The numeric label is not
    /// included (GPU/font text, no headless oracle).
    pub fn measurement_arcs(
        &self,
        mol: &MolCoord,
        radius_frac: f64,
        segments: usize,
    ) -> Vec<Vec<Vector4>> {
        let segments = segments.max(1);
        let mut arcs = Vec::new();
        for def in &self.defs {
            if def.kind != AtomIntrDefKind::Angle {
                continue;
            }
            let Some(p) = self.def_points(def, mol) else {
                continue;
            };
            if p.len() < 3 {
                continue;
            }
            let (arm0, arm2) = (p[0] - p[1], p[2] - p[1]);
            let len = arm0.length3d().min(arm2.length3d());
            if len < 1.0e-9 {
                continue;
            }
            let r = len * radius_frac;
            let d0 = normalize_or(arm0, Vector4::new3(1.0, 0.0, 0.0));
            let d2 = normalize_or(arm2, Vector4::new3(0.0, 1.0, 0.0));
            let arc = (0..=segments)
                .map(|k| {
                    let t = k as f64 / segments as f64;
                    let dir = normalize_or(d0.scale(1.0 - t) + d2.scale(t), d0);
                    p[1] + dir.scale(r)
                })
                .collect();
            arcs.push(arc);
        }
        arcs
    }

    /// Measured value of a definition: distance (Å) for a 2-atom Distance, angle
    /// (degrees) for a 3-atom Angle, torsion (degrees) for a 4-atom Torsion.
    /// Port of the value computation in C++ `AtomIntrRenderer::append{,Angle,
    /// Torsion}ById` (`(pos0-pos1).length()`, `toDegree(angle(pos0-pos1,
    /// pos2-pos1))`, `toDegree(Vector4D::torsion(pos0..pos3))`). Returns `None`
    /// when an atom cannot be resolved or the geometry is degenerate (e.g. a
    /// linear angle triple makes the torsion undefined).
    pub fn measure_value(&self, def: &AtomIntrDef, mol: &MolCoord) -> Option<f64> {
        let p = self.def_points(def, mol)?;
        match def.kind {
            AtomIntrDefKind::Distance => {
                if p.len() < 2 {
                    return None;
                }
                Some((p[0] - p[1]).length3d())
            }
            AtomIntrDefKind::Angle => {
                if p.len() < 3 {
                    return None;
                }
                p[0].sub_vec(p[1])
                    .angle(p[2].sub_vec(p[1]))
                    .ok()
                    .map(f64::to_degrees)
            }
            AtomIntrDefKind::Torsion => {
                if p.len() < 4 {
                    return None;
                }
                Vector4::torsion(p[0], p[1], p[2], p[3])
                    .ok()
                    .map(f64::to_degrees)
            }
        }
    }

    pub fn append(&mut self, selection1: impl Into<String>, selection2: impl Into<String>) -> i32 {
        let selection1 = selection1.into();
        let selection2 = selection2.into();
        if parse_selector(&selection1).is_err() || parse_selector(&selection2).is_err() {
            return -1;
        }
        self.push_def(AtomIntrDef {
            id: 0,
            kind: AtomIntrDefKind::Distance,
            selections: vec![selection1, selection2],
            atom_ids: Vec::new(),
            mol_ids: vec![self.client_mol_id; 2],
            positions: Vec::new(),
            show_message: true,
            active: true,
        })
    }

    pub fn append_by_id(&mut self, aid1: i32, mol_id2: i32, aid2: i32, show_message: bool) -> i32 {
        if self.client_mol_id == mol_id2 && aid1 == aid2 {
            return -1;
        }
        self.push_def(AtomIntrDef {
            id: 0,
            kind: AtomIntrDefKind::Distance,
            selections: Vec::new(),
            atom_ids: vec![aid1, aid2],
            mol_ids: vec![self.client_mol_id, mol_id2],
            positions: Vec::new(),
            show_message,
            active: true,
        })
    }

    pub fn append_by_2vecs(&mut self, v1: Vector4, v2: Vector4) -> i32 {
        self.push_def(AtomIntrDef {
            id: 0,
            kind: AtomIntrDefKind::Distance,
            selections: Vec::new(),
            atom_ids: Vec::new(),
            mol_ids: Vec::new(),
            positions: vec![v1, v2],
            show_message: true,
            active: true,
        })
    }

    pub fn append_by2_vecs(&mut self, v1: Vector4, v2: Vector4) -> i32 {
        self.append_by_2vecs(v1, v2)
    }

    pub fn append_by_2_vecs(&mut self, v1: Vector4, v2: Vector4) -> i32 {
        self.append_by_2vecs(v1, v2)
    }

    pub fn append_angle_by_id(
        &mut self,
        aid1: i32,
        mol_id2: i32,
        aid2: i32,
        mol_id3: i32,
        aid3: i32,
    ) -> i32 {
        self.push_def(AtomIntrDef {
            id: 0,
            kind: AtomIntrDefKind::Angle,
            selections: Vec::new(),
            atom_ids: vec![aid1, aid2, aid3],
            mol_ids: vec![self.client_mol_id, mol_id2, mol_id3],
            positions: Vec::new(),
            show_message: true,
            active: true,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_torsion_by_id(
        &mut self,
        aid1: i32,
        mol_id2: i32,
        aid2: i32,
        mol_id3: i32,
        aid3: i32,
        mol_id4: i32,
        aid4: i32,
    ) -> i32 {
        self.push_def(AtomIntrDef {
            id: 0,
            kind: AtomIntrDefKind::Torsion,
            selections: Vec::new(),
            atom_ids: vec![aid1, aid2, aid3, aid4],
            mol_ids: vec![self.client_mol_id, mol_id2, mol_id3, mol_id4],
            positions: Vec::new(),
            show_message: true,
            active: true,
        })
    }

    pub fn remove(&mut self, id: i32) -> bool {
        let Ok(index) = usize::try_from(id) else {
            return false;
        };
        let Some(def) = self.defs.get_mut(index) else {
            return false;
        };
        def.active = false;
        true
    }

    pub fn get_defs_json(&self) -> String {
        self.get_defs_json_impl(|_, atom_id| atom_id.to_string())
    }

    pub fn get_defs_json_with_molecule(&self, molecule: &MolCoord) -> String {
        self.get_defs_json_impl(|molecule_id, atom_id| {
            if molecule_id != self.client_mol_id {
                return atom_id.to_string();
            }
            molecule
                .get_atom_by_id(atom_id)
                .map(Atom::format_msg)
                .unwrap_or_else(|| atom_id.to_string())
        })
    }

    fn get_defs_json_impl(&self, atom_label: impl Fn(i32, i32) -> String) -> String {
        let entries = self
            .defs
            .iter()
            .filter(|def| def.active)
            .map(|def| atom_intr_def_json(def, &atom_label))
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    fn push_def(&mut self, mut def: AtomIntrDef) -> i32 {
        let id = i32::try_from(self.defs.len()).unwrap_or(i32::MAX);
        def.id = id;
        self.defs.push(def);
        id
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisoRenderer {
    detail: i32,
    width: f64,
    dotsep: f64,
    loopsize: f64,
    loopsize2: f64,
    target: String,
}

impl Default for DisoRenderer {
    fn default() -> Self {
        Self {
            detail: 5,
            width: 0.3,
            dotsep: 1.0,
            loopsize: 2.0,
            loopsize2: -1.0,
            target: String::new(),
        }
    }
}

impl DisoRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "disorder"
    }

    pub fn detail(&self) -> i32 {
        self.detail
    }

    pub fn get_detail(&self) -> i32 {
        self.detail()
    }

    pub fn set_detail(&mut self, value: i32) {
        self.detail = value;
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn get_width(&self) -> f64 {
        self.width()
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
    }

    pub fn dotsep(&self) -> f64 {
        self.dotsep
    }

    pub fn get_dot_sep(&self) -> f64 {
        self.dotsep()
    }

    pub fn set_dotsep(&mut self, value: f64) {
        self.dotsep = value;
    }

    pub fn set_dot_sep(&mut self, value: f64) {
        self.set_dotsep(value);
    }

    pub fn loopsize(&self) -> f64 {
        self.loopsize
    }

    pub fn get_strength(&self) -> f64 {
        self.loopsize()
    }

    pub fn set_loopsize(&mut self, value: f64) {
        self.loopsize = value;
    }

    pub fn set_strength(&mut self, value: f64) {
        self.set_loopsize(value);
    }

    pub fn loopsize2(&self) -> f64 {
        self.loopsize2
    }

    pub fn get_strength2(&self) -> f64 {
        self.loopsize2()
    }

    pub fn loopsize_2(&self) -> f64 {
        self.loopsize2()
    }

    pub fn set_loopsize2(&mut self, value: f64) {
        self.loopsize2 = value;
    }

    pub fn set_strength2(&mut self, value: f64) {
        self.set_loopsize2(value);
    }

    pub fn set_loopsize_2(&mut self, value: f64) {
        self.set_loopsize2(value);
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn get_tgt_rend_name(&self) -> &str {
        self.target()
    }

    pub fn set_target(&mut self, value: impl Into<String>) {
        self.target = value.into();
    }

    pub fn set_tgt_rend_name(&mut self, value: impl Into<String>) {
        self.set_target(value);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TubeSection {
    width: f64,
    tuber: f64,
    sharp: f64,
    section_type: i32,
    detail: i32,
    valid: bool,
}

impl Default for TubeSection {
    fn default() -> Self {
        Self {
            width: 0.35,
            tuber: 1.0,
            sharp: 0.4,
            section_type: Self::TS_ELLIPTICAL,
            detail: 16,
            valid: false,
        }
    }
}

impl TubeSection {
    pub const TSSF_UNKNOWN: i8 = 0;
    pub const TSSF_FRONT: i8 = 1;
    pub const TSSF_BACK: i8 = 2;
    pub const TSSF_SIDE1: i8 = 3;
    pub const TSSF_SIDE2: i8 = 4;
    pub const TS_ELLIPTICAL: i32 = 0;
    pub const TS_SQUARE: i32 = 1;
    pub const TS_RECT: i32 = 2;
    pub const TS_MOLSCR: i32 = 3;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    pub fn set_valid(&mut self, value: bool) {
        self.valid = value;
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn get_width(&self) -> f64 {
        self.width()
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
        self.invalidate();
    }

    pub fn tuber(&self) -> f64 {
        self.tuber
    }

    pub fn get_tuber(&self) -> f64 {
        self.tuber()
    }

    pub fn set_tuber(&mut self, value: f64) {
        self.tuber = value;
        self.invalidate();
    }

    pub fn sharp(&self) -> f64 {
        self.sharp
    }

    pub fn get_sharp(&self) -> f64 {
        self.sharp()
    }

    pub fn set_sharp(&mut self, value: f64) {
        self.sharp = value;
        self.invalidate();
    }

    pub fn section_type(&self) -> i32 {
        self.section_type
    }

    pub fn get_type(&self) -> i32 {
        self.section_type()
    }

    pub fn r#type(&self) -> i32 {
        self.section_type()
    }

    pub fn set_section_type(&mut self, value: i32) {
        self.section_type = value;
        self.invalidate();
    }

    pub fn set_type(&mut self, value: i32) {
        self.set_section_type(value);
    }

    pub fn detail(&self) -> i32 {
        self.detail
    }

    pub fn get_detail(&self) -> i32 {
        self.detail()
    }

    pub fn set_detail(&mut self, value: i32) {
        self.detail = value;
        self.invalidate();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JctTable {
    jct_type: i32,
    gamma: f64,
    basw: f64,
    arrow: f64,
    valid: bool,
}

impl Default for JctTable {
    fn default() -> Self {
        Self {
            jct_type: Self::JT_ARROW1,
            gamma: 2.2,
            basw: 0.0,
            arrow: 1.8,
            valid: false,
        }
    }
}

impl JctTable {
    pub const JT_SMOOTH1: i32 = 0;
    pub const JT_ARROW1: i32 = 1;
    pub const JT_FLAT: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    pub fn set_valid(&mut self, value: bool) {
        self.valid = value;
    }

    pub fn jct_type(&self) -> i32 {
        self.jct_type
    }

    pub fn get_type(&self) -> i32 {
        self.jct_type()
    }

    pub fn r#type(&self) -> i32 {
        self.jct_type()
    }

    pub fn set_jct_type(&mut self, value: i32) {
        self.jct_type = value;
        self.invalidate();
    }

    pub fn set_type(&mut self, value: i32) {
        self.set_jct_type(value);
    }

    pub fn gamma(&self) -> f64 {
        self.gamma
    }

    pub fn get_gamma(&self) -> f64 {
        self.gamma()
    }

    pub fn set_gamma(&mut self, value: f64) {
        self.gamma = value;
        self.invalidate();
    }

    pub fn basw(&self) -> f64 {
        self.basw
    }

    pub fn get_basw(&self) -> f64 {
        self.basw()
    }

    pub fn set_basw(&mut self, value: f64) {
        self.basw = value;
        self.invalidate();
    }

    pub fn arrow(&self) -> f64 {
        self.arrow
    }

    pub fn get_arrow(&self) -> f64 {
        self.arrow()
    }

    pub fn set_arrow(&mut self, value: f64) {
        self.arrow = value;
        self.invalidate();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplineRenderer {
    axialdetail: i32,
    smooth: f64,
    smoothcolor: bool,
    line_width: f64,
    start_cap_type: i32,
    end_cap_type: i32,
    segend_fade: bool,
}

impl Default for SplineRenderer {
    fn default() -> Self {
        Self {
            axialdetail: 6,
            smooth: 0.0,
            smoothcolor: true,
            line_width: 1.2,
            start_cap_type: Self::TUBE_CAP_SPHR,
            end_cap_type: Self::TUBE_CAP_SPHR,
            segend_fade: false,
        }
    }
}

impl SplineRenderer {
    pub const TUBE_CAP_SPHR: i32 = 0;
    pub const TUBE_CAP_FLAT: i32 = 1;
    pub const TUBE_CAP_NONE: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "spline"
    }

    pub fn axialdetail(&self) -> i32 {
        self.axialdetail
    }

    pub fn get_axial_detail(&self) -> i32 {
        self.axialdetail()
    }

    pub fn set_axialdetail(&mut self, value: i32) {
        self.axialdetail = value;
    }

    pub fn set_axial_detail(&mut self, value: i32) {
        self.set_axialdetail(value);
    }

    pub fn smooth(&self) -> f64 {
        self.smooth
    }

    pub fn get_smooth(&self) -> f64 {
        self.smooth()
    }

    pub fn set_smooth(&mut self, value: f64) {
        self.smooth = value;
    }

    pub fn smoothcolor(&self) -> bool {
        self.smoothcolor
    }

    pub fn is_smooth_color(&self) -> bool {
        self.smoothcolor()
    }

    pub fn set_smoothcolor(&mut self, value: bool) {
        self.smoothcolor = value;
    }

    pub fn set_smooth_color(&mut self, value: bool) {
        self.set_smoothcolor(value);
    }

    pub fn line_width(&self) -> f64 {
        self.line_width
    }

    pub fn get_line_width(&self) -> f64 {
        self.line_width()
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.line_width = value;
    }

    pub fn start_cap_type(&self) -> i32 {
        self.start_cap_type
    }

    pub fn get_start_cap_type(&self) -> i32 {
        self.start_cap_type()
    }

    pub fn start_captype(&self) -> i32 {
        self.start_cap_type()
    }

    pub fn captype_start(&self) -> i32 {
        self.start_cap_type()
    }

    pub fn set_start_cap_type(&mut self, value: i32) {
        self.start_cap_type = value;
    }

    pub fn set_start_captype(&mut self, value: i32) {
        self.set_start_cap_type(value);
    }

    pub fn set_captype_start(&mut self, value: i32) {
        self.set_start_cap_type(value);
    }

    pub fn end_cap_type(&self) -> i32 {
        self.end_cap_type
    }

    pub fn get_end_cap_type(&self) -> i32 {
        self.end_cap_type()
    }

    pub fn end_captype(&self) -> i32 {
        self.end_cap_type()
    }

    pub fn captype_end(&self) -> i32 {
        self.end_cap_type()
    }

    pub fn set_end_cap_type(&mut self, value: i32) {
        self.end_cap_type = value;
    }

    pub fn set_end_captype(&mut self, value: i32) {
        self.set_end_cap_type(value);
    }

    pub fn set_captype_end(&mut self, value: i32) {
        self.set_end_cap_type(value);
    }

    pub fn segend_fade(&self) -> bool {
        self.segend_fade
    }

    pub fn is_seg_end_fade(&self) -> bool {
        self.segend_fade()
    }

    pub fn set_segend_fade(&mut self, value: bool) {
        self.segend_fade = value;
    }

    pub fn set_seg_end_fade(&mut self, value: bool) {
        self.set_segend_fade(value);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TubeRenderer {
    spline: SplineRenderer,
    section: TubeSection,
    putty_mode: i32,
    putty_tgt: i32,
    putty_hiscl: f64,
    putty_loscl: f64,
}

impl Default for TubeRenderer {
    fn default() -> Self {
        Self {
            spline: SplineRenderer::default(),
            section: TubeSection::default(),
            putty_mode: Self::TBR_PUTTY_OFF,
            putty_tgt: Self::TBR_PUTTY_BFAC,
            putty_hiscl: 3.0,
            putty_loscl: 3.0,
        }
    }
}

impl TubeRenderer {
    pub const TBR_PUTTY_OFF: i32 = 0;
    pub const TBR_PUTTY_SCALE1: i32 = 1;
    pub const TBR_PUTTY_LINEAR1: i32 = 2;
    pub const TBR_PUTTY_BFAC: i32 = 1;
    pub const TBR_PUTTY_OCC: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "tube"
    }

    pub fn spline(&self) -> &SplineRenderer {
        &self.spline
    }

    pub fn spline_mut(&mut self) -> &mut SplineRenderer {
        &mut self.spline
    }

    pub fn section(&self) -> &TubeSection {
        &self.section
    }

    pub fn get_tube_section(&self) -> &TubeSection {
        self.section()
    }

    pub fn section_mut(&mut self) -> &mut TubeSection {
        &mut self.section
    }

    pub fn putty_mode(&self) -> i32 {
        self.putty_mode
    }

    pub fn get_putty_mode(&self) -> i32 {
        self.putty_mode()
    }

    pub fn set_putty_mode(&mut self, value: i32) {
        self.putty_mode = value;
    }

    pub fn putty_tgt(&self) -> i32 {
        self.putty_tgt
    }

    pub fn get_putty_tgt(&self) -> i32 {
        self.putty_tgt()
    }

    pub fn set_putty_tgt(&mut self, value: i32) {
        self.putty_tgt = value;
    }

    pub fn putty_hiscl(&self) -> f64 {
        self.putty_hiscl
    }

    pub fn get_putty_hi_scl(&self) -> f64 {
        self.putty_hiscl()
    }

    pub fn set_putty_hiscl(&mut self, value: f64) {
        self.putty_hiscl = value;
    }

    pub fn set_putty_hi_scl(&mut self, value: f64) {
        self.set_putty_hiscl(value);
    }

    pub fn putty_loscl(&self) -> f64 {
        self.putty_loscl
    }

    pub fn get_putty_lo_scl(&self) -> f64 {
        self.putty_loscl()
    }

    pub fn set_putty_loscl(&mut self, value: f64) {
        self.putty_loscl = value;
    }

    pub fn set_putty_lo_scl(&mut self, value: f64) {
        self.set_putty_loscl(value);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NARenderer {
    tube: TubeRenderer,
    show_tube: bool,
    show_basepair: bool,
    base_size: f64,
    base_detail: i32,
    base_thick: f64,
    base_type: i32,
}

impl Default for NARenderer {
    fn default() -> Self {
        Self {
            tube: TubeRenderer::default(),
            show_tube: true,
            show_basepair: true,
            base_size: 0.5,
            base_detail: 3,
            base_thick: 0.5,
            base_type: Self::NAREND_BP,
        }
    }
}

impl NARenderer {
    pub const NAREND_BP: i32 = 0;
    pub const NAREND_SIMPLE1: i32 = 1;
    pub const NAREND_DETAIL1: i32 = 2;
    pub const NAREND_DETAIL2: i32 = 3;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "nucl"
    }

    pub fn tube(&self) -> &TubeRenderer {
        &self.tube
    }

    pub fn tube_mut(&mut self) -> &mut TubeRenderer {
        &mut self.tube
    }

    pub fn show_tube(&self) -> bool {
        self.show_tube
    }

    pub fn get_rend_tube(&self) -> bool {
        self.show_tube()
    }

    pub fn set_show_tube(&mut self, value: bool) {
        self.show_tube = value;
    }

    pub fn set_rend_tube(&mut self, value: bool) {
        self.set_show_tube(value);
    }

    pub fn show_basepair(&self) -> bool {
        self.show_basepair
    }

    pub fn is_show_bp(&self) -> bool {
        self.show_basepair()
    }

    pub fn set_show_basepair(&mut self, value: bool) {
        self.show_basepair = value;
    }

    pub fn set_show_bp(&mut self, value: bool) {
        self.set_show_basepair(value);
    }

    pub fn base_size(&self) -> f64 {
        self.base_size
    }

    pub fn get_base_size(&self) -> f64 {
        self.base_size()
    }

    pub fn set_base_size(&mut self, value: f64) {
        self.base_size = value;
    }

    pub fn base_detail(&self) -> i32 {
        self.base_detail
    }

    pub fn get_base_detail(&self) -> i32 {
        self.base_detail()
    }

    pub fn set_base_detail(&mut self, value: i32) {
        self.base_detail = value;
    }

    pub fn base_thick(&self) -> f64 {
        self.base_thick
    }

    pub fn get_base_thick(&self) -> f64 {
        self.base_thick()
    }

    pub fn set_base_thick(&mut self, value: f64) {
        self.base_thick = value;
    }

    pub fn base_type(&self) -> i32 {
        self.base_type
    }

    pub fn get_base_type(&self) -> i32 {
        self.base_type()
    }

    pub fn set_base_type(&mut self, value: i32) {
        self.base_type = value;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RibbonRenderer {
    spline: SplineRenderer,
    helix: TubeSection,
    sheet: TubeSection,
    coil: TubeSection,
    helixhead: JctTable,
    helixtail: JctTable,
    sheethead: JctTable,
    sheettail: JctTable,
    helix_smooth: f64,
    sheet_smooth: f64,
    coil_smooth: f64,
    helix_usebackcol: bool,
    helix_backcol: SolidColor,
    sheet_usesidecol: bool,
    sheet_sidecol: SolidColor,
}

impl Default for RibbonRenderer {
    fn default() -> Self {
        Self {
            spline: SplineRenderer::default(),
            helix: TubeSection::default(),
            sheet: TubeSection::default(),
            coil: TubeSection::default(),
            helixhead: JctTable::default(),
            helixtail: JctTable::default(),
            sheethead: JctTable::default(),
            sheettail: JctTable::default(),
            helix_smooth: 0.0,
            sheet_smooth: 0.5,
            coil_smooth: 0.0,
            helix_usebackcol: false,
            helix_backcol: SolidColor::from_rgb(1.0, 1.0, 1.0),
            sheet_usesidecol: false,
            sheet_sidecol: SolidColor::from_rgb(1.0, 1.0, 1.0),
        }
    }
}

impl RibbonRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "ribbon"
    }

    pub fn spline(&self) -> &SplineRenderer {
        &self.spline
    }

    pub fn spline_mut(&mut self) -> &mut SplineRenderer {
        &mut self.spline
    }

    pub fn helix(&self) -> &TubeSection {
        &self.helix
    }

    pub fn get_helix_section(&self) -> &TubeSection {
        self.helix()
    }

    pub fn helix_mut(&mut self) -> &mut TubeSection {
        &mut self.helix
    }

    pub fn sheet(&self) -> &TubeSection {
        &self.sheet
    }

    pub fn get_sheet_section(&self) -> &TubeSection {
        self.sheet()
    }

    pub fn sheet_mut(&mut self) -> &mut TubeSection {
        &mut self.sheet
    }

    pub fn coil(&self) -> &TubeSection {
        &self.coil
    }

    pub fn get_coil_section(&self) -> &TubeSection {
        self.coil()
    }

    pub fn coil_mut(&mut self) -> &mut TubeSection {
        &mut self.coil
    }

    pub fn helixhead(&self) -> &JctTable {
        &self.helixhead
    }

    pub fn get_helix_head(&self) -> &JctTable {
        self.helixhead()
    }

    pub fn helixhead_mut(&mut self) -> &mut JctTable {
        &mut self.helixhead
    }

    pub fn helixtail(&self) -> &JctTable {
        &self.helixtail
    }

    pub fn get_helix_tail(&self) -> &JctTable {
        self.helixtail()
    }

    pub fn helixtail_mut(&mut self) -> &mut JctTable {
        &mut self.helixtail
    }

    pub fn sheethead(&self) -> &JctTable {
        &self.sheethead
    }

    pub fn get_sheet_head(&self) -> &JctTable {
        self.sheethead()
    }

    pub fn sheethead_mut(&mut self) -> &mut JctTable {
        &mut self.sheethead
    }

    pub fn sheettail(&self) -> &JctTable {
        &self.sheettail
    }

    pub fn get_sheet_tail(&self) -> &JctTable {
        self.sheettail()
    }

    pub fn sheettail_mut(&mut self) -> &mut JctTable {
        &mut self.sheettail
    }

    pub fn helix_smooth(&self) -> f64 {
        self.helix_smooth
    }

    pub fn get_helix_smooth(&self) -> f64 {
        self.helix_smooth()
    }

    pub fn set_helix_smooth(&mut self, value: f64) {
        self.helix_smooth = value;
    }

    pub fn sheet_smooth(&self) -> f64 {
        self.sheet_smooth
    }

    pub fn get_sheet_smooth(&self) -> f64 {
        self.sheet_smooth()
    }

    pub fn set_sheet_smooth(&mut self, value: f64) {
        self.sheet_smooth = value;
    }

    pub fn coil_smooth(&self) -> f64 {
        self.coil_smooth
    }

    pub fn get_coil_smooth(&self) -> f64 {
        self.coil_smooth()
    }

    pub fn set_coil_smooth(&mut self, value: f64) {
        self.coil_smooth = value;
    }

    pub fn helix_usebackcol(&self) -> bool {
        self.helix_usebackcol
    }

    pub fn is_use_hb_col(&self) -> bool {
        self.helix_usebackcol()
    }

    pub fn set_helix_usebackcol(&mut self, value: bool) {
        self.helix_usebackcol = value;
    }

    pub fn set_use_hb_col(&mut self, value: bool) {
        self.set_helix_usebackcol(value);
    }

    pub fn helix_backcol(&self) -> &SolidColor {
        &self.helix_backcol
    }

    pub fn get_helix_back_col(&self) -> &SolidColor {
        self.helix_backcol()
    }

    pub fn set_helix_backcol(&mut self, value: SolidColor) {
        self.helix_backcol = value;
    }

    pub fn set_helix_back_col(&mut self, value: SolidColor) {
        self.set_helix_backcol(value);
    }

    pub fn sheet_usesidecol(&self) -> bool {
        self.sheet_usesidecol
    }

    pub fn is_use_ss_col(&self) -> bool {
        self.sheet_usesidecol()
    }

    pub fn set_sheet_usesidecol(&mut self, value: bool) {
        self.sheet_usesidecol = value;
    }

    pub fn set_use_ss_col(&mut self, value: bool) {
        self.set_sheet_usesidecol(value);
    }

    pub fn sheet_sidecol(&self) -> &SolidColor {
        &self.sheet_sidecol
    }

    pub fn get_sheet_side_col(&self) -> &SolidColor {
        self.sheet_sidecol()
    }

    pub fn set_sheet_sidecol(&mut self, value: SolidColor) {
        self.sheet_sidecol = value;
    }

    pub fn set_sheet_side_col(&mut self, value: SolidColor) {
        self.set_sheet_sidecol(value);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ribbon2Renderer {
    axialdetail: i32,
    smoothcolor: bool,
    anchor_selection: String,
    anchor_weight: f64,
    helix_ribbon: bool,
    helix_smooth: f64,
    helix_width_mode: i32,
    helix_width: f64,
    helix_extend: f64,
    helix_wsmooth: f64,
    helix_wplus: f64,
    sheet_smooth: f64,
    sheet_wsmooth: f64,
    coil_smooth: f64,
    helix: TubeSection,
    sheet: TubeSection,
    coil: TubeSection,
    sheethead: JctTable,
    ribhelix: TubeSection,
    ribhelix_head: JctTable,
    ribhelix_tail: JctTable,
    start_cap_type: i32,
    end_cap_type: i32,
    segend_fade: bool,
    dump_curvature: bool,
}

impl Default for Ribbon2Renderer {
    fn default() -> Self {
        Self {
            axialdetail: 8,
            smoothcolor: false,
            anchor_selection: String::new(),
            anchor_weight: 10.0,
            helix_ribbon: false,
            helix_smooth: 3.0,
            helix_width_mode: Self::HWIDTH_AVER,
            helix_width: 2.3,
            helix_extend: 0.5,
            helix_wsmooth: 5.0,
            helix_wplus: 0.2,
            sheet_smooth: 2.0,
            sheet_wsmooth: 5.0,
            coil_smooth: -1.0,
            helix: TubeSection::default(),
            sheet: TubeSection::default(),
            coil: TubeSection::default(),
            sheethead: JctTable::default(),
            ribhelix: TubeSection::default(),
            ribhelix_head: JctTable::default(),
            ribhelix_tail: JctTable::default(),
            start_cap_type: Self::TUBE_CAP_FLAT,
            end_cap_type: Self::TUBE_CAP_FLAT,
            segend_fade: false,
            dump_curvature: false,
        }
    }
}

impl Ribbon2Renderer {
    pub const HWIDTH_CONST: i32 = 0;
    pub const HWIDTH_AVER: i32 = 1;
    pub const HWIDTH_WAVY: i32 = 2;
    pub const TUBE_CAP_SPHR: i32 = 0;
    pub const TUBE_CAP_FLAT: i32 = 1;
    pub const TUBE_CAP_NONE: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "cartoon"
    }

    pub fn axialdetail(&self) -> i32 {
        self.axialdetail
    }

    pub fn get_axial_detail(&self) -> i32 {
        self.axialdetail()
    }

    pub fn set_axialdetail(&mut self, value: i32) {
        self.axialdetail = value;
    }

    pub fn set_axial_detail(&mut self, value: i32) {
        self.set_axialdetail(value);
    }

    pub fn smoothcolor(&self) -> bool {
        self.smoothcolor
    }

    pub fn is_smooth_color(&self) -> bool {
        self.smoothcolor()
    }

    pub fn set_smoothcolor(&mut self, value: bool) {
        self.smoothcolor = value;
    }

    pub fn set_smooth_color(&mut self, value: bool) {
        self.set_smoothcolor(value);
    }

    pub fn anchor_selection(&self) -> &str {
        &self.anchor_selection
    }

    pub fn anchor_sel(&self) -> &str {
        self.anchor_selection()
    }

    pub fn get_anchor_sel(&self) -> &str {
        self.anchor_sel()
    }

    pub fn set_anchor_selection(&mut self, value: impl Into<String>) {
        self.anchor_selection = value.into();
    }

    pub fn set_anchor_sel(&mut self, value: impl Into<String>) {
        self.set_anchor_selection(value);
    }

    pub fn anchor_weight(&self) -> f64 {
        self.anchor_weight
    }

    pub fn get_anchor_wgt(&self) -> f64 {
        self.anchor_weight()
    }

    pub fn set_anchor_weight(&mut self, value: f64) {
        self.anchor_weight = value;
    }

    pub fn set_anchor_wgt(&mut self, value: f64) {
        self.set_anchor_weight(value);
    }

    pub fn helix_ribbon(&self) -> bool {
        self.helix_ribbon
    }

    pub fn set_helix_ribbon(&mut self, value: bool) {
        self.helix_ribbon = value;
    }

    pub fn helix_smooth(&self) -> f64 {
        self.helix_smooth
    }

    pub fn set_helix_smooth(&mut self, value: f64) {
        self.helix_smooth = value;
    }

    pub fn helix_width_mode(&self) -> i32 {
        self.helix_width_mode
    }

    pub fn get_helix_width_mode(&self) -> i32 {
        self.helix_width_mode()
    }

    pub fn set_helix_width_mode(&mut self, value: i32) {
        self.helix_width_mode = value;
    }

    pub fn is_width_aver(&self) -> bool {
        self.helix_width_mode == Self::HWIDTH_AVER
    }

    pub fn helix_waver(&self) -> bool {
        self.is_width_aver()
    }

    pub fn set_width_aver(&mut self, _value: bool) {
        self.helix_width_mode = Self::HWIDTH_AVER;
    }

    pub fn set_helix_waver(&mut self, value: bool) {
        if value {
            self.set_width_aver(true);
        }
    }

    pub fn helix_width(&self) -> f64 {
        self.helix_width
    }

    pub fn set_helix_width(&mut self, value: f64) {
        self.helix_width = value;
    }

    pub fn helix_extend(&self) -> f64 {
        self.helix_extend
    }

    pub fn set_helix_extend(&mut self, value: f64) {
        self.helix_extend = value;
    }

    pub fn helix_wsmooth(&self) -> f64 {
        self.helix_wsmooth
    }

    pub fn set_helix_wsmooth(&mut self, value: f64) {
        self.helix_wsmooth = value;
    }

    pub fn helix_wplus(&self) -> f64 {
        self.helix_wplus
    }

    pub fn set_helix_wplus(&mut self, value: f64) {
        self.helix_wplus = value;
    }

    pub fn sheet_smooth(&self) -> f64 {
        self.sheet_smooth
    }

    pub fn set_sheet_smooth(&mut self, value: f64) {
        self.sheet_smooth = value;
    }

    pub fn sheet_wsmooth(&self) -> f64 {
        self.sheet_wsmooth
    }

    pub fn set_sheet_wsmooth(&mut self, value: f64) {
        self.sheet_wsmooth = value;
    }

    pub fn coil_smooth(&self) -> f64 {
        self.coil_smooth
    }

    pub fn set_coil_smooth(&mut self, value: f64) {
        self.coil_smooth = value;
    }

    pub fn helix(&self) -> &TubeSection {
        &self.helix
    }

    pub fn get_helix_section(&self) -> &TubeSection {
        self.helix()
    }

    pub fn helix_mut(&mut self) -> &mut TubeSection {
        &mut self.helix
    }

    pub fn sheet(&self) -> &TubeSection {
        &self.sheet
    }

    pub fn get_sheet_section(&self) -> &TubeSection {
        self.sheet()
    }

    pub fn sheet_mut(&mut self) -> &mut TubeSection {
        &mut self.sheet
    }

    pub fn coil(&self) -> &TubeSection {
        &self.coil
    }

    pub fn get_coil_section(&self) -> &TubeSection {
        self.coil()
    }

    pub fn coil_mut(&mut self) -> &mut TubeSection {
        &mut self.coil
    }

    pub fn sheethead(&self) -> &JctTable {
        &self.sheethead
    }

    pub fn get_sheet_head(&self) -> &JctTable {
        self.sheethead()
    }

    pub fn sheethead_mut(&mut self) -> &mut JctTable {
        &mut self.sheethead
    }

    pub fn ribhelix(&self) -> &TubeSection {
        &self.ribhelix
    }

    pub fn get_rib_helix_section(&self) -> &TubeSection {
        self.ribhelix()
    }

    pub fn ribhelix_mut(&mut self) -> &mut TubeSection {
        &mut self.ribhelix
    }

    pub fn ribhelix_head(&self) -> &JctTable {
        &self.ribhelix_head
    }

    pub fn get_rib_helix_head(&self) -> &JctTable {
        self.ribhelix_head()
    }

    pub fn ribhelix_head_mut(&mut self) -> &mut JctTable {
        &mut self.ribhelix_head
    }

    pub fn ribhelix_tail(&self) -> &JctTable {
        &self.ribhelix_tail
    }

    pub fn get_rib_helix_tail(&self) -> &JctTable {
        self.ribhelix_tail()
    }

    pub fn ribhelix_tail_mut(&mut self) -> &mut JctTable {
        &mut self.ribhelix_tail
    }

    pub fn start_cap_type(&self) -> i32 {
        self.start_cap_type
    }

    pub fn get_start_cap_type(&self) -> i32 {
        self.start_cap_type()
    }

    pub fn start_captype(&self) -> i32 {
        self.start_cap_type()
    }

    pub fn captype_start(&self) -> i32 {
        self.start_cap_type()
    }

    pub fn set_start_cap_type(&mut self, value: i32) {
        self.start_cap_type = value;
    }

    pub fn set_start_captype(&mut self, value: i32) {
        self.set_start_cap_type(value);
    }

    pub fn set_captype_start(&mut self, value: i32) {
        self.set_start_cap_type(value);
    }

    pub fn end_cap_type(&self) -> i32 {
        self.end_cap_type
    }

    pub fn get_end_cap_type(&self) -> i32 {
        self.end_cap_type()
    }

    pub fn end_captype(&self) -> i32 {
        self.end_cap_type()
    }

    pub fn captype_end(&self) -> i32 {
        self.end_cap_type()
    }

    pub fn set_end_cap_type(&mut self, value: i32) {
        self.end_cap_type = value;
    }

    pub fn set_end_captype(&mut self, value: i32) {
        self.set_end_cap_type(value);
    }

    pub fn set_captype_end(&mut self, value: i32) {
        self.set_end_cap_type(value);
    }

    pub fn segend_fade(&self) -> bool {
        self.segend_fade
    }

    pub fn is_seg_end_fade(&self) -> bool {
        self.segend_fade()
    }

    pub fn set_segend_fade(&mut self, value: bool) {
        self.segend_fade = value;
    }

    pub fn set_seg_end_fade(&mut self, value: bool) {
        self.set_segend_fade(value);
    }

    pub fn dump_curvature(&self) -> bool {
        self.dump_curvature
    }

    pub fn set_dump_curvature(&mut self, value: bool) {
        self.dump_curvature = value;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RainbowColoring {
    start_hue: f64,
    end_hue: f64,
    sat: f64,
    bri: f64,
    mode: i32,
    incr_mode: i32,
}

impl Default for RainbowColoring {
    fn default() -> Self {
        Self {
            start_hue: 240.0,
            end_hue: 0.0,
            sat: 1.0,
            bri: 1.0,
            mode: Self::RBC_MOL,
            incr_mode: Self::RBC_INCR_RESID,
        }
    }
}

impl RainbowColoring {
    pub const RBC_MOL: i32 = 0;
    pub const RBC_CHAIN: i32 = 1;
    pub const RBC_INCR_RESID: i32 = 0;
    pub const RBC_INCR_CHAIN: i32 = 1;
    pub const RBC_INCR_SSHELIX: i32 = 2;
    pub const RBC_INCR_SSSHEET: i32 = 3;
    pub const RBC_INCR_SSHELIXSHEET: i32 = 4;
    pub const RBC_INCR_PROTSS: i32 = 5;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "rainbow"
    }

    pub fn start_hue(&self) -> f64 {
        self.start_hue
    }

    pub fn get_start_hue(&self) -> f64 {
        self.start_hue()
    }

    pub fn set_start_hue(&mut self, value: f64) {
        self.start_hue = value;
    }

    pub fn end_hue(&self) -> f64 {
        self.end_hue
    }

    pub fn get_end_hue(&self) -> f64 {
        self.end_hue()
    }

    pub fn set_end_hue(&mut self, value: f64) {
        self.end_hue = value;
    }

    pub fn sat(&self) -> f64 {
        self.sat
    }

    pub fn set_sat(&mut self, value: f64) {
        self.sat = value;
    }

    pub fn bri(&self) -> f64 {
        self.bri
    }

    pub fn set_bri(&mut self, value: f64) {
        self.bri = value;
    }

    pub fn mode(&self) -> i32 {
        self.mode
    }

    pub fn get_mode(&self) -> i32 {
        self.mode()
    }

    pub fn set_mode(&mut self, value: i32) {
        self.mode = value;
    }

    pub fn incr_mode(&self) -> i32 {
        self.incr_mode
    }

    pub fn get_incr_mode(&self) -> i32 {
        self.incr_mode()
    }

    pub fn set_incr_mode(&mut self, value: i32) {
        self.incr_mode = value;
    }

    /// RGBA for the `index`-th of `total` coloured units (residues), the C++
    /// `RainbowColoring::makeChainMap` gradient: `hue = start + index·(end−start)/
    /// total`, then `createHSB(hue/360, sat, bri)`. Spans start_hue (default 240 =
    /// blue) → end_hue (default 0 = red) across the chain.
    pub fn color_for_index(&self, index: usize, total: usize) -> [f32; 4] {
        let del_hue = if total == 0 {
            0.0
        } else {
            (self.end_hue - self.start_hue) / total as f64
        };
        let hue = self.start_hue + index as f64 * del_hue;
        self.color_for_hue(hue)
    }

    fn color_for_hue(&self, hue: f64) -> [f32; 4] {
        let color =
            SolidColor::create_hsb((hue / 360.0).rem_euclid(1.0), self.sat, self.bri, 1.0).rgba8();
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ]
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScriptColoring {
    script: String,
}

impl ScriptColoring {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "script"
    }

    pub fn script(&self) -> &str {
        &self.script
    }

    pub fn get_script(&self) -> &str {
        self.script()
    }

    pub fn set_script(&mut self, value: impl Into<String>) {
        self.script = value.into();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintEntry {
    selection: String,
    color: SolidColor,
}

impl PaintEntry {
    pub fn new(selection: impl Into<String>, color: SolidColor) -> Self {
        Self {
            selection: selection.into(),
            color,
        }
    }

    pub fn selection(&self) -> &str {
        &self.selection
    }

    pub fn color(&self) -> &SolidColor {
        &self.color
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PaintColoring {
    entries: Vec<PaintEntry>,
}

impl PaintColoring {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "paint"
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get_size(&self) -> i32 {
        i32::try_from(self.len()).unwrap_or(i32::MAX)
    }

    pub fn size(&self) -> i32 {
        self.get_size()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[PaintEntry] {
        &self.entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn append(&mut self, selection: impl Into<String>, color: SolidColor) {
        self.entries.push(PaintEntry::new(selection, color));
    }

    pub fn insert_before(&mut self, index: isize, selection: impl Into<String>, color: SolidColor) {
        let entry = PaintEntry::new(selection, color);
        if index < 0 || index as usize >= self.entries.len() {
            self.entries.push(entry);
        } else {
            self.entries.insert(index as usize, entry);
        }
    }

    pub fn get_color_at(&self, index: isize) -> Option<&SolidColor> {
        self.entry_at(index).map(PaintEntry::color)
    }

    pub fn get_selection_at(&self, index: isize) -> Option<&str> {
        self.entry_at(index).map(PaintEntry::selection)
    }

    pub fn get_sel_at(&self, index: isize) -> Option<&str> {
        self.get_selection_at(index)
    }

    pub fn remove_at(&mut self, index: isize) -> bool {
        if self.index_is_valid(index) {
            self.entries.remove(index as usize);
            true
        } else {
            false
        }
    }

    pub fn change_at(
        &mut self,
        index: isize,
        selection: impl Into<String>,
        color: SolidColor,
    ) -> bool {
        if self.index_is_valid(index) {
            self.entries[index as usize] = PaintEntry::new(selection, color);
            true
        } else {
            false
        }
    }

    fn entry_at(&self, index: isize) -> Option<&PaintEntry> {
        if self.index_is_valid(index) {
            self.entries.get(index as usize)
        } else {
            None
        }
    }

    fn index_is_valid(&self, index: isize) -> bool {
        index >= 0 && (index as usize) < self.entries.len()
    }
}

fn atom_intr_def_json(def: &AtomIntrDef, atom_label: &impl Fn(i32, i32) -> String) -> String {
    let labels = (0..def.kind.point_count())
        .map(|index| {
            let label = if let Some(selection) = def.selections.get(index) {
                selection.clone()
            } else if let Some(position) = def.positions.get(index) {
                format!("({:.2},{:.2},{:.2})", position.x, position.y, position.z)
            } else {
                let atom_id = def.atom_ids.get(index).copied().unwrap_or(-1);
                let molecule_id = def.mol_ids.get(index).copied().unwrap_or_default();
                atom_label(molecule_id, atom_id)
            };
            format!("\"a{index}\":\"{}\"", json_escape(&label))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":{},\"mode\":{},{labels}}}",
        def.id,
        def.kind.mode()
    )
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn simple_atom_primitives(
    molecule: &MolCoord,
    radius: f32,
    color: Option<ColorSpec>,
) -> Vec<RenderAtom> {
    molecule
        .atoms()
        .iter()
        .map(|atom| RenderAtom {
            center: Vector4::new3(atom.position.x, atom.position.y, atom.position.z),
            radius,
            color: color.clone(),
            object_uid: None,
            atom_index: None,
            selected: false,
            detail: None,
            edge: None,
            hit: None,
        })
        .collect()
}

/// JavaScript source and optional renderer metadata for `ScriptColoring`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptColoringOptions {
    pub script: String,
    pub renderer_name: Option<String>,
    pub renderer_type: Option<String>,
    /// Renderer-side schemes color every atom. Object-side schemes are reached
    /// through the CPK `$molcol` cascade and therefore color carbon atoms.
    pub all_atoms: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoleculeRenderOptions {
    pub object_uid: Option<u64>,
    pub atom_radius: f32,
    pub bond_radius: f32,
    pub vdw_scale: f32,
    /// Absolute per-element radii used by CPKRenderer. `None` uses the shared
    /// VDW table multiplied by `vdw_scale` for other renderer types.
    pub element_radii: Option<VdwRadii>,
    pub draw_atoms: bool,
    pub draw_bonds: bool,
    pub color_mode: MoleculeColorMode,
    pub uniform_color: Option<ColorSpec>,
    pub cpk_colors: std::collections::BTreeMap<String, ColorSpec>,
    pub selected_indices: Vec<usize>,
    /// Backbone-trace "putty" width modulation (TubeRenderer): off by default.
    pub putty: PuttyOptions,
    /// Explicit backbone-trace tube radius (SplineRenderer `line_width`/2), or
    /// `None` to derive it from `bond_radius`.
    pub mainchain_radius: Option<f32>,
    /// Per-secondary-structure ribbon cross-section shape (RibbonRenderer
    /// `helix`/`sheet`/`coil` TubeSection width/thickness).
    pub ribbon_sections: RibbonSections,
    /// Ribbon2 anchor weight (0..1): how strongly each backbone control point is
    /// smoothed toward the midpoint of its neighbours before splining (C++
    /// `Ribbon2Renderer` anchor weight / `SplineCoeffSet` smoothing). 0 = pass
    /// through the CA atoms; 1 = fully neighbour-averaged.
    pub anchor_weight: f64,
    /// Absolute atom sphere radius (Å), overriding the element-vdW×`vdw_scale`
    /// model when set — the ball-and-stick "small uniform ball" mode (C++
    /// `BallStickRenderer` `sphr`). `None` keeps the vdW-scaled radius.
    pub atom_radius_abs: Option<f32>,
    /// Atom-sphere UV tessellation level (C++ CPK/BallStick `detail`, ≈3 coarse
    /// .. 10 fine; 4 == the historical default). `None` keeps the default. Fed to
    /// [`cuemol_render::tessellate_snapshot_with_detail`] at mesh-build time.
    pub sphere_detail: Option<u32>,
    /// Ribbon/cartoon spline subdivisions per residue along the backbone (C++
    /// Ribbon/Spline/Tube `axialdetail`; 8 == the default). Higher = a smoother,
    /// denser backbone mesh. `None` keeps the default subdivision.
    pub ribbon_axial_detail: Option<u32>,
    /// Number of vertices around a TubeRenderer cross-section (`section.detail`).
    pub ribbon_radial_detail: Option<u32>,
    /// TubeRenderer section corner sharpness for roundsquare/fancy profiles.
    pub ribbon_section_sharp: f64,
    /// Absolute ribbon cross-section dims per SS class, resolved from a style's
    /// `<helix>/<sheet>/<coil>` `width`/`tuber` (C++ `TubeSection`). Overrides the
    /// factor-based [`MoleculeRenderOptions::ribbon_sections`] where present.
    pub styled_ribbon: StyledRibbonSections,
    /// Per-secondary-structure spline smoothing used by Ribbon/Cartoon.
    pub ribbon_smoothing: RibbonSectionValues,
    /// Ribbon head/tail transition tables (C++ `JctTable`).
    pub helix_head: RibbonJunction,
    pub helix_tail: RibbonJunction,
    pub sheet_head: RibbonJunction,
    pub sheet_tail: RibbonJunction,
    /// Optional face colors for the helix back and sheet side surfaces.
    pub helix_back_color: Option<[f32; 4]>,
    pub sheet_side_color: Option<[f32; 4]>,
    /// Cartoon anchor selection and its SplineCoeff point weight.
    pub cartoon_anchor_selection: Option<String>,
    pub cartoon_anchor_weight: f64,
    /// Cartoon cylinder/ribbon helix mode and cylinder width parameters.
    pub cartoon_helix_ribbon: bool,
    pub cartoon_helix_width: f64,
    pub cartoon_helix_width_plus: f64,
    /// Custom MainChainRenderer pivot atom name. `None` uses CA/P/C1'/O5'.
    pub pivot_atom: Option<String>,
    /// Interpolate renderer colors along each spline interval.
    pub smooth_color: bool,
    /// Tube end-cap geometry.
    pub start_cap_type: TubeCapType,
    pub end_cap_type: TubeCapType,
    /// Fade the first and last tube rings instead of capping a segment end.
    pub segment_end_fade: bool,
    /// Use one tube section for every secondary-structure class and suppress
    /// ribbon sheet arrowheads.
    pub uniform_backbone: bool,
    /// Color the backbone from its pivot atoms when no explicit scheme exists.
    pub backbone_atom_colors: bool,
    /// Atom indices this renderer is restricted to drawing (a renderer's C++
    /// `sel="..."` selection). `None` draws the whole molecule; `Some` draws only
    /// these atoms (and bonds whose both ends are included), keeping each atom's
    /// original index for picking. Honoured by [`molecule_render_snapshot`].
    pub draw_indices: Option<Vec<usize>>,
    /// Object-level `<coloring type="PaintColoring">` entries `(selection, color)`,
    /// first-match-wins. When non-empty, the ribbon backbone is coloured by paint
    /// (the C++ `$molcol`→object-coloring cascade — the backbone CA is carbon, so
    /// CPKColoring defers it to the object's paint). Empty = the default colours.
    pub object_paint: Vec<(String, String)>,
    /// Object `<coloring type="RainbowColoring">`: colours the backbone by a hue
    /// gradient over the chain residues (C++ RainbowColoring). Takes precedence
    /// over `object_paint` (it is the object's coloring scheme). `None` = no rainbow.
    pub rainbow: Option<RainbowColoring>,
    /// Object `<coloring type="BfacColoring">`: colours the backbone by B-factor/
    /// occupancy lerped low_color→high_color (C++ BfacColoring, auto-scaled to the
    /// molecule range by default). Another object coloring scheme. `None` = off.
    pub bfac: Option<BfacColoring>,
    /// Renderer-level integer atom property interpreted as a packed color code.
    /// Used by `AtomPropColoring` when importing PyMOL sessions.
    pub atom_color_property: Option<String>,
    /// JavaScript-backed atom coloring. The engine is created lazily only when a
    /// loaded scene actually uses `ScriptColoring`.
    pub script_coloring: Option<ScriptColoringOptions>,
    /// Multiple-bond geometry used by the source renderer. `Simple` emits two
    /// or three parallel lines; `BallStickType1` emits the original full bond
    /// plus one shortened, offset cylinder with spherical end caps.
    pub valence_bond_mode: ValenceBondMode,
    /// SimpleRenderer offsets along `MolBond::getDblBondDir`.
    pub simple_bond_offsets: [f64; 2],
    /// Backbone sampled as a PIECEWISE-LINEAR path through the raw Cα pivots
    /// (straight between residues) instead of a smoothed cubic spline — the C++
    /// `TraceRenderer` (a Cα line strip, `TraceRenderer.cpp:102`), as distinct from
    /// the splined `tube`/`spline`/`ribbon`/`cartoon`. Set only for the `trace`
    /// renderer; keeps the tube-mesh machinery (cross-section, coloring, sel, putty).
    pub linear_backbone: bool,
}

/// Ribbon cross-section width/thickness FACTORS (multiplied by the base coil
/// radius) per secondary-structure class — the configurable form of C++
/// `RibbonRenderer`'s helix/sheet/coil `TubeSection`s. Defaults reproduce the
/// historical flat helix/sheet bands and round coil.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RibbonSections {
    pub helix_width: f64,
    pub helix_thick: f64,
    pub sheet_width: f64,
    pub sheet_thick: f64,
    pub coil_width: f64,
    pub coil_thick: f64,
}

impl Default for RibbonSections {
    fn default() -> Self {
        Self {
            helix_width: 2.2,
            helix_thick: 0.7,
            sheet_width: 2.0,
            sheet_thick: 0.6,
            coil_width: 1.0,
            coil_thick: 1.0,
        }
    }
}

/// ABSOLUTE ribbon cross-section semi-axes per secondary-structure class,
/// resolved from a C++ `<helix>/<sheet>/<coil>` style sub-element. Each entry is
/// `(base_width, thick)` in Å — the binormal (flat-face) and normal (thickness)
/// semi-axes the C++ `TubeSection` builds as `(m_lw·tuber, m_lw)` (so the band's
/// aspect ratio equals `tuber`). When `Some`, it OVERRIDES the scale×factor
/// [`RibbonSections`] model for that class (faithful absolute dims vs the
/// independently-tuned defaults); `None` keeps the factor model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyledRibbonSections {
    pub helix: Option<(f64, f64)>,
    pub sheet: Option<(f64, f64)>,
    pub coil: Option<(f64, f64)>,
    pub helix_shape: SectionShape,
    pub sheet_shape: SectionShape,
    pub coil_shape: SectionShape,
    pub helix_sharp: f64,
    pub sheet_sharp: f64,
    pub coil_sharp: f64,
    pub helix_detail: Option<u32>,
    pub sheet_detail: Option<u32>,
    pub coil_detail: Option<u32>,
}

impl Default for StyledRibbonSections {
    fn default() -> Self {
        Self {
            helix: None,
            sheet: None,
            coil: None,
            helix_shape: SectionShape::Elliptical,
            sheet_shape: SectionShape::Elliptical,
            coil_shape: SectionShape::Elliptical,
            helix_sharp: 0.4,
            sheet_sharp: 0.4,
            coil_sharp: 0.4,
            helix_detail: None,
            sheet_detail: None,
            coil_detail: None,
        }
    }
}

impl StyledRibbonSections {
    fn for_ss(&self, ss: Secondary) -> Option<(f64, f64)> {
        match ss {
            Secondary::Helix => self.helix,
            Secondary::Sheet => self.sheet,
            Secondary::Coil => self.coil,
        }
    }

    fn shape_for(&self, ss: Secondary) -> SectionShape {
        match ss {
            Secondary::Helix => self.helix_shape,
            Secondary::Sheet => self.sheet_shape,
            Secondary::Coil => self.coil_shape,
        }
    }

    fn sharp_for(&self, ss: Secondary) -> f64 {
        match ss {
            Secondary::Helix => self.helix_sharp,
            Secondary::Sheet => self.sheet_sharp,
            Secondary::Coil => self.coil_sharp,
        }
        .clamp(0.0, 1.0)
    }

    fn max_detail(&self) -> Option<u32> {
        [self.helix_detail, self.sheet_detail, self.coil_detail]
            .into_iter()
            .flatten()
            .max()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RibbonSectionValues {
    pub helix: f64,
    pub sheet: f64,
    pub coil: f64,
}

impl RibbonSectionValues {
    fn for_ss(self, ss: Secondary) -> f64 {
        match ss {
            Secondary::Helix => self.helix,
            Secondary::Sheet => self.sheet,
            Secondary::Coil => self.coil,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RibbonJunctionType {
    #[default]
    Smooth,
    Arrow,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RibbonJunction {
    pub kind: RibbonJunctionType,
    pub gamma: f64,
    pub base_width: f64,
    pub arrow_width: f64,
}

impl Default for RibbonJunction {
    fn default() -> Self {
        Self {
            kind: RibbonJunctionType::Smooth,
            gamma: 2.2,
            base_width: 0.0,
            arrow_width: 1.8,
        }
    }
}

/// Ribbon cross-section profile (C++ `TubeSection` `TS_*`): a smooth ellipse vs a
/// flat-faced rectangle. A rectangle gives the sharp-edged "fancy" ribbon (each
/// side carries an axis-aligned face normal); an ellipse the rounded default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SectionShape {
    #[default]
    Elliptical,
    Rectangular,
    RoundSquare,
    Fancy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TubeCapType {
    None,
    Flat,
    #[default]
    Sphere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoleculeColorMode {
    Element,
    Uniform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValenceBondMode {
    #[default]
    Off,
    Simple,
    BallStickType1,
}

/// Tube/trace "putty" mode (C++ `TubeRenderer`): modulate the backbone tube
/// radius by a per-residue scalar (B-factor or occupancy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PuttyMode {
    Off,
    /// Linear ramp of width across [min, max] of the target value.
    Linear,
    /// Two-sided ramp pivoting at the mean (thin below mean, thick above).
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PuttyTarget {
    BFactor,
    Occupancy,
}

/// Putty parameters mirroring `TubeRenderer`'s `putty_mode`/`putty_tgt`/
/// `putty_loscl`/`putty_hiscl`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PuttyOptions {
    pub mode: PuttyMode,
    pub target: PuttyTarget,
    pub loscl: f64,
    pub hiscl: f64,
}

impl Default for PuttyOptions {
    fn default() -> Self {
        Self {
            mode: PuttyMode::Off,
            target: PuttyTarget::BFactor,
            loscl: 3.0,
            hiscl: 3.0,
        }
    }
}

impl PuttyOptions {
    /// The radius scale at a normalized target value (C++ `TubeRenderer::getEScl`).
    /// `raw` is the interpolated target value; `lo`/`hi`/`aver` are its min/max/
    /// mean over the backbone pivots.
    fn scale(&self, raw: f64, lo: f64, hi: f64, aver: f64) -> f64 {
        match self.mode {
            PuttyMode::Off => 1.0,
            PuttyMode::Linear => {
                let v = if (hi - lo).abs() < 1.0e-9 {
                    0.0
                } else {
                    (raw - lo) / (hi - lo)
                };
                (self.hiscl - 1.0 / self.loscl) * v + 1.0 / self.loscl
            }
            PuttyMode::Scale => {
                if raw < aver {
                    let v = if (aver - lo).abs() < 1.0e-9 {
                        0.0
                    } else {
                        (raw - lo) / (aver - lo)
                    };
                    ((self.loscl - 1.0) * v + 1.0) / self.loscl
                } else {
                    let v = if (hi - aver).abs() < 1.0e-9 {
                        0.0
                    } else {
                        (raw - aver) / (hi - aver)
                    };
                    (self.hiscl - 1.0) * v + 1.0
                }
            }
        }
    }
}

impl Default for MoleculeRenderOptions {
    fn default() -> Self {
        Self {
            object_uid: None,
            atom_radius: 0.45,
            bond_radius: 0.08,
            vdw_scale: 0.22,
            element_radii: None,
            draw_atoms: true,
            draw_bonds: true,
            color_mode: MoleculeColorMode::Element,
            uniform_color: None,
            cpk_colors: std::collections::BTreeMap::new(),
            selected_indices: Vec::new(),
            putty: PuttyOptions::default(),
            mainchain_radius: None,
            ribbon_sections: RibbonSections::default(),
            anchor_weight: 0.0,
            atom_radius_abs: None,
            sphere_detail: None,
            ribbon_axial_detail: None,
            ribbon_radial_detail: None,
            ribbon_section_sharp: 0.4,
            styled_ribbon: StyledRibbonSections::default(),
            ribbon_smoothing: RibbonSectionValues::default(),
            helix_head: RibbonJunction::default(),
            helix_tail: RibbonJunction::default(),
            sheet_head: RibbonJunction {
                kind: RibbonJunctionType::Arrow,
                ..RibbonJunction::default()
            },
            sheet_tail: RibbonJunction::default(),
            helix_back_color: None,
            sheet_side_color: None,
            cartoon_anchor_selection: None,
            cartoon_anchor_weight: 10.0,
            cartoon_helix_ribbon: false,
            cartoon_helix_width: 2.3,
            cartoon_helix_width_plus: 0.2,
            pivot_atom: None,
            smooth_color: false,
            start_cap_type: TubeCapType::None,
            end_cap_type: TubeCapType::None,
            segment_end_fade: false,
            uniform_backbone: false,
            backbone_atom_colors: false,
            draw_indices: None,
            object_paint: Vec::new(),
            rainbow: None,
            bfac: None,
            atom_color_property: None,
            script_coloring: None,
            valence_bond_mode: ValenceBondMode::Off,
            simple_bond_offsets: [0.1, 0.0],
            linear_backbone: false,
        }
    }
}

/// Renderer-factory defaults shared by interactive and file-rendering paths.
///
/// The public renderer name `cpk` resolves to CPK2Renderer, while the legacy
/// CPKRenderer remains script-constructible with its own detail default.
pub fn molecule_render_options_for_renderer(renderer_type: &str) -> MoleculeRenderOptions {
    let mut options = MoleculeRenderOptions::default();
    match renderer_type {
        "simple" => {
            options.valence_bond_mode = ValenceBondMode::Simple;
        }
        "cpk" => {
            options.element_radii = Some(VdwRadii::default());
            options.sphere_detail = Some(10);
            options.bond_radius = 0.0;
            options.draw_bonds = false;
        }
        "ballstick" => {
            options.vdw_scale = 0.16;
            options.bond_radius = 0.08;
            options.sphere_detail = Some(3);
        }
        "trace" | "ribbon" | "cartoon" | "tube" | "spline" | "nucl" => {
            options.vdw_scale = 0.12;
            options.bond_radius = 0.12;
        }
        "molsurf" | "dsurface" | "directsurf" | "dsurf2" => {
            options.vdw_scale = 0.32;
            options.bond_radius = 0.04;
        }
        _ => {}
    }
    options.linear_backbone = renderer_type == "trace";
    options
}

/// Apply a renderer's resolved `<style>` preset onto render options so a scene's
/// `<renderer style="DefaultBallStick">` inherits the styled geometry instead of
/// the hardcoded defaults. Mirrors the geometry-bearing half of C++
/// `BallStickRenderer`/`CPKRenderer` style properties:
///
/// - `bondw` → [`MoleculeRenderOptions::bond_radius`] (absolute bond cylinder radius)
/// - `sphr`  → [`MoleculeRenderOptions::atom_radius_abs`] (absolute atom ball radius)
///
/// Inline `.qsc` attributes override the named style (handled by
/// [`MolRenderer::resolved_attr`]). Only attributes that flow into the snapshot
/// mesh are mapped; colour/coloring/label/tessellation-detail attributes are
/// resolved by their own pipelines, not here.
/// The conventional scope id under which the bundled default styles are loaded
/// by [`default_style_manager`]. Renderer style resolution should query this id.
pub const DEFAULT_STYLE_SCOPE: u64 = 0;

/// A [`StyleManager`] pre-loaded with the bundled `default_style.xml` presets in
/// [`DEFAULT_STYLE_SCOPE`] — the startup state C++ `StyleMgr` reaches after
/// reading the system style sheet. Use it to resolve a scene renderer's
/// `style="..."` references when no per-scene style set is present.
pub fn default_style_manager() -> StyleManager {
    static DEFAULT_STYLES: OnceLock<StyleManager> = OnceLock::new();

    DEFAULT_STYLES
        .get_or_init(|| {
            let mut styles = StyleManager::new();
            styles.load_builtin_default_styles(DEFAULT_STYLE_SCOPE);
            styles
        })
        .clone()
}

/// Build the style cascade for a scene. QSC style sets are stored in low-to-high
/// priority order, so inserting each at the front makes later nodes win.
pub fn scene_style_manager(scene: &MolScene) -> StyleManager {
    let mut styles = default_style_manager();
    for (index, loaded) in scene.loaded_styles().iter().enumerate() {
        let id = loaded.node.str_attr("id");
        let name = if id.is_empty() {
            format!("scene-style-{index}")
        } else {
            id.to_string()
        };
        let read_only = loaded.node.str_attr("readonly").equals_ignore_case("true");
        styles.load_style_set_from_xml(DEFAULT_STYLE_SCOPE, name, &loaded.data, read_only);
    }
    styles
}

/// Resolve the complete QIF property set used by C++ `AtomIntrRenderer`.
pub fn atom_intr_render_options(
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
) -> AtomIntrRenderOptions {
    let resolved = |name: &str| renderer.resolved_attr(name, styles, scope_id);
    let real = |name: &str, fallback: f64| {
        resolved(name)
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(fallback)
    };
    let cap = |name: &str| match resolved(name)
        .unwrap_or_else(|| "sphere".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "flat" | "0" => AtomIntrCapType::Flat,
        "arrow" | "2" => AtomIntrCapType::Arrow,
        _ => AtomIntrCapType::Sphere,
    };
    let color = resolved("color").unwrap_or_else(|| "#ffff00".to_string());
    AtomIntrRenderOptions {
        mode: match resolved("mode")
            .unwrap_or_else(|| "fancy".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "simple" | "0" => AtomIntrRenderMode::Simple,
            _ => AtomIntrRenderMode::Fancy,
        },
        detail: real("detail", 5.0).max(0.0) as u32,
        stipples: std::array::from_fn(|index| {
            real(
                &format!("stipple{index}"),
                if index < 2 { 1.0 } else { -1.0 },
            )
        }),
        width: real("width", 0.1) as f32,
        color: solid_color_to_spec(&styles.compile_color(&color, scope_id)),
        start_cap: cap("captype_start"),
        end_cap: cap("captype_end"),
        arrow_height: real("arrowheight", 1.0),
        arrow_width: real("arrowwidth", 2.0),
    }
}

/// Build a [`RainbowColoring`] from a `<coloring type="RainbowColoring">` node.
/// Returns `None` for any other or absent coloring type.
pub fn rainbow_coloring_from_attrs<'a>(
    coloring_type: Option<&str>,
    attr: impl Fn(&str) -> Option<&'a str>,
) -> Option<RainbowColoring> {
    if coloring_type != Some("RainbowColoring") {
        return None;
    }
    let mut rainbow = RainbowColoring::new();
    if let Some(mode) = attr("mode") {
        rainbow.set_mode(match mode.trim() {
            "chain" => RainbowColoring::RBC_CHAIN,
            _ => RainbowColoring::RBC_MOL,
        });
    }
    if let Some(value) = attr("start_hue").and_then(|s| s.trim().parse().ok()) {
        rainbow.set_start_hue(value);
    }
    if let Some(value) = attr("end_hue").and_then(|s| s.trim().parse().ok()) {
        rainbow.set_end_hue(value);
    }
    if let Some(value) = attr("sat").and_then(|s| s.trim().parse().ok()) {
        rainbow.set_sat(value);
    }
    if let Some(value) = attr("bri").and_then(|s| s.trim().parse().ok()) {
        rainbow.set_bri(value);
    }
    if let Some(mode) = attr("incr_mode") {
        rainbow.set_incr_mode(match mode.trim() {
            "chain" => RainbowColoring::RBC_INCR_CHAIN,
            "protss" => RainbowColoring::RBC_INCR_PROTSS,
            _ => RainbowColoring::RBC_INCR_RESID,
        });
    }
    Some(rainbow)
}

/// Build a [`BfacColoring`] from a `<coloring type="BfacColoring">` node.
/// Missing properties keep the C++ defaults. Returns `None` for other types.
pub fn bfac_coloring_from_attrs<'a>(
    coloring_type: Option<&str>,
    attr: impl Fn(&str) -> Option<&'a str>,
) -> Option<BfacColoring> {
    if coloring_type != Some("BfacColoring") {
        return None;
    }
    let mut bfac = BfacColoring::new();
    if let Some(mode) = attr("mode") {
        bfac.set_mode(match mode.trim() {
            "occ" => BfacColoring::BFC_OCC,
            "center" => BfacColoring::BFC_CENTER,
            _ => BfacColoring::BFC_BFAC,
        });
    }
    if let Some(mode) = attr("auto") {
        bfac.set_auto_mode(match mode.trim() {
            "none" => BfacColoring::BFA_NONE,
            "rend" => BfacColoring::BFA_REND,
            _ => BfacColoring::BFA_MOL,
        });
    }
    if let Some(color) = attr("lowcol")
        .and_then(|value| cuemol_gfx::parse_color(value).ok())
        .and_then(|value| value.rgba8())
    {
        bfac.set_low_color(SolidColor::from_rgba_u8(color.r, color.g, color.b, color.a));
    }
    if let Some(color) = attr("highcol")
        .and_then(|value| cuemol_gfx::parse_color(value).ok())
        .and_then(|value| value.rgba8())
    {
        bfac.set_high_color(SolidColor::from_rgba_u8(color.r, color.g, color.b, color.a));
    }
    if let Some(value) = attr("lowpar").and_then(|s| s.trim().parse().ok()) {
        bfac.set_low_param(value);
    }
    if let Some(value) = attr("highpar").and_then(|s| s.trim().parse().ok()) {
        bfac.set_high_param(value);
    }
    Some(bfac)
}

/// Apply the object-side coloring reached through a renderer's `$molcol` color.
pub fn apply_object_coloring<'a>(
    options: &mut MoleculeRenderOptions,
    coloring_type: Option<&str>,
    attr: impl Fn(&str) -> Option<&'a str>,
    paint_entries: &[(String, String)],
    default_color: &SolidColor,
) {
    match coloring_type {
        Some("RainbowColoring") => {
            options.rainbow = rainbow_coloring_from_attrs(coloring_type, attr);
        }
        Some("BfacColoring") => {
            options.bfac = bfac_coloring_from_attrs(coloring_type, attr);
        }
        Some("PaintColoring") => {
            options.object_paint = paint_entries.to_vec();
        }
        Some("SolidColoring") => {
            options.object_paint = vec![("*".to_string(), default_color.script_string())];
        }
        Some("CPKColoring") => {
            let carbon = attr("col_C")
                .filter(|value| value.trim() != "$molcol")
                .map(str::to_string)
                .unwrap_or_else(|| default_color.script_string());
            options.object_paint = vec![("*".to_string(), carbon)];
        }
        Some("ScriptColoring") => {
            options.script_coloring =
                attr("script")
                    .filter(|script| !script.is_empty())
                    .map(|script| ScriptColoringOptions {
                        script: script.to_string(),
                        renderer_name: None,
                        renderer_type: None,
                        all_atoms: false,
                    });
        }
        _ if !paint_entries.is_empty() => {
            options.object_paint = paint_entries.to_vec();
        }
        _ => {}
    }
}

pub fn apply_renderer_styles(
    options: &mut MoleculeRenderOptions,
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
) {
    if let Some(bondw) = renderer
        .resolved_attr("bondw", styles, scope_id)
        .and_then(|value| value.trim().parse::<f32>().ok())
    {
        options.bond_radius = bondw;
    }
    if let Some(sphr) = renderer
        .resolved_attr("sphr", styles, scope_id)
        .and_then(|value| value.trim().parse::<f32>().ok())
    {
        options.atom_radius_abs = Some(sphr);
    }
    if let Some(detail) = renderer
        .resolved_attr("detail", styles, scope_id)
        .and_then(|value| value.trim().parse::<u32>().ok())
    {
        options.sphere_detail = Some(detail);
    }
    if let Some(mut radii) = options.element_radii {
        for (element, attribute) in [
            ("H", "vdwr_H"),
            ("C", "vdwr_C"),
            ("N", "vdwr_N"),
            ("O", "vdwr_O"),
            ("S", "vdwr_S"),
            ("P", "vdwr_P"),
            ("X", "vdwr_X"),
        ] {
            if let Some(value) = renderer
                .resolved_attr(attribute, styles, scope_id)
                .and_then(|value| value.trim().parse::<f64>().ok())
            {
                radii.set_radius_for_element(element, value);
            }
        }
        options.element_radii = Some(radii);
    }
    if let Some(axial) = renderer
        .resolved_attr("axialdetail", styles, scope_id)
        .and_then(|value| value.trim().parse::<u32>().ok())
    {
        options.ribbon_axial_detail = Some(axial);
    }
    // Ribbon cross-section sub-elements <helix>/<sheet>/<coil>: each contributes
    // absolute C++ TubeSection semi-axes (width·tuber, width) for that SS class.
    if let Some(helix) = resolve_ribbon_section(renderer, styles, scope_id, "helix") {
        options.styled_ribbon.helix = Some(helix);
    }
    if let Some(sheet) = resolve_ribbon_section(renderer, styles, scope_id, "sheet") {
        options.styled_ribbon.sheet = Some(sheet);
    }
    if let Some(coil) = resolve_ribbon_section(renderer, styles, scope_id, "coil") {
        options.styled_ribbon.coil = Some(coil);
    }
    // When a ribbon style sets helix/sheet (absolute dims) but omits <coil>, the
    // C++ coil section keeps the TubeSection default (width 0.35, tuber 1.0 =>
    // a round 0.35 tube). Apply it so all three SS classes share one unit system
    // instead of mixing absolute helix/sheet with the scale×factor coil.
    if options.styled_ribbon.coil.is_none()
        && (options.styled_ribbon.helix.is_some() || options.styled_ribbon.sheet.is_some())
    {
        options.styled_ribbon.coil = Some((0.35, 0.35));
    }
    // Cross-section PROFILE per class (C++ TubeSection `type`): rectangle vs ellipse.
    options.styled_ribbon.helix_shape = resolve_section_shape(renderer, styles, scope_id, "helix");
    options.styled_ribbon.sheet_shape = resolve_section_shape(renderer, styles, scope_id, "sheet");
    options.styled_ribbon.coil_shape = resolve_section_shape(renderer, styles, scope_id, "coil");
    for (tag, sharp, detail) in [
        (
            "helix",
            &mut options.styled_ribbon.helix_sharp,
            &mut options.styled_ribbon.helix_detail,
        ),
        (
            "sheet",
            &mut options.styled_ribbon.sheet_sharp,
            &mut options.styled_ribbon.sheet_detail,
        ),
        (
            "coil",
            &mut options.styled_ribbon.coil_sharp,
            &mut options.styled_ribbon.coil_detail,
        ),
    ] {
        if let Some(value) = renderer
            .resolved_child_attr(tag, "sharp", styles, scope_id)
            .and_then(|value| value.trim().parse::<f64>().ok())
        {
            *sharp = value.clamp(0.0, 1.0);
        }
        if let Some(value) = renderer
            .resolved_child_attr(tag, "detail", styles, scope_id)
            .and_then(|value| value.trim().parse::<u32>().ok())
        {
            *detail = Some(value.max(3));
        }
    }

    // Putty (C++ TubeRenderer width modulation by B-factor/occupancy): exact qsc
    // attr names + enum values from TubeRenderer.qif.
    if let Some(mode) = renderer
        .resolved_attr("putty_mode", styles, scope_id)
        .as_deref()
    {
        options.putty.mode = match mode {
            "linear1" => PuttyMode::Linear,
            "scale1" => PuttyMode::Scale,
            _ => PuttyMode::Off,
        };
    }
    if let Some(target) = renderer
        .resolved_attr("putty_tgt", styles, scope_id)
        .as_deref()
    {
        options.putty.target = match target {
            "occ" => PuttyTarget::Occupancy,
            _ => PuttyTarget::BFactor,
        };
    }
    if let Some(hi) = renderer
        .resolved_attr("putty_hiscl", styles, scope_id)
        .and_then(|value| value.trim().parse::<f64>().ok())
    {
        options.putty.hiscl = hi;
    }
    if let Some(lo) = renderer
        .resolved_attr("putty_loscl", styles, scope_id)
        .and_then(|value| value.trim().parse::<f64>().ok())
    {
        options.putty.loscl = lo;
    }

    // Renderer-level `<coloring type="...">` from the renderer's own style. The
    // `type` is read through the reserved-attribute resolver; normal properties use
    // resolved child attributes. CPK carbon `$molcol` continues to defer to the
    // object coloring in the render snapshot.
    if let Some(coloring_type) = renderer.resolved_child_type("coloring", styles, scope_id) {
        let attrs: std::collections::BTreeMap<String, String> = [
            "start_hue",
            "end_hue",
            "sat",
            "bri",
            "mode",
            "incr_mode",
            "auto",
            "lowcol",
            "highcol",
            "lowpar",
            "highpar",
            "col_H",
            "col_C",
            "col_N",
            "col_O",
            "col_P",
            "col_S",
            "col_X",
        ]
        .iter()
        .filter_map(|key| {
            renderer
                .resolved_child_attr("coloring", key, styles, scope_id)
                .map(|value| (key.to_string(), value))
        })
        .collect();
        let get = |name: &str| attrs.get(name).map(String::as_str);
        if let Some(rainbow) = rainbow_coloring_from_attrs(Some(coloring_type.as_str()), get) {
            options.rainbow = Some(rainbow);
            options.bfac = None;
            options.object_paint.clear();
        }
        if let Some(bfac) = bfac_coloring_from_attrs(Some(coloring_type.as_str()), get) {
            options.bfac = Some(bfac);
            options.rainbow = None;
            options.object_paint.clear();
        }
        if coloring_type == "SolidColoring" {
            options.color_mode = MoleculeColorMode::Uniform;
            options.uniform_color = renderer
                .resolved_attr("defaultcolor", styles, scope_id)
                .and_then(|value| cuemol_gfx::parse_color(&value).ok());
            options.rainbow = None;
            options.bfac = None;
            options.object_paint.clear();
        }
        if coloring_type == "CPKColoring" {
            options.cpk_colors = ["H", "C", "N", "O", "P", "S", "X"]
                .into_iter()
                .filter_map(|element| {
                    attrs
                        .get(&format!("col_{element}"))
                        .filter(|value| value.as_str() != "$molcol")
                        .and_then(|value| cuemol_gfx::parse_color(value).ok())
                        .map(|color| (element.to_string(), color))
                })
                .collect();
        }
        if coloring_type == "PaintColoring" {
            let node = renderer
                .qsc_children()
                .iter()
                .find(|node| node.tag_name() == "coloring")
                .or_else(|| {
                    renderer.style().and_then(|style| {
                        styles.resolve_style_child_node(scope_id, style, "coloring")
                    })
                });
            options.object_paint = node
                .into_iter()
                .flat_map(|node| node.children())
                .filter(|child| !child.is_attr() && child.tag_name() == "paint")
                .filter_map(|child| {
                    let selection = child.str_attr("sel").to_string();
                    let color = child.str_attr("color").to_string();
                    (!selection.is_empty() && !color.is_empty()).then_some((selection, color))
                })
                .collect();
            options.rainbow = None;
            options.bfac = None;
        }
        if coloring_type == "AtomPropColoring" {
            options.atom_color_property = Some("col".to_string());
        }
        if coloring_type == "ScriptColoring" {
            let node = renderer
                .qsc_children()
                .iter()
                .find(|node| node.tag_name() == "coloring")
                .or_else(|| {
                    renderer.style().and_then(|style| {
                        styles.resolve_style_child_node(scope_id, style, "coloring")
                    })
                });
            options.script_coloring = node
                .and_then(script_source_from_coloring_node)
                .filter(|script| !script.is_empty())
                .map(|script| ScriptColoringOptions {
                    script,
                    renderer_name: Some(renderer.name().to_string()),
                    renderer_type: Some(renderer.renderer_type().to_string()),
                    all_atoms: true,
                });
            options.rainbow = None;
            options.bfac = None;
            options.object_paint.clear();
            options.atom_color_property = None;
        }
    }

    match renderer.renderer_type() {
        "ribbon" => apply_ribbon_renderer_styles(options, renderer, styles, scope_id),
        "cartoon" => apply_cartoon_renderer_styles(options, renderer, styles, scope_id),
        _ => {}
    }
}

fn resolved_renderer_bool(
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
    name: &str,
    fallback: bool,
) -> bool {
    renderer
        .resolved_attr(name, styles, scope_id)
        .as_deref()
        .map(qsc_value_is_true)
        .unwrap_or(fallback)
}

fn resolve_ribbon_junction(
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
    tag: &str,
) -> RibbonJunction {
    let child_f64 = |name: &str, fallback: f64| {
        renderer
            .resolved_child_attr(tag, name, styles, scope_id)
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(fallback)
    };
    let kind = match renderer
        .resolved_child_type(tag, styles, scope_id)
        .or_else(|| renderer.resolved_child_attr(tag, "type", styles, scope_id))
        .as_deref()
    {
        Some("arrow") => RibbonJunctionType::Arrow,
        Some("flat") => RibbonJunctionType::Flat,
        _ => RibbonJunctionType::Smooth,
    };
    RibbonJunction {
        kind,
        gamma: child_f64("gamma", 2.2).clamp(0.1, 10.0),
        base_width: child_f64("basw", 0.0).clamp(0.0, 1.0),
        arrow_width: child_f64("arrow", 1.8).clamp(1.0, 3.0),
    }
}

fn apply_mainchain_renderer_styles(
    options: &mut MoleculeRenderOptions,
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
    smooth_color_default: bool,
    cap_default: TubeCapType,
) {
    options.smooth_color = resolved_renderer_bool(
        renderer,
        styles,
        scope_id,
        "smoothcolor",
        smooth_color_default,
    );
    options.pivot_atom = renderer
        .resolved_attr("pivotatom", styles, scope_id)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    options.start_cap_type = renderer
        .resolved_attr("start_captype", styles, scope_id)
        .as_deref()
        .map(parse_tube_cap_type)
        .unwrap_or(cap_default);
    options.end_cap_type = renderer
        .resolved_attr("end_captype", styles, scope_id)
        .as_deref()
        .map(parse_tube_cap_type)
        .unwrap_or(cap_default);
    options.segment_end_fade =
        resolved_renderer_bool(renderer, styles, scope_id, "segend_fade", false);
    options.ribbon_radial_detail = options
        .styled_ribbon
        .max_detail()
        .or(options.ribbon_radial_detail);
}

pub fn apply_ribbon_renderer_styles(
    options: &mut MoleculeRenderOptions,
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
) {
    apply_mainchain_renderer_styles(
        options,
        renderer,
        styles,
        scope_id,
        true,
        TubeCapType::Sphere,
    );
    let resolved_f64 = |name: &str, fallback: f64| {
        renderer
            .resolved_attr(name, styles, scope_id)
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(fallback)
    };
    options.ribbon_smoothing = RibbonSectionValues {
        helix: resolved_f64("helix_smooth", 0.0),
        sheet: resolved_f64("sheet_smooth", 0.5),
        coil: resolved_f64("coil_smooth", 0.0),
    };
    options.helix_head = resolve_ribbon_junction(renderer, styles, scope_id, "helixhead");
    options.helix_tail = resolve_ribbon_junction(renderer, styles, scope_id, "helixtail");
    options.sheet_head = resolve_ribbon_junction(renderer, styles, scope_id, "sheethead");
    options.sheet_tail = resolve_ribbon_junction(renderer, styles, scope_id, "sheettail");
    if resolved_renderer_bool(renderer, styles, scope_id, "helix_usebackcol", false) {
        options.helix_back_color = renderer
            .resolved_attr("helix_backcol", styles, scope_id)
            .and_then(|value| resolve_color_name(&value, styles));
    }
    if resolved_renderer_bool(renderer, styles, scope_id, "sheet_usesidecol", false) {
        options.sheet_side_color = renderer
            .resolved_attr("sheet_sidecol", styles, scope_id)
            .and_then(|value| resolve_color_name(&value, styles));
    }
}

pub fn apply_cartoon_renderer_styles(
    options: &mut MoleculeRenderOptions,
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
) {
    apply_mainchain_renderer_styles(
        options,
        renderer,
        styles,
        scope_id,
        false,
        TubeCapType::Flat,
    );
    let resolved_f64 = |name: &str, fallback: f64| {
        renderer
            .resolved_attr(name, styles, scope_id)
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(fallback)
    };
    options.ribbon_smoothing = RibbonSectionValues {
        helix: resolved_f64("helix_smooth", 3.0),
        sheet: resolved_f64("sheet_smooth", 2.0),
        coil: resolved_f64("coil_smooth", -1.0),
    };
    options.cartoon_anchor_selection = renderer
        .resolved_attr("anchor_sel", styles, scope_id)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    options.cartoon_anchor_weight = resolved_f64("anchor_weight", 10.0).clamp(0.0, 20.0);
    options.cartoon_helix_ribbon =
        resolved_renderer_bool(renderer, styles, scope_id, "helix_ribbon", false);
    options.cartoon_helix_width = resolved_f64("helix_width", 2.3).max(f64::EPSILON);
    options.cartoon_helix_width_plus = resolved_f64("helix_wplus", 0.2);
    if options.cartoon_helix_ribbon {
        if let Some(section) = resolve_ribbon_section(renderer, styles, scope_id, "ribhelix") {
            options.styled_ribbon.helix = Some(section);
        }
        options.styled_ribbon.helix_shape =
            resolve_section_shape(renderer, styles, scope_id, "ribhelix");
        if let Some(value) = renderer
            .resolved_child_attr("ribhelix", "sharp", styles, scope_id)
            .and_then(|value| value.trim().parse::<f64>().ok())
        {
            options.styled_ribbon.helix_sharp = value.clamp(0.0, 1.0);
        }
        if let Some(value) = renderer
            .resolved_child_attr("ribhelix", "detail", styles, scope_id)
            .and_then(|value| value.trim().parse::<u32>().ok())
        {
            options.styled_ribbon.helix_detail = Some(value.max(3));
        }
        options.helix_head = resolve_ribbon_junction(renderer, styles, scope_id, "ribhelix_head");
        options.helix_tail = resolve_ribbon_junction(renderer, styles, scope_id, "ribhelix_tail");
    } else {
        let radius =
            (options.cartoon_helix_width + options.cartoon_helix_width_plus).max(f64::EPSILON);
        let tuber = renderer
            .resolved_child_attr("helix", "tuber", styles, scope_id)
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(1.0)
            .max(f64::EPSILON);
        options.styled_ribbon.helix = Some((radius * tuber, radius));
    }
    options.sheet_head = resolve_ribbon_junction(renderer, styles, scope_id, "sheethead");
    options.ribbon_radial_detail = options
        .styled_ribbon
        .max_detail()
        .or(options.ribbon_radial_detail);
}

/// Apply TubeRenderer/NARenderer properties that are not shared by RibbonRenderer.
pub fn apply_tube_renderer_styles(
    options: &mut MoleculeRenderOptions,
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
) {
    let child_f64 = |name: &str, fallback: f64| {
        renderer
            .resolved_child_attr("section", name, styles, scope_id)
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(fallback)
    };
    let width = child_f64("width", 0.35).max(f64::EPSILON);
    let tuber = child_f64("tuber", 1.0).max(f64::EPSILON);
    let section = (width * tuber, width);
    options.styled_ribbon.helix = Some(section);
    options.styled_ribbon.sheet = Some(section);
    options.styled_ribbon.coil = Some(section);
    let shape = resolve_section_shape(renderer, styles, scope_id, "section");
    options.styled_ribbon.helix_shape = shape;
    options.styled_ribbon.sheet_shape = shape;
    options.styled_ribbon.coil_shape = shape;
    options.ribbon_section_sharp = child_f64("sharp", 0.4).clamp(0.0, 1.0);
    options.styled_ribbon.helix_sharp = options.ribbon_section_sharp;
    options.styled_ribbon.sheet_sharp = options.ribbon_section_sharp;
    options.styled_ribbon.coil_sharp = options.ribbon_section_sharp;
    options.ribbon_radial_detail = renderer
        .resolved_child_attr("section", "detail", styles, scope_id)
        .and_then(|value| value.trim().parse::<u32>().ok())
        .or(Some(16));
    options.styled_ribbon.helix_detail = options.ribbon_radial_detail;
    options.styled_ribbon.sheet_detail = options.ribbon_radial_detail;
    options.styled_ribbon.coil_detail = options.ribbon_radial_detail;
    options.anchor_weight = renderer
        .resolved_attr("smooth", styles, scope_id)
        .and_then(|value| value.trim().parse::<f64>().ok())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    options.smooth_color = renderer
        .resolved_attr("smoothcolor", styles, scope_id)
        .as_deref()
        .map(qsc_value_is_true)
        .unwrap_or(true);
    options.pivot_atom = renderer
        .resolved_attr("pivotatom", styles, scope_id)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    options.start_cap_type = renderer
        .resolved_attr("start_captype", styles, scope_id)
        .as_deref()
        .map(parse_tube_cap_type)
        .unwrap_or(TubeCapType::Sphere);
    options.end_cap_type = renderer
        .resolved_attr("end_captype", styles, scope_id)
        .as_deref()
        .map(parse_tube_cap_type)
        .unwrap_or(TubeCapType::Sphere);
    options.segment_end_fade = renderer
        .resolved_attr("segend_fade", styles, scope_id)
        .as_deref()
        .map(qsc_value_is_true)
        .unwrap_or(false);
    options.uniform_backbone = true;
    options.backbone_atom_colors = true;
}

fn qsc_value_is_true(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    )
}

fn parse_tube_cap_type(value: &str) -> TubeCapType {
    match value.trim() {
        "none" => TubeCapType::None,
        "flat" => TubeCapType::Flat,
        _ => TubeCapType::Sphere,
    }
}

/// Resolve a ribbon sub-element's `type` into a cross-section profile (C++
/// `TubeSection` "rectangle"/"rect" → `TS_RECT`, anything else → elliptical).
fn resolve_section_shape(
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
    tag: &str,
) -> SectionShape {
    match renderer
        .resolved_child_type(tag, styles, scope_id)
        .or_else(|| renderer.resolved_child_attr(tag, "type", styles, scope_id))
        .as_deref()
    {
        Some("rectangle") | Some("rect") => SectionShape::Rectangular,
        Some("roundsquare") => SectionShape::RoundSquare,
        Some("fancy1") => SectionShape::Fancy,
        _ => SectionShape::Elliptical,
    }
}

/// Resolve one ribbon cross-section sub-element (`<helix>`/`<sheet>`/`<coil>`)
/// into absolute semi-axes `(base_width, thick) = (width·tuber, width)`, the C++
/// `TubeSection` geometry. `width` is required (the element must declare a size);
/// a missing `tuber` defaults to 1.0 (a round section).
fn resolve_ribbon_section(
    renderer: &MolRenderer,
    styles: &StyleManager,
    scope_id: u64,
    tag: &str,
) -> Option<(f64, f64)> {
    let width = renderer
        .resolved_child_attr(tag, "width", styles, scope_id)?
        .trim()
        .parse::<f64>()
        .ok()?;
    let tuber = renderer
        .resolved_child_attr(tag, "tuber", styles, scope_id)
        .and_then(|value| value.trim().parse::<f64>().ok())
        .unwrap_or(1.0);
    Some((width * tuber, width))
}

/// Build the atom/bond/mesh snapshot for a molecule and tessellate it into a
/// single triangle mesh, honouring the resolved `sphere_detail` (C++ CPK/
/// BallStick `detail`). This is the styled counterpart of
/// [`cuemol_render::tessellate_snapshot`]: a renderer styled `DefaultCPK`
/// (detail=10) yields rounder atom spheres than the coarse default. Bonds and
/// pre-built meshes are unaffected by detail.
pub fn tessellate_styled_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
) -> RenderMesh {
    let snapshot = molecule_render_snapshot(molecule, options);
    match options.sphere_detail {
        Some(detail) => cuemol_render::tessellate_snapshot_with_detail(&snapshot, detail),
        None => cuemol_render::tessellate_snapshot(&snapshot),
    }
}

fn script_source_from_coloring_node(node: &cuemol_core::LDom2Node) -> Option<String> {
    let script = node
        .children()
        .iter()
        .find(|child| !child.is_attr() && child.tag_name() == "script")?;
    Some(if !script.value().is_empty() {
        script.value().to_string()
    } else {
        script.contents().to_string()
    })
}

fn evaluate_script_colors(
    molecule: &MolCoord,
    options: &ScriptColoringOptions,
) -> Vec<Option<ColorSpec>> {
    use boa_engine::{Context, Source};
    use serde_json::json;

    let residues = molecule.residue_summaries();
    let mut residue_by_atom = vec![serde_json::Value::Null; molecule.atoms().len()];
    for residue in residues {
        let props = molecule
            .residue_props()
            .get(&residue.key)
            .cloned()
            .unwrap_or_default();
        let record = json!({
            "name": residue.res_name,
            "type": props.get("basetype").cloned().unwrap_or_default(),
            "sindex": residue.key.script_index(),
            "index": residue.key.res_id,
            "chainName": residue.key.chain_id,
            "_atomIndices": residue.atom_indices,
            "_props": props,
        });
        for atom_index in record["_atomIndices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_u64)
        {
            if let Some(slot) = residue_by_atom.get_mut(atom_index as usize) {
                *slot = record.clone();
            }
        }
    }
    let atoms = molecule
        .atoms()
        .iter()
        .enumerate()
        .map(|(index, atom)| {
            json!({
                "id": atom.serial,
                "name": atom.name,
                "element": atom.element,
                "bfac": atom.b_factor,
                "occ": atom.occupancy,
                "confid": atom.alt_loc.map(|value| value.to_string()).unwrap_or_default(),
                "pos": {
                    "x": atom.position.x,
                    "y": atom.position.y,
                    "z": atom.position.z,
                    "w": atom.position.w,
                },
                "chainName": atom.chain_id,
                "residIndex": format!("{}{}", atom.res_id, atom.ins_code.unwrap_or(' ')).trim().to_string(),
                "residName": atom.res_name,
                "formalCharge": atom.formal_charge,
                "_intProps": atom.int_props,
                "_realProps": atom.real_props,
                "_residue": residue_by_atom[index],
            })
        })
        .collect::<Vec<_>>();
    let renderer = options
        .renderer_name
        .as_ref()
        .map_or(serde_json::Value::Null, |name| {
            json!({
                "name": name,
                "type": options.renderer_type.as_deref().unwrap_or_default(),
                "type_name": options.renderer_type.as_deref().unwrap_or_default(),
            })
        });
    let atoms_json = match serde_json::to_string(&atoms) {
        Ok(value) => value,
        Err(_) => return vec![None; molecule.atoms().len()],
    };
    let renderer_json = renderer.to_string();
    let script = format!(
        r##"(function () {{
const __rawAtoms = {atoms_json};
function __color(spec) {{
  return {{ __cuemolColor: String(spec), toString: function () {{ return String(spec); }} }};
}}
const cuemol = {{
  makeColor: __color,
  color: __color,
  createObj: function (name, value) {{
    return /color/i.test(String(name)) ? __color(value === undefined ? "#ffffff" : value) : {{ _class: String(name) }};
  }},
  newObj: function (name) {{ return this.createObj(name); }}
}};
function makeColor(spec) {{ return __color(spec); }}
function newObj(name) {{ return cuemol.newObj(name); }}
function __wrapAtom(atom) {{
  atom.getPropTypeName = function (name) {{
    return Object.prototype.hasOwnProperty.call(atom._intProps, name) ? "integer" :
      Object.prototype.hasOwnProperty.call(atom._realProps, name) ? "real" : "";
  }};
  atom.getAtomPropInt = function (name) {{ return atom._intProps[name] === undefined ? 0 : atom._intProps[name]; }};
  atom.getAtomPropReal = function (name) {{ return atom._realProps[name] === undefined ? 0 : atom._realProps[name]; }};
  atom.toString = function () {{ return atom.chainName + "." + atom.residIndex + "." + atom.name; }};
  atom.formatMsg = atom.toString;
  return atom;
}}
const __atoms = __rawAtoms.map(__wrapAtom);
function __wrapResidue(raw) {{
  raw.getAtom = function (name) {{
    const index = raw._atomIndices.find(function (value) {{ return __atoms[value].name.trim() === String(name).trim(); }});
    return index === undefined ? null : __atoms[index];
  }};
  raw.getAtomAltConf = raw.getAtom;
  raw.getAtomsJSON = function () {{ return JSON.stringify(raw._atomIndices.map(function (index) {{ return __atoms[index].name; }})); }};
  raw.getPivotPos = function () {{ const atom = raw.getAtom("CA") || __atoms[raw._atomIndices[0]]; return atom ? atom.pos : null; }};
  raw.getPropStr = function (name) {{ return raw._props[name] === undefined ? "" : raw._props[name]; }};
  raw.toString = function () {{ return raw.chainName + "." + raw.sindex; }};
  return raw;
}}
const __residues = Object.create(null);
__atoms.forEach(function (atom) {{
  const key = atom.chainName + "\u0000" + atom.residIndex;
  if (!__residues[key]) __residues[key] = __wrapResidue(atom._residue);
}});
const obj = {{
  getAtomSize: function () {{ return __atoms.length; }},
  getAtomByID: function (id) {{ return __atoms.find(function (atom) {{ return atom.id === Number(id); }}) || null; }},
  getAtom: function (chain, sindex, name) {{
    return __atoms.find(function (atom) {{
      return atom.chainName === String(chain) && atom.residIndex === String(sindex) && atom.name.trim() === String(name).trim();
    }}) || null;
  }},
  getResidue: function (chain, sindex) {{ return __residues[String(chain) + "\u0000" + String(sindex)] || null; }}
}};
const rend = {renderer_json};
function main(atom, resid) {{
{user_script}
}}
const __results = [];
for (let index = 0; index < __atoms.length; ++index) {{
  const atom = __atoms[index];
  const resid = __residues[atom.chainName + "\u0000" + atom.residIndex];
  try {{
    const value = main(atom, resid);
    __results.push(value && typeof value === "object" && typeof value.__cuemolColor === "string" ? value.__cuemolColor : null);
  }} catch (_) {{
    break;
  }}
}}
return JSON.stringify(__results);
}})()"##,
        user_script = options.script,
    );
    let mut context = Context::default();
    let encoded = match context.eval(Source::from_bytes(&script)) {
        Ok(value) => match value.to_string(&mut context) {
            Ok(value) => value.to_std_string_escaped(),
            Err(_) => return vec![None; molecule.atoms().len()],
        },
        Err(_) => return vec![None; molecule.atoms().len()],
    };
    let returned = serde_json::from_str::<Vec<Option<String>>>(&encoded).unwrap_or_default();
    let styles = default_style_manager();
    (0..molecule.atoms().len())
        .map(|index| {
            returned
                .get(index)
                .and_then(Option::as_deref)
                .map(|value| solid_color_to_spec(&styles.compile_color(value, DEFAULT_STYLE_SCOPE)))
        })
        .collect()
}

/// Resolve the final display colour for every atom under the object's coloring
/// cascade — Rainbow > Bfac > paint applied to carbon (the `$molcol` CPKColoring
/// rule), every other element keeping its CPK element colour — exactly what the
/// CPK/atom renderer draws.
///
/// Extracted from [`molecule_render_snapshot`] (which now calls it, so the two
/// stay identical by construction) so the molecular-surface renderer can colour
/// each surface vertex by its nearest atom and MATCH the atoms the same viewer
/// shows (C++ `MolSurfRenderer::getColorMol`: nearest atom → its scheme colour).
/// Returns one `ColorSpec` per atom, indexed by atom index.
pub fn resolve_atom_colors(molecule: &MolCoord, options: &MoleculeRenderOptions) -> Vec<ColorSpec> {
    let script_colors = options
        .script_coloring
        .as_ref()
        .map(|script| evaluate_script_colors(molecule, script));
    // Object coloring ($molcol cascade) for an atom renderer: CPKColoring carbon is
    // $molcol, so carbons defer to the object's paint/bfac scheme; other elements
    // keep their CPK colour. Built once when the object declares a scheme.
    let paint_styles = (!options.object_paint.is_empty()).then(default_style_manager);
    let bfac_atoms = options.bfac.clone().map(|mut bfac| {
        bfac.start_molecule(molecule);
        bfac
    });
    let rainbow_atoms = options
        .rainbow
        .as_ref()
        .map(|rainbow| rainbow_atom_colors(molecule, rainbow));
    let object_carbon_color = |index: usize, atom: &Atom| -> Option<ColorSpec> {
        if !atom.element.eq_ignore_ascii_case("C") {
            return None;
        }
        if let Some(colors) = &script_colors {
            if !options
                .script_coloring
                .as_ref()
                .is_some_and(|script| script.all_atoms)
            {
                if let Some(color) = colors.get(index).cloned().flatten() {
                    return Some(color);
                }
            }
        }
        // Object colour scheme precedence (same as the ribbon): Rainbow > Bfac > paint.
        if let Some(colors) = &rainbow_atoms {
            if let Some(color) = colors.get(index).copied().flatten() {
                return Some(rgba_to_spec(color));
            }
        }
        if let Some(bfac) = &bfac_atoms {
            return Some(solid_color_to_spec(&bfac.color_for_atom(atom)));
        }
        if let Some(styles) = &paint_styles {
            return resolve_paint_color(&options.object_paint, atom, molecule, styles)
                .map(rgba_to_spec);
        }
        None
    };
    molecule
        .atoms()
        .iter()
        .enumerate()
        .map(|(index, atom)| {
            if options
                .script_coloring
                .as_ref()
                .is_some_and(|script| script.all_atoms)
            {
                if let Some(color) = script_colors
                    .as_ref()
                    .and_then(|colors| colors.get(index))
                    .cloned()
                    .flatten()
                {
                    return color;
                }
            }
            if let Some(property) = options.atom_color_property.as_deref() {
                let color = atom
                    .int_prop(property)
                    .map(|code| SolidColor::from_code(code as u32))
                    .unwrap_or_else(|| SolidColor::from_rgb(0.5, 0.5, 0.5));
                return solid_color_to_spec(&color);
            }
            let element = atom.element.trim().to_ascii_uppercase();
            let custom_cpk = options.cpk_colors.get(&element).cloned().or_else(|| {
                (!matches!(element.as_str(), "H" | "C" | "N" | "O" | "P" | "S"))
                    .then(|| options.cpk_colors.get("X").cloned())
                    .flatten()
            });
            custom_cpk
                .or_else(|| object_carbon_color(index, atom))
                .or_else(|| options.uniform_color.clone())
                .unwrap_or_else(|| atom_color(atom.element.as_str(), options.color_mode))
        })
        .collect()
}

pub fn molecule_render_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
) -> RenderSceneSnapshot {
    let selected = options
        .selected_indices
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    // Per-renderer `sel="..."` draw restriction: when present, only these atoms
    // (and bonds whose both ends are included) are drawn — each keeps its original
    // index for picking. `None` draws the whole molecule.
    let draw = options.draw_indices.as_ref().map(|indices| {
        indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
    });
    let drawn = |index: usize| draw.as_ref().is_none_or(|set| set.contains(&index));
    // Object coloring ($molcol cascade): resolve every atom's final colour once
    // (Rainbow > Bfac > paint on carbon, else CPK). Shared with the molecular
    // surface renderer via `resolve_atom_colors` so a nearest-atom-coloured
    // surface matches these atoms exactly.
    let atom_colors = resolve_atom_colors(molecule, options);
    let mut atoms = if options.draw_atoms {
        molecule
            .atoms()
            .iter()
            .enumerate()
            .filter(|(index, _)| drawn(*index))
            .map(|(index, atom)| {
                let color = atom_colors[index].clone();
                RenderAtom::new(
                    Vector4::new3(atom.position.x, atom.position.y, atom.position.z),
                    atom_radius(atom.element.as_str(), options),
                    Some(color),
                )
                .with_picking(options.object_uid, Some(index), selected.contains(&index))
                .with_detail(options.sphere_detail)
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut bonds = Vec::new();
    if options.draw_bonds {
        for bond in molecule
            .bonds()
            .iter()
            .filter(|bond| drawn(bond.atom1) && drawn(bond.atom2))
        {
            append_valence_bond_geometry(
                molecule, bond, options, &selected, &mut atoms, &mut bonds,
            );
        }
    }
    // Frame the drawn subset: with a sel restriction the bbox comes from the drawn
    // atoms, else the whole molecule.
    let bounds = match &draw {
        None => molecule.bounding_box_for(None),
        Some(set) => bounds_of_atoms(molecule, set),
    };
    RenderSceneSnapshot {
        atoms,
        bonds,
        meshes: Vec::new(),
        transparency_groups: Vec::new(),
        bounds,
    }
}

#[derive(Debug, Clone, Copy)]
struct ScreenLineMeta {
    width: f32,
    object_uid: Option<u64>,
    atom_indices: Option<(usize, usize)>,
    selected: bool,
}

fn append_screen_line(
    bonds: &mut Vec<RenderBond>,
    from: Vector4,
    to: Vector4,
    start_color: ColorSpec,
    end_color: ColorSpec,
    meta: ScreenLineMeta,
) {
    bonds.push(
        RenderBond::new(from, to, (meta.width * 0.01).max(0.0025), Some(start_color))
            .with_end_color(Some(end_color))
            .with_picking(meta.object_uid, meta.atom_indices, meta.selected)
            .with_screen_style(meta.width, false),
    );
}

fn same_resolved_color(lhs: &ColorSpec, rhs: &ColorSpec) -> bool {
    lhs.rgba8() == rhs.rgba8()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionOverlayMode {
    Stick,
    Point,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionOverlayOptions {
    pub line_width: f32,
    pub displacement: [f32; 2],
    pub color: ColorSpec,
    pub mode: SelectionOverlayMode,
    pub object_uid: Option<u64>,
}

impl Default for SelectionOverlayOptions {
    fn default() -> Self {
        Self {
            line_width: 3.0,
            displacement: [3.0, 3.0],
            color: rgba_to_spec([0.0, 1.0, 1.0, 0.5]),
            mode: SelectionOverlayMode::Stick,
            object_uid: None,
        }
    }
}

/// Build the pixel-width overlay of C++ `SelectionRenderer`.
///
/// Stick mode draws selected-selected bonds and a three-axis aster for each
/// selected atom without a selected bond. Point mode emits one square screen
/// point per selected atom. The displacement is applied after projection.
pub fn selection_overlay_snapshot(
    molecule: &MolCoord,
    selected_indices: &[usize],
    options: &SelectionOverlayOptions,
) -> RenderSceneSnapshot {
    let selected = selected_indices
        .iter()
        .copied()
        .filter(|index| *index < molecule.atom_count())
        .collect::<std::collections::BTreeSet<_>>();
    if selected.is_empty() || options.line_width <= 0.0 {
        return RenderSceneSnapshot::empty();
    }

    let make_primitive =
        |from: Vector4, to: Vector4, atom_indices: Option<(usize, usize)>, point: bool| {
            let primitive = RenderBond::new(
                from,
                to,
                (options.line_width * 0.01).max(0.0025),
                Some(options.color.clone()),
            )
            .with_end_color(Some(options.color.clone()))
            .with_picking(options.object_uid, atom_indices, false)
            .with_screen_style(options.line_width, false)
            .with_screen_offset(options.displacement[0], options.displacement[1]);
            if point {
                primitive.as_screen_point()
            } else {
                primitive
            }
        };

    let mut bonds = Vec::new();
    match options.mode {
        SelectionOverlayMode::Point => {
            for index in &selected {
                let position = molecule.atoms()[*index].position;
                bonds.push(make_primitive(
                    position,
                    position,
                    Some((*index, *index)),
                    true,
                ));
            }
        }
        SelectionOverlayMode::Stick => {
            let mut bonded = std::collections::BTreeSet::new();
            for bond in molecule
                .bonds()
                .iter()
                .filter(|bond| selected.contains(&bond.atom1) && selected.contains(&bond.atom2))
            {
                bonded.insert(bond.atom1);
                bonded.insert(bond.atom2);
                bonds.push(make_primitive(
                    molecule.atoms()[bond.atom1].position,
                    molecule.atoms()[bond.atom2].position,
                    Some((bond.atom1, bond.atom2)),
                    false,
                ));
            }
            const ASTER_RADIUS: f64 = 0.25;
            for index in selected.iter().filter(|index| !bonded.contains(index)) {
                let center = molecule.atoms()[*index].position;
                for axis in [
                    Vector4::new3(ASTER_RADIUS, 0.0, 0.0),
                    Vector4::new3(0.0, ASTER_RADIUS, 0.0),
                    Vector4::new3(0.0, 0.0, ASTER_RADIUS),
                ] {
                    bonds.push(make_primitive(
                        center - axis,
                        center + axis,
                        Some((*index, *index)),
                        false,
                    ));
                }
            }
        }
    }

    let bounds = bounds_of_atoms(molecule, &selected);
    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds,
        meshes: Vec::new(),
        transparency_groups: Vec::new(),
        bounds,
    }
}

/// Build the unlit pixel-width geometry of C++ `SimpleRenderer`.
///
/// Bonded atoms are represented only by lines; an unbonded atom gets the
/// reference three-axis 0.25-A aster. Different atom colors meet sharply at the
/// midpoint. GUI picking uses the molecule-coordinate path, not render geometry.
pub fn simple_line_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
    width: f32,
) -> RenderSceneSnapshot {
    let selected = options
        .selected_indices
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let draw = options.draw_indices.as_ref().map(|indices| {
        indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
    });
    let drawn = |index: usize| draw.as_ref().is_none_or(|set| set.contains(&index));
    let colors = resolve_atom_colors(molecule, options);
    let mut bonds = Vec::new();
    let mut bonded = std::collections::BTreeSet::new();
    let width = width.max(0.0);

    if options.draw_bonds && width > 0.0 {
        for bond in molecule
            .bonds()
            .iter()
            .filter(|bond| drawn(bond.atom1) && drawn(bond.atom2))
        {
            let (Some(atom1), Some(atom2)) = (
                molecule.atoms().get(bond.atom1),
                molecule.atoms().get(bond.atom2),
            ) else {
                continue;
            };
            bonded.insert(bond.atom1);
            bonded.insert(bond.atom2);
            let pos1 = atom1.position;
            let pos2 = atom2.position;
            let color1 = colors[bond.atom1].clone();
            let color2 = colors[bond.atom2].clone();
            let selected_bond = selected.contains(&bond.atom1) || selected.contains(&bond.atom2);
            let mut offsets = vec![0.0];
            if options.valence_bond_mode == ValenceBondMode::Simple {
                offsets = match bond.order {
                    BondOrder::Double => options.simple_bond_offsets.to_vec(),
                    BondOrder::Triple => vec![
                        0.0,
                        options.simple_bond_offsets[0],
                        -options.simple_bond_offsets[0],
                    ],
                    _ => offsets,
                };
            }
            let direction = double_bond_direction(molecule, bond);
            for offset in offsets {
                let delta = direction.scale(offset);
                let from = pos1 + delta;
                let to = pos2 + delta;
                if same_resolved_color(&color1, &color2) {
                    append_screen_line(
                        &mut bonds,
                        from,
                        to,
                        color1.clone(),
                        color1.clone(),
                        ScreenLineMeta {
                            width,
                            object_uid: options.object_uid,
                            atom_indices: Some((bond.atom1, bond.atom2)),
                            selected: selected_bond,
                        },
                    );
                } else {
                    let midpoint = (from + to).scale(0.5);
                    append_screen_line(
                        &mut bonds,
                        from,
                        midpoint,
                        color1.clone(),
                        color1.clone(),
                        ScreenLineMeta {
                            width,
                            object_uid: options.object_uid,
                            atom_indices: Some((bond.atom1, bond.atom2)),
                            selected: selected_bond,
                        },
                    );
                    append_screen_line(
                        &mut bonds,
                        to,
                        midpoint,
                        color2.clone(),
                        color2.clone(),
                        ScreenLineMeta {
                            width,
                            object_uid: options.object_uid,
                            atom_indices: Some((bond.atom1, bond.atom2)),
                            selected: selected_bond,
                        },
                    );
                }
            }
        }
    }

    if options.draw_atoms && width > 0.0 {
        const ASTER_RADIUS: f64 = 0.25;
        for (index, atom) in molecule.atoms().iter().enumerate() {
            if !drawn(index) || bonded.contains(&index) {
                continue;
            }
            let center = atom.position;
            let color = colors[index].clone();
            for axis in [
                Vector4::new3(ASTER_RADIUS, 0.0, 0.0),
                Vector4::new3(0.0, ASTER_RADIUS, 0.0),
                Vector4::new3(0.0, 0.0, ASTER_RADIUS),
            ] {
                append_screen_line(
                    &mut bonds,
                    center - axis,
                    center + axis,
                    color.clone(),
                    color.clone(),
                    ScreenLineMeta {
                        width,
                        object_uid: options.object_uid,
                        atom_indices: Some((index, index)),
                        selected: selected.contains(&index),
                    },
                );
            }
        }
    }

    let bounds = match &draw {
        None => molecule.bounding_box_for(None),
        Some(set) => bounds_of_atoms(molecule, set),
    };
    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds,
        meshes: Vec::new(),
        transparency_groups: Vec::new(),
        bounds,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainchainLineMode {
    Trace,
    Spline,
}

fn interpolate_color(lhs: &ColorSpec, rhs: &ColorSpec, t: f64) -> ColorSpec {
    let lhs = lhs.rgba8().unwrap_or(cuemol_gfx::Rgba8 {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    });
    let rhs = rhs.rgba8().unwrap_or(lhs);
    let lerp =
        |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t.clamp(0.0, 1.0)) as u8;
    rgba_to_spec([
        f32::from(lerp(lhs.r, rhs.r)) / 255.0,
        f32::from(lerp(lhs.g, rhs.g)) / 255.0,
        f32::from(lerp(lhs.b, rhs.b)) / 255.0,
        f32::from(lerp(lhs.a, rhs.a)) / 255.0,
    ])
}

/// Build the unlit pixel-width C++ `TraceRenderer` or `SplineRenderer` line path.
pub fn mainchain_line_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
    mode: MainchainLineMode,
    width: f32,
    axial_detail: u32,
    smoothing: f64,
    smooth_color: bool,
) -> RenderSceneSnapshot {
    let selected = options
        .selected_indices
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let draw = options.draw_indices.as_ref().map(|indices| {
        indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
    });
    let chains = collect_mainchain_pivots(molecule, draw.as_ref(), options.pivot_atom.as_deref());
    let colors = resolve_atom_colors(molecule, options);
    let mut bonds = Vec::new();
    let width = width.max(0.0);

    for pivots in &chains {
        if pivots.len() == 1 {
            if mode == MainchainLineMode::Trace && width > 0.0 {
                let point = &pivots[0];
                let color = colors[point.atom_index].clone();
                for axis in [
                    Vector4::new3(0.25, 0.0, 0.0),
                    Vector4::new3(0.0, 0.25, 0.0),
                    Vector4::new3(0.0, 0.0, 0.25),
                ] {
                    append_screen_line(
                        &mut bonds,
                        point.pos - axis,
                        point.pos + axis,
                        color.clone(),
                        color.clone(),
                        ScreenLineMeta {
                            width,
                            object_uid: options.object_uid,
                            atom_indices: Some((point.atom_index, point.atom_index)),
                            selected: selected.contains(&point.atom_index),
                        },
                    );
                }
            }
            continue;
        }
        if mode == MainchainLineMode::Spline && pivots.len() == 2 {
            continue;
        }
        if width <= 0.0 {
            continue;
        }

        let subdiv = if mode == MainchainLineMode::Trace {
            1
        } else {
            axial_detail.max(1) as usize
        };
        let mut positions = pivots.iter().map(|point| point.pos).collect::<Vec<_>>();
        if mode == MainchainLineMode::Spline && positions.len() >= 3 && smoothing > 0.0 {
            let original = positions.clone();
            let weight = smoothing.clamp(0.0, 1.0);
            for index in 1..original.len() - 1 {
                let midpoint = (original[index - 1] + original[index + 1]).scale(0.5);
                positions[index] = original[index].scale(1.0 - weight) + midpoint.scale(weight);
            }
        }
        let samples = if mode == MainchainLineMode::Trace {
            linear_samples(&positions, subdiv)
        } else {
            cubic_spline_samples(&positions, subdiv)
        };
        let mut sample_colors = Vec::with_capacity(samples.len());
        for segment in 0..pivots.len() - 1 {
            let lhs = &colors[pivots[segment].atom_index];
            let rhs = &colors[pivots[segment + 1].atom_index];
            for step in 0..subdiv {
                sample_colors.push(interpolate_color(lhs, rhs, step as f64 / subdiv as f64));
            }
        }
        sample_colors.push(colors[pivots.last().expect("non-empty pivots").atom_index].clone());
        for index in 0..samples.len().saturating_sub(1) {
            let pivot_segment = (index / subdiv).min(pivots.len() - 2);
            let start_index = pivots[pivot_segment].atom_index;
            let end_index = pivots[pivot_segment + 1].atom_index;
            let start_color = sample_colors[index].clone();
            let end_color = if smooth_color {
                sample_colors[index + 1].clone()
            } else {
                start_color.clone()
            };
            append_screen_line(
                &mut bonds,
                samples[index],
                samples[index + 1],
                start_color,
                end_color,
                ScreenLineMeta {
                    width,
                    object_uid: options.object_uid,
                    atom_indices: Some((start_index, end_index)),
                    selected: selected.contains(&start_index) || selected.contains(&end_index),
                },
            );
        }
    }

    let bounds = match &draw {
        None => molecule.bounding_box_for(None),
        Some(set) => bounds_of_atoms(molecule, set),
    };
    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds,
        meshes: Vec::new(),
        transparency_groups: Vec::new(),
        bounds,
    }
}

fn append_valence_bond_geometry(
    molecule: &MolCoord,
    bond: &Bond,
    options: &MoleculeRenderOptions,
    selected: &std::collections::BTreeSet<usize>,
    atoms: &mut Vec<RenderAtom>,
    bonds: &mut Vec<RenderBond>,
) {
    let (Some(atom1), Some(atom2)) = (
        molecule.atoms().get(bond.atom1),
        molecule.atoms().get(bond.atom2),
    ) else {
        return;
    };
    let pos1 = atom1.position;
    let pos2 = atom2.position;
    let selected_bond = selected.contains(&bond.atom1) || selected.contains(&bond.atom2);
    let add_bond = |bonds: &mut Vec<RenderBond>, from: Vector4, to: Vector4| {
        bonds.push(
            RenderBond::new(from, to, options.bond_radius, Some(default_bond_color()))
                .with_picking(
                    options.object_uid,
                    Some((bond.atom1, bond.atom2)),
                    selected_bond,
                ),
        );
    };
    let multiple = matches!(bond.order, BondOrder::Double | BondOrder::Triple);
    if !multiple || options.valence_bond_mode == ValenceBondMode::Off {
        add_bond(bonds, pos1, pos2);
        return;
    }

    let direction = double_bond_direction(molecule, bond);
    match options.valence_bond_mode {
        ValenceBondMode::Off => unreachable!(),
        ValenceBondMode::Simple => match bond.order {
            BondOrder::Double => {
                for offset in options.simple_bond_offsets {
                    let delta = direction.scale(offset);
                    add_bond(bonds, pos1 + delta, pos2 + delta);
                }
            }
            BondOrder::Triple => {
                add_bond(bonds, pos1, pos2);
                let offset = options.simple_bond_offsets[0];
                for offset in [offset, -offset] {
                    let delta = direction.scale(offset);
                    add_bond(bonds, pos1 + delta, pos2 + delta);
                }
            }
            _ => unreachable!(),
        },
        ValenceBondMode::BallStickType1 => {
            add_bond(bonds, pos1, pos2);
            let axis = (pos2 - pos1)
                .normalize3d()
                .unwrap_or(Vector4::new3(1.0, 0.0, 0.0));
            let side = direction.scale(f64::from(options.bond_radius) * 2.5);
            let shorten = f64::from(options.atom_radius_abs.unwrap_or(0.3)) * 2.0;
            let end1 = pos1 + axis.scale(shorten) + side;
            let end2 = pos2 - axis.scale(shorten) + side;
            add_bond(bonds, end1, end2);
            for (position, atom_index) in [(end1, bond.atom1), (end2, bond.atom2)] {
                atoms.push(
                    RenderAtom::new(position, options.bond_radius, Some(default_bond_color()))
                        .with_picking(
                            options.object_uid,
                            Some(atom_index),
                            selected.contains(&atom_index),
                        )
                        .with_detail(options.sphere_detail),
                );
            }
        }
    }
}

fn default_bond_color() -> ColorSpec {
    ColorSpec {
        base: cuemol_gfx::ColorBase::Rgb {
            r: 0.72,
            g: 0.72,
            b: 0.72,
            a: 1.0,
        },
        modifiers: Vec::new(),
    }
}

fn double_bond_direction(molecule: &MolCoord, bond: &Bond) -> Vector4 {
    let side = |center: usize, other: usize| -> Option<(Vector4, usize)> {
        let center_atom = molecule.atoms().get(center)?;
        let other_atom = molecule.atoms().get(other)?;
        let neighbors = molecule
            .bonds()
            .iter()
            .filter_map(|candidate| {
                let neighbor = if candidate.atom1 == center && candidate.atom2 != other {
                    candidate.atom2
                } else if candidate.atom2 == center && candidate.atom1 != other {
                    candidate.atom1
                } else {
                    return None;
                };
                let atom = molecule.atoms().get(neighbor)?;
                (!atom.element.eq_ignore_ascii_case("H")).then_some(neighbor)
            })
            .collect::<Vec<_>>();
        let &distal = neighbors.last()?;
        let distal_atom = molecule.atoms().get(distal)?;
        let axis = (other_atom.position - center_atom.position)
            .normalize3d()
            .ok()?;
        let relative = distal_atom.position - center_atom.position;
        let normal = (relative - axis.scale(axis.dot3d(relative)))
            .normalize3d()
            .ok()?;
        Some((normal, neighbors.len()))
    };

    match (side(bond.atom1, bond.atom2), side(bond.atom2, bond.atom1)) {
        (Some((normal, _)), None) | (None, Some((normal, _))) => normal,
        (None, None) => Vector4::new3(1.0, 0.0, 0.0),
        (Some((normal1, count1)), Some((normal2, count2))) => {
            if count1 == 1 && count2 > 1 {
                normal1
            } else if count1 > 1 && count2 == 1 {
                normal2
            } else {
                normal1
            }
        }
    }
}

/// Bounding box of a subset of a molecule's atoms (by index), or `None` if empty.
fn bounds_of_atoms(
    molecule: &MolCoord,
    indices: &std::collections::BTreeSet<usize>,
) -> Option<(Vector4, Vector4)> {
    let mut bounds: Option<(Vector4, Vector4)> = None;
    for &index in indices {
        let Some(atom) = molecule.atoms().get(index) else {
            continue;
        };
        let p = atom.position;
        match &mut bounds {
            None => bounds = Some((p, p)),
            Some((min, max)) => {
                min.x = min.x.min(p.x);
                min.y = min.y.min(p.y);
                min.z = min.z.min(p.z);
                max.x = max.x.max(p.x);
                max.y = max.y.max(p.y);
                max.z = max.z.max(p.z);
            }
        }
    }
    bounds
}

/// Build a smooth round main-chain trace snapshot.
///
/// This helper gathers each chain's pivot atoms (CA for proteins, P/C1' for
/// nucleic acids), interpolates them with a Catmull-Rom spline, and emits round
/// primitives along the backbone. Ribbon and cartoon renderers use
/// [`ribbon_mesh_snapshot`] for their secondary-structure triangle meshes.
pub fn mainchain_trace_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
) -> RenderSceneSnapshot {
    const SUBDIV: usize = 8;
    let tube_radius = options
        .mainchain_radius
        .unwrap_or_else(|| (options.bond_radius * 3.0).clamp(0.18, 0.6));

    let draw = options.draw_indices.as_ref().map(|indices| {
        indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
    });
    let chains = collect_mainchain_pivots(molecule, draw.as_ref(), options.pivot_atom.as_deref());
    let putty = options.putty;

    // Putty statistics (min/max/mean of the target value) over every backbone
    // pivot — C++ TubeRenderer::beginRend computes these once for the object.
    let (lo, hi, aver) = putty_value_stats(&chains, &putty);
    let radius_at = |raw: f64| (tube_radius as f64 * putty.scale(raw, lo, hi, aver)) as f32;

    let mut atoms = Vec::new();
    let mut bonds = Vec::new();

    for (chain_index, pivots) in chains.iter().enumerate() {
        let color = mainchain_chain_color(chain_index);
        let positions = pivots.iter().map(|point| point.pos).collect::<Vec<_>>();
        if positions.len() == 1 {
            let r = if putty.mode == PuttyMode::Off {
                tube_radius
            } else {
                radius_at(pivot_target(&pivots[0], putty.target))
            };
            atoms.push(RenderAtom::new(positions[0], r, Some(color.clone())));
            continue;
        }
        let samples = cubic_spline_samples(&positions, SUBDIV);
        // Per-sample putty radius: the target value interpolated along the same
        // (segment + sub/SUBDIV) parametrization the spline samples use.
        let n = positions.len();
        let radius_of_sample = |i: usize| -> f32 {
            if putty.mode == PuttyMode::Off {
                return tube_radius;
            }
            let seg = (i / SUBDIV).min(n - 2);
            let rho = (i - seg * SUBDIV) as f64 / SUBDIV as f64;
            let raw = pivot_target(&pivots[seg], putty.target) * (1.0 - rho)
                + pivot_target(&pivots[seg + 1], putty.target) * rho;
            radius_at(raw)
        };
        for (i, point) in samples.iter().enumerate() {
            atoms.push(RenderAtom::new(
                *point,
                radius_of_sample(i),
                Some(color.clone()),
            ));
        }
        for (i, segment) in samples.windows(2).enumerate() {
            let r = (radius_of_sample(i) + radius_of_sample(i + 1)) * 0.5;
            bonds.push(RenderBond::new(
                segment[0],
                segment[1],
                r,
                Some(color.clone()),
            ));
        }
    }

    RenderSceneSnapshot {
        atoms,
        bonds,
        meshes: Vec::new(),
        transparency_groups: Vec::new(),
        bounds: molecule.bounding_box_for(None),
    }
}

/// Multiply each per-sample ribbon width scale by the putty factor at that
/// sample (the target value interpolated along the same segment/subdiv
/// parametrization the spline uses). Mirrors `mainchain_trace_snapshot`'s
/// per-sample putty radius so the ribbon and trace agree.
fn apply_putty_to_width_scales(
    width_scales: &mut [f64],
    pivots: &[MainchainPoint],
    subdiv: usize,
    putty: &PuttyOptions,
    lo: f64,
    hi: f64,
    aver: f64,
) {
    let n = pivots.len();
    if n < 2 {
        return;
    }
    let subdiv = subdiv.max(1);
    for (i, scale) in width_scales.iter_mut().enumerate() {
        let seg = (i / subdiv).min(n - 2);
        let rho = (i - seg * subdiv) as f64 / subdiv as f64;
        let raw = pivot_target(&pivots[seg], putty.target) * (1.0 - rho)
            + pivot_target(&pivots[seg + 1], putty.target) * rho;
        *scale *= putty.scale(raw, lo, hi, aver);
    }
}

/// Min/max/mean of the putty target value over every backbone pivot (C++
/// `TubeRenderer::beginRend` precompute). Returns the identity `(0,1,0.5)` when
/// putty is off or there are no pivots.
fn putty_value_stats(chains: &[Vec<MainchainPoint>], putty: &PuttyOptions) -> (f64, f64, f64) {
    if putty.mode == PuttyMode::Off {
        return (0.0, 1.0, 0.5);
    }
    let vals: Vec<f64> = chains
        .iter()
        .flatten()
        .map(|point| pivot_target(point, putty.target))
        .collect();
    if vals.is_empty() {
        return (0.0, 1.0, 0.5);
    }
    (
        vals.iter().copied().fold(f64::INFINITY, f64::min),
        vals.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        vals.iter().sum::<f64>() / vals.len() as f64,
    )
}

/// Resolve an atom's object paint colour: the first entry whose selection matches
/// the atom, its colour name resolved to RGBA (C++ first-match PaintColoring).
/// `None` when no entry matches.
fn resolve_paint_color(
    entries: &[(String, String)],
    atom: &Atom,
    molecule: &MolCoord,
    styles: &StyleManager,
) -> Option<[f32; 4]> {
    for (sel, color_name) in entries {
        let Ok(selector) = parse_selector(sel) else {
            continue;
        };
        if selector.matches_in_molecule(atom, molecule) {
            return resolve_color_name(color_name, styles);
        }
    }
    None
}

/// Wrap an RGBA `[f32;4]` in a flat `ColorSpec`.
fn rgba_to_spec(rgba: [f32; 4]) -> ColorSpec {
    ColorSpec {
        base: cuemol_gfx::ColorBase::Rgb {
            r: rgba[0],
            g: rgba[1],
            b: rgba[2],
            a: rgba[3],
        },
        modifiers: Vec::new(),
    }
}

/// Convert a `SolidColor` to a flat `ColorSpec`.
fn solid_color_to_spec(color: &SolidColor) -> ColorSpec {
    let c = color.rgba8();
    rgba_to_spec([
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ])
}

/// Resolve a colour name/value ("SteelBlue", "#FFFAF0", …) to RGBA, preferring
/// the bundled X11 table (the same source C++ uses) then the generic parser.
fn resolve_color_name(name: &str, styles: &StyleManager) -> Option<[f32; 4]> {
    let to_rgba = |c: cuemol_gfx::Rgba8| {
        [
            c.r as f32 / 255.0,
            c.g as f32 / 255.0,
            c.b as f32 / 255.0,
            c.a as f32 / 255.0,
        ]
    };
    if let Some(color) = styles.get_color(name, DEFAULT_STYLE_SCOPE) {
        return Some(to_rgba(color.rgba8()));
    }
    cuemol_gfx::parse_color(name)
        .ok()
        .and_then(|spec| spec.rgba8())
        .map(to_rgba)
}

/// Per-spline-sample paint colours for a ribbon chain: each sample takes its
/// segment's start residue's paint colour (resolved once per pivot). `None` for a
/// sample whose residue matches no paint entry (the ribbon keeps its section
/// colour there).
fn ribbon_paint_colors(
    sample_count: usize,
    pivots: &[MainchainPoint],
    subdiv: usize,
    entries: &[(String, String)],
    molecule: &MolCoord,
    styles: &StyleManager,
) -> Vec<Option<[f32; 4]>> {
    let n = pivots.len();
    let subdiv = subdiv.max(1);
    let pivot_colors: Vec<Option<[f32; 4]>> = pivots
        .iter()
        .map(|pivot| {
            molecule
                .atoms()
                .get(pivot.atom_index)
                .and_then(|atom| resolve_paint_color(entries, atom, molecule, styles))
        })
        .collect();
    (0..sample_count)
        .map(|i| {
            let seg = (i / subdiv).min(n.saturating_sub(1));
            pivot_colors.get(seg).copied().flatten()
        })
        .collect()
}

#[derive(Debug)]
struct RainbowResidue {
    chain: String,
    res_id: i32,
    ins_code: Option<char>,
    atom_indices: Vec<usize>,
    has_pivot: bool,
    secondary: Secondary,
}

/// Build the residue-to-colour map used by C++ `RainbowColoring::start`.
fn rainbow_atom_colors(molecule: &MolCoord, rainbow: &RainbowColoring) -> Vec<Option<[f32; 4]>> {
    let mut residues = Vec::new();
    let mut iter = ResidIterator::all(molecule);
    iter.first();
    while iter.has_more() {
        if let Some(residue) = iter.get() {
            let key = residue.key();
            let secondary = residue
                .atoms()
                .first()
                .map_or(Secondary::Coil, |atom| residue_secondary(molecule, atom));
            residues.push(RainbowResidue {
                chain: key.chain_id.clone(),
                res_id: key.res_id,
                ins_code: key.ins_code,
                atom_indices: residue.atom_indices().to_vec(),
                has_pivot: mainchain_pivot_point(molecule, residue, None).is_some(),
                secondary,
            });
        }
        iter.next();
    }

    let mut colors = vec![None; molecule.atoms().len()];
    if rainbow.mode() == RainbowColoring::RBC_CHAIN {
        let mut start = 0;
        while start < residues.len() {
            let chain = &residues[start].chain;
            let mut end = start + 1;
            while end < residues.len() && residues[end].chain == *chain {
                end += 1;
            }
            assign_rainbow_scope(&residues[start..end], rainbow, &mut colors);
            start = end;
        }
    } else {
        assign_rainbow_scope(&residues, rainbow, &mut colors);
    }
    colors
}

fn assign_rainbow_scope(
    residues: &[RainbowResidue],
    rainbow: &RainbowColoring,
    colors: &mut [Option<[f32; 4]>],
) {
    let markers = match rainbow.incr_mode() {
        RainbowColoring::RBC_INCR_CHAIN => residues
            .windows(2)
            .enumerate()
            .filter_map(|(index, pair)| (pair[0].chain != pair[1].chain).then_some(index))
            .collect::<std::collections::BTreeSet<_>>(),
        RainbowColoring::RBC_INCR_PROTSS => protss_increment_markers(residues),
        _ => std::collections::BTreeSet::new(),
    };
    let increment_count = match rainbow.incr_mode() {
        RainbowColoring::RBC_INCR_RESID => {
            residues.iter().filter(|residue| residue.has_pivot).count()
        }
        RainbowColoring::RBC_INCR_CHAIN | RainbowColoring::RBC_INCR_PROTSS => markers.len(),
        _ => 0,
    };
    let delta = if increment_count == 0 {
        0.0
    } else {
        (rainbow.end_hue() - rainbow.start_hue()) / increment_count as f64
    };
    let mut color_index = 0;
    for (index, residue) in residues.iter().enumerate() {
        let color = rainbow.color_for_hue(rainbow.start_hue() + color_index as f64 * delta);
        for &atom_index in &residue.atom_indices {
            if let Some(slot) = colors.get_mut(atom_index) {
                *slot = Some(color);
            }
        }
        let increment = match rainbow.incr_mode() {
            RainbowColoring::RBC_INCR_RESID => residue.has_pivot,
            RainbowColoring::RBC_INCR_CHAIN | RainbowColoring::RBC_INCR_PROTSS => {
                markers.contains(&index)
            }
            _ => false,
        };
        if increment {
            color_index += 1;
        }
    }
}

fn protss_increment_markers(residues: &[RainbowResidue]) -> std::collections::BTreeSet<usize> {
    let mut markers = std::collections::BTreeSet::new();
    let mut chain_start = 0;
    while chain_start < residues.len() {
        let chain = &residues[chain_start].chain;
        let mut chain_end = chain_start + 1;
        while chain_end < residues.len() && residues[chain_end].chain == *chain {
            chain_end += 1;
        }

        let mut ranges = Vec::<(usize, usize)>::new();
        for index in chain_start..chain_end {
            if residues[index].secondary == Secondary::Coil {
                continue;
            }
            let extends_previous = ranges.last().is_some_and(|(_, end)| {
                index == *end + 1
                    && residues[index].res_id == residues[*end].res_id + 1
                    && residues[index].ins_code == residues[*end].ins_code
            });
            if extends_previous {
                ranges.last_mut().expect("range exists").1 = index;
            } else {
                ranges.push((index, index));
            }
        }
        for pair in ranges.windows(2) {
            let gap_start = pair[0].1 + 1;
            let gap_end = pair[1].0;
            markers.insert(gap_start + (gap_end - gap_start).div_ceil(2));
        }
        chain_start = chain_end;
    }
    markers
}

/// Per-spline-sample rainbow colours resolved from the molecule-wide map.
fn ribbon_rainbow_colors(
    sample_count: usize,
    pivots: &[MainchainPoint],
    subdiv: usize,
    atom_colors: &[Option<[f32; 4]>],
) -> Vec<Option<[f32; 4]>> {
    let subdiv = subdiv.max(1);
    (0..sample_count)
        .map(|i| {
            let seg = (i / subdiv).min(pivots.len().saturating_sub(1));
            pivots
                .get(seg)
                .and_then(|pivot| atom_colors.get(pivot.atom_index))
                .copied()
                .flatten()
        })
        .collect()
}

/// Per-spline-sample B-factor colours for a ribbon chain: each sample takes its
/// segment residue's `BfacColoring::color_for_atom` (auto params already computed).
fn ribbon_bfac_colors(
    sample_count: usize,
    pivots: &[MainchainPoint],
    subdiv: usize,
    bfac: &BfacColoring,
    molecule: &MolCoord,
) -> Vec<Option<[f32; 4]>> {
    let subdiv = subdiv.max(1);
    let pivot_colors: Vec<Option<[f32; 4]>> = pivots
        .iter()
        .map(|pivot| {
            molecule.atoms().get(pivot.atom_index).map(|atom| {
                let c = bfac.color_for_atom(atom).rgba8();
                [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                ]
            })
        })
        .collect();
    (0..sample_count)
        .map(|i| {
            let seg = (i / subdiv).min(pivots.len().saturating_sub(1));
            pivot_colors.get(seg).copied().flatten()
        })
        .collect()
}

fn ribbon_atom_prop_colors(
    sample_count: usize,
    pivots: &[MainchainPoint],
    subdiv: usize,
    property: &str,
    molecule: &MolCoord,
) -> Vec<Option<[f32; 4]>> {
    let subdiv = subdiv.max(1);
    let pivot_colors = pivots
        .iter()
        .map(|pivot| {
            let color = molecule
                .atoms()
                .get(pivot.atom_index)
                .and_then(|atom| atom.int_prop(property))
                .map(|code| SolidColor::from_code(code as u32))
                .unwrap_or_else(|| SolidColor::from_rgb(0.5, 0.5, 0.5));
            let color = color.rgba8();
            Some([
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                color.a as f32 / 255.0,
            ])
        })
        .collect::<Vec<_>>();
    (0..sample_count)
        .map(|index| {
            let segment = (index / subdiv).min(pivots.len().saturating_sub(1));
            pivot_colors.get(segment).copied().flatten()
        })
        .collect()
}

fn ribbon_resolved_atom_colors(
    sample_count: usize,
    pivots: &[MainchainPoint],
    subdiv: usize,
    atom_colors: &[ColorSpec],
) -> Vec<Option<[f32; 4]>> {
    let pivot_colors = pivots
        .iter()
        .map(|pivot| {
            atom_colors
                .get(pivot.atom_index)
                .and_then(ColorSpec::rgba8)
                .map(|color| {
                    [
                        f32::from(color.r) / 255.0,
                        f32::from(color.g) / 255.0,
                        f32::from(color.b) / 255.0,
                        f32::from(color.a) / 255.0,
                    ]
                })
        })
        .collect::<Vec<_>>();
    let subdiv = subdiv.max(1);
    (0..sample_count)
        .map(|index| {
            let segment = (index / subdiv).min(pivots.len().saturating_sub(1));
            pivot_colors.get(segment).copied().flatten()
        })
        .collect()
}

fn interpolate_ribbon_colors(colors: &mut [Option<[f32; 4]>], pivot_count: usize, subdiv: usize) {
    if pivot_count < 2 || colors.is_empty() {
        return;
    }
    let source = colors.to_vec();
    let subdiv = subdiv.max(1);
    for (index, color) in colors.iter_mut().enumerate() {
        let segment = (index / subdiv).min(pivot_count - 2);
        let rho = (index - segment * subdiv) as f32 / subdiv as f32;
        let first = source.get(segment * subdiv).copied().flatten();
        let second = source
            .get(((segment + 1) * subdiv).min(source.len() - 1))
            .copied()
            .flatten();
        *color = match (first, second) {
            (Some(first), Some(second)) => Some(std::array::from_fn(|component| {
                first[component] * (1.0 - rho) + second[component] * rho
            })),
            (Some(color), None) | (None, Some(color)) => Some(color),
            (None, None) => None,
        };
    }
}

/// The putty target value (B-factor or occupancy) of a backbone pivot.
fn pivot_target(point: &MainchainPoint, target: PuttyTarget) -> f64 {
    match target {
        PuttyTarget::Occupancy => point.occupancy,
        PuttyTarget::BFactor => point.b_factor,
    }
}

/// Protein secondary-structure class of a residue, used to shape the ribbon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Secondary {
    Coil,
    Helix,
    Sheet,
}

/// One backbone pivot: its position, the residue's secondary structure, and the
/// pivot atom's B-factor / occupancy (for putty width modulation).
#[derive(Debug, Clone, Copy)]
struct MainchainPoint {
    pos: Vector4,
    ss: Secondary,
    b_factor: f64,
    occupancy: f64,
    /// Molecule atom index of this residue's backbone pivot (CA/P/C1'/O5'), used
    /// to evaluate per-residue paint coloring against the pivot atom.
    atom_index: usize,
}

/// Read a residue's `secondary` property (set by the PDB reader's HELIX/SHEET
/// parsing) via one of its atoms.
fn residue_secondary(molecule: &MolCoord, atom: &Atom) -> Secondary {
    match molecule.residue_prop_for_atom(atom, "secondary") {
        Some(value) if value.eq_ignore_ascii_case("helix") => Secondary::Helix,
        Some(value) if value.eq_ignore_ascii_case("sheet") => Secondary::Sheet,
        _ => Secondary::Coil,
    }
}

/// Collect each chain's backbone pivots (position + secondary structure) in
/// residue order, split into contiguous runs to be splined/traced.
///
/// A run breaks on a chain-name boundary OR a backbone GAP — consecutive pivots
/// farther apart than the link cutoff (a missing/unmodelled stretch, an
/// alternate fragment, or a residue with no pivot). This mirrors C++
/// `MainChainRenderer::isNewSegment` (chain change + `!isLinkedTo` + absent
/// pivot, MainChainRenderer.cpp:37/88), so a chain with an internal break draws
/// disjoint strips instead of one spline bridging the gap with a spurious bar.
fn collect_mainchain_pivots(
    molecule: &MolCoord,
    draw: Option<&std::collections::BTreeSet<usize>>,
    pivot_atom: Option<&str>,
) -> Vec<Vec<MainchainPoint>> {
    let mut runs: Vec<Vec<MainchainPoint>> = Vec::new();
    // Previous accepted pivot: (chain name, position, link cutoff).
    let mut prev: Option<(String, Vector4, f64)> = None;
    let mut iter = ResidIterator::all(molecule);
    iter.first();
    while iter.has_more() {
        if let Some(residue) = iter.get() {
            // Per-renderer `sel="..."`: skip residues with no selected atom so a
            // sel'd ribbon traces only its selection, not the whole backbone.
            let included = draw.is_none_or(|set| {
                residue
                    .atom_indices()
                    .iter()
                    .any(|index| set.contains(index))
            });
            if included {
                if let Some((point, cutoff)) = mainchain_pivot_point(molecule, residue, pivot_atom)
                {
                    let chain = residue.chain_name().to_string();
                    let pos = point.pos;
                    let new_run = match &prev {
                        None => true,
                        Some((prev_chain, prev_pos, prev_cutoff)) => {
                            *prev_chain != chain
                                || pivot_distance(*prev_pos, pos) > prev_cutoff.max(cutoff)
                        }
                    };
                    if new_run {
                        runs.push(Vec::new());
                    }
                    runs.last_mut().expect("a run was just pushed").push(point);
                    prev = Some((chain, pos, cutoff));
                }
            }
        }
        iter.next();
    }
    runs
}

/// Euclidean distance between two backbone pivot positions.
fn pivot_distance(a: Vector4, b: Vector4) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// Max distance between consecutive pivots for them to count as one backbone run.
/// Protein Cα–Cα is ~3.8 Å (so 5.0 keeps a contiguous chain joined but splits a
/// missing-residue gap, where the virtual Cα–Cα blows past ~7 Å); nucleic P–P is
/// ~6–7 Å (so 8.0). This is the distance analogue of C++'s link test
/// (`TopoBuilder.cpp:365` measures the polymer bond), the only signal available
/// for a Cα-only backbone where no peptide bond exists.
const PROTEIN_PIVOT_LINK_DIST: f64 = 5.0;
const NUCLEIC_PIVOT_LINK_DIST: f64 = 8.0;

/// Pick the residue's configured backbone pivot atom, falling back to CA
/// (protein) and then nucleic-acid defaults when the configured atom is absent.
/// Returns the pivot point and its link-distance cutoff (protein vs nucleic),
/// used to break the backbone run at chain gaps.
fn mainchain_pivot_point(
    molecule: &MolCoord,
    residue: &ResidueView,
    pivot_atom: Option<&str>,
) -> Option<(MainchainPoint, f64)> {
    let indices = residue.atom_indices();
    let default_names = ["CA", "P", "C1'", "O5'"];
    let custom_name = pivot_atom.filter(|name| !name.trim().is_empty());
    let custom_names = [custom_name.unwrap_or_default()];
    for name in custom_names
        .iter()
        .filter(|_| custom_name.is_some())
        .chain(default_names.iter())
    {
        for &index in indices {
            let Some(atom) = molecule.atoms().get(index) else {
                continue;
            };
            if atom.name.trim().eq_ignore_ascii_case(name) {
                let is_protein = name.eq_ignore_ascii_case("CA")
                    || indices.iter().any(|&index| {
                        molecule
                            .atoms()
                            .get(index)
                            .is_some_and(|atom| atom.name.trim().eq_ignore_ascii_case("CA"))
                    });
                let cutoff = if is_protein {
                    PROTEIN_PIVOT_LINK_DIST
                } else {
                    NUCLEIC_PIVOT_LINK_DIST
                };
                let point = MainchainPoint {
                    pos: Vector4::new3(atom.position.x, atom.position.y, atom.position.z),
                    ss: residue_secondary(molecule, atom),
                    b_factor: atom.b_factor as f64,
                    occupancy: atom.occupancy as f64,
                    atom_index: index,
                };
                return Some((point, cutoff));
            }
        }
    }
    None
}

const MAINCHAIN_PALETTE: [[f32; 4]; 6] = [
    [0.40, 0.70, 1.00, 1.0],
    [1.00, 0.55, 0.40, 1.0],
    [0.55, 0.85, 0.45, 1.0],
    [0.95, 0.80, 0.35, 1.0],
    [0.80, 0.55, 0.95, 1.0],
    [0.45, 0.85, 0.85, 1.0],
];

fn mainchain_palette_rgba(index: usize) -> [f32; 4] {
    MAINCHAIN_PALETTE[index % MAINCHAIN_PALETTE.len()]
}

fn mainchain_chain_color(index: usize) -> ColorSpec {
    let [r, g, b, _] = mainchain_palette_rgba(index);
    rgb_spec(r, g, b)
}

/// A sampled point on a [`CubicSpline`]: position with the tangent (first
/// derivative) and curvature (second derivative) vectors at that parameter.
#[derive(Debug, Clone, Copy)]
pub struct SplineSample {
    pub position: Vector4,
    pub tangent: Vector4,
    pub curvature: Vector4,
}

/// Natural cubic-spline interpolator — port of C++ molvis `CubicSpline`.
///
/// Interpolates a sequence of control points with a C2-continuous *natural* cubic
/// spline (zero second derivative at the two ends), chord-length weighted (each
/// interval's size is the Euclidean distance between its endpoints). The
/// evaluation parameter runs over `[0, n-1]`; an integer `i` returns control
/// point `i` exactly. This is the curve CueMol uses for the main-chain trace and
/// ribbon backbone (C++ uses Catmull-Rom nowhere — the natural cubic spline is the
/// faithful path).
pub struct CubicSpline {
    coeff0: Vec<Vector4>,
    coeff1: Vec<Vector4>,
    coeff2: Vec<Vector4>,
    coeff3: Vec<Vector4>,
    n_points: usize,
}

impl CubicSpline {
    /// Solve the spline coefficients for `points`. `None` for an empty input.
    pub fn generate(points: &[Vector4]) -> Option<CubicSpline> {
        let n = points.len();
        if n == 0 {
            return None;
        }
        let zero = Vector4::new4(0.0, 0.0, 0.0, 0.0);
        let mut c0 = vec![zero; n];
        let mut c1 = vec![zero; n];
        let mut c2 = vec![zero; n];
        let mut c3 = vec![zero; n];

        if n == 1 {
            c0[0] = points[0];
        } else if n == 2 {
            // Degenerate (line): a single linear segment.
            c0[0] = points[0];
            c1[0] = points[1].sub_vec(points[0]);
        } else {
            let int_no = n - 1; // number of intervals
            let equ_no = int_no - 1; // number of interior equations

            // interval sizes (chord lengths) and their reciprocals
            let mut h = vec![0.0; int_no];
            let mut ih = vec![0.0; int_no];
            for i in 0..int_no {
                h[i] = points[i].sub_vec(points[i + 1]).length3d();
                ih[i] = 1.0 / h[i];
            }

            // diagonal of the tridiagonal matrix
            let mut a = vec![0.0; equ_no];
            for i in 0..equ_no {
                a[i] = 2.0 * (h[i] + h[i + 1]);
            }

            // LR (tridiagonal) decomposition
            let mut m = vec![0.0; equ_no];
            let mut l = vec![0.0; equ_no.saturating_sub(1)];
            m[0] = a[0];
            for i in 0..equ_no.saturating_sub(1) {
                l[i] = h[i + 1] / m[i];
                m[i + 1] = a[i + 1] - l[i] * h[i + 1];
            }

            // right-hand side: 6 (ih_i (p_{i+1}-p_i) - ih_{i+1} (p_{i+2}-p_{i+1}))
            let mut dvec = vec![zero; equ_no];
            for i in 0..equ_no {
                let dif1 = points[i + 1].sub_vec(points[i]);
                let dif2 = points[i + 2].sub_vec(points[i + 1]);
                dvec[i] = dif1.scale(ih[i]).sub_vec(dif2.scale(ih[i + 1])).scale(6.0);
            }

            // forward elimination
            let mut yvec = vec![zero; equ_no];
            yvec[0] = dvec[0];
            for i in 1..equ_no {
                yvec[i] = dvec[i].sub_vec(yvec[i - 1].scale(l[i - 1]));
            }

            // back substitution -> second derivatives at the interior knots
            let mut xvec = vec![zero; equ_no];
            xvec[equ_no - 1] = yvec[equ_no - 1].scale(-1.0 / m[equ_no - 1]);
            for i in (0..equ_no - 1).rev() {
                xvec[i] = yvec[i]
                    .add_vec(xvec[i + 1].scale(h[i + 1]))
                    .scale(-1.0 / m[i]);
            }

            // assemble the per-interval cubic coefficients (local parameter [0,1])
            for i in 0..int_no {
                let d0 = if i == 0 { zero } else { xvec[i - 1] };
                let d1 = if i == int_no - 1 { zero } else { xvec[i] };
                let hsq = h[i] * h[i];
                c3[i] = d1.sub_vec(d0).scale(hsq / 6.0);
                c2[i] = d0.scale(0.5 * hsq);
                c1[i] = points[i + 1]
                    .sub_vec(points[i])
                    .sub_vec(d1.add_vec(d0.scale(2.0)).scale(hsq / 6.0));
                c0[i] = points[i];
            }
        }

        Some(CubicSpline {
            coeff0: c0,
            coeff1: c1,
            coeff2: c2,
            coeff3: c3,
            n_points: n,
        })
    }

    pub fn point_count(&self) -> usize {
        self.n_points
    }

    /// Evaluate the spline at `par` in `[0, n-1]`, returning position, tangent and
    /// curvature. The interval is `floor(par)` (clamped) and the polynomial uses
    /// the fractional part as its local parameter.
    pub fn interpolate(&self, par: f64) -> SplineSample {
        let max_interval = self.n_points.saturating_sub(2) as isize;
        let ncoeff = (par.floor() as isize).clamp(0, max_interval.max(0)) as usize;
        let c0 = self.coeff0[ncoeff];
        let c1 = self.coeff1[ncoeff];
        let c2 = self.coeff2[ncoeff];
        let c3 = self.coeff3[ncoeff];
        let f = par - ncoeff as f64;

        let position = c3
            .scale(f)
            .add_vec(c2)
            .scale(f)
            .add_vec(c1)
            .scale(f)
            .add_vec(c0);
        let tangent = c3
            .scale(3.0 * f)
            .add_vec(c2.scale(2.0))
            .scale(f)
            .add_vec(c1);
        let curvature = c3.scale(6.0 * f).add_vec(c2.scale(2.0));
        SplineSample {
            position,
            tangent,
            curvature,
        }
    }
}

/// Sample a natural cubic spline through `points`, with `subdiv` samples per
/// segment (count `(n-1)*subdiv + 1`, ending exactly on the last control point).
/// The spline passes through every control point, so the curve runs along the
/// backbone atoms while smoothing the path between them.
fn cubic_spline_samples(points: &[Vector4], subdiv: usize) -> Vec<Vector4> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let subdiv = subdiv.max(1);
    let n = points.len();
    let Some(spline) = CubicSpline::generate(points) else {
        return points.to_vec();
    };
    let mut out = Vec::with_capacity((n - 1) * subdiv + 1);
    for i in 0..n - 1 {
        for step in 0..subdiv {
            let par = i as f64 + step as f64 / subdiv as f64;
            out.push(spline.interpolate(par).position);
        }
    }
    out.push(points[n - 1]);
    out
}

/// Sample a PIECEWISE-LINEAR path through `points` (straight segments between
/// consecutive control points), with `subdiv` samples per segment — the same
/// count and parametrization as [`cubic_spline_samples`], so it is a drop-in
/// replacement, but every intermediate sample lies exactly on the chord. This is
/// the C++ `TraceRenderer` Cα line strip (the angular trace) vs the splined tube.
fn linear_samples(points: &[Vector4], subdiv: usize) -> Vec<Vector4> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let subdiv = subdiv.max(1);
    let n = points.len();
    let mut out = Vec::with_capacity((n - 1) * subdiv + 1);
    for i in 0..n - 1 {
        for step in 0..subdiv {
            let t = step as f64 / subdiv as f64;
            out.push(points[i].scale(1.0 - t) + points[i + 1].scale(t));
        }
    }
    out.push(points[n - 1]);
    out
}

/// Build a secondary-structure cartoon snapshot: a ribbon swept along each
/// chain's splined backbone, shaped per residue (helix/sheet = wide flat band,
/// coil = thin tube) and colored by secondary structure.
pub fn ribbon_mesh_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
) -> RenderSceneSnapshot {
    // C++ Ribbon/Spline `axialdetail`: spline subdivisions per residue (default 8).
    let subdiv = options.ribbon_axial_detail.unwrap_or(8).max(1) as usize;
    let scale = ((options.bond_radius * 4.0).clamp(0.25, 0.8)) as f64;
    let draw = options.draw_indices.as_ref().map(|indices| {
        indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
    });
    let chains = collect_mainchain_pivots(molecule, draw.as_ref(), options.pivot_atom.as_deref());
    // Putty (TubeRenderer): modulate the ribbon width per residue by the B-factor/
    // occupancy, on top of the section width scales. Stats are over all pivots.
    let putty = options.putty;
    let (putty_lo, putty_hi, putty_aver) = putty_value_stats(&chains, &putty);
    // Object paint coloring (resolved against the X11 table) colours the backbone
    // where the renderer defers to $molcol; built once when the scene paints.
    let paint_styles = (!options.object_paint.is_empty()).then(default_style_manager);
    // BfacColoring: auto-scale to the molecule's B-factor range once up front.
    let bfac_coloring = options.bfac.clone().map(|mut bfac| {
        bfac.start_molecule(molecule);
        bfac
    });
    let backbone_atom_colors = (options.backbone_atom_colors || options.script_coloring.is_some())
        .then(|| resolve_atom_colors(molecule, options));
    let cartoon_anchor_atoms = options
        .cartoon_anchor_selection
        .as_deref()
        .and_then(|selection| parse_selector(selection).ok())
        .map(|selection| {
            molecule
                .select_indices(&selection)
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        });
    let rainbow_colors = options
        .rainbow
        .as_ref()
        .map(|rainbow| rainbow_atom_colors(molecule, rainbow));
    let mut meshes = Vec::new();
    for pivots in &chains {
        if pivots.len() < 2 {
            continue;
        }
        let (samples, sections, mut width_scales) = sample_backbone(
            pivots,
            subdiv,
            &BackboneSamplingOptions {
                anchor_weight: options.anchor_weight,
                section_smoothing: options.ribbon_smoothing,
                cartoon_anchor_atoms: cartoon_anchor_atoms.as_ref(),
                cartoon_anchor_weight: options.cartoon_anchor_weight,
                linear: options.linear_backbone,
                uniform_section: options.uniform_backbone,
                helix_head: options.helix_head,
                helix_tail: options.helix_tail,
                sheet_head: options.sheet_head,
                sheet_tail: options.sheet_tail,
            },
        );
        if putty.mode != PuttyMode::Off {
            apply_putty_to_width_scales(
                &mut width_scales,
                pivots,
                subdiv,
                &putty,
                putty_lo,
                putty_hi,
                putty_aver,
            );
        }
        // Renderer coloring takes precedence, followed by the object scheme.
        let mut paint = if let Some(property) = options.atom_color_property.as_deref() {
            Some(ribbon_atom_prop_colors(
                samples.len(),
                pivots,
                subdiv,
                property,
                molecule,
            ))
        } else if let Some(atom_colors) = &rainbow_colors {
            Some(ribbon_rainbow_colors(
                samples.len(),
                pivots,
                subdiv,
                atom_colors,
            ))
        } else if let Some(bfac) = &bfac_coloring {
            Some(ribbon_bfac_colors(
                samples.len(),
                pivots,
                subdiv,
                bfac,
                molecule,
            ))
        } else if let Some(styles) = paint_styles.as_ref() {
            Some(ribbon_paint_colors(
                samples.len(),
                pivots,
                subdiv,
                &options.object_paint,
                molecule,
                styles,
            ))
        } else {
            backbone_atom_colors
                .as_ref()
                .map(|colors| ribbon_resolved_atom_colors(samples.len(), pivots, subdiv, colors))
        };
        if options.smooth_color {
            if let Some(colors) = &mut paint {
                interpolate_ribbon_colors(colors, pivots.len(), subdiv);
            }
        }
        let mesh_options = RibbonTubeMeshOptions {
            scale,
            sections: &options.ribbon_sections,
            styled: &options.styled_ribbon,
            paint: paint.as_deref(),
            radial_detail: options
                .ribbon_radial_detail
                .unwrap_or(TUBE_RADIAL as u32)
                .max(3) as usize,
            start_cap: options.start_cap_type,
            end_cap: options.end_cap_type,
            segment_end_fade: options.segment_end_fade,
            helix_back_color: options.helix_back_color,
            sheet_side_color: options.sheet_side_color,
        };
        let mesh = ribbon_tube_mesh(&samples, &sections, &width_scales, &mesh_options);
        if !mesh.vertices.is_empty() {
            meshes.push(mesh);
        }
    }
    let bounds = match &draw {
        None => molecule.bounding_box_for(None),
        Some(set) => bounds_of_atoms(molecule, set),
    };
    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds: Vec::new(),
        meshes,
        transparency_groups: Vec::new(),
        bounds,
    }
}

/// Spline the backbone positions and carry a per-sample secondary structure.
///
/// A spline segment between two residues keeps its class only when both ends
/// agree; mixed boundaries fall back to coil so the cross-section tapers
/// smoothly across secondary-structure transitions.
struct BackboneSamplingOptions<'a> {
    anchor_weight: f64,
    section_smoothing: RibbonSectionValues,
    cartoon_anchor_atoms: Option<&'a std::collections::BTreeSet<usize>>,
    cartoon_anchor_weight: f64,
    linear: bool,
    uniform_section: bool,
    helix_head: RibbonJunction,
    helix_tail: RibbonJunction,
    sheet_head: RibbonJunction,
    sheet_tail: RibbonJunction,
}

fn sample_backbone(
    points: &[MainchainPoint],
    subdiv: usize,
    options: &BackboneSamplingOptions<'_>,
) -> (Vec<Vector4>, Vec<Secondary>, Vec<f64>) {
    let mut positions = points.iter().map(|point| point.pos).collect::<Vec<_>>();
    // Anchor-weight smoothing (C++ Ribbon2/SplineCoeffSet): blend each interior
    // control point toward the midpoint of its neighbours before splining. The
    // angular `trace` (linear path) keeps the raw pivots — no pre-smoothing.
    if !options.linear && positions.len() >= 3 {
        let orig = positions.clone();
        for i in 1..orig.len() - 1 {
            let section_value = options.section_smoothing.for_ss(points[i].ss);
            let mut w = if options.anchor_weight > 1.0e-9 {
                options.anchor_weight.clamp(0.0, 1.0)
            } else if section_value > 0.0 {
                section_value / (section_value + 10.0)
            } else {
                0.0
            };
            if options
                .cartoon_anchor_atoms
                .is_some_and(|atoms| atoms.contains(&points[i].atom_index))
            {
                w /= options.cartoon_anchor_weight.max(1.0);
            }
            let mid = (orig[i - 1] + orig[i + 1]).scale(0.5);
            positions[i] = orig[i].scale(1.0 - w) + mid.scale(w);
        }
    }
    // `trace` runs straight between Cα pivots (C++ TraceRenderer line strip); every
    // other main-chain type smooths the path with a natural cubic spline.
    let samples = if options.linear {
        linear_samples(&positions, subdiv)
    } else {
        cubic_spline_samples(&positions, subdiv)
    };
    let subdiv = subdiv.max(1);
    let n = points.len();
    let mut sections = Vec::with_capacity(samples.len());
    for i in 0..n - 1 {
        let kind = merge_ss(points[i].ss, points[i + 1].ss);
        for _ in 0..subdiv {
            sections.push(kind);
        }
    }
    sections.push(points[n - 1].ss);
    // Keep lengths aligned with `samples` regardless of rounding.
    sections.truncate(samples.len());
    while sections.len() < samples.len() {
        sections.push(points[n - 1].ss);
    }
    let width_scales = if options.uniform_section {
        vec![1.0; sections.len()]
    } else {
        ribbon_junction_widths(
            &sections,
            subdiv,
            options.helix_head,
            options.helix_tail,
            options.sheet_head,
            options.sheet_tail,
        )
    };
    (samples, sections, width_scales)
}

fn merge_ss(a: Secondary, b: Secondary) -> Secondary {
    if a == b {
        a
    } else {
        Secondary::Coil
    }
}

/// Per-sample width multipliers that turn the C-terminal end of each sheet run
/// into an arrowhead: the last ~residue widens at its root (1.8x) then tapers to
/// a point (0x) at the tip. All other samples stay at 1.0.
fn ribbon_junction_widths(
    sections: &[Secondary],
    subdiv: usize,
    helix_head: RibbonJunction,
    helix_tail: RibbonJunction,
    sheet_head: RibbonJunction,
    sheet_tail: RibbonJunction,
) -> Vec<f64> {
    let mut widths = vec![1.0; sections.len()];
    let junction_len = subdiv.max(2);
    let count = sections.len();
    let mut i = 0;
    while i < count {
        let section = sections[i];
        if section == Secondary::Coil {
            i += 1;
            continue;
        }
        let start = i;
        while i < count && sections[i] == section {
            i += 1;
        }
        let end = i - 1;
        let (head, tail) = match section {
            Secondary::Helix => (helix_head, helix_tail),
            Secondary::Sheet => (sheet_head, sheet_tail),
            Secondary::Coil => unreachable!(),
        };
        let head_len = junction_len.min(end - start + 1);
        if head.kind == RibbonJunctionType::Arrow {
            let denominator = (head_len - 1).max(1) as f64;
            for k in 0..head_len {
                let progress = (k as f64 / denominator).powf(head.gamma.max(0.1));
                widths[end - k] = head.base_width + (head.arrow_width - head.base_width) * progress;
            }
        }
        let tail_len = junction_len.min(end - start + 1);
        if tail.kind == RibbonJunctionType::Arrow {
            let denominator = (tail_len - 1).max(1) as f64;
            for k in 0..tail_len {
                let progress = (k as f64 / denominator).powf(tail.gamma.max(0.1));
                widths[start + k] =
                    tail.base_width + (tail.arrow_width - tail.base_width) * progress;
            }
        }
    }
    widths
}

#[cfg(test)]
fn sheet_arrow_widths(sections: &[Secondary], subdiv: usize) -> Vec<f64> {
    ribbon_junction_widths(
        sections,
        subdiv,
        RibbonJunction::default(),
        RibbonJunction::default(),
        RibbonJunction {
            kind: RibbonJunctionType::Arrow,
            ..RibbonJunction::default()
        },
        RibbonJunction::default(),
    )
}

/// Cross-section half-width, half-thickness, and color for a secondary-structure
/// class. `scale` is the base coil radius.
fn section_params(
    ss: Secondary,
    scale: f64,
    sec: &RibbonSections,
    styled: &StyledRibbonSections,
) -> (f64, f64, [f32; 4]) {
    let (factor_width, factor_thick, color) = match ss {
        Secondary::Helix => (sec.helix_width, sec.helix_thick, [0.95, 0.32, 0.42, 1.0]),
        Secondary::Sheet => (sec.sheet_width, sec.sheet_thick, [0.95, 0.82, 0.30, 1.0]),
        Secondary::Coil => (sec.coil_width, sec.coil_thick, [0.55, 0.85, 0.90, 1.0]),
    };
    // A style's <helix>/<sheet>/<coil> sub-element gives ABSOLUTE C++ semi-axes
    // (width·tuber, width) that override the scale×factor model for that class.
    match styled.for_ss(ss) {
        Some((base_width, thick)) => (base_width, thick, color),
        None => (scale * factor_width, scale * factor_thick, color),
    }
}

/// A point on a RECTANGULAR cross-section perimeter at fraction `t` in [0,1),
/// returning `(offset from centre, outward normal)` in the (binormal=width,
/// normal=thick) frame. Faithful to C++ `TubeSection::setupRectSection`: four
/// flat sides, each carrying an axis-aligned FACE normal (so the band has sharp
/// edges, unlike the elliptical profile's smooth radial normals).
fn rect_ring_point(
    t: f64,
    width: f64,
    thick: f64,
    binormal: Vector4,
    normal: Vector4,
) -> (Vector4, Vector4) {
    let (w, h) = (width, thick);
    let perim = 4.0 * (w + h);
    let d = (t * perim).rem_euclid(perim);
    // (u along binormal, v along normal, face-normal in the same frame).
    let (u, v, nb, nn) = if d < 2.0 * w {
        (w - d, h, 0.0, 1.0) // top:    (w,h) -> (-w,h),  normal +normal
    } else if d < 2.0 * w + 2.0 * h {
        (-w, h - (d - 2.0 * w), -1.0, 0.0) // left:  (-w,h) -> (-w,-h), normal -binormal
    } else if d < 4.0 * w + 2.0 * h {
        (-w + (d - 2.0 * w - 2.0 * h), -h, 0.0, -1.0) // bottom: normal -normal
    } else {
        (w, -h + (d - 4.0 * w - 2.0 * h), 1.0, 0.0) // right:  normal +binormal
    };
    let offset = binormal.scale(u) + normal.scale(v);
    let nrm = normalize_or(binormal.scale(nb) + normal.scale(nn), normal);
    (offset, nrm)
}

fn superellipse_ring_point(
    t: f64,
    width: f64,
    thick: f64,
    binormal: Vector4,
    normal: Vector4,
    sharp: f64,
    fancy: bool,
) -> (Vector4, Vector4) {
    let angle = t * std::f64::consts::TAU;
    let (sin, cos) = angle.sin_cos();
    let exponent = 2.0 + sharp.clamp(0.0, 1.0) * if fancy { 18.0 } else { 8.0 };
    let power = 2.0 / exponent;
    let u = cos.signum() * cos.abs().powf(power) * width;
    let v = sin.signum() * sin.abs().powf(power) * thick;
    let safe_width = width.abs().max(f64::EPSILON);
    let safe_thick = thick.abs().max(f64::EPSILON);
    let grad_u = u.signum() * (u.abs() / safe_width).powf(exponent - 1.0) / safe_width;
    let grad_v = v.signum() * (v.abs() / safe_thick).powf(exponent - 1.0) / safe_thick;
    let offset = binormal.scale(u) + normal.scale(v);
    let nrm = normalize_or(binormal.scale(grad_u) + normal.scale(grad_v), normal);
    (offset, nrm)
}

#[allow(clippy::too_many_arguments)]
fn append_tube_cap(
    vertices: &mut Vec<MeshVertex>,
    indices: &mut Vec<u32>,
    base_ring: usize,
    radial_detail: usize,
    center: Vector4,
    tangent: Vector4,
    normal: Vector4,
    binormal: Vector4,
    width: f64,
    thick: f64,
    color: [f32; 4],
    shape: SectionShape,
    sharp: f64,
    cap_type: TubeCapType,
    at_start: bool,
) {
    if cap_type == TubeCapType::None {
        return;
    }
    let direction = tangent.scale(if at_start { -1.0 } else { 1.0 });
    let push_fan = |indices: &mut Vec<u32>, center: u32, ring: usize| {
        for index in 0..radial_detail {
            let next = (index + 1) % radial_detail;
            let current = (ring + index) as u32;
            let next = (ring + next) as u32;
            if at_start {
                indices.extend_from_slice(&[center, current, next]);
            } else {
                indices.extend_from_slice(&[center, next, current]);
            }
        }
    };
    if cap_type == TubeCapType::Flat {
        let center_index = vertices.len() as u32;
        vertices.push(MeshVertex {
            position: center,
            normal: direction,
            color,
        });
        push_fan(indices, center_index, base_ring);
        return;
    }

    let ring_point = |t: f64, width: f64, thick: f64| match shape {
        SectionShape::Rectangular => rect_ring_point(t, width, thick, binormal, normal),
        SectionShape::RoundSquare | SectionShape::Fancy => superellipse_ring_point(
            t,
            width,
            thick,
            binormal,
            normal,
            sharp,
            shape == SectionShape::Fancy,
        ),
        SectionShape::Elliptical => {
            let (sin, cos) = (t * std::f64::consts::TAU).sin_cos();
            let offset = binormal.scale(cos * width) + normal.scale(sin * thick);
            let nrm = normalize_or(
                binormal.scale(cos * thick) + normal.scale(sin * width),
                normal,
            );
            (offset, nrm)
        }
    };
    let levels = (radial_detail / 4).clamp(2, 6);
    let cap_depth = (width.abs() * thick.abs()).sqrt().max(f64::EPSILON);
    let mut previous_ring = base_ring;
    for level in 1..levels {
        let phi = std::f64::consts::FRAC_PI_2 * level as f64 / levels as f64;
        let ring_scale = phi.cos();
        let axial = phi.sin();
        let ring_start = vertices.len();
        for index in 0..radial_detail {
            let t = index as f64 / radial_detail as f64;
            let (offset, radial_normal) = ring_point(t, width * ring_scale, thick * ring_scale);
            vertices.push(MeshVertex {
                position: center + direction.scale(cap_depth * axial) + offset,
                normal: normalize_or(
                    radial_normal.scale(ring_scale) + direction.scale(axial),
                    direction,
                ),
                color,
            });
        }
        for index in 0..radial_detail {
            let next = (index + 1) % radial_detail;
            let a = (previous_ring + index) as u32;
            let b = (previous_ring + next) as u32;
            let c = (ring_start + index) as u32;
            let d = (ring_start + next) as u32;
            if at_start {
                indices.extend_from_slice(&[a, b, c, b, d, c]);
            } else {
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
        previous_ring = ring_start;
    }
    let pole = vertices.len() as u32;
    vertices.push(MeshVertex {
        position: center + direction.scale(cap_depth),
        normal: direction,
        color,
    });
    push_fan(indices, pole, previous_ring);
}

/// Historical radial detail used when section.detail is not specified.
const TUBE_RADIAL: usize = 12;

struct RibbonTubeMeshOptions<'a> {
    scale: f64,
    sections: &'a RibbonSections,
    styled: &'a StyledRibbonSections,
    paint: Option<&'a [Option<[f32; 4]>]>,
    radial_detail: usize,
    start_cap: TubeCapType,
    end_cap: TubeCapType,
    segment_end_fade: bool,
    helix_back_color: Option<[f32; 4]>,
    sheet_side_color: Option<[f32; 4]>,
}

/// Sweep a per-sample elliptical cross-section along `samples` into a ribbon
/// mesh. helix/sheet samples get a wide, flat ellipse (a band); coil samples a
/// small circle (a thin tube). `scale` is the base coil radius.
///
/// Cross-section frames are carried with a parallel-transport scheme so the
/// surface does not twist between consecutive rings.
fn ribbon_tube_mesh(
    samples: &[Vector4],
    sections: &[Secondary],
    width_scales: &[f64],
    options: &RibbonTubeMeshOptions<'_>,
) -> RenderMesh {
    let n = samples.len();
    if n < 2 {
        return RenderMesh::new();
    }

    // Tangents via central differences.
    let mut tangents = Vec::with_capacity(n);
    for i in 0..n {
        let prev = samples[i.saturating_sub(1)];
        let next = samples[(i + 1).min(n - 1)];
        tangents.push(normalize_or(next - prev, Vector4::new3(0.0, 0.0, 1.0)));
    }

    // Parallel-transport frame (normal, binormal) along the curve.
    let mut frames = Vec::with_capacity(n);
    let mut normal = perpendicular(tangents[0]);
    for (i, &tangent) in tangents.iter().enumerate() {
        if i > 0 {
            // Project the carried normal back onto the new tangent plane.
            normal = normalize_or(
                normal - tangent.scale(normal.dot3d(tangent)),
                perpendicular(tangent),
            );
        }
        let binormal = normalize_or(tangent.cross(normal), perpendicular(tangent));
        // Re-orthogonalize so the frame stays orthonormal.
        normal = normalize_or(binormal.cross(tangent), normal);
        frames.push((normal, binormal));
    }

    let radial_detail = options.radial_detail.max(3);
    // Ring vertices around the cross-section at each sample, with
    // width along the binormal and thickness along the normal. The profile is an
    // ellipse (smooth) or a rectangle (flat-faced) per the resolved style.
    let mut vertices = Vec::with_capacity(n * radial_detail);
    for (i, &center) in samples.iter().enumerate() {
        let (normal, binormal) = frames[i];
        let (base_width, thick, section_color) =
            section_params(sections[i], options.scale, options.sections, options.styled);
        // Object paint coloring (when resolved for this sample) overrides the
        // secondary-structure section colour — the $molcol→object cascade.
        let color = options
            .paint
            .and_then(|colors| colors.get(i).copied().flatten())
            .unwrap_or(section_color);
        let fade = if options.segment_end_fade {
            let fade_span = (n / 10).clamp(1, 8);
            let edge_distance = i.min(n - 1 - i);
            (edge_distance as f32 / fade_span as f32).min(1.0)
        } else {
            1.0
        };
        let width = base_width * width_scales[i];
        let shape = options.styled.shape_for(sections[i]);
        let section_sharp = options.styled.sharp_for(sections[i]);
        for j in 0..radial_detail {
            let t = (j as f64) / (radial_detail as f64);
            let (offset, nrm) = match shape {
                SectionShape::Rectangular => rect_ring_point(t, width, thick, binormal, normal),
                SectionShape::RoundSquare | SectionShape::Fancy => superellipse_ring_point(
                    t,
                    width,
                    thick,
                    binormal,
                    normal,
                    section_sharp,
                    shape == SectionShape::Fancy,
                ),
                SectionShape::Elliptical => {
                    let (sin, cos) = (t * std::f64::consts::TAU).sin_cos();
                    let offset = binormal.scale(cos * width) + normal.scale(sin * thick);
                    // Outward ellipse normal: gradient of (u/width)^2 + (v/thick)^2.
                    let nrm = normalize_or(
                        binormal.scale(cos * thick) + normal.scale(sin * width),
                        normal,
                    );
                    (offset, nrm)
                }
            };
            let mut vertex_color = color;
            if sections[i] == Secondary::Helix && offset.dot3d(normal) < 0.0 {
                if let Some(back_color) = options.helix_back_color {
                    vertex_color = back_color;
                }
            } else if sections[i] == Secondary::Sheet && nrm.dot3d(binormal).abs() > 0.7 {
                if let Some(side_color) = options.sheet_side_color {
                    vertex_color = side_color;
                }
            }
            vertex_color[3] *= fade;
            vertices.push(MeshVertex {
                position: center + offset,
                normal: nrm,
                color: vertex_color,
            });
        }
    }

    // Triangle indices stitching consecutive rings into quads.
    let mut indices = Vec::with_capacity((n - 1) * radial_detail * 6);
    for i in 0..n - 1 {
        for j in 0..radial_detail {
            let j_next = (j + 1) % radial_detail;
            let a = (i * radial_detail + j) as u32;
            let b = (i * radial_detail + j_next) as u32;
            let c = ((i + 1) * radial_detail + j) as u32;
            let d = ((i + 1) * radial_detail + j_next) as u32;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    if !options.segment_end_fade {
        let append_cap = |at_start: bool,
                          cap_type: TubeCapType,
                          vertices: &mut Vec<MeshVertex>,
                          indices: &mut Vec<u32>| {
            let sample_index = if at_start { 0 } else { n - 1 };
            let (normal, binormal) = frames[sample_index];
            let (base_width, thick, section_color) = section_params(
                sections[sample_index],
                options.scale,
                options.sections,
                options.styled,
            );
            let color = options
                .paint
                .and_then(|colors| colors.get(sample_index).copied().flatten())
                .unwrap_or(section_color);
            append_tube_cap(
                vertices,
                indices,
                sample_index * radial_detail,
                radial_detail,
                samples[sample_index],
                tangents[sample_index],
                normal,
                binormal,
                base_width * width_scales[sample_index],
                thick,
                color,
                options.styled.shape_for(sections[sample_index]),
                options.styled.sharp_for(sections[sample_index]),
                cap_type,
                at_start,
            );
        };
        append_cap(true, options.start_cap, &mut vertices, &mut indices);
        append_cap(false, options.end_cap, &mut vertices, &mut indices);
    }

    RenderMesh { vertices, indices }
}

/// Aromatic/saturated ring fills using the C++ BallStick defaults.
pub fn ring_meshes_snapshot(molecule: &MolCoord) -> RenderSceneSnapshot {
    ring_meshes_snapshot_with(molecule, 0.2, [1.0_f32, 1.0, 0.75, 1.0], None)
}

/// Aromatic/saturated ring fills (C++ `BallStickRenderer::drawRings`): for each
/// complete selected ring, draw two triangle fans offset from the ring plane by
/// `thickness`, using the resolved renderer ring color.
pub fn ring_meshes_snapshot_with(
    molecule: &MolCoord,
    thickness: f64,
    ring_color: [f32; 4],
    draw_indices: Option<&[usize]>,
) -> RenderSceneSnapshot {
    let db = cuemol_molstr::toppar::TopoDB::global();
    let summaries = molecule.residue_summaries();
    let draw = draw_indices.map(|indices| {
        indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
    });
    let mut meshes = Vec::new();
    for s in &summaries {
        let rings = db.rings(&s.res_name);
        if rings.is_empty() {
            continue;
        }
        let atom = |name: &str| -> Option<(usize, Vector4)> {
            s.atom_indices
                .iter()
                .find(|&&ai| molecule.atoms()[ai].name.trim().eq_ignore_ascii_case(name))
                .map(|&ai| (ai, molecule.atoms()[ai].position))
        };
        for ring in rings {
            let members: Vec<(usize, Vector4)> =
                ring.iter().filter_map(|name| atom(name)).collect();
            if members.len() != ring.len()
                || members.len() < 3
                || draw
                    .as_ref()
                    .is_some_and(|draw| members.iter().any(|(index, _)| !draw.contains(index)))
            {
                continue;
            }
            let verts = members
                .iter()
                .map(|(_, position)| *position)
                .collect::<Vec<_>>();
            let mut centroid = Vector4::new3(0.0, 0.0, 0.0);
            for v in &verts {
                centroid = centroid + *v;
            }
            centroid = centroid.scale(1.0 / verts.len() as f64);
            let normal = normalize_or(
                (verts[1] - verts[0]).cross(verts[2] - verts[0]),
                Vector4::new3(0.0, 0.0, 1.0),
            );
            let offset = normal.scale(thickness.max(0.0));
            let mut mesh = RenderMesh::new();
            mesh.vertices.push(MeshVertex {
                position: centroid + offset,
                normal,
                color: ring_color,
            });
            for v in &verts {
                mesh.vertices.push(MeshVertex {
                    position: *v + offset,
                    normal,
                    color: ring_color,
                });
            }
            let n = verts.len() as u32;
            for k in 0..n {
                mesh.indices.extend_from_slice(&[0, 1 + k, 1 + (k + 1) % n]);
            }
            let bottom = mesh.vertices.len() as u32;
            mesh.vertices.push(MeshVertex {
                position: centroid - offset,
                normal: -normal,
                color: ring_color,
            });
            for v in &verts {
                mesh.vertices.push(MeshVertex {
                    position: *v - offset,
                    normal: -normal,
                    color: ring_color,
                });
            }
            for k in 0..n {
                mesh.indices
                    .extend_from_slice(&[bottom, bottom + 1 + (k + 1) % n, bottom + 1 + k]);
            }
            meshes.push(mesh);
        }
    }
    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds: Vec::new(),
        meshes,
        transparency_groups: Vec::new(),
        bounds: molecule.bounding_box_for(None),
    }
}

/// Optional MDL/SDF stereobond wedge geometry. This helper is independent of
/// CueMol's `BallStickRenderer::VBMODE_TYPE1`, which renders multiple bonds as
/// parallel cylinders. Stereo flag 1 emits a solid wedge and flag 6 emits a
/// sequence of widening hash bars.
pub fn valence_wedge_meshes_snapshot(molecule: &MolCoord, width: f64) -> RenderSceneSnapshot {
    let atoms = molecule.atoms();
    let mut meshes = Vec::new();
    for bond in molecule.bonds() {
        if bond.stereo != 1 && bond.stereo != 6 {
            continue;
        }
        let (Some(a1), Some(a2)) = (atoms.get(bond.atom1), atoms.get(bond.atom2)) else {
            continue;
        };
        let (apex, wide) = (a1.position, a2.position);
        let axis = wide - apex;
        let perp = perpendicular(axis);
        let half = perp.scale(width * 0.5);
        let normal = normalize_or(axis.cross(perp), Vector4::new3(0.0, 0.0, 1.0));
        let color = [0.45_f32, 0.45, 0.45, 1.0];
        let mut mesh = RenderMesh::new();
        if bond.stereo == 1 {
            for position in [apex, wide + half, wide - half] {
                mesh.vertices.push(MeshVertex {
                    position,
                    normal,
                    color,
                });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2]);
        } else {
            let axis_unit = normalize_or(axis, Vector4::new3(1.0, 0.0, 0.0));
            let half_thickness = axis_unit.scale(width * 0.025);
            for index in 0..6 {
                let fraction = (index as f64 + 1.0) / 7.0;
                let center = apex + axis.scale(fraction);
                let bar_half = perp.scale(width * fraction * 0.5);
                let base = mesh.vertices.len() as u32;
                for position in [
                    center - bar_half - half_thickness,
                    center + bar_half - half_thickness,
                    center + bar_half + half_thickness,
                    center - bar_half + half_thickness,
                ] {
                    mesh.vertices.push(MeshVertex {
                        position,
                        normal,
                        color,
                    });
                }
                mesh.indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base,
                    base + 2,
                    base + 3,
                ]);
            }
        }
        meshes.push(mesh);
    }
    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds: Vec::new(),
        meshes,
        transparency_groups: Vec::new(),
        bounds: molecule.bounding_box_for(None),
    }
}

fn normalize_or(vec: Vector4, fallback: Vector4) -> Vector4 {
    vec.normalize3d().unwrap_or(fallback)
}

fn cubic_bezier_point(p1: Vector4, p2: Vector4, p3: Vector4, p4: Vector4, t: f64) -> Vector4 {
    let it = 1.0 - t;
    p1 * (it * it * it) + p2 * (3.0 * t * it * it) + p3 * (3.0 * t * t * it) + p4 * (t * t * t)
}

fn cubic_bezier_derivative_length(
    p1: Vector4,
    p2: Vector4,
    p3: Vector4,
    p4: Vector4,
    t: f64,
) -> f64 {
    let it = 1.0 - t;
    ((p4 - p3).scale(3.0 * t * t) + (p3 - p2).scale(6.0 * t * it) + (p2 - p1).scale(3.0 * it * it))
        .length3d()
}

/// Dots spaced (by arc length) `dot_sep` apart along the cubic Bézier through
/// `p1..p4`. Port of C++ `DisoRenderer::renderBezierDots` (accumulate the
/// differential length, emit a dot each time it crosses a `dot_sep` threshold).
fn bezier_dots(
    p1: Vector4,
    p2: Vector4,
    p3: Vector4,
    p4: Vector4,
    dot_sep: f64,
) -> Vec<(Vector4, f64)> {
    if !dot_sep.is_finite() || dot_sep <= 0.0 {
        return Vec::new();
    }
    let estimated_length = (p1 - p2).length3d() + (p2 - p3).length3d() + (p3 - p4).length3d();
    let ndiv = ((estimated_length.floor() as usize).saturating_mul(10)).max(100);
    let delta = 1.0 / ndiv as f64;
    let total_length = (1..=ndiv)
        .map(|index| {
            cubic_bezier_derivative_length(p1, p2, p3, p4, index as f64 / ndiv as f64) * delta
        })
        .sum::<f64>();
    if !total_length.is_finite() || total_length <= 0.0 {
        return Vec::new();
    }
    let dot_count = (total_length / dot_sep).ceil().max(1.0) as usize;
    let actual_separation = total_length / dot_count as f64;
    let mut dots = Vec::new();
    let mut len = 0.0;
    let mut threshold = actual_separation;
    for i in 1..ndiv {
        let t = i as f64 / ndiv as f64;
        len += cubic_bezier_derivative_length(p1, p2, p3, p4, t) * delta;
        if len > threshold {
            threshold += actual_separation;
            dots.push((cubic_bezier_point(p1, p2, p3, p4, t), t));
        }
    }
    dots
}

/// Disorder representation (C++ `DisoRenderer`): a dotted Bézier line bridging
/// each chain gap — consecutive same-chain residues that are NOT bonded to each
/// other (a missing/disordered stretch). The curve endpoints and tangents come
/// from the target main-chain renderer's spline at the adjacent residues.
pub fn disorder_lines_snapshot(
    molecule: &MolCoord,
    dot_sep: f64,
    dot_radius: f64,
    loopsize: f64,
    loopsize2: f64,
) -> RenderSceneSnapshot {
    disorder_lines_snapshot_with_style(
        molecule,
        &DisorderRenderOptions {
            dot_sep,
            dot_radius,
            loopsize,
            loopsize2,
            ..DisorderRenderOptions::default()
        },
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisorderRenderOptions {
    pub dot_sep: f64,
    pub dot_radius: f64,
    pub loopsize: f64,
    pub loopsize2: f64,
    pub detail: Option<u32>,
    pub color: Option<ColorSpec>,
    /// Per-atom colors resolved from the disorder renderer's coloring scheme.
    pub atom_colors: Option<Vec<ColorSpec>>,
    /// Pivot atom inherited from the target MainChainRenderer.
    pub pivot_atom: Option<String>,
    /// Target SplineRenderer axis smoothing.
    pub smoothing: f64,
    pub draw_indices: Option<Vec<usize>>,
}

impl Default for DisorderRenderOptions {
    fn default() -> Self {
        Self {
            dot_sep: 1.0,
            dot_radius: 0.3,
            loopsize: 2.0,
            loopsize2: -1.0,
            detail: Some(5),
            color: None,
            atom_colors: None,
            pivot_atom: None,
            smoothing: 0.0,
            draw_indices: None,
        }
    }
}

/// Styled disorder representation with renderer tessellation, color, and
/// selection applied to the generated dots.
pub fn disorder_lines_snapshot_with_style(
    molecule: &MolCoord,
    options: &DisorderRenderOptions,
) -> RenderSceneSnapshot {
    // C++ DisoRenderer: loopsize scales the N-terminal Bézier tangent (gap
    // bulge amplitude); loopsize2 the C-terminal one, with a non-positive
    // loopsize2 meaning "reuse loopsize".
    let loop1 = options.loopsize;
    let loop2 = if options.loopsize2 > 0.0 {
        options.loopsize2
    } else {
        options.loopsize
    };
    use std::collections::{HashMap, HashSet};
    let summaries = molecule.residue_summaries();
    let draw = options
        .draw_indices
        .as_ref()
        .map(|indices| indices.iter().copied().collect::<HashSet<_>>());

    // atom index -> residue index (position in `summaries`)
    let mut atom_res: HashMap<usize, usize> = HashMap::new();
    for (ri, s) in summaries.iter().enumerate() {
        for &ai in &s.atom_indices {
            atom_res.insert(ai, ri);
        }
    }
    // residue pairs joined by any inter-residue bond (= "linked", not disordered)
    let mut linked: HashSet<(usize, usize)> = HashSet::new();
    for bond in molecule.bonds() {
        if let (Some(&r1), Some(&r2)) = (atom_res.get(&bond.atom1), atom_res.get(&bond.atom2)) {
            if r1 != r2 {
                linked.insert((r1.min(r2), r1.max(r2)));
            }
        }
    }
    let pivot = |ri: usize| -> Option<(usize, Vector4)> {
        let s = &summaries[ri];
        let configured = options
            .pivot_atom
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .and_then(|name| {
                s.atom_indices
                    .iter()
                    .find(|&&ai| molecule.atoms()[ai].name.trim().eq_ignore_ascii_case(name))
                    .copied()
            });
        let topology = cuemol_molstr::toppar::TopoDB::global()
            .pivot_atom(&s.res_name)
            .and_then(|name| {
                s.atom_indices
                    .iter()
                    .find(|&&ai| molecule.atoms()[ai].name.trim().eq_ignore_ascii_case(name))
                    .copied()
            });
        if let Some(ai) = configured.or(topology) {
            return Some((ai, molecule.atoms()[ai].position));
        }
        for name in ["CA", "P", "C1'", "C1*", "O5'"] {
            if let Some(&ai) = s
                .atom_indices
                .iter()
                .find(|&&ai| molecule.atoms()[ai].name.trim().eq_ignore_ascii_case(name))
            {
                return Some((ai, molecule.atoms()[ai].position));
            }
        }
        None
    };
    let same_chain = |a: usize, b: usize| summaries[a].key.chain_id == summaries[b].key.chain_id;

    let pivots = (0..summaries.len()).map(pivot).collect::<Vec<_>>();
    let mut diff_vectors = vec![None; summaries.len()];
    let finish_run = |run: &mut Vec<usize>, diff_vectors: &mut Vec<Option<(Vector4, Vector4)>>| {
        if run.len() >= 3 {
            let mut points = run
                .iter()
                .filter_map(|&index| pivots[index].map(|(_, point)| point))
                .collect::<Vec<_>>();
            let smoothing = options.smoothing.clamp(0.0, 1.0);
            if smoothing > 0.0 {
                let original = points.clone();
                for index in 1..original.len() - 1 {
                    let midpoint = (original[index - 1] + original[index + 1]).scale(0.5);
                    points[index] =
                        original[index].scale(1.0 - smoothing) + midpoint.scale(smoothing);
                }
            }
            if let Some(spline) = CubicSpline::generate(&points) {
                for (offset, &residue_index) in run.iter().enumerate() {
                    let sample = spline.interpolate(offset as f64);
                    diff_vectors[residue_index] = Some((sample.position, sample.tangent));
                }
            }
        }
        run.clear();
    };
    let mut run = Vec::new();
    for (index, pivot_point) in pivots.iter().enumerate() {
        if pivot_point.is_none() {
            finish_run(&mut run, &mut diff_vectors);
            continue;
        }
        let continues = run.last().is_some_and(|&previous| {
            same_chain(previous, index)
                && linked.contains(&(previous.min(index), previous.max(index)))
        });
        if !continues {
            finish_run(&mut run, &mut diff_vectors);
        }
        run.push(index);
    }
    finish_run(&mut run, &mut diff_vectors);

    let mut atoms = Vec::new();
    for i in 1..summaries.len() {
        if !same_chain(i - 1, i) || linked.contains(&(i - 1, i)) {
            continue;
        }
        let (Some((pivot1, _)), Some((pivot2, _))) = (pivots[i - 1], pivots[i]) else {
            continue;
        };
        if draw.as_ref().is_some_and(|draw| {
            !summaries[i - 1]
                .atom_indices
                .iter()
                .any(|index| draw.contains(index))
                || !summaries[i]
                    .atom_indices
                    .iter()
                    .any(|index| draw.contains(index))
        }) {
            continue;
        }
        let (Some((p1, tan1)), Some((p4, tan4))) = (diff_vectors[i - 1], diff_vectors[i]) else {
            continue;
        };
        let p2 = p1 + tan1 * loop1;
        let p3 = p4 - tan4 * loop2;
        let endpoint_color = |atom_index: usize| {
            options
                .atom_colors
                .as_ref()
                .and_then(|colors| colors.get(atom_index))
                .cloned()
                .or_else(|| options.color.clone())
        };
        let color1 = endpoint_color(pivot1);
        let color2 = endpoint_color(pivot2);
        for (dot, parameter) in bezier_dots(p1, p2, p3, p4, options.dot_sep) {
            let color = match (&color1, &color2) {
                (Some(color1), Some(color2)) => Some(interpolate_color(color1, color2, parameter)),
                (Some(color), None) | (None, Some(color)) => Some(color.clone()),
                (None, None) => None,
            };
            atoms.push(
                RenderAtom::new(dot, options.dot_radius as f32, color).with_detail(options.detail),
            );
        }
    }
    RenderSceneSnapshot {
        atoms,
        bonds: Vec::new(),
        meshes: Vec::new(),
        transparency_groups: Vec::new(),
        bounds: molecule.bounding_box_for(None),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NARenderOptions {
    pub base_size: f64,
    pub base_thick: f64,
    pub base_detail: i32,
    pub base_type: i32,
    pub show_basepair: bool,
}

impl Default for NARenderOptions {
    fn default() -> Self {
        Self {
            base_size: 0.5,
            base_thick: 0.5,
            base_detail: 3,
            base_type: NARenderer::NAREND_BP,
            show_basepair: true,
        }
    }
}

/// Compatibility entry point for the C++ `NAREND_SIMPLE1` representation.
pub fn na_base_sticks_snapshot(
    molecule: &MolCoord,
    radius: f64,
    show_basepair: bool,
) -> RenderSceneSnapshot {
    na_renderer_snapshot(
        molecule,
        &NARenderOptions {
            base_size: radius,
            base_type: NARenderer::NAREND_SIMPLE1,
            show_basepair,
            ..NARenderOptions::default()
        },
    )
}

/// Nucleic-acid representation matching all four C++ `NARenderer` modes.
pub fn na_renderer_snapshot(molecule: &MolCoord, options: &NARenderOptions) -> RenderSceneSnapshot {
    use std::collections::{HashMap, HashSet};

    let summaries = molecule.residue_summaries();
    let atom_in = |indices: &[usize], name: &str| -> Option<(usize, Vector4)> {
        indices
            .iter()
            .find(|&&ai| molecule.atoms()[ai].name.trim().eq_ignore_ascii_case(name))
            .map(|&ai| (ai, molecule.atoms()[ai].position))
    };
    let base_kind = |summary: &cuemol_molstr::ResidueSummary| -> Option<&'static str> {
        let dynamic = molecule
            .get_residue_prop_str(
                &summary.key.chain_id,
                &summary.key.script_index(),
                "basetype",
            )
            .ok()
            .flatten();
        let value = dynamic.or_else(|| {
            cuemol_molstr::toppar::TopoDB::global().residue_property(&summary.res_name, "basetype")
        });
        match value {
            Some("pur") => Some("pur"),
            Some("pyr") => Some("pyr"),
            _ if atom_in(&summary.atom_indices, "N9").is_some() => Some("pur"),
            _ if atom_in(&summary.atom_indices, "N1").is_some() => Some("pyr"),
            _ => None,
        }
    };
    let pivot_in = |summary: &cuemol_molstr::ResidueSummary| -> Option<(usize, Vector4)> {
        let configured = cuemol_molstr::toppar::TopoDB::global()
            .pivot_atom(&summary.res_name)
            .and_then(|name| atom_in(&summary.atom_indices, name));
        configured
            .or_else(|| atom_in(&summary.atom_indices, "P"))
            .or_else(|| atom_in(&summary.atom_indices, "C1'"))
            .or_else(|| atom_in(&summary.atom_indices, "C1*"))
    };

    let mut residue_lookup = HashMap::new();
    for (index, summary) in summaries.iter().enumerate() {
        residue_lookup.insert(
            (summary.key.chain_id.clone(), summary.key.script_index()),
            index,
        );
    }
    let partner_of = |index: usize| -> Option<usize> {
        let summary = &summaries[index];
        if let Some(value) = molecule
            .get_residue_prop_str(
                &summary.key.chain_id,
                &summary.key.script_index(),
                "basepair",
            )
            .ok()
            .flatten()
        {
            let (chain, residue) = value.split_once('.')?;
            if let Some(&partner) = residue_lookup.get(&(chain.to_string(), residue.to_string())) {
                return Some(partner);
            }
        }
        let partner_resid = summary
            .atom_indices
            .iter()
            .find_map(|&ai| molecule.atoms()[ai].int_prop("base_pair"))?;
        summaries.iter().enumerate().find_map(|(candidate, other)| {
            (candidate != index && other.key.res_id == partner_resid).then_some(candidate)
        })
    };

    let mut bonds = Vec::new();
    let mut atoms = Vec::new();
    let mut meshes = Vec::new();
    let radius = options.base_size.max(0.0) as f32;
    let detail = Some(options.base_detail.max(0) as u32);
    let mut rendered = HashSet::new();

    for (index, summary) in summaries.iter().enumerate() {
        if rendered.contains(&index) {
            continue;
        }
        let Some(kind) = base_kind(summary) else {
            continue;
        };
        let Some((pivot_index, pivot)) = pivot_in(summary) else {
            continue;
        };
        let glyco_name = if kind == "pur" { "N9" } else { "N1" };
        let far_name = if kind == "pur" { "N1" } else { "N3" };

        match options.base_type {
            NARenderer::NAREND_BP => {
                if options.show_basepair {
                    if let Some(partner_index) = partner_of(index) {
                        if !rendered.contains(&partner_index) {
                            if let Some((_, partner_pivot)) = pivot_in(&summaries[partner_index]) {
                                bonds.push(RenderBond::new(pivot, partner_pivot, radius, None));
                                rendered.insert(partner_index);
                                rendered.insert(index);
                                continue;
                            }
                        }
                    }
                }
                if let Some((far_index, far)) = atom_in(&summary.atom_indices, far_name) {
                    bonds.push(RenderBond::new(pivot, far, radius, None).with_picking(
                        None,
                        Some((pivot_index, far_index)),
                        false,
                    ));
                    atoms.push(
                        RenderAtom::new(far, radius, None)
                            .with_picking(None, Some(far_index), false)
                            .with_detail(detail),
                    );
                }
            }
            NARenderer::NAREND_SIMPLE1 => {
                let Some((glyco_index, glyco)) = atom_in(&summary.atom_indices, glyco_name) else {
                    continue;
                };
                if options.show_basepair {
                    if let Some(partner_index) = partner_of(index) {
                        if !rendered.contains(&partner_index) {
                            let partner = &summaries[partner_index];
                            let partner_glyco_name = if base_kind(partner) == Some("pur") {
                                "N9"
                            } else {
                                "N1"
                            };
                            if let (
                                Some((partner_pivot_index, partner_pivot)),
                                Some((partner_glyco_index, partner_glyco)),
                            ) = (
                                pivot_in(partner),
                                atom_in(&partner.atom_indices, partner_glyco_name),
                            ) {
                                bonds.push(
                                    RenderBond::new(pivot, glyco, radius, None).with_picking(
                                        None,
                                        Some((pivot_index, glyco_index)),
                                        false,
                                    ),
                                );
                                bonds.push(RenderBond::new(glyco, partner_glyco, radius, None));
                                bonds.push(
                                    RenderBond::new(partner_glyco, partner_pivot, radius, None)
                                        .with_picking(
                                            None,
                                            Some((partner_glyco_index, partner_pivot_index)),
                                            false,
                                        ),
                                );
                                for (atom_index, position) in
                                    [(glyco_index, glyco), (partner_glyco_index, partner_glyco)]
                                {
                                    atoms.push(
                                        RenderAtom::new(position, radius, None)
                                            .with_picking(None, Some(atom_index), false)
                                            .with_detail(detail),
                                    );
                                }
                                rendered.insert(partner_index);
                                rendered.insert(index);
                                continue;
                            }
                        }
                    }
                }
                let Some((far_index, far)) = atom_in(&summary.atom_indices, far_name) else {
                    continue;
                };
                bonds.push(RenderBond::new(pivot, glyco, radius, None).with_picking(
                    None,
                    Some((pivot_index, glyco_index)),
                    false,
                ));
                bonds.push(RenderBond::new(glyco, far, radius, None).with_picking(
                    None,
                    Some((glyco_index, far_index)),
                    false,
                ));
                for (atom_index, position) in [(glyco_index, glyco), (far_index, far)] {
                    atoms.push(
                        RenderAtom::new(position, radius, None)
                            .with_picking(None, Some(atom_index), false)
                            .with_detail(detail),
                    );
                }
            }
            NARenderer::NAREND_DETAIL1 | NARenderer::NAREND_DETAIL2 => {
                let mut included: HashMap<String, (usize, Vector4)> = HashMap::new();
                let mut names = vec!["C1'", "C4", "C5", "N7", "C8", "N9", "N1", "C2", "N3", "C6"];
                if options.base_type == NARenderer::NAREND_DETAIL2 {
                    names.extend(["C2'", "C3'", "C4'", "O4'"]);
                }
                for name in names {
                    if let Some((atom_index, position)) = atom_in(&summary.atom_indices, name) {
                        included.insert(name.to_string(), (atom_index, position));
                        atoms.push(
                            RenderAtom::new(position, radius, None)
                                .with_picking(None, Some(atom_index), false)
                                .with_detail(detail),
                        );
                    }
                }
                let mut add_named_bond = |lhs: &str, rhs: &str| {
                    let (Some(&(lhs_index, lhs_pos)), Some(&(rhs_index, rhs_pos))) =
                        (included.get(lhs), included.get(rhs))
                    else {
                        return;
                    };
                    if lhs_index != rhs_index {
                        bonds.push(
                            RenderBond::new(lhs_pos, rhs_pos, radius, None).with_picking(
                                None,
                                Some((lhs_index, rhs_index)),
                                false,
                            ),
                        );
                    }
                };
                if kind == "pur" {
                    for (lhs, rhs) in [
                        ("C1'", "N9"),
                        ("N9", "C8"),
                        ("C8", "N7"),
                        ("N7", "C5"),
                        ("C4", "N9"),
                        ("N1", "C2"),
                        ("C2", "N3"),
                        ("N3", "C4"),
                        ("C4", "C5"),
                        ("C5", "C6"),
                        ("C6", "N1"),
                    ] {
                        add_named_bond(lhs, rhs);
                    }
                } else {
                    for (lhs, rhs) in [
                        ("C1'", "N1"),
                        ("N1", "C2"),
                        ("C2", "N3"),
                        ("N3", "C4"),
                        ("C4", "C5"),
                        ("C5", "C6"),
                        ("C6", "N1"),
                    ] {
                        add_named_bond(lhs, rhs);
                    }
                }
                if options.base_type == NARenderer::NAREND_DETAIL1 {
                    if let Some(&(c1_index, c1)) = included.get("C1'") {
                        bonds.push(RenderBond::new(pivot, c1, radius, None).with_picking(
                            None,
                            Some((pivot_index, c1_index)),
                            false,
                        ));
                    }
                } else {
                    for (lhs, rhs) in [
                        ("C1'", "C2'"),
                        ("C2'", "C3'"),
                        ("C3'", "C4'"),
                        ("C4'", "O4'"),
                        ("O4'", "C1'"),
                    ] {
                        add_named_bond(lhs, rhs);
                    }
                    if let Some(&(c4_index, c4)) = included.get("C4'") {
                        bonds.push(RenderBond::new(pivot, c4, radius, None).with_picking(
                            None,
                            Some((pivot_index, c4_index)),
                            false,
                        ));
                    }
                }
                append_na_ring_meshes(summary, &included, options.base_thick, &mut meshes);
            }
            _ => {}
        }
        rendered.insert(index);
    }

    RenderSceneSnapshot {
        atoms,
        bonds,
        meshes,
        transparency_groups: Vec::new(),
        bounds: molecule.bounding_box_for(None),
    }
}

fn append_na_ring_meshes(
    summary: &cuemol_molstr::ResidueSummary,
    included: &std::collections::HashMap<String, (usize, Vector4)>,
    thickness: f64,
    meshes: &mut Vec<RenderMesh>,
) {
    let rings = cuemol_molstr::toppar::TopoDB::global().rings(&summary.res_name);
    for ring in rings {
        let Some(points) = ring
            .iter()
            .map(|name| included.get(name).map(|(_, position)| *position))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        if points.len() < 3 {
            continue;
        }
        let normal = normalize_or(
            (points[1] - points[0]).cross(points[2] - points[0]),
            Vector4::new3(0.0, 0.0, 1.0),
        );
        let offset = normal * (thickness.max(0.0) * 0.5);
        let color = [0.75_f32, 0.75, 0.75, 1.0];
        let mut mesh = RenderMesh::new();
        for point in &points {
            mesh.vertices.push(MeshVertex {
                position: *point + offset,
                normal,
                color,
            });
        }
        for point in &points {
            mesh.vertices.push(MeshVertex {
                position: *point - offset,
                normal: normal * -1.0,
                color,
            });
        }
        let n = points.len() as u32;
        for index in 1..n - 1 {
            mesh.indices.extend_from_slice(&[0, index, index + 1]);
            mesh.indices
                .extend_from_slice(&[n, n + index + 1, n + index]);
        }
        for index in 0..n {
            let next = (index + 1) % n;
            mesh.indices
                .extend_from_slice(&[index, next, n + index, next, n + next, n + index]);
        }
        meshes.push(mesh);
    }
}

/// Read an atom's anisotropic displacement tensor (the PDB ANISOU components,
/// stored as integer props in U×10^4) as `[U11,U22,U33,U12,U13,U23]` in Å²;
/// `None` if the atom has no ANISOU record.
fn atom_anisou(atom: &Atom) -> Option<[f64; 6]> {
    let scaled = |name: &str| atom.int_prop(name).map(|v| v as f64 * 1.0e-4);
    Some([
        scaled("anisou_u11")?,
        scaled("anisou_u22")?,
        scaled("anisou_u33")?,
        scaled("anisou_u12")?,
        scaled("anisou_u13")?,
        scaled("anisou_u23")?,
    ])
}

/// Jacobi diagonalization of the symmetric anisotropic displacement tensor.
/// Columns of the returned matrix are normalized eigenvectors.
fn symmetric_eigen3(u: [f64; 6]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut matrix = [[u[0], u[3], u[4]], [u[3], u[1], u[5]], [u[4], u[5], u[2]]];
    let mut vectors = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..32 {
        let mut p = 0;
        let mut q = 1;
        for (i, j) in [(0, 1), (0, 2), (1, 2)] {
            if matrix[i][j].abs() > matrix[p][q].abs() {
                p = i;
                q = j;
            }
        }
        if matrix[p][q].abs() < 1.0e-14 {
            break;
        }
        let angle = 0.5 * (2.0 * matrix[p][q]).atan2(matrix[q][q] - matrix[p][p]);
        let (sin, cos) = angle.sin_cos();
        let app = matrix[p][p];
        let aqq = matrix[q][q];
        let apq = matrix[p][q];
        let mut k = 0;
        while k < 3 {
            if k == p || k == q {
                k += 1;
                continue;
            }
            let akp = matrix[k][p];
            let akq = matrix[k][q];
            matrix[k][p] = cos * akp - sin * akq;
            matrix[p][k] = matrix[k][p];
            matrix[k][q] = sin * akp + cos * akq;
            matrix[q][k] = matrix[k][q];
            k += 1;
        }
        matrix[p][p] = cos * cos * app - 2.0 * sin * cos * apq + sin * sin * aqq;
        matrix[q][q] = sin * sin * app + 2.0 * sin * cos * apq + cos * cos * aqq;
        matrix[p][q] = 0.0;
        matrix[q][p] = 0.0;
        for row in &mut vectors {
            let vip = row[p];
            let viq = row[q];
            row[p] = cos * vip - sin * viq;
            row[q] = sin * vip + cos * viq;
        }
    }
    ([matrix[0][0], matrix[1][1], matrix[2][2]], vectors)
}

#[derive(Clone, Copy)]
struct AnisouTransform {
    axes: [[f64; 3]; 3],
    vectors: [[f64; 3]; 3],
}

fn anisou_axes(u: [f64; 6], scale: f64) -> AnisouTransform {
    let (values, vectors) = symmetric_eigen3(u);
    let mut axes = [[0.0; 3]; 3];
    for axis in 0..3 {
        let length = values[axis].abs().sqrt() * scale;
        axes[axis] = [
            vectors[0][axis] * length,
            vectors[1][axis] * length,
            vectors[2][axis] * length,
        ];
    }
    AnisouTransform { axes, vectors }
}

/// Thermal-ellipsoid mesh for one atom. The symmetric displacement tensor is
/// diagonalized exactly as the C++ renderer does; negative eigenvalues are
/// mirrored to keep malformed records finite.
pub fn anisou_ellipsoid_mesh(
    center: Vector4,
    u: [f64; 6],
    scale: f64,
    color: [f32; 4],
    stacks: usize,
    slices: usize,
) -> RenderMesh {
    let transform = anisou_axes(u, scale);
    let axes = transform.axes;
    let vectors = transform.vectors;
    let mut mesh = RenderMesh::new();
    let stacks = stacks.max(2);
    let slices = slices.max(3);
    for i in 0..=stacks {
        let theta = std::f64::consts::PI * i as f64 / stacks as f64;
        let (st, ct) = (theta.sin(), theta.cos());
        for j in 0..=slices {
            let phi = std::f64::consts::TAU * j as f64 / slices as f64;
            let (sp, cp) = (phi.sin(), phi.cos());
            let unit = [st * cp, st * sp, ct];
            let position = Vector4::new3(
                center.x + axes[0][0] * unit[0] + axes[1][0] * unit[1] + axes[2][0] * unit[2],
                center.y + axes[0][1] * unit[0] + axes[1][1] * unit[1] + axes[2][1] * unit[2],
                center.z + axes[0][2] * unit[0] + axes[1][2] * unit[1] + axes[2][2] * unit[2],
            );
            let normal_component = |axis: usize| {
                let length = (axes[axis][0] * axes[axis][0]
                    + axes[axis][1] * axes[axis][1]
                    + axes[axis][2] * axes[axis][2])
                    .sqrt()
                    .max(1.0e-12);
                unit[axis] / length
            };
            let components = [
                normal_component(0),
                normal_component(1),
                normal_component(2),
            ];
            let normal = normalize_or(
                Vector4::new3(
                    vectors[0][0] * components[0]
                        + vectors[0][1] * components[1]
                        + vectors[0][2] * components[2],
                    vectors[1][0] * components[0]
                        + vectors[1][1] * components[1]
                        + vectors[1][2] * components[2],
                    vectors[2][0] * components[0]
                        + vectors[2][1] * components[1]
                        + vectors[2][2] * components[2],
                ),
                Vector4::new3(0.0, 0.0, 1.0),
            );
            mesh.vertices.push(MeshVertex {
                position,
                normal,
                color,
            });
        }
    }
    let cols = (slices + 1) as u32;
    for i in 0..stacks as u32 {
        for j in 0..slices as u32 {
            let a = i * cols + j;
            let b = a + 1;
            let c = (i + 1) * cols + j;
            let d = c + 1;
            mesh.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    mesh
}

#[derive(Clone, Copy)]
struct AnisouDiscGeometry {
    radius: f64,
    thickness: f64,
    color: [f32; 4],
    slices: usize,
}

fn append_anisou_disc(
    mesh: &mut RenderMesh,
    center: Vector4,
    transform: AnisouTransform,
    normal_axis: usize,
    geometry: AnisouDiscGeometry,
) {
    let axes = transform.axes;
    let vectors = transform.vectors;
    let tangent_axes = match normal_axis {
        0 => [1, 2],
        1 => [2, 0],
        _ => [0, 1],
    };
    let transform = |local: [f64; 3]| {
        Vector4::new3(
            center.x + axes[0][0] * local[0] + axes[1][0] * local[1] + axes[2][0] * local[2],
            center.y + axes[0][1] * local[0] + axes[1][1] * local[1] + axes[2][1] * local[2],
            center.z + axes[0][2] * local[0] + axes[1][2] * local[1] + axes[2][2] * local[2],
        )
    };
    let transform_normal = |local: [f64; 3]| {
        let mut world = [0.0; 3];
        for axis in 0..3 {
            let length = (axes[axis][0] * axes[axis][0]
                + axes[axis][1] * axes[axis][1]
                + axes[axis][2] * axes[axis][2])
                .sqrt()
                .max(1.0e-12);
            let component = local[axis] / length;
            for row in 0..3 {
                world[row] += vectors[row][axis] * component;
            }
        }
        normalize_or(
            Vector4::new3(world[0], world[1], world[2]),
            Vector4::new3(0.0, 0.0, 1.0),
        )
    };
    let slices = geometry.slices.max(4);
    let half = geometry.thickness / 2.0;

    let side_start = mesh.vertices.len() as u32;
    for side in [-half, half] {
        for index in 0..=slices {
            let angle = std::f64::consts::TAU * index as f64 / slices as f64;
            let mut local = [0.0; 3];
            local[normal_axis] = side;
            local[tangent_axes[0]] = geometry.radius * angle.cos();
            local[tangent_axes[1]] = geometry.radius * angle.sin();
            let mut local_normal = [0.0; 3];
            local_normal[tangent_axes[0]] = angle.cos();
            local_normal[tangent_axes[1]] = angle.sin();
            mesh.vertices.push(MeshVertex {
                position: transform(local),
                normal: transform_normal(local_normal),
                color: geometry.color,
            });
        }
    }
    let row = (slices + 1) as u32;
    for index in 0..slices as u32 {
        let a = side_start + index;
        let b = a + 1;
        let c = a + row;
        let d = c + 1;
        mesh.indices.extend_from_slice(&[a, b, c, b, d, c]);
    }

    for (side, sign) in [(-half, -1.0), (half, 1.0)] {
        let center_index = mesh.vertices.len() as u32;
        let mut local = [0.0; 3];
        local[normal_axis] = side;
        let mut local_normal = [0.0; 3];
        local_normal[normal_axis] = sign;
        let normal = transform_normal(local_normal);
        mesh.vertices.push(MeshVertex {
            position: transform(local),
            normal,
            color: geometry.color,
        });
        for index in 0..=slices {
            let angle = std::f64::consts::TAU * index as f64 / slices as f64;
            local[tangent_axes[0]] = geometry.radius * angle.cos();
            local[tangent_axes[1]] = geometry.radius * angle.sin();
            mesh.vertices.push(MeshVertex {
                position: transform(local),
                normal,
                color: geometry.color,
            });
        }
        for index in 0..slices as u32 {
            let a = center_index + 1 + index;
            let b = a + 1;
            if sign < 0.0 {
                mesh.indices.extend_from_slice(&[center_index, b, a]);
            } else {
                mesh.indices.extend_from_slice(&[center_index, a, b]);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnIsoURenderOptions {
    pub draw_discs: bool,
    pub disc_scale: f64,
    pub disc_thickness: f64,
    pub max_vertices: usize,
}

impl Default for AnIsoURenderOptions {
    fn default() -> Self {
        Self {
            draw_discs: true,
            disc_scale: 1.1,
            disc_thickness: 0.1,
            max_vertices: 1_000_000,
        }
    }
}

/// Snapshot rendering atoms as anisotropic thermal ellipsoids (the geometry the
/// `AnIsoURenderer` draws). Atoms with an ANISOU record become ellipsoid meshes;
/// atoms without are drawn as sphere primitives so the structure still renders.
pub fn anisou_mesh_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
) -> RenderSceneSnapshot {
    anisou_mesh_snapshot_with_options(molecule, options, &AnIsoURenderOptions::default())
}

pub fn anisou_mesh_snapshot_with_options(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
    anisou: &AnIsoURenderOptions,
) -> RenderSceneSnapshot {
    let draw = options.draw_indices.as_ref().map(|indices| {
        indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
    });
    let drawn = |index: usize| draw.as_ref().is_none_or(|set| set.contains(&index));
    let selected = options
        .selected_indices
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let colors = resolve_atom_colors(molecule, options);
    let scale = f64::from(options.atom_radius_abs.unwrap_or(0.3));
    let mut detail = options.sphere_detail.unwrap_or(3).max(2) as usize;
    let estimated_vertices = |value: usize| {
        let sphere = (2 * value + 1) * (4 * value + 1);
        let discs = if anisou.draw_discs {
            3 * (4 * (4 * value + 1) + 2)
        } else {
            0
        };
        sphere + discs
    };
    while detail > 2 && estimated_vertices(detail) > anisou.max_vertices {
        detail -= 1;
    }
    let stacks = 2 * detail;
    let slices = 4 * detail;
    let mut bond_options = options.clone();
    bond_options.draw_atoms = false;
    let mut snapshot = molecule_render_snapshot(molecule, &bond_options);
    if options.bond_radius <= 0.0 {
        snapshot.bonds.clear();
    }
    let mut meshes = Vec::new();
    let mut atoms = Vec::new();
    for (index, atom) in molecule.atoms().iter().enumerate() {
        if !drawn(index) {
            continue;
        }
        let spec = colors[index].clone();
        let col = spec
            .rgba8()
            .map(|c| {
                [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                ]
            })
            .unwrap_or([0.8, 0.8, 0.8, 1.0]);
        match atom_anisou(atom) {
            Some(u) => {
                let mut mesh = anisou_ellipsoid_mesh(atom.position, u, scale, col, stacks, slices);
                if anisou.draw_discs {
                    let transform = anisou_axes(u, scale);
                    let geometry = AnisouDiscGeometry {
                        radius: anisou.disc_scale,
                        thickness: anisou.disc_thickness,
                        color: col,
                        slices,
                    };
                    for normal_axis in 0..3 {
                        append_anisou_disc(
                            &mut mesh,
                            atom.position,
                            transform,
                            normal_axis,
                            geometry,
                        );
                    }
                }
                meshes.push(mesh);
            }
            None => {
                let radius = (f64::from(atom.b_factor).max(0.0)
                    / (8.0 * std::f64::consts::PI * std::f64::consts::PI))
                    .sqrt()
                    * scale;
                atoms.push(
                    RenderAtom::new(atom.position, radius as f32, Some(spec))
                        .with_picking(options.object_uid, Some(index), selected.contains(&index))
                        .with_detail(options.sphere_detail),
                );
            }
        }
    }
    snapshot.atoms.extend(atoms);
    snapshot.meshes.extend(meshes);
    snapshot
}

/// A unit vector perpendicular to `tangent`, chosen for numerical stability by
/// crossing with the world axis least aligned with the tangent.
fn perpendicular(tangent: Vector4) -> Vector4 {
    let ax = tangent.x.abs();
    let ay = tangent.y.abs();
    let az = tangent.z.abs();
    let axis = if ax <= ay && ax <= az {
        Vector4::new3(1.0, 0.0, 0.0)
    } else if ay <= az {
        Vector4::new3(0.0, 1.0, 0.0)
    } else {
        Vector4::new3(0.0, 0.0, 1.0)
    };
    normalize_or(tangent.cross(axis), Vector4::new3(1.0, 0.0, 0.0))
}

fn atom_radius(element: &str, options: &MoleculeRenderOptions) -> f32 {
    if let Some(radius) = options.atom_radius_abs {
        return radius;
    }
    if let Some(radii) = options.element_radii {
        return radii.radius_for_element(element) as f32;
    }
    if options.color_mode == MoleculeColorMode::Element {
        (VdwRadii::default().radius_for_element(element) as f32 * options.vdw_scale).max(0.08)
    } else {
        options.atom_radius
    }
}

fn atom_color(element: &str, mode: MoleculeColorMode) -> ColorSpec {
    if mode == MoleculeColorMode::Uniform {
        return rgb_spec(0.78, 0.80, 0.84);
    }
    match element.trim().to_ascii_uppercase().as_str() {
        "H" => rgb_spec(0.92, 0.92, 0.92),
        "C" => rgb_spec(0.38, 0.38, 0.38),
        "N" => rgb_spec(0.20, 0.35, 0.95),
        "O" => rgb_spec(0.92, 0.12, 0.12),
        "S" => rgb_spec(0.95, 0.82, 0.18),
        "P" => rgb_spec(0.95, 0.48, 0.16),
        _ => rgb_spec(0.58, 0.76, 0.72),
    }
}

fn rgb_spec(r: f32, g: f32, b: f32) -> ColorSpec {
    ColorSpec {
        base: cuemol_gfx::ColorBase::Rgb { r, g, b, a: 1.0 },
        modifiers: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_molstr::Atom;

    fn assert_f64_eq(lhs: f64, rhs: f64) {
        assert!((lhs - rhs).abs() < 1.0e-12, "{lhs} != {rhs}");
    }

    fn sample_backbone_test(
        points: &[MainchainPoint],
        subdiv: usize,
        smoothing: f64,
        linear: bool,
    ) -> (Vec<Vector4>, Vec<Secondary>, Vec<f64>) {
        sample_backbone(
            points,
            subdiv,
            &BackboneSamplingOptions {
                anchor_weight: smoothing,
                section_smoothing: RibbonSectionValues::default(),
                cartoon_anchor_atoms: None,
                cartoon_anchor_weight: 10.0,
                linear,
                uniform_section: false,
                helix_head: RibbonJunction::default(),
                helix_tail: RibbonJunction::default(),
                sheet_head: RibbonJunction {
                    kind: RibbonJunctionType::Arrow,
                    ..RibbonJunction::default()
                },
                sheet_tail: RibbonJunction::default(),
            },
        )
    }

    #[test]
    fn scene_styles_override_system_styles_and_later_sets_win() {
        let qsc = r#"<scene>
          <styles id="low"><style id="DefaultBallStick" bondw="0.31"/></styles>
          <styles id="high"><style id="DefaultBallStick" bondw="0.47"/></styles>
        </scene>"#;
        let mut manager = cuemol_molstr::MolSceneManager::new();
        let scene_uid = manager.new_scene("styles");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .load_qsc_str(qsc)
            .unwrap();
        let styles = scene_style_manager(manager.scene(scene_uid).unwrap());
        assert_eq!(
            styles
                .style_attr(DEFAULT_STYLE_SCOPE, "DefaultBallStick", "bondw")
                .as_deref(),
            Some("0.47")
        );
    }

    #[test]
    fn renderer_own_rainbow_coloring_sets_options_rainbow() {
        // A renderer whose OWN style declares <coloring type="RainbowColoring"> sets
        // options.rainbow (overriding the object coloring) — the renderer-level
        // coloring path, resolving the reserved `type` via style_child_type.
        let mut mol = MolCoord::with_atoms(vec![
            sample_atom("CA", "C", 0.0, 0.0, 0.0),
            sample_atom("CA", "C", 3.8, 0.0, 0.0),
        ]);
        mol.atoms_mut()[1].res_id = 2;

        let mut mgr = cuemol_molstr::MolSceneManager::new();
        let scene_uid = mgr.new_scene("s");
        let scene = mgr.scene_mut(scene_uid).unwrap();
        let obj_uid = scene.add_molecule("m", mol);
        let obj = scene.object_mut(obj_uid).unwrap();
        let rend_uid = obj.add_renderer("trace", "tr");
        obj.renderer_mut(rend_uid)
            .unwrap()
            .set_qsc_attr("style", "MyRainbow");

        let dir = std::env::temp_dir().join("cuemol_rend_rainbow_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rainbow.xml");
        std::fs::write(
            &path,
            r##"<?xml version="1.0"?>
<styles id="custom">
  <style id="MyRainbow" type="renderer">
    <coloring type="RainbowColoring" start_hue="240" end_hue="0"/>
  </style>
</styles>"##,
        )
        .unwrap();
        let mut styles = StyleManager::new();
        styles.load_builtin_default_styles(0);
        styles.load_style_set_from_file(0, &path, true);

        let renderer = scene.object(obj_uid).unwrap().renderer(rend_uid).unwrap();
        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &styles, 0);
        let rainbow = options
            .rainbow
            .expect("the renderer's own RainbowColoring should set options.rainbow");
        assert!((rainbow.start_hue() - 240.0).abs() < 1e-9);
        assert!((rainbow.end_hue() - 0.0).abs() < 1e-9);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn renderer_inline_cpk_and_solid_coloring_set_render_colors() {
        let mut manager = cuemol_molstr::MolSceneManager::new();
        let scene_uid = manager.new_scene("coloring");
        let object_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("mol", MolCoord::new());
        let renderer_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "simple");
        let renderer = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap();
        let mut cpk = cuemol_core::LDom2Node::new();
        cpk.set_tag_name("coloring");
        cpk.set_type_name("CPKColoring");
        cpk.set_str_attr("col_C", "$molcol");
        cpk.set_str_attr("col_O", "#123456");
        cpk.set_str_attr("col_X", "#808080");
        renderer.set_qsc_children(vec![cpk]);

        let styles = default_style_manager();
        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &styles, scene_uid.get());
        assert!(!options.cpk_colors.contains_key("C"));
        assert_eq!(
            options.cpk_colors["O"].rgba8().unwrap(),
            cuemol_gfx::Rgba8 {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0xff,
            }
        );
        options.object_paint = vec![("*".to_string(), "#ff00ff".to_string())];
        let carbon = MolCoord::with_atoms(vec![sample_atom("CA", "C", 0.0, 0.0, 0.0)]);
        assert_eq!(
            resolve_atom_colors(&carbon, &options)[0].rgba8().unwrap(),
            cuemol_gfx::Rgba8 {
                r: 0xff,
                g: 0,
                b: 0xff,
                a: 0xff,
            }
        );

        let mut solid = cuemol_core::LDom2Node::new();
        solid.set_tag_name("coloring");
        solid.set_type_name("SolidColoring");
        renderer.set_qsc_children(vec![solid]);
        renderer.set_qsc_attr("defaultcolor", "#654321");
        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &styles, scene_uid.get());
        assert_eq!(options.color_mode, MoleculeColorMode::Uniform);
        assert_eq!(
            options.uniform_color.unwrap().rgba8().unwrap(),
            cuemol_gfx::Rgba8 {
                r: 0x65,
                g: 0x43,
                b: 0x21,
                a: 0xff,
            }
        );
    }

    #[test]
    fn bfac_coloring_reads_all_panel_properties() {
        let attrs = std::collections::BTreeMap::from([
            ("mode", "occ"),
            ("auto", "none"),
            ("lowcol", "#112233"),
            ("highcol", "#abcdef"),
            ("lowpar", "0.25"),
            ("highpar", "0.75"),
        ]);
        let coloring =
            bfac_coloring_from_attrs(Some("BfacColoring"), |name| attrs.get(name).copied())
                .unwrap();
        assert_eq!(coloring.mode(), BfacColoring::BFC_OCC);
        assert_eq!(coloring.auto_mode(), BfacColoring::BFA_NONE);
        assert_f64_eq(coloring.low_param(), 0.25);
        assert_f64_eq(coloring.high_param(), 0.75);
    }

    #[test]
    fn object_solid_coloring_only_replaces_molcol_atoms() {
        let mut options = MoleculeRenderOptions::default();
        let default_color = SolidColor::from_rgba_u8(0x65, 0x43, 0x21, 0xff);
        apply_object_coloring(
            &mut options,
            Some("SolidColoring"),
            |_| None,
            &[],
            &default_color,
        );
        let molecule = MolCoord::with_atoms(vec![
            sample_atom("CA", "C", 0.0, 0.0, 0.0),
            sample_atom("O", "O", 1.0, 0.0, 0.0),
        ]);
        let colors = resolve_atom_colors(&molecule, &options);
        assert_eq!(colors[0].rgba8().unwrap().r, 0x65);
        assert_eq!(colors[0].rgba8().unwrap().g, 0x43);
        assert_eq!(colors[0].rgba8().unwrap().b, 0x21);
        assert_ne!(colors[1].rgba8().unwrap(), colors[0].rgba8().unwrap());
    }

    #[test]
    fn tube_renderer_properties_resolve_into_mesh_options() {
        let mut manager = cuemol_molstr::MolSceneManager::new();
        let scene_uid = manager.new_scene("tube");
        let object_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("mol", MolCoord::new());
        let renderer_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("tube", "tube");
        let renderer = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap();
        for (name, value) in [
            ("type", "roundsquare"),
            ("width", "0.4"),
            ("tuber", "2.5"),
            ("sharp", "0.75"),
            ("detail", "9"),
        ] {
            renderer.set_qsc_child_attr("section", name, value);
        }
        for (name, value) in [
            ("smooth", "0.25"),
            ("smoothcolor", "false"),
            ("pivotatom", "N"),
            ("start_captype", "flat"),
            ("end_captype", "none"),
            ("segend_fade", "true"),
        ] {
            renderer.set_qsc_attr(name, value);
        }

        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &default_style_manager(), 0);
        apply_tube_renderer_styles(&mut options, renderer, &default_style_manager(), 0);
        assert_eq!(options.styled_ribbon.helix, Some((1.0, 0.4)));
        assert_eq!(options.styled_ribbon.sheet, Some((1.0, 0.4)));
        assert_eq!(options.styled_ribbon.coil_shape, SectionShape::RoundSquare);
        assert_eq!(options.ribbon_radial_detail, Some(9));
        assert!((options.ribbon_section_sharp - 0.75).abs() < f64::EPSILON);
        assert!((options.anchor_weight - 0.25).abs() < f64::EPSILON);
        assert!(!options.smooth_color);
        assert_eq!(options.pivot_atom.as_deref(), Some("N"));
        assert_eq!(options.start_cap_type, TubeCapType::Flat);
        assert_eq!(options.end_cap_type, TubeCapType::None);
        assert!(options.segment_end_fade);
        assert!(options.uniform_backbone);
        assert!(options.backbone_atom_colors);
    }

    #[test]
    fn ribbon_renderer_properties_resolve_sections_junctions_and_face_colors() {
        let mut manager = cuemol_molstr::MolSceneManager::new();
        let scene_uid = manager.new_scene("ribbon");
        let object_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("mol", MolCoord::new());
        let renderer_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("ribbon", "ribbon");
        let renderer = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap();
        for (child, name, value) in [
            ("helix", "type", "roundsquare"),
            ("helix", "width", "0.25"),
            ("helix", "tuber", "4"),
            ("helix", "sharp", "0.8"),
            ("helix", "detail", "11"),
            ("sheethead", "type", "arrow"),
            ("sheethead", "gamma", "3"),
            ("sheethead", "basw", "0.2"),
            ("sheethead", "arrow", "2.1"),
        ] {
            renderer.set_qsc_child_attr(child, name, value);
        }
        for (name, value) in [
            ("axialdetail", "7"),
            ("helix_smooth", "1.5"),
            ("sheet_smooth", "0.75"),
            ("coil_smooth", "0.25"),
            ("helix_usebackcol", "true"),
            ("helix_backcol", "#112233"),
            ("sheet_usesidecol", "true"),
            ("sheet_sidecol", "#445566"),
        ] {
            renderer.set_qsc_attr(name, value);
        }

        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &default_style_manager(), 0);
        assert_eq!(options.styled_ribbon.helix, Some((1.0, 0.25)));
        assert_eq!(options.styled_ribbon.helix_shape, SectionShape::RoundSquare);
        assert!((options.styled_ribbon.helix_sharp - 0.8).abs() < f64::EPSILON);
        assert_eq!(options.ribbon_radial_detail, Some(11));
        assert_eq!(options.ribbon_axial_detail, Some(7));
        assert_eq!(
            options.ribbon_smoothing,
            RibbonSectionValues {
                helix: 1.5,
                sheet: 0.75,
                coil: 0.25,
            }
        );
        assert_eq!(options.sheet_head.kind, RibbonJunctionType::Arrow);
        assert!((options.sheet_head.gamma - 3.0).abs() < f64::EPSILON);
        assert!((options.sheet_head.base_width - 0.2).abs() < f64::EPSILON);
        assert!((options.sheet_head.arrow_width - 2.1).abs() < f64::EPSILON);
        assert_eq!(
            options.helix_back_color,
            Some([17.0 / 255.0, 34.0 / 255.0, 51.0 / 255.0, 1.0])
        );
        assert_eq!(
            options.sheet_side_color,
            Some([68.0 / 255.0, 85.0 / 255.0, 102.0 / 255.0, 1.0])
        );
    }

    #[test]
    fn cartoon_renderer_properties_resolve_anchor_and_helix_modes() {
        let mut manager = cuemol_molstr::MolSceneManager::new();
        let scene_uid = manager.new_scene("cartoon");
        let object_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("mol", MolCoord::new());
        let renderer_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("cartoon", "cartoon");
        let renderer = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap();
        for (name, value) in [
            ("anchor_sel", "chain A"),
            ("anchor_weight", "12"),
            ("helix_ribbon", "true"),
            ("helix_width", "2.6"),
            ("helix_wplus", "0.4"),
        ] {
            renderer.set_qsc_attr(name, value);
        }
        for (name, value) in [
            ("type", "rectangle"),
            ("width", "0.3"),
            ("tuber", "5"),
            ("detail", "12"),
        ] {
            renderer.set_qsc_child_attr("ribhelix", name, value);
        }
        renderer.set_qsc_child_attr("ribhelix_head", "type", "flat");

        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &default_style_manager(), 0);
        assert_eq!(options.cartoon_anchor_selection.as_deref(), Some("chain A"));
        assert!((options.cartoon_anchor_weight - 12.0).abs() < f64::EPSILON);
        assert!(options.cartoon_helix_ribbon);
        assert!((options.cartoon_helix_width - 2.6).abs() < f64::EPSILON);
        assert!((options.cartoon_helix_width_plus - 0.4).abs() < f64::EPSILON);
        assert_eq!(options.styled_ribbon.helix, Some((1.5, 0.3)));
        assert_eq!(options.styled_ribbon.helix_shape, SectionShape::Rectangular);
        assert_eq!(options.ribbon_radial_detail, Some(12));
        assert_eq!(options.helix_head.kind, RibbonJunctionType::Flat);
        assert_eq!(options.start_cap_type, TubeCapType::Flat);
        assert!(!options.smooth_color);
    }

    #[test]
    fn rainbow_rbc_chain_restarts_the_gradient_per_chain() {
        // Default 240(blue)->0(red). For the SECOND chain (global offset 5 of 10):
        // RBC_CHAIN restarts at local index 0 -> blue; RBC_MOL continues at global
        // index 5 -> mid-gradient (not blue).
        let is_blue = |c: [f32; 4]| c[2] > 0.8 && c[0] < 0.2;
        let atoms = (0..10)
            .map(|index| {
                let mut atom = ca_atom(index % 5 + 1, index as f64 * 3.8, 0.0, 0.0);
                atom.serial = index + 1;
                atom.chain_id = if index < 5 { "A" } else { "B" }.to_string();
                atom
            })
            .collect();
        let molecule = MolCoord::with_atoms(atoms);
        let mut rb = RainbowColoring::new();
        rb.set_mode(RainbowColoring::RBC_CHAIN);
        let chain = rainbow_atom_colors(&molecule, &rb);
        assert!(
            is_blue(chain[5].unwrap()),
            "RBC_CHAIN: 2nd chain restarts blue, got {:?}",
            chain[5]
        );
        rb.set_mode(RainbowColoring::RBC_MOL);
        let mol = rainbow_atom_colors(&molecule, &rb);
        assert!(
            !is_blue(mol[5].unwrap()),
            "RBC_MOL: 2nd chain continues the global gradient (not blue), got {:?}",
            mol[5]
        );
    }

    #[test]
    fn rainbow_increment_modes_match_chain_and_protein_ss_markers() {
        let mut chain_atoms = Vec::new();
        for chain_index in 0..3 {
            for res_id in 1..=2 {
                let mut atom = ca_atom(res_id, (chain_index * 10 + res_id) as f64, 0.0, 0.0);
                atom.serial = chain_index * 2 + res_id;
                atom.chain_id = char::from(b'A' + chain_index as u8).to_string();
                chain_atoms.push(atom);
            }
        }
        let molecule = MolCoord::with_atoms(chain_atoms);
        let mut rainbow = RainbowColoring::new();
        rainbow.set_incr_mode(RainbowColoring::RBC_INCR_CHAIN);
        let colors = rainbow_atom_colors(&molecule, &rainbow);
        assert!(colors[0].unwrap()[2] > 0.9, "first chain starts blue");
        assert!(colors[2].unwrap()[1] > 0.9, "middle chain is green");
        assert!(colors[4].unwrap()[0] > 0.9, "last chain reaches red");
        assert_eq!(colors[0], colors[1]);
        assert_eq!(colors[2], colors[3]);
        assert_eq!(colors[4], colors[5]);

        rainbow.set_mode(RainbowColoring::RBC_CHAIN);
        let local = rainbow_atom_colors(&molecule, &rainbow);
        assert!(local.iter().flatten().all(|color| color[2] > 0.9));

        let mut protein = MolCoord::with_atoms(
            (1..=8)
                .map(|res_id| ca_atom(res_id, res_id as f64 * 3.8, 0.0, 0.0))
                .collect(),
        );
        for res_id in [1, 2, 7, 8] {
            protein
                .set_residue_prop_str("A", &res_id.to_string(), "secondary", "helix")
                .unwrap();
        }
        rainbow.set_mode(RainbowColoring::RBC_MOL);
        rainbow.set_incr_mode(RainbowColoring::RBC_INCR_PROTSS);
        let protss = rainbow_atom_colors(&protein, &rainbow);
        assert!(protss[4].unwrap()[2] > 0.9, "marker residue keeps old hue");
        assert!(protss[5].unwrap()[0] > 0.9, "hue changes after marker");
    }

    fn sample_atom(name: &str, element: &str, x: f64, y: f64, z: f64) -> Atom {
        Atom {
            serial: 1,
            name: name.to_string(),
            alt_loc: None,
            res_name: "ALA".to_string(),
            chain_id: "A".to_string(),
            res_id: 1,
            ins_code: None,
            position: Vector4::new3(x, y, z),
            occupancy: 1.0,
            b_factor: 0.0,
            element: element.to_string(),
            formal_charge: None,
            int_props: Default::default(),
            real_props: Default::default(),
        }
    }

    #[test]
    fn script_coloring_evaluates_atom_and_residue_objects_in_one_batch() {
        let carbon = sample_atom("CA", "C", 0.0, 0.0, 0.0);
        let mut oxygen = sample_atom("O", "O", 1.0, 0.0, 0.0);
        oxygen.serial = 2;
        oxygen.res_name = "GLY".to_string();
        oxygen.res_id = 2;
        let molecule = MolCoord::with_atoms(vec![carbon, oxygen]);
        let options = MoleculeRenderOptions {
            script_coloring: Some(ScriptColoringOptions {
                script: r##"
if (atom.element === "C" && resid.name === "ALA") return cuemol.makeColor("#ff0000");
if (atom.id === 2 && resid.sindex === "2") return makeColor("#0000ff");
return null;
"##
                .to_string(),
                renderer_name: Some("main".to_string()),
                renderer_type: Some("simple".to_string()),
                all_atoms: true,
            }),
            ..MoleculeRenderOptions::default()
        };

        let colors = resolve_atom_colors(&molecule, &options);
        assert_eq!(
            colors[0].rgba8().unwrap(),
            cuemol_gfx::Rgba8 {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            }
        );
        assert_eq!(
            colors[1].rgba8().unwrap(),
            cuemol_gfx::Rgba8 {
                r: 0,
                g: 0,
                b: 255,
                a: 255,
            }
        );
    }

    #[test]
    fn renderer_script_coloring_node_populates_render_options() {
        let carbon = sample_atom("CA", "C", 0.0, 0.0, 0.0);
        let mut oxygen = sample_atom("O", "O", 1.0, 0.0, 0.0);
        oxygen.serial = 2;
        let molecule = MolCoord::with_atoms(vec![carbon, oxygen]);
        let mut manager = cuemol_molstr::MolSceneManager::new();
        let scene_uid = manager.new_scene("script");
        let scene = manager.scene_mut(scene_uid).unwrap();
        let object_uid = scene.add_molecule("molecule", molecule.clone());
        let object = scene.object_mut(object_uid).unwrap();
        let renderer_uid = object.add_renderer("simple", "main");
        let renderer = object.renderer_mut(renderer_uid).unwrap();
        let mut coloring = cuemol_core::LDom2Node::new();
        coloring.set_tag_name("coloring");
        coloring.set_type_name("ScriptColoring");
        coloring
            .append_child("script")
            .set_contents("return cuemol.makeColor(atom.element === \"C\" ? \"red\" : \"blue\");");
        renderer.set_qsc_children(vec![coloring]);

        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &default_style_manager(), 0);
        let script = options.script_coloring.as_ref().unwrap();
        assert!(script.all_atoms);
        assert_eq!(script.renderer_name.as_deref(), Some("main"));
        assert_eq!(script.renderer_type.as_deref(), Some("simple"));
        let colors = resolve_atom_colors(&molecule, &options);
        assert_eq!(colors[0].rgba8().unwrap().r, 255);
        assert_eq!(colors[1].rgba8().unwrap().b, 255);
    }

    #[test]
    fn script_coloring_rejects_non_color_values_and_stops_after_an_error() {
        let nitrogen = sample_atom("N", "N", 0.0, 0.0, 0.0);
        let mut oxygen = sample_atom("O", "O", 1.0, 0.0, 0.0);
        oxygen.serial = 2;
        let mut sulfur = sample_atom("S", "S", 2.0, 0.0, 0.0);
        sulfur.serial = 3;
        let molecule = MolCoord::with_atoms(vec![nitrogen, oxygen, sulfur]);
        let options = MoleculeRenderOptions {
            script_coloring: Some(ScriptColoringOptions {
                script: r##"
if (atom.id === 1) return "#00ff00";
if (atom.id === 2) throw new Error("stop");
return cuemol.makeColor("#0000ff");
"##
                .to_string(),
                renderer_name: None,
                renderer_type: None,
                all_atoms: true,
            }),
            ..MoleculeRenderOptions::default()
        };

        let colors = resolve_atom_colors(&molecule, &options);
        assert_eq!(colors[0], atom_color("N", MoleculeColorMode::Element));
        assert_eq!(colors[1], atom_color("O", MoleculeColorMode::Element));
        assert_eq!(colors[2], atom_color("S", MoleculeColorMode::Element));
    }

    #[test]
    fn atom_prop_coloring_uses_packed_atom_colors_and_gray_fallback() {
        let mut colored = sample_atom("C1", "C", 0.0, 0.0, 0.0);
        colored
            .int_props
            .insert("col".to_string(), 0xff00_00ff_u32 as i32);
        let molecule = MolCoord::with_atoms(vec![colored, sample_atom("C2", "C", 2.0, 0.0, 0.0)]);

        let mut manager = cuemol_molstr::MolSceneManager::new();
        let scene_uid = manager.new_scene("s");
        let scene = manager.scene_mut(scene_uid).unwrap();
        let object_uid = scene.add_molecule("m", molecule.clone());
        let object = scene.object_mut(object_uid).unwrap();
        let renderer_uid = object.add_renderer("simple", "simple1");
        let renderer = object.renderer_mut(renderer_uid).unwrap();
        let mut coloring = cuemol_core::LDom2Node::new();
        coloring.set_tag_name("coloring");
        coloring.set_type_name("AtomPropColoring");
        renderer.set_qsc_children(vec![coloring]);

        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &default_style_manager(), 0);
        assert_eq!(options.atom_color_property.as_deref(), Some("col"));
        let colors = resolve_atom_colors(&molecule, &options);
        assert_eq!(
            colors[0].rgba8().unwrap(),
            cuemol_gfx::Rgba8 {
                r: 0,
                g: 0,
                b: 255,
                a: 255,
            }
        );
        assert_eq!(
            colors[1].rgba8().unwrap(),
            cuemol_gfx::Rgba8 {
                r: 128,
                g: 128,
                b: 128,
                a: 255,
            }
        );
    }

    /// Build a 2-atom ball-and-stick scene with the given style preset (and an
    /// optional inline `bondw` override), resolve its styles against the bundled
    /// default styles, and return the (bond cylinder radius, atom sphere radius)
    /// the snapshot actually produces — an INDEPENDENT geometry oracle (reads the
    /// emitted mesh radii, not the parsed XML).
    fn styled_ballstick_radii(style: &str, inline_bondw: Option<&str>) -> (f32, f32) {
        let mut mol = MolCoord::with_atoms(vec![
            sample_atom("C1", "C", 0.0, 0.0, 0.0),
            sample_atom("C2", "C", 1.5, 0.0, 0.0),
        ]);
        mol.add_bond(0, 1, cuemol_molstr::BondOrder::Single);

        let mut mgr = cuemol_molstr::MolSceneManager::new();
        let scene_uid = mgr.new_scene("s");
        let scene = mgr.scene_mut(scene_uid).unwrap();
        let obj_uid = scene.add_molecule("m", mol);
        let obj = scene.object_mut(obj_uid).unwrap();
        let rend_uid = obj.add_renderer("ballstick", "bs");
        let renderer = obj.renderer_mut(rend_uid).unwrap();
        renderer.set_qsc_attr("style", style);
        if let Some(bondw) = inline_bondw {
            renderer.set_qsc_attr("bondw", bondw);
        }

        let mut styles = StyleManager::new();
        styles.load_builtin_default_styles(0);

        let obj = scene.object(obj_uid).unwrap();
        let renderer = obj.renderer(rend_uid).unwrap();
        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &styles, 0);
        let snapshot = molecule_render_snapshot(obj.mol(), &options);
        (snapshot.bonds[0].radius, snapshot.atoms[0].radius)
    }

    #[test]
    fn ballstick_named_style_drives_bond_and_atom_geometry() {
        // DefaultBallStick: bondw=0.2, sphr=0.3 — both differ from the hardcoded
        // defaults (bond_radius=0.08, vdW-scaled atom), so a passing assert proves
        // the style value flowed all the way into the rendered mesh.
        let (bond, atom) = styled_ballstick_radii("DefaultBallStick", None);
        assert!((bond - 0.2).abs() < 1.0e-6, "bond radius {bond}");
        assert!((atom - 0.3).abs() < 1.0e-6, "atom radius {atom}");

        // A DIFFERENT named style yields measurably DIFFERENT (thicker) geometry —
        // the non-circular oracle: distinct presets => distinct mesh radii.
        let (thick_bond, thick_atom) = styled_ballstick_radii("ThickBallStick", None);
        assert!((thick_bond - 0.3).abs() < 1.0e-6, "thick bond {thick_bond}");
        assert!((thick_atom - 0.5).abs() < 1.0e-6, "thick atom {thick_atom}");
        assert!(thick_bond > bond && thick_atom > atom);

        // Inline `.qsc` attribute overrides the named style (applyStyles order).
        let (override_bond, _) = styled_ballstick_radii("DefaultBallStick", Some("0.9"));
        assert!(
            (override_bond - 0.9).abs() < 1.0e-6,
            "inline override {override_bond}"
        );

        // No style => the hardcoded default radius is left untouched.
        let (plain_bond, _) = styled_ballstick_radii("", None);
        assert!(
            (plain_bond - 0.08).abs() < 1.0e-6,
            "plain default {plain_bond}"
        );
    }

    /// Tessellate a single-atom CPK scene, either resolving the named `style`
    /// (when `forced_detail` is None) or forcing `sphere_detail` directly, and
    /// return the atom-sphere vertex count. The geometry oracle for `detail`:
    /// reads the EMITTED mesh density, not the parsed XML.
    fn cpk_vertex_count(style: &str, forced_detail: Option<Option<u32>>) -> usize {
        let mol = MolCoord::with_atoms(vec![sample_atom("C1", "C", 0.0, 0.0, 0.0)]);

        let mut mgr = cuemol_molstr::MolSceneManager::new();
        let scene_uid = mgr.new_scene("s");
        let scene = mgr.scene_mut(scene_uid).unwrap();
        let obj_uid = scene.add_molecule("m", mol);
        let obj = scene.object_mut(obj_uid).unwrap();
        let rend_uid = obj.add_renderer("cpk", "cpk");
        obj.renderer_mut(rend_uid)
            .unwrap()
            .set_qsc_attr("style", style);

        let styles = default_style_manager();
        let obj = scene.object(obj_uid).unwrap();
        let renderer = obj.renderer(rend_uid).unwrap();
        let mut options = MoleculeRenderOptions {
            draw_bonds: false,
            ..MoleculeRenderOptions::default()
        };
        match forced_detail {
            Some(detail) => options.sphere_detail = detail,
            None => apply_renderer_styles(&mut options, renderer, &styles, DEFAULT_STYLE_SCOPE),
        }
        tessellate_styled_snapshot(obj.mol(), &options)
            .vertices
            .len()
    }

    #[test]
    fn cpk_style_detail_drives_sphere_tessellation_density() {
        // DefaultCPK carries detail=10 (fine); no style keeps the coarse default.
        let styled = cpk_vertex_count("DefaultCPK", None);
        let coarse = cpk_vertex_count("", Some(None));

        // The style resolved to detail=10 specifically: the styled mesh matches a
        // mesh tessellated with detail 10 forced directly, and is denser than the
        // default (detail 4). Non-circular: distinct detail => distinct vertex count.
        assert_eq!(styled, cpk_vertex_count("", Some(Some(10))));
        assert!(
            styled > coarse,
            "DefaultCPK (detail=10) should tessellate finer than the default: {styled} vs {coarse}"
        );
        assert!(cpk_vertex_count("", Some(Some(10))) > cpk_vertex_count("", Some(Some(4))));
    }

    #[test]
    fn cpk_element_radius_attributes_drive_atom_geometry() {
        let mol = MolCoord::with_atoms(vec![
            sample_atom("H1", "H", 0.0, 0.0, 0.0),
            sample_atom("C1", "C", 2.0, 0.0, 0.0),
            sample_atom("XE", "XE", 5.0, 0.0, 0.0),
        ]);
        let mut manager = cuemol_molstr::MolSceneManager::new();
        let scene_uid = manager.new_scene("scene");
        let scene = manager.scene_mut(scene_uid).unwrap();
        let object_uid = scene.add_molecule("molecule", mol);
        let object = scene.object_mut(object_uid).unwrap();
        let renderer_uid = object.add_renderer("cpk", "cpk");
        let renderer = object.renderer_mut(renderer_uid).unwrap();
        renderer.set_qsc_attr("vdwr_H", "0.8");
        renderer.set_qsc_attr("vdwr_C", "2.4");

        let object = scene.object(object_uid).unwrap();
        let renderer = object.renderer(renderer_uid).unwrap();
        let mut options = MoleculeRenderOptions {
            draw_bonds: false,
            element_radii: Some(VdwRadii::default()),
            ..MoleculeRenderOptions::default()
        };
        apply_renderer_styles(
            &mut options,
            renderer,
            &default_style_manager(),
            DEFAULT_STYLE_SCOPE,
        );
        let snapshot = molecule_render_snapshot(object.mol(), &options);
        assert_eq!(snapshot.atoms.len(), 3);
        assert!((snapshot.atoms[0].radius - 0.8).abs() < 1.0e-6);
        assert!((snapshot.atoms[1].radius - 2.4).abs() < 1.0e-6);
        assert!((snapshot.atoms[2].radius - 1.7).abs() < 1.0e-6);
    }

    /// Resolve a ribbon renderer's style (plus an optional inline `axialdetail`
    /// override) and return the total backbone-mesh vertex count — the geometry
    /// oracle for `axialdetail` (more subdivisions => denser mesh).
    fn ribbon_vertex_count(style: &str, inline_axial: Option<&str>) -> usize {
        let mol = MolCoord::with_atoms(vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 1.0, 0.5, 0.0),
            ca_atom(3, 2.0, 0.0, 0.5),
            ca_atom(4, 3.0, 0.5, 0.0),
            ca_atom(5, 4.0, 0.0, 0.0),
        ]);
        let mut mgr = cuemol_molstr::MolSceneManager::new();
        let scene_uid = mgr.new_scene("s");
        let scene = mgr.scene_mut(scene_uid).unwrap();
        let obj_uid = scene.add_molecule("m", mol);
        let obj = scene.object_mut(obj_uid).unwrap();
        let rend_uid = obj.add_renderer("ribbon", "rb");
        let renderer = obj.renderer_mut(rend_uid).unwrap();
        renderer.set_qsc_attr("style", style);
        if let Some(axial) = inline_axial {
            renderer.set_qsc_attr("axialdetail", axial);
        }

        let styles = default_style_manager();
        let obj = scene.object(obj_uid).unwrap();
        let renderer = obj.renderer(rend_uid).unwrap();
        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &styles, DEFAULT_STYLE_SCOPE);
        ribbon_mesh_snapshot(obj.mol(), &options)
            .meshes
            .iter()
            .map(|mesh| mesh.vertices.len())
            .sum()
    }

    #[test]
    fn ribbon_style_axialdetail_drives_backbone_mesh_density() {
        // DefaultRibbon carries axialdetail=8 (== the default subdivision), so a
        // styled ribbon matches a default-options ribbon.
        let mol = MolCoord::with_atoms(vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 1.0, 0.5, 0.0),
            ca_atom(3, 2.0, 0.0, 0.5),
            ca_atom(4, 3.0, 0.5, 0.0),
            ca_atom(5, 4.0, 0.0, 0.0),
        ]);
        let reference_defaults = MoleculeRenderOptions {
            start_cap_type: TubeCapType::Sphere,
            end_cap_type: TubeCapType::Sphere,
            ..MoleculeRenderOptions::default()
        };
        let default_count: usize = ribbon_mesh_snapshot(&mol, &reference_defaults)
            .meshes
            .iter()
            .map(|mesh| mesh.vertices.len())
            .sum();
        let styled = ribbon_vertex_count("DefaultRibbon", None);
        assert_eq!(styled, default_count);

        // An inline axialdetail=20 overrides the style and densifies the mesh —
        // the value flows from the attribute into the backbone subdivision.
        let denser = ribbon_vertex_count("DefaultRibbon", Some("20"));
        assert!(
            denser > styled,
            "axialdetail=20 should densify the ribbon mesh: {denser} vs {styled}"
        );
    }

    #[test]
    fn ribbon_style_section_shape_is_rectangular_or_elliptical_per_type() {
        // Build a helix run and render it under two styles that share the same
        // dims but differ in section `type`: DefaultRibbon (rectangle) vs
        // RoundRibbon (elliptical).
        fn first_ring_distinct_normals(style: &str) -> (SectionShape, usize) {
            let mut mol = MolCoord::with_atoms(vec![
                ca_atom(1, 0.0, 0.0, 0.0),
                ca_atom(2, 3.0, 0.0, 0.0),
                ca_atom(3, 6.0, 0.0, 0.0),
                ca_atom(4, 9.0, 0.0, 0.0),
            ]);
            for res_id in 1..=4 {
                mol.set_residue_prop_str("A", &res_id.to_string(), "secondary", "helix")
                    .unwrap();
            }
            let mut mgr = cuemol_molstr::MolSceneManager::new();
            let scene_uid = mgr.new_scene("s");
            let scene = mgr.scene_mut(scene_uid).unwrap();
            let obj_uid = scene.add_molecule("m", mol);
            let obj = scene.object_mut(obj_uid).unwrap();
            let rend_uid = obj.add_renderer("ribbon", "rb");
            obj.renderer_mut(rend_uid)
                .unwrap()
                .set_qsc_attr("style", style);

            let styles = default_style_manager();
            let obj = scene.object(obj_uid).unwrap();
            let renderer = obj.renderer(rend_uid).unwrap();
            let mut options = MoleculeRenderOptions::default();
            apply_renderer_styles(&mut options, renderer, &styles, DEFAULT_STYLE_SCOPE);

            let snap = ribbon_mesh_snapshot(obj.mol(), &options);
            let ring = &snap.meshes[0].vertices[0..TUBE_RADIAL];
            // Cluster normals by direction (dot > 0.99 => same flat face).
            let mut reps: Vec<Vector4> = Vec::new();
            for vertex in ring {
                if !reps.iter().any(|r| r.dot3d(vertex.normal) > 0.99) {
                    reps.push(vertex.normal);
                }
            }
            (options.styled_ribbon.helix_shape, reps.len())
        }

        let (rect_shape, rect_normals) = first_ring_distinct_normals("DefaultRibbon");
        let (ell_shape, ell_normals) = first_ring_distinct_normals("RoundRibbon");

        assert_eq!(rect_shape, SectionShape::Rectangular);
        assert_eq!(ell_shape, SectionShape::Elliptical);
        // A rectangle's flat sides collapse to a few axis-aligned face normals;
        // the ellipse's smooth profile gives a distinct normal per ring vertex.
        assert!(
            rect_normals <= 4,
            "rectangular section should have <=4 face normals, got {rect_normals}"
        );
        assert!(
            ell_normals >= 10,
            "elliptical section should have many distinct normals, got {ell_normals}"
        );
    }

    #[test]
    fn ribbon_style_section_sets_cross_section_aspect_from_tuber() {
        // A helix run styled by DefaultRibbon (<helix width="0.2" tuber="6.0"/>).
        let mut mol = MolCoord::with_atoms(vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 3.0, 0.0, 0.0),
            ca_atom(3, 6.0, 0.0, 0.0),
            ca_atom(4, 9.0, 0.0, 0.0),
        ]);
        for res_id in 1..=4 {
            mol.set_residue_prop_str("A", &res_id.to_string(), "secondary", "helix")
                .unwrap();
        }

        let mut mgr = cuemol_molstr::MolSceneManager::new();
        let scene_uid = mgr.new_scene("s");
        let scene = mgr.scene_mut(scene_uid).unwrap();
        let obj_uid = scene.add_molecule("m", mol);
        let obj = scene.object_mut(obj_uid).unwrap();
        let rend_uid = obj.add_renderer("ribbon", "rb");
        // RoundRibbon: same helix/sheet dims as DefaultRibbon but an ELLIPTICAL
        // profile, so the ring aspect equals tuber exactly (a rectangle's corners
        // would push the max distance past base_width — covered by the shape test).
        obj.renderer_mut(rend_uid)
            .unwrap()
            .set_qsc_attr("style", "RoundRibbon");

        let styles = default_style_manager();
        let obj = scene.object(obj_uid).unwrap();
        let renderer = obj.renderer(rend_uid).unwrap();
        let mut options = MoleculeRenderOptions::default();
        apply_renderer_styles(&mut options, renderer, &styles, DEFAULT_STYLE_SCOPE);

        // The nested <helix> resolves to absolute semi-axes (width*tuber, width).
        assert_eq!(options.styled_ribbon.helix, Some((0.2 * 6.0, 0.2)));
        assert_eq!(options.styled_ribbon.sheet, Some((0.2 * 7.0, 0.2)));
        assert_eq!(options.styled_ribbon.helix_shape, SectionShape::Elliptical);
        // RoundRibbon omits <coil>, so the coil falls back to the C++
        // TubeSection default (0.35 round) — keeping one unit system.
        assert_eq!(options.styled_ribbon.coil, Some((0.35, 0.35)));

        // The EMITTED helix ring's aspect ratio equals tuber=6.0 — the C++ flat
        // band — and is distinctly flatter than the unstyled factor model (~3.14).
        let aspect = |opts: &MoleculeRenderOptions| -> f64 {
            let snap = ribbon_mesh_snapshot(obj.mol(), opts);
            let ring = &snap.meshes[0].vertices[0..TUBE_RADIAL];
            let n = TUBE_RADIAL as f64;
            let c = (
                ring.iter().map(|v| v.position.x).sum::<f64>() / n,
                ring.iter().map(|v| v.position.y).sum::<f64>() / n,
                ring.iter().map(|v| v.position.z).sum::<f64>() / n,
            );
            let dists: Vec<f64> = ring
                .iter()
                .map(|v| {
                    let (dx, dy, dz) = (v.position.x - c.0, v.position.y - c.1, v.position.z - c.2);
                    (dx * dx + dy * dy + dz * dz).sqrt()
                })
                .collect();
            dists.iter().cloned().fold(0.0_f64, f64::max)
                / dists
                    .iter()
                    .cloned()
                    .fold(f64::INFINITY, f64::min)
                    .max(1.0e-9)
        };
        let styled_aspect = aspect(&options);
        let default_aspect = aspect(&MoleculeRenderOptions::default());
        assert!(
            (styled_aspect - 6.0).abs() < 0.05,
            "styled helix aspect should be tuber=6.0, got {styled_aspect}"
        );
        assert!(
            styled_aspect > default_aspect + 1.0,
            "styled (tuber=6) should be flatter than the default factor band: {styled_aspect} vs {default_aspect}"
        );
    }

    #[test]
    fn cubic_spline_is_natural_interpolating_and_c2_continuous() {
        // A non-collinear 3D path so the curve genuinely bends.
        let pts = [
            Vector4::new3(0.0, 0.0, 0.0),
            Vector4::new3(1.0, 1.0, 0.0),
            Vector4::new3(2.0, 0.0, 1.0),
            Vector4::new3(3.0, 1.0, 0.0),
            Vector4::new3(4.0, 0.0, 0.0),
        ];
        let spline = CubicSpline::generate(&pts).unwrap();
        assert_eq!(spline.point_count(), 5);

        // (1) Interpolation: integer parameters land exactly on control points.
        for (i, p) in pts.iter().enumerate() {
            let s = spline.interpolate(i as f64);
            assert!((s.position - *p).length3d() < 1e-9, "control point {i}");
        }

        // (2) Natural boundary condition: zero curvature at both ends.
        assert!(spline.interpolate(0.0).curvature.length3d() < 1e-9);
        assert!(spline.interpolate(4.0).curvature.length3d() < 1e-9);

        // (3) C1 + (4) C2 continuity across an interior knot (par = 2): tangent
        // and curvature agree from the left and the right.
        let eps = 1e-6;
        let left = spline.interpolate(2.0 - eps);
        let right = spline.interpolate(2.0 + eps);
        assert!(
            (left.tangent - right.tangent).length3d() < 1e-3,
            "C1 at knot"
        );
        assert!(
            (left.curvature - right.curvature).length3d() < 1e-3,
            "C2 at knot"
        );

        // (5) The analytic tangent matches a central finite difference of position.
        let mid = spline.interpolate(1.5);
        let fd = (spline.interpolate(1.5 + eps).position - spline.interpolate(1.5 - eps).position)
            .scale(1.0 / (2.0 * eps));
        assert!(
            (mid.tangent - fd).length3d() < 1e-4,
            "tangent vs finite diff"
        );

        // (6) The curve actually bends: the mid-segment sample leaves the chord.
        let chord_mid = (pts[1] + pts[2]).scale(0.5);
        assert!(
            (mid.position - chord_mid).length3d() > 1e-3,
            "curve deviates from chord"
        );
    }

    fn intr_atom(serial: i32, x: f64, y: f64, z: f64) -> Atom {
        let mut a = sample_atom("X", "C", x, y, z);
        a.serial = serial;
        a
    }

    /// (x-c)ᵀ U⁻¹ (x-c) for a symmetric 3×3 U = [u11,u22,u33,u12,u13,u23].
    fn quad_form(d: [f64; 3], u: [f64; 6]) -> f64 {
        let (a, b, c, p, q, r) = (u[0], u[1], u[2], u[3], u[4], u[5]);
        // cofactors of the symmetric matrix [[a,p,q],[p,b,r],[q,r,c]]
        let det = a * (b * c - r * r) - p * (p * c - r * q) + q * (p * r - b * q);
        let i00 = (b * c - r * r) / det;
        let i11 = (a * c - q * q) / det;
        let i22 = (a * b - p * p) / det;
        let i01 = (q * r - p * c) / det;
        let i02 = (p * r - q * b) / det;
        let i12 = (p * q - a * r) / det;
        d[0] * d[0] * i00
            + d[1] * d[1] * i11
            + d[2] * d[2] * i22
            + 2.0 * d[0] * d[1] * i01
            + 2.0 * d[0] * d[2] * i02
            + 2.0 * d[1] * d[2] * i12
    }

    #[test]
    fn anisou_ellipsoid_is_a_correct_probability_contour() {
        let center = Vector4::new3(1.0, 2.0, 3.0);
        let color = [1.0, 1.0, 1.0, 1.0];
        let scale = 1.5;

        // Isotropic U = 0.01 Å²: the ellipsoid is a sphere of radius scale·0.1.
        let iso = anisou_ellipsoid_mesh(
            center,
            [0.01, 0.01, 0.01, 0.0, 0.0, 0.0],
            scale,
            color,
            8,
            12,
        );
        let dist = |v: &MeshVertex| (v.position - center).length3d();
        let max = iso.vertices.iter().map(dist).fold(0.0_f64, f64::max);
        let min = iso.vertices.iter().map(dist).fold(f64::INFINITY, f64::min);
        assert!((max - scale * 0.1).abs() < 1e-6 && (min - scale * 0.1).abs() < 1e-6);

        // Anisotropic, off-diagonal U: every vertex lies on the contour
        // (v-c)ᵀ U⁻¹ (v-c) = scale² — proving it's the correct ellipsoid surface.
        let u = [0.04, 0.02, 0.01, 0.008, 0.005, -0.003];
        let ell = anisou_ellipsoid_mesh(center, u, scale, color, 16, 20);
        assert!(ell.triangle_count() > 100);
        for v in &ell.vertices {
            let d = [
                v.position.x - center.x,
                v.position.y - center.y,
                v.position.z - center.z,
            ];
            assert!(
                (quad_form(d, u) - scale * scale).abs() < 1e-6,
                "vertex off contour"
            );
        }

        // The principal stretch is along x (largest U component) — wider than y.
        let ext =
            |f: &dyn Fn(&MeshVertex) -> f64| ell.vertices.iter().map(f).fold(0.0_f64, f64::max);
        let max_x = ext(&|v| (v.position.x - center.x).abs());
        let max_y = ext(&|v| (v.position.y - center.y).abs());
        assert!(max_x > max_y, "elongated along the dominant axis");

        // The C++ renderer mirrors negative eigenvalues and never emits NaNs.
        let bad = anisou_ellipsoid_mesh(
            center,
            [-1.0, 0.01, 0.01, 0.0, 0.0, 0.0],
            scale,
            color,
            6,
            8,
        );
        assert!(bad.vertices.iter().all(|v| v.position.x.is_finite()));
    }

    fn diso_ca(serial: i32, res_id: i32, x: f64) -> Atom {
        let mut a = sample_atom("CA", "C", x, 0.0, 0.0);
        a.serial = serial;
        a.res_id = res_id;
        a
    }

    fn disorder_gap_molecule() -> MolCoord {
        let mut molecule = MolCoord::with_atoms(vec![
            diso_ca(1, 1, 0.0),
            diso_ca(2, 2, 2.0),
            diso_ca(3, 3, 4.0),
            diso_ca(4, 4, 14.0),
            diso_ca(5, 5, 16.0),
            diso_ca(6, 6, 18.0),
        ]);
        for (atom1, atom2) in [(0, 1), (1, 2), (3, 4), (4, 5)] {
            molecule.add_bond(atom1, atom2, cuemol_molstr::BondOrder::Single);
        }
        molecule
    }

    #[test]
    fn gradient_color_interpolation_matches_cpp_integer_truncation() {
        let black = rgba_to_spec([0.0, 0.0, 0.0, 0.0]);
        let white = rgba_to_spec([1.0, 1.0, 1.0, 1.0]);
        let middle = interpolate_color(&black, &white, 0.5).rgba8().unwrap();
        assert_eq!(
            (middle.r, middle.g, middle.b, middle.a),
            (127, 127, 127, 127)
        );
    }

    #[test]
    fn disorder_lines_bridge_unbonded_chain_gaps_only() {
        // Both sides need a target-renderer spline, as in C++ getDiffVec().
        let mol = disorder_gap_molecule();
        let snap = disorder_lines_snapshot(&mol, 1.0, 0.2, 1.0, -1.0);
        assert!(!snap.atoms.is_empty(), "gap should be dotted");
        // ~10 dots (10 Å / 1 Å spacing), all between the two pivots.
        assert!(
            snap.atoms.len() >= 8 && snap.atoms.len() <= 12,
            "got {}",
            snap.atoms.len()
        );
        assert!(snap
            .atoms
            .iter()
            .all(|a| a.center.x > 3.9 && a.center.x < 14.1));

        let styled = disorder_lines_snapshot_with_style(
            &mol,
            &DisorderRenderOptions {
                dot_sep: 1.0,
                dot_radius: 0.2,
                loopsize: 1.0,
                detail: Some(9),
                color: Some(rgb_spec(0.2, 0.4, 0.6)),
                draw_indices: Some(vec![2, 3]),
                ..DisorderRenderOptions::default()
            },
        );
        assert!(styled.atoms.iter().all(|atom| atom.detail == Some(9)));
        assert!(styled.atoms.iter().all(|atom| {
            atom.color.as_ref().and_then(ColorSpec::rgba8)
                == Some(cuemol_gfx::Rgba8 {
                    r: 51,
                    g: 102,
                    b: 153,
                    a: 255,
                })
        }));
        assert!(disorder_lines_snapshot_with_style(
            &mol,
            &DisorderRenderOptions {
                dot_sep: 1.0,
                dot_radius: 0.2,
                loopsize: 1.0,
                draw_indices: Some(vec![2]),
                ..DisorderRenderOptions::default()
            },
        )
        .atoms
        .is_empty());

        // Once the residues are bonded (linked), there is no disorder line.
        let mut linked = disorder_gap_molecule();
        linked.add_bond(2, 3, cuemol_molstr::BondOrder::Single);
        let snap2 = disorder_lines_snapshot(&linked, 1.0, 0.2, 1.0, -1.0);
        assert!(snap2.atoms.is_empty(), "linked residues are not disordered");

        let isolated = MolCoord::with_atoms(vec![diso_ca(1, 1, 0.0), diso_ca(2, 2, 10.0)]);
        assert!(disorder_lines_snapshot(&isolated, 1.0, 0.2, 1.0, -1.0)
            .atoms
            .is_empty());
    }

    #[test]
    fn disorder_dots_follow_target_pivot_and_endpoint_colors() {
        let base = disorder_gap_molecule();
        let mut atoms = Vec::new();
        for (index, atom) in base.atoms().iter().cloned().enumerate() {
            let mut nitrogen = atom.clone();
            nitrogen.serial += 100;
            nitrogen.name = "N".to_string();
            nitrogen.element = "N".to_string();
            nitrogen.position.y = if index < 3 {
                index as f64
            } else {
                (5 - index) as f64
            };
            atoms.extend([atom, nitrogen]);
        }
        let mut molecule = MolCoord::with_atoms(atoms);
        for (atom1, atom2) in [(0, 2), (2, 4), (6, 8), (8, 10)] {
            molecule.add_bond(atom1, atom2, cuemol_molstr::BondOrder::Single);
        }
        let mut colors = vec![rgb_spec(1.0, 1.0, 1.0); molecule.atom_count()];
        colors[5] = rgb_spec(1.0, 0.0, 0.0);
        colors[7] = rgb_spec(0.0, 0.0, 1.0);
        let snapshot = disorder_lines_snapshot_with_style(
            &molecule,
            &DisorderRenderOptions {
                pivot_atom: Some("N".to_string()),
                atom_colors: Some(colors),
                // Selecting any atom in both endpoint residues includes the gap;
                // it does not require selecting the target pivot atoms themselves.
                draw_indices: Some(vec![4, 6]),
                ..DisorderRenderOptions::default()
            },
        );
        assert!(!snapshot.atoms.is_empty());
        assert!(snapshot.atoms.iter().any(|atom| atom.center.y > 0.1));
        let first = snapshot.atoms.first().unwrap().color.as_ref().unwrap();
        let last = snapshot.atoms.last().unwrap().color.as_ref().unwrap();
        assert_ne!(first.rgba8(), last.rgba8());
    }

    #[test]
    fn disorder_loopsize_scales_the_gap_arc_amplitude() {
        // Three residues on each side form target splines around one unbonded gap. The
        // neighbours bend the Bézier tangents off the chord (+y), so a larger
        // loopsize bulges the disorder arc higher in +y.
        fn bent_ca(serial: i32, res_id: i32, x: f64, y: f64) -> Atom {
            let mut a = sample_atom("CA", "C", x, y, 0.0);
            a.serial = serial;
            a.res_id = res_id;
            a
        }
        let mut mol = MolCoord::with_atoms(vec![
            bent_ca(1, 1, -4.0, -2.0),
            bent_ca(2, 2, -2.0, -1.0),
            bent_ca(3, 3, 0.0, 0.0),
            bent_ca(4, 4, 10.0, 0.0),
            bent_ca(5, 5, 12.0, -1.0),
            bent_ca(6, 6, 14.0, -2.0),
        ]);
        for (atom1, atom2) in [(0, 1), (1, 2), (3, 4), (4, 5)] {
            mol.add_bond(atom1, atom2, cuemol_molstr::BondOrder::Single);
        }
        let max_y = |loopsize: f64| {
            disorder_lines_snapshot(&mol, 1.0, 0.2, loopsize, -1.0)
                .atoms
                .iter()
                .map(|a| a.center.y)
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let small = max_y(1.0);
        let large = max_y(3.0);
        assert!(
            large > small + 0.5,
            "larger loopsize should bulge the arc higher: {small} vs {large}"
        );

        let inherited = disorder_lines_snapshot(&mol, 1.0, 0.2, 2.0, -1.0);
        let zero = disorder_lines_snapshot(&mol, 1.0, 0.2, 2.0, 0.0);
        assert_eq!(
            inherited
                .atoms
                .iter()
                .map(|atom| atom.center)
                .collect::<Vec<_>>(),
            zero.atoms
                .iter()
                .map(|atom| atom.center)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn putty_modulates_tube_radius_by_bfactor() {
        fn ca_b(serial: i32, res_id: i32, x: f64, b: f32) -> Atom {
            let mut a = sample_atom("CA", "C", x, 0.0, 0.0);
            a.serial = serial;
            a.res_id = res_id;
            a.b_factor = b;
            a
        }
        // A backbone with B-factors rising along the chain (10..90, mean 45).
        let mol = MolCoord::with_atoms(vec![
            ca_b(1, 1, 0.0, 10.0),
            ca_b(2, 2, 3.0, 30.0),
            ca_b(3, 3, 6.0, 50.0),
            ca_b(4, 4, 9.0, 90.0),
        ]);

        // Putty OFF -> a uniform tube radius.
        let off = mainchain_trace_snapshot(&mol, &MoleculeRenderOptions::default());
        let r0 = off.atoms.first().unwrap().radius;
        assert!(
            off.atoms.iter().all(|a| (a.radius - r0).abs() < 1.0e-6),
            "putty off should be uniform"
        );

        // Putty SCALE on B-factor -> radius grows with B (thin low, thick high).
        let opts = MoleculeRenderOptions {
            putty: PuttyOptions {
                mode: PuttyMode::Scale,
                target: PuttyTarget::BFactor,
                loscl: 3.0,
                hiscl: 3.0,
            },
            ..MoleculeRenderOptions::default()
        };
        let on = mainchain_trace_snapshot(&mol, &opts);
        let first = on.atoms.first().unwrap().radius;
        let last = on.atoms.last().unwrap().radius;
        // Low-B end scaled by 1/loscl (~0.33), high-B end by hiscl (~3.0).
        assert!(
            first < r0,
            "low-B end should be thinner than the off radius"
        );
        assert!(
            last > first + 0.1,
            "high-B end should be thicker: {first} vs {last}"
        );
    }

    #[test]
    fn mainchain_radius_overrides_the_trace_tube_width() {
        let mol = MolCoord::with_atoms(vec![
            diso_ca(1, 1, 0.0),
            diso_ca(2, 2, 3.0),
            diso_ca(3, 3, 6.0),
        ]);
        // Default radius is derived from bond_radius (0.08*3 = 0.24).
        let def = mainchain_trace_snapshot(&mol, &MoleculeRenderOptions::default());
        assert!((def.atoms[0].radius - 0.24).abs() < 1.0e-6);
        // The SplineRenderer line_width override (radius = line_width/2) wins.
        let opts = MoleculeRenderOptions {
            mainchain_radius: Some(0.5),
            ..MoleculeRenderOptions::default()
        };
        let wide = mainchain_trace_snapshot(&mol, &opts);
        assert!(
            wide.atoms.iter().all(|a| (a.radius - 0.5).abs() < 1.0e-6),
            "explicit mainchain radius should be applied uniformly"
        );
    }

    #[test]
    fn na_base_sticks_pick_glycosidic_n_by_base_type() {
        fn na_atom(name: &str, res_id: i32, x: f64, y: f64) -> Atom {
            let mut a = sample_atom(name, "N", x, y, 0.0);
            a.res_id = res_id;
            a
        }
        let mol = MolCoord::with_atoms(vec![
            // Purine (res 1): has N9 -> glycosidic N9, far N1.
            na_atom("C1'", 1, 0.0, 0.0),
            na_atom("N9", 1, 1.0, 0.0),
            na_atom("N1", 1, 2.0, 0.0),
            // Pyrimidine (res 2): no N9 -> glycosidic N1, far N3.
            na_atom("C1'", 2, 0.0, 5.0),
            na_atom("N1", 2, 1.0, 5.0),
            na_atom("N3", 2, 2.0, 5.0),
        ]);
        let snap = na_base_sticks_snapshot(&mol, 0.3, false);
        // Two sticks per base (sugar->glycosidic, glycosidic->far).
        assert_eq!(snap.bonds.len(), 4);
        let has_stick = |fx: f64, fy: f64, tx: f64, ty: f64| {
            snap.bonds.iter().any(|b| {
                (b.from.x - fx).abs() < 1e-9
                    && (b.from.y - fy).abs() < 1e-9
                    && (b.to.x - tx).abs() < 1e-9
                    && (b.to.y - ty).abs() < 1e-9
            })
        };
        // Purine glycosidic stick goes to N9 (1,0); pyrimidine to N1 (1,5).
        assert!(has_stick(0.0, 0.0, 1.0, 0.0), "purine C1'->N9");
        assert!(has_stick(1.0, 0.0, 2.0, 0.0), "purine N9->N1");
        assert!(has_stick(0.0, 5.0, 1.0, 5.0), "pyrimidine C1'->N1");
        assert!(has_stick(1.0, 5.0, 2.0, 5.0), "pyrimidine N1->N3");
    }

    #[test]
    fn na_show_basepair_draws_ladder_rungs_between_paired_bases() {
        fn nt(name: &str, resn: &str, res_id: i32, x: f64, y: f64) -> Atom {
            let mut a = sample_atom(name, "N", x, y, 0.0);
            a.res_name = resn.to_string();
            a.res_id = res_id;
            a
        }
        // Adenine and uracil with two in-plane Watson-Crick contacts.
        let mut mol = MolCoord::with_atoms(vec![
            nt("C1'", "A", 1, -1.0, -1.0),
            nt("N9", "A", 1, 0.0, -1.0),
            nt("N1", "A", 1, 0.0, 1.0),
            nt("N6", "A", 1, 0.0, 0.0),
            nt("C4", "A", 1, 0.0, 0.0),
            nt("C5", "A", 1, 1.0, 0.0),
            nt("C8", "A", 1, 0.0, 1.0),
            nt("C1'", "U", 2, 4.0, -1.0),
            nt("N1", "U", 2, 3.0, -1.0),
            nt("N3", "U", 2, 3.0, 1.0),
            nt("O4", "U", 2, 3.0, 0.0),
            nt("C2", "U", 2, 3.0, 0.0),
            nt("C4", "U", 2, 4.0, 0.0),
            nt("C6", "U", 2, 3.0, 1.0),
        ]);
        // Before computing base pairs, show_basepair adds no rungs (just the
        // per-base sticks: 2 per residue).
        let base_only = na_base_sticks_snapshot(&mol, 0.3, true).bonds.len();
        assert_eq!(base_only, 4);

        assert_eq!(mol.calc_base_pair(5.0, 30.0), 1);
        let on = na_base_sticks_snapshot(&mol, 0.3, true);
        // The paired form is pivot->glyco, glyco->glyco, glyco->pivot.
        assert_eq!(on.bonds.len(), 3);
        assert!(
            on.bonds.iter().any(|b| {
                let (f, t) = ((b.from.x, b.from.y), (b.to.x, b.to.y));
                (f == (0.0, -1.0) && t == (3.0, -1.0)) || (f == (3.0, -1.0) && t == (0.0, -1.0))
            }),
            "rung connects the paired bases' glycosidic N atoms"
        );
        // With show_basepair off, the rung is gone.
        assert_eq!(
            na_base_sticks_snapshot(&mol, 0.3, false).bonds.len(),
            base_only
        );
    }

    #[test]
    fn na_renderer_supports_basepair_and_detailed_modes() {
        let names = [
            ("P", -2.0, 0.0),
            ("C1'", -1.0, 0.0),
            ("C2'", -1.0, -1.0),
            ("C3'", -2.0, -1.5),
            ("C4'", -2.5, -0.5),
            ("O4'", -2.0, 0.5),
            ("N9", 0.0, 0.0),
            ("C8", 0.5, 1.0),
            ("N7", 1.5, 1.0),
            ("C5", 2.0, 0.0),
            ("C4", 1.0, -0.5),
            ("N3", 1.0, -1.5),
            ("C2", 2.0, -2.0),
            ("N1", 3.0, -1.5),
            ("C6", 3.0, -0.5),
        ];
        let mol = MolCoord::with_atoms(
            names
                .iter()
                .map(|(name, x, y)| {
                    let mut atom = sample_atom(name, "C", *x, *y, 0.0);
                    atom.res_name = "A".to_string();
                    atom
                })
                .collect(),
        );

        let basepair = na_renderer_snapshot(
            &mol,
            &NARenderOptions {
                base_type: NARenderer::NAREND_BP,
                show_basepair: false,
                ..NARenderOptions::default()
            },
        );
        assert_eq!(basepair.bonds.len(), 1);
        assert_eq!(basepair.atoms.len(), 1);

        let detail1 = na_renderer_snapshot(
            &mol,
            &NARenderOptions {
                base_type: NARenderer::NAREND_DETAIL1,
                ..NARenderOptions::default()
            },
        );
        assert_eq!(detail1.meshes.len(), 2, "purine base has two rings");
        assert!(detail1.atoms.iter().all(|atom| atom.detail == Some(3)));

        let detail2 = na_renderer_snapshot(
            &mol,
            &NARenderOptions {
                base_type: NARenderer::NAREND_DETAIL2,
                ..NARenderOptions::default()
            },
        );
        assert_eq!(
            detail2.meshes.len(),
            3,
            "detail2 also draws the ribose ring"
        );
        assert!(detail2.atoms.len() > detail1.atoms.len());
    }

    #[test]
    fn ring_meshes_drawn_for_aromatic_residues() {
        // PHE's 6-membered ring (CG CD1 CE1 CZ CE2 CD2) as a hexagon in z=0.
        let names = ["CG", "CD1", "CE1", "CZ", "CE2", "CD2"];
        let mol = MolCoord::with_atoms(
            (0..6)
                .map(|k| {
                    let ang = std::f64::consts::TAU * k as f64 / 6.0;
                    let mut a = sample_atom(names[k], "C", ang.cos(), ang.sin(), 0.0);
                    a.res_name = "PHE".to_string();
                    a.res_id = 1;
                    a
                })
                .collect(),
        );
        let snap = ring_meshes_snapshot_with(&mol, 0.2, [0.2, 0.4, 0.6, 1.0], None);
        assert_eq!(snap.meshes.len(), 1, "one ring mesh for PHE");
        let m = &snap.meshes[0];
        assert_eq!(m.triangle_count(), 12, "two hexagon fans = 12 triangles");
        assert_eq!(m.vertices.len(), 14, "two copies of centroid + 6 atoms");
        assert!(m
            .vertices
            .iter()
            .all(|vertex| (vertex.position.z.abs() - 0.2).abs() < 1.0e-9));
        assert!(m.vertices[0].normal.z.abs() > 0.99);
        assert_eq!(m.vertices[0].color, [0.2, 0.4, 0.6, 1.0]);

        assert!(
            ring_meshes_snapshot_with(&mol, 0.2, [1.0; 4], Some(&[0, 1, 2, 3, 4]),)
                .meshes
                .is_empty()
        );

        // A non-aromatic residue yields no ring mesh.
        let mut ala = sample_atom("CA", "C", 0.0, 0.0, 0.0);
        ala.res_name = "ALA".to_string();
        let ala_mol = MolCoord::with_atoms(vec![ala]);
        assert!(ring_meshes_snapshot(&ala_mol).meshes.is_empty());
    }

    #[test]
    fn valence_wedge_meshes_drawn_for_stereo_bonds() {
        let mut mol = MolCoord::with_atoms(vec![
            sample_atom("C", "C", 0.0, 0.0, 0.0),
            sample_atom("C", "C", 1.0, 0.0, 0.0),
        ]);
        // An MDL up-wedge bond (stereo = 1).
        mol.add_bond_stereo(0, 1, cuemol_molstr::BondOrder::Single, 1);
        let snap = valence_wedge_meshes_snapshot(&mol, 0.4);
        assert_eq!(snap.meshes.len(), 1);
        let m = &snap.meshes[0];
        assert_eq!(m.triangle_count(), 1);
        assert_eq!(m.vertices.len(), 3);
        // Apex at atom1 (0,0,0); the base straddles atom2 (1,0,0).
        assert_eq!(m.vertices[0].position, Vector4::new3(0.0, 0.0, 0.0));
        let base_mid = (m.vertices[1].position + m.vertices[2].position).scale(0.5);
        assert!((base_mid - Vector4::new3(1.0, 0.0, 0.0)).length3d() < 1.0e-9);

        // A plain (non-stereo) bond produces no wedge.
        let mut plain = MolCoord::with_atoms(vec![
            sample_atom("C", "C", 0.0, 0.0, 0.0),
            sample_atom("C", "C", 1.0, 0.0, 0.0),
        ]);
        plain.add_bond(0, 1, cuemol_molstr::BondOrder::Single);
        assert!(valence_wedge_meshes_snapshot(&plain, 0.4).meshes.is_empty());

        let mut hashed = MolCoord::with_atoms(vec![
            sample_atom("C", "C", 0.0, 0.0, 0.0),
            sample_atom("C", "C", 1.0, 0.0, 0.0),
        ]);
        hashed.add_bond_stereo(0, 1, cuemol_molstr::BondOrder::Single, 6);
        let hashed = valence_wedge_meshes_snapshot(&hashed, 0.4);
        assert_eq!(hashed.meshes.len(), 1);
        assert_eq!(hashed.meshes[0].triangle_count(), 12);
        assert_eq!(hashed.meshes[0].vertices.len(), 24);
    }

    #[test]
    fn simple_valence_bonds_match_reference_offsets() {
        let mut double = MolCoord::with_atoms(vec![
            sample_atom("C1", "C", 0.0, 0.0, 0.0),
            sample_atom("C2", "C", 2.0, 0.0, 0.0),
            sample_atom("C3", "C", 0.0, 1.0, 0.0),
        ]);
        double.add_bond(0, 1, BondOrder::Double);
        double.add_bond(0, 2, BondOrder::Single);
        let options = MoleculeRenderOptions {
            draw_atoms: false,
            valence_bond_mode: ValenceBondMode::Simple,
            ..MoleculeRenderOptions::default()
        };
        let snapshot = molecule_render_snapshot(&double, &options);
        assert_eq!(snapshot.bonds.len(), 3);
        let multiple = &snapshot.bonds[..2];
        assert!((multiple[0].from.y - 0.1).abs() < 1.0e-12);
        assert!((multiple[0].to.y - 0.1).abs() < 1.0e-12);
        assert!(multiple[1].from.y.abs() < 1.0e-12);
        assert!(multiple[1].to.y.abs() < 1.0e-12);

        let mut triple = MolCoord::with_atoms(double.atoms().to_vec());
        triple.add_bond(0, 1, BondOrder::Triple);
        triple.add_bond(0, 2, BondOrder::Single);
        let snapshot = molecule_render_snapshot(&triple, &options);
        assert_eq!(snapshot.bonds.len(), 4);
        let mut offsets = snapshot.bonds[..3]
            .iter()
            .map(|bond| bond.from.y)
            .collect::<Vec<_>>();
        offsets.sort_by(f64::total_cmp);
        assert_eq!(offsets, vec![-0.1, 0.0, 0.1]);
    }

    #[test]
    fn ballstick_type1_matches_reference_secondary_cylinder() {
        let mut molecule = MolCoord::with_atoms(vec![
            sample_atom("C1", "C", 0.0, 0.0, 0.0),
            sample_atom("C2", "C", 2.0, 0.0, 0.0),
            sample_atom("C3", "C", 0.0, 1.0, 0.0),
        ]);
        molecule.add_bond(0, 1, BondOrder::Double);
        molecule.add_bond(0, 2, BondOrder::Single);
        let options = MoleculeRenderOptions {
            draw_atoms: false,
            bond_radius: 0.2,
            atom_radius_abs: Some(0.3),
            valence_bond_mode: ValenceBondMode::BallStickType1,
            ..MoleculeRenderOptions::default()
        };
        let snapshot = molecule_render_snapshot(&molecule, &options);
        assert_eq!(snapshot.bonds.len(), 3);
        assert_eq!(snapshot.atoms.len(), 2);
        let secondary = &snapshot.bonds[1];
        assert!((secondary.from - Vector4::new3(0.6, 0.5, 0.0)).length3d() < 1.0e-6);
        assert!((secondary.to - Vector4::new3(1.4, 0.5, 0.0)).length3d() < 1.0e-6);
        assert_eq!(snapshot.atoms[0].center, secondary.from);
        assert_eq!(snapshot.atoms[1].center, secondary.to);
    }

    #[test]
    fn anisou_snapshot_builds_ellipsoids_for_anisou_atoms() {
        let mut mol = MolCoord::with_atoms(vec![
            sample_atom("N", "N", 0.0, 0.0, 0.0),
            sample_atom("CA", "C", 2.0, 0.0, 0.0),
        ]);
        // Atom 0 carries an ANISOU record (the 6 tensor components); atom 1 does not.
        for (name, value) in [
            ("anisou_u11", 1111),
            ("anisou_u22", 2222),
            ("anisou_u33", 3333),
            ("anisou_u12", 444),
            ("anisou_u13", 555),
            ("anisou_u23", 666),
        ] {
            mol.atoms_mut()[0].set_int_prop(name, value);
        }
        let snap = anisou_mesh_snapshot(&mol, &MoleculeRenderOptions::default());
        // Atom 0 (with ANISOU) -> an ellipsoid mesh; atom 1 -> a sphere primitive.
        assert_eq!(snap.meshes.len(), 1);
        assert_eq!(snap.atoms.len(), 1);
        assert!(snap.meshes[0].triangle_count() > 0);
    }

    #[test]
    fn anisou_snapshot_honors_renderer_options_and_b_factor_radius() {
        let mut ellipsoid = sample_atom("N", "N", 0.0, 0.0, 0.0);
        for (name, value) in [
            ("anisou_u11", 100),
            ("anisou_u22", 100),
            ("anisou_u33", 100),
            ("anisou_u12", 0),
            ("anisou_u13", 0),
            ("anisou_u23", 0),
        ] {
            ellipsoid.set_int_prop(name, value);
        }
        let mut isotropic = sample_atom("CA", "C", 2.0, 0.0, 0.0);
        isotropic.b_factor = 8.0 * std::f32::consts::PI * std::f32::consts::PI;
        let molecule = MolCoord::with_atoms(vec![ellipsoid, isotropic]);
        let options = MoleculeRenderOptions {
            atom_radius_abs: Some(0.5),
            sphere_detail: Some(2),
            ..MoleculeRenderOptions::default()
        };
        let without_discs = anisou_mesh_snapshot_with_options(
            &molecule,
            &options,
            &AnIsoURenderOptions {
                draw_discs: false,
                ..AnIsoURenderOptions::default()
            },
        );
        let with_discs =
            anisou_mesh_snapshot_with_options(&molecule, &options, &AnIsoURenderOptions::default());

        assert_eq!(without_discs.meshes.len(), 1);
        assert_eq!(without_discs.atoms.len(), 1);
        assert!((without_discs.atoms[0].radius - 0.5).abs() < 1.0e-6);
        let radius = without_discs.meshes[0]
            .vertices
            .iter()
            .map(|vertex| vertex.position.length3d())
            .fold(0.0_f64, f64::max);
        assert!((radius - 0.05).abs() < 1.0e-6);
        assert!(with_discs.meshes[0].triangle_count() > without_discs.meshes[0].triangle_count());
    }

    #[test]
    fn atom_intr_measures_distance_angle_and_signed_torsion() {
        let mol = MolCoord::with_atoms(vec![
            intr_atom(1, 0.0, 0.0, 0.0),
            intr_atom(2, 3.0, 4.0, 0.0), // distance 1-2 = 5
            intr_atom(3, 1.0, 0.0, 0.0),
            intr_atom(4, 0.0, 0.0, 0.0), // angle 3-4-5 vertex
            intr_atom(5, 0.0, 1.0, 0.0), // angle = 90 deg
            intr_atom(6, 0.0, 1.0, 0.0),
            intr_atom(7, 0.0, 0.0, 0.0),
            intr_atom(8, 1.0, 0.0, 0.0),
            intr_atom(9, 1.0, 0.0, 1.0),   // torsion 6-7-8-9 = +90 deg
            intr_atom(10, 1.0, 0.0, -1.0), // torsion 6-7-8-10 = -90 deg
        ]);

        let mut r = AtomIntrRenderer::new();
        let dist = r.append_by_id(1, 0, 2, true);
        let angle = r.append_angle_by_id(3, 0, 4, 0, 5);
        let tors_pos = r.append_torsion_by_id(6, 0, 7, 0, 8, 0, 9);
        let tors_neg = r.append_torsion_by_id(6, 0, 7, 0, 8, 0, 10);
        // An unresolvable atom id yields no measurement.
        let missing = r.append_by_id(1, 0, 999, true);

        let value = |id: i32| {
            let def = r.defs().iter().find(|d| d.id() == id).unwrap();
            r.measure_value(def, &mol)
        };

        assert!((value(dist).unwrap() - 5.0).abs() < 1e-9);
        assert!((value(angle).unwrap() - 90.0).abs() < 1e-9);
        // The torsion is SIGNED — the sign convention matches C++ Vector4D::torsion
        // (verified against VectorHelper.cpp). Pinning both +90 and -90 guards the
        // sign, not just the magnitude.
        assert!((value(tors_pos).unwrap() - 90.0).abs() < 1e-9);
        assert!((value(tors_neg).unwrap() + 90.0).abs() < 1e-9);
        assert_eq!(value(missing), None);
    }

    #[test]
    fn atom_intr_measurement_lines_connect_the_measured_atoms() {
        let mol = MolCoord::with_atoms(vec![
            intr_atom(1, 0.0, 0.0, 0.0),
            intr_atom(2, 3.0, 0.0, 0.0),
            intr_atom(3, 0.0, 0.0, 0.0),
            intr_atom(4, 1.0, 0.0, 0.0),
            intr_atom(5, 1.0, 1.0, 0.0),
        ]);
        let mut r = AtomIntrRenderer::new();
        r.append_by_id(1, 0, 2, true); // distance -> 1 segment
        r.append_angle_by_id(3, 0, 4, 0, 5); // angle -> 2 arm segments
        let lines = r.measurement_lines(&mol);
        assert_eq!(lines.len(), 3, "1 distance segment + 2 angle arms");
        // The distance segment spans atoms 1 (0,0,0) and 2 (3,0,0).
        assert!(lines
            .iter()
            .any(|(a, b)| (a.x == 0.0 && b.x == 3.0) || (a.x == 3.0 && b.x == 0.0)));
    }

    #[test]
    fn atom_intr_selection_points_use_selected_atom_centroids() {
        let mut n1 = intr_atom(1, 0.0, 0.0, 0.0);
        n1.name = "N".to_string();
        let mut n2 = intr_atom(2, 2.0, 0.0, 0.0);
        n2.name = "N".to_string();
        let mut ca1 = intr_atom(3, 1.0, 2.0, 0.0);
        ca1.name = "CA".to_string();
        let mut ca2 = intr_atom(4, 1.0, 4.0, 0.0);
        ca2.name = "CA".to_string();
        let mol = MolCoord::with_atoms(vec![n1, n2, ca1, ca2]);

        let mut renderer = AtomIntrRenderer::new();
        let id = renderer.append("name N", "name CA");
        let def = &renderer.defs()[id as usize];
        assert!((renderer.measure_value(def, &mol).unwrap() - 3.0).abs() < 1.0e-9);
        assert_eq!(
            renderer.measurement_lines(&mol),
            vec![(Vector4::new3(1.0, 0.0, 0.0), Vector4::new3(1.0, 3.0, 0.0))]
        );

        let mut invalid = AtomIntrRenderer::new();
        assert_eq!(invalid.append("(", "name CA"), -1);
        assert!(invalid.defs().is_empty());
    }

    #[test]
    fn atom_intr_angle_arc_is_a_circular_sweep_at_the_vertex() {
        let mol = MolCoord::with_atoms(vec![
            intr_atom(1, 2.0, 0.0, 0.0), // arm 0 along +x
            intr_atom(2, 0.0, 0.0, 0.0), // vertex
            intr_atom(3, 0.0, 2.0, 0.0), // arm 2 along +y
        ]);
        let mut r = AtomIntrRenderer::new();
        r.append_angle_by_id(1, 0, 2, 0, 3);
        let arcs = r.measurement_arcs(&mol, 0.5, 8);
        assert_eq!(arcs.len(), 1);
        let arc = &arcs[0];
        assert_eq!(arc.len(), 9);
        // radius = 0.5 * min arm length (2) = 1; every arc point is at distance 1
        // from the vertex (0,0,0), sweeping +x -> +y.
        for pt in arc {
            let d = (pt.x * pt.x + pt.y * pt.y + pt.z * pt.z).sqrt();
            assert!((d - 1.0).abs() < 1.0e-9, "arc point off radius: {d}");
        }
        assert!((arc[0].x - 1.0).abs() < 1.0e-9 && arc[0].y.abs() < 1.0e-9);
        assert!((arc[8].y - 1.0).abs() < 1.0e-9 && arc[8].x.abs() < 1.0e-9);
        // Distance defs have no arc.
        let mut r2 = AtomIntrRenderer::new();
        r2.append_by_id(1, 0, 2, true);
        assert!(r2.measurement_arcs(&mol, 0.5, 8).is_empty());
    }

    #[test]
    fn molecule_render_snapshot_contains_atoms_bonds_bounds_and_selection() {
        let mut mol = MolCoord::with_atoms(vec![
            sample_atom("CA", "C", 1.0, 2.0, 3.0),
            sample_atom("N", "N", 2.0, 2.0, 3.0),
        ]);
        mol.add_bond(0, 1, cuemol_molstr::BondOrder::Single);
        let snapshot = molecule_render_snapshot(
            &mol,
            &MoleculeRenderOptions {
                object_uid: Some(7),
                selected_indices: vec![1],
                ..MoleculeRenderOptions::default()
            },
        );

        assert_eq!(snapshot.atom_count(), 2);
        assert_eq!(snapshot.bond_count(), 1);
        assert_eq!(snapshot.bounds, mol.bounding_box_for(None));
        assert_eq!(snapshot.atoms[0].object_uid, Some(7));
        assert!(!snapshot.atoms[0].selected);
        assert!(snapshot.atoms[1].selected);
        assert!(snapshot.bonds[0].selected);

        let atoms_only = molecule_render_snapshot(
            &mol,
            &MoleculeRenderOptions {
                draw_bonds: false,
                ..MoleculeRenderOptions::default()
            },
        );
        assert_eq!(atoms_only.atom_count(), 2);
        assert_eq!(atoms_only.bond_count(), 0);

        let bonds_only = molecule_render_snapshot(
            &mol,
            &MoleculeRenderOptions {
                draw_atoms: false,
                ..MoleculeRenderOptions::default()
            },
        );
        assert_eq!(bonds_only.atom_count(), 0);
        assert_eq!(bonds_only.bond_count(), 1);
    }

    fn ca_atom(res_id: i32, x: f64, y: f64, z: f64) -> Atom {
        let mut atom = sample_atom("CA", "C", x, y, z);
        atom.res_id = res_id;
        atom
    }

    #[test]
    fn simple_line_snapshot_matches_valence_midpoint_and_aster_geometry() {
        let mut molecule = MolCoord::with_atoms(vec![
            sample_atom("C1", "C", 0.0, 0.0, 0.0),
            sample_atom("N1", "N", 1.0, 0.0, 0.0),
            sample_atom("O1", "O", 3.0, 0.0, 0.0),
        ]);
        molecule.add_bond(0, 1, BondOrder::Double);
        let snapshot = simple_line_snapshot(
            &molecule,
            &MoleculeRenderOptions {
                object_uid: Some(9),
                valence_bond_mode: ValenceBondMode::Simple,
                ..MoleculeRenderOptions::default()
            },
            2.4,
        );

        assert_eq!(snapshot.atom_count(), 0);
        assert_eq!(snapshot.mesh_count(), 0);
        assert_eq!(snapshot.bond_count(), 7);
        assert!(snapshot
            .bonds
            .iter()
            .all(|bond| bond.screen_width == Some(2.4)));
        assert_eq!(
            snapshot
                .bonds
                .iter()
                .filter(|bond| bond.atom_indices == Some((2, 2)))
                .count(),
            3,
            "the unbonded atom must be a three-axis aster"
        );
        for bond in snapshot
            .bonds
            .iter()
            .filter(|bond| bond.atom_indices == Some((0, 1)))
        {
            assert_eq!(bond.color, bond.end_color);
            assert!(
                ((bond.to - bond.from).length3d() - 0.5).abs() < 1.0e-9,
                "different atom colors must split each line into equal halves"
            );
        }
    }

    #[test]
    fn selection_overlay_matches_stick_point_offset_and_isolated_atom_geometry() {
        let mut molecule = MolCoord::with_atoms(vec![
            sample_atom("C1", "C", 0.0, 0.0, 0.0),
            sample_atom("C2", "C", 1.0, 0.0, 0.0),
            sample_atom("N1", "N", 3.0, 0.0, 0.0),
        ]);
        molecule.add_bond(0, 1, BondOrder::Single);
        let options = SelectionOverlayOptions {
            line_width: 4.0,
            displacement: [5.0, -2.0],
            object_uid: Some(12),
            ..SelectionOverlayOptions::default()
        };
        let stick = selection_overlay_snapshot(&molecule, &[0, 1, 2], &options);
        assert_eq!(stick.atom_count(), 0);
        assert_eq!(stick.bond_count(), 4);
        assert!(stick.bonds.iter().all(|primitive| {
            primitive.screen_width == Some(4.0)
                && primitive.screen_offset == [5.0, -2.0]
                && !primitive.screen_point
                && primitive.object_uid == Some(12)
        }));
        assert_eq!(
            stick
                .bonds
                .iter()
                .filter(|primitive| primitive.atom_indices == Some((2, 2)))
                .count(),
            3
        );

        let points = selection_overlay_snapshot(
            &molecule,
            &[0, 2],
            &SelectionOverlayOptions {
                mode: SelectionOverlayMode::Point,
                ..options
            },
        );
        assert_eq!(points.bond_count(), 2);
        assert!(points
            .bonds
            .iter()
            .all(|primitive| primitive.screen_point && primitive.from == primitive.to));
    }

    #[test]
    fn mainchain_line_snapshot_distinguishes_trace_spline_and_isolated_residue() {
        let molecule = MolCoord::with_atoms(vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 3.0, 2.0, 0.0),
            ca_atom(3, 6.0, 0.0, 0.0),
        ]);
        let options = MoleculeRenderOptions::default();
        let trace = mainchain_line_snapshot(
            &molecule,
            &options,
            MainchainLineMode::Trace,
            1.2,
            1,
            0.0,
            true,
        );
        assert_eq!(trace.bond_count(), 2);
        assert_eq!(trace.mesh_count(), 0);
        assert!(trace
            .bonds
            .iter()
            .all(|bond| bond.screen_width == Some(1.2)));

        let spline = mainchain_line_snapshot(
            &molecule,
            &options,
            MainchainLineMode::Spline,
            1.8,
            4,
            0.0,
            false,
        );
        assert_eq!(spline.bond_count(), 8);
        assert_eq!(spline.mesh_count(), 0);
        assert!(spline.bonds.iter().all(|bond| bond.color == bond.end_color));

        let isolated = MolCoord::with_atoms(vec![ca_atom(1, 0.0, 0.0, 0.0)]);
        let isolated_trace = mainchain_line_snapshot(
            &isolated,
            &options,
            MainchainLineMode::Trace,
            1.2,
            1,
            0.0,
            true,
        );
        assert_eq!(isolated_trace.bond_count(), 3);
        let isolated_spline = mainchain_line_snapshot(
            &isolated,
            &options,
            MainchainLineMode::Spline,
            1.2,
            6,
            0.0,
            true,
        );
        assert_eq!(isolated_spline.bond_count(), 0);
        let two_residue_spline = mainchain_line_snapshot(
            &MolCoord::with_atoms(vec![ca_atom(1, 0.0, 0.0, 0.0), ca_atom(2, 3.0, 0.0, 0.0)]),
            &options,
            MainchainLineMode::Spline,
            1.2,
            6,
            0.0,
            true,
        );
        assert_eq!(two_residue_spline.bond_count(), 0);
    }

    #[test]
    fn mainchain_trace_snapshot_splines_pivot_atoms_into_a_tube() {
        // Four CA pivots, one per residue, laid out along a line.
        let mol = MolCoord::with_atoms(vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 1.0, 0.0, 0.0),
            ca_atom(3, 2.0, 0.0, 0.0),
            ca_atom(4, 3.0, 0.0, 0.0),
        ]);
        let trace = mainchain_trace_snapshot(&mol, &MoleculeRenderOptions::default());
        // 4 control points, SUBDIV=8 -> 3*8 + 1 = 25 spline samples (spheres),
        // connected by 24 segments.
        assert_eq!(trace.atom_count(), 25);
        assert_eq!(trace.bond_count(), 24);
        assert!(trace.bounds.is_some());
    }

    #[test]
    fn mainchain_trace_snapshot_uses_only_pivot_atoms() {
        // Side-chain atoms must not change the backbone trace.
        let backbone_only =
            MolCoord::with_atoms(vec![ca_atom(1, 0.0, 0.0, 0.0), ca_atom(2, 1.0, 0.0, 0.0)]);
        let mut cb1 = sample_atom("CB", "C", 0.0, 1.0, 0.0);
        cb1.res_id = 1;
        let mut cb2 = sample_atom("CB", "C", 1.0, 1.0, 0.0);
        cb2.res_id = 2;
        let with_sidechains = MolCoord::with_atoms(vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            cb1,
            ca_atom(2, 1.0, 0.0, 0.0),
            cb2,
        ]);
        let opts = MoleculeRenderOptions::default();
        assert_eq!(
            mainchain_trace_snapshot(&backbone_only, &opts).atom_count(),
            mainchain_trace_snapshot(&with_sidechains, &opts).atom_count()
        );
    }

    #[test]
    fn mainchain_trace_snapshot_handles_single_residue() {
        let mol = MolCoord::with_atoms(vec![ca_atom(1, 0.0, 0.0, 0.0)]);
        let trace = mainchain_trace_snapshot(&mol, &MoleculeRenderOptions::default());
        assert_eq!(trace.atom_count(), 1);
        assert_eq!(trace.bond_count(), 0);
    }

    #[test]
    fn trace_runs_straight_through_pivots_unlike_the_smooth_spline() {
        // C++ TraceRenderer draws a Cα line strip — straight segments between
        // pivots, NOT a smoothed spline (which `tube`/`spline`/`ribbon` use). On a
        // turning backbone the spline bulges off the chords; the linear trace does
        // not. Discriminating oracle (advisor-steered): straightness, not face count
        // (the tube representation has triangles either way).
        let coords = [
            (0.0, 0.0, 0.0),
            (4.0, 0.0, 0.0),
            (8.0, 4.0, 0.0),
            (12.0, 4.0, 0.0),
        ];
        let pivots: Vec<MainchainPoint> = coords
            .iter()
            .enumerate()
            .map(|(i, &(x, y, z))| MainchainPoint {
                pos: Vector4::new3(x, y, z),
                ss: Secondary::Coil,
                b_factor: 0.0,
                occupancy: 1.0,
                atom_index: i,
            })
            .collect();
        let positions: Vec<Vector4> = pivots.iter().map(|p| p.pos).collect();
        let subdiv = 8usize;
        let dist = |a: &Vector4, b: &Vector4| {
            ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
        };
        let max_chord_dev = |samples: &[Vector4]| {
            let mut max = 0.0_f64;
            for i in 0..positions.len() - 1 {
                for step in 0..subdiv {
                    let t = step as f64 / subdiv as f64;
                    let chord = positions[i].scale(1.0 - t) + positions[i + 1].scale(t);
                    max = max.max(dist(&samples[i * subdiv + step], &chord));
                }
            }
            max
        };

        let (linear, _, _) = sample_backbone_test(&pivots, subdiv, 0.0, true);
        let (spline, _, _) = sample_backbone_test(&pivots, subdiv, 0.0, false);
        // Same sample count -> a drop-in path swap, the tube machinery is unchanged.
        assert_eq!(linear.len(), spline.len());
        // Linear trace samples lie exactly on the pivot chords.
        assert!(
            max_chord_dev(&linear) < 1.0e-9,
            "linear trace must stay on the pivot chords (dev {})",
            max_chord_dev(&linear)
        );
        // The smooth spline bulges measurably off the chords (the distinction).
        assert!(
            max_chord_dev(&spline) > 0.1,
            "the spline must deviate from the straight chords (dev {})",
            max_chord_dev(&spline)
        );
        // Both still pass through the real Cα pivots (endpoints coincide).
        assert!(dist(&linear[0], &positions[0]) < 1.0e-9);
        assert!(dist(&linear[subdiv], &positions[1]) < 1.0e-9);
    }

    #[test]
    fn backbone_runs_break_at_chain_gaps_but_not_contiguous_ca() {
        // C++ MainChainRenderer::isNewSegment breaks a backbone segment at chain
        // boundaries AND chemical-link gaps. The Rust path used to break ONLY on
        // chain name, so a chain with an internal break got one spline bridging the
        // gap. Oracle: count runs from collect_mainchain_pivots.
        // (1) Contiguous Ca-only chain stays ONE run (anti-regression: a bond-based
        // break would vanish this, since Ca-only structures form no peptide bonds).
        let contiguous: Vec<Atom> = (1..=5)
            .map(|i| ca_atom(i, (i - 1) as f64 * 3.8, 0.0, 0.0))
            .collect();
        let mol = MolCoord::with_atoms(contiguous);
        let runs = collect_mainchain_pivots(&mol, None, None);
        assert_eq!(runs.len(), 1, "a contiguous Ca-only chain is one run");
        assert_eq!(runs[0].len(), 5);

        // (2) A single chain with an internal coordinate GAP breaks into two runs;
        // no run bridges residues 3 and 50 (42 A apart >> the 5 A Ca cutoff).
        let mut gapped: Vec<Atom> = vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 3.8, 0.0, 0.0),
            ca_atom(3, 7.6, 0.0, 0.0),
            ca_atom(50, 50.0, 0.0, 0.0),
            ca_atom(51, 53.8, 0.0, 0.0),
        ];
        // keep them all chain A so only the gap (not a chain change) splits them.
        for atom in &mut gapped {
            atom.chain_id = "A".to_string();
        }
        let runs = collect_mainchain_pivots(&MolCoord::with_atoms(gapped), None, None);
        assert_eq!(runs.len(), 2, "an internal backbone gap splits the run");
        assert_eq!(runs[0].len(), 3);
        assert_eq!(runs[1].len(), 2);

        // (3) Distinct chains always break, even when spatially close.
        let mut two_chain = vec![ca_atom(1, 0.0, 0.0, 0.0), ca_atom(2, 3.8, 0.0, 0.0)];
        let mut b1 = ca_atom(1, 7.6, 0.0, 0.0);
        let mut b2 = ca_atom(2, 11.4, 0.0, 0.0);
        b1.chain_id = "B".to_string();
        b2.chain_id = "B".to_string();
        two_chain.push(b1);
        two_chain.push(b2);
        let runs = collect_mainchain_pivots(&MolCoord::with_atoms(two_chain), None, None);
        assert_eq!(runs.len(), 2, "two chains are two runs");
    }

    #[test]
    fn ribbon_anchor_weight_smooths_the_spline_toward_neighbor_midpoints() {
        // A backbone with a single sharp peak (pivot 2 at y=3) on an otherwise
        // flat chain. Anchor smoothing blends the peak toward its neighbours'
        // midpoint (y=0), so the spline's peak height drops as the weight rises.
        let pivots: Vec<MainchainPoint> = [
            (0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (2.0, 3.0, 0.0),
            (3.0, 0.0, 0.0),
            (4.0, 0.0, 0.0),
        ]
        .iter()
        .map(|&(x, y, z)| MainchainPoint {
            pos: Vector4::new3(x, y, z),
            ss: Secondary::Coil,
            b_factor: 0.0,
            occupancy: 1.0,
            atom_index: 0,
        })
        .collect();
        let max_y = |w: f64| {
            sample_backbone_test(&pivots, 8, w, false)
                .0
                .iter()
                .map(|p| p.y)
                .fold(0.0_f64, f64::max)
        };
        let sharp = max_y(0.0);
        let smoothed = max_y(1.0);
        assert!(
            smoothed < sharp - 0.3,
            "anchor smoothing should flatten the zigzag: {sharp} vs {smoothed}"
        );
    }

    #[test]
    fn ribbon_sections_are_configurable() {
        // section_params multiplies the base radius by the configured per-SS
        // width/thickness factors (RibbonRenderer TubeSection).
        let custom = RibbonSections {
            helix_width: 5.0,
            helix_thick: 0.1,
            ..RibbonSections::default()
        };
        let styled = StyledRibbonSections::default();
        let (w, t, _) = section_params(Secondary::Helix, 1.0, &custom, &styled);
        assert_eq!(w, 5.0);
        assert_eq!(t, 0.1);
        // The default keeps the historical flat helix band (2.2 x 0.7).
        let (dw, dt, _) =
            section_params(Secondary::Helix, 1.0, &RibbonSections::default(), &styled);
        assert_eq!(dw, 2.2);
        assert_eq!(dt, 0.7);
        // Sheet/coil factors are independent.
        assert_eq!(
            section_params(Secondary::Coil, 2.0, &RibbonSections::default(), &styled).0,
            2.0
        );
        // A styled <helix> override replaces the factor model with absolute dims.
        let abs = StyledRibbonSections {
            helix: Some((1.2, 0.2)),
            ..StyledRibbonSections::default()
        };
        let (sw, st, _) = section_params(Secondary::Helix, 9.0, &custom, &abs);
        assert_eq!((sw, st), (1.2, 0.2)); // ignores scale + factors when styled
    }

    #[test]
    fn ribbon_mesh_snapshot_builds_tube_geometry() {
        let mol = MolCoord::with_atoms(vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 3.0, 0.0, 0.0),
            ca_atom(3, 6.0, 0.0, 0.0),
            ca_atom(4, 9.0, 0.0, 0.0),
        ]);
        let snap = ribbon_mesh_snapshot(&mol, &MoleculeRenderOptions::default());
        // Tube geometry lives in meshes, not in atoms/bonds.
        assert_eq!(snap.atom_count(), 0);
        assert_eq!(snap.bond_count(), 0);
        assert_eq!(snap.mesh_count(), 1);
        // 4 pivots, SUBDIV=8 -> 25 ring samples; each of the 24 ring gaps emits
        // TUBE_RADIAL quads = 2 triangles.
        assert_eq!(snap.meshes[0].triangle_count(), 24 * TUBE_RADIAL * 2);
        assert_eq!(snap.meshes[0].vertices.len(), 25 * TUBE_RADIAL);
        // Vertex normals must be unit length for correct lighting.
        for vertex in &snap.meshes[0].vertices {
            assert!((vertex.normal.length3d() - 1.0).abs() < 1.0e-5);
        }
        assert!(snap.bounds.is_some());
    }

    #[test]
    fn tube_mesh_honors_pivot_radial_detail_and_caps() {
        let mut atoms = Vec::new();
        for (residue, x) in [(1, 0.0), (2, 3.0), (3, 6.0)] {
            atoms.push(ca_atom(residue, x, 0.0, 0.0));
            let mut nitrogen = ca_atom(residue, x, 2.0, 0.0);
            nitrogen.name = "N".to_string();
            nitrogen.element = "N".to_string();
            atoms.push(nitrogen);
        }
        let molecule = MolCoord::with_atoms(atoms);
        let mut options = MoleculeRenderOptions {
            pivot_atom: Some("N".to_string()),
            ribbon_axial_detail: Some(2),
            ribbon_radial_detail: Some(9),
            start_cap_type: TubeCapType::Flat,
            end_cap_type: TubeCapType::Flat,
            uniform_backbone: true,
            ..MoleculeRenderOptions::default()
        };
        options.styled_ribbon.helix = Some((0.4, 0.4));
        options.styled_ribbon.sheet = Some((0.4, 0.4));
        options.styled_ribbon.coil = Some((0.4, 0.4));
        let snapshot = ribbon_mesh_snapshot(&molecule, &options);
        let mesh = &snapshot.meshes[0];
        assert_eq!(mesh.vertices.len(), 5 * 9 + 2);
        assert_eq!(mesh.triangle_count(), 4 * 9 * 2 + 2 * 9);
        let first_center = mesh.vertices[..9]
            .iter()
            .fold(Vector4::new3(0.0, 0.0, 0.0), |sum, vertex| {
                sum + vertex.position
            })
            .scale(1.0 / 9.0);
        assert!((first_center.y - 2.0).abs() < 1.0e-6);

        options.pivot_atom = Some("MISSING".to_string());
        let fallback = ribbon_mesh_snapshot(&molecule, &options);
        let fallback_center = fallback.meshes[0].vertices[..9]
            .iter()
            .fold(Vector4::new3(0.0, 0.0, 0.0), |sum, vertex| {
                sum + vertex.position
            })
            .scale(1.0 / 9.0);
        assert!(fallback_center.y.abs() < 1.0e-6);
    }

    #[test]
    fn ribbon_mesh_snapshot_skips_single_residue_chain() {
        let mol = MolCoord::with_atoms(vec![ca_atom(1, 0.0, 0.0, 0.0)]);
        let snap = ribbon_mesh_snapshot(&mol, &MoleculeRenderOptions::default());
        assert_eq!(snap.mesh_count(), 0);
    }

    #[test]
    fn ribbon_mesh_snapshot_colors_by_secondary_structure() {
        let mut mol = MolCoord::with_atoms(vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 3.0, 0.0, 0.0),
            ca_atom(3, 6.0, 0.0, 0.0),
            ca_atom(4, 9.0, 0.0, 0.0),
        ]);
        for res_id in 1..=4 {
            mol.set_residue_prop_str("A", &res_id.to_string(), "secondary", "helix")
                .unwrap();
        }
        let snap = ribbon_mesh_snapshot(&mol, &MoleculeRenderOptions::default());
        assert_eq!(snap.mesh_count(), 1);
        // Every vertex carries the helix color and a unit normal.
        let helix_color = section_params(
            Secondary::Helix,
            1.0,
            &RibbonSections::default(),
            &StyledRibbonSections::default(),
        )
        .2;
        assert!(snap.meshes[0]
            .vertices
            .iter()
            .all(|vertex| vertex.color == helix_color));
        assert!(snap.meshes[0]
            .vertices
            .iter()
            .all(|vertex| (vertex.normal.length3d() - 1.0).abs() < 1.0e-5));
        // A helix band is wider than it is thick (flat ribbon, not a tube).
        let (width, thick, _) = section_params(
            Secondary::Helix,
            1.0,
            &RibbonSections::default(),
            &StyledRibbonSections::default(),
        );
        assert!(width > thick * 2.0);
    }

    #[test]
    fn ribbon_cross_section_geometry_is_flat_for_helix_round_for_coil() {
        // The previous test pins the section *params*; this pins that the frame +
        // ellipse construction actually produces a flattened ring for a helix
        // (aspect = width/thick ~ 3) and a round ring for a coil (~1) — the
        // ribbon-vs-tube distinction in the real mesh geometry.
        fn first_ring_aspect(mol: &MolCoord) -> f64 {
            let snap = ribbon_mesh_snapshot(mol, &MoleculeRenderOptions::default());
            let ring = &snap.meshes[0].vertices[0..TUBE_RADIAL];
            let n = TUBE_RADIAL as f64;
            let cx = ring.iter().map(|v| v.position.x).sum::<f64>() / n;
            let cy = ring.iter().map(|v| v.position.y).sum::<f64>() / n;
            let cz = ring.iter().map(|v| v.position.z).sum::<f64>() / n;
            let dists: Vec<f64> = ring
                .iter()
                .map(|v| {
                    let (dx, dy, dz) = (v.position.x - cx, v.position.y - cy, v.position.z - cz);
                    (dx * dx + dy * dy + dz * dz).sqrt()
                })
                .collect();
            let max = dists.iter().cloned().fold(0.0_f64, f64::max);
            let min = dists.iter().cloned().fold(f64::INFINITY, f64::min);
            max / min.max(1.0e-9)
        }

        let coords = vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 3.0, 0.0, 0.0),
            ca_atom(3, 6.0, 0.0, 0.0),
            ca_atom(4, 9.0, 0.0, 0.0),
        ];
        let mut helix = MolCoord::with_atoms(coords.clone());
        for res_id in 1..=4 {
            helix
                .set_residue_prop_str("A", &res_id.to_string(), "secondary", "helix")
                .unwrap();
        }
        let coil = MolCoord::with_atoms(coords);

        assert!(
            first_ring_aspect(&helix) > 2.0,
            "helix ring is not a flat band"
        );
        assert!(first_ring_aspect(&coil) < 1.5, "coil ring is not round");
    }

    #[test]
    fn sheet_arrow_widths_taper_the_run_tip() {
        let sections = vec![Secondary::Sheet; 10];
        let widths = sheet_arrow_widths(&sections, 4);
        assert_eq!(widths[9], 0.0); // tip tapers to a point
        assert_eq!(widths[0], 1.0); // body keeps full width
        assert!(widths[9] < widths[6]); // monotone taper toward the tip
    }

    #[test]
    fn distance_pick_draw_object_matches_qif_state_and_reference_clear_behavior() {
        let mut draw = DistPickDrawObj::new();
        assert_eq!(draw.type_name(), "DistPickDrawObj");
        assert!(!draw.is_enabled());
        assert_eq!(
            draw.color().rgba8(),
            SolidColor::from_rgba(1.0, 1.0, 1.0, 0.5).rgba8()
        );
        assert_f64_eq(draw.width(), 4.0);
        assert!(draw.positions().is_empty());

        draw.set_enabled(true);
        draw.append_position(Vector4::new3(1.0, 2.0, 3.0));
        assert_eq!(draw.positions(), &[Vector4::new3(1.0, 2.0, 3.0)]);

        let mut molecule = MolCoord::new();
        molecule.push_atom(Atom {
            serial: 7,
            name: "CA".to_string(),
            alt_loc: None,
            res_name: "ALA".to_string(),
            chain_id: "A".to_string(),
            res_id: 1,
            ins_code: None,
            position: Vector4::new3(4.0, 5.0, 6.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "C".to_string(),
            formal_charge: None,
            int_props: Default::default(),
            real_props: Default::default(),
        });
        assert!(draw.append_atom(&molecule, 7));
        assert!(!draw.append_atom(&molecule, 99));
        assert_eq!(draw.positions().len(), 2);

        draw.set_enabled(false);
        assert!(!draw.is_enabled());
        assert!(draw.positions().is_empty());
    }

    #[test]
    fn atom_intr_data_modes_and_constructors_match_reference() {
        let default = AtomIntrData::default();
        assert_eq!(default.mode, 0);
        assert_eq!(default.label_cache_id, -1);
        assert!(default.kind().is_none());
        assert!(default
            .elements
            .iter()
            .all(|element| element.mode == AtomIntrElemMode::Position));

        let distance = AtomIntrData::distance_by_selection(10, "name CA", 20, "name N");
        assert_eq!(distance.kind(), Some(AtomIntrDefKind::Distance));
        assert_eq!(distance.elements[0].molecule_id, 10);
        assert_eq!(distance.elements[0].selection.as_deref(), Some("name CA"));
        assert_eq!(distance.elements[1].molecule_id, 20);

        let angle = AtomIntrData::angle_by_id([(10, 1), (20, 2), (30, 3)]);
        assert_eq!(angle.kind(), Some(AtomIntrDefKind::Angle));
        assert_eq!(angle.elements[2].atom_id, 3);
        assert_eq!(angle.elements[2].mode, AtomIntrElemMode::AtomId);

        let torsion = AtomIntrData::torsion_by_id([(1, 4), (2, 5), (3, 6), (4, 7)]);
        assert_eq!(torsion.kind(), Some(AtomIntrDefKind::Torsion));
        assert_eq!(torsion.elements[3].molecule_id, 4);
        assert_eq!(torsion.elements[3].atom_id, 7);
    }

    #[test]
    fn cpk_renderers_match_reference_defaults() {
        let mut cpk = CpkRenderer::new();
        assert_eq!(cpk.type_name(), "cpk");
        assert!(!cpk.is_rend_bond());
        assert_eq!(cpk.detail(), 3);
        assert_f64_eq(cpk.vdw_radius("H"), 1.2);
        assert_f64_eq(cpk.vdw_radius("C"), 1.7);
        assert_f64_eq(cpk.vdw_radius("N"), 1.55);
        assert_f64_eq(cpk.vdw_radius("O"), 1.52);
        assert_f64_eq(cpk.vdw_radius("S"), 1.8);
        assert_f64_eq(cpk.vdw_radius("P"), 1.8);
        assert_f64_eq(cpk.vdw_radius("Xe"), 1.7);
        cpk.set_vdwr_x(2.0);
        assert_f64_eq(cpk.vdw_radius("unknown"), 2.0);

        let mut renderer = Cpk2Renderer::new();
        assert_eq!(renderer.type_name(), "cpk");
        assert!(!renderer.is_rend_bond());
        assert_eq!(renderer.detail(), 10);
        assert_eq!(renderer.gl_render_mode(), Cpk2Renderer::REND_DEFAULT);
        assert_eq!(renderer.get_gl_render_mode(), Cpk2Renderer::REND_DEFAULT);
        assert_eq!(renderer.glrender_mode(), Cpk2Renderer::REND_DEFAULT);
        assert_eq!(Cpk2Renderer::REND_VBO, 0);
        assert_eq!(Cpk2Renderer::REND_GLU, 0);
        assert!(!renderer.use_shader());
        assert!(!renderer.check_shader_ok());
        assert!(!renderer.sphere_gpu_prim_valid());

        renderer.set_detail(5);
        assert_eq!(renderer.detail(), 5);
        renderer.set_check_shader_ok(true);
        renderer.set_sphere_gpu_prim_valid(true);
        renderer.invalidate_display_cache();
        assert!(!renderer.check_shader_ok());
        assert!(!renderer.sphere_gpu_prim_valid());

        let cpk_options = molecule_render_options_for_renderer("cpk");
        assert_eq!(cpk_options.sphere_detail, Some(10));
        assert_eq!(cpk_options.element_radii, Some(VdwRadii::default()));
        assert!(!cpk_options.draw_bonds);
        assert_eq!(
            molecule_render_options_for_renderer("simple").valence_bond_mode,
            ValenceBondMode::Simple
        );
    }

    #[test]
    fn ballstick_and_anisou_defaults_match_qif() {
        let mut renderer = BallStickRenderer::new();
        assert_eq!(renderer.type_name(), "ballstick");
        assert!(renderer.is_rend_bond());
        assert_f64_eq(renderer.bondw(), 0.2);
        assert_f64_eq(renderer.sphr(), 0.3);
        assert_eq!(renderer.detail(), 3);
        assert!(!renderer.ring());
        assert_f64_eq(renderer.thickness(), 0.2);
        assert_eq!(renderer.ring_color().rgba8().r, 255);
        assert_eq!(renderer.ring_color().rgba8().g, 255);
        assert_eq!(renderer.ring_color().rgba8().b, 191);
        assert_eq!(renderer.valbond_mode(), BallStickRenderer::VBMODE_OFF);
        assert_eq!(renderer.get_vb_mode(), BallStickRenderer::VBMODE_OFF);
        assert_eq!(renderer.gl_render_mode(), BallStickRenderer::REND_DEFAULT);
        assert_eq!(
            renderer.get_gl_render_mode(),
            BallStickRenderer::REND_DEFAULT
        );
        assert_eq!(BallStickRenderer::REND_VBO, 2);
        assert_eq!(BallStickRenderer::REND_GLU, 3);

        renderer.set_draw_ring_only(true);
        assert!(!renderer.is_rend_bond());

        let anisou = AnIsoURenderer::new();
        assert_eq!(anisou.type_name(), "anisou");
        assert!(anisou.drawdisc());
        assert_f64_eq(anisou.discscale(), 1.1);
        assert_f64_eq(anisou.discthick(), 0.1);
        assert_eq!(anisou.maxverts(), 1_000_000);
        assert_eq!(anisou.get_glu_vert_max(), 1_000_000);
    }

    #[test]
    fn line_and_mainchain_renderer_defaults_match_qif() {
        let mut atom_intr = AtomIntrRenderer::new();
        assert_eq!(atom_intr.type_name(), "atomintr");
        assert_eq!(atom_intr.mode(), AtomIntrRenderer::AIR_FANCY);
        assert!(!atom_intr.showlabel());
        assert_eq!(atom_intr.detail(), 5);
        assert_eq!(atom_intr.stipples(), [1.0, 1.0, -1.0, -1.0, -1.0, -1.0]);
        assert_f64_eq(atom_intr.width(), 0.1);
        assert_eq!(atom_intr.start_cap_type(), AtomIntrRenderer::END_SPHERE);
        assert_eq!(atom_intr.end_cap_type(), AtomIntrRenderer::END_SPHERE);
        assert_f64_eq(atom_intr.arrow_height(), 1.0);
        assert_f64_eq(atom_intr.arrow_width(), 2.0);
        assert_eq!(atom_intr.font_name(), "sans-serif");
        assert_eq!(atom_intr.get_mode(), AtomIntrRenderer::AIR_FANCY);
        assert!(!atom_intr.is_show_label());
        assert_f64_eq(atom_intr.get_stipple0(), 1.0);
        let dist_id = atom_intr.append("name N", "name CA");
        atom_intr.set_client_mol_id(10);
        let angle_id = atom_intr.append_angle_by_id(1, 10, 2, 10, 3);
        let torsion_id = atom_intr.append_torsion_by_id(1, 10, 2, 10, 3, 10, 4);
        assert_eq!((dist_id, angle_id, torsion_id), (0, 1, 2));
        assert_eq!(atom_intr.defs().len(), 3);
        assert_eq!(
            atom_intr.get_defs_json(),
            "[{\"id\":0,\"mode\":1,\"a0\":\"name N\",\"a1\":\"name CA\"},{\"id\":1,\"mode\":2,\"a0\":\"1\",\"a1\":\"2\",\"a2\":\"3\"},{\"id\":2,\"mode\":3,\"a0\":\"1\",\"a1\":\"2\",\"a2\":\"3\",\"a3\":\"4\"}]"
        );
        assert!(atom_intr.remove(angle_id));
        assert!(atom_intr.remove(angle_id));
        assert!(!atom_intr.remove(99));
        assert!(!atom_intr.defs()[angle_id as usize].is_active());
        assert_eq!(atom_intr.defs()[angle_id as usize].to_data().mode, 0);
        assert_eq!(
            atom_intr.get_defs_json(),
            "[{\"id\":0,\"mode\":1,\"a0\":\"name N\",\"a1\":\"name CA\"},{\"id\":2,\"mode\":3,\"a0\":\"1\",\"a1\":\"2\",\"a2\":\"3\",\"a3\":\"4\"}]"
        );
        assert_eq!(
            atom_intr.append_by_2vecs(Vector4::new3(1.0, 2.0, 3.0), Vector4::new3(4.0, 5.0, 6.0)),
            3
        );
        assert!(atom_intr
            .get_defs_json()
            .contains("\"id\":3,\"mode\":1,\"a0\":\"(1.00,2.00,3.00)\""));
        assert_eq!(atom_intr.append_by_id(7, 10, 7, true), -1);
        assert!(atom_intr.defs().iter().any(|def| def.id() == dist_id));
        assert!(atom_intr.defs().iter().any(|def| def.id() == torsion_id));

        let diso = DisoRenderer::new();
        assert_eq!(diso.type_name(), "disorder");
        assert_eq!(diso.detail(), 5);
        assert_f64_eq(diso.width(), 0.3);
        assert_f64_eq(diso.dotsep(), 1.0);
        assert_f64_eq(diso.loopsize(), 2.0);
        assert_f64_eq(diso.loopsize2(), -1.0);
        assert_eq!(diso.target(), "");
        assert_f64_eq(diso.get_strength(), 2.0);
        assert_eq!(diso.get_tgt_rend_name(), "");

        let spline = SplineRenderer::new();
        assert_eq!(spline.type_name(), "spline");
        assert_eq!(spline.axialdetail(), 6);
        assert_eq!(spline.get_axial_detail(), 6);
        assert!(spline.smoothcolor());
        assert!(spline.is_smooth_color());
        assert_f64_eq(spline.get_line_width(), 1.2);
        assert_eq!(spline.start_cap_type(), SplineRenderer::TUBE_CAP_SPHR);
        assert_eq!(spline.get_start_cap_type(), SplineRenderer::TUBE_CAP_SPHR);
        assert_eq!(spline.end_cap_type(), SplineRenderer::TUBE_CAP_SPHR);
        assert!(!spline.is_seg_end_fade());
    }

    #[test]
    fn tube_ribbon_and_na_defaults_match_reference_runtime() {
        let section = TubeSection::new();
        assert_f64_eq(section.width(), 0.35);
        assert_f64_eq(section.tuber(), 1.0);
        assert_f64_eq(section.sharp(), 0.4);
        assert_eq!(section.section_type(), TubeSection::TS_ELLIPTICAL);
        assert_eq!(section.detail(), 16);
        assert_eq!(section.get_type(), TubeSection::TS_ELLIPTICAL);
        assert_eq!(section.r#type(), TubeSection::TS_ELLIPTICAL);
        assert_eq!(section.get_detail(), 16);

        let jct = JctTable::new();
        assert_eq!(jct.jct_type(), JctTable::JT_ARROW1);
        assert_f64_eq(jct.gamma(), 2.2);
        assert_f64_eq(jct.basw(), 0.0);
        assert_f64_eq(jct.arrow(), 1.8);
        assert_eq!(jct.get_type(), JctTable::JT_ARROW1);
        assert_eq!(jct.r#type(), JctTable::JT_ARROW1);
        assert_f64_eq(jct.get_gamma(), 2.2);

        let tube = TubeRenderer::new();
        assert_eq!(tube.type_name(), "tube");
        assert_eq!(tube.putty_mode(), TubeRenderer::TBR_PUTTY_OFF);
        assert_eq!(tube.get_putty_mode(), TubeRenderer::TBR_PUTTY_OFF);
        assert_eq!(tube.putty_tgt(), TubeRenderer::TBR_PUTTY_BFAC);
        assert_eq!(tube.get_putty_tgt(), TubeRenderer::TBR_PUTTY_BFAC);
        assert_f64_eq(tube.putty_hiscl(), 3.0);
        assert_f64_eq(tube.get_putty_hi_scl(), 3.0);
        assert_f64_eq(tube.putty_loscl(), 3.0);
        assert_f64_eq(tube.get_putty_lo_scl(), 3.0);
        assert_eq!(tube.get_tube_section(), tube.section());

        let na = NARenderer::new();
        assert_eq!(na.type_name(), "nucl");
        assert!(na.show_tube());
        assert!(na.get_rend_tube());
        assert!(na.show_basepair());
        assert!(na.is_show_bp());
        assert_f64_eq(na.base_size(), 0.5);
        assert_f64_eq(na.get_base_size(), 0.5);
        assert_eq!(na.base_detail(), 3);
        assert_eq!(na.get_base_detail(), 3);
        assert_f64_eq(na.base_thick(), 0.5);
        assert_f64_eq(na.get_base_thick(), 0.5);
        assert_eq!(na.base_type(), NARenderer::NAREND_BP);
        assert_eq!(na.get_base_type(), NARenderer::NAREND_BP);

        let ribbon = RibbonRenderer::new();
        assert_eq!(ribbon.type_name(), "ribbon");
        assert_f64_eq(ribbon.helix_smooth(), 0.0);
        assert_eq!(ribbon.get_helix_section(), ribbon.helix());
        assert_eq!(ribbon.get_sheet_head(), ribbon.sheethead());
        assert_f64_eq(ribbon.get_helix_smooth(), 0.0);
        assert_f64_eq(ribbon.sheet_smooth(), 0.5);
        assert_f64_eq(ribbon.coil_smooth(), 0.0);
        assert!(!ribbon.helix_usebackcol());
        assert!(!ribbon.is_use_hb_col());
        assert!(!ribbon.sheet_usesidecol());
        assert!(!ribbon.is_use_ss_col());

        let cartoon = Ribbon2Renderer::new();
        assert_eq!(cartoon.type_name(), "cartoon");
        assert_eq!(cartoon.axialdetail(), 8);
        assert_eq!(cartoon.get_axial_detail(), 8);
        assert!(!cartoon.smoothcolor());
        assert!(!cartoon.is_smooth_color());
        assert_f64_eq(cartoon.anchor_weight(), 10.0);
        assert_f64_eq(cartoon.get_anchor_wgt(), 10.0);
        assert!(!cartoon.helix_ribbon());
        assert_f64_eq(cartoon.helix_smooth(), 3.0);
        assert_eq!(cartoon.helix_width_mode(), Ribbon2Renderer::HWIDTH_AVER);
        assert_eq!(cartoon.get_helix_width_mode(), Ribbon2Renderer::HWIDTH_AVER);
        assert!(cartoon.is_width_aver());
        assert_f64_eq(cartoon.helix_width(), 2.3);
        assert_f64_eq(cartoon.helix_extend(), 0.5);
        assert_f64_eq(cartoon.helix_wsmooth(), 5.0);
        assert_f64_eq(cartoon.helix_wplus(), 0.2);
        assert_f64_eq(cartoon.sheet_smooth(), 2.0);
        assert_f64_eq(cartoon.sheet_wsmooth(), 5.0);
        assert_f64_eq(cartoon.coil_smooth(), -1.0);
        assert_eq!(cartoon.start_cap_type(), Ribbon2Renderer::TUBE_CAP_FLAT);
        assert_eq!(cartoon.get_start_cap_type(), Ribbon2Renderer::TUBE_CAP_FLAT);
        assert_eq!(cartoon.end_cap_type(), Ribbon2Renderer::TUBE_CAP_FLAT);
        assert!(!cartoon.segend_fade());
        assert!(!cartoon.is_seg_end_fade());
        assert!(!cartoon.dump_curvature());
    }

    #[test]
    fn coloring_defaults_and_table_ops_match_qif() {
        let mut rainbow = RainbowColoring::new();
        assert_eq!(rainbow.type_name(), "rainbow");
        assert_f64_eq(rainbow.start_hue(), 240.0);
        assert_f64_eq(rainbow.get_start_hue(), 240.0);
        assert_f64_eq(rainbow.end_hue(), 0.0);
        assert_f64_eq(rainbow.sat(), 1.0);
        assert_f64_eq(rainbow.bri(), 1.0);
        assert_eq!(rainbow.mode(), RainbowColoring::RBC_MOL);
        assert_eq!(rainbow.get_mode(), RainbowColoring::RBC_MOL);
        assert_eq!(rainbow.incr_mode(), RainbowColoring::RBC_INCR_RESID);
        assert_eq!(rainbow.get_incr_mode(), RainbowColoring::RBC_INCR_RESID);
        rainbow.set_mode(RainbowColoring::RBC_CHAIN);
        assert_eq!(rainbow.mode(), RainbowColoring::RBC_CHAIN);

        let mut script = ScriptColoring::new();
        assert_eq!(script.type_name(), "script");
        assert_eq!(script.script(), "");
        assert_eq!(script.get_script(), "");
        script.set_script("atom.elem == 'C'");
        assert_eq!(script.script(), "atom.elem == 'C'");

        let mut paint = PaintColoring::new();
        assert_eq!(paint.type_name(), "paint");
        assert!(paint.is_empty());
        paint.append("name CA", SolidColor::from_rgb(1.0, 0.0, 0.0));
        paint.insert_before(0, "name N", SolidColor::from_rgb(0.0, 1.0, 0.0));
        paint.insert_before(-1, "name O", SolidColor::from_rgb(0.0, 0.0, 1.0));
        assert_eq!(paint.len(), 3);
        assert_eq!(paint.get_size(), 3);
        assert_eq!(paint.size(), 3);
        assert_eq!(paint.get_selection_at(0), Some("name N"));
        assert_eq!(paint.get_sel_at(0), Some("name N"));
        assert_eq!(paint.get_selection_at(1), Some("name CA"));
        assert_eq!(paint.get_selection_at(2), Some("name O"));
        assert!(paint.change_at(1, "name C", SolidColor::from_rgb(1.0, 1.0, 0.0)));
        assert_eq!(paint.get_selection_at(1), Some("name C"));
        assert!(paint.remove_at(1));
        assert_eq!(paint.len(), 2);
        assert!(!paint.remove_at(10));
        paint.clear();
        assert!(paint.is_empty());
    }
}
