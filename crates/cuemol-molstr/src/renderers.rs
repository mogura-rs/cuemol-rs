use crate::selection::{parse_selector, Selector};
use crate::{Atom, MolCoord};
use cuemol_core::{LDom2Node, Result, Vector4};
use cuemol_gfx::SolidColor;
use cuemol_qsys::DrawObj;
use std::collections::VecDeque;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SolidColoring;

impl SolidColoring {
    pub fn new() -> Self {
        Self
    }

    pub fn type_name(&self) -> &'static str {
        "SolidColoring"
    }

    pub fn color_for_atom(&self, _atom: &Atom) -> Option<SolidColor> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpkColoring {
    col_c: SolidColor,
    col_n: SolidColor,
    col_o: SolidColor,
    col_h: SolidColor,
    col_s: SolidColor,
    col_p: SolidColor,
    col_x: SolidColor,
}

impl Default for CpkColoring {
    fn default() -> Self {
        Self {
            col_h: SolidColor::from_rgb(0.0, 1.0, 1.0),
            col_c: SolidColor::from_rgb(1.0, 1.0, 0.75),
            col_n: SolidColor::from_rgb(0.0, 0.0, 1.0),
            col_o: SolidColor::from_rgb(1.0, 0.0, 0.0),
            col_p: SolidColor::from_rgb(1.0, 1.0, 0.0),
            col_s: SolidColor::from_rgb(0.0, 1.0, 0.0),
            col_x: SolidColor::from_rgb(0.7, 0.7, 0.7),
        }
    }
}

