use crate::misc::INVALID_UID;
use cuemol_core::{parse_ldom2_xml, LDom2Node};
use cuemol_gfx::{parse_color, SolidColor};
use std::collections::BTreeMap;

/// Bundled default style sheet (C++ `data/default_style.xml`): named renderer
/// style presets (`DefaultBallStick`/`DefaultCPK`/…) plus the X11 named-colour
/// table. Vendored verbatim and parsed once via [`parse_ldom2_xml`].
const DEFAULT_STYLE_XML: &str = include_str!("../data/default_style.xml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleSet {
    uid: u64,
    source: String,
    name: String,
    read_only: bool,
    modified: bool,
    colors: BTreeMap<String, SolidColor>,
    strings: BTreeMap<String, String>,
    styles: BTreeMap<String, LDom2Node>,
}

impl StyleSet {
    pub fn new(uid: u64, name: impl Into<String>) -> Self {
        Self {
            uid,
            source: String::new(),
            name: name.into(),
            read_only: false,
            modified: false,
            colors: BTreeMap::new(),
            strings: BTreeMap::new(),
            styles: BTreeMap::new(),
        }
    }

    pub fn uid(&self) -> u64 {
        self.uid
    }

    pub fn set_uid(&mut self, value: u64) {
        self.uid = value;
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn set_source(&mut self, value: impl Into<String>) {
        self.source = value.into();
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
        self.modified = true;
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn readonly(&self) -> bool {
        self.read_only()
    }

    pub fn set_read_only(&mut self, value: bool) {
        self.read_only = value;
    }

    pub fn set_readonly(&mut self, value: bool) {
        self.set_read_only(value);
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn set_modified(&mut self, value: bool) {
        self.modified = value;
    }

    pub fn get_color(&self, name: &str) -> Option<&SolidColor> {
        self.colors.get(&canonical_key(name))
    }

    pub fn has_color(&self, name: &str) -> bool {
        self.get_color(name).is_some()
    }

    pub fn set_color(&mut self, name: impl AsRef<str>, color: SolidColor) -> bool {
        if self.read_only {
            return false;
        }
        self.colors.insert(canonical_key(name.as_ref()), color);
        self.modified = true;
        true
    }

    pub fn remove_color(&mut self, name: &str) -> bool {
        if self.read_only {
            return false;
        }
        let removed = self.colors.remove(&canonical_key(name)).is_some();
        if removed {
            self.modified = true;
        }
        removed
    }

    pub fn color_defs_json(&self) -> String {
        json_names(self.colors.keys().map(String::as_str))
    }

    pub fn get_color_defs_json(&self) -> String {
        self.color_defs_json()
    }

    pub fn get_color_defs_json_2(&self) -> String {
        self.color_defs_json()
    }

    pub fn get_str_data(&self, category: &str, key: &str) -> String {
        self.strings
            .get(&str_data_key("string", category, key))
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_str_data2(&self, category: &str, key: &str) -> String {
        self.get_str_data(category, key)
    }

    pub fn get_str_data_2(&self, category: &str, key: &str) -> String {
        self.get_str_data(category, key)
    }

    pub fn has_str_data(&self, category: &str, key: &str) -> bool {
        self.strings
            .contains_key(&str_data_key("string", category, key))
    }

    pub fn set_str_data(
        &mut self,
        category: impl AsRef<str>,
        key: impl AsRef<str>,
        value: impl Into<String>,
    ) -> bool {
        if self.read_only {
            return false;
        }
        self.strings.insert(
            str_data_key("string", category.as_ref(), key.as_ref()),
            value.into(),
        );
        self.modified = true;
        true
    }

    pub fn remove_str_data(&mut self, category: &str, key: &str) -> bool {
        if self.read_only {
            return false;
        }
        let removed = self
            .strings
            .remove(&str_data_key("string", category, key))
            .is_some();
        if removed {
            self.modified = true;
        }
        removed
    }

    pub fn str_data_names_json(&self, category: &str) -> String {
        let prefix = format!("string\t{category}\t");
        json_names(
            self.strings
                .keys()
                .filter_map(|key| key.strip_prefix(&prefix)),
        )
    }

    pub fn get_str_data_names_json(&self, category: &str) -> String {
        self.str_data_names_json(category)
    }

    pub fn has_style(&self, name: &str) -> bool {
        self.styles.contains_key(&style_key(name))
    }

    pub fn get_style(&self, name: &str) -> Option<&LDom2Node> {
        self.styles.get(&style_key(name))
    }

    pub fn set_style(&mut self, name: impl AsRef<str>, node: LDom2Node) -> bool {
        if self.read_only {
            return false;
        }
        self.styles.insert(style_key(name.as_ref()), node);
        self.modified = true;
        true
    }

    pub fn remove_style(&mut self, name: &str) -> bool {
        if self.read_only {
            return false;
        }
        let removed = self.styles.remove(&style_key(name)).is_some();
        if removed {
            self.modified = true;
        }
        removed
    }

    pub fn style_names_json(&self) -> String {
        json_names(
            self.styles
                .keys()
                .filter_map(|key| key.strip_suffix("\tstyle")),
        )
    }

    pub fn get_style_names_json(&self) -> String {
        self.style_names_json()
    }

    pub fn get_material_names_json(&self) -> String {
        "[]".to_string()
    }

    pub fn get_material_names_json_2(&self) -> String {
        self.get_material_names_json()
    }

    pub fn make_style_key(name: &str) -> String {
        style_key(name)
    }

    pub fn make_str_data_key(db: &str, category: &str, key: &str) -> String {
        str_data_key(db, category, key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleManager {
    default_dir: String,
    next_uid: u64,
    scopes: BTreeMap<u64, Vec<StyleSet>>,
    pending_events: Vec<(u64, String)>,
}

impl Default for StyleManager {
    fn default() -> Self {
        Self {
            default_dir: String::new(),
            next_uid: 1,
            scopes: BTreeMap::new(),
            pending_events: Vec::new(),
        }
    }
}

impl StyleManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_dir(&self) -> &str {
        &self.default_dir
    }

    pub fn set_default_dir(&mut self, value: impl Into<String>) {
        self.default_dir = value.into();
    }

    pub fn create_style_set(&mut self, name: impl Into<String>, scope_id: u64) -> u64 {
        let uid = self.alloc_uid();
        let set = StyleSet::new(uid, name);
        self.scopes.entry(scope_id).or_default().push(set);
        uid
    }

    pub fn has_style_set(&self, name: &str, scope_id: u64) -> u64 {
        self.scopes
            .get(&scope_id)
            .and_then(|sets| sets.iter().find(|set| set.name() == name))
            .map(StyleSet::uid)
            .unwrap_or(INVALID_UID)
    }

    pub fn destroy_style_set(&mut self, scope_id: u64, style_set_id: u64) -> bool {
        let Some(sets) = self.scopes.get_mut(&scope_id) else {
            return false;
        };
        let before = sets.len();
        sets.retain(|set| set.uid() != style_set_id);
        before != sets.len()
    }

    pub fn register_style_set(&mut self, mut set: StyleSet, before: isize, scope_id: u64) -> bool {
        if self.has_style_set(set.name(), scope_id) != INVALID_UID {
            return false;
        }
        if set.uid() == INVALID_UID {
            set.set_uid(self.alloc_uid());
        } else {
            self.next_uid = self.next_uid.max(set.uid() + 1);
        }

        let sets = self.scopes.entry(scope_id).or_default();
        if before < 0 || before as usize >= sets.len() {
            sets.push(set);
        } else {
            sets.insert(before as usize, set);
        }
        true
    }

    pub fn get_style_set(&self, style_set_id: u64) -> Option<&StyleSet> {
        self.scopes
            .values()
            .flat_map(|sets| sets.iter())
            .find(|set| set.uid() == style_set_id)
    }

    pub fn get_style_set_mut(&mut self, style_set_id: u64) -> Option<&mut StyleSet> {
        self.scopes
            .values_mut()
            .flat_map(|sets| sets.iter_mut())
            .find(|set| set.uid() == style_set_id)
    }

    pub fn style_set_source(&self, style_set_id: u64) -> String {
        self.get_style_set(style_set_id)
            .map(|set| set.source().to_string())
            .unwrap_or_default()
    }

    pub fn get_style_set_source(&self, style_set_id: u64) -> String {
        self.style_set_source(style_set_id)
    }

    pub fn style_sets_json(&self, scope_id: u64) -> String {
        let names = self
            .scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .map(StyleSet::name);
        json_names(names)
    }

    pub fn get_style_sets_json(&self, scope_id: u64) -> String {
        self.style_sets_json(scope_id)
    }

    pub fn get_color(&self, name: &str, scope_id: u64) -> Option<&SolidColor> {
        if let Some(color) = self
            .scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .find_map(|set| set.get_color(name))
        {
            return Some(color);
        }
        if scope_id == INVALID_UID {
            return None;
        }
        self.scopes
            .get(&INVALID_UID)
            .into_iter()
            .flatten()
            .find_map(|set| set.get_color(name))
    }

    pub fn get_color_from_set(&self, name: &str, style_set_id: u64) -> Option<&SolidColor> {
        self.get_style_set(style_set_id)
            .and_then(|set| set.get_color(name))
    }

    pub fn set_color(
        &mut self,
        key: impl AsRef<str>,
        value: SolidColor,
        _scope_id: u64,
        style_set_id: u64,
    ) -> bool {
        self.get_style_set_mut(style_set_id)
            .is_some_and(|set| set.set_color(key, value))
    }

    pub fn remove_color(&mut self, key: &str, _scope_id: u64, style_set_id: u64) -> bool {
        self.get_style_set_mut(style_set_id)
            .is_some_and(|set| set.remove_color(key))
    }

    pub fn color_defs_json(&self, scope_id: u64) -> String {
        let names = self
            .scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .flat_map(|set| set.colors.keys().map(String::as_str));
        json_names(names)
    }

    pub fn get_color_defs_json(&self, scope_id: u64) -> String {
        self.color_defs_json(scope_id)
    }

    pub fn get_color_defs_json2(&self, scope_id: u64) -> String {
        self.color_defs_json(scope_id)
    }

    pub fn get_color_defs_json_2(&self, scope_id: u64) -> String {
        self.color_defs_json(scope_id)
    }

    pub fn get_str_data(&self, category: &str, key: &str, scope_id: u64) -> String {
        self.find_in_scope(scope_id, |set| {
            set.has_str_data(category, key)
                .then(|| set.get_str_data(category, key))
        })
        .unwrap_or_default()
    }

    pub fn set_str_data(
        &mut self,
        category: impl AsRef<str>,
        key: impl AsRef<str>,
        value: impl Into<String>,
        _scope_id: u64,
        style_set_id: u64,
    ) -> bool {
        self.get_style_set_mut(style_set_id)
            .is_some_and(|set| set.set_str_data(category.as_ref(), key.as_ref(), value.into()))
    }

    pub fn remove_str_data(
        &mut self,
        category: &str,
        key: &str,
        _scope_id: u64,
        style_set_id: u64,
    ) -> bool {
        self.get_style_set_mut(style_set_id)
            .is_some_and(|set| set.remove_str_data(category, key))
    }

    pub fn str_data_defs_json(&self, category: &str, scope_id: u64) -> String {
        let prefix = format!("string\t{category}\t");
        let names = self
            .scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .flat_map(|set| set.strings.keys())
            .filter_map(|key| key.strip_prefix(&prefix));
        json_names(names)
    }

    pub fn get_str_data_defs_json(&self, category: &str, scope_id: u64) -> String {
        self.str_data_defs_json(category, scope_id)
    }

    pub fn get_str_data_defs_json2(&self, category: &str, scope_id: u64) -> String {
        self.str_data_defs_json(category, scope_id)
    }

    pub fn get_str_data_defs_json_2(&self, category: &str, scope_id: u64) -> String {
        self.str_data_defs_json(category, scope_id)
    }

    pub fn style_names_json(&self, scope_id: u64) -> String {
        let names = self
            .scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .flat_map(|set| {
                set.styles
                    .keys()
                    .filter_map(|key| key.strip_suffix("\tstyle"))
            });
        json_names(names)
    }

    pub fn get_style_names_json(&self, scope_id: u64) -> String {
        self.style_names_json(scope_id)
    }

    pub fn get_material_names_json(&self, _scope_id: u64) -> String {
        "[]".to_string()
    }

    pub fn get_material_names_json2(&self, scope_id: u64) -> String {
        self.get_material_names_json(scope_id)
    }

    pub fn get_material_names_json_2(&self, scope_id: u64) -> String {
        self.get_material_names_json(scope_id)
    }

    pub fn create_style_from_obj(&mut self, name: impl Into<String>, scope_id: u64) -> u64 {
        self.create_style_set(name, scope_id)
    }

    pub fn compile_color(&self, rep: &str, _scope_id: u64) -> SolidColor {
        parse_color(rep)
            .ok()
            .and_then(|spec| spec.rgba8())
            .map(SolidColor::from)
            .unwrap_or_default()
    }

    pub fn save_style_set_to_file(
        &self,
        _scope_id: u64,
        style_set_id: u64,
        path_name: impl AsRef<std::path::Path>,
    ) -> bool {
        let Some(set) = self.get_style_set(style_set_id) else {
            return false;
        };
        std::fs::write(path_name, set.color_defs_json()).is_ok()
    }

    pub fn load_style_set_from_file(
        &mut self,
        scope_id: u64,
        path_name: impl AsRef<std::path::Path>,
        read_only: bool,
    ) -> u64 {
        let Ok(content) = std::fs::read_to_string(&path_name) else {
            return INVALID_UID;
        };
        let name = path_name
            .as_ref()
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("style")
            .to_string();
        let uid = self.create_style_set(name, scope_id);
        if let Some(set) = self.get_style_set_mut(uid) {
            populate_style_set_from_xml(set, &content);
            set.set_source(path_name.as_ref().to_string_lossy());
            set.set_read_only(read_only);
            set.set_modified(false);
        }
        uid
    }

    /// Register the bundled `default_style.xml` (C++ `StyleMgr` loads it at
    /// startup into the system scope) as a read-only style set in `scope_id`.
    /// Returns its style-set UID. The presets it defines become resolvable via
    /// [`StyleManager::style_attr`]/[`StyleManager::resolve_style_attr`].
    pub fn load_builtin_default_styles(&mut self, scope_id: u64) -> u64 {
        let uid = self.create_style_set("system", scope_id);
        if let Some(set) = self.get_style_set_mut(uid) {
            populate_style_set_from_xml(set, DEFAULT_STYLE_XML);
            set.set_source("<builtin>/default_style.xml");
            set.set_read_only(true);
            set.set_modified(false);
        }
        uid
    }

    /// Look up a single style preset's attribute (e.g. `DefaultBallStick` /
    /// `bondw`) by scanning the style sets in `scope_id`, then the global scope
    /// `0` (C++ `StyleMgr` cascades scene scope → `m_pGlob`). Reads the top-level
    /// `<style>` XML attribute; returns `None` when the style or attribute is
    /// absent (or empty). Nested child elements (`<helix>`/`<coloring>`) are not
    /// flattened here — only the directly-attached presets used for geometry.
    pub fn style_attr(&self, scope_id: u64, style_id: &str, attr: &str) -> Option<String> {
        let lookup = |scope: u64| -> Option<String> {
            self.scopes.get(&scope)?.iter().find_map(|set| {
                let value = set.get_style(style_id)?.str_attr(attr).to_string();
                (!value.is_empty()).then_some(value)
            })
        };
        lookup(scope_id).or_else(|| (scope_id != 0).then(|| lookup(0)).flatten())
    }

    /// Resolve a renderer `style="A,B,C"` list for one attribute, mirroring C++
    /// `applyStyles`: presets apply left-to-right so a later one overrides an
    /// earlier one — the LAST style in the list that defines `attr` wins.
    pub fn resolve_style_attr(
        &self,
        scope_id: u64,
        style_list: &str,
        attr: &str,
    ) -> Option<String> {
        let mut result = None;
        for name in style_list.split(',') {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            if let Some(value) = self.style_attr(scope_id, name, attr) {
                result = Some(value);
            }
        }
        result
    }

    /// Like [`StyleManager::style_attr`] but for an attribute on a nested child
    /// element of the preset (e.g. `DefaultRibbon` → `<helix tuber="6.0">`).
    /// Used for the ribbon cross-section sub-elements `<helix>/<sheet>/<coil>`.
    pub fn style_child_attr(
        &self,
        scope_id: u64,
        style_id: &str,
        child_tag: &str,
        attr: &str,
    ) -> Option<String> {
        let lookup = |scope: u64| -> Option<String> {
            self.scopes.get(&scope)?.iter().find_map(|set| {
                let value = set
                    .get_style(style_id)?
                    .find_child(child_tag)?
                    .str_attr(attr)
                    .to_string();
                (!value.is_empty()).then_some(value)
            })
        };
        lookup(scope_id).or_else(|| (scope_id != 0).then(|| lookup(0)).flatten())
    }

    /// Resolve a `style="A,B"` list for a nested child-element attribute (later
    /// style wins), the nested counterpart of [`StyleManager::resolve_style_attr`].
    pub fn resolve_style_child_attr(
        &self,
        scope_id: u64,
        style_list: &str,
        child_tag: &str,
        attr: &str,
    ) -> Option<String> {
        let mut result = None;
        for name in style_list.split(',') {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            if let Some(value) = self.style_child_attr(scope_id, name, child_tag, attr) {
                result = Some(value);
            }
        }
        result
    }

    pub fn get_style_value(&self, scope_id: u64, set_name: &str, key: &str) -> String {
        let (style_name, prop_name) = split_style_key(key);
        self.scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .filter(|set| set_name.is_empty() || set.name() == set_name)
            .find_map(|set| {
                set.get_style(&style_name)
                    .and_then(|node| find_style_value(node, &prop_name))
            })
            .unwrap_or_default()
    }

    pub fn set_style_value(
        &mut self,
        scope_id: u64,
        set_name: &str,
        key: &str,
        value: impl Into<String>,
    ) {
        let value = value.into();
        let (style_name, prop_name) = split_style_key(key);
        let Some(set) = self
            .scopes
            .get_mut(&scope_id)
            .into_iter()
            .flatten()
            .find(|set| set_name.is_empty() || set.name() == set_name)
        else {
            return;
        };

        let style_key = style_key(&style_name);
        let node = set.styles.entry(style_key).or_insert_with(|| {
            let mut node = LDom2Node::new();
            node.set_tag_name("style");
            node
        });
        set_style_value_node(node, &prop_name, value);
        set.modified = true;
        self.pending_events.push((scope_id, style_name));
    }

    pub fn pending_event_count(&self) -> usize {
        self.pending_events.len()
    }

    pub fn fire_pending_events(&mut self) {
        self.pending_events.clear();
    }

    fn find_in_scope<T>(
        &self,
        scope_id: u64,
        mut finder: impl FnMut(&StyleSet) -> Option<T>,
    ) -> Option<T> {
        self.scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .find_map(&mut finder)
    }

    fn alloc_uid(&mut self) -> u64 {
        let uid = self.next_uid;
        self.next_uid += 1;
        uid
    }
}

fn canonical_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

fn str_data_key(db: &str, category: &str, key: &str) -> String {
    format!("{db}\t{category}\t{key}")
}

fn style_key(name: &str) -> String {
    format!("{name}\tstyle")
}

fn split_style_key(key: &str) -> (String, String) {
    match key.split_once('.') {
        Some((style, prop)) => (style.to_string(), prop.to_string()),
        None => (key.to_string(), String::new()),
    }
}

fn find_style_value(node: &LDom2Node, prop_name: &str) -> Option<String> {
    if prop_name.is_empty() {
        return Some(node.value().to_string());
    }
    let mut current = node;
    for part in prop_name.split('.') {
        current = current.find_child(part)?;
    }
    Some(current.value().to_string())
}

/// Parse an LDom2 style XML document (`<styles>`) into a [`StyleSet`], mirroring
/// C++ `StyleSet::readStyleSetFromStream`: each `<style id=...>` preset is stored
/// by id (keeping its full attribute/child node for later resolution) and each
/// `<color id=... value=...>` entry populates the named-colour table.
///
/// Intentionally NOT parsed (categorical sinks deferred elsewhere): `<material>`
/// and `<path>` feed ICC colour-management / external-renderer preamble config,
/// and `<sel>`/`<setting>` are scene preferences — none affect rendered geometry.
/// Returns the number of styles registered (0 if the document fails to parse).
fn populate_style_set_from_xml(set: &mut StyleSet, xml: &str) -> usize {
    let Ok(tree) = parse_ldom2_xml(xml) else {
        return 0;
    };
    let mut style_count = 0;
    for child in tree.top().children() {
        if child.is_attr() {
            continue;
        }
        match child.tag_name() {
            "style" => {
                let id = child.str_attr("id").to_string();
                if !id.is_empty() && set.set_style(id, child.clone()) {
                    style_count += 1;
                }
            }
            "color" => {
                let id = child.str_attr("id").to_string();
                // The `value` attribute is reserved by the LDom2 parser and
                // surfaces as the node value, not a child attribute.
                let value = child.value().to_string();
                if id.is_empty() || value.is_empty() {
                    continue;
                }
                if let Some(rgba) = parse_color(&value).ok().and_then(|spec| spec.rgba8()) {
                    set.set_color(id, SolidColor::from(rgba));
                }
            }
            _ => {}
        }
    }
    style_count
}

fn set_style_value_node(node: &mut LDom2Node, prop_name: &str, value: String) {
    if prop_name.is_empty() {
        node.set_value(value);
        return;
    }
    let mut current = node;
    for part in prop_name.split('.') {
        if current.find_child(part).is_none() {
            current.append_child(part);
        }
        current = current.find_child_mut(part).unwrap();
    }
    current.set_value(value);
}

fn json_names<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let values = names
        .into_iter()
        .map(|name| format!("\"{}\"", escape_json(name)))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(","))
}

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_set_color_string_and_style_tables_match_reference_contract() {
        let mut set = StyleSet::new(7, "Default");
        assert_eq!(set.uid(), 7);
        assert_eq!(set.name(), "Default");
        assert!(!set.read_only());
        assert!(!set.modified());

        assert!(set.set_color("MolCol", SolidColor::from_rgb(1.0, 0.0, 0.0)));
        assert!(set.has_color("molcol"));
        assert!(set.color_defs_json().contains("\"molcol\""));
        assert!(set.modified());
        set.set_modified(false);
        assert!(set.remove_color("MOLCOL"));
        assert!(!set.has_color("molcol"));
        assert!(set.modified());

        assert!(set.set_str_data("sel", "protein", "resn ALA"));
        assert!(set.has_str_data("sel", "protein"));
        assert_eq!(set.get_str_data("sel", "protein"), "resn ALA");
        assert_eq!(set.str_data_names_json("sel"), "[\"protein\"]");
        assert!(set.remove_str_data("sel", "protein"));
        assert_eq!(set.get_str_data("sel", "protein"), "");

        let mut node = LDom2Node::new();
        node.set_tag_name("style");
        node.set_value("root");
        assert!(set.set_style("Ribbon", node));
        assert!(set.has_style("Ribbon"));
        assert_eq!(set.style_names_json(), "[\"Ribbon\"]");
        assert!(set.remove_style("Ribbon"));
        assert!(!set.has_style("Ribbon"));
    }

    #[test]
    fn readonly_style_set_rejects_mutations() {
        let mut set = StyleSet::new(1, "ro");
        set.set_read_only(true);
        assert!(!set.set_color("x", SolidColor::from_rgb(1.0, 0.0, 0.0)));
        assert!(!set.set_str_data("cat", "key", "value"));
        assert!(!set.set_style("style", LDom2Node::new()));
    }

    #[test]
    fn style_manager_registers_searches_and_edits_sets() {
        let mut manager = StyleManager::new();
        manager.set_default_dir("/tmp/styles");
        assert_eq!(manager.default_dir(), "/tmp/styles");

        let uid = manager.create_style_set("Default", 10);
        assert_ne!(uid, INVALID_UID);
        assert_eq!(manager.has_style_set("Default", 10), uid);
        assert_eq!(manager.style_sets_json(10), "[\"Default\"]");

        assert!(manager.set_color("molcol", SolidColor::from_rgb(0.0, 1.0, 0.0), 10, uid));
        assert!(manager.get_color("molcol", 10).is_some());
        assert_eq!(manager.color_defs_json(10), "[\"molcol\"]");

        assert!(manager.set_str_data("sel", "all", "*", 10, uid));
        assert_eq!(manager.get_str_data("sel", "all", 10), "*");
        assert_eq!(manager.str_data_defs_json("sel", 10), "[\"all\"]");

        manager.set_style_value(10, "Default", "Ribbon.line_width", "1.5");
        assert_eq!(manager.pending_event_count(), 1);
        assert_eq!(
            manager.get_style_value(10, "Default", "Ribbon.line_width"),
            "1.5"
        );
        assert_eq!(manager.style_names_json(10), "[\"Ribbon\"]");
        manager.fire_pending_events();
        assert_eq!(manager.pending_event_count(), 0);

        assert!(manager.destroy_style_set(10, uid));
        assert_eq!(manager.has_style_set("Default", 10), INVALID_UID);
    }

    #[test]
    fn style_manager_registration_rejects_duplicate_names() {
        let mut manager = StyleManager::new();
        let first = StyleSet::new(100, "Default");
        let duplicate = StyleSet::new(101, "Default");
        assert!(manager.register_style_set(first, -1, 1));
        assert!(!manager.register_style_set(duplicate, -1, 1));
        assert_eq!(manager.has_style_set("Default", 1), 100);
    }

    #[test]
    fn builtin_default_styles_parse_presets_and_named_colors() {
        let mut manager = StyleManager::new();
        let uid = manager.load_builtin_default_styles(0);
        assert_ne!(uid, INVALID_UID);

        // The 47 `<style>` presets are registered and read-only.
        let set = manager.get_style_set(uid).unwrap();
        assert!(set.read_only());
        assert!(set.has_style("DefaultBallStick"));
        assert!(set.has_style("DefaultCPK"));

        // Geometry attributes parse from the `<style>` element attributes.
        assert_eq!(manager.style_attr(0, "DefaultBallStick", "bondw").as_deref(), Some("0.2"));
        assert_eq!(manager.style_attr(0, "DefaultBallStick", "sphr").as_deref(), Some("0.3"));
        assert_eq!(manager.style_attr(0, "ThickBallStick", "bondw").as_deref(), Some("0.3"));
        assert_eq!(manager.style_attr(0, "ThickBallStick", "sphr").as_deref(), Some("0.5"));
        assert_eq!(manager.style_attr(0, "DefaultCPK", "detail").as_deref(), Some("10"));
        // An undefined attribute on a real style is None, not empty string.
        assert!(manager.style_attr(0, "DefaultCPK", "bondw").is_none());
        // An unknown style is None.
        assert!(manager.style_attr(0, "NoSuchStyle", "bondw").is_none());

        // The X11 named-colour table is populated (`<color id=.. value=..>`).
        assert!(set.has_color("Yellow"));
        assert!(set.has_color("SlateBlue"));
    }

    #[test]
    fn resolve_style_attr_walks_the_comma_list_last_wins() {
        let mut manager = StyleManager::new();
        manager.load_builtin_default_styles(0);

        // `DefaultLabel` carries color/font; `DefaultAtomIntr` carries width —
        // the two contribute disjoint attributes, both resolvable from the list.
        assert_eq!(
            manager.resolve_style_attr(0, "DefaultLabel,DefaultAtomIntr", "width").as_deref(),
            Some("1.0")
        );
        assert_eq!(
            manager.resolve_style_attr(0, "DefaultLabel,DefaultAtomIntr", "color").as_deref(),
            Some("Yellow")
        );
        // Later style overrides earlier when both define an attribute:
        // DashedStick1AtomIntr.width=0.1 wins over DefaultAtomIntr.width=1.0.
        assert_eq!(
            manager
                .resolve_style_attr(0, "DefaultAtomIntr,DashedStick1AtomIntr", "width")
                .as_deref(),
            Some("0.1")
        );
        // Whitespace tolerance + unknown names skipped.
        assert_eq!(
            manager.resolve_style_attr(0, " DefaultBallStick , ", "bondw").as_deref(),
            Some("0.2")
        );
        assert!(manager.resolve_style_attr(0, "", "bondw").is_none());
    }

    #[test]
    fn style_child_attr_reads_nested_ribbon_section_elements() {
        let mut manager = StyleManager::new();
        manager.load_builtin_default_styles(0);

        // DefaultRibbon's nested <helix tuber="6.0" width="0.2"/> and
        // <sheet tuber="7.0" width="0.2"/> cross-section sub-elements.
        assert_eq!(
            manager.style_child_attr(0, "DefaultRibbon", "helix", "tuber").as_deref(),
            Some("6.0")
        );
        assert_eq!(
            manager.style_child_attr(0, "DefaultRibbon", "helix", "width").as_deref(),
            Some("0.2")
        );
        assert_eq!(
            manager.style_child_attr(0, "DefaultRibbon", "sheet", "tuber").as_deref(),
            Some("7.0")
        );
        // No <coil> sub-element in DefaultRibbon.
        assert!(manager.style_child_attr(0, "DefaultRibbon", "coil", "width").is_none());
        // Resolve across a style list (later style wins): RoundRibbon's helix is
        // elliptical with the same tuber=6.0.
        assert_eq!(
            manager
                .resolve_style_child_attr(0, "DefaultRibbon,RoundRibbon", "helix", "type")
                .as_deref(),
            Some("elliptical")
        );
    }

    #[test]
    fn load_style_set_from_file_parses_a_real_style_document() {
        // The stubbed loader used to only check the file exists; it now parses.
        let dir = std::env::temp_dir().join("cuemol_style_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("custom_style.xml");
        std::fs::write(
            &path,
            r##"<?xml version="1.0"?>
<styles id="custom">
  <style id="MyBallStick" type="renderer" bondw="0.42" sphr="0.7"/>
  <color id="Crimson" value="#DC143C"/>
</styles>"##,
        )
        .unwrap();

        let mut manager = StyleManager::new();
        let uid = manager.load_style_set_from_file(5, &path, true);
        assert_ne!(uid, INVALID_UID);
        assert_eq!(manager.style_attr(5, "MyBallStick", "bondw").as_deref(), Some("0.42"));
        assert!(manager.get_style_set(uid).unwrap().has_color("Crimson"));

        std::fs::remove_file(&path).ok();
    }
}
