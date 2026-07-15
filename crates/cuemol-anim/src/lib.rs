use cuemol_core::{Matrix4, Uid, Vector4};
use cuemol_molstr::MolCoord;
use std::sync::atomic::{AtomicU64, Ordering};

pub type TimeValue = i64;

static NEXT_ANIM_UID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationKeyframe {
    pub target: Uid,
    pub time: f64,
}

impl AnimationKeyframe {
    pub fn new(target: Uid, time: f64) -> Self {
        Self { target, time }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimObj {
    uid: Uid,
    name: String,
    disabled: bool,
    time_ref_name: String,
    rel_start: TimeValue,
    rel_end: TimeValue,
    abs_start: TimeValue,
    abs_end: TimeValue,
    quadric: f64,
    coeff: f64,
    absc: f64,
    grad: f64,
}

impl Default for AnimObj {
    fn default() -> Self {
        let mut value = Self {
            uid: Uid::new(NEXT_ANIM_UID.fetch_add(1, Ordering::Relaxed)),
            name: String::new(),
            disabled: false,
            time_ref_name: String::new(),
            rel_start: 0,
            rel_end: 0,
            abs_start: 0,
            abs_end: 0,
            quadric: 0.0,
            coeff: 0.0,
            absc: 0.0,
            grad: 1.0,
        };
        value.set_quadric(0.0);
        value
    }
}

impl AnimObj {
    pub const AO_PRE: i32 = 0;
    pub const AO_ACTIVE: i32 = 1;
    pub const AO_POST: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn get_uid(&self) -> Uid {
        self.uid()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    pub fn disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled()
    }

    pub fn set_disabled(&mut self, value: bool) {
        self.disabled = value;
    }

    pub fn time_ref_name(&self) -> &str {
        &self.time_ref_name
    }

    pub fn get_time_ref_name(&self) -> &str {
        self.time_ref_name()
    }

    pub fn set_time_ref_name(&mut self, value: impl Into<String>) {
        self.time_ref_name = value.into();
    }

    pub fn rel_start(&self) -> TimeValue {
        self.rel_start
    }

    pub fn get_scr_rel_start(&self) -> TimeValue {
        self.rel_start()
    }

    pub fn set_scr_rel_start(&mut self, value: TimeValue) {
        self.set_rel_start(value);
    }

    pub fn set_rel_start(&mut self, value: TimeValue) {
        self.rel_start = value;
        if self.rel_start > self.rel_end {
            self.rel_end = value;
        }
    }

    pub fn rel_end(&self) -> TimeValue {
        self.rel_end
    }

    pub fn get_scr_rel_end(&self) -> TimeValue {
        self.rel_end()
    }

    pub fn set_scr_rel_end(&mut self, value: TimeValue) {
        self.set_rel_end(value);
    }

    pub fn set_rel_end(&mut self, value: TimeValue) {
        self.rel_end = value;
        if self.rel_start > self.rel_end {
            self.rel_start = value;
        }
    }

    pub fn abs_start(&self) -> TimeValue {
        self.abs_start
    }

    pub fn get_scr_abs_start(&self) -> TimeValue {
        self.abs_start()
    }

    pub fn set_abs_start(&mut self, value: TimeValue) {
        self.abs_start = value;
    }

    pub fn abs_end(&self) -> TimeValue {
        self.abs_end
    }

    pub fn get_scr_abs_end(&self) -> TimeValue {
        self.abs_end()
    }

    pub fn set_abs_end(&mut self, value: TimeValue) {
        self.abs_end = value;
    }

    pub fn quadric(&self) -> f64 {
        self.quadric
    }

    pub fn get_quadric(&self) -> f64 {
        self.quadric()
    }

    pub fn set_quadric(&mut self, value: f64) {
        if value < 0.01 {
            self.quadric = -0.0;
            self.coeff = 0.0;
            self.grad = 1.0;
            self.absc = 0.0;
        } else {
            self.quadric = value.clamp(0.0, 0.5);
            self.coeff = 1.0 / (2.0 * self.quadric * (1.0 - self.quadric));
            self.grad = 1.0 / (1.0 - self.quadric);
            self.absc = 0.5 * (1.0 - self.grad);
        }
    }

    pub fn conv_rho(&self, rho: f64) -> f64 {
        let rho = rho.clamp(0.0, 1.0);
        if rho < 0.5 {
            self.conv_rho_impl(rho)
        } else {
            1.0 - self.conv_rho_impl(1.0 - rho)
        }
    }

    pub fn rho_at(&self, elapsed: TimeValue) -> f64 {
        let span = self.abs_end - self.abs_start;
        if span == 0 {
            if elapsed - self.abs_start < 0 {
                0.0
            } else {
                1.0
            }
        } else {
            self.conv_rho((elapsed - self.abs_start) as f64 / span as f64)
        }
    }

    fn conv_rho_impl(&self, rho: f64) -> f64 {
        if rho < self.quadric {
            self.coeff * rho * rho
        } else {
            self.grad * rho + self.absc
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NoopAnimObj {
    base: AnimObj,
}

impl NoopAnimObj {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "NoopAnimObj"
    }

    pub fn base(&self) -> &AnimObj {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        &mut self.base
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CamMotion {
    base: AnimObj,
    endcam: String,
    ignorecenter: bool,
    ignorerotate: bool,
    ignorezoom: bool,
    ignoreslab: bool,
    keepquatpositive: bool,
}

impl Default for CamMotion {
    fn default() -> Self {
        Self {
            base: AnimObj::new(),
            endcam: String::new(),
            ignorecenter: false,
            ignorerotate: false,
            ignorezoom: false,
            ignoreslab: false,
            keepquatpositive: true,
        }
    }
}

impl CamMotion {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base(&self) -> &AnimObj {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        &mut self.base
    }

    pub fn endcam(&self) -> &str {
        &self.endcam
    }

    pub fn get_dest_cam_name(&self) -> &str {
        self.endcam()
    }

    pub fn set_endcam(&mut self, value: impl Into<String>) {
        self.endcam = value.into();
    }

    pub fn set_dest_cam_name(&mut self, value: impl Into<String>) {
        self.set_endcam(value);
    }

    pub fn ignorecenter(&self) -> bool {
        self.ignorecenter
    }

    pub fn set_ignorecenter(&mut self, value: bool) {
        self.ignorecenter = value;
    }

    pub fn ignorerotate(&self) -> bool {
        self.ignorerotate
    }

    pub fn set_ignorerotate(&mut self, value: bool) {
        self.ignorerotate = value;
    }

    pub fn ignorezoom(&self) -> bool {
        self.ignorezoom
    }

    pub fn set_ignorezoom(&mut self, value: bool) {
        self.ignorezoom = value;
    }

    pub fn ignoreslab(&self) -> bool {
        self.ignoreslab
    }

    pub fn set_ignoreslab(&mut self, value: bool) {
        self.ignoreslab = value;
    }

    pub fn keepquatpositive(&self) -> bool {
        self.keepquatpositive
    }

    pub fn set_keepquatpositive(&mut self, value: bool) {
        self.keepquatpositive = value;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RealPropAnim {
    base: AnimObj,
    start_value: f64,
    end_value: f64,
    prop: String,
    rend: String,
}

impl Default for RealPropAnim {
    fn default() -> Self {
        Self {
            base: AnimObj::new(),
            start_value: 0.0,
            end_value: 1.0,
            prop: String::new(),
            rend: String::new(),
        }
    }
}

impl RealPropAnim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base(&self) -> &AnimObj {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        &mut self.base
    }

    pub fn start_value(&self) -> f64 {
        self.start_value
    }

    pub fn get_start_value(&self) -> f64 {
        self.start_value()
    }

    pub fn set_start_value(&mut self, value: f64) {
        self.start_value = value;
    }

    pub fn end_value(&self) -> f64 {
        self.end_value
    }

    pub fn get_end_value(&self) -> f64 {
        self.end_value()
    }

    pub fn set_end_value(&mut self, value: f64) {
        self.end_value = value;
    }

    pub fn prop(&self) -> &str {
        &self.prop
    }

    pub fn get_prop_name(&self) -> &str {
        self.prop()
    }

    pub fn set_prop(&mut self, value: impl Into<String>) {
        self.prop = value.into();
    }

    pub fn set_prop_name(&mut self, value: impl Into<String>) {
        self.set_prop(value);
    }

    pub fn rend(&self) -> &str {
        &self.rend
    }

    pub fn get_rend_names(&self) -> &str {
        self.rend()
    }

    pub fn set_rend(&mut self, value: impl Into<String>) {
        self.rend = value.into();
    }

    pub fn set_rend_names(&mut self, value: impl Into<String>) {
        self.set_rend(value);
    }

    pub fn value_at_rho(&self, rho: f64) -> f64 {
        self.start_value * (1.0 - rho) + self.end_value * rho
    }
}

/// Renderer-property animation base (C++ `anim::RendPropAnim`, an abstract
/// `PropAnim` subclass): a named property animated over a list of target
/// renderers. The concrete value interpolation lives in the subclasses
/// ([`RealPropAnim`] scalar, `ShowHideAnim`, `RendXformAnim`); this type holds
/// the property name + the target-renderer name list and its parsing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RendPropAnim {
    base: AnimObj,
    prop_name: String,
    rend_names: String,
    /// Parsed `(object, renderer)` target pairs (object empty = scene-level).
    rend_name_list: Vec<(String, String)>,
}

impl RendPropAnim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base(&self) -> &AnimObj {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        &mut self.base
    }

    pub fn prop_name(&self) -> &str {
        &self.prop_name
    }

    pub fn set_prop_name(&mut self, value: impl Into<String>) {
        self.prop_name = value.into();
    }

    pub fn rend_names(&self) -> &str {
        &self.rend_names
    }

    /// Parse a comma-separated target list into `(object, renderer)` pairs (port
    /// of C++ `RendPropAnim::setRendNames`): each element is `object/renderer`
    /// (split at the first `/`) or a bare `renderer` (scene-level, empty
    /// object). The normalized `rend_names` string re-joins only the non-empty
    /// entries, exactly as C++ does (empty entries stay in the pair list but are
    /// dropped from the normalized string).
    pub fn set_rend_names(&mut self, value: &str) {
        self.rend_name_list.clear();
        let mut normalized: Vec<String> = Vec::new();
        for elem in value.split(',') {
            let (sobj, srend) = match elem.find('/') {
                Some(pos) => (
                    elem[..pos].trim().to_string(),
                    elem[pos + 1..].trim().to_string(),
                ),
                None => (String::new(), elem.trim().to_string()),
            };
            if sobj.is_empty() {
                if !srend.is_empty() {
                    normalized.push(srend.clone());
                }
            } else if !srend.is_empty() {
                normalized.push(format!("{sobj}/{srend}"));
            }
            self.rend_name_list.push((sobj, srend));
        }
        self.rend_names = normalized.join(",");
    }

    pub fn rend_name_list(&self) -> &[(String, String)] {
        &self.rend_name_list
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolAnim {
    base: AnimObj,
    start_value: f64,
    end_value: f64,
    prop: String,
    mol: String,
}

impl Default for MolAnim {
    fn default() -> Self {
        Self {
            base: AnimObj::new(),
            start_value: 0.0,
            end_value: 1.0,
            prop: String::new(),
            mol: String::new(),
        }
    }
}

impl MolAnim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base(&self) -> &AnimObj {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        &mut self.base
    }

    pub fn start_value(&self) -> f64 {
        self.start_value
    }

    pub fn get_start_value(&self) -> f64 {
        self.start_value()
    }

    pub fn set_start_value(&mut self, value: f64) {
        self.start_value = value;
    }

    pub fn end_value(&self) -> f64 {
        self.end_value
    }

    pub fn get_end_value(&self) -> f64 {
        self.end_value()
    }

    pub fn set_end_value(&mut self, value: f64) {
        self.end_value = value;
    }

    pub fn prop(&self) -> &str {
        &self.prop
    }

    pub fn get_prop_name(&self) -> &str {
        self.prop()
    }

    pub fn set_prop(&mut self, value: impl Into<String>) {
        self.prop = value.into();
    }

    pub fn set_prop_name(&mut self, value: impl Into<String>) {
        self.set_prop(value);
    }

    pub fn mol(&self) -> &str {
        &self.mol
    }

    pub fn get_mol_name(&self) -> &str {
        self.mol()
    }

    pub fn set_mol(&mut self, value: impl Into<String>) {
        self.mol = value.into();
    }

    pub fn set_mol_name(&mut self, value: impl Into<String>) {
        self.set_mol(value);
    }

    pub fn value_at_rho(&self, rho: f64) -> f64 {
        self.start_value * (1.0 - rho) + self.end_value * rho
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RendXformAnim {
    base: AnimObj,
    prop: String,
    rend: String,
    start_tran: Vector4,
    end_tran: Vector4,
}

impl Default for RendXformAnim {
    fn default() -> Self {
        Self {
            base: AnimObj::new(),
            prop: "xformMat".to_string(),
            rend: String::new(),
            start_tran: Vector4::ZERO,
            end_tran: Vector4::ZERO,
        }
    }
}

impl RendXformAnim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base(&self) -> &AnimObj {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        &mut self.base
    }

    pub fn prop(&self) -> &str {
        &self.prop
    }

    pub fn get_prop_name(&self) -> &str {
        self.prop()
    }

    pub fn set_prop(&mut self, _value: impl Into<String>) {
        self.prop = "xformMat".to_string();
    }

    pub fn set_prop_name(&mut self, value: impl Into<String>) {
        self.set_prop(value);
    }

    pub fn rend(&self) -> &str {
        &self.rend
    }

    pub fn get_rend_names(&self) -> &str {
        self.rend()
    }

    pub fn set_rend(&mut self, value: impl Into<String>) {
        self.rend = value.into();
    }

    pub fn set_rend_names(&mut self, value: impl Into<String>) {
        self.set_rend(value);
    }

    pub fn start_tran(&self) -> Vector4 {
        self.start_tran
    }

    pub fn get_start_d_pos(&self) -> Vector4 {
        self.start_tran()
    }

    pub fn set_start_tran(&mut self, value: Vector4) {
        self.start_tran = value;
    }

    pub fn set_start_d_pos(&mut self, value: Vector4) {
        self.set_start_tran(value);
    }

    pub fn end_tran(&self) -> Vector4 {
        self.end_tran
    }

    pub fn get_end_d_pos(&self) -> Vector4 {
        self.end_tran()
    }

    pub fn set_end_tran(&mut self, value: Vector4) {
        self.end_tran = value;
    }

    pub fn set_end_d_pos(&mut self, value: Vector4) {
        self.set_end_tran(value);
    }

    pub fn translation_at_rho(&self, rho: f64) -> Vector4 {
        self.start_tran * (1.0 - rho) + self.end_tran * rho
    }

    pub fn matrix_at_rho(&self, rho: f64) -> Matrix4 {
        Matrix4::make_trans_mat(self.translation_at_rho(rho))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShowHideAnim {
    base: AnimObj,
    hide: bool,
    fade: bool,
    rend: String,
    tgt_alpha: f64,
}

impl Default for ShowHideAnim {
    fn default() -> Self {
        Self {
            base: AnimObj::new(),
            hide: false,
            fade: true,
            rend: String::new(),
            tgt_alpha: 1.0,
        }
    }
}

impl ShowHideAnim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base(&self) -> &AnimObj {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        &mut self.base
    }

    pub fn prop(&self) -> &'static str {
        "visible"
    }

    pub fn hide(&self) -> bool {
        self.hide
    }

    pub fn is_hide(&self) -> bool {
        self.hide()
    }

    pub fn set_hide(&mut self, value: bool) {
        self.hide = value;
    }

    pub fn fade(&self) -> bool {
        self.fade
    }

    pub fn is_fade(&self) -> bool {
        self.fade()
    }

    pub fn set_fade(&mut self, value: bool) {
        self.fade = value;
    }

    pub fn rend(&self) -> &str {
        &self.rend
    }

    pub fn get_rend_names(&self) -> &str {
        self.rend()
    }

    pub fn set_rend(&mut self, value: impl Into<String>) {
        self.rend = value.into();
    }

    pub fn set_rend_names(&mut self, value: impl Into<String>) {
        self.set_rend(value);
    }

    pub fn tgt_alpha(&self) -> f64 {
        self.tgt_alpha
    }

    pub fn get_tgt_alpha(&self) -> f64 {
        self.tgt_alpha()
    }

    pub fn set_tgt_alpha(&mut self, value: f64) {
        self.tgt_alpha = value;
    }

    pub fn alpha_at_rho(&self, rho: f64) -> f64 {
        if !self.fade {
            self.tgt_alpha
        } else if self.hide {
            (1.0 - rho) * self.tgt_alpha
        } else {
            rho * self.tgt_alpha
        }
    }

    pub fn visible_at_end(&self) -> bool {
        !self.hide
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimpleSpin {
    base: AnimObj,
    angle: f64,
    axis: Vector4,
}

impl Default for SimpleSpin {
    fn default() -> Self {
        Self {
            base: AnimObj::new(),
            angle: 0.0,
            axis: Vector4::new4(0.0, 1.0, 0.0, 0.0),
        }
    }
}

impl SimpleSpin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base(&self) -> &AnimObj {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        &mut self.base
    }

    pub fn angle(&self) -> f64 {
        self.angle
    }

    pub fn get_angle(&self) -> f64 {
        self.angle()
    }

    pub fn set_angle(&mut self, value: f64) {
        self.angle = value;
    }

    pub fn axis(&self) -> Vector4 {
        self.axis
    }

    pub fn get_scr_axis(&self) -> Vector4 {
        self.axis()
    }

    pub fn set_axis(&mut self, value: Vector4) -> bool {
        let len = value.length();
        if len.abs() < 1.0e-8 {
            return false;
        }
        self.axis = value / len;
        true
    }

    pub fn set_scr_axis(&mut self, value: Vector4) -> bool {
        self.set_axis(value)
    }

    pub fn angle_at_rho(&self, rho: f64) -> f64 {
        self.angle * rho
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideInOutAnim {
    xform: RendXformAnim,
    hide: bool,
    direction: f64,
    distance: f64,
    rend: String,
}

impl Default for SlideInOutAnim {
    fn default() -> Self {
        let mut xform = RendXformAnim::new();
        xform.set_prop("visible");
        Self {
            xform,
            hide: false,
            direction: 0.0,
            distance: 1.0,
            rend: String::new(),
        }
    }
}

impl SlideInOutAnim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base(&self) -> &AnimObj {
        self.xform.base()
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        self.xform.base_mut()
    }

    pub fn prop(&self) -> &'static str {
        "visible"
    }

    pub fn rend(&self) -> &str {
        &self.rend
    }

    pub fn get_rend_names(&self) -> &str {
        self.rend()
    }

    pub fn set_rend(&mut self, value: impl Into<String>) {
        self.rend = value.into();
    }

    pub fn set_rend_names(&mut self, value: impl Into<String>) {
        self.set_rend(value);
    }

    pub fn hide(&self) -> bool {
        self.hide
    }

    pub fn is_hide(&self) -> bool {
        self.hide()
    }

    pub fn set_hide(&mut self, value: bool) {
        self.hide = value;
    }

    pub fn direction(&self) -> f64 {
        self.direction
    }

    pub fn get_direction(&self) -> f64 {
        self.direction()
    }

    pub fn set_direction(&mut self, value: f64) {
        self.direction = value;
    }

    pub fn distance(&self) -> f64 {
        self.distance
    }

    pub fn get_distance(&self) -> f64 {
        self.distance()
    }

    pub fn set_distance(&mut self, value: f64) {
        self.distance = value;
    }

    pub fn calc_slide_positions(&self, zoom: f64) -> (Vector4, Vector4) {
        let dist = zoom * self.distance;
        let theta = self.direction.to_radians();
        let offset = Vector4::new3(dist * theta.cos(), dist * theta.sin(), 0.0);
        if self.hide {
            (Vector4::ZERO, offset)
        } else {
            (offset, Vector4::ZERO)
        }
    }

    pub fn translation_at_rho(&self, zoom: f64, rho: f64) -> Vector4 {
        let (start, end) = self.calc_slide_positions(zoom);
        start * (1.0 - rho) + end * rho
    }
}

macro_rules! impl_anim_obj_deref {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl std::ops::Deref for $ty {
                type Target = AnimObj;

                fn deref(&self) -> &Self::Target {
                    self.base()
                }
            }

            impl std::ops::DerefMut for $ty {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    self.base_mut()
                }
            }
        )+
    };
}

impl_anim_obj_deref!(
    NoopAnimObj,
    CamMotion,
    RealPropAnim,
    RendPropAnim,
    MolAnim,
    RendXformAnim,
    ShowHideAnim,
    SimpleSpin,
    SlideInOutAnim,
);

#[derive(Debug, Clone, PartialEq)]
pub struct MorphFrame {
    name: String,
    src: String,
    srctype: String,
    positions: Vec<Vector4>,
}

impl MorphFrame {
    pub fn new(
        name: impl Into<String>,
        src: impl Into<String>,
        srctype: impl Into<String>,
        positions: Vec<Vector4>,
    ) -> Self {
        Self {
            name: name.into(),
            src: src.into(),
            srctype: srctype.into(),
            positions,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn positions(&self) -> &[Vector4] {
        &self.positions
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MorphMol {
    molecule: MolCoord,
    frame: f64,
    scale_frame: bool,
    frames: Vec<MorphFrame>,
}

impl Default for MorphMol {
    fn default() -> Self {
        Self {
            molecule: MolCoord::new(),
            frame: 0.0,
            scale_frame: false,
            frames: Vec::new(),
        }
    }
}

impl MorphMol {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn molecule(&self) -> &MolCoord {
        &self.molecule
    }

    pub fn molecule_mut(&mut self) -> &mut MolCoord {
        &mut self.molecule
    }

    pub fn frame(&self) -> f64 {
        self.frame
    }

    pub fn get_frame(&self) -> f64 {
        self.frame()
    }

    pub fn set_frame(&mut self, value: f64) {
        self.frame = value;
    }

    pub fn scale_frame(&self) -> bool {
        self.scale_frame
    }

    pub fn is_scale_frame(&self) -> bool {
        self.scale_frame()
    }

    pub fn set_scale_frame(&mut self, value: bool) {
        self.scale_frame = value;
    }

    pub fn frame_size(&self) -> usize {
        self.frames.len()
    }

    pub fn get_frame_size(&self) -> usize {
        self.frame_size()
    }

    pub fn nframe(&self) -> usize {
        self.frame_size()
    }

    pub fn frames(&self) -> &[MorphFrame] {
        &self.frames
    }

    pub fn insert_before(&mut self, molecule: &MolCoord, index: usize) {
        let positions = molecule.atoms().iter().map(|atom| atom.position).collect();
        let frame = MorphFrame::new("", "", "molecule", positions);
        if index >= self.frames.len() {
            self.frames.push(frame);
        } else {
            self.frames.insert(index, frame);
        }
    }

    pub fn remove_frame(&mut self, index: usize) -> bool {
        if index >= self.frames.len() {
            false
        } else {
            self.frames.remove(index);
            true
        }
    }

    pub fn append_frame(&mut self, frame: MorphFrame) {
        self.frames.push(frame);
    }

    pub fn append_this_frame(&mut self) -> bool {
        if !self.frames.is_empty() {
            return false;
        }
        let positions = self
            .molecule
            .atoms()
            .iter()
            .map(|atom| atom.position)
            .collect();
        self.frames
            .push(MorphFrame::new("(this)", "<this>", "<this>", positions));
        true
    }

    pub fn frame_info_json(&self) -> String {
        let body = self
            .frames
            .iter()
            .map(|frame| {
                let name = if frame.srctype == "<this>" {
                    "(this)"
                } else {
                    &frame.name
                };
                format!(
                    "{{\"name\": \"{}\",\"src\": \"{}\",\"srctype\": \"{}\"}}",
                    json_escape(name),
                    json_escape(&frame.src),
                    json_escape(&frame.srctype)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{body}]")
    }

    pub fn get_frame_info_json(&self) -> String {
        self.frame_info_json()
    }

    pub fn interpolated_positions(&self, frame: f64) -> Vec<Vector4> {
        let nframes = self.frames.len();
        if nframes == 0 {
            return Vec::new();
        }
        if nframes == 1 {
            return self.frames[0].positions.clone();
        }
        let x = if self.scale_frame {
            frame.clamp(0.0, (nframes - 1) as f64)
        } else {
            frame.clamp(0.0, 1.0) * (nframes - 1) as f64
        };
        let index = x.floor() as usize;
        let rho = x - index as f64;
        if index >= nframes - 1 || rho.abs() < 1.0e-8 {
            return self.frames[index.min(nframes - 1)].positions.clone();
        }
        let lhs = &self.frames[index].positions;
        let rhs = &self.frames[index + 1].positions;
        lhs.iter()
            .zip(rhs)
            .map(|(l, r)| *l * (1.0 - rho) + *r * rho)
            .collect()
    }
}

impl std::ops::Deref for MorphMol {
    type Target = MolCoord;

    fn deref(&self) -> &Self::Target {
        &self.molecule
    }
}

impl std::ops::DerefMut for MorphMol {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.molecule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimPlayState {
    Stop,
    Play,
    Pause,
}

impl AnimPlayState {
    pub fn code(self) -> i32 {
        match self {
            Self::Stop => AnimMgr::AM_STOP,
            Self::Play => AnimMgr::AM_RUNNING,
            Self::Pause => AnimMgr::AM_PAUSED,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimMgr {
    uid: Uid,
    length: TimeValue,
    startcam: String,
    loop_enabled: bool,
    elapsed: TimeValue,
    play_state: AnimPlayState,
    frame_no: i32,
    objects: Vec<AnimObj>,
}

impl Default for AnimMgr {
    fn default() -> Self {
        Self {
            uid: Uid::new(NEXT_ANIM_UID.fetch_add(1, Ordering::Relaxed)),
            length: 0,
            startcam: String::new(),
            loop_enabled: false,
            elapsed: 0,
            play_state: AnimPlayState::Stop,
            frame_no: 0,
            objects: Vec::new(),
        }
    }
}

impl AnimMgr {
    pub const AM_STOP: i32 = 0;
    pub const AM_RUNNING: i32 = 1;
    pub const AM_PAUSED: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn get_uid(&self) -> Uid {
        self.uid()
    }

    pub fn length(&self) -> TimeValue {
        self.length
    }

    pub fn get_scr_length(&self) -> TimeValue {
        self.length()
    }

    pub fn set_scr_length(&mut self, value: TimeValue) {
        self.set_length(value);
    }

    pub fn set_length(&mut self, value: TimeValue) {
        self.length = value.max(0);
    }

    pub fn startcam(&self) -> &str {
        &self.startcam
    }

    pub fn get_start_cam_name(&self) -> &str {
        self.startcam()
    }

    pub fn set_start_cam_name(&mut self, value: impl Into<String>) {
        self.set_startcam(value);
    }

    pub fn set_startcam(&mut self, value: impl Into<String>) {
        self.startcam = value.into();
    }

    pub fn loop_enabled(&self) -> bool {
        self.loop_enabled
    }

    pub fn is_loop(&self) -> bool {
        self.loop_enabled()
    }

    pub fn set_loop_enabled(&mut self, value: bool) {
        self.loop_enabled = value;
    }

    pub fn r#loop(&self) -> bool {
        self.loop_enabled()
    }

    pub fn set_loop(&mut self, value: bool) {
        self.set_loop_enabled(value);
    }

    pub fn elapsed(&self) -> TimeValue {
        self.elapsed
    }

    pub fn get_scr_elapsed(&self) -> TimeValue {
        self.elapsed()
    }

    pub fn play_state(&self) -> AnimPlayState {
        self.play_state
    }

    pub fn play_state_code(&self) -> i32 {
        self.play_state.code()
    }

    pub fn get_play_state(&self) -> i32 {
        self.play_state_code()
    }

    pub fn size(&self) -> usize {
        self.objects.len()
    }

    pub fn get_size(&self) -> usize {
        self.size()
    }

    pub fn frame_no(&self) -> i32 {
        self.frame_no
    }

    pub fn get_frame_no(&self) -> i32 {
        self.frame_no()
    }

    pub fn frameno(&self) -> i32 {
        self.frame_no()
    }

    pub fn start(&mut self) {
        self.elapsed = 0;
        self.play_state = AnimPlayState::Play;
    }

    pub fn run(&mut self) {
        self.start();
    }

    pub fn stop(&mut self) {
        self.elapsed = 0;
        self.play_state = AnimPlayState::Stop;
    }

    pub fn pause(&mut self) {
        self.play_state = AnimPlayState::Pause;
    }

    pub fn go_time(&mut self, value: TimeValue) {
        self.elapsed = value.clamp(0, self.length.max(value));
        self.play_state = AnimPlayState::Pause;
    }

    pub fn move_time(&mut self, value: TimeValue) {
        self.go_time(value);
    }

    pub fn r#move(&mut self, value: TimeValue) {
        self.go_time(value);
    }

    pub fn setup_render(&mut self, start: TimeValue, end: TimeValue, frame_rate: f64) -> i32 {
        let duration = (end - start).max(0) as f64;
        self.frame_no = 0;
        (duration * frame_rate / 1000.0).ceil() as i32
    }

    pub fn write_frame(&mut self) {
        self.frame_no += 1;
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }

    pub fn get_at(&self, index: usize) -> Option<&AnimObj> {
        self.objects.get(index)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&AnimObj> {
        self.objects.iter().find(|object| object.name() == name)
    }

    pub fn append(&mut self, object: AnimObj) {
        self.objects.push(object);
    }

    pub fn insert_before(&mut self, index: usize, object: AnimObj) {
        if index >= self.objects.len() {
            self.objects.push(object);
        } else {
            self.objects.insert(index, object);
        }
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        if index >= self.objects.len() {
            false
        } else {
            self.objects.remove(index);
            true
        }
    }

    pub fn resolve_rel_time(&mut self) {
        let mut current = 0;
        for object in &mut self.objects {
            let start = current + object.rel_start();
            object.set_abs_start(start);
            object.set_abs_end(start + (object.rel_end() - object.rel_start()).max(0));
            current = object.abs_end();
        }
    }
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_molstr::Atom;

    fn atom(serial: i32, x: f64) -> Atom {
        Atom {
            serial,
            name: "C".to_string(),
            alt_loc: None,
            res_name: "RES".to_string(),
            chain_id: "A".to_string(),
            res_id: 1,
            ins_code: None,
            position: Vector4::new3(x, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "C".to_string(),
            formal_charge: None,
            int_props: Default::default(),
        }
    }

    #[test]
    fn anim_obj_timing_and_quadric_match_reference() {
        let mut obj = AnimObj::new();
        assert!(!obj.disabled());
        assert_eq!(obj.rel_start(), 0);
        assert_eq!(obj.rel_end(), 0);
        assert_eq!(obj.quadric(), 0.0);
        obj.set_rel_start(100);
        assert_eq!(obj.rel_end(), 100);
        obj.set_rel_end(300);
        obj.set_abs_start(1000);
        obj.set_abs_end(2000);
        assert_eq!(obj.rho_at(500), 0.0);
        assert_eq!(obj.rho_at(2500), 1.0);
        assert_eq!(obj.rho_at(1500), 0.5);
        obj.set_quadric(0.25);
        assert_eq!(obj.quadric(), 0.25);
        assert!(obj.conv_rho(0.125) < 0.125);
    }

    #[test]
    fn simple_animation_defaults_match_qif() {
        let cam = CamMotion::new();
        assert_eq!(cam.endcam(), "");
        assert!(!cam.ignorecenter());
        assert!(!cam.ignorerotate());
        assert!(!cam.ignorezoom());
        assert!(!cam.ignoreslab());
        assert!(cam.keepquatpositive());

        let spin = SimpleSpin::new();
        assert_eq!(spin.angle(), 0.0);
        assert_eq!(spin.axis(), Vector4::new4(0.0, 1.0, 0.0, 0.0));

        let show = ShowHideAnim::new();
        assert_eq!(show.prop(), "visible");
        assert!(!show.hide());
        assert!(show.fade());
        assert_eq!(show.tgt_alpha(), 1.0);
        assert_eq!(show.alpha_at_rho(0.25), 0.25);

        let slide = SlideInOutAnim::new();
        assert_eq!(slide.prop(), "visible");
        assert!(!slide.hide());
        assert_eq!(slide.direction(), 0.0);
        assert_eq!(slide.distance(), 1.0);
        assert_eq!(
            slide.translation_at_rho(2.0, 0.0),
            Vector4::new3(2.0, 0.0, 0.0)
        );
        assert_eq!(slide.translation_at_rho(2.0, 1.0), Vector4::ZERO);
    }

    #[test]
    fn property_animations_interpolate_reference_values() {
        let mut real = RealPropAnim::new();
        assert_eq!(real.start_value(), 0.0);
        assert_eq!(real.end_value(), 1.0);
        real.set_start_value(2.0);
        real.set_end_value(6.0);
        real.set_prop("alpha");
        real.set_rend("r1");
        assert_eq!(real.value_at_rho(0.25), 3.0);
        assert_eq!(real.prop(), "alpha");
        assert_eq!(real.rend(), "r1");

        let mut mol = MolAnim::new();
        mol.set_start_value(10.0);
        mol.set_end_value(20.0);
        mol.set_prop("frame");
        mol.set_mol("mol1");
        assert_eq!(mol.value_at_rho(0.5), 15.0);
        assert_eq!(mol.mol(), "mol1");

        let mut xform = RendXformAnim::new();
        assert_eq!(xform.prop(), "xformMat");
        xform.set_prop("ignored");
        assert_eq!(xform.prop(), "xformMat");
        xform.set_start_tran(Vector4::new3(0.0, 0.0, 0.0));
        xform.set_end_tran(Vector4::new3(10.0, 0.0, 0.0));
        assert_eq!(xform.translation_at_rho(0.4), Vector4::new3(4.0, 0.0, 0.0));
    }

    #[test]
    fn rend_prop_anim_parses_target_renderer_names() {
        let mut anim = RendPropAnim::new();
        anim.set_prop_name("color");
        // Mixed scene-level (bare) and object-scoped (`obj/rend`) targets, with
        // surrounding whitespace to exercise trimming.
        anim.set_rend_names("r1, obj1/r2 , r3");
        assert_eq!(anim.prop_name(), "color");
        assert_eq!(
            anim.rend_name_list(),
            &[
                ("".to_string(), "r1".to_string()),
                ("obj1".to_string(), "r2".to_string()),
                ("".to_string(), "r3".to_string()),
            ]
        );
        // Normalized string re-joins non-empty entries (object-scoped keep `/`).
        assert_eq!(anim.rend_names(), "r1,obj1/r2,r3");
        // Inherited AnimObj timing curve still applies.
        anim.base_mut().set_abs_start(0);
        anim.base_mut().set_abs_end(100);
        assert_eq!(anim.base().rho_at(0), 0.0);
        assert_eq!(anim.base().rho_at(100), 1.0);
    }

    #[test]
    fn morph_mol_frames_and_interpolation_match_reference_flow() {
        let mut mol = MolCoord::new();
        mol.push_atom(atom(1, 0.0));
        mol.push_atom(atom(2, 10.0));

        let mut morph = MorphMol::new();
        *morph.molecule_mut() = mol;
        assert_eq!(morph.frame(), 0.0);
        assert!(!morph.scale_frame());
        assert!(morph.append_this_frame());
        assert!(!morph.append_this_frame());
        assert_eq!(morph.frame_size(), 1);
        assert!(morph.frame_info_json().contains("(this)"));

        morph.append_frame(MorphFrame::new(
            "end",
            "src",
            "molecule",
            vec![Vector4::new3(10.0, 0.0, 0.0), Vector4::new3(20.0, 0.0, 0.0)],
        ));
        let mid = morph.interpolated_positions(0.5);
        assert_eq!(mid[0], Vector4::new3(5.0, 0.0, 0.0));
        assert_eq!(mid[1], Vector4::new3(15.0, 0.0, 0.0));
        assert!(morph.remove_frame(1));
        assert!(!morph.remove_frame(10));
    }

    #[test]
    fn anim_manager_edit_and_state_methods_match_qif() {
        let mut manager = AnimMgr::new();
        assert_eq!(manager.startcam(), "");
        assert!(!manager.loop_enabled());
        assert_eq!(manager.play_state(), AnimPlayState::Stop);
        assert_eq!(manager.play_state_code(), AnimMgr::AM_STOP);
        manager.set_length(1000);
        manager.set_startcam("cam1");
        manager.set_loop_enabled(true);
        manager.start();
        assert_eq!(manager.play_state(), AnimPlayState::Play);
        manager.pause();
        assert_eq!(manager.play_state(), AnimPlayState::Pause);
        manager.go_time(500);
        assert_eq!(manager.elapsed(), 500);
        manager.stop();
        assert_eq!(manager.elapsed(), 0);

        let mut obj = AnimObj::new();
        obj.set_name("a1");
        obj.set_rel_end(100);
        manager.append(obj.clone());
        manager.insert_before(10, AnimObj::new());
        assert_eq!(manager.size(), 2);
        assert!(manager.get_by_name("a1").is_some());
        manager.resolve_rel_time();
        assert_eq!(manager.get_at(0).unwrap().abs_start(), 0);
        assert_eq!(manager.get_at(0).unwrap().abs_end(), 100);
        assert!(manager.remove_at(1));
        assert!(!manager.remove_at(10));
        assert_eq!(manager.setup_render(0, 1000, 30.0), 30);
        manager.write_frame();
        assert_eq!(manager.frame_no(), 1);
        manager.clear();
        assert_eq!(manager.size(), 0);
    }
}
