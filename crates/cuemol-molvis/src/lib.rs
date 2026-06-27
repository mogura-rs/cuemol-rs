use cuemol_core::Vector4;
use cuemol_gfx::{ColorSpec, SolidColor};
use cuemol_molstr::{parse_selector, Atom, MolCoord, MolRenderer, ResidIterator, ResidueView};
use cuemol_qsys::{DrawObj, StyleManager};
use cuemol_render::{MeshVertex, RenderAtom, RenderBond, RenderMesh, RenderSceneSnapshot};

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
            width: 3.0,
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

    pub fn set_gl_render_mode(&mut self, value: i32) {
        self.gl_render_mode = value;
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

    pub fn valbond(&self) -> i32 {
        self.valbond_mode()
    }

    pub fn set_valbond_mode(&mut self, value: i32) {
        self.valbond_mode = value;
    }

    pub fn set_valbond(&mut self, value: i32) {
        self.set_valbond_mode(value);
    }

    pub fn gl_render_mode(&self) -> i32 {
        self.gl_render_mode
    }

    pub fn set_gl_render_mode(&mut self, value: i32) {
        self.gl_render_mode = value;
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

    pub fn set_maxverts(&mut self, value: i32) {
        self.maxverts = value;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomIntrRenderer {
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
    next_def_id: i32,
}

impl Default for AtomIntrRenderer {
    fn default() -> Self {
        Self {
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
            next_def_id: 1,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomIntrDefKind {
    Distance,
    Angle,
    Torsion,
}

impl AtomIntrDefKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Distance => "distance",
            Self::Angle => "angle",
            Self::Torsion => "torsion",
        }
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

    pub fn type_name(&self) -> &'static str {
        "atomintr"
    }

    pub fn mode(&self) -> i32 {
        self.mode
    }

    pub fn set_mode(&mut self, value: i32) {
        self.mode = value;
    }

    pub fn showlabel(&self) -> bool {
        self.showlabel
    }

    pub fn set_showlabel(&mut self, value: bool) {
        self.showlabel = value;
    }

    pub fn detail(&self) -> i32 {
        self.detail
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
        if !def.positions.is_empty() {
            return Some(def.positions.clone());
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
        self.push_def(AtomIntrDef {
            id: 0,
            kind: AtomIntrDefKind::Distance,
            selections: vec![selection1.into(), selection2.into()],
            atom_ids: Vec::new(),
            mol_ids: Vec::new(),
            positions: Vec::new(),
            show_message: true,
        })
    }

    pub fn append_by_id(
        &mut self,
        aid1: i32,
        mol_id1: i32,
        aid2: i32,
        mol_id2: i32,
        show_message: bool,
    ) -> i32 {
        self.push_def(AtomIntrDef {
            id: 0,
            kind: AtomIntrDefKind::Distance,
            selections: Vec::new(),
            atom_ids: vec![aid1, aid2],
            mol_ids: vec![mol_id1, mol_id2],
            positions: Vec::new(),
            show_message,
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
        mol_id1: i32,
        aid2: i32,
        mol_id2: i32,
        aid3: i32,
        mol_id3: i32,
    ) -> i32 {
        self.push_def(AtomIntrDef {
            id: 0,
            kind: AtomIntrDefKind::Angle,
            selections: Vec::new(),
            atom_ids: vec![aid1, aid2, aid3],
            mol_ids: vec![mol_id1, mol_id2, mol_id3],
            positions: Vec::new(),
            show_message: true,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_torsion_by_id(
        &mut self,
        aid1: i32,
        mol_id1: i32,
        aid2: i32,
        mol_id2: i32,
        aid3: i32,
        mol_id3: i32,
        aid4: i32,
        mol_id4: i32,
    ) -> i32 {
        self.push_def(AtomIntrDef {
            id: 0,
            kind: AtomIntrDefKind::Torsion,
            selections: Vec::new(),
            atom_ids: vec![aid1, aid2, aid3, aid4],
            mol_ids: vec![mol_id1, mol_id2, mol_id3, mol_id4],
            positions: Vec::new(),
            show_message: true,
        })
    }

    pub fn remove(&mut self, id: i32) -> bool {
        let Some(index) = self.defs.iter().position(|def| def.id == id) else {
            return false;
        };
        self.defs.remove(index);
        true
    }

    pub fn get_defs_json(&self) -> String {
        let entries = self
            .defs
            .iter()
            .map(atom_intr_def_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    fn push_def(&mut self, mut def: AtomIntrDef) -> i32 {
        let id = self.next_def_id;
        self.next_def_id += 1;
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

    pub fn set_detail(&mut self, value: i32) {
        self.detail = value;
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
    }

    pub fn dotsep(&self) -> f64 {
        self.dotsep
    }

    pub fn set_dotsep(&mut self, value: f64) {
        self.dotsep = value;
    }

    pub fn loopsize(&self) -> f64 {
        self.loopsize
    }

    pub fn set_loopsize(&mut self, value: f64) {
        self.loopsize = value;
    }

    pub fn loopsize2(&self) -> f64 {
        self.loopsize2
    }

    pub fn loopsize_2(&self) -> f64 {
        self.loopsize2()
    }

    pub fn set_loopsize2(&mut self, value: f64) {
        self.loopsize2 = value;
    }

    pub fn set_loopsize_2(&mut self, value: f64) {
        self.set_loopsize2(value);
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn set_target(&mut self, value: impl Into<String>) {
        self.target = value.into();
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

    pub fn set_width(&mut self, value: f64) {
        self.width = value;
        self.invalidate();
    }

    pub fn tuber(&self) -> f64 {
        self.tuber
    }

    pub fn set_tuber(&mut self, value: f64) {
        self.tuber = value;
        self.invalidate();
    }

    pub fn sharp(&self) -> f64 {
        self.sharp
    }

    pub fn set_sharp(&mut self, value: f64) {
        self.sharp = value;
        self.invalidate();
    }

    pub fn section_type(&self) -> i32 {
        self.section_type
    }

    pub fn set_section_type(&mut self, value: i32) {
        self.section_type = value;
        self.invalidate();
    }

    pub fn detail(&self) -> i32 {
        self.detail
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
            jct_type: Self::JT_SMOOTH1,
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

    pub fn set_jct_type(&mut self, value: i32) {
        self.jct_type = value;
        self.invalidate();
    }

    pub fn gamma(&self) -> f64 {
        self.gamma
    }

    pub fn set_gamma(&mut self, value: f64) {
        self.gamma = value;
        self.invalidate();
    }

    pub fn basw(&self) -> f64 {
        self.basw
    }

    pub fn set_basw(&mut self, value: f64) {
        self.basw = value;
        self.invalidate();
    }

    pub fn arrow(&self) -> f64 {
        self.arrow
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

    pub fn set_axialdetail(&mut self, value: i32) {
        self.axialdetail = value;
    }

    pub fn smooth(&self) -> f64 {
        self.smooth
    }

    pub fn set_smooth(&mut self, value: f64) {
        self.smooth = value;
    }

    pub fn smoothcolor(&self) -> bool {
        self.smoothcolor
    }

    pub fn set_smoothcolor(&mut self, value: bool) {
        self.smoothcolor = value;
    }

    pub fn line_width(&self) -> f64 {
        self.line_width
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.line_width = value;
    }

    pub fn start_cap_type(&self) -> i32 {
        self.start_cap_type
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

    pub fn set_segend_fade(&mut self, value: bool) {
        self.segend_fade = value;
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

    pub fn section_mut(&mut self) -> &mut TubeSection {
        &mut self.section
    }

    pub fn putty_mode(&self) -> i32 {
        self.putty_mode
    }

    pub fn set_putty_mode(&mut self, value: i32) {
        self.putty_mode = value;
    }

    pub fn putty_tgt(&self) -> i32 {
        self.putty_tgt
    }

    pub fn set_putty_tgt(&mut self, value: i32) {
        self.putty_tgt = value;
    }

    pub fn putty_hiscl(&self) -> f64 {
        self.putty_hiscl
    }

    pub fn set_putty_hiscl(&mut self, value: f64) {
        self.putty_hiscl = value;
    }

    pub fn putty_loscl(&self) -> f64 {
        self.putty_loscl
    }

    pub fn set_putty_loscl(&mut self, value: f64) {
        self.putty_loscl = value;
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

    pub fn set_show_tube(&mut self, value: bool) {
        self.show_tube = value;
    }

    pub fn show_basepair(&self) -> bool {
        self.show_basepair
    }

    pub fn set_show_basepair(&mut self, value: bool) {
        self.show_basepair = value;
    }

    pub fn base_size(&self) -> f64 {
        self.base_size
    }

    pub fn set_base_size(&mut self, value: f64) {
        self.base_size = value;
    }

    pub fn base_detail(&self) -> i32 {
        self.base_detail
    }

    pub fn set_base_detail(&mut self, value: i32) {
        self.base_detail = value;
    }

    pub fn base_thick(&self) -> f64 {
        self.base_thick
    }

    pub fn set_base_thick(&mut self, value: f64) {
        self.base_thick = value;
    }

    pub fn base_type(&self) -> i32 {
        self.base_type
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

    pub fn helix_mut(&mut self) -> &mut TubeSection {
        &mut self.helix
    }

    pub fn sheet(&self) -> &TubeSection {
        &self.sheet
    }

    pub fn sheet_mut(&mut self) -> &mut TubeSection {
        &mut self.sheet
    }

    pub fn coil(&self) -> &TubeSection {
        &self.coil
    }

    pub fn coil_mut(&mut self) -> &mut TubeSection {
        &mut self.coil
    }

    pub fn helixhead(&self) -> &JctTable {
        &self.helixhead
    }

    pub fn helixtail(&self) -> &JctTable {
        &self.helixtail
    }

    pub fn sheethead(&self) -> &JctTable {
        &self.sheethead
    }

    pub fn sheettail(&self) -> &JctTable {
        &self.sheettail
    }

    pub fn helix_smooth(&self) -> f64 {
        self.helix_smooth
    }

    pub fn set_helix_smooth(&mut self, value: f64) {
        self.helix_smooth = value;
    }

    pub fn sheet_smooth(&self) -> f64 {
        self.sheet_smooth
    }

    pub fn set_sheet_smooth(&mut self, value: f64) {
        self.sheet_smooth = value;
    }

    pub fn coil_smooth(&self) -> f64 {
        self.coil_smooth
    }

    pub fn set_coil_smooth(&mut self, value: f64) {
        self.coil_smooth = value;
    }

    pub fn helix_usebackcol(&self) -> bool {
        self.helix_usebackcol
    }

    pub fn set_helix_usebackcol(&mut self, value: bool) {
        self.helix_usebackcol = value;
    }

    pub fn helix_backcol(&self) -> &SolidColor {
        &self.helix_backcol
    }

    pub fn set_helix_backcol(&mut self, value: SolidColor) {
        self.helix_backcol = value;
    }

    pub fn sheet_usesidecol(&self) -> bool {
        self.sheet_usesidecol
    }

    pub fn set_sheet_usesidecol(&mut self, value: bool) {
        self.sheet_usesidecol = value;
    }

    pub fn sheet_sidecol(&self) -> &SolidColor {
        &self.sheet_sidecol
    }

    pub fn set_sheet_sidecol(&mut self, value: SolidColor) {
        self.sheet_sidecol = value;
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

    pub fn set_axialdetail(&mut self, value: i32) {
        self.axialdetail = value;
    }

    pub fn smoothcolor(&self) -> bool {
        self.smoothcolor
    }

    pub fn set_smoothcolor(&mut self, value: bool) {
        self.smoothcolor = value;
    }

    pub fn anchor_selection(&self) -> &str {
        &self.anchor_selection
    }

    pub fn anchor_sel(&self) -> &str {
        self.anchor_selection()
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

    pub fn set_anchor_weight(&mut self, value: f64) {
        self.anchor_weight = value;
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

    pub fn sheet(&self) -> &TubeSection {
        &self.sheet
    }

    pub fn coil(&self) -> &TubeSection {
        &self.coil
    }

    pub fn sheethead(&self) -> &JctTable {
        &self.sheethead
    }

    pub fn ribhelix(&self) -> &TubeSection {
        &self.ribhelix
    }

    pub fn ribhelix_head(&self) -> &JctTable {
        &self.ribhelix_head
    }

    pub fn ribhelix_tail(&self) -> &JctTable {
        &self.ribhelix_tail
    }

    pub fn start_cap_type(&self) -> i32 {
        self.start_cap_type
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

    pub fn set_segend_fade(&mut self, value: bool) {
        self.segend_fade = value;
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

    pub fn set_start_hue(&mut self, value: f64) {
        self.start_hue = value;
    }

    pub fn end_hue(&self) -> f64 {
        self.end_hue
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

    pub fn set_mode(&mut self, value: i32) {
        self.mode = value;
    }

    pub fn incr_mode(&self) -> i32 {
        self.incr_mode
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
        let color = SolidColor::create_hsb((hue / 360.0).rem_euclid(1.0), self.sat, self.bri, 1.0)
            .rgba8();
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

fn atom_intr_def_json(def: &AtomIntrDef) -> String {
    format!(
        "{{\"id\":{},\"type\":\"{}\",\"sels\":{},\"atoms\":{},\"mols\":{},\"positions\":{},\"showmsg\":{}}}",
        def.id,
        def.kind.as_str(),
        json_string_array(&def.selections),
        json_i32_array(&def.atom_ids),
        json_i32_array(&def.mol_ids),
        json_vector_array(&def.positions),
        def.show_message
    )
}

fn json_string_array(values: &[String]) -> String {
    let body = values
        .iter()
        .map(|value| format!("\"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

fn json_i32_array(values: &[i32]) -> String {
    let body = values
        .iter()
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

fn json_vector_array(values: &[Vector4]) -> String {
    let body = values
        .iter()
        .map(|value| format!("[{},{},{}]", value.x, value.y, value.z))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
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
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoleculeRenderOptions {
    pub object_uid: Option<u64>,
    pub atom_radius: f32,
    pub bond_radius: f32,
    pub vdw_scale: f32,
    pub draw_atoms: bool,
    pub draw_bonds: bool,
    pub color_mode: MoleculeColorMode,
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
    /// Absolute ribbon cross-section dims per SS class, resolved from a style's
    /// `<helix>/<sheet>/<coil>` `width`/`tuber` (C++ `TubeSection`). Overrides the
    /// factor-based [`MoleculeRenderOptions::ribbon_sections`] where present.
    pub styled_ribbon: StyledRibbonSections,
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
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StyledRibbonSections {
    pub helix: Option<(f64, f64)>,
    pub sheet: Option<(f64, f64)>,
    pub coil: Option<(f64, f64)>,
    pub helix_shape: SectionShape,
    pub sheet_shape: SectionShape,
    pub coil_shape: SectionShape,
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
}

/// Ribbon cross-section profile (C++ `TubeSection` `TS_*`): a smooth ellipse vs a
/// flat-faced rectangle. A rectangle gives the sharp-edged "fancy" ribbon (each
/// side carries an axis-aligned face normal); an ellipse the rounded default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SectionShape {
    #[default]
    Elliptical,
    Rectangular,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoleculeColorMode {
    Element,
    Uniform,
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
            draw_atoms: true,
            draw_bonds: true,
            color_mode: MoleculeColorMode::Element,
            selected_indices: Vec::new(),
            putty: PuttyOptions::default(),
            mainchain_radius: None,
            ribbon_sections: RibbonSections::default(),
            anchor_weight: 0.0,
            atom_radius_abs: None,
            sphere_detail: None,
            ribbon_axial_detail: None,
            styled_ribbon: StyledRibbonSections::default(),
            draw_indices: None,
            object_paint: Vec::new(),
            rainbow: None,
        }
    }
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
    let mut styles = StyleManager::new();
    styles.load_builtin_default_styles(DEFAULT_STYLE_SCOPE);
    styles
}

/// Build a [`RainbowColoring`] from an object's `<coloring type="RainbowColoring">`
/// scheme name + attribute accessor (start_hue/end_hue/sat/bri). Returns `None`
/// for any other (or absent) coloring type. Shared by the desktop GUIs and CLI so
/// both resolve rainbow coloring identically.
pub fn rainbow_coloring_from_attrs<'a>(
    coloring_type: Option<&str>,
    attr: impl Fn(&str) -> Option<&'a str>,
) -> Option<RainbowColoring> {
    if coloring_type != Some("RainbowColoring") {
        return None;
    }
    let mut rainbow = RainbowColoring::new();
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
    Some(rainbow)
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

    // Putty (C++ TubeRenderer width modulation by B-factor/occupancy): exact qsc
    // attr names + enum values from TubeRenderer.qif.
    if let Some(mode) = renderer.resolved_attr("putty_mode", styles, scope_id).as_deref() {
        options.putty.mode = match mode {
            "linear1" => PuttyMode::Linear,
            "scale1" => PuttyMode::Scale,
            _ => PuttyMode::Off,
        };
    }
    if let Some(target) = renderer.resolved_attr("putty_tgt", styles, scope_id).as_deref() {
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
        .resolved_child_attr(tag, "type", styles, scope_id)
        .as_deref()
    {
        Some("rectangle") | Some("rect") => SectionShape::Rectangular,
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
    let draw = options
        .draw_indices
        .as_ref()
        .map(|indices| indices.iter().copied().collect::<std::collections::BTreeSet<_>>());
    let drawn = |index: usize| draw.as_ref().is_none_or(|set| set.contains(&index));
    let atoms = if options.draw_atoms {
        molecule
            .atoms()
            .iter()
            .enumerate()
            .filter(|(index, _)| drawn(*index))
            .map(|(index, atom)| {
                RenderAtom::new(
                    Vector4::new3(atom.position.x, atom.position.y, atom.position.z),
                    atom_radius(atom.element.as_str(), options),
                    Some(atom_color(atom.element.as_str(), options.color_mode)),
                )
                .with_picking(
                    options.object_uid,
                    Some(index),
                    selected.contains(&index),
                )
                .with_detail(options.sphere_detail)
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let bonds = if options.draw_bonds {
        molecule
            .bonds()
            .iter()
            .filter(|bond| drawn(bond.atom1) && drawn(bond.atom2))
            .filter_map(|bond| {
                let atom1 = molecule.atoms().get(bond.atom1)?;
                let atom2 = molecule.atoms().get(bond.atom2)?;
                let selected_bond =
                    selected.contains(&bond.atom1) || selected.contains(&bond.atom2);
                Some(
                    RenderBond::new(
                        Vector4::new3(atom1.position.x, atom1.position.y, atom1.position.z),
                        Vector4::new3(atom2.position.x, atom2.position.y, atom2.position.z),
                        options.bond_radius,
                        Some(ColorSpec {
                            base: cuemol_gfx::ColorBase::Rgb {
                                r: 0.72,
                                g: 0.72,
                                b: 0.72,
                                a: 1.0,
                            },
                            modifiers: Vec::new(),
                        }),
                    )
                    .with_picking(
                        options.object_uid,
                        Some((bond.atom1, bond.atom2)),
                        selected_bond,
                    ),
                )
            })
            .collect()
    } else {
        Vec::new()
    };
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
        bounds,
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

/// Build a smooth main-chain trace ("cartoon"/tube-style) snapshot.
///
/// The current GPU pipeline only draws spheres and line segments, so a full
/// secondary-structure ribbon mesh is not available. Instead we approximate the
/// cartoon look the way the original `SplineRenderer` does conceptually: gather
/// each chain's pivot atoms (CA for proteins, P/C1' for nucleic acids) in residue
/// order, interpolate them with a Catmull-Rom spline, and emit a chain of small
/// spheres plus connecting segments — a smooth tube along the backbone.
pub fn mainchain_trace_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
) -> RenderSceneSnapshot {
    const SUBDIV: usize = 8;
    let tube_radius = options
        .mainchain_radius
        .unwrap_or_else(|| (options.bond_radius * 3.0).clamp(0.18, 0.6));

    let draw = options
        .draw_indices
        .as_ref()
        .map(|indices| indices.iter().copied().collect::<std::collections::BTreeSet<_>>());
    let chains = collect_mainchain_pivots(molecule, draw.as_ref());
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
            atoms.push(RenderAtom::new(*point, radius_of_sample(i), Some(color.clone())));
        }
        for (i, segment) in samples.windows(2).enumerate() {
            let r = (radius_of_sample(i) + radius_of_sample(i + 1)) * 0.5;
            bonds.push(RenderBond::new(segment[0], segment[1], r, Some(color.clone())));
        }
    }

    RenderSceneSnapshot {
        atoms,
        bonds,
        meshes: Vec::new(),
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
fn putty_value_stats(
    chains: &[Vec<MainchainPoint>],
    putty: &PuttyOptions,
) -> (f64, f64, f64) {
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

/// Per-spline-sample rainbow colours for a ribbon chain: each sample takes its
/// segment residue's hue along the molecule-wide gradient (`chain_offset` is this
/// chain's global residue start, `total_residues` the molecule total).
fn ribbon_rainbow_colors(
    sample_count: usize,
    chain_offset: usize,
    total_residues: usize,
    pivot_count: usize,
    subdiv: usize,
    rainbow: &RainbowColoring,
) -> Vec<Option<[f32; 4]>> {
    let subdiv = subdiv.max(1);
    (0..sample_count)
        .map(|i| {
            let seg = (i / subdiv).min(pivot_count.saturating_sub(1));
            Some(rainbow.color_for_index(chain_offset + seg, total_residues))
        })
        .collect()
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
/// residue order.
///
/// Chains are split whenever the residue iterator crosses a chain-name boundary,
/// so each returned point list is one contiguous backbone to be splined.
fn collect_mainchain_pivots(
    molecule: &MolCoord,
    draw: Option<&std::collections::BTreeSet<usize>>,
) -> Vec<Vec<MainchainPoint>> {
    let mut chains: Vec<(String, Vec<MainchainPoint>)> = Vec::new();
    let mut iter = ResidIterator::all(molecule);
    iter.first();
    while iter.has_more() {
        if let Some(residue) = iter.get() {
            // Per-renderer `sel="..."`: skip residues with no selected atom so a
            // sel'd ribbon traces only its selection, not the whole backbone.
            let included = draw.is_none_or(|set| {
                residue.atom_indices().iter().any(|index| set.contains(index))
            });
            if included {
                if let Some(point) = mainchain_pivot_point(molecule, residue) {
                    let chain = residue.chain_name().to_string();
                    if chains.last().map(|(name, _)| name != &chain).unwrap_or(true) {
                        chains.push((chain, Vec::new()));
                    }
                    if let Some((_, points)) = chains.last_mut() {
                        points.push(point);
                    }
                }
            }
        }
        iter.next();
    }
    chains.into_iter().map(|(_, points)| points).collect()
}

/// Pick the residue's backbone pivot atom: CA (protein) then nucleic-acid fallbacks.
fn mainchain_pivot_point(molecule: &MolCoord, residue: &ResidueView) -> Option<MainchainPoint> {
    let indices = residue.atom_indices();
    for name in ["CA", "P", "C1'", "O5'"] {
        for &index in indices {
            let Some(atom) = molecule.atoms().get(index) else {
                continue;
            };
            if atom.name.trim().eq_ignore_ascii_case(name) {
                return Some(MainchainPoint {
                    pos: Vector4::new3(atom.position.x, atom.position.y, atom.position.z),
                    ss: residue_secondary(molecule, atom),
                    b_factor: atom.b_factor as f64,
                    occupancy: atom.occupancy as f64,
                    atom_index: index,
                });
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
                xvec[i] = yvec[i].add_vec(xvec[i + 1].scale(h[i + 1])).scale(-1.0 / m[i]);
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

        let position = c3.scale(f).add_vec(c2).scale(f).add_vec(c1).scale(f).add_vec(c0);
        let tangent = c3.scale(3.0 * f).add_vec(c2.scale(2.0)).scale(f).add_vec(c1);
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
    let draw = options
        .draw_indices
        .as_ref()
        .map(|indices| indices.iter().copied().collect::<std::collections::BTreeSet<_>>());
    let chains = collect_mainchain_pivots(molecule, draw.as_ref());
    // Putty (TubeRenderer): modulate the ribbon width per residue by the B-factor/
    // occupancy, on top of the section width scales. Stats are over all pivots.
    let putty = options.putty;
    let (putty_lo, putty_hi, putty_aver) = putty_value_stats(&chains, &putty);
    // Object paint coloring (resolved against the X11 table) colours the backbone
    // where the renderer defers to $molcol; built once when the scene paints.
    let paint_styles = (!options.object_paint.is_empty()).then(default_style_manager);
    // RainbowColoring (when set) is the object's colour scheme and spans the whole
    // molecule's residues (RBC_MOL default), so index pivots globally across chains.
    let total_residues: usize = chains.iter().map(|chain| chain.len()).sum();
    let mut residue_offset = 0usize;
    let mut meshes = Vec::new();
    for pivots in &chains {
        let chain_offset = residue_offset;
        residue_offset += pivots.len();
        if pivots.len() < 2 {
            continue;
        }
        let (samples, sections, mut width_scales) =
            sample_backbone(pivots, subdiv, options.anchor_weight);
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
        // Rainbow (object colour scheme) takes precedence over $molcol→paint.
        let paint = if let Some(rainbow) = &options.rainbow {
            Some(ribbon_rainbow_colors(
                samples.len(),
                chain_offset,
                total_residues,
                pivots.len(),
                subdiv,
                rainbow,
            ))
        } else {
            paint_styles.as_ref().map(|styles| {
                ribbon_paint_colors(
                    samples.len(),
                    pivots,
                    subdiv,
                    &options.object_paint,
                    molecule,
                    styles,
                )
            })
        };
        let mesh = ribbon_tube_mesh(
            &samples,
            &sections,
            &width_scales,
            scale,
            &options.ribbon_sections,
            &options.styled_ribbon,
            paint.as_deref(),
        );
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
        bounds,
    }
}

/// Spline the backbone positions and carry a per-sample secondary structure.
///
/// A spline segment between two residues keeps its class only when both ends
/// agree; mixed boundaries fall back to coil so the cross-section tapers
/// smoothly across secondary-structure transitions.
fn sample_backbone(
    points: &[MainchainPoint],
    subdiv: usize,
    anchor_weight: f64,
) -> (Vec<Vector4>, Vec<Secondary>, Vec<f64>) {
    let mut positions = points.iter().map(|point| point.pos).collect::<Vec<_>>();
    // Anchor-weight smoothing (C++ Ribbon2/SplineCoeffSet): blend each interior
    // control point toward the midpoint of its neighbours before splining.
    let w = anchor_weight.clamp(0.0, 1.0);
    if w > 1.0e-9 && positions.len() >= 3 {
        let orig = positions.clone();
        for i in 1..orig.len() - 1 {
            let mid = (orig[i - 1] + orig[i + 1]).scale(0.5);
            positions[i] = orig[i].scale(1.0 - w) + mid.scale(w);
        }
    }
    let samples = cubic_spline_samples(&positions, subdiv);
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
    let width_scales = sheet_arrow_widths(&sections, subdiv);
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
fn sheet_arrow_widths(sections: &[Secondary], subdiv: usize) -> Vec<f64> {
    let mut widths = vec![1.0; sections.len()];
    let head = subdiv.max(2);
    let count = sections.len();
    let mut i = 0;
    while i < count {
        if sections[i] != Secondary::Sheet {
            i += 1;
            continue;
        }
        let start = i;
        while i < count && sections[i] == Secondary::Sheet {
            i += 1;
        }
        let end = i - 1;
        let head_len = head.min(end - start + 1);
        for k in 0..head_len {
            // k counts back from the tip: tip (k=0) -> 0, root -> 1.8x.
            widths[end - k] = 1.8 * (k as f64 / head_len as f64);
        }
    }
    widths
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

/// Number of radial samples around the ribbon cross-section.
const TUBE_RADIAL: usize = 12;

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
    scale: f64,
    sec: &RibbonSections,
    styled: &StyledRibbonSections,
    paint: Option<&[Option<[f32; 4]>]>,
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

    // Ring vertices: TUBE_RADIAL points of the cross-section at each sample, with
    // width along the binormal and thickness along the normal. The profile is an
    // ellipse (smooth) or a rectangle (flat-faced) per the resolved style.
    let mut vertices = Vec::with_capacity(n * TUBE_RADIAL);
    for (i, &center) in samples.iter().enumerate() {
        let (normal, binormal) = frames[i];
        let (base_width, thick, section_color) = section_params(sections[i], scale, sec, styled);
        // Object paint coloring (when resolved for this sample) overrides the
        // secondary-structure section colour — the $molcol→object cascade.
        let color = paint
            .and_then(|colors| colors.get(i).copied().flatten())
            .unwrap_or(section_color);
        let width = base_width * width_scales[i];
        let shape = styled.shape_for(sections[i]);
        for j in 0..TUBE_RADIAL {
            let t = (j as f64) / (TUBE_RADIAL as f64);
            let (offset, nrm) = match shape {
                SectionShape::Rectangular => rect_ring_point(t, width, thick, binormal, normal),
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
            vertices.push(MeshVertex {
                position: center + offset,
                normal: nrm,
                color,
            });
        }
    }

    // Triangle indices stitching consecutive rings into quads.
    let mut indices = Vec::with_capacity((n - 1) * TUBE_RADIAL * 6);
    for i in 0..n - 1 {
        for j in 0..TUBE_RADIAL {
            let j_next = (j + 1) % TUBE_RADIAL;
            let a = (i * TUBE_RADIAL + j) as u32;
            let b = (i * TUBE_RADIAL + j_next) as u32;
            let c = ((i + 1) * TUBE_RADIAL + j) as u32;
            let d = ((i + 1) * TUBE_RADIAL + j_next) as u32;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    RenderMesh { vertices, indices }
}

/// Aromatic/saturated ring fills (C++ `BallStickRenderer::drawRings`): for each
/// residue with ring definitions in the topology props (`TopoDB::rings`), draw
/// the ring as a flat filled polygon — a triangle fan from the ring centroid to
/// its member atoms, with the ring's plane normal. Returned as snapshot meshes.
pub fn ring_meshes_snapshot(molecule: &MolCoord) -> RenderSceneSnapshot {
    let db = cuemol_molstr::toppar::TopoDB::global();
    let summaries = molecule.residue_summaries();
    let ring_color = [1.0_f32, 1.0, 0.75, 1.0];
    let mut meshes = Vec::new();
    for s in &summaries {
        let rings = db.rings(&s.res_name);
        if rings.is_empty() {
            continue;
        }
        let pos = |name: &str| -> Option<Vector4> {
            s.atom_indices
                .iter()
                .find(|&&ai| molecule.atoms()[ai].name.trim().eq_ignore_ascii_case(name))
                .map(|&ai| molecule.atoms()[ai].position)
        };
        for ring in rings {
            let verts: Vec<Vector4> = ring.iter().filter_map(|n| pos(n)).collect();
            if verts.len() < 3 {
                continue;
            }
            let mut centroid = Vector4::new3(0.0, 0.0, 0.0);
            for v in &verts {
                centroid = centroid + *v;
            }
            centroid = centroid.scale(1.0 / verts.len() as f64);
            let normal = normalize_or(
                (verts[1] - verts[0]).cross(verts[2] - verts[0]),
                Vector4::new3(0.0, 0.0, 1.0),
            );
            let mut mesh = RenderMesh::new();
            mesh.vertices.push(MeshVertex {
                position: centroid,
                normal,
                color: ring_color,
            });
            for v in &verts {
                mesh.vertices.push(MeshVertex {
                    position: *v,
                    normal,
                    color: ring_color,
                });
            }
            // Triangle fan: centroid (vertex 0) to each consecutive ring edge.
            let n = verts.len() as u32;
            for k in 0..n {
                mesh.indices
                    .extend_from_slice(&[0, 1 + k, 1 + (k + 1) % n]);
            }
            meshes.push(mesh);
        }
    }
    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds: Vec::new(),
        meshes,
        bounds: molecule.bounding_box_for(None),
    }
}

/// Valence-bond wedge fills (C++ `BallStickRenderer` VBMODE_TYPE1): draw each
/// stereo-flagged bond as a flat triangle — apex at atom1 (the stereo origin),
/// widening to `width` at atom2. Reads the MDL/SDF bond `stereo` flag (1 = up
/// wedge, 6 = down/hash), the FAITHFUL source for which bonds are stereo bonds
/// (no chirality perception needed). HONEST SCOPE: the solid-vs-hashed fill
/// distinction (up vs down) is a texture detail not represented here — both
/// produce the same wedge triangle geometry; only the presence/shape is modeled.
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
        for position in [apex, wide + half, wide - half] {
            mesh.vertices.push(MeshVertex {
                position,
                normal,
                color,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2]);
        meshes.push(mesh);
    }
    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds: Vec::new(),
        meshes,
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

/// Dots spaced (by arc length) `dot_sep` apart along the cubic Bézier through
/// `p1..p4`. Port of C++ `DisoRenderer::renderBezierDots` (accumulate the
/// differential length, emit a dot each time it crosses a `dot_sep` threshold).
fn bezier_dots(p1: Vector4, p2: Vector4, p3: Vector4, p4: Vector4, dot_sep: f64) -> Vec<Vector4> {
    let ndiv = 200usize;
    let dot_sep = dot_sep.max(0.05);
    let mut dots = Vec::new();
    let mut prev = p1;
    let mut len = 0.0;
    let mut thres = dot_sep;
    for i in 1..ndiv {
        let t = i as f64 / ndiv as f64;
        let v = cubic_bezier_point(p1, p2, p3, p4, t);
        len += (v - prev).length3d();
        prev = v;
        if len > thres {
            thres += dot_sep;
            dots.push(v);
        }
    }
    dots
}

/// Disorder representation (C++ `DisoRenderer`): a dotted Bézier line bridging
/// each chain gap — consecutive same-chain residues that are NOT bonded to each
/// other (a missing/disordered stretch). The curve runs through the residues'
/// pivot atoms, with the neighbouring pivots giving the Bézier tangents.
pub fn disorder_lines_snapshot(
    molecule: &MolCoord,
    dot_sep: f64,
    dot_radius: f64,
    loopsize: f64,
    loopsize2: f64,
) -> RenderSceneSnapshot {
    // C++ DisoRenderer: loopsize scales the N-terminal Bézier tangent (gap
    // bulge amplitude); loopsize2 the C-terminal one, with a negative loopsize2
    // meaning "reuse loopsize".
    let loop1 = loopsize;
    let loop2 = if loopsize2 < 0.0 { loopsize } else { loopsize2 };
    use std::collections::{HashMap, HashSet};
    let summaries = molecule.residue_summaries();

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
    let pivot = |ri: usize| -> Option<Vector4> {
        let s = &summaries[ri];
        for name in ["CA", "P", "C1'", "O5'"] {
            if let Some(&ai) = s
                .atom_indices
                .iter()
                .find(|&&ai| molecule.atoms()[ai].name.trim().eq_ignore_ascii_case(name))
            {
                return Some(molecule.atoms()[ai].position);
            }
        }
        None
    };
    let same_chain = |a: usize, b: usize| summaries[a].key.chain_id == summaries[b].key.chain_id;

    let mut atoms = Vec::new();
    for i in 1..summaries.len() {
        if !same_chain(i - 1, i) || linked.contains(&(i - 1, i)) {
            continue;
        }
        let (Some(p1), Some(p4)) = (pivot(i - 1), pivot(i)) else {
            continue;
        };
        let chord = p4 - p1;
        let third = chord.length3d() / 3.0;
        // Bézier tangents from the surrounding pivots (chord fallback at the ends).
        let tan1 = if i >= 2 && same_chain(i - 2, i - 1) {
            pivot(i - 2).map(|p0| normalize_or(p1 - p0, chord) * third)
        } else {
            None
        }
        .unwrap_or(chord * (1.0 / 3.0));
        let tan4 = if i + 1 < summaries.len() && same_chain(i, i + 1) {
            pivot(i + 1).map(|p5| normalize_or(p5 - p4, chord) * third)
        } else {
            None
        }
        .unwrap_or(chord * (1.0 / 3.0));
        let p2 = p1 + tan1 * loop1;
        let p3 = p4 - tan4 * loop2;
        for dot in bezier_dots(p1, p2, p3, p4, dot_sep) {
            atoms.push(RenderAtom::new(dot, dot_radius as f32, None));
        }
    }
    RenderSceneSnapshot {
        atoms,
        bonds: Vec::new(),
        meshes: Vec::new(),
        bounds: molecule.bounding_box_for(None),
    }
}

/// Nucleic-acid base representation (C++ `NARenderer`, the `rendResidSimple1`
/// core): each nucleotide's base is shown as an oriented stick along its base axis
/// — from the sugar `C1'` to the glycosidic nitrogen (`N9` for purines, `N1` for
/// pyrimidines), then on to the far ring atom (`N1`/`N3`). Purine vs pyrimidine is
/// detected by the presence of `N9`. (The base-pair *ladder* rungs and the filled
/// base block are deferred — they need base-pair partners / per-base polygons.)
pub fn na_base_sticks_snapshot(
    molecule: &MolCoord,
    radius: f64,
    show_basepair: bool,
) -> RenderSceneSnapshot {
    use std::collections::{HashMap, HashSet};
    let summaries = molecule.residue_summaries();
    let pos_in = |indices: &[usize], name: &str| -> Option<Vector4> {
        indices
            .iter()
            .find(|&&ai| molecule.atoms()[ai].name.trim().eq_ignore_ascii_case(name))
            .map(|&ai| molecule.atoms()[ai].position)
    };
    let mut bonds = Vec::new();
    // Glycosidic-N position per residue id, for the base-pair ladder rungs.
    let mut glyco_by_resid: HashMap<i32, Vector4> = HashMap::new();
    for s in &summaries {
        let Some(sugar) = pos_in(&s.atom_indices, "C1'").or_else(|| pos_in(&s.atom_indices, "C1*"))
        else {
            continue;
        };
        let (glyco, far) = match pos_in(&s.atom_indices, "N9") {
            Some(n9) => (n9, pos_in(&s.atom_indices, "N1")), // purine
            None => match pos_in(&s.atom_indices, "N1") {
                Some(n1) => (n1, pos_in(&s.atom_indices, "N3")), // pyrimidine
                None => continue,
            },
        };
        bonds.push(RenderBond::new(sugar, glyco, radius as f32, None));
        if let Some(far) = far {
            bonds.push(RenderBond::new(glyco, far, radius as f32, None));
        }
        glyco_by_resid.insert(s.key.res_id, glyco);
    }

    // Base-pair ladder rungs: connect the glycosidic N of base-paired residues
    // (the `base_pair` atom int-prop set by MolCoord::calc_base_pair). C++
    // NARenderer draws these connectors only when show_basepair is on.
    if show_basepair {
        let mut drawn: HashSet<(i32, i32)> = HashSet::new();
        for s in &summaries {
            let resid = s.key.res_id;
            let partner = s
                .atom_indices
                .iter()
                .find_map(|&ai| molecule.atoms()[ai].int_prop("base_pair"));
            if let Some(partner) = partner {
                let key = (resid.min(partner), resid.max(partner));
                if drawn.insert(key) {
                    if let (Some(&a), Some(&b)) =
                        (glyco_by_resid.get(&resid), glyco_by_resid.get(&partner))
                    {
                        bonds.push(RenderBond::new(a, b, radius as f32, None));
                    }
                }
            }
        }
    }

    RenderSceneSnapshot {
        atoms: Vec::new(),
        bonds,
        meshes: Vec::new(),
        bounds: molecule.bounding_box_for(None),
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

/// Cholesky factor `M` (lower-triangular, `M·Mᵀ = U`) of the symmetric 3×3 tensor
/// `U = [u11,u22,u33,u12,u13,u23]`. `None` when `U` is not positive-definite.
fn cholesky3(u: [f64; 6]) -> Option<[[f64; 3]; 3]> {
    let (a, b, c, d, e, f) = (u[0], u[1], u[2], u[3], u[4], u[5]);
    if a <= 0.0 {
        return None;
    }
    let l00 = a.sqrt();
    let l10 = d / l00;
    let l20 = e / l00;
    let t11 = b - l10 * l10;
    if t11 <= 0.0 {
        return None;
    }
    let l11 = t11.sqrt();
    let l21 = (f - l20 * l10) / l11;
    let t22 = c - l20 * l20 - l21 * l21;
    if t22 <= 0.0 {
        return None;
    }
    let l22 = t22.sqrt();
    Some([[l00, 0.0, 0.0], [l10, l11, 0.0], [l20, l21, l22]])
}

/// Thermal-ellipsoid mesh for one atom: a unit sphere transformed by `scale·M`,
/// where `M` is the Cholesky factor of the ADP tensor `U` (so the surface is the
/// probability contour `(x-c)ᵀU⁻¹(x-c) = scale²`). Port of C++ `AnIsoURenderer`'s
/// sphere-through-the-U-transform. Vertex normals are `M⁻ᵀ·u` (the true ellipsoid
/// normal). Falls back to an isotropic sphere when `U` is not positive-definite.
pub fn anisou_ellipsoid_mesh(
    center: Vector4,
    u: [f64; 6],
    scale: f64,
    color: [f32; 4],
    stacks: usize,
    slices: usize,
) -> RenderMesh {
    let m = cholesky3(u).unwrap_or_else(|| {
        let r = (((u[0] + u[1] + u[2]) / 3.0).max(1.0e-6)).sqrt();
        [[r, 0.0, 0.0], [0.0, r, 0.0], [0.0, 0.0, r]]
    });
    let mut mesh = RenderMesh::new();
    let stacks = stacks.max(2);
    let slices = slices.max(3);
    for i in 0..=stacks {
        let theta = std::f64::consts::PI * i as f64 / stacks as f64;
        let (st, ct) = (theta.sin(), theta.cos());
        for j in 0..=slices {
            let phi = std::f64::consts::TAU * j as f64 / slices as f64;
            let (sp, cp) = (phi.sin(), phi.cos());
            let uu = [st * cp, st * sp, ct]; // unit-sphere point (= unit normal)
            // position = center + scale · M · uu
            let mu = [
                m[0][0] * uu[0],
                m[1][0] * uu[0] + m[1][1] * uu[1],
                m[2][0] * uu[0] + m[2][1] * uu[1] + m[2][2] * uu[2],
            ];
            let position = Vector4::new3(
                center.x + scale * mu[0],
                center.y + scale * mu[1],
                center.z + scale * mu[2],
            );
            // normal = normalize(M⁻ᵀ · uu): solve Mᵀ n = uu (upper-tri back-subst).
            let n2 = uu[2] / m[2][2];
            let n1 = (uu[1] - m[2][1] * n2) / m[1][1];
            let n0 = (uu[0] - m[1][0] * n1 - m[2][0] * n2) / m[0][0];
            let normal = normalize_or(Vector4::new3(n0, n1, n2), Vector4::new3(0.0, 0.0, 1.0));
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

/// Snapshot rendering atoms as anisotropic thermal ellipsoids (the geometry the
/// `AnIsoURenderer` draws). Atoms with an ANISOU record become ellipsoid meshes;
/// atoms without are drawn as sphere primitives so the structure still renders.
pub fn anisou_mesh_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
) -> RenderSceneSnapshot {
    const PROB_SCALE: f64 = 1.5382; // 50%-probability ellipsoid (ORTEP convention)
    let mut meshes = Vec::new();
    let mut atoms = Vec::new();
    for atom in molecule.atoms() {
        let spec = atom_color(&atom.element, options.color_mode);
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
                meshes.push(anisou_ellipsoid_mesh(
                    atom.position,
                    u,
                    PROB_SCALE,
                    col,
                    12,
                    16,
                ));
            }
            None => {
                atoms.push(RenderAtom::new(
                    atom.position,
                    options.bond_radius.max(0.2),
                    Some(spec),
                ));
            }
        }
    }
    RenderSceneSnapshot {
        atoms,
        bonds: Vec::new(),
        meshes,
        bounds: molecule.bounding_box_for(None),
    }
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
        }
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
        assert!((override_bond - 0.9).abs() < 1.0e-6, "inline override {override_bond}");

        // No style => the hardcoded default radius is left untouched.
        let (plain_bond, _) = styled_ballstick_radii("", None);
        assert!((plain_bond - 0.08).abs() < 1.0e-6, "plain default {plain_bond}");
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
        tessellate_styled_snapshot(obj.mol(), &options).vertices.len()
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
        let default_count: usize = ribbon_mesh_snapshot(&mol, &MoleculeRenderOptions::default())
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
                    let (dx, dy, dz) =
                        (v.position.x - c.0, v.position.y - c.1, v.position.z - c.2);
                    (dx * dx + dy * dy + dz * dz).sqrt()
                })
                .collect();
            dists.iter().cloned().fold(0.0_f64, f64::max)
                / dists.iter().cloned().fold(f64::INFINITY, f64::min).max(1.0e-9)
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
        assert!((left.tangent - right.tangent).length3d() < 1e-3, "C1 at knot");
        assert!((left.curvature - right.curvature).length3d() < 1e-3, "C2 at knot");

        // (5) The analytic tangent matches a central finite difference of position.
        let mid = spline.interpolate(1.5);
        let fd = (spline.interpolate(1.5 + eps).position - spline.interpolate(1.5 - eps).position)
            .scale(1.0 / (2.0 * eps));
        assert!((mid.tangent - fd).length3d() < 1e-4, "tangent vs finite diff");

        // (6) The curve actually bends: the mid-segment sample leaves the chord.
        let chord_mid = (pts[1] + pts[2]).scale(0.5);
        assert!((mid.position - chord_mid).length3d() > 1e-3, "curve deviates from chord");
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
        let iso = anisou_ellipsoid_mesh(center, [0.01, 0.01, 0.01, 0.0, 0.0, 0.0], scale, color, 8, 12);
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
            assert!((quad_form(d, u) - scale * scale).abs() < 1e-6, "vertex off contour");
        }

        // The principal stretch is along x (largest U component) — wider than y.
        let ext = |f: &dyn Fn(&MeshVertex) -> f64| {
            ell.vertices.iter().map(f).fold(0.0_f64, f64::max)
        };
        let max_x = ext(&|v| (v.position.x - center.x).abs());
        let max_y = ext(&|v| (v.position.y - center.y).abs());
        assert!(max_x > max_y, "elongated along the dominant axis");

        // A non-positive-definite tensor falls back to an isotropic sphere (no NaNs).
        let bad = anisou_ellipsoid_mesh(center, [-1.0, 0.01, 0.01, 0.0, 0.0, 0.0], scale, color, 6, 8);
        assert!(bad.vertices.iter().all(|v| v.position.x.is_finite()));
    }

    fn diso_ca(serial: i32, res_id: i32, x: f64) -> Atom {
        let mut a = sample_atom("CA", "C", x, 0.0, 0.0);
        a.serial = serial;
        a.res_id = res_id;
        a
    }

    #[test]
    fn disorder_lines_bridge_unbonded_chain_gaps_only() {
        // Two same-chain residues 10 Å apart with NO bond between them = a
        // disordered gap -> a dotted Bézier line bridges them.
        let mol = MolCoord::with_atoms(vec![diso_ca(1, 1, 0.0), diso_ca(2, 2, 10.0)]);
        let snap = disorder_lines_snapshot(&mol, 1.0, 0.2, 1.0, -1.0);
        assert!(!snap.atoms.is_empty(), "gap should be dotted");
        // ~10 dots (10 Å / 1 Å spacing), all between the two pivots.
        assert!(snap.atoms.len() >= 8 && snap.atoms.len() <= 12, "got {}", snap.atoms.len());
        assert!(snap
            .atoms
            .iter()
            .all(|a| a.center.x > -0.1 && a.center.x < 10.1));

        // Once the residues are bonded (linked), there is no disorder line.
        let mut linked = MolCoord::with_atoms(vec![diso_ca(1, 1, 0.0), diso_ca(2, 2, 10.0)]);
        linked.add_bond(0, 1, cuemol_molstr::BondOrder::Single);
        let snap2 = disorder_lines_snapshot(&linked, 1.0, 0.2, 1.0, -1.0);
        assert!(snap2.atoms.is_empty(), "linked residues are not disordered");
    }

    #[test]
    fn disorder_loopsize_scales_the_gap_arc_amplitude() {
        // res1-res2 and res3-res4 are bonded; res2-res3 is the unbonded gap. The
        // neighbours bend the Bézier tangents off the chord (+y), so a larger
        // loopsize bulges the disorder arc higher in +y.
        fn bent_ca(serial: i32, res_id: i32, x: f64, y: f64) -> Atom {
            let mut a = sample_atom("CA", "C", x, y, 0.0);
            a.serial = serial;
            a.res_id = res_id;
            a
        }
        let mut mol = MolCoord::with_atoms(vec![
            bent_ca(1, 1, 0.0, -2.0),
            bent_ca(2, 2, 0.0, 0.0),
            bent_ca(3, 3, 10.0, 0.0),
            bent_ca(4, 4, 10.0, -2.0),
        ]);
        mol.add_bond(0, 1, cuemol_molstr::BondOrder::Single);
        mol.add_bond(2, 3, cuemol_molstr::BondOrder::Single);
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
        assert!(first < r0, "low-B end should be thinner than the off radius");
        assert!(last > first + 0.1, "high-B end should be thicker: {first} vs {last}");
    }

    #[test]
    fn mainchain_radius_overrides_the_trace_tube_width() {
        let mol = MolCoord::with_atoms(vec![diso_ca(1, 1, 0.0), diso_ca(2, 2, 3.0), diso_ca(3, 3, 6.0)]);
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
        // Adenine (res 1) and Uracil (res 2) bases ~2 A apart -> base-paired.
        let mut mol = MolCoord::with_atoms(vec![
            nt("C1'", "A", 1, 0.0, 0.0),
            nt("N9", "A", 1, 1.0, 0.0),
            nt("N1", "A", 1, 2.0, 0.0),
            nt("C1'", "U", 2, 0.0, 2.0),
            nt("N1", "U", 2, 1.0, 2.0),
            nt("N3", "U", 2, 2.0, 2.0),
        ]);
        // Before computing base pairs, show_basepair adds no rungs (just the
        // per-base sticks: 2 per residue).
        let base_only = na_base_sticks_snapshot(&mol, 0.3, true).bonds.len();
        assert_eq!(base_only, 4);

        assert_eq!(mol.calc_base_pair(5.0, 30.0), 1);
        let on = na_base_sticks_snapshot(&mol, 0.3, true);
        assert_eq!(on.bonds.len(), base_only + 1, "one ladder rung for the pair");
        // The rung connects the glycosidic N atoms: A.N9 (1,0) <-> U.N1 (1,2).
        assert!(
            on.bonds.iter().any(|b| {
                let (f, t) = ((b.from.x, b.from.y), (b.to.x, b.to.y));
                (f == (1.0, 0.0) && t == (1.0, 2.0)) || (f == (1.0, 2.0) && t == (1.0, 0.0))
            }),
            "rung connects the paired bases' glycosidic N atoms"
        );
        // With show_basepair off, the rung is gone.
        assert_eq!(na_base_sticks_snapshot(&mol, 0.3, false).bonds.len(), base_only);
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
        let snap = ring_meshes_snapshot(&mol);
        assert_eq!(snap.meshes.len(), 1, "one ring mesh for PHE");
        let m = &snap.meshes[0];
        assert_eq!(m.triangle_count(), 6, "hexagon fan = 6 triangles");
        assert_eq!(m.vertices.len(), 7, "centroid + 6 ring atoms");
        // All vertices in the z=0 plane; the ring normal is ±z.
        assert!(m.vertices.iter().all(|v| v.position.z.abs() < 1.0e-9));
        assert!(m.vertices[0].normal.z.abs() > 0.99);

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
            intr_atom(9, 1.0, 0.0, 1.0), // torsion 6-7-8-9 = +90 deg
            intr_atom(10, 1.0, 0.0, -1.0), // torsion 6-7-8-10 = -90 deg
        ]);

        let mut r = AtomIntrRenderer::new();
        let dist = r.append_by_id(1, 0, 2, 0, true);
        let angle = r.append_angle_by_id(3, 0, 4, 0, 5, 0);
        let tors_pos = r.append_torsion_by_id(6, 0, 7, 0, 8, 0, 9, 0);
        let tors_neg = r.append_torsion_by_id(6, 0, 7, 0, 8, 0, 10, 0);
        // An unresolvable atom id yields no measurement.
        let missing = r.append_by_id(1, 0, 999, 0, true);

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
        r.append_by_id(1, 0, 2, 0, true); // distance -> 1 segment
        r.append_angle_by_id(3, 0, 4, 0, 5, 0); // angle -> 2 arm segments
        let lines = r.measurement_lines(&mol);
        assert_eq!(lines.len(), 3, "1 distance segment + 2 angle arms");
        // The distance segment spans atoms 1 (0,0,0) and 2 (3,0,0).
        assert!(lines
            .iter()
            .any(|(a, b)| (a.x == 0.0 && b.x == 3.0) || (a.x == 3.0 && b.x == 0.0)));
    }

    #[test]
    fn atom_intr_angle_arc_is_a_circular_sweep_at_the_vertex() {
        let mol = MolCoord::with_atoms(vec![
            intr_atom(1, 2.0, 0.0, 0.0), // arm 0 along +x
            intr_atom(2, 0.0, 0.0, 0.0), // vertex
            intr_atom(3, 0.0, 2.0, 0.0), // arm 2 along +y
        ]);
        let mut r = AtomIntrRenderer::new();
        r.append_angle_by_id(1, 0, 2, 0, 3, 0);
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
        r2.append_by_id(1, 0, 2, 0, true);
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
        let backbone_only = MolCoord::with_atoms(vec![
            ca_atom(1, 0.0, 0.0, 0.0),
            ca_atom(2, 1.0, 0.0, 0.0),
        ]);
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
            sample_backbone(&pivots, 8, w)
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
        let (dw, dt, _) = section_params(Secondary::Helix, 1.0, &RibbonSections::default(), &styled);
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
        let helix_color =
            section_params(Secondary::Helix, 1.0, &RibbonSections::default(), &StyledRibbonSections::default()).2;
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
                    let (dx, dy, dz) =
                        (v.position.x - cx, v.position.y - cy, v.position.z - cz);
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
        assert_f64_eq(draw.width(), 3.0);
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
        });
        assert!(draw.append_atom(&molecule, 7));
        assert!(!draw.append_atom(&molecule, 99));
        assert_eq!(draw.positions().len(), 2);

        draw.set_enabled(false);
        assert!(!draw.is_enabled());
        assert!(draw.positions().is_empty());
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
        assert_eq!(renderer.gl_render_mode(), BallStickRenderer::REND_DEFAULT);
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
        let dist_id = atom_intr.append("name N", "name CA");
        let angle_id = atom_intr.append_angle_by_id(1, 10, 2, 10, 3, 10);
        let torsion_id = atom_intr.append_torsion_by_id(1, 10, 2, 10, 3, 10, 4, 10);
        assert_eq!(atom_intr.defs().len(), 3);
        assert!(atom_intr.get_defs_json().contains("\"type\":\"torsion\""));
        assert!(atom_intr.remove(angle_id));
        assert!(!atom_intr.remove(angle_id));
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

        let spline = SplineRenderer::new();
        assert_eq!(spline.type_name(), "spline");
        assert_eq!(spline.axialdetail(), 6);
        assert!(spline.smoothcolor());
        assert_eq!(spline.start_cap_type(), SplineRenderer::TUBE_CAP_SPHR);
        assert_eq!(spline.end_cap_type(), SplineRenderer::TUBE_CAP_SPHR);
    }

    #[test]
    fn tube_ribbon_and_na_defaults_match_qif() {
        let section = TubeSection::new();
        assert_f64_eq(section.width(), 0.35);
        assert_f64_eq(section.tuber(), 1.0);
        assert_f64_eq(section.sharp(), 0.4);
        assert_eq!(section.section_type(), TubeSection::TS_ELLIPTICAL);
        assert_eq!(section.detail(), 16);

        let jct = JctTable::new();
        assert_eq!(jct.jct_type(), JctTable::JT_SMOOTH1);
        assert_f64_eq(jct.gamma(), 2.2);
        assert_f64_eq(jct.basw(), 0.0);
        assert_f64_eq(jct.arrow(), 1.8);

        let tube = TubeRenderer::new();
        assert_eq!(tube.type_name(), "tube");
        assert_eq!(tube.putty_mode(), TubeRenderer::TBR_PUTTY_OFF);
        assert_eq!(tube.putty_tgt(), TubeRenderer::TBR_PUTTY_BFAC);
        assert_f64_eq(tube.putty_hiscl(), 3.0);
        assert_f64_eq(tube.putty_loscl(), 3.0);

        let na = NARenderer::new();
        assert_eq!(na.type_name(), "nucl");
        assert!(na.show_tube());
        assert!(na.show_basepair());
        assert_f64_eq(na.base_size(), 0.5);
        assert_eq!(na.base_detail(), 3);
        assert_f64_eq(na.base_thick(), 0.5);
        assert_eq!(na.base_type(), NARenderer::NAREND_BP);

        let ribbon = RibbonRenderer::new();
        assert_eq!(ribbon.type_name(), "ribbon");
        assert_f64_eq(ribbon.helix_smooth(), 0.0);
        assert_f64_eq(ribbon.sheet_smooth(), 0.5);
        assert_f64_eq(ribbon.coil_smooth(), 0.0);
        assert!(!ribbon.helix_usebackcol());
        assert!(!ribbon.sheet_usesidecol());

        let cartoon = Ribbon2Renderer::new();
        assert_eq!(cartoon.type_name(), "cartoon");
        assert_eq!(cartoon.axialdetail(), 8);
        assert!(!cartoon.smoothcolor());
        assert_f64_eq(cartoon.anchor_weight(), 10.0);
        assert!(!cartoon.helix_ribbon());
        assert_f64_eq(cartoon.helix_smooth(), 3.0);
        assert_eq!(cartoon.helix_width_mode(), Ribbon2Renderer::HWIDTH_AVER);
        assert!(cartoon.is_width_aver());
        assert_f64_eq(cartoon.helix_width(), 2.3);
        assert_f64_eq(cartoon.helix_extend(), 0.5);
        assert_f64_eq(cartoon.helix_wsmooth(), 5.0);
        assert_f64_eq(cartoon.helix_wplus(), 0.2);
        assert_f64_eq(cartoon.sheet_smooth(), 2.0);
        assert_f64_eq(cartoon.sheet_wsmooth(), 5.0);
        assert_f64_eq(cartoon.coil_smooth(), -1.0);
        assert_eq!(cartoon.start_cap_type(), Ribbon2Renderer::TUBE_CAP_FLAT);
        assert_eq!(cartoon.end_cap_type(), Ribbon2Renderer::TUBE_CAP_FLAT);
        assert!(!cartoon.segend_fade());
        assert!(!cartoon.dump_curvature());
    }

    #[test]
    fn coloring_defaults_and_table_ops_match_qif() {
        let mut rainbow = RainbowColoring::new();
        assert_eq!(rainbow.type_name(), "rainbow");
        assert_f64_eq(rainbow.start_hue(), 240.0);
        assert_f64_eq(rainbow.end_hue(), 0.0);
        assert_f64_eq(rainbow.sat(), 1.0);
        assert_f64_eq(rainbow.bri(), 1.0);
        assert_eq!(rainbow.mode(), RainbowColoring::RBC_MOL);
        assert_eq!(rainbow.incr_mode(), RainbowColoring::RBC_INCR_RESID);
        rainbow.set_mode(RainbowColoring::RBC_CHAIN);
        assert_eq!(rainbow.mode(), RainbowColoring::RBC_CHAIN);

        let mut script = ScriptColoring::new();
        assert_eq!(script.type_name(), "script");
        assert_eq!(script.script(), "");
        script.set_script("atom.elem == 'C'");
        assert_eq!(script.script(), "atom.elem == 'C'");

        let mut paint = PaintColoring::new();
        assert_eq!(paint.type_name(), "paint");
        assert!(paint.is_empty());
        paint.append("name CA", SolidColor::from_rgb(1.0, 0.0, 0.0));
        paint.insert_before(0, "name N", SolidColor::from_rgb(0.0, 1.0, 0.0));
        paint.insert_before(-1, "name O", SolidColor::from_rgb(0.0, 0.0, 1.0));
        assert_eq!(paint.len(), 3);
        assert_eq!(paint.get_selection_at(0), Some("name N"));
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
