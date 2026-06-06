use cuemol_core::Vector4;
use cuemol_gfx::{ColorSpec, SolidColor};
use cuemol_molstr::{Atom, MolCoord, ResidIterator, ResidueView};
use cuemol_qsys::DrawObj;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoleculeColorMode {
    Element,
    Uniform,
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
        }
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
    let atoms = if options.draw_atoms {
        molecule
            .atoms()
            .iter()
            .enumerate()
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
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let bonds = if options.draw_bonds {
        molecule
            .bonds()
            .iter()
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
    RenderSceneSnapshot {
        atoms,
        bonds,
        meshes: Vec::new(),
        bounds: molecule.bounding_box_for(None),
    }
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
    let tube_radius = (options.bond_radius * 3.0).clamp(0.18, 0.6);

    let chains = collect_mainchain_pivots(molecule);
    let mut atoms = Vec::new();
    let mut bonds = Vec::new();

    for (chain_index, pivots) in chains.iter().enumerate() {
        let color = mainchain_chain_color(chain_index);
        let positions = pivots.iter().map(|point| point.pos).collect::<Vec<_>>();
        if positions.len() == 1 {
            atoms.push(RenderAtom::new(positions[0], tube_radius, Some(color.clone())));
            continue;
        }
        let samples = catmull_rom_samples(&positions, SUBDIV);
        for point in &samples {
            atoms.push(RenderAtom::new(*point, tube_radius, Some(color.clone())));
        }
        for segment in samples.windows(2) {
            bonds.push(RenderBond::new(
                segment[0],
                segment[1],
                tube_radius,
                Some(color.clone()),
            ));
        }
    }

    RenderSceneSnapshot {
        atoms,
        bonds,
        meshes: Vec::new(),
        bounds: molecule.bounding_box_for(None),
    }
}

/// Protein secondary-structure class of a residue, used to shape the ribbon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Secondary {
    Coil,
    Helix,
    Sheet,
}

