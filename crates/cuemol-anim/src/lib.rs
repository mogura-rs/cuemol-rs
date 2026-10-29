use cuemol_core::{CueMolError, LDom2Node, Matrix4, Quat, Result, Uid, Vector4, NANOS_PER_SECOND};
use cuemol_molstr::MolCoord;
use cuemol_qsys::{
    AnimObjEvent, Camera, Renderer, Scene, SceneExporter, ScrEventManager, ScrEventManagerHandle,
    View,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
mod runtime;

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

    pub fn clone_object(&self) -> Self {
        let mut cloned = self.clone();
        cloned.assign_new_uid();
        cloned
    }

    fn assign_new_uid(&mut self) {
        self.uid = Uid::new(NEXT_ANIM_UID.fetch_add(1, Ordering::Relaxed));
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

    pub fn start(&self) -> TimeValue {
        self.rel_start()
    }

    pub fn set_scr_rel_start(&mut self, value: TimeValue) {
        self.set_rel_start(value);
    }

    pub fn set_start(&mut self, value: TimeValue) {
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

    pub fn end(&self) -> TimeValue {
        self.rel_end()
    }

    pub fn set_scr_rel_end(&mut self, value: TimeValue) {
        self.set_rel_end(value);
    }

    pub fn set_end(&mut self, value: TimeValue) {
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

macro_rules! impl_anim_obj_clone {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl $ty {
                pub fn clone_object(&self) -> Self {
                    let mut cloned = self.clone();
                    cloned.base_mut().assign_new_uid();
                    cloned
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

impl_anim_obj_clone!(
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
pub enum AnimMotion {
    Noop(NoopAnimObj),
    Camera(CamMotion),
    RealProperty(RealPropAnim),
    Molecule(MolAnim),
    RendererTransform(RendXformAnim),
    ShowHide(ShowHideAnim),
    Spin(SimpleSpin),
    Slide(SlideInOutAnim),
    Unknown { node: LDom2Node, base: AnimObj },
}

impl AnimMotion {
    pub fn clone_object(&self) -> Self {
        let mut cloned = self.clone();
        cloned.base_mut().assign_new_uid();
        cloned
    }

    pub fn type_name(&self) -> &str {
        match self {
            Self::Noop(_) => "NoopAnimObj",
            Self::Camera(_) => "CamMotion",
            Self::RealProperty(_) => "RealPropAnim",
            Self::Molecule(_) => "MolAnim",
            Self::RendererTransform(_) => "RendXformAnim",
            Self::ShowHide(_) => "ShowHideAnim",
            Self::Spin(_) => "SimpleSpin",
            Self::Slide(_) => "SlideInOutAnim",
            Self::Unknown { node, .. } => node.type_name(),
        }
    }

    pub fn base(&self) -> &AnimObj {
        match self {
            Self::Noop(value) => value.base(),
            Self::Camera(value) => value.base(),
            Self::RealProperty(value) => value.base(),
            Self::Molecule(value) => value.base(),
            Self::RendererTransform(value) => value.base(),
            Self::ShowHide(value) => value.base(),
            Self::Spin(value) => value.base(),
            Self::Slide(value) => value.base(),
            Self::Unknown { base, .. } => base,
        }
    }

    pub fn base_mut(&mut self) -> &mut AnimObj {
        match self {
            Self::Noop(value) => value.base_mut(),
            Self::Camera(value) => value.base_mut(),
            Self::RealProperty(value) => value.base_mut(),
            Self::Molecule(value) => value.base_mut(),
            Self::RendererTransform(value) => value.base_mut(),
            Self::ShowHide(value) => value.base_mut(),
            Self::Spin(value) => value.base_mut(),
            Self::Slide(value) => value.base_mut(),
            Self::Unknown { base, .. } => base,
        }
    }

    pub fn from_ldom2_node(node: &LDom2Node) -> Result<Self> {
        if node.tag_name() != "motion" {
            return Err(CueMolError::parse(format!(
                "expected motion node, got {}",
                node.tag_name()
            )));
        }
        let type_name = node.type_name();
        let mut motion = match type_name {
            "NoopAnimObj" => Self::Noop(NoopAnimObj::new()),
            "CamMotion" => Self::Camera(CamMotion::new()),
            "RealPropAnim" => Self::RealProperty(RealPropAnim::new()),
            "MolAnim" => Self::Molecule(MolAnim::new()),
            "RendXformAnim" => Self::RendererTransform(RendXformAnim::new()),
            "ShowHideAnim" => Self::ShowHide(ShowHideAnim::new()),
            "SimpleSpin" => Self::Spin(SimpleSpin::new()),
            "SlideInOutAnim" => Self::Slide(SlideInOutAnim::new()),
            _ => Self::Unknown {
                node: node.clone(),
                base: AnimObj::new(),
            },
        };
        read_common_motion_attrs(node, motion.base_mut())?;
        match &mut motion {
            Self::Camera(value) => {
                value.set_endcam(node.str_attr("endcam").to_string());
                value.set_ignorecenter(read_bool_attr(node, "ignorecenter", false));
                value.set_ignorerotate(read_bool_attr(node, "ignorerotate", false));
                value.set_ignorezoom(read_bool_attr(node, "ignorezoom", false));
                value.set_ignoreslab(read_bool_attr(node, "ignoreslab", false));
                value.set_keepquatpositive(read_bool_attr(node, "keepquatpositive", true));
            }
            Self::RealProperty(value) => {
                value.set_start_value(read_f64_attr(node, "startValue", 0.0)?);
                value.set_end_value(read_f64_attr(node, "endValue", 1.0)?);
                value.set_prop(node.str_attr("prop").to_string());
                value.set_rend(node.str_attr("rend").to_string());
            }
            Self::Molecule(value) => {
                value.set_start_value(read_f64_attr(node, "startValue", 0.0)?);
                value.set_end_value(read_f64_attr(node, "endValue", 1.0)?);
                value.set_prop(node.str_attr("prop").to_string());
                value.set_mol(node.str_attr("mol").to_string());
            }
            Self::RendererTransform(value) => {
                value.set_prop(node.str_attr("prop").to_string());
                value.set_rend(node.str_attr("rend").to_string());
                value.set_start_tran(read_vector_attr(node, "start_tran", Vector4::ZERO)?);
                value.set_end_tran(read_vector_attr(node, "end_tran", Vector4::ZERO)?);
            }
            Self::ShowHide(value) => {
                value.set_hide(read_bool_attr(node, "hide", false));
                value.set_fade(read_bool_attr(node, "fade", true));
                value.set_rend(node.str_attr("rend").to_string());
                value.set_tgt_alpha(read_f64_attr(node, "tgt_alpha", 1.0)?);
            }
            Self::Spin(value) => {
                value.set_angle(read_f64_attr(node, "angle", 0.0)?);
                let axis = read_vector_attr(node, "axis", Vector4::new4(0.0, 1.0, 0.0, 0.0))?;
                if !value.set_axis(axis) {
                    return Err(CueMolError::parse("SimpleSpin axis is zero"));
                }
            }
            Self::Slide(value) => {
                value.set_rend(node.str_attr("rend").to_string());
                value.set_hide(read_bool_attr(node, "hide", false));
                value.set_direction(read_f64_attr(node, "direction", 0.0)?);
                value.set_distance(read_f64_attr(node, "distance", 1.0)?);
            }
            Self::Noop(_) | Self::Unknown { .. } => {}
        }
        Ok(motion)
    }

    pub fn to_ldom2_node(&self) -> LDom2Node {
        if let Self::Unknown { node, .. } = self {
            return node.clone();
        }
        let mut node = LDom2Node::new();
        node.set_tag_name("motion");
        node.set_type_name(self.type_name());
        write_common_motion_attrs(&mut node, self.base());
        match self {
            Self::Camera(value) => {
                node.set_str_attr("endcam", value.endcam());
                node.set_str_attr("ignorecenter", value.ignorecenter().to_string());
                node.set_str_attr("ignorerotate", value.ignorerotate().to_string());
                node.set_str_attr("ignorezoom", value.ignorezoom().to_string());
                node.set_str_attr("ignoreslab", value.ignoreslab().to_string());
                node.set_str_attr("keepquatpositive", value.keepquatpositive().to_string());
            }
            Self::RealProperty(value) => {
                node.set_str_attr("startValue", value.start_value().to_string());
                node.set_str_attr("endValue", value.end_value().to_string());
                node.set_str_attr("prop", value.prop());
                node.set_str_attr("rend", value.rend());
            }
            Self::Molecule(value) => {
                node.set_str_attr("startValue", value.start_value().to_string());
                node.set_str_attr("endValue", value.end_value().to_string());
                node.set_str_attr("prop", value.prop());
                node.set_str_attr("mol", value.mol());
            }
            Self::RendererTransform(value) => {
                node.set_str_attr("prop", value.prop());
                node.set_str_attr("rend", value.rend());
                node.set_str_attr("start_tran", value.start_tran().script_string());
                node.set_str_attr("end_tran", value.end_tran().script_string());
            }
            Self::ShowHide(value) => {
                node.set_str_attr("hide", value.hide().to_string());
                node.set_str_attr("fade", value.fade().to_string());
                node.set_str_attr("rend", value.rend());
                node.set_str_attr("tgt_alpha", value.tgt_alpha().to_string());
            }
            Self::Spin(value) => {
                node.set_str_attr("angle", value.angle().to_string());
                node.set_str_attr("axis", value.axis().script_string());
            }
            Self::Slide(value) => {
                node.set_str_attr("rend", value.rend());
                node.set_str_attr("hide", value.hide().to_string());
                node.set_str_attr("direction", value.direction().to_string());
                node.set_str_attr("distance", value.distance().to_string());
            }
            Self::Noop(_) | Self::Unknown { .. } => {}
        }
        node
    }
}

fn read_common_motion_attrs(node: &LDom2Node, base: &mut AnimObj) -> Result<()> {
    if let Ok(uid) = node.str_attr("uid").as_str().parse::<u64>() {
        if uid == 0 || uid == u64::MAX {
            return Err(CueMolError::parse("invalid animation uid"));
        }
        base.uid = Uid::new(uid);
        NEXT_ANIM_UID.fetch_max(uid + 1, Ordering::Relaxed);
    }
    base.set_name(node.str_attr("name").to_string());
    base.set_disabled(read_bool_attr(node, "disabled", false));
    base.set_time_ref_name(node.str_attr("timeRefName").to_string());
    base.set_rel_start(read_i64_attr(node, "start", 0)?);
    base.set_rel_end(read_i64_attr(node, "end", base.rel_start())?);
    base.set_quadric(read_f64_attr(node, "quadric", 0.0)?);
    Ok(())
}

fn write_common_motion_attrs(node: &mut LDom2Node, base: &AnimObj) {
    node.set_str_attr("uid", base.uid().get().to_string());
    node.set_str_attr("name", base.name());
    node.set_str_attr("disabled", base.disabled().to_string());
    node.set_str_attr("timeRefName", base.time_ref_name());
    node.set_str_attr("start", base.rel_start().to_string());
    node.set_str_attr("end", base.rel_end().to_string());
    node.set_str_attr("quadric", base.quadric().to_string());
}

fn read_bool_attr(node: &LDom2Node, name: &str, default: bool) -> bool {
    let value = node.str_attr(name).to_string();
    if value.is_empty() {
        default
    } else {
        value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes") || value == "1"
    }
}

fn read_i64_attr(node: &LDom2Node, name: &str, default: i64) -> Result<i64> {
    let value = node.str_attr(name).to_string();
    if value.is_empty() {
        return Ok(default);
    }
    value
        .parse()
        .map_err(|_| CueMolError::parse(format!("invalid {name} animation value")))
}

fn read_f64_attr(node: &LDom2Node, name: &str, default: f64) -> Result<f64> {
    let value = node.str_attr(name).to_string();
    if value.is_empty() {
        return Ok(default);
    }
    value
        .parse()
        .map_err(|_| CueMolError::parse(format!("invalid {name} animation value")))
}

fn read_vector_attr(node: &LDom2Node, name: &str, default: Vector4) -> Result<Vector4> {
    let value = node.str_attr(name).to_string();
    if value.is_empty() {
        Ok(default)
    } else {
        Vector4::from_script_string(&value)
    }
}

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

    pub fn source(&self) -> &str {
        &self.src
    }

    pub fn source_type(&self) -> &str {
        &self.srctype
    }

    pub fn is_this(&self) -> bool {
        self.srctype == "<this>"
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
        self.update_molecule_positions();
    }

    pub fn scale_frame(&self) -> bool {
        self.scale_frame
    }

    pub fn is_scale_frame(&self) -> bool {
        self.scale_frame()
    }

    pub fn set_scale_frame(&mut self, value: bool) {
        self.scale_frame = value;
        self.update_molecule_positions();
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
        let positions = self.positions_for_molecule(molecule);
        let frame = MorphFrame::new("", "", "molecule", positions);
        self.insert_frame_before(frame, index);
    }

    pub fn positions_for_molecule(&self, molecule: &MolCoord) -> Vec<Vector4> {
        let mut source = BTreeMap::new();
        for atom in molecule.atoms() {
            source
                .entry((atom.chain_id.as_str(), atom.res_id, atom.name.as_str()))
                .or_insert(atom.position);
        }
        self.molecule
            .atoms()
            .iter()
            .map(|atom| {
                source
                    .get(&(atom.chain_id.as_str(), atom.res_id, atom.name.as_str()))
                    .copied()
                    .unwrap_or(atom.position)
            })
            .collect()
    }

    pub fn insert_frame_before(&mut self, frame: MorphFrame, index: usize) {
        if index >= self.frames.len() {
            self.frames.push(frame);
        } else {
            self.frames.insert(index, frame);
        }
    }

    pub fn remove_frame(&mut self, index: usize) -> bool {
        if index >= self.frames.len() || self.frames[index].srctype == "<this>" {
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
            .push(MorphFrame::new("", "", "<this>", positions));
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

    fn update_molecule_positions(&mut self) {
        if self.frames.is_empty() {
            return;
        }
        let positions = self.interpolated_positions(self.frame);
        if positions.len() != self.molecule.atom_count() {
            return;
        }
        for (atom, position) in self.molecule.atoms_mut().iter_mut().zip(positions) {
            atom.position = position;
        }
        self.molecule.fire_atoms_moved();
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
    render_end_frame: i32,
    render_delta: f64,
    render_start_camera: Option<Camera>,
    saved_properties: runtime::SavedProperties,
    target_scene_uid: Option<Uid>,
    event_manager: Option<ScrEventManagerHandle>,
    motions: Vec<AnimMotion>,
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
            render_end_frame: -1,
            render_delta: 0.0,
            render_start_camera: None,
            saved_properties: runtime::SavedProperties::default(),
            target_scene_uid: None,
            event_manager: None,
            motions: Vec::new(),
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
        self.length = value;
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
        self.render_start_camera = None;
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
        self.motions.len()
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

    pub fn target_scene_uid(&self) -> Option<Uid> {
        self.target_scene_uid
    }

    pub fn set_target_scene_uid(&mut self, value: Uid) {
        self.target_scene_uid = Some(value);
    }

    pub fn attach_event_manager(&mut self, manager: ScrEventManagerHandle) {
        self.event_manager = Some(manager);
    }

    pub fn detach_event_manager(&mut self) {
        self.event_manager = None;
    }

    pub fn start(&mut self, view: &View) {
        self.target_scene_uid = Some(view.scene_uid());
        self.start_detached();
    }

    pub fn start_detached(&mut self) {
        match self.play_state {
            AnimPlayState::Stop => {
                self.elapsed = 0;
                self.play_state = AnimPlayState::Play;
            }
            AnimPlayState::Pause => {
                self.play_state = AnimPlayState::Play;
            }
            AnimPlayState::Play => {}
        }
    }

    pub fn run(&mut self) {
        self.start_detached();
    }

    pub fn stop(&mut self) {
        self.elapsed = 0;
        self.play_state = AnimPlayState::Stop;
    }

    pub fn stop_in_scene<T>(&mut self, scene: &mut Scene<T>) {
        self.saved_properties.restore(scene);
        self.render_start_camera = None;
        self.stop();
    }

    pub fn initial_camera(&self) -> Option<&Camera> {
        self.render_start_camera.as_ref()
    }

    pub fn pause(&mut self) {
        if self.play_state != AnimPlayState::Play {
            return;
        }
        let remaining = self.length.saturating_sub(self.elapsed).max(0);
        self.play_state = if remaining > 0 {
            AnimPlayState::Pause
        } else {
            AnimPlayState::Stop
        };
    }

    pub fn go_time(&mut self, value: TimeValue, view: &mut View) {
        self.target_scene_uid = Some(view.scene_uid());
        self.go_time_detached(value);
    }

    pub fn go_time_detached(&mut self, value: TimeValue) {
        self.elapsed = value;
        let remaining = self.length.saturating_sub(value).max(0);
        self.play_state = if remaining > 0 {
            AnimPlayState::Pause
        } else {
            AnimPlayState::Stop
        };
    }

    pub fn go_time_in_scene<T>(
        &mut self,
        value: TimeValue,
        scene: &mut Scene<T>,
        view_uid: Uid,
    ) -> Result<()> {
        if scene.view(view_uid).is_none() {
            return Err(CueMolError::NotFound(format!("view {}", view_uid.get())));
        }
        self.saved_properties.restore(scene);
        self.saved_properties.capture(scene, &self.motions);
        let start_camera = if self.startcam.is_empty() {
            scene.view(view_uid).map(View::camera).unwrap_or_default()
        } else {
            scene
                .get_camera(&self.startcam)
                .unwrap_or_else(|| scene.view(view_uid).map(View::camera).unwrap_or_default())
        };
        self.target_scene_uid = Some(scene.uid());
        self.go_time_detached(value);
        self.render_start_camera = Some(start_camera.clone());
        self.apply_time_in_scene(value, scene, view_uid, &start_camera)
    }

    fn apply_time_in_scene<T>(
        &self,
        elapsed: TimeValue,
        scene: &mut Scene<T>,
        view_uid: Uid,
        start_camera: &Camera,
    ) -> Result<()> {
        if scene.view(view_uid).is_none() {
            return Err(CueMolError::NotFound(format!("view {}", view_uid.get())));
        }
        let (work_camera, motion_starts) =
            motion_cameras(scene, &self.motions, start_camera, elapsed)?;
        let primary_property_motions = primary_property_motions(scene, &self.motions);
        initialize_primary_property_motions(
            scene,
            &self.motions,
            &primary_property_motions,
            start_camera.zoom(),
        );

        for (index, motion) in self.motions.iter().enumerate() {
            let base = motion.base();
            if base.disabled() {
                continue;
            }
            let phase = MotionPhase::at(base, elapsed);
            let rho = phase.rho(base, elapsed);
            let motion_camera = motion_starts[index].as_ref().unwrap_or(start_camera);
            match motion {
                AnimMotion::Camera(_) | AnimMotion::Spin(_) => {}
                AnimMotion::RealProperty(value) => {
                    if phase == MotionPhase::Before {
                        continue;
                    }
                    apply_to_renderer_targets(scene, value.rend(), |renderer| {
                        set_renderer_real(renderer, value.prop(), value.value_at_rho(rho));
                    });
                }
                AnimMotion::Molecule(value) => {
                    if phase == MotionPhase::Before {
                        continue;
                    }
                    if let Some(uid) = scene.object_by_name(value.mol()).map(|object| object.uid())
                    {
                        let current = value.value_at_rho(rho);
                        if let Some(object) = scene.object_mut(uid) {
                            if value.prop() == "visible" {
                                object.set_visible(current >= 0.5);
                            } else {
                                object.set_property_str(value.prop(), current.to_string());
                            }
                        }
                    }
                }
                AnimMotion::RendererTransform(value) => {
                    if phase == MotionPhase::Before {
                        continue;
                    }
                    let rotation = Matrix4::make_rot_mat(motion_camera.rotation().conj());
                    let translation = rotation.transform_point(Vector4::new4(
                        value.translation_at_rho(rho).x,
                        value.translation_at_rho(rho).y,
                        value.translation_at_rho(rho).z,
                        0.0,
                    ));
                    apply_to_renderer_targets(scene, value.rend(), |renderer| {
                        set_renderer_transform(renderer, Matrix4::make_trans_mat(translation));
                    });
                }
                AnimMotion::ShowHide(value) => {
                    if phase == MotionPhase::Before {
                        continue;
                    }
                    apply_to_renderer_targets(scene, value.rend(), |renderer| match phase {
                        MotionPhase::Before => unreachable!(),
                        MotionPhase::Active => {
                            set_renderer_visible(renderer, true);
                            if value.fade() {
                                set_renderer_alpha(renderer, value.alpha_at_rho(rho));
                            }
                        }
                        MotionPhase::After => {
                            set_renderer_visible(renderer, !value.hide());
                            if value.fade() {
                                set_renderer_alpha(renderer, value.tgt_alpha());
                            }
                        }
                    });
                }
                AnimMotion::Slide(value) => {
                    if phase == MotionPhase::Before {
                        continue;
                    }
                    let translation = value.translation_at_rho(motion_camera.zoom(), rho);
                    let rotation = Matrix4::make_rot_mat(motion_camera.rotation().conj());
                    let translation = rotation.transform_point(Vector4::new4(
                        translation.x,
                        translation.y,
                        translation.z,
                        0.0,
                    ));
                    apply_to_renderer_targets(scene, value.rend(), |renderer| match phase {
                        MotionPhase::Before => unreachable!(),
                        MotionPhase::Active => {
                            set_renderer_visible(renderer, true);
                            set_renderer_transform(renderer, Matrix4::make_trans_mat(translation));
                        }
                        MotionPhase::After => {
                            set_renderer_visible(renderer, !value.hide());
                            set_renderer_transform(
                                renderer,
                                if value.hide() {
                                    Matrix4::IDENTITY
                                } else {
                                    Matrix4::make_trans_mat(translation)
                                },
                            );
                        }
                    });
                }
                AnimMotion::Noop(_) | AnimMotion::Unknown { .. } => {}
            }
        }

        let view = scene
            .view_mut(view_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("view {}", view_uid.get())))?;
        view.set_camera(&work_camera);
        Ok(())
    }

    pub fn move_time(&mut self, value: TimeValue) {
        self.go_time_detached(value);
    }

    pub fn r#move(&mut self, value: TimeValue) {
        self.go_time_detached(value);
    }

    pub fn setup_render(&mut self, start: TimeValue, end: TimeValue, frame_rate: f64) -> i32 {
        let first = (start as f64 * frame_rate / NANOS_PER_SECOND as f64).ceil();
        let last = (end as f64 * frame_rate / NANOS_PER_SECOND as f64).floor();
        self.render_start_camera = None;
        if !frame_rate.is_finite()
            || frame_rate <= 0.0
            || end < start
            || first < i32::MIN as f64
            || last >= i32::MAX as f64
            || last - first + 1.0 > i32::MAX as f64
        {
            self.frame_no = 0;
            self.render_end_frame = -1;
            self.render_delta = 0.0;
            return 0;
        }
        self.frame_no = first as i32;
        self.render_end_frame = last as i32;
        self.render_delta = NANOS_PER_SECOND as f64 / frame_rate;
        self.play_state = AnimPlayState::Play;
        (self.render_end_frame - self.frame_no + 1).max(0)
    }

    pub fn write_frame(&mut self, writer: &mut SceneExporter) {
        if !self.begin_frame(writer) {
            return;
        }
        writer.write();
        self.end_frame(writer);
    }

    pub fn begin_frame(&mut self, writer: &mut SceneExporter) -> bool {
        if self.frame_no > self.render_end_frame {
            return false;
        }
        self.elapsed = (self.frame_no as f64 * self.render_delta).floor() as TimeValue;
        if let Some(scene_uid) = self.target_scene_uid {
            writer.attach(scene_uid);
        }
        if !self.startcam.is_empty() {
            writer.set_camera(self.startcam.clone());
        }
        true
    }

    pub fn begin_frame_in_scene<T>(
        &mut self,
        writer: &mut SceneExporter,
        scene: &mut Scene<T>,
        view_uid: Uid,
    ) -> Result<bool> {
        if self.frame_no > self.render_end_frame {
            return Ok(false);
        }
        let current_camera = scene
            .view(view_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("view {}", view_uid.get())))?
            .camera();
        if self.render_start_camera.is_none() {
            self.saved_properties.restore(scene);
            self.saved_properties.capture(scene, &self.motions);
        }
        let start_camera = self
            .render_start_camera
            .get_or_insert_with(|| scene.get_camera(&self.startcam).unwrap_or(current_camera))
            .clone();
        let elapsed = (self.frame_no as f64 * self.render_delta).floor() as TimeValue;
        self.apply_time_in_scene(elapsed, scene, view_uid, &start_camera)?;
        self.target_scene_uid = Some(scene.uid());
        self.elapsed = elapsed;
        writer.attach(scene.uid());
        writer.set_camera_object(scene.view(view_uid).map(View::camera));
        Ok(true)
    }

    pub fn end_frame(&mut self, writer: &mut SceneExporter) {
        if self.target_scene_uid.is_some() {
            writer.detach();
        }
        self.frame_no = self.frame_no.saturating_add(1);
    }

    pub fn clear(&mut self) {
        if self.play_state != AnimPlayState::Stop {
            self.stop();
        }
        self.motions.clear();
        self.elapsed = 0;
    }

    pub fn get_at(&self, index: usize) -> Option<&AnimObj> {
        self.motions.get(index).map(AnimMotion::base)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&AnimObj> {
        self.motions
            .iter()
            .map(AnimMotion::base)
            .find(|object| object.name() == name)
    }

    pub fn motions(&self) -> &[AnimMotion] {
        &self.motions
    }

    pub fn motions_mut(&mut self) -> &mut [AnimMotion] {
        &mut self.motions
    }

    pub fn motion_by_uid(&self, uid: Uid) -> Option<&AnimMotion> {
        self.motions
            .iter()
            .find(|motion| motion.base().uid() == uid)
    }

    pub fn motion_by_uid_mut(&mut self, uid: Uid) -> Option<&mut AnimMotion> {
        self.motions
            .iter_mut()
            .find(|motion| motion.base().uid() == uid)
    }

    /// Mutate one animation object through its manager, then rebuild the
    /// timeline and emit the C++-compatible `SEM_ANIM/SEM_PROPCHG` event.
    pub fn edit_motion_by_uid<R>(
        &mut self,
        uid: Uid,
        property_name: &str,
        parent_name: &str,
        operation: impl FnOnce(&mut AnimMotion) -> R,
    ) -> Option<R> {
        let index = self
            .motions
            .iter()
            .position(|motion| motion.base().uid() == uid)?;
        let result = operation(&mut self.motions[index]);
        self.update_timeline();
        self.fire_anim_prop_event(property_name, parent_name);
        Some(result)
    }

    pub fn append_motion(&mut self, motion: AnimMotion) {
        let index = self.motions.len();
        self.motions.push(motion);
        self.update_timeline();
        self.fire_anim_event("animObjAdded", ScrEventManager::SEM_ADDED, index as i32);
    }

    pub fn insert_motion_before(&mut self, index: usize, motion: AnimMotion) {
        let inserted_index = index.min(self.motions.len());
        if index >= self.motions.len() {
            self.motions.push(motion);
        } else {
            self.motions.insert(index, motion);
        }
        self.update_timeline();
        self.fire_anim_event(
            "animObjAdded",
            ScrEventManager::SEM_ADDED,
            inserted_index as i32,
        );
    }

    pub fn append(&mut self, object: AnimObj) {
        let index = self.motions.len();
        self.motions
            .push(AnimMotion::Noop(NoopAnimObj { base: object }));
        self.update_timeline();
        self.fire_anim_event("animObjAdded", ScrEventManager::SEM_ADDED, index as i32);
    }

    pub fn insert_before(&mut self, index: usize, object: AnimObj) {
        let motion = AnimMotion::Noop(NoopAnimObj { base: object });
        let inserted_index = index.min(self.motions.len());
        if index >= self.motions.len() {
            self.motions.push(motion);
        } else {
            self.motions.insert(index, motion);
        }
        self.update_timeline();
        self.fire_anim_event(
            "animObjAdded",
            ScrEventManager::SEM_ADDED,
            inserted_index as i32,
        );
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        if index >= self.motions.len() {
            false
        } else {
            self.fire_anim_event(
                "animObjRemoving",
                ScrEventManager::SEM_REMOVING,
                index as i32,
            );
            self.motions.remove(index);
            self.update_timeline();
            true
        }
    }

    fn fire_anim_event(&self, descr: &str, event_type: i32, index: i32) {
        let Some(manager) = &self.event_manager else {
            return;
        };
        let mut event = AnimObjEvent::new();
        event.set_descr(descr);
        event.set_event_type(event_type);
        event.set_source(
            self.target_scene_uid
                .map(Uid::get)
                .unwrap_or(cuemol_qsys::INVALID_UID),
        );
        event.set_index(index);
        let _ = manager.with(|manager| manager.fire_anim_obj_event(&event));
    }

    fn fire_anim_prop_event(&self, property_name: &str, parent_name: &str) {
        let Some(manager) = &self.event_manager else {
            return;
        };
        let mut event = AnimObjEvent::new();
        event.set_descr(property_name);
        event.set_event_type(ScrEventManager::SEM_PROPCHG);
        event.set_source(
            self.target_scene_uid
                .map(Uid::get)
                .unwrap_or(cuemol_qsys::INVALID_UID),
        );
        event.set_prop_event(property_name, parent_name);
        let _ = manager.with(|manager| manager.fire_anim_obj_event(&event));
    }

    fn update_timeline(&mut self) {
        if self.play_state != AnimPlayState::Stop {
            self.stop();
        }
        let _ = self.try_resolve_rel_time();
        self.length = self
            .motions
            .iter()
            .map(|motion| motion.base().abs_end())
            .max()
            .unwrap_or(0)
            .max(0);
    }

    /// Resolve relative motion times using each motion's `timeRefName`.
    ///
    /// A motion without a reference uses its relative values as absolute
    /// values. A referenced motion is offset from the referenced motion's end,
    /// matching C++ `AnimMgr::resolveTimeImpl`.
    pub fn try_resolve_rel_time(&mut self) -> Result<()> {
        let specs = self
            .motions
            .iter()
            .map(|motion| {
                let base = motion.base();
                MotionTimingSpec {
                    name: base.name().to_string(),
                    reference: base.time_ref_name().to_string(),
                    rel_start: base.rel_start(),
                    rel_end: base.rel_end(),
                }
            })
            .collect::<Vec<_>>();
        let mut states = vec![TimingResolveState::Unresolved; specs.len()];
        let mut resolved = vec![(0, 0); specs.len()];
        for index in 0..specs.len() {
            resolve_motion_timing(index, &specs, &mut states, &mut resolved)?;
        }
        for (motion, (start, end)) in self.motions.iter_mut().zip(resolved) {
            motion.base_mut().set_abs_start(start);
            motion.base_mut().set_abs_end(end);
        }
        Ok(())
    }

    /// Compatibility wrapper for the original void scripting method. Use
    /// [`Self::try_resolve_rel_time`] when resolution errors must be reported.
    pub fn resolve_rel_time(&mut self) {
        let _ = self.try_resolve_rel_time();
    }

    pub fn from_ldom2_node(node: &LDom2Node) -> Result<Self> {
        if node.tag_name() != "animation" {
            return Err(CueMolError::parse(format!(
                "expected animation node, got {}",
                node.tag_name()
            )));
        }
        let mut manager = Self::new();
        manager.set_length(read_i64_attr(node, "length", 0)?);
        manager.set_startcam(node.str_attr("startcam").to_string());
        manager.set_loop(read_bool_attr(node, "loop", false));
        for child in node
            .children()
            .iter()
            .filter(|child| !child.is_attr() && child.tag_name() == "motion")
        {
            // The C++ reader uses appendImpl here to avoid edit events and
            // timeline updates until all forward references are available.
            manager.motions.push(AnimMotion::from_ldom2_node(child)?);
        }
        manager.try_resolve_rel_time()?;
        if node.str_attr("length").is_empty() {
            manager.length = manager
                .motions
                .iter()
                .map(|motion| motion.base().abs_end())
                .max()
                .unwrap_or(0);
        }
        Ok(manager)
    }

    pub fn to_ldom2_node(&self) -> LDom2Node {
        let mut node = LDom2Node::new();
        node.set_tag_name("animation");
        node.set_str_attr("length", self.length.to_string());
        node.set_str_attr("startcam", &self.startcam);
        node.set_str_attr("loop", self.loop_enabled.to_string());
        for motion in &self.motions {
            node.append_child_node(motion.to_ldom2_node());
        }
        node
    }
}

#[derive(Debug)]
struct MotionTimingSpec {
    name: String,
    reference: String,
    rel_start: TimeValue,
    rel_end: TimeValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimingResolveState {
    Unresolved,
    Resolving,
    Resolved,
}

fn resolve_motion_timing(
    index: usize,
    specs: &[MotionTimingSpec],
    states: &mut [TimingResolveState],
    resolved: &mut [(TimeValue, TimeValue)],
) -> Result<()> {
    match states[index] {
        TimingResolveState::Resolved => return Ok(()),
        TimingResolveState::Resolving => {
            let name = if specs[index].name.is_empty() {
                format!("#{index}")
            } else {
                specs[index].name.clone()
            };
            return Err(CueMolError::invalid_argument(format!(
                "animation time reference cycle at <{name}>"
            )));
        }
        TimingResolveState::Unresolved => {}
    }

    states[index] = TimingResolveState::Resolving;
    let spec = &specs[index];
    let base = if spec.reference.is_empty() {
        0
    } else {
        let reference_index = specs
            .iter()
            .position(|candidate| candidate.name == spec.reference)
            .ok_or_else(|| {
                CueMolError::invalid_argument(format!(
                    "animation time reference <{}> was not found",
                    spec.reference
                ))
            })?;
        resolve_motion_timing(reference_index, specs, states, resolved)?;
        resolved[reference_index].1
    };
    let start = base.checked_add(spec.rel_start).ok_or_else(|| {
        CueMolError::invalid_argument("animation start time overflowed the supported range")
    })?;
    let end = base.checked_add(spec.rel_end).ok_or_else(|| {
        CueMolError::invalid_argument("animation end time overflowed the supported range")
    })?;
    resolved[index] = (start, end);
    states[index] = TimingResolveState::Resolved;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotionPhase {
    Before,
    Active,
    After,
}

impl MotionPhase {
    fn at(base: &AnimObj, elapsed: TimeValue) -> Self {
        if elapsed < base.abs_start() {
            Self::Before
        } else if base.abs_end() < elapsed {
            Self::After
        } else {
            Self::Active
        }
    }

    fn rho(self, base: &AnimObj, elapsed: TimeValue) -> f64 {
        match self {
            Self::Before => 0.0,
            Self::Active => base.rho_at(elapsed),
            Self::After => 1.0,
        }
    }
}

type PrimaryPropertyMotions = BTreeMap<(Uid, String), (usize, Option<Uid>)>;

fn primary_property_motions<T>(scene: &Scene<T>, motions: &[AnimMotion]) -> PrimaryPropertyMotions {
    let mut result = BTreeMap::new();
    for (motion_index, motion) in motions.iter().enumerate() {
        if motion.base().disabled() {
            continue;
        }
        let (targets, property) = match motion {
            AnimMotion::RealProperty(value) => (
                renderer_target_ids(scene, value.rend())
                    .into_iter()
                    .map(|(object_uid, renderer_uid)| (renderer_uid, Some(object_uid)))
                    .collect::<Vec<_>>(),
                value.prop(),
            ),
            AnimMotion::Molecule(value) => (
                scene
                    .object_by_name(value.mol())
                    .map(|object| vec![(object.uid(), None)])
                    .unwrap_or_default(),
                value.prop(),
            ),
            AnimMotion::RendererTransform(value) => (
                renderer_target_ids(scene, value.rend())
                    .into_iter()
                    .map(|(object_uid, renderer_uid)| (renderer_uid, Some(object_uid)))
                    .collect::<Vec<_>>(),
                "xformMat",
            ),
            AnimMotion::ShowHide(value) => (
                renderer_target_ids(scene, value.rend())
                    .into_iter()
                    .map(|(object_uid, renderer_uid)| (renderer_uid, Some(object_uid)))
                    .collect::<Vec<_>>(),
                "visible",
            ),
            AnimMotion::Slide(value) => (
                renderer_target_ids(scene, value.rend())
                    .into_iter()
                    .map(|(object_uid, renderer_uid)| (renderer_uid, Some(object_uid)))
                    .collect::<Vec<_>>(),
                "visible",
            ),
            AnimMotion::Noop(_)
            | AnimMotion::Camera(_)
            | AnimMotion::Spin(_)
            | AnimMotion::Unknown { .. } => continue,
        };
        for (target_uid, owner_uid) in targets {
            let key = (target_uid, property.to_string());
            match result.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((motion_index, owner_uid));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if motion.base().abs_start() < motions[entry.get().0].base().abs_start() {
                        entry.insert((motion_index, owner_uid));
                    }
                }
            }
        }
    }
    result
}

fn initialize_primary_property_motions<T>(
    scene: &mut Scene<T>,
    motions: &[AnimMotion],
    primary_motions: &PrimaryPropertyMotions,
    zoom: f64,
) {
    for ((target_uid, _), (motion_index, owner_uid)) in primary_motions {
        let motion = &motions[*motion_index];
        match motion {
            AnimMotion::RealProperty(value) => {
                if let Some(renderer) = owner_uid.and_then(|object_uid| {
                    scene
                        .object_mut(object_uid)
                        .and_then(|object| object.renderer_mut(*target_uid))
                }) {
                    set_renderer_real(renderer, value.prop(), value.start_value());
                }
            }
            AnimMotion::Molecule(value) => {
                if let Some(object) = scene.object_mut(*target_uid) {
                    if value.prop() == "visible" {
                        object.set_visible(value.start_value() >= 0.5);
                    } else {
                        object.set_property_str(value.prop(), value.start_value().to_string());
                    }
                }
            }
            AnimMotion::RendererTransform(value) => {
                if let Some(renderer) = owner_uid.and_then(|object_uid| {
                    scene
                        .object_mut(object_uid)
                        .and_then(|object| object.renderer_mut(*target_uid))
                }) {
                    set_renderer_transform(renderer, Matrix4::make_trans_mat(value.start_tran()));
                }
            }
            AnimMotion::ShowHide(value) => {
                if let Some(renderer) = owner_uid.and_then(|object_uid| {
                    scene
                        .object_mut(object_uid)
                        .and_then(|object| object.renderer_mut(*target_uid))
                }) {
                    set_renderer_visible(renderer, value.hide());
                    if value.fade() {
                        set_renderer_alpha(
                            renderer,
                            if value.hide() { value.tgt_alpha() } else { 0.0 },
                        );
                    }
                }
            }
            AnimMotion::Slide(value) => {
                if let Some(renderer) = owner_uid.and_then(|object_uid| {
                    scene
                        .object_mut(object_uid)
                        .and_then(|object| object.renderer_mut(*target_uid))
                }) {
                    let (start, _) = value.calc_slide_positions(zoom);
                    set_renderer_transform(renderer, Matrix4::make_trans_mat(start));
                    set_renderer_visible(renderer, value.hide());
                }
            }
            AnimMotion::Noop(_)
            | AnimMotion::Camera(_)
            | AnimMotion::Spin(_)
            | AnimMotion::Unknown { .. } => {}
        }
    }
}

fn motion_cameras<T>(
    scene: &Scene<T>,
    motions: &[AnimMotion],
    initial: &Camera,
    elapsed: TimeValue,
) -> Result<(Camera, Vec<Option<Camera>>)> {
    let mut starts = vec![None; motions.len()];
    let times = motions
        .iter()
        .filter(|motion| !motion.base().disabled() && motion.base().abs_start() <= elapsed)
        .map(|motion| motion.base().abs_start())
        .chain(std::iter::once(elapsed))
        .collect::<std::collections::BTreeSet<_>>();
    let mut camera = initial.clone();
    for time in times {
        camera = initial.clone();
        for (index, motion) in motions.iter().enumerate() {
            let base = motion.base();
            let phase = MotionPhase::at(base, time);
            if base.disabled() || phase == MotionPhase::Before {
                continue;
            }
            let start = starts[index].get_or_insert_with(|| camera.clone());
            let rho = phase.rho(base, time);
            match motion {
                AnimMotion::Camera(motion) => {
                    if let Some(destination) = scene.get_camera(motion.endcam()) {
                        apply_camera_motion(&mut camera, start, &destination, motion, rho);
                    }
                }
                AnimMotion::Spin(motion) => {
                    let rotation = Quat::from_axis_angle(
                        motion.axis(),
                        motion.angle_at_rho(rho).to_radians(),
                    )?;
                    camera.set_rotation(camera.rotation().mul_quat(rotation));
                }
                _ => {}
            }
        }
    }
    Ok((camera, starts))
}

fn apply_camera_motion(
    camera: &mut Camera,
    start: &Camera,
    destination: &Camera,
    motion: &CamMotion,
    rho: f64,
) {
    if !motion.ignorerotate() {
        if let Ok(rotation) = slerp_quat(
            start.rotation(),
            destination.rotation(),
            rho,
            motion.keepquatpositive(),
        ) {
            camera.set_rotation(rotation);
        }
    }
    if !motion.ignorecenter() {
        camera.set_center(start.center() * (1.0 - rho) + destination.center() * rho);
    }
    if !motion.ignorezoom() {
        camera.set_zoom(start.zoom() * (1.0 - rho) + destination.zoom() * rho);
    }
    if !motion.ignoreslab() {
        camera.set_slab_depth(start.slab_depth() * (1.0 - rho) + destination.slab_depth() * rho);
    }
}

fn slerp_quat(lhs: Quat, rhs: Quat, rho: f64, keep_positive: bool) -> Result<Quat> {
    if keep_positive {
        return Quat::slerp(lhs, rhs, rho);
    }
    let lhs = lhs.normalize()?;
    let rhs = rhs.normalize()?;
    let dot = (lhs.x * rhs.x + lhs.y * rhs.y + lhs.z * rhs.z + lhs.w * rhs.w).clamp(-1.0, 1.0);
    let sine_squared = 1.0 - dot * dot;
    if sine_squared.abs() < 1.0e-8 {
        return Ok(rhs);
    }
    let sine = sine_squared.sqrt();
    let angle = dot.acos();
    let lhs_scale = ((1.0 - rho) * angle).sin() / sine;
    let rhs_scale = (rho * angle).sin() / sine;
    Ok(lhs.scale(lhs_scale).add_quat(rhs.scale(rhs_scale)))
}

fn renderer_target_ids<T>(scene: &Scene<T>, targets: &str) -> Vec<(Uid, Uid)> {
    let mut result = Vec::new();
    for target in targets.split(',').map(str::trim) {
        if target.is_empty() {
            continue;
        }
        if let Some((object_name, renderer_name)) = target.split_once('/') {
            let Some(object) = scene.object_by_name(object_name.trim()) else {
                continue;
            };
            if let Some(renderer) = object.renderer_by_name(renderer_name.trim()) {
                result.push((object.uid(), renderer.uid()));
            }
            continue;
        }
        for object_uid in scene.object_uids() {
            let Some(object) = scene.object(object_uid) else {
                continue;
            };
            if let Some(renderer) = object.renderer_by_name(target) {
                result.push((object_uid, renderer.uid()));
                break;
            }
        }
    }
    result
}

fn apply_to_renderer_targets<T>(
    scene: &mut Scene<T>,
    targets: &str,
    mut operation: impl FnMut(&mut Renderer),
) {
    for (object_uid, renderer_uid) in renderer_target_ids(scene, targets) {
        if let Some(renderer) = scene
            .object_mut(object_uid)
            .and_then(|object| object.renderer_mut(renderer_uid))
        {
            operation(renderer);
        }
    }
}

fn set_renderer_visible(renderer: &mut Renderer, value: bool) {
    renderer.set_visible(value);
    renderer.set_property_str("visible", value.to_string());
}

fn set_renderer_alpha(renderer: &mut Renderer, value: f64) {
    renderer.set_alpha(value);
    renderer.set_property_str("alpha", value.to_string());
}

fn set_renderer_transform(renderer: &mut Renderer, value: Matrix4) {
    renderer.set_xform_matrix(value);
    renderer.set_property_str("xformMat", value.to_script_string());
}

fn set_renderer_real(renderer: &mut Renderer, property: &str, value: f64) {
    match property {
        "alpha" | "default_alpha" => set_renderer_alpha(renderer, value),
        "visible" => set_renderer_visible(renderer, value >= 0.5),
        _ => renderer.set_property_str(property, value.to_string()),
    }
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::{
        parse_ldom2_xml, write_ldom2_xml, LDom2Tree, LScrCallBack, LScrObject, LVarArgs, LVariant,
    };
    use cuemol_molstr::Atom;
    use std::cell::RefCell;
    use std::rc::Rc;

    type RecordedAnimCall = (String, i32, i32, i32, String);

    struct RecordingAnimCallback {
        calls: Rc<RefCell<Vec<RecordedAnimCall>>>,
    }

    impl LScrObject for RecordingAnimCallback {}

    impl LScrCallBack for RecordingAnimCallback {
        fn invoke(&self, args: &mut LVarArgs) -> Result<bool> {
            self.calls.borrow_mut().push((
                args.get(1).get_string_value()?.as_str().to_string(),
                args.get(2).get_int_value()?,
                args.get(3).get_int_value()?,
                args.get(4).get_int_value()?,
                args.get(5).get_string_value()?.as_str().to_string(),
            ));
            *args.retval_mut() = LVariant::Boolean(false);
            Ok(true)
        }
    }

    fn atom(serial: i32, x: f64) -> Atom {
        Atom {
            serial,
            name: format!("C{serial}"),
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
            real_props: Default::default(),
        }
    }

    #[test]
    fn anim_obj_timing_and_quadric_match_reference() {
        let mut obj = AnimObj::new();
        assert!(!obj.disabled());
        assert_eq!(obj.rel_start(), 0);
        assert_eq!(obj.rel_end(), 0);
        obj.set_start(10);
        obj.set_end(20);
        assert_eq!(obj.start(), 10);
        assert_eq!(obj.end(), 20);
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
    fn cloned_animation_gets_a_distinct_uid() {
        let mut original = RealPropAnim::new();
        original.set_name("width");
        original.set_start_value(2.0);
        original.set_end_value(4.0);
        original.set_rel_end(100);

        let snapshot = original.clone();
        let cloned = original.clone_object();

        assert_eq!(snapshot.uid(), original.uid());
        assert_ne!(cloned.uid(), original.uid());
        assert_eq!(cloned.name(), original.name());
        assert_eq!(cloned.start_value(), original.start_value());
        assert_eq!(cloned.end_value(), original.end_value());
        assert_eq!(cloned.rel_end(), original.rel_end());

        let motion = AnimMotion::RealProperty(original);
        let cloned_motion = motion.clone_object();
        assert_ne!(cloned_motion.base().uid(), motion.base().uid());
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
    fn animation_ldom_roundtrip_preserves_typed_and_unknown_motions() {
        let tree = parse_ldom2_xml(
            r#"<animation length="2500" startcam="start" loop="true">
                 <motion type="SimpleSpin" name="spin" start="100" end="2100"
                         quadric="0.2" angle="180" axis="(0,0,1,0)"/>
                 <motion type="FutureMotion" name="future" start="0" end="50"
                         payload="a&amp;b"/>
               </animation>"#,
        )
        .unwrap();
        let manager = AnimMgr::from_ldom2_node(tree.top()).unwrap();
        assert_eq!(manager.length(), 2500);
        assert_eq!(manager.startcam(), "start");
        assert!(manager.loop_enabled());
        assert_eq!(manager.motions().len(), 2);
        let AnimMotion::Spin(spin) = &manager.motions()[0] else {
            panic!("expected SimpleSpin")
        };
        assert_eq!(spin.name(), "spin");
        assert_eq!(spin.angle(), 180.0);
        assert_eq!(spin.axis(), Vector4::new4(0.0, 0.0, 1.0, 0.0));
        assert!(matches!(
            &manager.motions()[1],
            AnimMotion::Unknown { node, .. }
                if node.type_name() == "FutureMotion"
                    && node.str_attr("payload").as_str() == "a&b"
        ));

        let output = write_ldom2_xml(&LDom2Tree::from(manager.to_ldom2_node()));
        assert!(output.contains("type=\"SimpleSpin\""));
        assert!(output.contains("type=\"FutureMotion\""));
        assert!(output.contains("payload=\"a&amp;b\""));
        let reparsed = parse_ldom2_xml(&output).unwrap();
        assert_eq!(
            AnimMgr::from_ldom2_node(reparsed.top())
                .unwrap()
                .motions()
                .len(),
            2
        );
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
        let mut partial = MolCoord::new();
        partial.push_atom(atom(2, 30.0));
        assert_eq!(
            morph.positions_for_molecule(&partial),
            vec![Vector4::new3(0.0, 0.0, 0.0), Vector4::new3(30.0, 0.0, 0.0)]
        );
        morph.set_frame(0.5);
        assert_eq!(
            morph.molecule().atoms()[0].position,
            Vector4::new3(5.0, 0.0, 0.0)
        );
        assert_eq!(
            morph.molecule().atoms()[1].position,
            Vector4::new3(15.0, 0.0, 0.0)
        );
        assert_eq!(morph.molecule().atoms_moved_generation(), 1);
        assert!(morph.remove_frame(1));
        assert!(!morph.remove_frame(10));
    }

    #[test]
    fn anim_manager_applies_timeline_to_scene_view_objects_and_renderers() {
        let mut scene = Scene::new(Uid::new(1), "scene");
        let view_uid = scene.create_view();
        let object_uid = scene.add_object("mol", ());
        let renderer_uid = scene
            .object_mut(object_uid)
            .unwrap()
            .create_renderer("simple");
        scene
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap()
            .set_name("main");

        let mut destination = Camera::new();
        destination.set_zoom(70.0);
        destination.set_center(Vector4::new3(10.0, 0.0, 0.0));
        scene.set_camera("end", destination);

        let set_timing = |base: &mut AnimObj| {
            base.set_rel_start(0);
            base.set_rel_end(100);
        };
        let mut camera = CamMotion::new();
        camera.set_endcam("end");
        set_timing(camera.base_mut());

        let mut real = RealPropAnim::new();
        real.set_rend("main");
        real.set_prop("width");
        real.set_start_value(1.0);
        real.set_end_value(3.0);
        set_timing(real.base_mut());

        let mut transform = RendXformAnim::new();
        transform.set_rend("main");
        transform.set_start_tran(Vector4::ZERO);
        transform.set_end_tran(Vector4::new3(10.0, 0.0, 0.0));
        set_timing(transform.base_mut());

        let mut show_hide = ShowHideAnim::new();
        show_hide.set_rend("main");
        show_hide.set_hide(true);
        show_hide.set_tgt_alpha(0.8);
        set_timing(show_hide.base_mut());

        let mut molecule = MolAnim::new();
        molecule.set_mol("mol");
        molecule.set_prop("frame");
        set_timing(molecule.base_mut());

        let mut manager = AnimMgr::new();
        manager.set_length(100);
        manager.append_motion(AnimMotion::Camera(camera));
        manager.append_motion(AnimMotion::RealProperty(real));
        manager.append_motion(AnimMotion::RendererTransform(transform));
        manager.append_motion(AnimMotion::ShowHide(show_hide));
        manager.append_motion(AnimMotion::Molecule(molecule));
        manager.go_time_in_scene(50, &mut scene, view_uid).unwrap();

        let view = scene.view(view_uid).unwrap();
        assert_eq!(view.zoom(), 60.0);
        assert_eq!(view.center(), Vector4::new3(5.0, 0.0, 0.0));
        let object = scene.object(object_uid).unwrap();
        assert_eq!(object.property_str("frame"), Some("0.5"));
        let renderer = object.renderer(renderer_uid).unwrap();
        assert_eq!(renderer.property_str("width"), Some("2"));
        assert_eq!(
            renderer.xform_matrix().get_trans_part(),
            Vector4::new3(5.0, 0.0, 0.0)
        );
        assert!(renderer.visible());
        assert!((renderer.alpha() - 0.4).abs() < 1.0e-12);

        manager.go_time_in_scene(150, &mut scene, view_uid).unwrap();
        let renderer = scene
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert!(!renderer.visible());
        assert!((renderer.alpha() - 0.8).abs() < 1.0e-12);
    }

    #[test]
    fn future_property_animation_does_not_override_the_active_animation() {
        let mut scene = Scene::new(Uid::new(1), "scene");
        let view_uid = scene.create_view();
        let object_uid = scene.add_object("mol", ());
        let renderer_uid = scene
            .object_mut(object_uid)
            .unwrap()
            .create_renderer("simple");
        scene
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap()
            .set_name("main");

        let mut first = RealPropAnim::new();
        first.set_rend("main");
        first.set_prop("width");
        first.set_start_value(1.0);
        first.set_end_value(3.0);
        first.set_rel_end(100);

        let mut second = RealPropAnim::new();
        second.set_rend("main");
        second.set_prop("width");
        second.set_start_value(3.0);
        second.set_end_value(5.0);
        second.set_rel_start(100);
        second.set_rel_end(200);

        let mut alpha = RealPropAnim::new();
        alpha.set_rend("main");
        alpha.set_prop("alpha");
        alpha.set_start_value(0.2);
        alpha.set_end_value(0.6);
        alpha.set_rel_end(100);

        let mut future_show = ShowHideAnim::new();
        future_show.set_rend("main");
        future_show.set_rel_start(100);
        future_show.set_rel_end(200);

        let mut manager = AnimMgr::new();
        manager.append_motion(AnimMotion::RealProperty(first));
        manager.append_motion(AnimMotion::RealProperty(second));
        manager.append_motion(AnimMotion::RealProperty(alpha));
        manager.append_motion(AnimMotion::ShowHide(future_show));
        manager.go_time_in_scene(50, &mut scene, view_uid).unwrap();

        let renderer = scene
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert_eq!(renderer.property_str("width"), Some("2"));
        assert!((renderer.alpha() - 0.4).abs() < 1.0e-12);
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
        let mut view = View::new(Uid::new(20), Uid::new(10));
        manager.start(&view);
        assert_eq!(manager.play_state(), AnimPlayState::Play);
        manager.pause();
        assert_eq!(manager.play_state(), AnimPlayState::Pause);
        manager.go_time(500, &mut view);
        assert_eq!(manager.elapsed(), 500);
        manager.stop();
        assert_eq!(manager.elapsed(), 0);

        let mut obj = AnimObj::new();
        obj.set_name("a1");
        obj.set_rel_end(100);
        manager.append(obj.clone());
        assert_eq!(manager.length(), 100);
        manager.start(&view);
        manager.insert_before(10, AnimObj::new());
        assert_eq!(manager.play_state(), AnimPlayState::Stop);
        assert_eq!(manager.size(), 2);
        assert!(manager.get_by_name("a1").is_some());
        manager.resolve_rel_time();
        assert_eq!(manager.get_at(0).unwrap().abs_start(), 0);
        assert_eq!(manager.get_at(0).unwrap().abs_end(), 100);
        assert!(manager.remove_at(1));
        assert!(!manager.remove_at(10));
        assert_eq!(manager.setup_render(0, NANOS_PER_SECOND, 30.0), 31);
        let mut writer = SceneExporter::default();
        manager.write_frame(&mut writer);
        assert_eq!(manager.frame_no(), 1);
        assert_eq!(manager.elapsed(), 0);
        assert_eq!(writer.write_count(), 1);
        assert_eq!(writer.camera(), "cam1");
        assert_eq!(writer.client_scene(), None);
        assert_eq!(manager.setup_render(0, NANOS_PER_SECOND, 30.0), 31);
        assert!(manager.begin_frame(&mut writer));
        assert_eq!(manager.frame_no(), 0);
        assert_eq!(writer.client_scene(), Some(Uid::new(10)));
        manager.end_frame(&mut writer);
        assert_eq!(manager.frame_no(), 1);
        assert_eq!(writer.client_scene(), None);
        manager.clear();
        assert_eq!(manager.size(), 0);
    }

    #[test]
    fn anim_manager_playback_state_transitions_match_reference() {
        let mut manager = AnimMgr::new();
        let view = View::new(Uid::new(20), Uid::new(10));

        manager.pause();
        assert_eq!(manager.play_state(), AnimPlayState::Stop);
        manager.set_length(100);
        manager.start(&view);
        manager.go_time_detached(40);
        assert_eq!(manager.play_state(), AnimPlayState::Pause);
        manager.start(&view);
        assert_eq!(manager.play_state(), AnimPlayState::Play);
        assert_eq!(manager.elapsed(), 40);
        manager.pause();
        assert_eq!(manager.play_state(), AnimPlayState::Pause);

        manager.go_time_detached(100);
        assert_eq!(manager.play_state(), AnimPlayState::Stop);
        assert_eq!(manager.elapsed(), 100);
        manager.go_time_detached(-5);
        assert_eq!(manager.play_state(), AnimPlayState::Pause);
        assert_eq!(manager.elapsed(), -5);
        manager.clear();
        assert_eq!(manager.play_state(), AnimPlayState::Stop);
        assert_eq!(manager.elapsed(), 0);

        manager.set_length(-10);
        assert_eq!(manager.length(), -10);
    }

    #[test]
    fn anim_manager_fires_script_events_for_list_edits() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let callback = Rc::new(RecordingAnimCallback {
            calls: Rc::clone(&calls),
        });
        let events = ScrEventManagerHandle::new();
        events.with_mut(|manager| {
            manager.add_listener(callback);
            manager.append("", ScrEventManager::SEM_ANIM, -1, 77);
        });

        let mut manager = AnimMgr::new();
        manager.set_target_scene_uid(Uid::new(77));
        manager.attach_event_manager(events);
        manager.append(AnimObj::new());
        manager.insert_before(0, AnimObj::new());
        assert!(manager.remove_at(1));
        assert!(!manager.remove_at(9));
        let first_uid = manager.get_at(0).unwrap().uid();
        manager
            .edit_motion_by_uid(first_uid, "end", "timing", |motion| {
                motion.base_mut().set_rel_end(250)
            })
            .unwrap();
        assert_eq!(manager.length(), 250);

        let calls = calls.borrow();
        assert_eq!(calls.len(), 4);
        assert_eq!(
            &calls[0],
            &(
                "animObjAdded".to_string(),
                ScrEventManager::SEM_ANIM,
                ScrEventManager::SEM_ADDED,
                77,
                "{\"index\":0}".to_string(),
            )
        );
        assert_eq!(calls[1].0, "animObjAdded");
        assert_eq!(calls[1].4, "{\"index\":0}");
        assert_eq!(calls[2].0, "animObjRemoving");
        assert_eq!(calls[2].2, ScrEventManager::SEM_REMOVING);
        assert_eq!(calls[2].4, "{\"index\":1}");
        assert_eq!(calls[3].0, "end");
        assert_eq!(calls[3].2, ScrEventManager::SEM_PROPCHG);
        assert!(calls[3].4.contains("\"propname\": \"end\""));
        assert!(calls[3].4.contains("\"parentname\": \"timing\""));
    }

    #[test]
    fn anim_manager_resolves_named_time_references_and_forward_references() {
        let mut child = AnimObj::new();
        child.set_name("child");
        child.set_time_ref_name("base");
        child.set_rel_start(10);
        child.set_rel_end(30);

        let mut independent = AnimObj::new();
        independent.set_name("independent");
        independent.set_rel_start(50);
        independent.set_rel_end(80);

        let mut base = AnimObj::new();
        base.set_name("base");
        base.set_rel_start(100);
        base.set_rel_end(200);

        let mut grandchild = AnimObj::new();
        grandchild.set_name("grandchild");
        grandchild.set_time_ref_name("child");
        grandchild.set_rel_start(5);
        grandchild.set_rel_end(15);

        let mut manager = AnimMgr::new();
        // The dependent objects intentionally precede their references.
        manager.append(child);
        manager.append(independent);
        manager.append(grandchild);
        manager.append(base);
        manager.try_resolve_rel_time().unwrap();

        let child = manager.get_by_name("child").unwrap();
        assert_eq!((child.abs_start(), child.abs_end()), (210, 230));
        let independent = manager.get_by_name("independent").unwrap();
        assert_eq!((independent.abs_start(), independent.abs_end()), (50, 80));
        let base = manager.get_by_name("base").unwrap();
        assert_eq!((base.abs_start(), base.abs_end()), (100, 200));
        let grandchild = manager.get_by_name("grandchild").unwrap();
        assert_eq!((grandchild.abs_start(), grandchild.abs_end()), (235, 245));
    }

    #[test]
    fn anim_manager_rejects_missing_and_cyclic_time_references() {
        let mut missing = AnimObj::new();
        missing.set_name("missing-user");
        missing.set_time_ref_name("absent");
        let mut manager = AnimMgr::new();
        manager.append(missing);
        let error = manager.try_resolve_rel_time().unwrap_err().to_string();
        assert!(error.contains("<absent> was not found"));

        let mut lhs = AnimObj::new();
        lhs.set_name("lhs");
        lhs.set_time_ref_name("rhs");
        let mut rhs = AnimObj::new();
        rhs.set_name("rhs");
        rhs.set_time_ref_name("lhs");
        let mut manager = AnimMgr::new();
        manager.append(lhs);
        manager.append(rhs);
        let error = manager.try_resolve_rel_time().unwrap_err().to_string();
        assert!(error.contains("time reference cycle"));

        let tree = parse_ldom2_xml(
            r#"<animation>
                 <motion type="NoopAnimObj" name="a" timeRefName="b"/>
                 <motion type="NoopAnimObj" name="b" timeRefName="a"/>
               </animation>"#,
        )
        .unwrap();
        assert!(AnimMgr::from_ldom2_node(tree.top()).is_err());
    }
}