impl CpkColoring {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "CPKColoring"
    }

    pub fn col_c(&self) -> &SolidColor {
        &self.col_c
    }

    pub fn set_col_c(&mut self, value: SolidColor) {
        self.col_c = value;
    }

    pub fn col_n(&self) -> &SolidColor {
        &self.col_n
    }

    pub fn set_col_n(&mut self, value: SolidColor) {
        self.col_n = value;
    }

    pub fn col_o(&self) -> &SolidColor {
        &self.col_o
    }

    pub fn set_col_o(&mut self, value: SolidColor) {
        self.col_o = value;
    }

    pub fn col_h(&self) -> &SolidColor {
        &self.col_h
    }

    pub fn set_col_h(&mut self, value: SolidColor) {
        self.col_h = value;
    }

    pub fn col_s(&self) -> &SolidColor {
        &self.col_s
    }

    pub fn set_col_s(&mut self, value: SolidColor) {
        self.col_s = value;
    }

    pub fn col_p(&self) -> &SolidColor {
        &self.col_p
    }

    pub fn set_col_p(&mut self, value: SolidColor) {
        self.col_p = value;
    }

    pub fn col_x(&self) -> &SolidColor {
        &self.col_x
    }

    pub fn set_col_x(&mut self, value: SolidColor) {
        self.col_x = value;
    }

    pub fn set_atom_color(&mut self, element: &str, color: SolidColor) {
        match normalized_element(element).as_str() {
            "H" => self.col_h = color,
            "C" => self.col_c = color,
            "N" => self.col_n = color,
            "O" => self.col_o = color,
            "S" => self.col_s = color,
            "P" => self.col_p = color,
            _ => self.col_x = color,
        }
    }

    pub fn atom_color(&self, element: &str) -> &SolidColor {
        match normalized_element(element).as_str() {
            "H" => &self.col_h,
            "C" => &self.col_c,
            "N" => &self.col_n,
            "O" => &self.col_o,
            "S" => &self.col_s,
            "P" => &self.col_p,
            _ => &self.col_x,
        }
    }

    pub fn color_for_atom(&self, atom: &Atom) -> SolidColor {
        self.atom_color(&atom.element).clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BfacColoring {
    mode: i32,
    auto_mode: i32,
    low_color: SolidColor,
    high_color: SolidColor,
    low_param: f64,
    high_param: f64,
    auto_low_param: f64,
    auto_high_param: f64,
    center: Vector4,
}

impl Default for BfacColoring {
    fn default() -> Self {
        Self {
            mode: Self::BFC_BFAC,
            auto_mode: Self::BFA_MOL,
            low_color: SolidColor::from_rgb(0.0, 0.0, 1.0),
            high_color: SolidColor::from_rgb(1.0, 0.0, 0.0),
            low_param: 20.0,
            high_param: 80.0,
            auto_low_param: 20.0,
            auto_high_param: 80.0,
            center: Vector4::new3(0.0, 0.0, 0.0),
        }
    }
}

impl BfacColoring {
    pub const BFC_BFAC: i32 = 1;
    pub const BFC_OCC: i32 = 2;
    pub const BFC_CENTER: i32 = 3;
    pub const BFA_NONE: i32 = 0;
    pub const BFA_MOL: i32 = 1;
    pub const BFA_REND: i32 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "BfacColoring"
    }

    pub fn mode(&self) -> i32 {
        self.mode
    }

    pub fn set_mode(&mut self, value: i32) {
        self.mode = value;
    }

    pub fn auto_mode(&self) -> i32 {
        self.auto_mode
    }

    pub fn auto(&self) -> i32 {
        self.auto_mode()
    }

    pub fn set_auto_mode(&mut self, value: i32) {
        self.auto_mode = value;
    }

    pub fn set_auto(&mut self, value: i32) {
        self.set_auto_mode(value);
    }

    pub fn is_auto_mode(&self) -> bool {
        self.auto_mode != Self::BFA_NONE
    }

    pub fn low_color(&self) -> &SolidColor {
        &self.low_color
    }

    pub fn lowcol(&self) -> &SolidColor {
        self.low_color()
    }

    pub fn set_low_color(&mut self, value: SolidColor) {
        self.low_color = value;
    }

    pub fn set_lowcol(&mut self, value: SolidColor) {
        self.set_low_color(value);
    }

    pub fn high_color(&self) -> &SolidColor {
        &self.high_color
    }

    pub fn highcol(&self) -> &SolidColor {
        self.high_color()
    }

    pub fn set_high_color(&mut self, value: SolidColor) {
        self.high_color = value;
    }

    pub fn set_highcol(&mut self, value: SolidColor) {
        self.set_high_color(value);
    }

    pub fn low_param(&self) -> f64 {
        self.low_param
    }

    pub fn lowpar(&self) -> f64 {
        self.low_param()
    }

    pub fn set_low_param(&mut self, value: f64) {
        self.low_param = value;
    }

    pub fn set_lowpar(&mut self, value: f64) {
        self.set_low_param(value);
    }

    pub fn high_param(&self) -> f64 {
        self.high_param
    }

    pub fn highpar(&self) -> f64 {
        self.high_param()
    }

    pub fn set_high_param(&mut self, value: f64) {
        self.high_param = value;
    }

    pub fn set_highpar(&mut self, value: f64) {
        self.set_high_param(value);
    }

    pub fn auto_low_param(&self) -> f64 {
        self.auto_low_param
    }

    pub fn auto_high_param(&self) -> f64 {
        self.auto_high_param
    }

    pub fn center(&self) -> Vector4 {
        self.center
    }

    pub fn start_molecule(&mut self, molecule: &MolCoord) -> bool {
        if self.mode != Self::BFC_CENTER && !self.is_auto_mode() {
            return true;
        }
        if molecule.atoms().is_empty() {
            self.center = Vector4::new3(0.0, 0.0, 0.0);
            self.auto_low_param = 0.0;
            self.auto_high_param = 0.0;
            return true;
        }

        if self.mode == Self::BFC_CENTER {
            self.center = molecule_center(molecule);
            if !self.is_auto_mode() {
                return true;
            }
        }

        let mut min_value = f64::INFINITY;
        let mut max_value = f64::NEG_INFINITY;
        for atom in molecule.atoms() {
            let value = self.param_for_atom(atom);
            min_value = min_value.min(value);
            max_value = max_value.max(value);
        }
        self.auto_low_param = min_value;
        self.auto_high_param = max_value;
        true
    }

    pub fn color_for_atom(&self, atom: &Atom) -> SolidColor {
        let param = self.param_for_atom(atom);
        let (low, high) = if self.is_auto_mode() {
            (self.auto_low_param, self.auto_high_param)
        } else {
            (self.low_param, self.high_param)
        };

        if param < low {
            return self.low_color.clone();
        }
        if param > high {
            return self.high_color.clone();
        }

        let ratio = if (high - low).abs() < 1.0e-8 {
            1.0
        } else {
            (param - low) / (high - low)
        };
        interpolate_color(&self.high_color, &self.low_color, ratio)
    }

    pub fn param_for_atom(&self, atom: &Atom) -> f64 {
        if self.mode == Self::BFC_OCC {
            f64::from(atom.occupancy)
        } else if self.mode == Self::BFC_CENTER {
            (atom.position - self.center).length3d()
        } else {
            f64::from(atom.b_factor)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameLabel {
    aid: Option<i32>,
    str_aid: Option<String>,
    text: String,
    cache_id: Option<i32>,
}

impl NameLabel {
    pub fn by_id(aid: i32, text: impl Into<String>) -> Self {
        Self {
            aid: Some(aid),
            str_aid: None,
            text: text.into(),
            cache_id: None,
        }
    }

    pub fn by_str_aid(aid: impl Into<String>) -> Self {
        Self {
            aid: None,
            str_aid: Some(aid.into()),
            text: String::new(),
            cache_id: None,
        }
    }

    pub fn aid(&self) -> Option<i32> {
        self.aid
    }

    pub fn str_aid(&self) -> Option<&str> {
        self.str_aid.as_deref()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cache_id(&self) -> Option<i32> {
        self.cache_id
    }

    fn aid_for_write(&self) -> Option<String> {
        if let Some(aid) = self.aid {
            Some(aid.to_string())
        } else {
            self.str_aid.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NameLabelRenderer {
    max_label: i32,
    dispx: f64,
    dispy: f64,
    color: SolidColor,
    font_size: f64,
    font_name: String,
    font_style: String,
    font_weight: String,
    labels: VecDeque<NameLabel>,
    cache_revision: u64,
}

impl Default for NameLabelRenderer {
    fn default() -> Self {
        Self {
            max_label: 50,
            dispx: 0.0,
            dispy: 0.0,
            color: SolidColor::from_rgb(1.0, 1.0, 0.0),
            font_size: 12.0,
            font_name: "sans-serif".to_string(),
            font_style: "normal".to_string(),
            font_weight: "normal".to_string(),
            labels: VecDeque::new(),
            cache_revision: 0,
        }
    }
}

impl NameLabelRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "*namelabel"
    }

    pub fn max_label(&self) -> i32 {
        self.max_label
    }

    pub fn get_max_label(&self) -> i32 {
        self.max_label()
    }

    pub fn maxlabel(&self) -> i32 {
        self.max_label()
    }

    pub fn set_max_label(&mut self, value: i32) {
        self.max_label = value;
        self.trim_labels();
    }

    pub fn set_maxlabel(&mut self, value: i32) {
        self.set_max_label(value);
    }

    pub fn dispx(&self) -> f64 {
        self.dispx
    }

    pub fn set_dispx(&mut self, value: f64) {
        self.dispx = value;
    }

    pub fn dispy(&self) -> f64 {
        self.dispy
    }

    pub fn set_dispy(&mut self, value: f64) {
        self.dispy = value;
    }

    pub fn color(&self) -> &SolidColor {
        &self.color
    }

    pub fn set_color(&mut self, value: SolidColor) {
        self.color = value;
    }

    pub fn font_size(&self) -> f64 {
        self.font_size
    }

    pub fn get_font_size(&self) -> f64 {
        self.font_size()
    }

    pub fn set_font_size(&mut self, value: f64) {
        if (self.font_size - value).abs() > 1.0e-8 {
            self.font_size = value;
            self.invalidate_all();
        }
    }

    pub fn font_name(&self) -> &str {
        &self.font_name
    }

    pub fn get_font_name(&self) -> &str {
        self.font_name()
    }

    pub fn set_font_name(&mut self, value: impl Into<String>) {
        let value = value.into();
        if self.font_name != value {
            self.font_name = value;
            self.invalidate_all();
        }
    }

    pub fn font_style(&self) -> &str {
        &self.font_style
    }

    pub fn get_font_style(&self) -> &str {
        self.font_style()
    }

    pub fn set_font_style(&mut self, value: impl Into<String>) {
        let value = value.into();
        if self.font_style != value {
            self.font_style = value;
            self.invalidate_all();
        }
    }

    pub fn font_weight(&self) -> &str {
        &self.font_weight
    }

    pub fn get_font_wgt(&self) -> &str {
        self.font_weight()
    }

    pub fn set_font_weight(&mut self, value: impl Into<String>) {
        let value = value.into();
        if self.font_weight != value {
            self.font_weight = value;
            self.invalidate_all();
        }
    }

    pub fn set_font_wgt(&mut self, value: impl Into<String>) {
        self.set_font_weight(value);
    }

    pub fn is_transparent(&self) -> bool {
        true
    }

    pub fn is_hit_test_supported(&self) -> bool {
        false
    }

    pub fn labels(&self) -> &VecDeque<NameLabel> {
        &self.labels
    }

    pub fn label_count(&self) -> usize {
        self.labels.len()
    }

    pub fn cache_revision(&self) -> u64 {
        self.cache_revision
    }

    pub fn add_label_by_id(&mut self, aid: i32) -> bool {
        self.add_label_by_id_with_text(aid, "")
    }

    pub fn add_label(&mut self, aid: i32) -> bool {
        self.add_label_by_id(aid)
    }

    pub fn add_label_by_id_with_text(&mut self, aid: i32, text: impl Into<String>) -> bool {
        if self.labels.iter().any(|label| label.aid == Some(aid)) {
            return false;
        }
        self.labels.push_back(NameLabel::by_id(aid, text));
        self.trim_labels();
        true
    }

    pub fn remove_label_by_id(&mut self, aid: i32) -> bool {
        let Some(index) = self.labels.iter().position(|label| label.aid == Some(aid)) else {
            return false;
        };
        self.labels.remove(index);
        true
    }

    pub fn remove_label(&mut self, aid: i32) -> bool {
        self.remove_label_by_id(aid)
    }

    pub fn invalidate_all(&mut self) {
        self.cache_revision = self.cache_revision.saturating_add(1);
        for label in &mut self.labels {
            label.cache_id = None;
        }
    }

    pub fn default_label_for_atom(atom: &Atom) -> String {
        let mut residue = atom.res_id.to_string();
        if let Some(ins_code) = atom.ins_code {
            residue.push(ins_code);
        }
        let mut label = format!(
            "{} {}{} {}",
            atom.chain_id, atom.res_name, residue, atom.name
        );
        if let Some(alt_loc) = atom.alt_loc {
            label.push(':');
            label.push(alt_loc);
        }
        label
    }

    pub fn label_text_for_atom(&self, label: &NameLabel, atom: &Atom) -> String {
        if label.text.is_empty() {
            Self::default_label_for_atom(atom)
        } else {
            label.text.clone()
        }
    }

    pub fn write_to_ldom2_node(&self, node: &mut LDom2Node) -> usize {
        let mut count = 0;
        for label in &self.labels {
            let Some(aid) = label.aid_for_write() else {
                continue;
            };
            if aid.is_empty() {
                continue;
            }
            let child = node.append_child("label");
            child.set_attr_flag(false);
            child.set_str_attr("aid", aid);
            count += 1;
        }
        count
    }

    pub fn read_from_ldom2_node(&mut self, node: &LDom2Node) -> usize {
        let mut count = 0;
        for child in node
            .children()
            .iter()
            .filter(|child| child.tag_name() == "label")
        {
            let aid = child.str_attr("aid").to_string();
            if aid.is_empty() {
                continue;
            }
            self.labels.push_back(NameLabel::by_str_aid(aid));
            count += 1;
        }
        count
    }

    fn trim_labels(&mut self) {
        let limit = self.max_label.max(0) as usize;
        while self.labels.len() > limit {
            self.labels.pop_front();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionRenderMode {
    Stick,
    Point,
}

impl SelectionRenderMode {
    pub fn code(self) -> i32 {
        match self {
            Self::Stick => SelectionRenderer::MODE_STICK,
            Self::Point => SelectionRenderer::MODE_POINT,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionRenderer {
    linew: f64,
    dispx: f64,
    dispy: f64,
    color: SolidColor,
    mode: SelectionRenderMode,
    selection_text: String,
    selection: Selector,
    display_cache_valid: bool,
}

impl Default for SelectionRenderer {
    fn default() -> Self {
        Self {
            linew: 3.0,
            dispx: 3.0,
            dispy: 3.0,
            color: SolidColor::from_rgba(0.0, 1.0, 1.0, 0.5),
            mode: SelectionRenderMode::Stick,
            selection_text: "!*".to_string(),
            selection: Selector::None,
            display_cache_valid: false,
        }
    }
}

impl SelectionRenderer {
    pub const MODE_STICK: i32 = 0;
    pub const MODE_POINT: i32 = 1;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "*selection"
    }

    pub fn linew(&self) -> f64 {
        self.linew
    }

    pub fn set_linew(&mut self, value: f64) {
        self.linew = value;
        self.invalidate_display_cache();
    }

    pub fn dispx(&self) -> f64 {
        self.dispx
    }

    pub fn set_dispx(&mut self, value: f64) {
        self.dispx = value;
        self.invalidate_display_cache();
    }

    pub fn dispy(&self) -> f64 {
        self.dispy
    }

    pub fn set_dispy(&mut self, value: f64) {
        self.dispy = value;
        self.invalidate_display_cache();
    }

    pub fn color(&self) -> &SolidColor {
        &self.color
    }

    pub fn set_color(&mut self, value: SolidColor) {
        self.color = value;
        self.invalidate_display_cache();
    }

    pub fn mode(&self) -> SelectionRenderMode {
        self.mode
    }

    pub fn mode_code(&self) -> i32 {
        self.mode.code()
    }

    pub fn set_mode(&mut self, value: SelectionRenderMode) {
        self.mode = value;
        self.invalidate_display_cache();
    }

    pub fn set_mode_code(&mut self, value: i32) {
        self.mode = if value == Self::MODE_POINT {
            SelectionRenderMode::Point
        } else {
            SelectionRenderMode::Stick
        };
        self.invalidate_display_cache();
    }

    pub fn selection_text(&self) -> &str {
        &self.selection_text
    }

    pub fn selection(&self) -> &Selector {
        &self.selection
    }

    pub fn get_selection(&self) -> &Selector {
        self.selection()
    }

    pub fn sel(&self) -> &Selector {
        self.selection()
    }

    pub fn set_selection_text(&mut self, value: impl Into<String>) -> Result<()> {
        let value = value.into();
        let selection = parse_selector(&value)?;
        self.selection_text = value;
        self.selection = selection;
        self.invalidate_display_cache();
        Ok(())
    }

    pub fn is_rend_bond(&self) -> bool {
        self.mode == SelectionRenderMode::Stick
    }

    pub fn is_transparent(&self) -> bool {
        true
    }

    pub fn display_cache_valid(&self) -> bool {
        self.display_cache_valid
    }

    pub fn mark_display_cache_valid(&mut self) {
        self.display_cache_valid = true;
    }

    pub fn invalidate_display_cache(&mut self) {
        self.display_cache_valid = false;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RectSelDrawObj {
    draw_obj: DrawObj,
    color: SolidColor,
    paint_color: SolidColor,
    started: bool,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
}

impl Default for RectSelDrawObj {
    fn default() -> Self {
        Self {
            draw_obj: DrawObj::new(),
            color: SolidColor::from_rgb(0.2, 0.8, 1.0),
            paint_color: SolidColor::from_rgba(0.2, 0.8, 1.0, 0.5),
            started: false,
            start_x: 0,
            start_y: 0,
            end_x: 0,
            end_y: 0,
        }
    }
}

impl RectSelDrawObj {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "RectSelDrawObj"
    }

    pub fn is_enabled(&self) -> bool {
        self.draw_obj.is_enabled()
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.draw_obj.set_enabled(value);
        if !value {
            self.started = false;
        }
    }

    pub fn color(&self) -> &SolidColor {
        &self.color
    }

    pub fn set_color(&mut self, value: SolidColor) {
        self.color = value;
    }

    pub fn paint_color(&self) -> &SolidColor {
        &self.paint_color
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn start(&mut self, x: i32, y: i32) {
        self.started = true;
        self.start_x = x;
        self.start_y = y;
        self.end_x = x;
        self.end_y = y;
    }

    pub fn move_to(&mut self, x: i32, y: i32) {
        self.end_x = x;
        self.end_y = y;
    }

    pub fn r#move(&mut self, x: i32, y: i32) {
        self.move_to(x, y);
    }

    pub fn end(&mut self) {
        self.started = false;
    }

    pub fn left(&self) -> i32 {
        self.start_x.min(self.end_x)
    }

    pub fn get_left(&self) -> i32 {
        self.left()
    }

    pub fn top(&self) -> i32 {
        self.start_y.min(self.end_y)
    }

    pub fn get_top(&self) -> i32 {
        self.top()
    }

    pub fn width(&self) -> i32 {
        (self.start_x - self.end_x).abs()
    }

    pub fn get_width(&self) -> i32 {
        self.width()
    }

    pub fn height(&self) -> i32 {
        (self.start_y - self.end_y).abs()
    }

    pub fn get_height(&self) -> i32 {
        self.height()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimpleRenderer {
    line_width: f64,
    val_bond: bool,
    vb_scl1: f64,
    vb_scl2: f64,
    use_shader: bool,
    check_shader_ok: bool,
    line_gpu_prim_valid: bool,
    display_cache_valid: bool,
}

impl Default for SimpleRenderer {
    fn default() -> Self {
        Self {
            line_width: 1.2,
            val_bond: true,
            vb_scl1: 0.1,
            vb_scl2: 0.0,
            use_shader: false,
            check_shader_ok: false,
            line_gpu_prim_valid: false,
            display_cache_valid: false,
        }
    }
}

impl SimpleRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "simple"
    }

    pub fn is_rend_bond(&self) -> bool {
        true
    }

    pub fn line_width(&self) -> f64 {
        self.line_width
    }

    pub fn get_line_width(&self) -> f64 {
        self.line_width()
    }

    pub fn width(&self) -> f64 {
        self.line_width()
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.line_width = value;
        self.invalidate_display_cache();
    }

    pub fn set_width(&mut self, value: f64) {
        self.set_line_width(value);
    }

    pub fn val_bond(&self) -> bool {
        self.val_bond
    }

    pub fn get_val_bond(&self) -> bool {
        self.val_bond()
    }

    pub fn valbond(&self) -> bool {
        self.val_bond()
    }

    pub fn set_val_bond(&mut self, value: bool) {
        self.val_bond = value;
        self.invalidate_display_cache();
    }

    pub fn set_valbond(&mut self, value: bool) {
        self.set_val_bond(value);
    }

    pub fn vb_scl1(&self) -> f64 {
        self.vb_scl1
    }

    pub fn get_vb_scl1(&self) -> f64 {
        self.vb_scl1()
    }

    pub fn vb_scl_1(&self) -> f64 {
        self.vb_scl1()
    }

    pub fn set_vb_scl1(&mut self, value: f64) {
        self.vb_scl1 = value;
        self.invalidate_display_cache();
    }

    pub fn set_vb_scl_1(&mut self, value: f64) {
        self.set_vb_scl1(value);
    }

    pub fn vb_scl2(&self) -> f64 {
        self.vb_scl2
    }

    pub fn get_vb_scl2(&self) -> f64 {
        self.vb_scl2()
    }

    pub fn vb_scl_2(&self) -> f64 {
        self.vb_scl2()
    }

    pub fn set_vb_scl2(&mut self, value: f64) {
        self.vb_scl2 = value;
        self.invalidate_display_cache();
    }

    pub fn set_vb_scl_2(&mut self, value: f64) {
        self.set_vb_scl2(value);
    }

    pub fn use_shader(&self) -> bool {
        self.use_shader
    }

    pub fn check_shader_ok(&self) -> bool {
        self.check_shader_ok
    }

    pub fn line_gpu_prim_valid(&self) -> bool {
        self.line_gpu_prim_valid
    }

    pub fn display_cache_valid(&self) -> bool {
        self.display_cache_valid
    }

    pub fn invalidate_display_cache(&mut self) {
        self.display_cache_valid = false;
        self.line_gpu_prim_valid = false;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceRenderer {
    line_width: f64,
    piv_atom_name: String,
    display_cache_valid: bool,
}

impl Default for TraceRenderer {
    fn default() -> Self {
        Self {
            line_width: 1.2,
            piv_atom_name: String::new(),
            display_cache_valid: false,
        }
    }
}

impl TraceRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_name(&self) -> &'static str {
        "trace"
    }

    pub fn line_width(&self) -> f64 {
        self.line_width
    }

    pub fn get_line_width(&self) -> f64 {
        self.line_width()
    }

    pub fn width(&self) -> f64 {
        self.line_width()
    }

    pub fn set_line_width(&mut self, value: f64) {
        self.line_width = value;
        self.invalidate_display_cache();
    }

    pub fn set_width(&mut self, value: f64) {
        self.set_line_width(value);
    }

    pub fn is_hit_test_supported(&self) -> bool {
        true
    }

    pub fn piv_atom_name(&self) -> &str {
        &self.piv_atom_name
    }

    pub fn get_piv_atom_name(&self) -> &str {
        self.piv_atom_name()
    }

    pub fn pivotatom(&self) -> &str {
        self.piv_atom_name()
    }

    pub fn set_piv_atom_name(&mut self, value: impl Into<String>) {
        self.piv_atom_name = value.into();
        self.invalidate_display_cache();
    }

    pub fn set_pivotatom(&mut self, value: impl Into<String>) {
        self.set_piv_atom_name(value);
    }

    pub fn display_cache_valid(&self) -> bool {
        self.display_cache_valid
    }

    pub fn invalidate_display_cache(&mut self) {
        self.display_cache_valid = false;
    }
}

fn normalized_element(element: &str) -> String {
    element.trim().to_ascii_uppercase()
}

fn molecule_center(molecule: &MolCoord) -> Vector4 {
    if molecule.atoms().is_empty() {
        return Vector4::new3(0.0, 0.0, 0.0);
    }

    let mut center = Vector4::new3(0.0, 0.0, 0.0);
    for atom in molecule.atoms() {
        center = center + atom.position;
    }
    center / molecule.atoms().len() as f64
}

fn interpolate_color(high: &SolidColor, low: &SolidColor, ratio: f64) -> SolidColor {
    let clamped = ratio.clamp(0.0, 1.0);
    let inv = 1.0 - clamped;
    SolidColor::from_rgba(
        high.fr() * clamped + low.fr() * inv,
        high.fg() * clamped + low.fg() * inv,
        high.fb() * clamped + low.fb() * inv,
        high.fa() * clamped + low.fa() * inv,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(element: &str, b_factor: f32, occupancy: f32, x: f64) -> Atom {
        Atom {
            serial: 1,
            name: element.to_string(),
            alt_loc: None,
            res_name: "RES".to_string(),
            chain_id: "A".to_string(),
            res_id: 1,
            ins_code: None,
            position: Vector4::new3(x, 0.0, 0.0),
            occupancy,
            b_factor,
            element: element.to_string(),
            formal_charge: None,
            int_props: Default::default(),
        }
    }

    #[test]
    fn standard_coloring_defaults_match_reference() {
        let solid = SolidColoring::new();
        let carbon = atom("C", 10.0, 1.0, 0.0);
        assert_eq!(solid.type_name(), "SolidColoring");
        assert!(solid.color_for_atom(&carbon).is_none());

        let cpk = CpkColoring::new();
        assert_eq!(cpk.type_name(), "CPKColoring");
        assert_eq!(
            cpk.col_h().rgba8(),
            SolidColor::from_rgb(0.0, 1.0, 1.0).rgba8()
        );
        assert_eq!(
            cpk.col_c().rgba8(),
            SolidColor::from_rgb(1.0, 1.0, 0.75).rgba8()
        );
        assert_eq!(
            cpk.col_n().rgba8(),
            SolidColor::from_rgb(0.0, 0.0, 1.0).rgba8()
        );
        assert_eq!(
            cpk.col_o().rgba8(),
            SolidColor::from_rgb(1.0, 0.0, 0.0).rgba8()
        );
        assert_eq!(
            cpk.col_p().rgba8(),
            SolidColor::from_rgb(1.0, 1.0, 0.0).rgba8()
        );
        assert_eq!(
            cpk.col_s().rgba8(),
            SolidColor::from_rgb(0.0, 1.0, 0.0).rgba8()
        );
        assert_eq!(
            cpk.col_x().rgba8(),
            SolidColor::from_rgb(0.7, 0.7, 0.7).rgba8()
        );
        assert_eq!(cpk.color_for_atom(&carbon).rgba8(), cpk.col_c().rgba8());
        assert_eq!(
            cpk.color_for_atom(&atom("Xe", 10.0, 1.0, 0.0)).rgba8(),
            cpk.col_x().rgba8()
        );
    }

    #[test]
    fn cpk_coloring_setters_and_element_dispatch_match_reference() {
        let mut cpk = CpkColoring::new();
        let custom = SolidColor::from_rgb(0.25, 0.5, 0.75);
        cpk.set_atom_color("c", custom.clone());
        assert_eq!(cpk.atom_color("C").rgba8(), custom.rgba8());
        assert_eq!(
            cpk.color_for_atom(&atom("C", 0.0, 1.0, 0.0)).rgba8(),
            custom.rgba8()
        );

        let fallback = SolidColor::from_rgb(0.1, 0.2, 0.3);
        cpk.set_atom_color("Mg", fallback.clone());
        assert_eq!(cpk.atom_color("Mg").rgba8(), fallback.rgba8());
    }

    #[test]
    fn bfac_coloring_defaults_and_manual_gradient_match_reference() {
        let mut coloring = BfacColoring::new();
        coloring.set_auto_mode(BfacColoring::BFA_NONE);
        assert_eq!(coloring.type_name(), "BfacColoring");
        assert_eq!(coloring.mode(), BfacColoring::BFC_BFAC);
        assert_eq!(coloring.auto_mode(), BfacColoring::BFA_NONE);
        assert_eq!(
            coloring.low_color().rgba8(),
            SolidColor::from_rgb(0.0, 0.0, 1.0).rgba8()
        );
        assert_eq!(
            coloring.high_color().rgba8(),
            SolidColor::from_rgb(1.0, 0.0, 0.0).rgba8()
        );
        assert_eq!(coloring.low_param(), 20.0);
        assert_eq!(coloring.high_param(), 80.0);
        assert_eq!(coloring.lowcol(), coloring.low_color());
        assert_eq!(coloring.highcol(), coloring.high_color());
        assert_eq!(coloring.lowpar(), 20.0);
        assert_eq!(coloring.highpar(), 80.0);

        let custom_low = SolidColor::from_rgb(0.0, 1.0, 0.0);
        let custom_high = SolidColor::from_rgb(1.0, 1.0, 0.0);
        coloring.set_lowcol(custom_low.clone());
        coloring.set_highcol(custom_high.clone());
        coloring.set_lowpar(10.0);
        coloring.set_highpar(90.0);
        assert_eq!(coloring.low_color(), &custom_low);
        assert_eq!(coloring.high_color(), &custom_high);
        assert_eq!(coloring.low_param(), 10.0);
        assert_eq!(coloring.high_param(), 90.0);

        assert_eq!(
            coloring.color_for_atom(&atom("C", 0.0, 1.0, 0.0)).rgba8(),
            coloring.low_color().rgba8()
        );
        assert_eq!(
            coloring.color_for_atom(&atom("C", 100.0, 1.0, 0.0)).rgba8(),
            coloring.high_color().rgba8()
        );
        let mid = coloring.color_for_atom(&atom("C", 50.0, 1.0, 0.0)).rgba8();
        assert_eq!(mid.r, 128);
        assert_eq!(mid.g, 255);
        assert_eq!(mid.b, 0);
    }

    #[test]
    fn bfac_coloring_auto_and_modes_match_reference() {
        let mut molecule = MolCoord::new();
        molecule.push_atom(atom("C", 10.0, 0.25, -1.0));
        molecule.push_atom(atom("O", 30.0, 0.75, 1.0));

        let mut coloring = BfacColoring::new();
        assert_eq!(coloring.auto_mode(), BfacColoring::BFA_MOL);
        assert!(coloring.start_molecule(&molecule));
        assert_eq!(coloring.auto_low_param(), 10.0);
        assert_eq!(coloring.auto_high_param(), 30.0);

        coloring.set_mode(BfacColoring::BFC_OCC);
        assert!(coloring.start_molecule(&molecule));
        assert_eq!(coloring.auto_low_param(), 0.25);
        assert_eq!(coloring.auto_high_param(), 0.75);

        coloring.set_mode(BfacColoring::BFC_CENTER);
        coloring.set_auto_mode(BfacColoring::BFA_NONE);
        assert!(coloring.start_molecule(&molecule));
        assert_eq!(coloring.center(), Vector4::new3(0.0, 0.0, 0.0));
        assert_eq!(coloring.param_for_atom(&molecule.atoms()[0]), 1.0);
    }

    #[test]
    fn name_label_renderer_defaults_and_label_queue_match_reference() {
        let mut renderer = NameLabelRenderer::new();
        assert_eq!(renderer.type_name(), "*namelabel");
        assert_eq!(renderer.max_label(), 50);
        assert_eq!(renderer.dispx(), 0.0);
        assert_eq!(renderer.dispy(), 0.0);
        assert_eq!(
            renderer.color().rgba8(),
            SolidColor::from_rgb(1.0, 1.0, 0.0).rgba8()
        );
        assert_eq!(renderer.font_size(), 12.0);
        assert_eq!(renderer.font_name(), "sans-serif");
        assert_eq!(renderer.font_style(), "normal");
        assert_eq!(renderer.font_weight(), "normal");
        assert!(renderer.is_transparent());
        assert!(!renderer.is_hit_test_supported());

        assert!(renderer.add_label_by_id(1));
        assert!(!renderer.add_label_by_id(1));
        assert!(renderer.add_label_by_id_with_text(2, "custom"));
        assert_eq!(renderer.label_count(), 2);
        assert_eq!(renderer.labels()[1].text(), "custom");

        renderer.set_max_label(1);
        assert_eq!(renderer.label_count(), 1);
        assert_eq!(renderer.labels()[0].aid(), Some(2));
        assert!(renderer.remove_label_by_id(2));
        assert!(!renderer.remove_label_by_id(2));
    }

    #[test]
    fn name_label_renderer_formats_atoms_and_ldom2_labels() {
        let atom = Atom {
            serial: 42,
            name: "CA".to_string(),
            alt_loc: Some('B'),
            res_name: "ALA".to_string(),
            chain_id: "A".to_string(),
            res_id: 10,
            ins_code: Some('I'),
            position: Vector4::new3(0.0, 0.0, 0.0),
            occupancy: 1.0,
            b_factor: 0.0,
            element: "C".to_string(),
            formal_charge: None,
            int_props: Default::default(),
        };
        assert_eq!(
            NameLabelRenderer::default_label_for_atom(&atom),
            "A ALA10I CA:B"
        );

        let mut renderer = NameLabelRenderer::new();
        assert!(renderer.add_label_by_id(42));
        let mut node = LDom2Node::new();
        node.set_tag_name("renderer");
        assert_eq!(renderer.write_to_ldom2_node(&mut node), 1);
        assert_eq!(node.children()[0].tag_name(), "label");
        assert_eq!(node.children()[0].str_attr("aid").to_string(), "42");

        let mut loaded = NameLabelRenderer::new();
        assert_eq!(loaded.read_from_ldom2_node(&node), 1);
        assert_eq!(loaded.labels()[0].str_aid(), Some("42"));

        let before = loaded.cache_revision();
        loaded.set_font_name("serif");
        assert_eq!(loaded.cache_revision(), before + 1);
    }

    #[test]
    fn selection_renderer_defaults_modes_and_selection_match_reference() {
        let mut renderer = SelectionRenderer::new();
        assert_eq!(renderer.type_name(), "*selection");
        assert_eq!(renderer.linew(), 3.0);
        assert_eq!(renderer.dispx(), 3.0);
        assert_eq!(renderer.dispy(), 3.0);
        assert_eq!(
            renderer.color().rgba8(),
            SolidColor::from_rgba(0.0, 1.0, 1.0, 0.5).rgba8()
        );
        assert_eq!(renderer.mode(), SelectionRenderMode::Stick);
        assert_eq!(renderer.mode_code(), SelectionRenderer::MODE_STICK);
        assert!(renderer.is_rend_bond());
        assert!(renderer.is_transparent());
        assert_eq!(renderer.selection_text(), "!*");
        assert_eq!(renderer.selection(), &Selector::None);
        assert_eq!(renderer.sel(), &Selector::None);

        renderer.mark_display_cache_valid();
        assert!(renderer.display_cache_valid());
        renderer.set_mode(SelectionRenderMode::Point);
        assert_eq!(renderer.mode_code(), SelectionRenderer::MODE_POINT);
        assert!(!renderer.is_rend_bond());
        assert!(!renderer.display_cache_valid());

        renderer.set_selection_text("name CA").unwrap();
        assert_eq!(renderer.selection_text(), "name CA");
        assert!(renderer.set_selection_text("name").is_err());
    }

    #[test]
    fn rect_selection_draw_object_tracks_reference_geometry() {
        let mut draw = RectSelDrawObj::new();
        assert_eq!(draw.type_name(), "RectSelDrawObj");
        assert!(!draw.is_enabled());
        assert!(!draw.is_started());
        assert_eq!(
            draw.color().rgba8(),
            SolidColor::from_rgb(0.2, 0.8, 1.0).rgba8()
        );
        assert_eq!(
            draw.paint_color().rgba8(),
            SolidColor::from_rgba(0.2, 0.8, 1.0, 0.5).rgba8()
        );

        draw.set_enabled(true);
        draw.start(20, 10);
        draw.move_to(5, 30);
        assert!(draw.is_started());
        assert_eq!(draw.left(), 5);
        assert_eq!(draw.top(), 10);
        assert_eq!(draw.width(), 15);
        assert_eq!(draw.height(), 20);
        draw.set_enabled(false);
        assert!(!draw.is_started());
    }

    #[test]
    fn simple_renderer_defaults_and_accessors_match_reference() {
        let mut renderer = SimpleRenderer::new();
        assert!(renderer.val_bond());
        assert!(renderer.valbond());
        assert_eq!(renderer.width(), 1.2);
        assert_eq!(renderer.vb_scl1(), 0.1);
        assert_eq!(renderer.vb_scl2(), 0.0);
        assert_eq!(renderer.type_name(), "simple");
        assert!(renderer.is_rend_bond());
        assert!(!renderer.use_shader());
        assert!(!renderer.check_shader_ok());
        assert!(!renderer.line_gpu_prim_valid());

        renderer.set_valbond(false);
        assert!(!renderer.val_bond());
        renderer.set_val_bond(true);
        assert!(renderer.val_bond());

        renderer.set_vb_scl1(-0.1);
        assert_eq!(renderer.vb_scl1(), -0.1);
        renderer.set_vb_scl2(0.1);
        assert_eq!(renderer.vb_scl2(), 0.1);
        renderer.set_width(2.5);
        assert_eq!(renderer.line_width(), 2.5);
    }

    #[test]
    fn trace_renderer_accessors_match_reference() {
        let mut renderer = TraceRenderer::new();
        assert_eq!(renderer.type_name(), "trace");
        assert_eq!(renderer.width(), 1.2);
        assert!(renderer.is_hit_test_supported());
        assert_eq!(renderer.piv_atom_name(), "");

        renderer.set_width(2.0);
        assert_eq!(renderer.line_width(), 2.0);
        renderer.set_piv_atom_name("CA");
        assert_eq!(renderer.piv_atom_name(), "CA");
        renderer.set_piv_atom_name("CB");
        assert_eq!(renderer.piv_atom_name(), "CB");
    }
}