/// One backbone pivot: its position plus the residue's secondary structure.
#[derive(Debug, Clone, Copy)]
struct MainchainPoint {
    pos: Vector4,
    ss: Secondary,
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
fn collect_mainchain_pivots(molecule: &MolCoord) -> Vec<Vec<MainchainPoint>> {
    let mut chains: Vec<(String, Vec<MainchainPoint>)> = Vec::new();
    let mut iter = ResidIterator::all(molecule);
    iter.first();
    while iter.has_more() {
        if let Some(residue) = iter.get() {
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
        iter.next();
    }
    chains.into_iter().map(|(_, points)| points).collect()
}

/// Pick the residue's backbone pivot atom: CA (protein) then nucleic-acid fallbacks.
fn mainchain_pivot_point(molecule: &MolCoord, residue: &ResidueView) -> Option<MainchainPoint> {
    let atoms = residue.atoms();
    for name in ["CA", "P", "C1'", "O5'"] {
        if let Some(atom) = atoms
            .iter()
            .find(|atom| atom.name.trim().eq_ignore_ascii_case(name))
        {
            return Some(MainchainPoint {
                pos: Vector4::new3(atom.position.x, atom.position.y, atom.position.z),
                ss: residue_secondary(molecule, atom),
            });
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

/// Sample a Catmull-Rom spline through `points`, with `subdiv` samples per segment.
///
/// Catmull-Rom passes through every control point, so the curve runs exactly
/// along the backbone atoms while smoothing the path between them.
fn catmull_rom_samples(points: &[Vector4], subdiv: usize) -> Vec<Vector4> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let subdiv = subdiv.max(1);
    let n = points.len();
    let mut out = Vec::with_capacity((n - 1) * subdiv + 1);
    for i in 0..n - 1 {
        let p0 = points[i.saturating_sub(1)];
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = points[(i + 2).min(n - 1)];
        for step in 0..subdiv {
            let t = step as f64 / subdiv as f64;
            out.push(catmull_rom_point(p0, p1, p2, p3, t));
        }
    }
    out.push(points[n - 1]);
    out
}

fn catmull_rom_point(p0: Vector4, p1: Vector4, p2: Vector4, p3: Vector4, t: f64) -> Vector4 {
    let t2 = t * t;
    let t3 = t2 * t;
    // Uniform Catmull-Rom basis (tension 0.5).
    let a = p1.scale(2.0);
    let b = (p2 - p0).scale(t);
    let c = (p0.scale(2.0) - p1.scale(5.0) + p2.scale(4.0) - p3).scale(t2);
    let d = (p1.scale(3.0) - p0 + p3 - p2.scale(3.0)).scale(t3);
    (a + b + c + d).scale(0.5)
}

/// Build a secondary-structure cartoon snapshot: a ribbon swept along each
/// chain's splined backbone, shaped per residue (helix/sheet = wide flat band,
/// coil = thin tube) and colored by secondary structure.
pub fn ribbon_mesh_snapshot(
    molecule: &MolCoord,
    options: &MoleculeRenderOptions,
) -> RenderSceneSnapshot {
    const SUBDIV: usize = 8;
    let scale = ((options.bond_radius * 4.0).clamp(0.25, 0.8)) as f64;
    let chains = collect_mainchain_pivots(molecule);
    let mut meshes = Vec::new();
    for pivots in &chains {
        if pivots.len() < 2 {
            continue;
        }
        let (samples, sections, width_scales) = sample_backbone(pivots, SUBDIV);
        let mesh = ribbon_tube_mesh(&samples, &sections, &width_scales, scale);
        if !mesh.vertices.is_empty() {
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

/// Spline the backbone positions and carry a per-sample secondary structure.
///
/// A spline segment between two residues keeps its class only when both ends
/// agree; mixed boundaries fall back to coil so the cross-section tapers
/// smoothly across secondary-structure transitions.
fn sample_backbone(
    points: &[MainchainPoint],
    subdiv: usize,
) -> (Vec<Vector4>, Vec<Secondary>, Vec<f64>) {
    let positions = points.iter().map(|point| point.pos).collect::<Vec<_>>();
    let samples = catmull_rom_samples(&positions, subdiv);
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
fn section_params(ss: Secondary, scale: f64) -> (f64, f64, [f32; 4]) {
    match ss {
        Secondary::Helix => (scale * 2.2, scale * 0.7, [0.95, 0.32, 0.42, 1.0]),
        Secondary::Sheet => (scale * 2.0, scale * 0.6, [0.95, 0.82, 0.30, 1.0]),
        Secondary::Coil => (scale, scale, [0.55, 0.85, 0.90, 1.0]),
    }
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

    // Ring vertices: an ellipse of TUBE_RADIAL points at each sample, with width
    // along the binormal and thickness along the normal.
    let mut vertices = Vec::with_capacity(n * TUBE_RADIAL);
    for (i, &center) in samples.iter().enumerate() {
        let (normal, binormal) = frames[i];
        let (base_width, thick, color) = section_params(sections[i], scale);
        let width = base_width * width_scales[i];
        for j in 0..TUBE_RADIAL {
            let theta = (j as f64) / (TUBE_RADIAL as f64) * std::f64::consts::TAU;
            let (sin, cos) = theta.sin_cos();
            let offset = binormal.scale(cos * width) + normal.scale(sin * thick);
            // Outward ellipse normal: gradient of (u/width)^2 + (v/thick)^2.
            let nrm = normalize_or(
                binormal.scale(cos * thick) + normal.scale(sin * width),
                normal,
            );
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

fn normalize_or(vec: Vector4, fallback: Vector4) -> Vector4 {
    vec.normalize3d().unwrap_or(fallback)
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
        let helix_color = section_params(Secondary::Helix, 1.0).2;
        assert!(snap.meshes[0]
            .vertices
            .iter()
            .all(|vertex| vertex.color == helix_color));
        assert!(snap.meshes[0]
            .vertices
            .iter()
            .all(|vertex| (vertex.normal.length3d() - 1.0).abs() < 1.0e-5));
        // A helix band is wider than it is thick (flat ribbon, not a tube).
        let (width, thick, _) = section_params(Secondary::Helix, 1.0);
        assert!(width > thick * 2.0);
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
