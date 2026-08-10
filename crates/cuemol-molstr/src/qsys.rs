use crate::{
    read_mmcif_file, read_mmcif_str, read_mol2_file, read_mol2_str, read_pdb_file, read_pdb_str,
    read_qdf_mol_bytes, read_qdfpdb_file, read_sdf_file, read_sdf_str, write_pdb_string, Atom,
    MolCoord, Selector, SolidColoring,
};
use cuemol_core::{
    parse_ldom2_xml, write_ldom2_xml, CueMolError, LDom2InStream, LDom2Node, LDom2Tree, Matrix4,
    Quat, RangeSet, Result, Uid, Vector4,
};
use cuemol_gfx::{RawHitData, SolidColor};
use cuemol_qsys::{Camera, CenterMark, StereoMode, StyleManager, View};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const OBJECT_UID_BASE: u64 = 1_000_000_000;
const RENDERER_UID_BASE: u64 = 2_000_000_000;
const VIEW_UID_BASE: u64 = 3_000_000_000;
const UID_SCENE_STRIDE: u64 = 1_000_000;
const UID_OBJECT_STRIDE: u64 = 1_000;

type PendingAtomIntrRenderer = (String, Vec<PendingAtomIntrDef>);

enum PendingAtomIntrDef {
    Atoms(AtomIntrKind, Vec<String>),
    Selections(AtomIntrKind, Vec<String>),
    Positions(AtomIntrKind, Vec<Vector4>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolRenderer {
    uid: Uid,
    name: String,
    renderer_type: String,
    qsc_attrs: BTreeMap<String, String>,
    qsc_children: Vec<LDom2Node>,
    atom_intr_lines: Vec<AtomIntrLine>,
    selection: Selector,
    coloring: SolidColoring,
    default_color: SolidColor,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomIntrLine {
    pub a0: String,
    pub a1: String,
    pub a2: Option<String>,
    pub a3: Option<String>,
    kind: AtomIntrKind,
    aids: Vec<String>,
    selectors: Vec<String>,
    positions: Vec<Vector4>,
    active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomIntrKind {
    Distance,
    Angle,
    Torsion,
}

impl AtomIntrKind {
    fn mode(self) -> u8 {
        match self {
            Self::Distance => 1,
            Self::Angle => 2,
            Self::Torsion => 3,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::Distance => "line",
            Self::Angle => "angle",
            Self::Torsion => "torsion",
        }
    }

    fn atom_count(self) -> usize {
        usize::from(self.mode()) + 1
    }
}

impl AtomIntrLine {
    pub fn kind(&self) -> AtomIntrKind {
        self.kind
    }

    pub fn aids(&self) -> &[String] {
        &self.aids
    }

    pub fn positions(&self) -> &[Vector4] {
        &self.positions
    }

    pub fn selectors(&self) -> &[String] {
        &self.selectors
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn deactivate(&mut self) -> bool {
        let changed = self.active;
        self.active = false;
        changed
    }

    pub fn from_atom_references(
        kind: AtomIntrKind,
        aids: Vec<String>,
        labels: Vec<String>,
    ) -> Self {
        Self {
            a0: labels.first().cloned().unwrap_or_default(),
            a1: labels.get(1).cloned().unwrap_or_default(),
            a2: labels.get(2).cloned(),
            a3: labels.get(3).cloned(),
            kind,
            aids,
            selectors: Vec::new(),
            positions: Vec::new(),
            active: true,
        }
    }

    pub fn from_selections(kind: AtomIntrKind, selectors: Vec<String>) -> Self {
        Self {
            a0: selectors.first().cloned().unwrap_or_default(),
            a1: selectors.get(1).cloned().unwrap_or_default(),
            a2: selectors.get(2).cloned(),
            a3: selectors.get(3).cloned(),
            kind,
            aids: Vec::new(),
            selectors,
            positions: Vec::new(),
            active: true,
        }
    }

    pub fn from_positions(kind: AtomIntrKind, positions: Vec<Vector4>) -> Self {
        let labels = positions
            .iter()
            .map(|position| format!("({:.2},{:.2},{:.2})", position.x, position.y, position.z))
            .collect::<Vec<_>>();
        Self::from_positions_with_labels(kind, positions, labels)
    }

    pub fn from_positions_with_labels(
        kind: AtomIntrKind,
        positions: Vec<Vector4>,
        mut labels: Vec<String>,
    ) -> Self {
        let supplied = labels.len();
        labels.extend(positions.iter().skip(supplied).map(ToString::to_string));
        Self {
            a0: labels.first().cloned().unwrap_or_default(),
            a1: labels.get(1).cloned().unwrap_or_default(),
            a2: labels.get(2).cloned(),
            a3: labels.get(3).cloned(),
            kind,
            aids: Vec::new(),
            selectors: Vec::new(),
            positions,
            active: true,
        }
    }
}

pub fn atom_intr_defs_json(lines: &[AtomIntrLine]) -> String {
    let entries = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.active)
        .map(|(id, line)| {
            let labels = [&line.a0, &line.a1]
                .into_iter()
                .map(Some)
                .chain([line.a2.as_ref(), line.a3.as_ref()])
                .take(line.kind.atom_count())
                .enumerate()
                .filter_map(|(index, label)| {
                    label.map(|label| format!("\"a{index}\":\"{}\"", escape_json(label)))
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{\"id\":{id},\"mode\":{},{labels}}}", line.kind.mode())
        })
        .collect::<Vec<_>>();
    format!("[{}]", entries.join(","))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QscLoadStatus {
    Loaded { scene_uid: Uid },
    GitLfsPointer { oid: String, size: u64 },
}

pub type MolView = View;

impl MolRenderer {
    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn renderer_type(&self) -> &str {
        &self.renderer_type
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

    pub fn set_selection(&mut self, value: Selector) {
        self.selection = value;
    }

    pub fn set_sel(&mut self, value: Selector) {
        self.set_selection(value);
    }

    pub fn coloring(&self) -> &SolidColoring {
        &self.coloring
    }

    pub fn get_col_schm(&self) -> &SolidColoring {
        self.coloring()
    }

    pub fn set_col_schm(&mut self, value: SolidColoring) {
        self.coloring = value;
    }

    pub fn set_coloring(&mut self, value: SolidColoring) {
        self.set_col_schm(value);
    }

    pub fn default_color(&self) -> &SolidColor {
        &self.default_color
    }

    pub fn get_default_color(&self) -> &SolidColor {
        self.default_color()
    }

    pub fn defaultcolor(&self) -> &SolidColor {
        self.default_color()
    }

    pub fn set_default_color(&mut self, value: SolidColor) {
        self.default_color = value;
    }

    pub fn set_defaultcolor(&mut self, value: SolidColor) {
        self.set_default_color(value);
    }

    pub fn qsc_attr(&self, name: &str) -> Option<&str> {
        self.qsc_attrs.get(name).map(String::as_str)
    }

    pub fn qsc_attrs(&self) -> &BTreeMap<String, String> {
        &self.qsc_attrs
    }

    pub fn qsc_children(&self) -> &[LDom2Node] {
        &self.qsc_children
    }

    pub fn set_qsc_children(&mut self, children: Vec<LDom2Node>) {
        self.qsc_children = children;
    }

    pub fn qsc_child_attr(&self, child_tag: &str, attr: &str) -> Option<String> {
        let child = self
            .qsc_children
            .iter()
            .find(|child| child.tag_name() == child_tag)?;
        let value = if attr == "type" && !child.type_name().is_empty() {
            child.type_name().to_string()
        } else {
            child.str_attr(attr).to_string()
        };
        (!value.is_empty()).then_some(value)
    }

    pub fn set_qsc_child_attr(
        &mut self,
        child_tag: impl Into<String>,
        attr: &str,
        value: impl Into<String>,
    ) {
        let child_tag = child_tag.into();
        let value = value.into();
        let child = if let Some(index) = self
            .qsc_children
            .iter()
            .position(|child| child.tag_name() == child_tag)
        {
            &mut self.qsc_children[index]
        } else {
            let mut child = LDom2Node::new();
            child.set_tag_name(&child_tag);
            self.qsc_children.push(child);
            self.qsc_children.last_mut().unwrap()
        };
        if attr == "type" {
            child.set_type_name(value);
        } else {
            child.set_str_attr(attr, value);
        }
    }

    pub fn remove_qsc_child_attr(&mut self, child_tag: &str, attr: &str) -> bool {
        let Some(index) = self
            .qsc_children
            .iter()
            .position(|child| child.tag_name() == child_tag)
        else {
            return false;
        };
        let child = &mut self.qsc_children[index];
        let removed = if attr == "type" {
            let changed = !child.type_name().is_empty();
            child.set_type_name("");
            changed
        } else {
            child.remove_child(attr)
        };
        if child.child_count() == 0 && child.type_name().is_empty() && child.contents().is_empty() {
            self.qsc_children.remove(index);
        }
        removed
    }

    pub fn atom_intr_lines(&self) -> &[AtomIntrLine] {
        &self.atom_intr_lines
    }

    pub fn set_atom_intr_lines(&mut self, lines: Vec<AtomIntrLine>) {
        self.atom_intr_lines = lines;
    }

    pub fn group(&self) -> Option<&str> {
        self.qsc_attr("group")
    }

    pub fn style(&self) -> Option<&str> {
        self.qsc_attr("style")
    }

    /// Resolve a renderer property through the C++ value cascade (`applyStyles`):
    /// an inline `.qsc` attribute on this renderer overrides any preset; failing
    /// that, the renderer's `style="A,B"` list is resolved against `styles` in
    /// `scope_id` (a later style in the list wins). Returns `None` when neither
    /// the renderer nor any referenced style defines `attr`.
    ///
    /// This is what wires a scene's `<renderer style="DefaultBallStick">` to the
    /// preset's `bondw`/`sphr`/… so renderers inherit styled geometry instead of
    /// hardcoded defaults.
    pub fn resolved_attr(
        &self,
        attr: &str,
        styles: &StyleManager,
        scope_id: u64,
    ) -> Option<String> {
        if let Some(inline) = self.qsc_attr(attr) {
            if !inline.is_empty() {
                return Some(inline.to_string());
            }
        }
        let style_list = self.style()?;
        styles.resolve_style_attr(scope_id, style_list, attr)
    }

    /// Resolve an attribute on a nested child element of the renderer's style
    /// preset (e.g. the ribbon `<helix>`/`<sheet>`/`<coil>` cross-section
    /// sub-elements). Flat `.qsc` inline attributes cannot express nested
    /// children, so this resolves through the `style="A,B"` list only.
    pub fn resolved_child_attr(
        &self,
        child_tag: &str,
        attr: &str,
        styles: &StyleManager,
        scope_id: u64,
    ) -> Option<String> {
        if let Some(node) = self
            .qsc_children
            .iter()
            .find(|node| node.tag_name() == child_tag)
        {
            let value = node.str_attr(attr).to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
        let style_list = self.style()?;
        styles.resolve_style_child_attr(scope_id, style_list, child_tag, attr)
    }

    /// Resolve the `type` of a nested child element of this renderer's style preset
    /// (e.g. the renderer's own `<coloring type="...">` scheme). `type` is an LDom2
    /// reserved attribute, so it has a dedicated resolver.
    pub fn resolved_child_type(
        &self,
        child_tag: &str,
        styles: &StyleManager,
        scope_id: u64,
    ) -> Option<String> {
        if let Some(node) = self
            .qsc_children
            .iter()
            .find(|node| node.tag_name() == child_tag)
        {
            let value = if node.type_name().is_empty() {
                node.str_attr("type").to_string()
            } else {
                node.type_name().to_string()
            };
            if !value.is_empty() {
                return Some(value);
            }
        }
        let style_list = self.style()?;
        styles.resolve_style_child_type(scope_id, style_list, child_tag)
    }

    /// The renderer's `xformMat` display transform (C++ `Renderer::getXformMatrix`,
    /// applied to its geometry via `multMatrix` at display). Identity when unset or
    /// malformed. Captured from the `<renderer xformMat="(...)">` attribute.
    pub fn xform_matrix(&self) -> Matrix4 {
        self.qsc_attr("xformMat")
            .and_then(parse_xform_mat)
            .unwrap_or(Matrix4::IDENTITY)
    }

    pub fn is_visible(&self) -> bool {
        !matches!(self.qsc_attr("visible"), Some("false" | "0" | "no" | "off"))
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.qsc_attrs
            .insert("visible".to_string(), visible.to_string());
    }

    pub fn set_qsc_attr(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.qsc_attrs.insert(name.into(), value.into());
    }

    pub fn remove_qsc_attr(&mut self, name: &str) -> bool {
        self.qsc_attrs.remove(name).is_some()
    }

    pub fn append_atom_intr_positions(
        &mut self,
        kind: AtomIntrKind,
        positions: Vec<Vector4>,
    ) -> Result<usize> {
        if positions.len() != kind.atom_count() {
            return Err(CueMolError::invalid_argument(format!(
                "AtomIntr {} requires {} positions",
                kind.tag(),
                kind.atom_count()
            )));
        }
        let id = self.atom_intr_lines.len();
        self.atom_intr_lines
            .push(AtomIntrLine::from_positions(kind, positions));
        Ok(id)
    }

    pub fn append_atom_intr_selections(
        &mut self,
        kind: AtomIntrKind,
        selections: Vec<String>,
    ) -> Result<usize> {
        if selections.len() != kind.atom_count() {
            return Err(CueMolError::invalid_argument(format!(
                "AtomIntr {} requires {} selections",
                kind.tag(),
                kind.atom_count()
            )));
        }
        for selection in &selections {
            crate::parse_selector(selection)?;
        }
        let id = self.atom_intr_lines.len();
        self.atom_intr_lines
            .push(AtomIntrLine::from_selections(kind, selections));
        Ok(id)
    }

    pub fn remove_atom_intr(&mut self, id: usize) -> bool {
        let Some(line) = self.atom_intr_lines.get_mut(id) else {
            return false;
        };
        line.deactivate()
    }

    pub fn append_atom_intr_line(&mut self, line: AtomIntrLine) -> Result<usize> {
        let count = line.kind.atom_count();
        let valid = line.aids.len() == count
            || line.selectors.len() == count
            || line.positions.len() == count;
        if !valid {
            return Err(CueMolError::invalid_argument(format!(
                "AtomIntr {} requires {count} elements",
                line.kind.tag()
            )));
        }
        let id = self.atom_intr_lines.len();
        self.atom_intr_lines.push(line);
        Ok(id)
    }

    pub fn get_defs_json(&self) -> String {
        atom_intr_defs_json(&self.atom_intr_lines)
    }

    pub fn is_hit_test_supported(&self) -> bool {
        !matches!(
            self.renderer_type.as_str(),
            "selection" | "sel" | "*group" | "unitcell"
        )
    }

    pub fn interp_hit(&self, hit_data: &dyn RawHitData, molecule: &MolCoord) -> String {
        let size = hit_data.data_size(self.uid);
        if size == 0 {
            return String::new();
        }
        let atom_ids = (0..size)
            .filter_map(|index| hit_data.data_at(self.uid, index, 0))
            .collect::<Vec<_>>();
        if atom_ids.is_empty() {
            return String::new();
        }

        if atom_ids.len() > 1 {
            let mut ranges = RangeSet::new();
            for atom_id in atom_ids {
                ranges.insert_value(atom_id);
            }
            return format!(
                "\"objtype\":\"MolCoord\",\"size\":{},\"sel\":\"aid {}\",",
                size, ranges
            );
        }

        let atom_id = atom_ids[0];
        let Some(atom) = molecule.get_atom_by_id(atom_id) else {
            return String::new();
        };
        format!(
            "\"objtype\":\"MolCoord\",\"size\":1,\"sel\":\"aid {atom_id}\",\"atom_id\":{atom_id},\"message\":\"{}\",\"occ\":{:.2},\"bfac\":{:.2},\"x\":{:.2},\"y\":{:.2},\"z\":{:.2},",
            escape_json(&format_atom_message(atom)),
            atom.occupancy,
            atom.b_factor,
            atom.position.x,
            atom.position.y,
            atom.position.z,
        )
    }
}

fn format_atom_message(atom: &Atom) -> String {
    let insertion = atom
        .ins_code
        .map_or_else(String::new, |value| value.to_string());
    format!(
        "{} {} {}{} {}",
        atom.chain_id, atom.res_name, atom.res_id, insertion, atom.name
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolObject {
    uid: Uid,
    name: String,
    object_type: String,
    source_type: String,
    source: String,
    alt_source: String,
    modified: bool,
    ui_locked: bool,
    ui_collapsed: bool,
    ui_order: i32,
    mol: MolCoord,
    renderers: Vec<MolRenderer>,
    next_renderer_uid: u64,
    /// Object-level `<coloring type="PaintColoring">` entries `(selection, color)`
    /// in declaration order (first matching selection wins). Empty when the scene
    /// declares no paint coloring. Renderers whose colour resolves to `$molcol`
    /// (e.g. CPKColoring carbon) defer to this — the C++ object→renderer colour
    /// cascade (`ColSchmHolder::getColor`/`evalMolColor`).
    paint_coloring: Vec<(String, String)>,
    /// The object's `<coloring type="...">` scheme name (e.g. `PaintColoring`,
    /// `RainbowColoring`), and that tag's attributes (e.g. rainbow `start_hue`/
    /// `end_hue`/`sat`/`bri`). `None` when no object coloring is declared.
    coloring_type: Option<String>,
    coloring_attrs: BTreeMap<String, String>,
    qsc_coloring_node: Option<LDom2Node>,
    qsc_extra_attrs: BTreeMap<String, String>,
    qsc_extension_nodes: Vec<LDom2Node>,
    qsc_extra_data: BTreeMap<String, Vec<u8>>,
    /// The object's `xformMat` transform (C++ `Object::getXformMatrix`). In C++
    /// it is propagated onto every atom (`MolAtom::getPos` applies it), so the
    /// rendered structure sits at its transformed position — e.g. a structure
    /// superposed by `superposeSSM1`'s by-property mode persists its fit here.
    /// `None` (identity) for a freshly loaded molecule. The render path bakes it
    /// into the snapshot geometry instead of mutating `MolCoord`, so CLI atom
    /// queries / exports of the same object stay in raw coordinates.
    xform: Option<Matrix4>,
    /// Object-level visibility (C++ `Object::visible`, `Object.qif:33`, default
    /// `true`). An invisible object hides ALL its renderers at display — the
    /// C++ display loop skips it before the per-renderer check (`Scene.cpp:635`
    /// `if (pObj.isnull() || !pObj->isVisible()) continue;`). Persisted as a
    /// `visible="false"` attribute on the `<object>` element only when hidden
    /// (the default-true property is omitted otherwise, like `xformMat`).
    visible: bool,
}

impl MolObject {
    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn object_type(&self) -> &str {
        &self.object_type
    }

    pub fn set_object_type(&mut self, object_type: impl Into<String>) {
        self.object_type = object_type.into();
        self.modified = true;
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.modified = true;
    }

    pub fn mol(&self) -> &MolCoord {
        &self.mol
    }

    pub fn source_type(&self) -> &str {
        &self.source_type
    }

    pub fn mol_mut(&mut self) -> &mut MolCoord {
        self.modified = true;
        &mut self.mol
    }

    pub fn replace_mol(&mut self, molecule: MolCoord) {
        if self.mol != molecule {
            self.mol = molecule;
            self.modified = true;
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
    }

    pub fn alt_source(&self) -> &str {
        &self.alt_source
    }

    pub fn set_alt_source(&mut self, source: impl Into<String>) {
        self.alt_source = source.into();
        self.modified = true;
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    pub fn is_ui_locked(&self) -> bool {
        self.ui_locked
    }

    pub fn set_ui_locked(&mut self, locked: bool) {
        self.ui_locked = locked;
        self.modified = true;
    }

    pub fn is_ui_collapsed(&self) -> bool {
        self.ui_collapsed
    }

    pub fn set_ui_collapsed(&mut self, collapsed: bool) {
        self.ui_collapsed = collapsed;
        self.modified = true;
    }

    pub fn ui_order(&self) -> i32 {
        self.ui_order
    }

    pub fn set_ui_order(&mut self, order: i32) {
        self.ui_order = order;
    }

    pub fn renderers(&self) -> &[MolRenderer] {
        &self.renderers
    }

    pub fn move_renderer_relative(
        &mut self,
        renderer_uid: Uid,
        target_uid: Uid,
        after: bool,
    ) -> bool {
        if renderer_uid == target_uid {
            return false;
        }
        let Some(source_index) = self
            .renderers
            .iter()
            .position(|renderer| renderer.uid == renderer_uid)
        else {
            return false;
        };
        if !self
            .renderers
            .iter()
            .any(|renderer| renderer.uid == target_uid)
        {
            return false;
        }
        let renderer = self.renderers.remove(source_index);
        let target_index = self
            .renderers
            .iter()
            .position(|candidate| candidate.uid == target_uid)
            .expect("target renderer was checked before removing a different renderer");
        let insert_index = target_index + usize::from(after);
        if insert_index == source_index {
            self.renderers.insert(source_index, renderer);
            return false;
        }
        self.renderers.insert(insert_index, renderer);
        for (index, renderer) in self.renderers.iter_mut().enumerate() {
            renderer
                .qsc_attrs
                .insert("ui_order".to_string(), (index + 1).to_string());
        }
        self.modified = true;
        true
    }

    /// Object-level paint-coloring entries `(selection, color)`, first-match-wins.
    pub fn paint_coloring(&self) -> &[(String, String)] {
        &self.paint_coloring
    }

    pub fn set_paint_coloring(&mut self, entries: Vec<(String, String)>) {
        self.paint_coloring = entries;
    }

    /// The object's `<coloring type="...">` scheme name, if any.
    pub fn coloring_type(&self) -> Option<&str> {
        self.coloring_type.as_deref()
    }

    /// An attribute of the object's `<coloring>` tag (e.g. rainbow `start_hue`).
    pub fn coloring_attr(&self, name: &str) -> Option<&str> {
        self.coloring_attrs.get(name).map(String::as_str)
    }

    pub fn coloring_attrs(&self) -> &BTreeMap<String, String> {
        &self.coloring_attrs
    }

    pub fn set_coloring(&mut self, coloring_type: Option<String>, attrs: BTreeMap<String, String>) {
        self.qsc_coloring_node = if coloring_type.as_deref() == Some("ScriptColoring") {
            attrs.get("script").map(|script| {
                let mut node = LDom2Node::new();
                node.set_tag_name("coloring");
                node.set_type_name("ScriptColoring");
                node.append_child("script").set_contents(script.as_str());
                node
            })
        } else {
            None
        };
        self.coloring_type = coloring_type;
        self.coloring_attrs = attrs;
    }

    pub fn qsc_coloring_node(&self) -> Option<&LDom2Node> {
        self.qsc_coloring_node.as_ref()
    }

    pub fn set_qsc_coloring_node(&mut self, node: Option<LDom2Node>) {
        let Some(node) = node else {
            self.coloring_type = None;
            self.coloring_attrs.clear();
            self.paint_coloring.clear();
            self.qsc_coloring_node = None;
            self.modified = true;
            return;
        };
        let coloring_type = if node.type_name().is_empty() {
            let value = node.str_attr("type").to_string();
            (!value.is_empty()).then_some(value)
        } else {
            Some(node.type_name().to_string())
        };
        self.coloring_attrs = node
            .children()
            .iter()
            .filter(|child| child.is_attr() && child.tag_name() != "type")
            .map(|child| (child.tag_name().to_string(), child.value().to_string()))
            .collect();
        if coloring_type.as_deref() == Some("ScriptColoring") {
            if let Some(script) = node
                .children()
                .iter()
                .find(|child| !child.is_attr() && child.tag_name() == "script")
            {
                let source = if !script.value().is_empty() {
                    script.value()
                } else {
                    script.contents()
                };
                self.coloring_attrs
                    .insert("script".to_string(), source.to_string());
            }
        }
        self.paint_coloring = node
            .children()
            .iter()
            .filter(|child| !child.is_attr() && child.tag_name() == "paint")
            .filter_map(|child| {
                let selection = child.str_attr("sel").to_string();
                let color = child.str_attr("color").to_string();
                (!selection.is_empty() && !color.is_empty()).then_some((selection, color))
            })
            .collect();
        self.coloring_type = coloring_type;
        self.qsc_coloring_node = Some(node);
        self.modified = true;
    }

    pub fn qsc_extra_attrs(&self) -> &BTreeMap<String, String> {
        &self.qsc_extra_attrs
    }

    pub fn set_qsc_extra_attr(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.qsc_extra_attrs.insert(name.into(), value.into());
        self.modified = true;
    }

    pub fn remove_qsc_extra_attr(&mut self, name: &str) -> bool {
        let removed = self.qsc_extra_attrs.remove(name).is_some();
        self.modified |= removed;
        removed
    }

    pub fn qsc_extension_nodes(&self) -> &[LDom2Node] {
        &self.qsc_extension_nodes
    }

    pub fn qsc_extra_data(&self, source: &str) -> Option<&[u8]> {
        self.qsc_extra_data.get(source).map(Vec::as_slice)
    }

    pub fn qsc_extra_data_map(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.qsc_extra_data
    }

    pub fn replace_qsc_extensions(
        &mut self,
        nodes: Vec<LDom2Node>,
        extra_data: BTreeMap<String, Vec<u8>>,
    ) {
        self.qsc_extension_nodes = nodes;
        self.qsc_extra_data = extra_data;
        self.modified = true;
    }

    /// The object's `xformMat` transform, identity when unset. Applied to the
    /// rendered geometry (C++ propagates it onto atom positions).
    pub fn xform_matrix(&self) -> Matrix4 {
        self.xform.unwrap_or(Matrix4::IDENTITY)
    }

    pub fn set_xform_matrix(&mut self, matrix: Matrix4) {
        self.xform = if matrix == Matrix4::IDENTITY {
            None
        } else {
            Some(matrix)
        };
        self.modified = true;
    }

    /// Whether this object is visible (drawn) — C++ `Object::isVisible`. An
    /// invisible object contributes no geometry regardless of its renderers'
    /// own visibility (the higher-level gate of `Scene.cpp:635`).
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        self.modified = true;
    }

    pub fn renderer_by_name(&self, name: &str) -> Option<&MolRenderer> {
        self.renderers.iter().find(|renderer| renderer.name == name)
    }

    pub fn renderer(&self, uid: Uid) -> Option<&MolRenderer> {
        self.renderers.iter().find(|renderer| renderer.uid == uid)
    }

    pub fn renderer_mut(&mut self, uid: Uid) -> Option<&mut MolRenderer> {
        self.modified = true;
        self.renderers
            .iter_mut()
            .find(|renderer| renderer.uid == uid)
    }

    pub fn destroy_renderer(&mut self, uid: Uid) -> bool {
        let before = self.renderers.len();
        self.renderers.retain(|renderer| renderer.uid != uid);
        let changed = self.renderers.len() != before;
        self.modified |= changed;
        changed
    }

    /// Replace a renderer with the same serialized state and a new runtime UID.
    /// CueMol2 implements renderer type changes by serializing the old renderer
    /// with a type override, deleting it, and attaching the deserialized result.
    pub fn change_renderer_type(
        &mut self,
        uid: Uid,
        renderer_type: impl Into<String>,
    ) -> Option<Uid> {
        let index = self
            .renderers
            .iter()
            .position(|renderer| renderer.uid == uid)?;
        let renderer_type = renderer_type.into();
        if self.renderers[index].renderer_type == renderer_type {
            return Some(uid);
        }
        let mut renderer = self.renderers.remove(index);
        let new_uid = Uid::new(self.next_renderer_uid);
        self.next_renderer_uid += 1;
        renderer.uid = new_uid;
        renderer.renderer_type = renderer_type;
        self.renderers.insert(index, renderer);
        self.modified = true;
        Some(new_uid)
    }

    pub fn add_renderer(
        &mut self,
        renderer_type: impl Into<String>,
        name: impl Into<String>,
    ) -> Uid {
        self.add_renderer_with_attrs(renderer_type, name, BTreeMap::new())
    }

    fn add_renderer_with_attrs(
        &mut self,
        renderer_type: impl Into<String>,
        name: impl Into<String>,
        mut qsc_attrs: BTreeMap<String, String>,
    ) -> Uid {
        let uid = Uid::new(self.next_renderer_uid);
        self.next_renderer_uid += 1;
        qsc_attrs
            .entry("ui_order".to_string())
            .or_insert_with(|| (uid.get() as i32).to_string());
        self.modified = true;
        self.renderers.push(MolRenderer {
            uid,
            name: name.into(),
            renderer_type: renderer_type.into(),
            qsc_attrs,
            qsc_children: Vec::new(),
            atom_intr_lines: Vec::new(),
            selection: Selector::All,
            coloring: SolidColoring::new(),
            default_color: SolidColor::from_rgb(1.0, 1.0, 1.0),
        });
        uid
    }

    pub fn add_renderer_with_uid(
        &mut self,
        uid: Uid,
        renderer_type: impl Into<String>,
        name: impl Into<String>,
    ) -> Option<Uid> {
        if uid.get() == 0 || self.renderers.iter().any(|renderer| renderer.uid == uid) {
            return None;
        }
        self.next_renderer_uid = self.next_renderer_uid.max(uid.get().saturating_add(1));
        let mut qsc_attrs = BTreeMap::new();
        qsc_attrs.insert("ui_order".to_string(), (uid.get() as i32).to_string());
        self.modified = true;
        self.renderers.push(MolRenderer {
            uid,
            name: name.into(),
            renderer_type: renderer_type.into(),
            qsc_attrs,
            qsc_children: Vec::new(),
            atom_intr_lines: Vec::new(),
            selection: Selector::All,
            coloring: SolidColoring::new(),
            default_color: SolidColor::from_rgb(1.0, 1.0, 1.0),
        });
        Some(uid)
    }

    pub fn rend_uid_list(&self) -> String {
        self.renderers
            .iter()
            .map(|renderer| renderer.uid().get().to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn force_embed(&mut self) {
        self.source = "datachunk:".to_string();
        self.alt_source.clear();
        self.modified = true;
    }

    pub fn create_preset_renderer_with_style(
        &mut self,
        styles: &StyleManager,
        scope_id: u64,
        preset_name: &str,
        group_name: &str,
        name_prefix: &str,
    ) -> Result<Uid> {
        let node = styles
            .style_node(scope_id, preset_name)
            .ok_or_else(|| CueMolError::NotFound(format!("renderer preset {preset_name}")))?;
        if !node.type_name().ends_with("-rendpreset") {
            return Err(CueMolError::invalid_argument(format!(
                "{preset_name} is not a renderer preset ({})",
                node.type_name()
            )));
        }
        let group_uid = self.add_renderer("*group", group_name);
        for child in node
            .children()
            .iter()
            .filter(|child| child.tag_name() == "renderer")
        {
            let renderer_type = child.type_name();
            if renderer_type.is_empty() {
                continue;
            }
            let mut attributes = child
                .children()
                .iter()
                .filter(|attribute| attribute.is_attr())
                .map(|attribute| {
                    (
                        attribute.tag_name().to_string(),
                        attribute.value().to_string(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            attributes.insert("group".to_string(), group_name.to_string());
            let name = format!("{name_prefix}{renderer_type}");
            let uid = self.add_renderer_with_attrs(renderer_type, name, attributes);
            if let Some(renderer) = self.renderer_mut(uid) {
                renderer.qsc_children = child
                    .children()
                    .iter()
                    .filter(|node| !node.is_attr())
                    .cloned()
                    .collect();
            }
        }
        Ok(group_uid)
    }

    pub fn detach_renderer(&mut self, uid: Uid) -> Option<MolRenderer> {
        let index = self
            .renderers
            .iter()
            .position(|renderer| renderer.uid == uid)?;
        self.modified = true;
        Some(self.renderers.remove(index))
    }

    pub fn attach_renderer(&mut self, renderer: MolRenderer) -> bool {
        if self
            .renderers
            .iter()
            .any(|existing| existing.uid == renderer.uid)
        {
            return false;
        }
        self.next_renderer_uid = self.next_renderer_uid.max(renderer.uid.get() + 1);
        self.renderers.push(renderer);
        self.modified = true;
        true
    }

    /// Attach a detached renderer copy with a fresh runtime UID and a unique name.
    /// The destination group replaces the source renderer's original group.
    pub fn attach_renderer_copy(
        &mut self,
        renderer: &MolRenderer,
        destination_group: Option<&str>,
    ) -> Uid {
        let mut renderer = renderer.clone();
        let uid = Uid::new(self.next_renderer_uid);
        self.next_renderer_uid += 1;
        renderer.uid = uid;
        let base_name = renderer.name.clone();
        if self
            .renderers
            .iter()
            .any(|existing| existing.name == base_name)
        {
            for index in 1_u64.. {
                let candidate = format!("{base_name}({index})");
                if self
                    .renderers
                    .iter()
                    .all(|existing| existing.name != candidate)
                {
                    renderer.name = candidate;
                    break;
                }
            }
        }
        if let Some(group) = destination_group.filter(|group| !group.is_empty()) {
            renderer.set_qsc_attr("group", group);
        } else {
            renderer.remove_qsc_attr("group");
        }
        renderer.set_qsc_attr("ui_order", uid.get().to_string());
        self.renderers.push(renderer);
        self.modified = true;
        uid
    }

    pub fn add_atom_intr_renderer(
        &mut self,
        name: impl Into<String>,
        aids: &[(String, String)],
    ) -> Result<Uid> {
        let defs = aids
            .iter()
            .map(|(aid1, aid2)| (AtomIntrKind::Distance, vec![aid1.clone(), aid2.clone()]))
            .collect::<Vec<_>>();
        self.add_atom_intr_renderer_defs_with_attrs(name, &defs, BTreeMap::new())
    }

    pub fn add_atom_intr_renderer_defs(
        &mut self,
        name: impl Into<String>,
        defs: &[(AtomIntrKind, Vec<String>)],
    ) -> Result<Uid> {
        self.add_atom_intr_renderer_defs_with_attrs(name, defs, BTreeMap::new())
    }

    fn add_atom_intr_renderer_defs_with_attrs(
        &mut self,
        name: impl Into<String>,
        defs: &[(AtomIntrKind, Vec<String>)],
        qsc_attrs: BTreeMap<String, String>,
    ) -> Result<Uid> {
        let defs = defs
            .iter()
            .map(|(kind, aids)| PendingAtomIntrDef::Atoms(*kind, aids.clone()))
            .collect::<Vec<_>>();
        self.add_pending_atom_intr_renderer(name, &defs, qsc_attrs)
    }

    fn add_pending_atom_intr_renderer(
        &mut self,
        name: impl Into<String>,
        defs: &[PendingAtomIntrDef],
        mut qsc_attrs: BTreeMap<String, String>,
    ) -> Result<Uid> {
        let mut lines = Vec::new();
        for def in defs {
            match def {
                PendingAtomIntrDef::Atoms(kind, aids) => {
                    if aids.len() != kind.atom_count() {
                        return Err(CueMolError::invalid_argument(format!(
                            "AtomIntr {} requires {} atoms",
                            kind.tag(),
                            kind.atom_count()
                        )));
                    }
                    let labels = aids
                        .iter()
                        .map(|aid| self.resolve_atom_label(aid))
                        .collect::<Result<Vec<_>>>()?;
                    lines.push(AtomIntrLine::from_atom_references(
                        *kind,
                        aids.clone(),
                        labels,
                    ));
                }
                PendingAtomIntrDef::Selections(kind, selections) => {
                    if selections.len() != kind.atom_count() {
                        return Err(CueMolError::invalid_argument(format!(
                            "AtomIntr {} requires {} selections",
                            kind.tag(),
                            kind.atom_count()
                        )));
                    }
                    for selection in selections {
                        crate::parse_selector(selection)?;
                    }
                    lines.push(AtomIntrLine::from_selections(*kind, selections.clone()));
                }
                PendingAtomIntrDef::Positions(kind, positions) => {
                    if positions.len() != kind.atom_count() {
                        return Err(CueMolError::invalid_argument(format!(
                            "AtomIntr {} requires {} positions",
                            kind.tag(),
                            kind.atom_count()
                        )));
                    }
                    lines.push(AtomIntrLine::from_positions(*kind, positions.clone()));
                }
            }
        }
        let uid = Uid::new(self.next_renderer_uid);
        self.next_renderer_uid += 1;
        qsc_attrs
            .entry("ui_order".to_string())
            .or_insert_with(|| (uid.get() as i32).to_string());
        self.renderers.push(MolRenderer {
            uid,
            name: name.into(),
            renderer_type: "atomintr".to_string(),
            qsc_attrs,
            qsc_children: Vec::new(),
            atom_intr_lines: lines,
            selection: Selector::All,
            coloring: SolidColoring::new(),
            default_color: SolidColor::from_rgb(1.0, 1.0, 1.0),
        });
        Ok(uid)
    }

    pub fn add_atom_intr_position_renderer_defs(
        &mut self,
        name: impl Into<String>,
        defs: &[(AtomIntrKind, Vec<Vector4>)],
    ) -> Result<Uid> {
        let defs = defs
            .iter()
            .map(|(kind, positions)| PendingAtomIntrDef::Positions(*kind, positions.clone()))
            .collect::<Vec<_>>();
        self.add_pending_atom_intr_renderer(name, &defs, BTreeMap::new())
    }

    /// Resolve an atom-interaction reference (`aid`) to an atom. The reference is
    /// either a numeric index into the atom array or a `chain.resid.name` triple.
    fn resolve_atom(&self, aid: &str) -> Option<&Atom> {
        if let Ok(index) = aid.parse::<usize>() {
            self.mol.atoms().get(index)
        } else {
            let parts = aid.split('.').collect::<Vec<_>>();
            if parts.len() != 3 {
                return None;
            }
            let chain = parts[0];
            let resid = parts[1].parse::<i32>().ok();
            let name = parts[2];
            self.mol.atoms().iter().find(|atom| {
                atom.chain_id == chain
                    && Some(atom.res_id) == resid
                    && atom.name.trim().eq_ignore_ascii_case(name)
            })
        }
    }

    fn resolve_atom_label(&self, aid: &str) -> Result<String> {
        self.resolve_atom(aid)
            .map(atom_display_name)
            .ok_or_else(|| CueMolError::NotFound(format!("atom id {aid}")))
    }

    fn atom_intr_points(&self, line: &AtomIntrLine) -> Option<Vec<Vector4>> {
        if !line.active {
            return None;
        }
        if !line.positions.is_empty() {
            return Some(line.positions.clone());
        }
        if !line.selectors.is_empty() {
            return line
                .selectors
                .iter()
                .map(|selection| {
                    let selector = crate::parse_selector(selection).ok()?;
                    let indices = self.mol.select_indices(&selector);
                    let count = indices.len();
                    (count > 0).then(|| {
                        indices
                            .into_iter()
                            .map(|index| self.mol.atoms()[index].position)
                            .fold(Vector4::ZERO, |sum, position| sum + position)
                            / count as f64
                    })
                })
                .collect();
        }
        line.aids
            .iter()
            .map(|aid| self.resolve_atom(aid).map(|atom| atom.position))
            .collect()
    }

    /// Distance (Å) measured by each distance definition, in declaration order.
    /// Angle and torsion definitions return `None`; use [`Self::atom_intr_values`]
    /// to evaluate all three CueMol measurement modes.
    pub fn atom_intr_distances(&self, renderer: &MolRenderer) -> Vec<Option<f64>> {
        renderer
            .atom_intr_lines
            .iter()
            .map(|line| {
                if line.kind != AtomIntrKind::Distance {
                    return None;
                }
                let points = self.atom_intr_points(line)?;
                let p0 = points[0];
                let p1 = points[1];
                Some((p0 - p1).length3d())
            })
            .collect()
    }

    /// Distance (Å), angle (degrees), or signed torsion (degrees) for every
    /// AtomIntr definition. Invalid atom references or degenerate geometry return
    /// `None`, matching the fallible geometry operations used by CueMol.
    pub fn atom_intr_values(&self, renderer: &MolRenderer) -> Vec<Option<f64>> {
        renderer
            .atom_intr_lines
            .iter()
            .map(|line| {
                let points = self.atom_intr_points(line)?;
                match line.kind {
                    AtomIntrKind::Distance => Some((points[0] - points[1]).length3d()),
                    AtomIntrKind::Angle => points[0]
                        .sub_vec(points[1])
                        .angle(points[2].sub_vec(points[1]))
                        .ok()
                        .map(f64::to_degrees),
                    AtomIntrKind::Torsion => {
                        Vector4::torsion(points[0], points[1], points[2], points[3])
                            .ok()
                            .map(f64::to_degrees)
                    }
                }
            })
            .collect()
    }

    /// Resolved 3D endpoints of each atom-interaction line (in line order),
    /// skipping any line whose atoms cannot be resolved. This is the single
    /// source of truth both the desktop GUIs and the CLI use for measurement
    /// geometry, so an `atomintr` renderer draws its lines rather than the whole
    /// molecule.
    pub fn atom_intr_segments(&self, renderer: &MolRenderer) -> Vec<(Vector4, Vector4)> {
        renderer
            .atom_intr_lines
            .iter()
            .flat_map(|line| {
                let points = self.atom_intr_points(line);
                points
                    .map(|points| {
                        points
                            .windows(2)
                            .map(|pair| (pair[0], pair[1]))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Screen-label anchors and formatted values for each AtomIntr definition.
    /// Positions match C++ `renderDistLabel`, `renderAngleLabel`, and
    /// `renderTorsionLabel`; the GUI projects these anchors after object and
    /// renderer transforms.
    pub fn atom_intr_labels(&self, renderer: &MolRenderer) -> Vec<(Vector4, String)> {
        renderer
            .atom_intr_lines
            .iter()
            .filter_map(|line| {
                let points = self.atom_intr_points(line)?;
                match line.kind {
                    AtomIntrKind::Distance => {
                        let value = (points[0] - points[1]).length3d();
                        Some(((points[0] + points[1]) * 0.5, format!("{value:.2}")))
                    }
                    AtomIntrKind::Angle => {
                        let lhs = points[0] - points[1];
                        let rhs = points[2] - points[1];
                        let value = lhs.angle(rhs).ok()?.to_degrees();
                        let separation = (lhs.length3d() + rhs.length3d()) * 0.5;
                        let direction = ((points[0] + points[2]) * 0.5 - points[1])
                            .normalize3d()
                            .ok()?;
                        let position = points[1] + direction * (separation * 0.2);
                        Some((position, format!("{value:.2}")))
                    }
                    AtomIntrKind::Torsion => {
                        let value = Vector4::torsion(points[0], points[1], points[2], points[3])
                            .ok()?
                            .to_degrees();
                        Some(((points[1] + points[2]) * 0.5, format!("{value:.2}")))
                    }
                }
            })
            .collect()
    }

    /// Atom positions and text held by a `*namelabel` renderer. CueMol's QSC
    /// format normally stores only `aid`; the optional `text` attribute keeps
    /// labels imported from PyMOL while remaining readable by CueMol2.
    pub fn name_label_entries(&self, renderer: &MolRenderer) -> Vec<(Vector4, String)> {
        renderer
            .qsc_children()
            .iter()
            .filter(|child| child.tag_name() == "label")
            .filter_map(|child| {
                let aid = child.str_attr("aid").to_string();
                let atom = self.resolve_atom(&aid)?;
                let text = child.str_attr("text").to_string();
                let text = if text.is_empty() {
                    let mut residue = atom.res_id.to_string();
                    if let Some(insertion) = atom.ins_code {
                        residue.push(insertion);
                    }
                    format!(
                        "{} {}{} {}",
                        atom.chain_id,
                        atom.res_name,
                        residue,
                        atom.name.trim()
                    )
                } else {
                    text
                };
                Some((atom.position, text))
            })
            .collect()
    }

    /// JSON array of `{a0, a1, distance}` for an atom-interaction renderer, where
    /// `distance` is the measured value (or `null` when unresolved).
    pub fn atom_intr_measurements_json(&self, renderer: &MolRenderer) -> String {
        let values = self.atom_intr_values(renderer);
        let entries = renderer
            .atom_intr_lines
            .iter()
            .zip(values)
            .map(|(line, value)| {
                let value = match value {
                    Some(value) => format!("{value:.3}"),
                    None => "null".to_string(),
                };
                let labels = [&line.a0, &line.a1]
                    .into_iter()
                    .map(Some)
                    .chain([line.a2.as_ref(), line.a3.as_ref()])
                    .take(line.kind.atom_count())
                    .enumerate()
                    .filter_map(|(index, label)| {
                        label.map(|label| format!("\"a{index}\":\"{}\"", escape_json(label)))
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let value_name = match line.kind {
                    AtomIntrKind::Distance => "distance",
                    AtomIntrKind::Angle => "angle",
                    AtomIntrKind::Torsion => "torsion",
                };
                format!("{{{labels},\"{value_name}\":{value}}}")
            })
            .collect::<Vec<_>>();
        format!("[{}]", entries.join(","))
    }
}

/// A surface object captured from a `.qsc` load. The data is either OBJ/PLY text
/// or CueMol's textual QDF1 envelope (`qdfsurf`); a front end converts it to
/// geometry through the reader selected by `srctype`.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedSurface {
    pub uid: Uid,
    pub name: String,
    pub object_type: String,
    pub srctype: String,
    pub data: String,
    pub raw_data: Vec<u8>,
    pub qsc_node: Option<LDom2Node>,
}

/// A native density-map object captured from a `.qsc` QDF data chunk. Parsing
/// is deferred to the desktop layer to keep the molecule crate independent of
/// the crystallography crate.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedDensityMap {
    pub name: String,
    pub object_type: String,
    pub srctype: String,
    pub data: String,
    pub raw_data: Vec<u8>,
    pub qsc_node: Option<LDom2Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedLWRenderer {
    pub name: String,
    pub data_id: i32,
    pub visible: bool,
    pub xform: Matrix4,
}

/// A lightweight object captured from a `.qsc` QDF data chunk. The typed LWO
/// payload is parsed by `cuemol-lwview`, outside this molecule-only crate.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedLWObject {
    pub uid: Uid,
    pub name: String,
    pub srctype: String,
    pub raw_data: Vec<u8>,
    pub visible: bool,
    pub xform: Matrix4,
    pub renderers: Vec<LoadedLWRenderer>,
    pub qsc_node: Option<LDom2Node>,
}

/// An object class not yet interpreted by this crate. The original DOM and its
/// resolved source bytes are retained so a QSC load/save cycle is lossless.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedQscObject {
    pub name: String,
    pub object_type: String,
    pub node: LDom2Node,
    pub raw_data: Option<Vec<u8>>,
}

/// A scene-local CueMol style sheet captured from a QSC `<styles>` element.
/// `node` preserves source metadata while `data` contains the resolved XML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedStyle {
    pub node: LDom2Node,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MolScene {
    uid: Uid,
    name: String,
    source: String,
    source_type: String,
    base_path: String,
    objects: Vec<MolObject>,
    cameras: Vec<Camera>,
    next_object_uid: u64,
    /// `(name, type)` of `.qsc` objects skipped on load because their class or source
    /// format is unsupported. Supported surface and density-map objects are captured
    /// separately. Recorded so a partial load is graceful but not silent.
    skipped_qsc_objects: Vec<(String, String)>,
    /// Surface objects captured from the last `.qsc` load (mesh text + srctype). A
    /// front end turns these into displayable meshes; distinct from `skipped` because
    /// these ARE representable (an OBJ/PLY mesh), just not molecules.
    loaded_surfaces: Vec<LoadedSurface>,
    loaded_density_maps: Vec<LoadedDensityMap>,
    loaded_lw_objects: Vec<LoadedLWObject>,
    loaded_qsc_objects: Vec<LoadedQscObject>,
    loaded_styles: Vec<LoadedStyle>,
    scene_attributes: BTreeMap<String, String>,
    active_object_uid: Option<Uid>,
    active_renderer_uid: Option<Uid>,
    active_view_uid: Option<Uid>,
    scene_extension_nodes: Vec<LDom2Node>,
    /// Persistent SceneXMLWriter options from `<qsc_opts>`.
    qsc_options: Option<LDom2Node>,
    /// The persistent CueMol `<animation>` subtree. It is kept as LDOM2 so motion
    /// classes unknown to this crate survive a load/save cycle.
    animation_node: Option<LDom2Node>,
}

impl MolScene {
    pub fn first_object_uid(scene_uid: Uid) -> Uid {
        Uid::new(OBJECT_UID_BASE + scene_uid.get() * UID_SCENE_STRIDE + 1)
    }

    pub fn next_object_uid_hint(&self) -> Uid {
        Uid::new(self.next_object_uid)
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
    }

    pub fn source_type(&self) -> &str {
        &self.source_type
    }

    pub fn set_source_type(&mut self, source_type: impl Into<String>) {
        self.source_type = source_type.into();
    }

    pub fn base_path(&self) -> &str {
        &self.base_path
    }

    pub fn set_base_path(&mut self, base_path: impl Into<String>) {
        self.base_path = base_path.into();
    }

    pub fn objects(&self) -> &[MolObject] {
        &self.objects
    }

    /// `(name, type)` of objects skipped during the last `.qsc` load because their
    /// class or source format is unsupported. Lets a front end report partial loads.
    pub fn skipped_qsc_objects(&self) -> &[(String, String)] {
        &self.skipped_qsc_objects
    }

    /// Surface objects captured from the last `.qsc` load (OBJ/PLY mesh text + srctype).
    /// A front end converts each to a displayable mesh via the OBJ/PLY readers. Empty
    /// for a scene with no surface objects.
    pub fn loaded_surfaces(&self) -> &[LoadedSurface] {
        &self.loaded_surfaces
    }

    pub fn loaded_surfaces_mut(&mut self) -> &mut [LoadedSurface] {
        &mut self.loaded_surfaces
    }

    pub fn loaded_surface(&self, uid: Uid) -> Option<&LoadedSurface> {
        self.loaded_surfaces
            .iter()
            .find(|surface| surface.uid == uid)
    }

    pub fn loaded_surface_mut(&mut self, uid: Uid) -> Option<&mut LoadedSurface> {
        self.loaded_surfaces
            .iter_mut()
            .find(|surface| surface.uid == uid)
    }

    pub fn remove_loaded_surface(&mut self, uid: Uid) -> bool {
        let previous = self.loaded_surfaces.len();
        self.loaded_surfaces.retain(|surface| surface.uid != uid);
        previous != self.loaded_surfaces.len()
    }

    pub fn loaded_density_maps(&self) -> &[LoadedDensityMap] {
        &self.loaded_density_maps
    }

    pub fn loaded_density_maps_mut(&mut self) -> &mut [LoadedDensityMap] {
        &mut self.loaded_density_maps
    }

    pub fn remove_loaded_density_map(&mut self, index: usize) -> bool {
        if index >= self.loaded_density_maps.len() {
            return false;
        }
        self.loaded_density_maps.remove(index);
        true
    }

    pub fn loaded_lw_objects(&self) -> &[LoadedLWObject] {
        &self.loaded_lw_objects
    }

    pub fn loaded_lw_objects_mut(&mut self) -> &mut [LoadedLWObject] {
        &mut self.loaded_lw_objects
    }

    pub fn remove_loaded_lw_object(&mut self, uid: Uid) -> bool {
        let previous = self.loaded_lw_objects.len();
        self.loaded_lw_objects.retain(|object| object.uid != uid);
        previous != self.loaded_lw_objects.len()
    }

    pub fn loaded_qsc_objects(&self) -> &[LoadedQscObject] {
        &self.loaded_qsc_objects
    }

    pub fn loaded_styles(&self) -> &[LoadedStyle] {
        &self.loaded_styles
    }

    pub fn loaded_styles_mut(&mut self) -> &mut Vec<LoadedStyle> {
        &mut self.loaded_styles
    }

    pub fn add_loaded_style(&mut self, style: LoadedStyle) -> usize {
        self.loaded_styles.push(style);
        self.loaded_styles.len() - 1
    }

    pub fn remove_loaded_style(&mut self, index: usize) -> Option<LoadedStyle> {
        (index < self.loaded_styles.len()).then(|| self.loaded_styles.remove(index))
    }

    pub fn scene_attributes(&self) -> &BTreeMap<String, String> {
        &self.scene_attributes
    }

    pub fn replace_scene_attributes(&mut self, attributes: BTreeMap<String, String>) {
        self.scene_attributes = attributes;
    }

    pub fn scene_attribute(&self, name: &str) -> Option<&str> {
        self.scene_attributes.get(name).map(String::as_str)
    }

    pub fn set_scene_attribute(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.scene_attributes.insert(name.into(), value.into());
    }

    pub fn active_object_uid(&self) -> Option<Uid> {
        self.active_object_uid
    }

    pub fn set_active_object_uid(&mut self, uid: Option<Uid>) {
        self.active_object_uid = uid;
    }

    pub fn active_renderer_uid(&self) -> Option<Uid> {
        self.active_renderer_uid
    }

    pub fn set_active_renderer_uid(&mut self, uid: Option<Uid>) {
        self.active_renderer_uid = uid;
    }

    pub fn active_view_uid(&self) -> Option<Uid> {
        self.active_view_uid
    }

    pub fn set_active_view_uid(&mut self, uid: Option<Uid>) {
        self.active_view_uid = uid;
    }

    pub fn remove_scene_attribute(&mut self, name: &str) -> bool {
        self.scene_attributes.remove(name).is_some()
    }

    pub fn scene_extension_nodes(&self) -> &[LDom2Node] {
        &self.scene_extension_nodes
    }

    pub fn animation_node(&self) -> Option<&LDom2Node> {
        self.animation_node.as_ref()
    }

    pub fn qsc_options(&self) -> Option<&LDom2Node> {
        self.qsc_options.as_ref()
    }

    pub fn set_qsc_options(&mut self, node: Option<LDom2Node>) -> Result<()> {
        if let Some(node) = node.as_ref() {
            if node.tag_name() != "qsc_opts" {
                return Err(CueMolError::invalid_argument(format!(
                    "expected qsc_opts node, got {}",
                    node.tag_name()
                )));
            }
        }
        self.qsc_options = node;
        Ok(())
    }

    pub fn set_animation_node(&mut self, node: Option<LDom2Node>) -> Result<()> {
        if let Some(node) = node.as_ref() {
            if node.tag_name() != "animation" {
                return Err(CueMolError::invalid_argument(format!(
                    "expected animation node, got {}",
                    node.tag_name()
                )));
            }
        }
        self.animation_node = node;
        Ok(())
    }

    pub fn add_loaded_surface(&mut self, mut surface: LoadedSurface) -> Uid {
        if surface.uid.get() == 0
            || self
                .loaded_surfaces
                .iter()
                .any(|entry| entry.uid == surface.uid)
            || self.objects.iter().any(|entry| entry.uid() == surface.uid)
        {
            surface.uid = Uid::new(self.next_object_uid);
            self.next_object_uid += 1;
        } else {
            self.next_object_uid = self
                .next_object_uid
                .max(surface.uid.get().saturating_add(1));
        }
        let uid = surface.uid;
        self.loaded_surfaces.push(surface);
        uid
    }

    pub fn add_loaded_density_map(&mut self, map: LoadedDensityMap) {
        self.loaded_density_maps.push(map);
    }

    pub fn add_loaded_lw_object(&mut self, mut object: LoadedLWObject) -> Uid {
        if object.uid.get() == 0
            || self
                .loaded_lw_objects
                .iter()
                .any(|entry| entry.uid == object.uid)
            || self.objects.iter().any(|entry| entry.uid() == object.uid)
            || self
                .loaded_surfaces
                .iter()
                .any(|entry| entry.uid == object.uid)
        {
            object.uid = Uid::new(self.next_object_uid);
            self.next_object_uid += 1;
        } else {
            self.next_object_uid = self.next_object_uid.max(object.uid.get().saturating_add(1));
        }
        let uid = object.uid;
        self.loaded_lw_objects.push(object);
        uid
    }

    pub fn cameras(&self) -> &[Camera] {
        &self.cameras
    }

    pub fn replace_cameras(&mut self, cameras: Vec<Camera>) {
        self.cameras = cameras;
    }

    pub fn camera_by_name(&self, name: &str) -> Option<&Camera> {
        self.cameras.iter().find(|camera| camera.name() == name)
    }

    pub fn set_camera(&mut self, camera: Camera) {
        if let Some(existing) = self
            .cameras
            .iter_mut()
            .find(|existing| existing.name() == camera.name())
        {
            *existing = camera;
        } else {
            self.cameras.push(camera);
        }
    }

    pub fn destroy_camera(&mut self, name: &str) -> bool {
        let previous = self.cameras.len();
        self.cameras.retain(|camera| camera.name() != name);
        previous != self.cameras.len()
    }

    pub fn object_by_name(&self, name: &str) -> Option<&MolObject> {
        self.objects.iter().find(|object| object.name == name)
    }

    pub fn object(&self, uid: Uid) -> Option<&MolObject> {
        self.objects.iter().find(|object| object.uid == uid)
    }

    pub fn object_mut(&mut self, uid: Uid) -> Option<&mut MolObject> {
        self.objects.iter_mut().find(|object| object.uid == uid)
    }

    pub fn destroy_object(&mut self, uid: Uid) -> bool {
        let previous = self.objects.len();
        self.objects.retain(|object| object.uid != uid);
        let changed = previous != self.objects.len();
        if changed && self.active_object_uid == Some(uid) {
            self.active_object_uid = None;
            self.active_renderer_uid = None;
        }
        changed
    }

    pub fn clear_all_data(&mut self) {
        self.objects.clear();
        self.next_object_uid = OBJECT_UID_BASE + self.uid.get() * UID_SCENE_STRIDE + 1;
        self.loaded_surfaces.clear();
        self.loaded_density_maps.clear();
        self.loaded_lw_objects.clear();
        self.loaded_qsc_objects.clear();
        self.loaded_styles.clear();
        self.cameras.clear();
        self.skipped_qsc_objects.clear();
        self.scene_attributes.clear();
        self.active_object_uid = None;
        self.active_renderer_uid = None;
        self.scene_extension_nodes.clear();
        self.qsc_options = None;
        self.animation_node = None;
    }

    pub fn load_object(
        &mut self,
        file_path: impl AsRef<Path>,
        object_name: impl Into<String>,
        file_format: &str,
    ) -> Result<Uid> {
        let file_path = file_path.as_ref();
        let mol = match file_format {
            "pdb" => read_pdb_file(file_path)?,
            "qdfpdb" => read_qdfpdb_file(file_path)?,
            "qdfmol" => crate::QdfMolFileReader::new().read_file(file_path)?,
            "mmcif" | "cif" => read_mmcif_file(file_path)?,
            "sdf" | "mol" => read_sdf_file(file_path)?,
            "mol2" => read_mol2_file(file_path)?,
            other => {
                return Err(CueMolError::invalid_argument(format!(
                    "unknown format {other}"
                )))
            }
        };
        let uid = self.add_molecule_with_source_type(object_name, mol, file_format);
        if let Some(object) = self.object_mut(uid) {
            object.set_source(file_path.to_string_lossy());
            object.set_modified(false);
        }
        Ok(uid)
    }

    pub fn add_molecule(&mut self, name: impl Into<String>, mol: MolCoord) -> Uid {
        self.add_molecule_with_source_type(name, mol, "qdfpdb")
    }

    pub fn add_molecule_with_uid(
        &mut self,
        uid: Uid,
        name: impl Into<String>,
        mol: MolCoord,
    ) -> Option<Uid> {
        if uid.get() == 0
            || self.objects.iter().any(|object| object.uid() == uid)
            || self
                .loaded_surfaces
                .iter()
                .any(|surface| surface.uid == uid)
            || self
                .loaded_lw_objects
                .iter()
                .any(|object| object.uid == uid)
        {
            return None;
        }
        self.next_object_uid = self.next_object_uid.max(uid.get().saturating_add(1));
        Some(self.insert_molecule(uid, name, mol, "qdfpdb"))
    }

    pub fn add_molecule_with_source_type(
        &mut self,
        name: impl Into<String>,
        mol: MolCoord,
        source_type: impl Into<String>,
    ) -> Uid {
        let uid = Uid::new(self.next_object_uid);
        self.next_object_uid += 1;
        self.insert_molecule(uid, name, mol, source_type)
    }

    fn insert_molecule(
        &mut self,
        uid: Uid,
        name: impl Into<String>,
        mol: MolCoord,
        source_type: impl Into<String>,
    ) -> Uid {
        let object_base = OBJECT_UID_BASE + self.uid.get() * UID_SCENE_STRIDE;
        let object_local = uid.get().checked_sub(object_base);
        let next_renderer_uid = object_local
            .filter(|local| *local < UID_SCENE_STRIDE)
            .map_or_else(
                || uid.get().saturating_add(1),
                |local| {
                    RENDERER_UID_BASE
                        + self.uid.get() * UID_SCENE_STRIDE
                        + local * UID_OBJECT_STRIDE
                        + 1
                },
            );
        self.objects.push(MolObject {
            uid,
            name: name.into(),
            object_type: "MolCoord".to_string(),
            source_type: source_type.into(),
            source: String::new(),
            alt_source: String::new(),
            modified: false,
            ui_locked: false,
            ui_collapsed: false,
            ui_order: uid.get() as i32,
            mol,
            renderers: Vec::new(),
            next_renderer_uid,
            paint_coloring: Vec::new(),
            coloring_type: None,
            coloring_attrs: BTreeMap::new(),
            qsc_coloring_node: None,
            qsc_extra_attrs: BTreeMap::new(),
            qsc_extension_nodes: Vec::new(),
            qsc_extra_data: BTreeMap::new(),
            xform: None,
            visible: true,
        });
        uid
    }

    pub fn load_qsc_str(&mut self, input: &str) -> Result<()> {
        self.load_qsc_bytes_with_base_path(input.as_bytes(), None)
    }

    pub fn load_qsc_bytes(&mut self, input: &[u8]) -> Result<()> {
        self.load_qsc_bytes_with_base_path(input, None)
    }

    pub fn load_qsc_bytes_with_base_path(
        &mut self,
        input: &[u8],
        base_path: Option<&Path>,
    ) -> Result<()> {
        let stream = LDom2InStream::from_bytes(input)?;
        if stream.tree().top().tag_name() != "scene" {
            return Err(CueMolError::parse("QSC root element is not scene"));
        }
        let declared_base = stream.tree().top().str_attr("base_path").to_string();
        let resolved_base = (!declared_base.is_empty()).then(|| {
            let path = Path::new(&declared_base);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                base_path.unwrap_or_else(|| Path::new(".")).join(path)
            }
        });
        let effective_base = resolved_base.as_deref().or(base_path);
        let object_nodes = stream
            .tree()
            .top()
            .children()
            .iter()
            .filter(|node| node.tag_name() == "object")
            .cloned()
            .collect::<Vec<_>>();
        let camera_nodes = stream
            .tree()
            .top()
            .children()
            .iter()
            .filter(|node| node.tag_name() == "camera")
            .cloned()
            .collect::<Vec<_>>();
        let animation_node = stream
            .tree()
            .top()
            .children()
            .iter()
            .find(|node| node.tag_name() == "animation")
            .cloned();
        let qsc_options = stream
            .tree()
            .top()
            .children()
            .iter()
            .find(|node| node.tag_name() == "qsc_opts")
            .cloned();
        let loaded_styles = stream
            .tree()
            .top()
            .children()
            .iter()
            .filter(|node| node.tag_name() == "styles")
            .map(|node| {
                let data = if node.str_attr("src").is_empty() {
                    write_ldom2_xml(&LDom2Tree::from(node.clone()))
                } else {
                    let source_line = qsc_node_lines(node)
                        .into_iter()
                        .next()
                        .ok_or_else(|| CueMolError::parse("QSC styles element not found"))?;
                    String::from_utf8_lossy(&load_qsc_source_data(
                        &source_line,
                        &BTreeMap::new(),
                        effective_base,
                    )?)
                    .into_owned()
                };
                Ok(LoadedStyle {
                    node: node.clone(),
                    data,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let scene_attributes = stream
            .tree()
            .top()
            .children()
            .iter()
            .filter(|node| node.is_attr())
            .map(|node| (node.tag_name().to_string(), node.value().to_string()))
            .collect::<BTreeMap<_, _>>();
        let scene_extension_nodes = stream
            .tree()
            .top()
            .children()
            .iter()
            .filter(|node| {
                !node.is_attr()
                    && !matches!(
                        node.tag_name(),
                        "qsc_opts" | "styles" | "object" | "camera" | "animation"
                    )
            })
            .cloned()
            .collect::<Vec<_>>();

        if !object_nodes.is_empty() {
            let chunks = stream
                .chunk_ids()
                .iter()
                .map(|chunk_id| {
                    stream
                        .chunk(chunk_id.as_str())
                        .map(|data| (chunk_id.to_string(), data.to_vec()))
                })
                .collect::<Result<BTreeMap<_, _>>>()?;
            for node in object_nodes {
                self.load_qsc_object_block(&qsc_node_lines(&node), &node, &chunks, effective_base)?;
            }
        }
        for node in camera_nodes {
            self.set_camera(parse_qsc_camera_node(&node, self, effective_base)?);
        }
        self.qsc_options = qsc_options;
        self.animation_node = animation_node;
        self.loaded_styles = loaded_styles;
        self.scene_attributes = scene_attributes;
        self.scene_extension_nodes = scene_extension_nodes;
        Ok(())
    }

    fn load_qsc_object_block(
        &mut self,
        block: &[String],
        object_node: &LDom2Node,
        chunks: &BTreeMap<String, Vec<u8>>,
        base_path: Option<&Path>,
    ) -> Result<()> {
        let object_line = block
            .first()
            .ok_or_else(|| CueMolError::parse("QSC object element not found"))?;
        let object_name = attr_value(object_line, "name")
            .ok_or_else(|| CueMolError::parse("QSC object name not found"))?;
        // Object class dispatch: MolCoord chunks can be PDB text or native QDF.
        // A `.qsc` saved by C++ CueMol2 can also hold non-molecule objects such as
        // MolSurf and DensityMap. Capture supported representations and record only
        // unsupported classes or source formats as skipped.
        // A missing `type` means MolCoord (the Rust writer always emits `type="MolCoord"`).
        let object_type = attr_value(object_line, "type").unwrap_or_else(|| "MolCoord".to_string());
        if !matches!(object_type.as_str(), "MolCoord" | "MorphMol") {
            // Capture every supported surface representation. QDF1 is an ASCII
            // envelope, so it remains safe in the text datachunk map.
            let srctype = attr_value(object_line, "srctype").unwrap_or_default();
            if matches!(object_type.as_str(), "MolSurf" | "MolSurfObj")
                && matches!(srctype.as_str(), "obj" | "ply" | "qdfsurf")
            {
                if let Ok(data) = load_qsc_source_data(object_line, chunks, base_path) {
                    self.add_loaded_surface(LoadedSurface {
                        uid: Uid::new(0),
                        name: object_name.clone(),
                        object_type: object_type.clone(),
                        srctype,
                        data: String::from_utf8_lossy(&data).into_owned(),
                        raw_data: data,
                        qsc_node: Some(object_node.clone()),
                    });
                    return Ok(());
                }
            }
            if matches!(object_type.as_str(), "DensityMap" | "ElePotMap")
                && matches!(
                    srctype.as_str(),
                    "qdfmap"
                        | "ccp4map"
                        | "xplormap"
                        | "brix"
                        | "mmcifmap"
                        | "mtzmap"
                        | "apbs"
                        | "qdfpot"
                )
            {
                if let Ok(data) = load_qsc_source_data(object_line, chunks, base_path) {
                    self.loaded_density_maps.push(LoadedDensityMap {
                        name: object_name.clone(),
                        object_type: object_type.clone(),
                        srctype,
                        data: String::from_utf8_lossy(&data).into_owned(),
                        raw_data: data,
                        qsc_node: Some(object_node.clone()),
                    });
                    return Ok(());
                }
            }
            if object_type == "LWObject" && matches!(srctype.as_str(), "qdflwobj" | "qdf_lwobj") {
                if let Ok(data) = load_qsc_source_data(object_line, chunks, base_path) {
                    let visible = attr_value(object_line, "visible").is_none_or(|value| {
                        !matches!(value.as_str(), "false" | "0" | "no" | "off")
                    });
                    let renderers = block
                        .iter()
                        .map(|line| line.trim())
                        .filter(|line| line.starts_with("<renderer "))
                        .filter(|line| attr_value(line, "type").as_deref() == Some("lwrend"))
                        .map(|line| LoadedLWRenderer {
                            name: attr_value(line, "name").unwrap_or_else(|| "lwrend".to_string()),
                            data_id: attr_value(line, "data_id")
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(0),
                            visible: attr_value(line, "visible").is_none_or(|value| {
                                !matches!(value.as_str(), "false" | "0" | "no" | "off")
                            }),
                            xform: attr_value(line, "xformMat")
                                .as_deref()
                                .and_then(parse_xform_mat)
                                .unwrap_or(Matrix4::IDENTITY),
                        })
                        .collect();
                    self.add_loaded_lw_object(LoadedLWObject {
                        uid: Uid::new(0),
                        name: object_name.clone(),
                        srctype,
                        raw_data: data,
                        visible,
                        xform: attr_value(object_line, "xformMat")
                            .as_deref()
                            .and_then(parse_xform_mat)
                            .unwrap_or(Matrix4::IDENTITY),
                        renderers,
                        qsc_node: Some(object_node.clone()),
                    });
                    return Ok(());
                }
            }
            let source = attr_value(object_line, "src").unwrap_or_default();
            let raw_data = if source.is_empty() {
                None
            } else {
                load_qsc_source_data(object_line, chunks, base_path).ok()
            };
            self.loaded_qsc_objects.push(LoadedQscObject {
                name: object_name.clone(),
                object_type: object_type.clone(),
                node: object_node.clone(),
                raw_data,
            });
            self.skipped_qsc_objects.push((object_name, object_type));
            return Ok(());
        }
        let data = load_qsc_source_data(object_line, chunks, base_path)?;
        let srctype = attr_value(object_line, "srctype").unwrap_or_else(|| "qdfpdb".to_string());
        let molecule = match srctype.as_str() {
            "qdfmol" => {
                let qdf_data = if data.starts_with(b"QDF0") {
                    data.as_slice()
                } else {
                    trim_ascii_bytes(&data)
                };
                read_qdf_mol_bytes(qdf_data)?
            }
            "qdfpdb" | "pdb" | "" => {
                read_pdb_str(std::str::from_utf8(&data).map_err(|error| {
                    CueMolError::parse(format!("QSC PDB chunk is not UTF-8: {error}"))
                })?)?
            }
            "mmcif" | "cif" => read_mmcif_str(std::str::from_utf8(&data).map_err(|error| {
                CueMolError::parse(format!("QSC mmCIF source is not UTF-8: {error}"))
            })?)?,
            "sdf" | "mol" => read_sdf_str(std::str::from_utf8(&data).map_err(|error| {
                CueMolError::parse(format!("QSC SDF source is not UTF-8: {error}"))
            })?)?,
            "mol2" => read_mol2_str(std::str::from_utf8(&data).map_err(|error| {
                CueMolError::parse(format!("QSC MOL2 source is not UTF-8: {error}"))
            })?)?,
            other => {
                return Err(CueMolError::invalid_argument(format!(
                    "unsupported QSC molecule source type {other}"
                )))
            }
        };
        let object_uid = self.add_molecule_with_source_type(object_name, molecule, srctype);
        let qsc_extra_attrs = object_node
            .children()
            .iter()
            .filter(|node| {
                node.is_attr()
                    && !matches!(
                        node.tag_name(),
                        "type"
                            | "name"
                            | "src"
                            | "alt_src"
                            | "srctype"
                            | "locked"
                            | "ui_collapsed"
                            | "ui_order"
                            | "xformMat"
                            | "visible"
                    )
            })
            .map(|node| (node.tag_name().to_string(), node.value().to_string()))
            .collect::<BTreeMap<_, _>>();
        let qsc_extension_nodes = object_node
            .children()
            .iter()
            .filter(|node| !node.is_attr() && !matches!(node.tag_name(), "coloring" | "renderer"))
            .cloned()
            .collect::<Vec<_>>();
        let mut qsc_extra_data = BTreeMap::new();
        for node in &qsc_extension_nodes {
            collect_qsc_extension_data(node, chunks, base_path, &mut qsc_extra_data)?;
        }
        let object = self
            .object_mut(object_uid)
            .ok_or_else(|| CueMolError::NotFound("loaded QSC object".to_string()))?;
        object.object_type = object_type;
        object.qsc_extra_attrs = qsc_extra_attrs;
        object.qsc_extension_nodes = qsc_extension_nodes;
        object.qsc_extra_data = qsc_extra_data;
        object.set_source(attr_value(object_line, "src").unwrap_or_default());
        object.alt_source = attr_value(object_line, "alt_src").unwrap_or_default();
        object.ui_locked = attr_value(object_line, "locked")
            .is_some_and(|value| matches!(value.as_str(), "true" | "1" | "yes" | "on"));
        object.ui_collapsed = attr_value(object_line, "ui_collapsed")
            .is_some_and(|value| matches!(value.as_str(), "true" | "1" | "yes" | "on"));
        if let Some(order) =
            attr_value(object_line, "ui_order").and_then(|value| value.parse::<i32>().ok())
        {
            object.ui_order = order;
        }

        // Object-level `xformMat` (C++ `Object::xformMat`): the structure's saved
        // transform (e.g. a persisted superposition fit). Applied to the rendered
        // geometry by the render path.
        if let Some(matrix) = attr_value(object_line, "xformMat")
            .as_deref()
            .and_then(parse_xform_mat)
        {
            object.set_xform_matrix(matrix);
        }

        // Object-level `visible` (C++ `Object::visible`, default true): only a
        // hidden object carries the attribute. Mirror `MolRenderer::is_visible`'s
        // tolerance of `false`/`0`/`no`/`off` so the same false-spellings parse.
        if let Some(value) = attr_value(object_line, "visible") {
            object.set_visible(!matches!(value.as_str(), "false" | "0" | "no" | "off"));
        }

        let mut current_atomintr: Option<PendingAtomIntrRenderer> = None;
        let mut current_atomintr_attrs = BTreeMap::new();
        for line in block.iter().map(|line| line.trim()) {
            if line.starts_with("<renderer ") {
                let attrs = attr_map(line);
                let renderer_type = attr_value(line, "type").unwrap_or_default();
                let renderer_name =
                    attr_value(line, "name").unwrap_or_else(|| renderer_type.clone());
                if renderer_type == "atomintr" && !line.ends_with("/>") {
                    current_atomintr_attrs = attrs;
                    current_atomintr = Some((renderer_name, Vec::new()));
                } else if renderer_type == "atomintr" {
                    object.add_atom_intr_renderer_defs_with_attrs(renderer_name, &[], attrs)?;
                } else if !renderer_name.is_empty() {
                    object.add_renderer_with_attrs(renderer_type, renderer_name, attrs);
                }
            } else if let Some(kind) = atom_intr_kind_from_line(line) {
                if let Some((_, defs)) = current_atomintr.as_mut() {
                    let aids = (1..=kind.atom_count())
                        .map(|index| attr_value(line, &format!("aid{index}")))
                        .collect::<Option<Vec<_>>>();
                    if let Some(aids) = aids {
                        defs.push(PendingAtomIntrDef::Atoms(kind, aids));
                        continue;
                    }
                    let selections = (1..=kind.atom_count())
                        .map(|index| attr_value(line, &format!("sel{index}")))
                        .collect::<Option<Vec<_>>>();
                    if let Some(selections) = selections {
                        defs.push(PendingAtomIntrDef::Selections(kind, selections));
                        continue;
                    }
                    let positions = (1..=kind.atom_count())
                        .map(|index| {
                            let value = attr_value(line, &format!("pos{index}"))?;
                            Vector4::from_script_string(&value).ok()
                        })
                        .collect::<Option<Vec<_>>>();
                    if let Some(positions) = positions {
                        defs.push(PendingAtomIntrDef::Positions(kind, positions));
                    } else {
                        return Err(CueMolError::parse(format!(
                            "AtomIntr {} requires aid, sel, or pos attributes",
                            kind.tag()
                        )));
                    }
                }
            } else if line.starts_with("</renderer>") {
                if let Some((name, defs)) = current_atomintr.take() {
                    object.add_pending_atom_intr_renderer(
                        name,
                        &defs,
                        std::mem::take(&mut current_atomintr_attrs),
                    )?;
                }
            }
        }

        if let Some((name, defs)) = current_atomintr.take() {
            object.add_pending_atom_intr_renderer(name, &defs, current_atomintr_attrs)?;
        }

        if let Some(coloring) = object_node
            .children()
            .iter()
            .find(|node| !node.is_attr() && node.tag_name() == "coloring")
        {
            let coloring_type = if coloring.type_name().is_empty() {
                let value = coloring.str_attr("type").to_string();
                (!value.is_empty()).then_some(value)
            } else {
                Some(coloring.type_name().to_string())
            };
            let mut coloring_attrs = coloring
                .children()
                .iter()
                .filter(|node| node.is_attr() && node.tag_name() != "type")
                .map(|node| (node.tag_name().to_string(), node.value().to_string()))
                .collect::<BTreeMap<_, _>>();
            if coloring_type.as_deref() == Some("ScriptColoring") {
                if let Some(script) = coloring
                    .children()
                    .iter()
                    .find(|node| !node.is_attr() && node.tag_name() == "script")
                {
                    let source = if !script.value().is_empty() {
                        script.value()
                    } else {
                        script.contents()
                    };
                    coloring_attrs.insert("script".to_string(), source.to_string());
                }
            }
            let paint_entries = coloring
                .children()
                .iter()
                .filter(|node| !node.is_attr() && node.tag_name() == "paint")
                .filter_map(|node| {
                    let selection = node.str_attr("sel").to_string();
                    let color = node.str_attr("color").to_string();
                    (!selection.is_empty() && !color.is_empty()).then_some((selection, color))
                })
                .collect::<Vec<_>>();
            object.set_coloring(coloring_type, coloring_attrs);
            object.set_paint_coloring(paint_entries);
            object.qsc_coloring_node = Some(coloring.clone());
        }

        let renderer_nodes = object_node
            .children()
            .iter()
            .filter(|node| !node.is_attr() && node.tag_name() == "renderer");
        for node in renderer_nodes {
            let name = node.str_attr("name").to_string();
            let renderer_type = if node.type_name().is_empty() {
                node.str_attr("type").to_string()
            } else {
                node.type_name().to_string()
            };
            let renderer = object.renderers.iter_mut().find(|renderer| {
                (!name.is_empty() && renderer.name == name)
                    || (name.is_empty()
                        && !renderer_type.is_empty()
                        && renderer.renderer_type == renderer_type)
            });
            let Some(renderer) = renderer else {
                continue;
            };
            renderer.qsc_children = node
                .children()
                .iter()
                .filter(|child| {
                    !child.is_attr() && !matches!(child.tag_name(), "line" | "angle" | "torsion")
                })
                .cloned()
                .collect();
        }
        object.set_modified(false);
        Ok(())
    }

    pub fn write_qsc_string(&self) -> String {
        let mut output = String::new();
        output.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\n");
        output.push_str("<scene");
        for (name, value) in &self.scene_attributes {
            output.push_str(&format!(" {name}=\"{}\"", escape_xml(value)));
        }
        output.push_str(">\n");
        for node in &self.scene_extension_nodes {
            for line in qsc_node_lines(node) {
                output.push('\t');
                output.push_str(&line);
                output.push('\n');
            }
        }
        if let Some(options) = &self.qsc_options {
            for line in qsc_node_lines(options) {
                output.push('\t');
                output.push_str(&line);
                output.push('\n');
            }
        } else {
            output.push_str("\t<qsc_opts base64=\"false\" compress=\"none\" version=\"QDF0\"/>\n");
        }
        for style in &self.loaded_styles {
            for line in qsc_node_lines(&style.node) {
                output.push('\t');
                output.push_str(&line);
                output.push('\n');
            }
        }
        let mut next_extra_chunk = self.objects.len();
        let object_extra_chunk_ids = self
            .objects
            .iter()
            .map(|object| {
                object
                    .qsc_extra_data
                    .keys()
                    .filter(|source| source.starts_with("datachunk:"))
                    .map(|source| {
                        let chunk_id = format!("{next_extra_chunk:05}");
                        next_extra_chunk += 1;
                        (source.clone(), chunk_id)
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .collect::<Vec<_>>();
        for (index, object) in self.objects.iter().enumerate() {
            let chunk_id = format!("{index:05}");
            // Emit `xformMat` only for a transformed object (C++ omits the identity
            // default), in the column-major `(...)` format the reader parses.
            let xform_attr = if object.xform.is_some() {
                format!(" xformMat=\"{}\"", format_xform_mat(object.xform_matrix()))
            } else {
                String::new()
            };
            // C++ omits the default-true `visible` property; emit it only when the
            // object is hidden (`visible="false"`), as a `<object>` attribute —
            // qlib serializes string-convertible scalars as attributes
            // (`LDOM2Tree.cpp:172-174` `setValue`+`setAttrFlag(true)`).
            let visible_attr = if object.is_visible() {
                String::new()
            } else {
                " visible=\"false\"".to_string()
            };
            let locked_attr = if object.is_ui_locked() {
                " locked=\"true\""
            } else {
                ""
            };
            let collapsed_attr = if object.is_ui_collapsed() {
                " ui_collapsed=\"true\""
            } else {
                ""
            };
            let alt_source_attr = if object.alt_source().is_empty() {
                String::new()
            } else {
                format!(" alt_src=\"{}\"", escape_xml(object.alt_source()))
            };
            let extra_attrs = object
                .qsc_extra_attrs
                .iter()
                .map(|(name, value)| format!(" {name}=\"{}\"", escape_xml(value)))
                .collect::<String>();
            output.push_str(&format!(
                "\t<object type=\"{}\" name=\"{}\" src=\"datachunk:{}\" srctype=\"{}\"{}{}{}{}{}{}>\n",
                escape_xml(&object.object_type),
                escape_xml(object.name()),
                chunk_id,
                escape_xml(object.source_type()),
                xform_attr,
                visible_attr,
                locked_attr,
                collapsed_attr,
                alt_source_attr,
                extra_attrs
            ));
            if let Some(coloring) = object.qsc_coloring_node() {
                for line in qsc_node_lines(coloring) {
                    output.push_str("\t\t");
                    output.push_str(&line);
                    output.push('\n');
                }
            } else if let Some(coloring_type) = object.coloring_type() {
                let attrs = object
                    .coloring_attrs
                    .iter()
                    .map(|(name, value)| format!(" {name}=\"{}\"", escape_xml(value)))
                    .collect::<String>();
                if object.paint_coloring().is_empty() {
                    output.push_str(&format!(
                        "\t\t<coloring type=\"{}\"{attrs}/>\n",
                        escape_xml(coloring_type)
                    ));
                } else {
                    output.push_str(&format!(
                        "\t\t<coloring type=\"{}\"{attrs}>\n",
                        escape_xml(coloring_type)
                    ));
                    for (selection, color) in object.paint_coloring() {
                        output.push_str(&format!(
                            "\t\t\t<paint sel=\"{}\" color=\"{}\"/>\n",
                            escape_xml(selection),
                            escape_xml(color)
                        ));
                    }
                    output.push_str("\t\t</coloring>\n");
                }
            }
            for renderer in object.renderers() {
                if renderer.renderer_type() == "atomintr" {
                    output.push_str(&format!(
                        "\t\t<renderer{}>\n",
                        format_renderer_attrs(renderer, Some("DefaultLabel,DefaultAtomIntr"))
                    ));
                    for line in &renderer.atom_intr_lines {
                        if !line.active {
                            continue;
                        }
                        let attrs = if !line.selectors.is_empty() {
                            line.selectors
                                .iter()
                                .enumerate()
                                .map(|(index, selection)| {
                                    format!(" sel{}=\"{}\"", index + 1, escape_xml(selection))
                                })
                                .collect::<String>()
                        } else if line.positions.is_empty() {
                            line.aids
                                .iter()
                                .enumerate()
                                .map(|(index, aid)| {
                                    format!(" aid{}=\"{}\"", index + 1, escape_xml(aid))
                                })
                                .collect::<String>()
                        } else {
                            line.positions
                                .iter()
                                .enumerate()
                                .map(|(index, position)| {
                                    format!(
                                        " pos{}=\"{}\"",
                                        index + 1,
                                        escape_xml(&position.to_string())
                                    )
                                })
                                .collect::<String>()
                        };
                        output.push_str(&format!("\t\t\t<{}{attrs}/>\n", line.kind.tag()));
                    }
                    for child in renderer.qsc_children() {
                        for line in qsc_node_lines(child) {
                            output.push_str("\t\t\t");
                            output.push_str(&line);
                            output.push('\n');
                        }
                    }
                    output.push_str("\t\t</renderer>\n");
                } else if renderer.qsc_children().is_empty() {
                    output.push_str(&format!(
                        "\t\t<renderer{}/>\n",
                        format_renderer_attrs(renderer, None)
                    ));
                } else {
                    output.push_str(&format!(
                        "\t\t<renderer{}>\n",
                        format_renderer_attrs(renderer, None)
                    ));
                    for child in renderer.qsc_children() {
                        for line in qsc_node_lines(child) {
                            output.push_str("\t\t\t");
                            output.push_str(&line);
                            output.push('\n');
                        }
                    }
                    output.push_str("\t\t</renderer>\n");
                }
            }
            for extension in &object.qsc_extension_nodes {
                let mut extension = extension.clone();
                remap_qsc_extension_sources(
                    &mut extension,
                    object_extra_chunk_ids
                        .get(index)
                        .expect("extra chunk plan matches objects"),
                );
                append_qsc_node(&mut output, &extension, 2);
            }
            output.push_str("\t</object>\n");
        }
        let surface_base = next_extra_chunk;
        for (index, surface) in self.loaded_surfaces.iter().enumerate() {
            let chunk_id = format!("{:05}", surface_base + index);
            if let Some(source_node) = &surface.qsc_node {
                let mut node = source_node.clone();
                if node.str_attr("src").as_str().starts_with("datachunk:") {
                    node.set_str_attr("src", format!("datachunk:{chunk_id}"));
                }
                append_qsc_node(&mut output, &node, 1);
            } else {
                output.push_str(&format!(
                    "\t<object type=\"{}\" name=\"{}\" src=\"datachunk:{}\" srctype=\"{}\">\n\t</object>\n",
                    escape_xml(&surface.object_type),
                    escape_xml(&surface.name),
                    chunk_id,
                    escape_xml(&surface.srctype),
                ));
            }
        }
        let map_base = surface_base + self.loaded_surfaces.len();
        for (index, map) in self.loaded_density_maps.iter().enumerate() {
            let chunk_id = format!("{:05}", map_base + index);
            if let Some(source_node) = &map.qsc_node {
                let mut node = source_node.clone();
                if node.str_attr("src").as_str().starts_with("datachunk:") {
                    node.set_str_attr("src", format!("datachunk:{chunk_id}"));
                }
                append_qsc_node(&mut output, &node, 1);
            } else {
                output.push_str(&format!(
                    "\t<object type=\"{}\" name=\"{}\" src=\"datachunk:{}\" srctype=\"{}\">\n\t</object>\n",
                    escape_xml(&map.object_type),
                    escape_xml(&map.name),
                    chunk_id,
                    escape_xml(&map.srctype),
                ));
            }
        }
        let lw_base = map_base + self.loaded_density_maps.len();
        for (index, object) in self.loaded_lw_objects.iter().enumerate() {
            let chunk_id = format!("{:05}", lw_base + index);
            if let Some(source_node) = &object.qsc_node {
                let mut node = source_node.clone();
                if node.str_attr("src").as_str().starts_with("datachunk:") {
                    node.set_str_attr("src", format!("datachunk:{chunk_id}"));
                }
                append_qsc_node(&mut output, &node, 1);
                continue;
            }
            let visible = if object.visible {
                String::new()
            } else {
                " visible=\"false\"".to_string()
            };
            let xform = if object.xform == Matrix4::IDENTITY {
                String::new()
            } else {
                format!(" xformMat=\"{}\"", format_xform_mat(object.xform))
            };
            output.push_str(&format!(
                "\t<object type=\"LWObject\" name=\"{}\" src=\"datachunk:{}\" srctype=\"{}\"{}{}>\n",
                escape_xml(&object.name),
                chunk_id,
                escape_xml(&object.srctype),
                visible,
                xform,
            ));
            for renderer in &object.renderers {
                let visible = if renderer.visible {
                    String::new()
                } else {
                    " visible=\"false\"".to_string()
                };
                let xform = if renderer.xform == Matrix4::IDENTITY {
                    String::new()
                } else {
                    format!(" xformMat=\"{}\"", format_xform_mat(renderer.xform))
                };
                output.push_str(&format!(
                    "\t\t<renderer type=\"lwrend\" name=\"{}\" data_id=\"{}\"{}{} />\n",
                    escape_xml(&renderer.name),
                    renderer.data_id,
                    visible,
                    xform,
                ));
            }
            output.push_str("\t</object>\n");
        }
        let unknown_base = lw_base + self.loaded_lw_objects.len();
        for (index, object) in self.loaded_qsc_objects.iter().enumerate() {
            let mut node = object.node.clone();
            if object.raw_data.is_some() && node.str_attr("src").as_str().starts_with("datachunk:")
            {
                node.set_str_attr("src", format!("datachunk:{:05}", unknown_base + index));
            }
            append_qsc_node(&mut output, &node, 1);
        }
        for camera in &self.cameras {
            let source = if camera.source().is_empty() {
                String::new()
            } else {
                format!(" src=\"{}\"", escape_xml(camera.source()))
            };
            let alt_source = if camera.alt_source().is_empty() {
                String::new()
            } else {
                format!(" alt_src=\"{}\"", escape_xml(camera.alt_source()))
            };
            let visibility_nodes = camera
                .visibility_settings()
                .filter_map(|(uid, visible, is_object)| {
                    if is_object {
                        let object = self
                            .objects
                            .iter()
                            .find(|object| object.uid().get() == uid)?;
                        Some(format!(
                            "\t\t\t<object target=\"{}\" value=\"{}\"/>",
                            escape_xml(object.name()),
                            visible
                        ))
                    } else {
                        let (object, renderer) = self.objects.iter().find_map(|object| {
                            object
                                .renderers()
                                .iter()
                                .find(|renderer| renderer.uid().get() == uid)
                                .map(|renderer| (object, renderer))
                        })?;
                        Some(format!(
                            "\t\t\t<renderer target=\"{}\" type=\"{}\" client=\"{}\" value=\"{}\"/>",
                            escape_xml(renderer.name()),
                            escape_xml(renderer.renderer_type()),
                            escape_xml(object.name()),
                            visible
                        ))
                    }
                })
                .collect::<Vec<_>>();
            output.push_str(&format!(
                "\t<camera center=\"{}\" centerMark=\"{}\" distance=\"{}\" name=\"{}\" perspec=\"{}\" rotation=\"{}\" slab=\"{}\" stereoDist=\"{}\" stereoMode=\"{}\" zoom=\"{}\"{}{}",
                format_vector3(camera.center()),
                center_mark_to_qsc(camera.center_mark()),
                format_float(camera.cam_dist()),
                escape_xml(camera.name()),
                camera.is_perspective(),
                format_quat(camera.rotation()),
                format_float(camera.slab_depth()),
                format_float(camera.stereo_distance()),
                stereo_mode_to_qsc(camera.stereo_mode()),
                format_float(camera.zoom()),
                source,
                alt_source,
            ));
            if visibility_nodes.is_empty() {
                output.push_str("/>\n");
            } else {
                output.push_str(">\n\t\t<visibilities>\n");
                for node in visibility_nodes {
                    output.push_str(&node);
                    output.push('\n');
                }
                output.push_str("\t\t</visibilities>\n\t</camera>\n");
            }
        }
        if let Some(animation) = &self.animation_node {
            for line in qsc_node_lines(animation) {
                output.push('\t');
                output.push_str(&line);
                output.push('\n');
            }
        }
        output.push_str("</scene>\n\n");
        output.push_str("========== End of XML ==========\n\n");
        for (index, object) in self.objects.iter().enumerate() {
            let chunk_id = format!("{index:05}");
            if object.source_type() == "qdfmol" {
                append_qsc_data_chunk(
                    &mut output,
                    &chunk_id,
                    &crate::write_qdf_mol_bytes(object.mol()),
                );
            } else {
                output.push_str(&format!("==========START datachunk:{chunk_id}==========\n"));
                output.push_str(&write_pdb_string(object.mol()));
                output.push_str(&format!("\n==========END datachunk:{chunk_id}==========\n"));
            }
        }
        for (object, chunk_ids) in self.objects.iter().zip(&object_extra_chunk_ids) {
            for (source, chunk_id) in chunk_ids {
                if let Some(data) = object.qsc_extra_data.get(source) {
                    append_qsc_data_chunk(&mut output, chunk_id, data);
                }
            }
        }
        for (index, surface) in self.loaded_surfaces.iter().enumerate() {
            let chunk_id = format!("{:05}", surface_base + index);
            if surface
                .qsc_node
                .as_ref()
                .is_some_and(|node| !node.str_attr("src").as_str().starts_with("datachunk:"))
            {
                continue;
            }
            let data = if surface.raw_data.is_empty() {
                surface.data.as_bytes()
            } else {
                &surface.raw_data
            };
            append_qsc_data_chunk(&mut output, &chunk_id, data);
        }
        for (index, map) in self.loaded_density_maps.iter().enumerate() {
            let chunk_id = format!("{:05}", map_base + index);
            if map
                .qsc_node
                .as_ref()
                .is_some_and(|node| !node.str_attr("src").as_str().starts_with("datachunk:"))
            {
                continue;
            }
            append_qsc_data_chunk(&mut output, &chunk_id, &map.raw_data);
        }
        for (index, object) in self.loaded_lw_objects.iter().enumerate() {
            let chunk_id = format!("{:05}", lw_base + index);
            if object
                .qsc_node
                .as_ref()
                .is_some_and(|node| !node.str_attr("src").as_str().starts_with("datachunk:"))
            {
                continue;
            }
            append_qsc_data_chunk(&mut output, &chunk_id, &object.raw_data);
        }
        for (index, object) in self.loaded_qsc_objects.iter().enumerate() {
            if !object
                .node
                .str_attr("src")
                .as_str()
                .starts_with("datachunk:")
            {
                continue;
            }
            if let Some(data) = &object.raw_data {
                append_qsc_data_chunk(&mut output, &format!("{:05}", unknown_base + index), data);
            }
        }
        output
    }
}

#[derive(Debug, Default, Clone)]
pub struct MolSceneManager {
    scenes: Vec<MolScene>,
    next_scene_uid: u64,
}

impl MolSceneManager {
    pub fn new() -> Self {
        Self {
            scenes: Vec::new(),
            next_scene_uid: 1,
        }
    }

    pub fn new_scene(&mut self, name: impl Into<String>) -> Uid {
        let uid = Uid::new(self.next_scene_uid);
        self.next_scene_uid += 1;
        self.scenes.push(MolScene {
            uid,
            name: name.into(),
            source: String::new(),
            source_type: String::new(),
            base_path: String::new(),
            objects: Vec::new(),
            cameras: Vec::new(),
            next_object_uid: OBJECT_UID_BASE + uid.get() * UID_SCENE_STRIDE + 1,
            skipped_qsc_objects: Vec::new(),
            loaded_surfaces: Vec::new(),
            loaded_density_maps: Vec::new(),
            loaded_lw_objects: Vec::new(),
            loaded_qsc_objects: Vec::new(),
            loaded_styles: Vec::new(),
            scene_attributes: BTreeMap::new(),
            active_object_uid: None,
            active_renderer_uid: None,
            active_view_uid: None,
            scene_extension_nodes: Vec::new(),
            qsc_options: None,
            animation_node: None,
        });
        uid
    }

    pub fn scene(&self, uid: Uid) -> Option<&MolScene> {
        self.scenes.iter().find(|scene| scene.uid == uid)
    }

    pub fn scenes(&self) -> &[MolScene] {
        &self.scenes
    }

    pub fn scene_uids(&self) -> Vec<Uid> {
        self.scenes.iter().map(MolScene::uid).collect()
    }

    pub fn next_scene_uid_hint(&self) -> Uid {
        Uid::new(self.next_scene_uid)
    }

    pub fn scene_mut(&mut self, uid: Uid) -> Option<&mut MolScene> {
        self.scenes.iter_mut().find(|scene| scene.uid == uid)
    }

    pub fn destroy_scene(&mut self, uid: Uid) -> bool {
        let before = self.scenes.len();
        self.scenes.retain(|scene| scene.uid != uid);
        before != self.scenes.len()
    }

    pub fn object(&self, uid: Uid) -> Option<&MolObject> {
        self.scenes.iter().find_map(|scene| scene.object(uid))
    }

    pub fn object_mut(&mut self, uid: Uid) -> Option<&mut MolObject> {
        self.scenes
            .iter_mut()
            .find_map(|scene| scene.object_mut(uid))
    }

    pub fn scene_uid_for_object(&self, uid: Uid) -> Option<Uid> {
        self.scenes
            .iter()
            .find(|scene| scene.object(uid).is_some())
            .map(MolScene::uid)
    }

    pub fn scene_uid_for_surface(&self, uid: Uid) -> Option<Uid> {
        self.scenes
            .iter()
            .find(|scene| scene.loaded_surface(uid).is_some())
            .map(MolScene::uid)
    }

    pub fn destroy_all_scenes(&mut self) {
        self.scenes.clear();
    }

    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }

    pub fn load_qsc_file(&mut self, path: impl AsRef<Path>) -> Result<Uid> {
        match self.load_qsc_file_checked(path)? {
            QscLoadStatus::Loaded { scene_uid } => Ok(scene_uid),
            QscLoadStatus::GitLfsPointer { oid, size } => Err(CueMolError::parse(format!(
                "QSC file is a Git LFS pointer: oid={oid}, size={size}"
            ))),
        }
    }

    pub fn load_qsc_file_checked(&mut self, path: impl AsRef<Path>) -> Result<QscLoadStatus> {
        let path = path.as_ref();
        let input = fs::read(path)?;
        if let Ok(text) = std::str::from_utf8(&input) {
            if let Some((oid, size)) = parse_git_lfs_pointer(text) {
                return Ok(QscLoadStatus::GitLfsPointer { oid, size });
            }
        }
        let uid = self.new_scene("loaded scene");
        self.scene_mut(uid)
            .ok_or_else(|| CueMolError::NotFound("loaded scene".to_string()))?
            .load_qsc_bytes_with_base_path(&input, path.parent())?;
        Ok(QscLoadStatus::Loaded { scene_uid: uid })
    }

    pub fn write_qsc_file(&self, scene_uid: Uid, path: impl AsRef<Path>) -> Result<()> {
        let scene = self
            .scene(scene_uid)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", scene_uid.get())))?;
        fs::write(path, scene.write_qsc_string())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewSceneResult {
    pub result_scene: Uid,
    pub result_view: Option<Uid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadObjectResult {
    pub result_object: Uid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadSceneResult {
    pub result_scene: Option<Uid>,
    pub status: QscLoadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewRendererResult {
    pub result_renderer: Uid,
}

#[derive(Debug, Clone)]
pub struct MolCommandManager {
    scene_manager: MolSceneManager,
    views: Vec<MolView>,
    next_view_uid: u64,
}

impl Default for MolCommandManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MolCommandManager {
    pub fn new() -> Self {
        Self {
            scene_manager: MolSceneManager::new(),
            views: Vec::new(),
            next_view_uid: VIEW_UID_BASE + 1,
        }
    }

    pub fn scene_manager(&self) -> &MolSceneManager {
        &self.scene_manager
    }

    pub fn scene_manager_mut(&mut self) -> &mut MolSceneManager {
        &mut self.scene_manager
    }

    pub fn view(&self, uid: Uid) -> Option<&MolView> {
        self.views.iter().find(|view| view.uid() == uid)
    }

    pub fn view_mut(&mut self, uid: Uid) -> Option<&mut MolView> {
        self.views.iter_mut().find(|view| view.uid() == uid)
    }

    pub fn views(&self) -> &[MolView] {
        &self.views
    }

    pub fn replace_views(&mut self, views: Vec<MolView>) {
        self.next_view_uid = views
            .iter()
            .map(|view| view.uid().get())
            .max()
            .unwrap_or(VIEW_UID_BASE)
            .saturating_add(1);
        self.views = views;
    }

    pub fn destroy_scene_command(&mut self, uid: Uid) -> bool {
        if !self.scene_manager.destroy_scene(uid) {
            return false;
        }
        self.views.retain(|view| view.scene_uid() != uid);
        true
    }

    pub fn destroy_all_scenes_command(&mut self) {
        self.scene_manager.destroy_all_scenes();
        self.views.clear();
    }

    pub fn new_scene_command(
        &mut self,
        scene_name: impl Into<String>,
        create_view: bool,
    ) -> NewSceneResult {
        let scene_uid = self.scene_manager.new_scene(scene_name);
        let result_view = create_view.then(|| {
            let uid = Uid::new(self.next_view_uid);
            self.next_view_uid += 1;
            self.views.push(MolView::new(uid, scene_uid));
            uid
        });
        NewSceneResult {
            result_scene: scene_uid,
            result_view,
        }
    }

    pub fn load_object_command(
        &mut self,
        target_scene: Uid,
        file_path: impl AsRef<Path>,
        object_name: impl Into<String>,
        file_format: &str,
    ) -> Result<LoadObjectResult> {
        let scene = self
            .scene_manager
            .scene_mut(target_scene)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", target_scene.get())))?;
        let object_uid = scene.load_object(file_path, object_name, file_format)?;
        Ok(LoadObjectResult {
            result_object: object_uid,
        })
    }

    pub fn load_scene_command(
        &mut self,
        file_path: impl AsRef<Path>,
        target_scene: Option<Uid>,
    ) -> Result<LoadSceneResult> {
        let file_path = file_path.as_ref();
        let input = fs::read(file_path)?;
        if let Ok(text) = std::str::from_utf8(&input) {
            if let Some((oid, size)) = parse_git_lfs_pointer(text) {
                return Ok(LoadSceneResult {
                    result_scene: None,
                    status: QscLoadStatus::GitLfsPointer { oid, size },
                });
            }
        }

        let scene_name = file_path
            .file_stem()
            .or_else(|| file_path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "loaded scene".to_string());
        let scene_uid = match target_scene {
            Some(uid) => {
                let mut loaded = self
                    .scene_manager
                    .scene(uid)
                    .cloned()
                    .ok_or_else(|| CueMolError::NotFound(format!("scene {}", uid.get())))?;
                loaded.clear_all_data();
                loaded.load_qsc_bytes_with_base_path(&input, file_path.parent())?;
                loaded.set_name(&scene_name);
                *self
                    .scene_manager
                    .scene_mut(uid)
                    .expect("target scene was validated above") = loaded;
                uid
            }
            None => {
                let uid = self.scene_manager.new_scene(&scene_name);
                let load_result = self
                    .scene_manager
                    .scene_mut(uid)
                    .expect("newly created scene must exist")
                    .load_qsc_bytes_with_base_path(&input, file_path.parent());
                if let Err(error) = load_result {
                    self.scene_manager.destroy_scene(uid);
                    return Err(error);
                }
                uid
            }
        };
        let scene = self
            .scene_manager
            .scene_mut(scene_uid)
            .expect("loaded scene must exist");
        scene.set_source(file_path.to_string_lossy());
        scene.set_source_type("qsc");
        scene.set_base_path(
            file_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_string_lossy(),
        );
        Ok(LoadSceneResult {
            result_scene: Some(scene_uid),
            status: QscLoadStatus::Loaded { scene_uid },
        })
    }

    pub fn new_renderer_command(
        &mut self,
        target_scene: Uid,
        target_object: Uid,
        renderer_type: impl Into<String>,
        renderer_name: impl Into<String>,
    ) -> Result<NewRendererResult> {
        let object = self
            .scene_manager
            .scene_mut(target_scene)
            .ok_or_else(|| CueMolError::NotFound(format!("scene {}", target_scene.get())))?
            .object_mut(target_object)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", target_object.get())))?;
        let renderer_uid = object.add_renderer(renderer_type, renderer_name);
        Ok(NewRendererResult {
            result_renderer: renderer_uid,
        })
    }
}

fn atom_display_name(atom: &Atom) -> String {
    format!(
        "{} {} {} {}",
        atom.chain_id,
        atom.res_name,
        atom.res_id,
        atom.name.trim()
    )
}

fn collect_qsc_extension_data(
    node: &LDom2Node,
    chunks: &BTreeMap<String, Vec<u8>>,
    base_path: Option<&Path>,
    output: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let source = node.str_attr("src").to_string();
    let source_type = node.str_attr("srctype").to_string();
    if !source.is_empty() && !source_type.is_empty() && !output.contains_key(&source) {
        let line = qsc_node_lines(node)
            .into_iter()
            .next()
            .ok_or_else(|| CueMolError::parse("QSC extension source element not found"))?;
        output.insert(source, load_qsc_source_data(&line, chunks, base_path)?);
    }
    for child in node.children().iter().filter(|child| !child.is_attr()) {
        collect_qsc_extension_data(child, chunks, base_path, output)?;
    }
    Ok(())
}

fn load_qsc_source_data(
    object_line: &str,
    chunks: &BTreeMap<String, Vec<u8>>,
    base_path: Option<&Path>,
) -> Result<Vec<u8>> {
    let src = attr_value(object_line, "src").unwrap_or_default();
    if src.starts_with("datachunk:") {
        return chunks
            .get(&src)
            .cloned()
            .ok_or_else(|| CueMolError::parse(format!("QSC datachunk not found: {src}")));
    }
    if src.is_empty() {
        return chunks
            .values()
            .next()
            .cloned()
            .ok_or_else(|| CueMolError::parse("QSC object source not found"));
    }

    let alt_src = attr_value(object_line, "alt_src").unwrap_or_default();
    let mut errors = Vec::new();
    for source in [src.as_str(), alt_src.as_str()]
        .into_iter()
        .filter(|source| !source.is_empty())
    {
        let path = Path::new(source);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base_path.unwrap_or_else(|| Path::new(".")).join(path)
        };
        match crate::read_bytes_maybe_compressed(&path) {
            Ok(data) => return Ok(data),
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    Err(CueMolError::Io(format!(
        "failed to load QSC object source: {}",
        errors.join("; ")
    )))
}

fn attr_value(line: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let marker = format!("{name}={quote}");
        if let Some(marker_start) = line.find(&marker) {
            let start = marker_start + marker.len();
            let end = line[start..].find(quote)? + start;
            return unescape_xml_value(&line[start..end]);
        }
    }
    None
}

fn unescape_xml_value(input: &str) -> Option<String> {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(index) = rest.find('&') {
        output.push_str(&rest[..index]);
        rest = &rest[index + 1..];
        let end = rest.find(';')?;
        let entity = &rest[..end];
        match entity {
            "amp" => output.push('&'),
            "lt" => output.push('<'),
            "gt" => output.push('>'),
            "quot" => output.push('"'),
            "apos" => output.push('\''),
            value if value.starts_with("#x") => {
                output.push(char::from_u32(u32::from_str_radix(&value[2..], 16).ok()?)?);
            }
            value if value.starts_with('#') => {
                output.push(char::from_u32(value[1..].parse().ok()?)?);
            }
            _ => return None,
        }
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    Some(output)
}

fn atom_intr_kind_from_line(line: &str) -> Option<AtomIntrKind> {
    if line.starts_with("<line ") {
        Some(AtomIntrKind::Distance)
    } else if line.starts_with("<angle ") {
        Some(AtomIntrKind::Angle)
    } else if line.starts_with("<torsion ") {
        Some(AtomIntrKind::Torsion)
    } else {
        None
    }
}

fn attr_map(line: &str) -> BTreeMap<String, String> {
    let Some(mut rest) = line.trim().strip_prefix('<') else {
        return BTreeMap::new();
    };
    rest = rest.trim_start_matches(|ch: char| !ch.is_whitespace());
    let mut attrs = BTreeMap::new();
    loop {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with('/') || rest.starts_with('>') {
            return attrs;
        }
        let Some(eq_pos) = rest.find('=') else {
            return attrs;
        };
        let name = rest[..eq_pos].trim();
        let after_eq = rest[eq_pos + 1..].trim_start();
        let Some(quote) = after_eq
            .chars()
            .next()
            .filter(|quote| matches!(quote, '"' | '\''))
        else {
            return attrs;
        };
        let after_quote = &after_eq[quote.len_utf8()..];
        let Some(end_quote) = after_quote.find(quote) else {
            return attrs;
        };
        let Some(value) = unescape_xml_value(&after_quote[..end_quote]) else {
            return attrs;
        };
        attrs.insert(name.to_string(), value);
        rest = &after_quote[end_quote + 1..];
    }
}

fn format_renderer_attrs(renderer: &MolRenderer, default_style: Option<&str>) -> String {
    let mut attrs = renderer.qsc_attrs.clone();
    attrs.remove("ui_order");
    attrs.insert("type".to_string(), renderer.renderer_type().to_string());
    attrs.insert("name".to_string(), renderer.name().to_string());
    attrs.entry("group".to_string()).or_default();
    if let Some(default_style) = default_style {
        attrs
            .entry("style".to_string())
            .or_insert_with(|| default_style.to_string());
    }
    let ordered = ["type", "group", "name", "style"];
    let mut parts = Vec::new();
    for name in ordered {
        if let Some(value) = attrs.remove(name) {
            parts.push(format!("{name}=\"{}\"", escape_xml(&value)));
        }
    }
    for (name, value) in attrs {
        parts.push(format!("{name}=\"{}\"", escape_xml(&value)));
    }
    format!(" {}", parts.join(" "))
}

fn trim_ascii_bytes(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn qsc_node_lines(node: &LDom2Node) -> Vec<String> {
    write_ldom2_xml(&LDom2Tree::from(node.clone()))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("<?xml"))
        .map(str::to_string)
        .collect()
}

fn remap_qsc_extension_sources(node: &mut LDom2Node, chunk_ids: &BTreeMap<String, String>) {
    let source = node.str_attr("src").to_string();
    if let Some(chunk_id) = chunk_ids.get(&source) {
        node.set_str_attr("src", format!("datachunk:{chunk_id}"));
    }
    for child in node
        .children_mut()
        .iter_mut()
        .filter(|child| !child.is_attr())
    {
        remap_qsc_extension_sources(child, chunk_ids);
    }
}

fn append_qsc_node(output: &mut String, node: &LDom2Node, indent: usize) {
    let pad = "\t".repeat(indent);
    for line in qsc_node_lines(node) {
        output.push_str(&pad);
        output.push_str(&line);
        output.push('\n');
    }
}

fn parse_qsc_camera_node(
    node: &LDom2Node,
    scene: &MolScene,
    base_path: Option<&Path>,
) -> Result<Camera> {
    let source = node.str_attr("src").to_string();
    let alt_source = node.str_attr("alt_src").to_string();
    let mut camera = Camera::default();
    if !source.is_empty() {
        let source_line = qsc_node_lines(node)
            .into_iter()
            .next()
            .ok_or_else(|| CueMolError::parse("QSC camera element not found"))?;
        let data = load_qsc_source_data(&source_line, &BTreeMap::new(), base_path)?;
        let external = parse_ldom2_xml(&String::from_utf8_lossy(&data))?;
        if external.top().tag_name() != "camera" {
            return Err(CueMolError::parse(format!(
                "external camera root is not camera: {}",
                external.top().tag_name()
            )));
        }
        apply_qsc_camera_node(&mut camera, external.top(), scene)?;
    }
    apply_qsc_camera_node(&mut camera, node, scene)?;
    camera.set_source(source);
    camera.set_alt_source(alt_source);
    Ok(camera)
}

fn apply_qsc_camera_node(camera: &mut Camera, node: &LDom2Node, scene: &MolScene) -> Result<()> {
    let attr = |name: &str| {
        let value = node.str_attr(name).to_string();
        (!value.is_empty()).then_some(value)
    };
    if let Some(name) = attr("name") {
        camera.set_name(name);
    }
    if let Some(center) = attr("center") {
        camera.set_center(parse_vector3(&center)?);
    }
    if let Some(rotation) = attr("rotation") {
        camera.set_rotation(parse_quat(&rotation)?);
    }
    if let Some(distance) = attr("distance").and_then(|value| value.parse().ok()) {
        camera.set_cam_dist(distance);
    }
    if let Some(slab) = attr("slab").and_then(|value| value.parse().ok()) {
        camera.set_slab_depth(slab);
    }
    if let Some(zoom) = attr("zoom").and_then(|value| value.parse().ok()) {
        camera.set_zoom(zoom);
    }
    if let Some(stereo_distance) = attr("stereoDist").and_then(|value| value.parse().ok()) {
        camera.set_stereo_distance(stereo_distance);
    }
    if let Some(perspective) = attr("perspec") {
        camera.set_perspective(perspective.eq_ignore_ascii_case("true"));
    }
    if let Some(center_mark) = attr("centerMark") {
        camera.set_center_mark(center_mark_from_qsc(&center_mark));
    }
    if let Some(stereo_mode) = attr("stereoMode") {
        camera.set_stereo_mode(stereo_mode_from_qsc(&stereo_mode));
    }

    let Some(visibilities) = node.find_child("visibilities") else {
        return Ok(());
    };
    camera.clear_visibility_settings();
    for setting in visibilities
        .children()
        .iter()
        .filter(|child| !child.is_attr())
    {
        let visible = setting.value().eq_ignore_ascii_case("true");
        match setting.tag_name() {
            "object" => {
                let target = setting.str_attr("target").to_string();
                if let Some(object) = scene.object_by_name(&target) {
                    camera.vis_append(object.uid().get(), visible, true);
                }
            }
            "renderer" => {
                let client = setting.str_attr("client").to_string();
                let target = setting.str_attr("target").to_string();
                let renderer_type = setting.str_attr("type").to_string();
                let renderer = scene.object_by_name(&client).and_then(|object| {
                    object.renderer_by_name(&target).filter(|renderer| {
                        renderer_type.is_empty() || renderer.renderer_type() == renderer_type
                    })
                });
                if let Some(renderer) = renderer {
                    camera.vis_append(renderer.uid().get(), visible, false);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_vector3(input: &str) -> Result<Vector4> {
    let values = parse_tuple(input)?;
    if values.len() != 3 {
        return Err(CueMolError::parse(format!(
            "invalid vector3 tuple: {input}"
        )));
    }
    Ok(Vector4::new3(values[0], values[1], values[2]))
}

fn parse_quat(input: &str) -> Result<Quat> {
    let values = parse_tuple(input)?;
    if values.len() != 4 {
        return Err(CueMolError::parse(format!(
            "invalid quaternion tuple: {input}"
        )));
    }
    Ok(Quat::new(values[0], values[1], values[2], values[3]))
}

/// Parse a C++ `LScrMatrix4D` serialization into a [`Matrix4`]. The value is
/// either `(identity)` or a `(m1,...,m16)` tuple in COLUMN-MAJOR order (C++
/// `Matrix4D::ai` linear index). `Matrix4::set_ai` uses the same column-major
/// linear index, so the elements map across directly (no transpose). Returns
/// `None` on a malformed value, which callers treat as identity.
fn parse_xform_mat(input: &str) -> Option<Matrix4> {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("(identity)") {
        return Some(Matrix4::IDENTITY);
    }
    let values = parse_tuple(trimmed).ok()?;
    if values.len() != 16 {
        return None;
    }
    let mut matrix = Matrix4::IDENTITY;
    for (index, value) in values.iter().enumerate() {
        matrix.set_ai(index + 1, *value).ok()?;
    }
    Some(matrix)
}

/// Serialize a [`Matrix4`] in the C++ `LScrMatrix4D::toString` format: `(identity)`
/// for the identity matrix, otherwise `(m1,...,m16)` in column-major order.
fn format_xform_mat(matrix: Matrix4) -> String {
    if matrix == Matrix4::IDENTITY {
        return "(identity)".to_string();
    }
    let parts: Vec<String> = (1..=16)
        .map(|index| format_float(matrix.ai(index).unwrap_or(0.0)))
        .collect();
    format!("({})", parts.join(","))
}

fn parse_tuple(input: &str) -> Result<Vec<f64>> {
    let trimmed = input.trim().trim_start_matches('(').trim_end_matches(')');
    trimmed
        .split(',')
        .map(|value| {
            value
                .trim()
                .parse::<f64>()
                .map_err(|_| CueMolError::parse(format!("invalid tuple number: {value}")))
        })
        .collect()
}

fn format_vector3(value: Vector4) -> String {
    format!(
        "({},{},{})",
        format_float(value.x),
        format_float(value.y),
        format_float(value.z)
    )
}

fn format_quat(value: Quat) -> String {
    format!(
        "({},{},{},{})",
        format_float(value.x),
        format_float(value.y),
        format_float(value.z),
        format_float(value.w)
    )
}

fn format_float(value: f64) -> String {
    let mut formatted = format!("{value:.6}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    if formatted == "-0" {
        "0".to_string()
    } else {
        formatted
    }
}

fn center_mark_from_qsc(value: &str) -> CenterMark {
    match value {
        "none" => CenterMark::None,
        "axis" => CenterMark::Axis,
        _ => CenterMark::Cross,
    }
}

fn center_mark_to_qsc(value: CenterMark) -> &'static str {
    match value {
        CenterMark::None => "none",
        CenterMark::Cross => "crosshair",
        CenterMark::Axis => "axis",
    }
}

fn stereo_mode_from_qsc(value: &str) -> StereoMode {
    match value {
        "parallel" | "para" => StereoMode::Parallel,
        "cross" => StereoMode::Cross,
        "hardware" => StereoMode::Hardware,
        _ => StereoMode::None,
    }
}

fn stereo_mode_to_qsc(value: StereoMode) -> &'static str {
    match value {
        StereoMode::None => "none",
        StereoMode::Parallel => "para",
        StereoMode::Cross => "cross",
        StereoMode::Hardware => "hardware",
    }
}

fn parse_git_lfs_pointer(input: &str) -> Option<(String, u64)> {
    let mut lines = input.lines();
    if lines.next()? != "version https://git-lfs.github.com/spec/v1" {
        return None;
    }
    let mut oid = None;
    let mut size = None;
    for line in lines {
        if let Some(value) = line.strip_prefix("oid sha256:") {
            oid = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("size ") {
            size = value.parse::<u64>().ok();
        }
    }
    Some((oid?, size?))
}

fn escape_json(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn append_qsc_data_chunk(output: &mut String, chunk_id: &str, data: &[u8]) {
    output.push_str(&format!("==========START datachunk:{chunk_id}==========\n"));
    let encoded = if data.starts_with(b"QDF0") {
        cuemol_qsys::encode_qdf0_bytes(data, 1, "10").ok()
    } else {
        None
    };
    let text = encoded.as_deref().unwrap_or(data);
    output.push_str(&String::from_utf8_lossy(text));
    if !text.ends_with(b"\n") {
        output.push('\n');
    }
    output.push_str(&format!("==========END datachunk:{chunk_id}==========\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qsc_camera_display_modes_match_script_enums() {
        assert_eq!(center_mark_from_qsc("crosshair"), CenterMark::Cross);
        assert_eq!(center_mark_from_qsc("axis"), CenterMark::Axis);
        assert_eq!(center_mark_to_qsc(CenterMark::Axis), "axis");
        assert_eq!(stereo_mode_from_qsc("para"), StereoMode::Parallel);
        assert_eq!(stereo_mode_from_qsc("hardware"), StereoMode::Hardware);
        assert_eq!(stereo_mode_to_qsc(StereoMode::Parallel), "para");
        assert_eq!(stereo_mode_to_qsc(StereoMode::Hardware), "hardware");
    }

    #[test]
    fn scene_object_and_renderer_uids_are_globally_unique() {
        let mut manager = MolSceneManager::new();
        let scene1 = manager.new_scene("one");
        let scene2 = manager.new_scene("two");
        let object1 = manager
            .scene_mut(scene1)
            .unwrap()
            .add_molecule("a", MolCoord::new());
        let object2 = manager
            .scene_mut(scene2)
            .unwrap()
            .add_molecule("b", MolCoord::new());
        let renderer1 = manager
            .scene_mut(scene1)
            .unwrap()
            .object_mut(object1)
            .unwrap()
            .add_renderer("simple", "r1");
        let renderer2 = manager
            .scene_mut(scene2)
            .unwrap()
            .object_mut(object2)
            .unwrap()
            .add_renderer("simple", "r2");

        let ids = [scene1, scene2, object1, object2, renderer1, renderer2]
            .into_iter()
            .map(Uid::get)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 6);
        assert_eq!(manager.object(object1).unwrap().name(), "a");
        assert_eq!(manager.object(object2).unwrap().name(), "b");
    }

    #[test]
    fn molecule_renderer_interprets_single_and_multiple_atom_hits() {
        let molecule = read_pdb_str(
            "ATOM      1  CA  ALA A   1       1.000   2.000   3.000  0.50 12.00\nATOM      2  CB  ALA A   1       4.000   5.000   6.000  1.00 20.00\n",
        )
        .unwrap();
        let mut renderer = MolRenderer {
            uid: Uid::new(7),
            name: "simple".to_string(),
            renderer_type: "simple".to_string(),
            qsc_attrs: BTreeMap::new(),
            qsc_children: Vec::new(),
            atom_intr_lines: Vec::new(),
            selection: Selector::All,
            coloring: SolidColoring::new(),
            default_color: SolidColor::from_rgb(1.0, 1.0, 1.0),
        };
        renderer.set_sel(crate::parse_selector("name CA").unwrap());
        assert_eq!(renderer.sel(), &crate::parse_selector("name CA").unwrap());
        renderer.set_coloring(SolidColoring::new());
        assert_eq!(renderer.coloring().type_name(), "SolidColoring");
        assert_eq!(renderer.defaultcolor(), renderer.default_color());
        let default_color = SolidColor::from_rgb(0.25, 0.5, 0.75);
        renderer.set_defaultcolor(default_color.clone());
        assert_eq!(renderer.default_color(), &default_color);
        let mut context = cuemol_gfx::HittestContext::new();
        context.add_hit(renderer.uid(), 0.2, vec![1]);
        let mut hit_data = cuemol_gfx::HitData::new();
        hit_data.create_nearest(&context);
        let single = renderer.interp_hit(&hit_data, &molecule);
        assert!(renderer.is_hit_test_supported());
        assert!(single.contains("\"objtype\":\"MolCoord\""));
        assert!(single.contains("\"sel\":\"aid 1\""));
        assert!(single.contains("\"atom_id\":1"));
        assert!(single.contains("\"occ\":0.50"));
        assert!(single.contains("\"x\":1.00"));

        context.add_hit(renderer.uid(), 0.3, vec![2]);
        hit_data.create_all(&context);
        let multiple = renderer.interp_hit(&hit_data, &molecule);
        assert!(multiple.contains("\"size\":2"));
        assert!(multiple.contains("\"sel\":\"aid 1:2\""));

        let mut unsupported = renderer.clone();
        unsupported.renderer_type = "selection".to_string();
        assert!(!unsupported.is_hit_test_supported());
    }

    #[test]
    fn parse_xform_mat_reads_column_major_rotation() {
        assert_eq!(parse_xform_mat("(identity)"), Some(Matrix4::IDENTITY));
        assert_eq!(parse_xform_mat("(IDENTITY)"), Some(Matrix4::IDENTITY));

        // R_z(+90) [ (x,y,z)->(-y,x,z) ] + translate(0,0,5), column-major:
        // cols [0,1,0,0][-1,0,0,0][0,0,1,0][0,0,5,1].
        let matrix = parse_xform_mat("(0,1,0,0,-1,0,0,0,0,0,1,0,0,0,5,1)")
            .expect("well-formed 16-tuple parses");
        // The row-major result; the -1 in row0/col1 and +1 in row1/col0 catch a
        // column-major<->row-major transpose; tz=5 sits in row2/col3.
        assert_eq!(matrix.get_at(1, 2).unwrap(), -1.0, "row0,col1 = -sin");
        assert_eq!(matrix.get_at(2, 1).unwrap(), 1.0, "row1,col0 = +sin");
        assert_eq!(matrix.get_at(3, 4).unwrap(), 5.0, "row2,col3 = tz");
        assert_eq!(matrix.get_at(4, 4).unwrap(), 1.0, "homogeneous w");

        // Apply to a point to confirm the orientation end to end.
        let mut p = Vector4::new3(1.0, 0.0, 0.0);
        matrix.xform3d(&mut p);
        assert!((p.x).abs() < 1e-9 && (p.y - 1.0).abs() < 1e-9 && (p.z - 5.0).abs() < 1e-9);

        // Round-trips through the column-major writer.
        assert_eq!(parse_xform_mat(&format_xform_mat(matrix)), Some(matrix));
        assert_eq!(format_xform_mat(Matrix4::IDENTITY), "(identity)");

        // Malformed -> None (callers fall back to identity).
        assert_eq!(parse_xform_mat("(1,2,3)"), None);
        assert_eq!(parse_xform_mat("garbage"), None);
    }

    #[test]
    fn object_visible_round_trips_through_qsc() {
        // C++ serializes the default-true `visible` property only when hidden,
        // as a `<object>` attribute (qlib `LDOM2Tree.cpp:172` writes string-
        // convertible scalars with `setAttrFlag(true)`). Mirror that: a hidden
        // object writes `visible="false"`; a visible one omits the attribute.
        let mut manager = MolSceneManager::new();
        let scene = manager.new_scene("s");
        let mol =
            read_pdb_str("ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00\n")
                .unwrap();
        let scene_ref = manager.scene_mut(scene).unwrap();
        let _visible_uid = scene_ref.add_molecule("shown", mol.clone());
        let hidden_uid = scene_ref.add_molecule("hidden", mol);
        scene_ref.object_mut(hidden_uid).unwrap().set_visible(false);

        let xml = manager.scene(scene).unwrap().write_qsc_string();
        // The hidden object carries the attribute; the visible one does not.
        assert!(
            xml.contains(
                "name=\"hidden\" src=\"datachunk:00001\" srctype=\"qdfpdb\" visible=\"false\""
            ),
            "hidden object must serialize visible=\"false\":\n{xml}"
        );
        assert!(
            xml.contains("name=\"shown\" src=\"datachunk:00000\" srctype=\"qdfpdb\">"),
            "visible object must omit the visible attribute (default true):\n{xml}"
        );

        // Round-trip: re-loading preserves both visibilities.
        let mut reloaded = MolSceneManager::new();
        let scene2 = reloaded.new_scene("reloaded");
        reloaded
            .scene_mut(scene2)
            .unwrap()
            .load_qsc_str(&xml)
            .unwrap();
        let objects: Vec<bool> = reloaded
            .scene(scene2)
            .unwrap()
            .objects()
            .iter()
            .map(|object| object.is_visible())
            .collect();
        assert_eq!(
            objects,
            vec![true, false],
            "visibility survives the round-trip"
        );
    }

    #[test]
    fn object_qif_state_round_trips_through_qsc() {
        let mut manager = MolSceneManager::new();
        let scene = manager.new_scene("s");
        let object_uid = manager.scene_mut(scene).unwrap().add_molecule(
            "mol",
            read_pdb_str("ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00\n")
                .unwrap(),
        );
        let object = manager
            .scene_mut(scene)
            .unwrap()
            .object_mut(object_uid)
            .unwrap();
        object.set_ui_locked(true);
        object.set_ui_collapsed(true);
        object.set_ui_order(19);
        object.set_alt_source("fallback.pdb");
        let renderer_uid = object.add_renderer("simple", "r1");
        assert_eq!(object.rend_uid_list(), renderer_uid.get().to_string());
        assert!(object.modified());

        let xml = manager.scene(scene).unwrap().write_qsc_string();
        assert!(xml.contains("locked=\"true\""));
        assert!(xml.contains("ui_collapsed=\"true\""));
        assert!(xml.contains("alt_src=\"fallback.pdb\""));
        assert!(!xml.contains("ui_order=\"19\""));

        let mut reloaded = MolSceneManager::new();
        let target_scene = reloaded.new_scene("target");
        reloaded
            .scene_mut(target_scene)
            .unwrap()
            .load_qsc_str(&xml)
            .unwrap();
        let object = reloaded
            .scene(target_scene)
            .unwrap()
            .object_by_name("mol")
            .unwrap();
        assert!(object.is_ui_locked());
        assert!(object.is_ui_collapsed());
        assert_eq!(object.alt_source(), "fallback.pdb");
        assert_eq!(object.source(), "datachunk:00000");
        assert!(!object.modified());
    }

    #[test]
    fn renderer_type_change_replaces_uid_and_preserves_serialized_state() {
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("renderer type");
        let object_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", MolCoord::new());
        let object = manager.object_mut(object_uid).unwrap();
        let renderer_uid = object.add_renderer("simple", "lines");
        let renderer = object.renderer_mut(renderer_uid).unwrap();
        renderer.set_qsc_attr("group", "representations");
        renderer.set_qsc_attr("sel", "protein");
        renderer.set_qsc_attr("ui_order", "7");
        renderer.set_qsc_child_attr("section", "width", "0.45");
        renderer.set_visible(false);

        let replacement_uid = object
            .change_renderer_type(renderer_uid, "cartoon")
            .unwrap();
        assert_ne!(replacement_uid, renderer_uid);
        assert!(object.renderer(renderer_uid).is_none());
        let replacement = object.renderer(replacement_uid).unwrap();
        assert_eq!(replacement.renderer_type(), "cartoon");
        assert_eq!(replacement.name(), "lines");
        assert_eq!(replacement.group(), Some("representations"));
        assert_eq!(replacement.qsc_attr("sel"), Some("protein"));
        assert_eq!(replacement.qsc_attr("ui_order"), Some("7"));
        assert_eq!(
            replacement.qsc_child_attr("section", "width").as_deref(),
            Some("0.45")
        );
        assert!(!replacement.is_visible());
    }

    #[test]
    fn qsc_xml_entities_round_trip_in_names_and_renderer_properties() {
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("entities");
        let object_uid = manager.scene_mut(scene_uid).unwrap().add_molecule(
            "A&B <object>",
            read_pdb_str("ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00\n")
                .unwrap(),
        );
        let renderer_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "R&1");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap()
            .set_qsc_attr("style", "one&two");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap()
            .set_qsc_child_attr("section", "type", "roundsquare");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap()
            .renderer_mut(renderer_uid)
            .unwrap()
            .set_qsc_child_attr("section", "width", "0.45");

        let qsc = manager.scene(scene_uid).unwrap().write_qsc_string();
        assert!(qsc.contains("name=\"A&amp;B &lt;object&gt;\""));
        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&qsc)
            .unwrap();
        let object = reloaded
            .scene(reloaded_uid)
            .unwrap()
            .object_by_name("A&B <object>")
            .unwrap();
        let renderer = object.renderer_by_name("R&1").unwrap();
        assert_eq!(renderer.qsc_attr("style"), Some("one&two"));
        assert_eq!(
            renderer.qsc_child_attr("section", "type").as_deref(),
            Some("roundsquare")
        );
        assert_eq!(
            renderer.qsc_child_attr("section", "width").as_deref(),
            Some("0.45")
        );
    }

    #[test]
    fn atom_intr_angle_and_torsion_round_trip_through_qsc() {
        let molecule = read_pdb_str(
            "ATOM      1  A1  GLY A   1       1.000   0.000   0.000  1.00 10.00\n\
             ATOM      2  A2  GLY A   1       0.000   0.000   0.000  1.00 10.00\n\
             ATOM      3  A3  GLY A   1       0.000   1.000   0.000  1.00 10.00\n\
             ATOM      4  A4  GLY A   1       0.000   1.000   1.000  1.00 10.00\n",
        )
        .unwrap();
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("measurements");
        let scene = manager.scene_mut(scene_uid).unwrap();
        let object_uid = scene.add_molecule("mol", molecule);
        let object = scene.object_mut(object_uid).unwrap();
        object
            .add_atom_intr_renderer_defs(
                "measure",
                &[
                    (
                        AtomIntrKind::Distance,
                        vec!["0".to_string(), "1".to_string()],
                    ),
                    (
                        AtomIntrKind::Angle,
                        vec!["0".to_string(), "1".to_string(), "2".to_string()],
                    ),
                    (
                        AtomIntrKind::Torsion,
                        vec![
                            "0".to_string(),
                            "1".to_string(),
                            "2".to_string(),
                            "3".to_string(),
                        ],
                    ),
                ],
            )
            .unwrap();

        let renderer = object.renderer_by_name("measure").unwrap();
        let values = object.atom_intr_values(renderer);
        assert!((values[0].unwrap() - 1.0).abs() < 1.0e-9);
        assert!((values[1].unwrap() - 90.0).abs() < 1.0e-9);
        assert!(values[2].unwrap().abs() > 89.9);
        assert_eq!(object.atom_intr_segments(renderer).len(), 6);
        let labels = object.atom_intr_labels(renderer);
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0].1, "1.00");
        assert_eq!(labels[1].1, "90.00");
        assert_eq!(labels[0].0, Vector4::new3(0.5, 0.0, 0.0));
        assert_eq!(labels[2].0, Vector4::new3(0.0, 0.5, 0.0));

        let qsc = manager.scene(scene_uid).unwrap().write_qsc_string();
        assert!(qsc.contains("<angle aid1=\"0\" aid2=\"1\" aid3=\"2\"/>"));
        assert!(qsc.contains("<torsion aid1=\"0\" aid2=\"1\" aid3=\"2\" aid4=\"3\"/>"));

        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&qsc)
            .unwrap();
        let object = reloaded
            .scene(reloaded_uid)
            .unwrap()
            .object_by_name("mol")
            .unwrap();
        let renderer = object.renderer_by_name("measure").unwrap();
        assert!(renderer.get_defs_json().contains("\"mode\":2"));
        assert!(renderer.get_defs_json().contains("\"mode\":3"));
        assert_eq!(object.atom_intr_segments(renderer).len(), 6);
    }

    #[test]
    fn atom_intr_fixed_positions_round_trip_through_qsc() {
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("fixed-measurements");
        let molecule =
            read_pdb_str("ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00\n")
                .unwrap();
        let object_uid = manager
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("dummy", molecule);
        let object = manager
            .scene_mut(scene_uid)
            .unwrap()
            .object_mut(object_uid)
            .unwrap();
        object
            .add_atom_intr_position_renderer_defs(
                "pse-distance",
                &[(
                    AtomIntrKind::Distance,
                    vec![Vector4::new3(1.0, 2.0, 3.0), Vector4::new3(4.0, 6.0, 3.0)],
                )],
            )
            .unwrap();

        let renderer = object.renderer_by_name("pse-distance").unwrap();
        assert_eq!(object.atom_intr_values(renderer), vec![Some(5.0)]);
        assert!(renderer.atom_intr_lines()[0].aids().is_empty());
        assert_eq!(renderer.atom_intr_lines()[0].positions().len(), 2);

        let qsc = manager.scene(scene_uid).unwrap().write_qsc_string();
        assert!(qsc.contains("<line pos1=\"(1,2,3)\" pos2=\"(4,6,3)\"/>"));

        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&qsc)
            .unwrap();
        let object = reloaded
            .scene(reloaded_uid)
            .unwrap()
            .objects()
            .first()
            .unwrap();
        let renderer = object.renderer_by_name("pse-distance").unwrap();
        assert_eq!(object.atom_intr_values(renderer), vec![Some(5.0)]);
        assert_eq!(object.atom_intr_segments(renderer).len(), 1);
    }

    #[test]
    fn qsc_load_skips_non_molecule_objects_gracefully() {
        // A `.qsc` saved by C++ CueMol2 can hold non-molecule objects (a MolSurf surface
        // or DensityMap map) whose datachunk is a QDF binary, not PDB. The loader used to
        // feed EVERY object's datachunk to `read_pdb_str`, so such an object corrupted or
        // failed the WHOLE scene load. Now they are identified by `type` and skipped
        // gracefully — the molecules still load, and the skip is recorded (not silent).
        let mut manager = MolSceneManager::new();
        let scene = manager.new_scene("s");
        let mol =
            read_pdb_str("ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00\n")
                .unwrap();
        manager.scene_mut(scene).unwrap().add_molecule("mol1", mol);
        let xml = manager.scene(scene).unwrap().write_qsc_string();
        // Splice in a surface object (its datachunk is deliberately absent — the loader
        // must skip it BEFORE trying to resolve/parse the chunk).
        let surf = "\t<object type=\"MolSurf\" name=\"surf1\" src=\"datachunk:90000\" srctype=\"qdfsurf\">\n\t</object>\n</scene>";
        let xml = xml.replace("</scene>", surf);

        let mut reloaded = MolSceneManager::new();
        let scene2 = reloaded.new_scene("reloaded");
        // The load must SUCCEED (not error on the surface object)...
        reloaded
            .scene_mut(scene2)
            .unwrap()
            .load_qsc_str(&xml)
            .unwrap();
        let scene_ref = reloaded.scene(scene2).unwrap();
        // ...load only the molecule...
        assert_eq!(
            scene_ref.objects().len(),
            1,
            "only the MolCoord object loads"
        );
        assert_eq!(scene_ref.objects()[0].name(), "mol1");
        assert!(scene_ref.objects()[0].mol().atom_count() >= 1);
        // ...and record the skipped surface (not silently dropped).
        assert_eq!(
            scene_ref.skipped_qsc_objects(),
            &[("surf1".to_string(), "MolSurf".to_string())],
            "the skipped surface object is recorded"
        );
    }

    #[test]
    fn qsc_load_captures_obj_text_surface_object() {
        // A MolSurf object whose datachunk is OBJ mesh TEXT (the Rust `.qsc` convention)
        // is captured into loaded_surfaces so a front end can display it.
        let mut manager = MolSceneManager::new();
        let scene = manager.new_scene("s");
        let mol =
            read_pdb_str("ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00  0.00\n")
                .unwrap();
        manager.scene_mut(scene).unwrap().add_molecule("mol1", mol);
        let mut xml = manager.scene(scene).unwrap().write_qsc_string();
        // Splice in an OBJ-text surface object + its datachunk block (a single triangle).
        let surf_obj = "\t<object type=\"MolSurf\" name=\"surf1\" src=\"datachunk:90000\" srctype=\"obj\">\n\t</object>\n</scene>";
        xml = xml.replace("</scene>", surf_obj);
        xml.push_str(
            "\n==========START datachunk:90000==========\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n==========END datachunk:90000==========\n",
        );

        let mut reloaded = MolSceneManager::new();
        let scene2 = reloaded.new_scene("reloaded");
        reloaded
            .scene_mut(scene2)
            .unwrap()
            .load_qsc_str(&xml)
            .unwrap();
        let scene_ref = reloaded.scene(scene2).unwrap();
        // The molecule still loads, and the OBJ surface is captured (not skipped).
        assert_eq!(scene_ref.objects().len(), 1, "the MolCoord object loads");
        assert_eq!(
            scene_ref.loaded_surfaces().len(),
            1,
            "the OBJ surface is captured"
        );
        assert_eq!(scene_ref.loaded_surfaces()[0].name, "surf1");
        assert_eq!(scene_ref.loaded_surfaces()[0].srctype, "obj");
        assert_ne!(
            scene_ref.loaded_surfaces()[0].uid,
            scene_ref.objects()[0].uid()
        );
        assert!(scene_ref.loaded_surfaces()[0].data.contains("v 0 0 0"));
        assert!(
            scene_ref.skipped_qsc_objects().is_empty(),
            "an OBJ-text surface is displayable, not a skip"
        );
        let surface_uid = scene_ref.loaded_surfaces()[0].uid;
        let next_uid = reloaded
            .scene_mut(scene2)
            .unwrap()
            .add_molecule("after-surface", MolCoord::new());
        assert_ne!(surface_uid, next_uid);
    }

    #[test]
    fn qsc_loads_native_qdfmol_datachunk() {
        let mut source = read_pdb_str(
            "ATOM      1  CA  ALA A   1       1.000   2.000   3.000  1.00 10.00\n\
             ATOM      2  C   ALA A   1       2.000   2.000   3.000  1.00 11.00\n",
        )
        .unwrap();
        source.atoms_mut()[0].set_int_prop("custom", 42);
        let qdf0 = crate::write_qdf_mol_bytes(&source);
        let encoded = cuemol_qsys::encode_qdf0_bytes(&qdf0, 1, "13").unwrap();
        let encoded = String::from_utf8(encoded).unwrap();
        let qsc = format!(
            "<scene>\n\
             <object type=\"MolCoord\" name=\"native\" src=\"datachunk:00000\" srctype=\"qdfmol\">\n\
             </object>\n\
             </scene>\n\
             ========== End of XML ==========\n\
             ==========START datachunk:00000==========\n\
             {encoded}\
             ==========END datachunk:00000==========\n"
        );

        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("qdf");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .load_qsc_str(&qsc)
            .unwrap();
        let loaded = manager
            .scene(scene_uid)
            .unwrap()
            .object_by_name("native")
            .unwrap();
        assert_eq!(loaded.mol().atom_count(), 2);
        assert_eq!(loaded.mol().atoms()[0].position, source.atoms()[0].position);
        assert_eq!(loaded.source_type(), "qdfmol");
        assert_eq!(loaded.mol().atoms()[0].int_prop("custom"), Some(42));

        let rewritten = manager.scene(scene_uid).unwrap().write_qsc_string();
        assert!(rewritten.contains("srctype=\"qdfmol\""));
        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("rewritten");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&rewritten)
            .unwrap();
        let reloaded_object = reloaded
            .scene(reloaded_uid)
            .unwrap()
            .object_by_name("native")
            .unwrap();
        assert_eq!(reloaded_object.source_type(), "qdfmol");
        assert_eq!(
            reloaded_object.mol().atoms()[0].int_prop("custom"),
            Some(42)
        );
    }

    #[test]
    fn qsc_preserves_molecule_derived_extensions_and_chunks() {
        let start =
            "ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n";
        let end =
            "ATOM      1  CA  GLY A   1      10.000   0.000   0.000  1.00 10.00           C  \nEND\n";
        let qsc = format!(
            "<scene>\n\
             <object type=\"MorphMol\" name=\"morph\" src=\"datachunk:00000\" \
             srctype=\"qdfpdb\" scale_frame=\"true\">\n\
             <frames><this/><mol name=\"end\" src=\"datachunk:00001\" \
             srctype=\"qdfpdb\"/></frames>\n\
             </object>\n\
             </scene>\n\
             ========== End of XML ==========\n\
             ==========START datachunk:00000==========\n\
             {start}\
             ==========END datachunk:00000==========\n\
             ==========START datachunk:00001==========\n\
             {end}\
             ==========END datachunk:00001==========\n"
        );
        let mut scene = MolSceneManager::new();
        let uid = scene.new_scene("morph");
        scene.scene_mut(uid).unwrap().load_qsc_str(&qsc).unwrap();
        let object = &scene.scene(uid).unwrap().objects()[0];
        assert_eq!(object.object_type(), "MorphMol");
        assert_eq!(
            object.qsc_extra_attrs().get("scale_frame"),
            Some(&"true".to_string())
        );
        assert_eq!(object.qsc_extension_nodes()[0].tag_name(), "frames");
        assert!(object.qsc_extra_data("datachunk:00001").is_some());

        let rewritten = scene.scene(uid).unwrap().write_qsc_string();
        assert!(rewritten.contains("type=\"MorphMol\""));
        assert!(rewritten.contains("scale_frame=\"true\""));
        assert!(rewritten.contains("<frames>"));
        assert!(rewritten.contains("name=\"end\""));
        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&rewritten)
            .unwrap();
        let object = &reloaded.scene(reloaded_uid).unwrap().objects()[0];
        assert_eq!(object.object_type(), "MorphMol");
        assert!(object.qsc_extra_data("datachunk:00001").is_some());
    }

    #[test]
    fn qsc_file_loads_relative_external_source_and_alt_source() {
        let directory = std::env::temp_dir().join(format!(
            "cuemol-qsc-linked-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&directory).unwrap();
        let molecule_path = directory.join("linked.pdb");
        fs::write(
            &molecule_path,
            "ATOM      1  CA  ALA A   1       1.000   2.000   3.000  1.00 10.00\n",
        )
        .unwrap();
        let qsc_path = directory.join("linked.qsc");
        fs::write(
            &qsc_path,
            "<scene>\n\
             <object\n\
               type='MolCoord'\n\
               name='linked'\n\
               src='missing.pdb'\n\
               alt_src='linked.pdb'\n\
               srctype='pdb'>\n\
             </object>\n\
             </scene>\n",
        )
        .unwrap();

        let mut manager = MolSceneManager::new();
        let scene_uid = manager.load_qsc_file(&qsc_path).unwrap();
        let object = manager
            .scene(scene_uid)
            .unwrap()
            .object_by_name("linked")
            .unwrap();
        assert_eq!(object.source_type(), "pdb");
        assert_eq!(object.mol().atom_count(), 1);
        assert_eq!(
            object.mol().atoms()[0].position,
            Vector4::new3(1.0, 2.0, 3.0)
        );

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn qsc_loads_non_utf8_qdf0_datachunk() {
        let source =
            read_pdb_str("ATOM      1  CA  ALA A   1       1.000   2.000   3.000  1.00 10.00\n")
                .unwrap();
        let qdf0 = crate::write_qdf_mol_bytes(&source);
        assert!(std::str::from_utf8(&qdf0).is_err());
        let mut qsc = br#"<scene>
<object type="MolCoord" name="native-raw" src="datachunk:00000" srctype="qdfmol">
</object>
</scene>
========== End of XML ==========
==========START datachunk:00000==========
"#
        .to_vec();
        qsc.extend_from_slice(&qdf0);
        qsc.extend_from_slice(b"\n==========END datachunk:00000==========\n");

        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("qdf0");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .load_qsc_bytes(&qsc)
            .unwrap();
        let loaded = manager
            .scene(scene_uid)
            .unwrap()
            .object_by_name("native-raw")
            .unwrap();
        assert_eq!(loaded.mol().atom_count(), 1);
        assert_eq!(loaded.mol().atoms()[0].position, source.atoms()[0].position);
    }

    #[test]
    fn qsc_animation_only_scene_preserves_unknown_motions() {
        let qsc = r#"<scene>
          <qsc_opts base64="true" compress="xzip" version="QDF1"/>
          <animation length="1200" startcam="cam&amp;one" loop="true">
            <motion type="SimpleSpin" name="spin" start="0" end="1200" angle="90"/>
            <motion type="PluginMotion" name="extra" payload="x&lt;y"/>
          </animation>
        </scene>"#;
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("animation");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .load_qsc_str(qsc)
            .unwrap();
        let animation = manager.scene(scene_uid).unwrap().animation_node().unwrap();
        assert_eq!(animation.str_attr("startcam").as_str(), "cam&one");
        assert_eq!(
            manager
                .scene(scene_uid)
                .unwrap()
                .qsc_options()
                .unwrap()
                .str_attr("compress")
                .as_str(),
            "xzip"
        );
        assert_eq!(
            animation
                .children()
                .iter()
                .filter(|child| child.tag_name() == "motion")
                .count(),
            2
        );

        let rewritten = manager.scene(scene_uid).unwrap().write_qsc_string();
        assert!(rewritten.contains("type=\"PluginMotion\""));
        assert!(rewritten.contains("payload=\"x&lt;y\""));
        assert!(
            rewritten.contains("<qsc_opts base64=\"true\" compress=\"xzip\" version=\"QDF1\"/>")
        );
        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&rewritten)
            .unwrap();
        assert_eq!(
            reloaded
                .scene(reloaded_uid)
                .unwrap()
                .animation_node()
                .unwrap()
                .children()
                .iter()
                .filter(|child| child.tag_name() == "motion")
                .count(),
            2
        );
    }

    #[test]
    fn qsc_scene_styles_round_trip_without_losing_order_or_metadata() {
        let qsc = r#"<scene>
          <styles id="low">
            <style id="LocalBall" bondw="0.31"/>
          </styles>
          <styles id="high" readonly="true">
            <style id="LocalBall" bondw="0.47"/>
          </styles>
        </scene>"#;
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("styles");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .load_qsc_str(qsc)
            .unwrap();

        let scene = manager.scene(scene_uid).unwrap();
        assert_eq!(scene.loaded_styles().len(), 2);
        assert_eq!(scene.loaded_styles()[0].node.str_attr("id").as_str(), "low");
        assert_eq!(
            scene.loaded_styles()[1].node.str_attr("readonly").as_str(),
            "true"
        );
        assert!(scene.loaded_styles()[1].data.contains("bondw=\"0.47\""));

        let rewritten = scene.write_qsc_string();
        assert!(rewritten.find("id=\"low\"").unwrap() < rewritten.find("id=\"high\"").unwrap());
        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("styles-reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&rewritten)
            .unwrap();
        assert_eq!(
            reloaded.scene(reloaded_uid).unwrap().loaded_styles().len(),
            2
        );
    }

    #[test]
    fn qsc_scene_metadata_extensions_and_camera_visibilities_round_trip() {
        let qsc = r#"<scene aoEnabled="true" aa_method="fxaa">
          <plugin-state id="extension" enabled="yes"><entry key="x"/></plugin-state>
          <object type="MolCoord" name="mol" src="datachunk:00000" srctype="qdfpdb">
            <renderer type="simple" name="main"/>
          </object>
          <camera name="saved" zoom="40">
            <visibilities>
              <object target="mol" value="false"/>
              <renderer target="main" type="simple" client="mol" value="true"/>
            </visibilities>
          </camera>
        </scene>
========== End of XML ==========
==========START datachunk:00000==========
ATOM      1  CA  ALA A   1       1.000   2.000   3.000  1.00 10.00
==========END datachunk:00000==========
"#;
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("metadata");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .load_qsc_str(qsc)
            .unwrap();

        let scene = manager.scene(scene_uid).unwrap();
        assert_eq!(scene.scene_attribute("aoEnabled"), Some("true"));
        assert_eq!(scene.scene_attribute("aa_method"), Some("fxaa"));
        assert_eq!(scene.scene_extension_nodes().len(), 1);
        assert_eq!(
            scene.scene_extension_nodes()[0].str_attr("id").as_str(),
            "extension"
        );
        let camera = scene.camera_by_name("saved").unwrap();
        assert_eq!(camera.visibility_size(), 2);
        let object = scene.object_by_name("mol").unwrap();
        let renderer = object.renderer_by_name("main").unwrap();
        let settings = camera.visibility_settings().collect::<Vec<_>>();
        assert!(settings.contains(&(object.uid().get(), false, true)));
        assert!(settings.contains(&(renderer.uid().get(), true, false)));

        let rewritten = scene.write_qsc_string();
        assert!(rewritten.contains("<scene aa_method=\"fxaa\" aoEnabled=\"true\">"));
        assert!(rewritten.contains("<plugin-state id=\"extension\" enabled=\"yes\">"));
        assert!(rewritten.contains("<visibilities>"));
        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("metadata-reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&rewritten)
            .unwrap();
        let reloaded_scene = reloaded.scene(reloaded_uid).unwrap();
        assert_eq!(
            reloaded_scene
                .camera_by_name("saved")
                .unwrap()
                .visibility_size(),
            2
        );
        assert_eq!(reloaded_scene.scene_extension_nodes().len(), 1);
    }

    #[test]
    fn qsc_camera_loads_relative_external_file_then_applies_inline_overrides() {
        let directory = std::env::temp_dir().join(format!(
            "cuemol-qsc-camera-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("saved.xml"),
            r#"<camera name="external" zoom="12" slab="25" center="(1,2,3)"/>"#,
        )
        .unwrap();
        let qsc_path = directory.join("scene.qsc");
        fs::write(
            &qsc_path,
            r#"<scene><camera src="saved.xml" name="inline" zoom="18"/></scene>"#,
        )
        .unwrap();

        let mut manager = MolSceneManager::new();
        let scene_uid = manager.load_qsc_file(&qsc_path).unwrap();
        let camera = manager
            .scene(scene_uid)
            .unwrap()
            .camera_by_name("inline")
            .unwrap();
        assert_eq!(camera.source(), "saved.xml");
        assert_eq!(camera.zoom(), 18.0);
        assert_eq!(camera.slab_depth(), 25.0);
        assert_eq!(camera.center(), Vector4::new3(1.0, 2.0, 3.0));
        assert!(manager
            .scene(scene_uid)
            .unwrap()
            .write_qsc_string()
            .contains("src=\"saved.xml\""));

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn qsc_object_and_renderer_coloring_children_round_trip() {
        let qsc = r#"<scene>
          <object type="MolCoord" name="mol" src="datachunk:00000" srctype="qdfpdb">
            <coloring type="PaintColoring"><paint sel="*" color="red"/></coloring>
            <renderer type="trace" name="main">
              <coloring type="RainbowColoring" start_hue="120" end_hue="30"/>
              <plugin-option id="keep" value="yes"/>
            </renderer>
          </object>
        </scene>
========== End of XML ==========
==========START datachunk:00000==========
ATOM      1  CA  ALA A   1       1.000   2.000   3.000  1.00 10.00
==========END datachunk:00000==========
"#;
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("renderer-children");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .load_qsc_str(qsc)
            .unwrap();
        let object = manager
            .scene(scene_uid)
            .unwrap()
            .object_by_name("mol")
            .unwrap();
        assert_eq!(object.coloring_type(), Some("PaintColoring"));
        assert_eq!(
            object.paint_coloring(),
            &[("*".to_string(), "red".to_string())]
        );
        let renderer = object.renderer_by_name("main").unwrap();
        let styles = StyleManager::new();
        assert_eq!(
            renderer
                .resolved_child_type("coloring", &styles, 0)
                .as_deref(),
            Some("RainbowColoring")
        );
        assert_eq!(
            renderer
                .resolved_child_attr("coloring", "start_hue", &styles, 0)
                .as_deref(),
            Some("120")
        );
        assert_eq!(renderer.qsc_children().len(), 2);

        let rewritten = manager.scene(scene_uid).unwrap().write_qsc_string();
        assert!(rewritten.contains("<coloring type=\"PaintColoring\">"));
        assert!(rewritten.contains("<coloring type=\"RainbowColoring\" start_hue=\"120\""));
        assert!(rewritten.contains("<plugin-option id=\"keep\" value=\"yes\"/>"));
        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("renderer-children-reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&rewritten)
            .unwrap();
        assert_eq!(
            reloaded
                .scene(reloaded_uid)
                .unwrap()
                .object_by_name("mol")
                .unwrap()
                .renderer_by_name("main")
                .unwrap()
                .qsc_children()
                .len(),
            2
        );
    }

    #[test]
    fn script_coloring_source_round_trips_as_child_contents() {
        let qsc = r##"<scene>
          <object type="MolCoord" name="mol" src="datachunk:00000" srctype="qdfpdb">
            <coloring type="ScriptColoring"><script><![CDATA[return cuemol.makeColor("#112233");]]></script></coloring>
            <renderer type="simple" name="main">
              <coloring type="ScriptColoring"><script><![CDATA[return cuemol.makeColor(atom.element === "C" ? "red" : "blue");]]></script></coloring>
            </renderer>
          </object>
        </scene>
========== End of XML ==========
==========START datachunk:00000==========
ATOM      1  CA  ALA A   1       1.000   2.000   3.000  1.00 10.00
==========END datachunk:00000==========
"##;
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("script-coloring");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .load_qsc_str(qsc)
            .unwrap();
        let object = manager
            .scene(scene_uid)
            .unwrap()
            .object_by_name("mol")
            .unwrap();
        assert_eq!(object.coloring_type(), Some("ScriptColoring"));
        assert_eq!(
            object.coloring_attr("script"),
            Some("return cuemol.makeColor(\"#112233\");")
        );
        let renderer_script = object.renderer_by_name("main").unwrap().qsc_children()[0]
            .find_child("script")
            .unwrap()
            .contents();
        assert!(renderer_script.contains("atom.element"));

        let rewritten = manager.scene(scene_uid).unwrap().write_qsc_string();
        assert!(rewritten.contains("<script><![CDATA[return cuemol.makeColor"));
        assert!(!rewritten.contains("script=\""));
        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("script-coloring-reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&rewritten)
            .unwrap();
        assert_eq!(
            reloaded
                .scene(reloaded_uid)
                .unwrap()
                .object_by_name("mol")
                .unwrap()
                .coloring_attr("script"),
            Some("return cuemol.makeColor(\"#112233\");")
        );
    }

    #[test]
    fn unsupported_qsc_object_preserves_dom_and_datachunk() {
        let qsc = r#"<scene>
          <object type="PluginObject" name="opaque" src="datachunk:00000" srctype="plugin-data">
            <renderer type="PluginRenderer" name="custom"><option id="mode" value="x"/></renderer>
          </object>
        </scene>
========== End of XML ==========
==========START datachunk:00000==========
opaque payload
==========END datachunk:00000==========
"#;
        let mut manager = MolSceneManager::new();
        let scene_uid = manager.new_scene("unknown");
        manager
            .scene_mut(scene_uid)
            .unwrap()
            .load_qsc_str(qsc)
            .unwrap();
        let scene = manager.scene(scene_uid).unwrap();
        assert_eq!(
            scene.skipped_qsc_objects(),
            &[("opaque".into(), "PluginObject".into())]
        );
        assert_eq!(scene.loaded_qsc_objects().len(), 1);
        assert!(scene.loaded_qsc_objects()[0]
            .raw_data
            .as_deref()
            .unwrap()
            .starts_with(b"opaque payload"));

        let rewritten = scene.write_qsc_string();
        assert!(rewritten.contains("type=\"PluginObject\""));
        assert!(rewritten.contains("<option id=\"mode\" value=\"x\"/>"));
        assert!(rewritten.contains("opaque payload"));
        let mut reloaded = MolSceneManager::new();
        let reloaded_uid = reloaded.new_scene("unknown-reloaded");
        reloaded
            .scene_mut(reloaded_uid)
            .unwrap()
            .load_qsc_str(&rewritten)
            .unwrap();
        assert_eq!(
            reloaded
                .scene(reloaded_uid)
                .unwrap()
                .loaded_qsc_objects()
                .len(),
            1
        );
    }
}
