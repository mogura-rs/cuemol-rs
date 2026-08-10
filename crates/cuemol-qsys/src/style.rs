use crate::{
    misc::{Renderer, INVALID_UID},
    SysConfig,
};
use cuemol_core::{parse_ldom2_xml, LDom2Node};
use cuemol_gfx::{parse_color, set_named_color_resolver, ColorPtr, NamedColorResolver, SolidColor};
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, RwLock,
};

/// Bundled default style sheet (C++ `data/default_style.xml`): named renderer
/// style presets (`DefaultBallStick`/`DefaultCPK`/…) plus the X11 named-colour
/// table. Vendored verbatim and parsed once via [`parse_ldom2_xml`].
const DEFAULT_STYLE_XML: &str = include_str!("../data/default_style.xml");

pub const MATERIAL_AMBIENT: usize = 0;
pub const MATERIAL_DIFFUSE: usize = 1;
pub const MATERIAL_SPECULAR: usize = 2;
pub const MATERIAL_SHININESS: usize = 3;
pub const MATERIAL_EMISSION: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct StyleMaterial {
    system_values: [f64; 5],
    has_system_values: bool,
    dependent_values: BTreeMap<String, String>,
}

impl Default for StyleMaterial {
    fn default() -> Self {
        Self {
            system_values: [0.2, 0.8, 0.0, 0.0, 0.0],
            has_system_values: false,
            dependent_values: BTreeMap::new(),
        }
    }
}

impl StyleMaterial {
    pub fn set_system_value(&mut self, parameter: usize, value: f64) -> bool {
        let Some(slot) = self.system_values.get_mut(parameter) else {
            return false;
        };
        *slot = value;
        self.has_system_values = true;
        true
    }

    pub fn system_value(&self, parameter: usize) -> Option<f64> {
        self.has_system_values
            .then(|| self.system_values.get(parameter).copied())
            .flatten()
    }

    pub fn has_system_values(&self) -> bool {
        self.has_system_values
    }

    pub fn set_dependent_value(
        &mut self,
        renderer_type: impl Into<String>,
        value: impl Into<String>,
    ) {
        self.dependent_values
            .insert(renderer_type.into(), value.into());
    }

    pub fn dependent_value(&self, renderer_type: &str) -> Option<&str> {
        self.dependent_values.get(renderer_type).map(String::as_str)
    }

    pub fn dependent_values(&self) -> &BTreeMap<String, String> {
        &self.dependent_values
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyleSet {
    uid: u64,
    source: String,
    name: String,
    read_only: bool,
    modified: bool,
    colors: BTreeMap<String, SolidColor>,
    strings: BTreeMap<String, String>,
    styles: BTreeMap<String, LDom2Node>,
    materials: BTreeMap<String, StyleMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleInfo {
    pub name: String,
    pub description: String,
    pub style_type: String,
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
            materials: BTreeMap::new(),
        }
    }

    pub fn uid(&self) -> u64 {
        self.uid
    }

    pub fn get_uid(&self) -> u64 {
        self.uid()
    }

    pub fn set_uid(&mut self, value: u64) {
        self.uid = value;
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn get_source(&self) -> &str {
        self.source()
    }

    pub fn src(&self) -> &str {
        self.source()
    }

    pub fn set_source(&mut self, value: impl Into<String>) {
        self.source = value.into();
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
        self.modified = true;
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only()
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

    pub fn is_modified(&self) -> bool {
        self.modified()
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
        let inserted = self
            .colors
            .insert(canonical_key(name.as_ref()), color)
            .is_none();
        self.modified = true;
        inserted
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
        let inserted = self
            .strings
            .insert(
                str_data_key("string", category.as_ref(), key.as_ref()),
                value.into(),
            )
            .is_none();
        self.modified = true;
        inserted
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

    pub fn get_path(&self, key: &str) -> String {
        let value = self.get_str_data("path", key);
        if value.is_empty() {
            return value;
        }
        SysConfig::global()
            .lock()
            .expect("SysConfig lock poisoned")
            .conv_path_name(&value)
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
        let key = style_key(name.as_ref());
        if self.styles.contains_key(&key) {
            return false;
        }
        self.styles.insert(key, node);
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
        format!("[{}]", style_info_entries(self).join(","))
    }

    pub fn get_style_names_json(&self) -> String {
        self.style_names_json()
    }

    pub fn style_infos(&self) -> Vec<StyleInfo> {
        self.styles
            .iter()
            .filter_map(|(key, node)| {
                let name = key.strip_suffix("\tstyle")?;
                let style_type = if node.type_name().is_empty() {
                    node.str_attr("type").to_string()
                } else {
                    node.type_name().to_string()
                };
                Some(StyleInfo {
                    name: name.to_string(),
                    description: node.str_attr("desc").to_string(),
                    style_type,
                })
            })
            .collect()
    }

    pub fn put_material_string(
        &mut self,
        id: impl Into<String>,
        renderer_type: impl Into<String>,
        value: impl Into<String>,
    ) -> bool {
        if self.read_only {
            return false;
        }
        self.materials
            .entry(id.into())
            .or_default()
            .set_dependent_value(renderer_type, value);
        self.modified = true;
        true
    }

    pub fn put_material_number(
        &mut self,
        id: impl Into<String>,
        parameter: usize,
        value: f64,
    ) -> bool {
        if self.read_only {
            return false;
        }
        let changed = self
            .materials
            .entry(id.into())
            .or_default()
            .set_system_value(parameter, value);
        self.modified |= changed;
        changed
    }

    pub fn get_material(&self, id: &str) -> Option<&StyleMaterial> {
        self.materials.get(id)
    }

    pub fn get_material_names_json(&self) -> String {
        json_names(self.materials.keys().map(String::as_str))
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

#[derive(Debug, Clone, PartialEq)]
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

    pub fn get_default_dir(&self) -> &str {
        self.default_dir()
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
        self.take_style_set(scope_id, style_set_id).is_some()
    }

    pub fn clear_scope(&mut self, scope_id: u64) {
        self.scopes.remove(&scope_id);
        self.pending_events
            .retain(|(event_scope, _)| *event_scope != scope_id);
    }

    /// Remove and return a style set and its insertion index for undo/redo.
    pub fn take_style_set(
        &mut self,
        scope_id: u64,
        style_set_id: u64,
    ) -> Option<(StyleSet, usize)> {
        let sets = self.scopes.get_mut(&scope_id)?;
        let index = sets.iter().position(|set| set.uid() == style_set_id)?;
        Some((sets.remove(index), index))
    }

    pub fn style_set_index(&self, scope_id: u64, style_set_id: u64) -> Option<usize> {
        self.scopes
            .get(&scope_id)?
            .iter()
            .position(|set| set.uid() == style_set_id)
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

    pub fn style_sets(&self, scope_id: u64) -> &[StyleSet] {
        self.scopes.get(&scope_id).map(Vec::as_slice).unwrap_or(&[])
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

    pub fn get_color2(&self, name: &str, scope_id: u64, style_set_id: u64) -> Option<&SolidColor> {
        self.scopes
            .get(&scope_id)?
            .iter()
            .find(|set| set.uid() == style_set_id)?
            .get_color(name)
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

    pub fn get_color_defs_json2(&self, scope_id: u64, style_set_id: u64) -> String {
        self.scopes
            .get(&scope_id)
            .and_then(|sets| sets.iter().find(|set| set.uid() == style_set_id))
            .map_or_else(|| "[]".to_string(), StyleSet::color_defs_json)
    }

    pub fn get_color_defs_json_2(&self, scope_id: u64, style_set_id: u64) -> String {
        self.get_color_defs_json2(scope_id, style_set_id)
    }

    pub fn get_str_data(&self, category: &str, key: &str, scope_id: u64) -> String {
        let lookup = |scope_id| {
            self.find_in_scope(scope_id, |set| {
                set.has_str_data(category, key)
                    .then(|| set.get_str_data(category, key))
            })
        };
        lookup(scope_id)
            .or_else(|| (scope_id != 0).then(|| lookup(0)).flatten())
            .unwrap_or_default()
    }

    pub fn str_data_names(&self, category: &str, scope_id: u64) -> Vec<String> {
        let prefix = format!("string\t{category}\t");
        let mut names = std::collections::BTreeSet::new();
        for scope in [0, scope_id] {
            names.extend(
                self.scopes
                    .get(&scope)
                    .into_iter()
                    .flatten()
                    .flat_map(|set| set.strings.keys())
                    .filter_map(|key| key.strip_prefix(&prefix))
                    .map(str::to_string),
            );
        }
        names.into_iter().collect()
    }

    pub fn str_data_entries(&self, category: &str, scope_id: u64) -> BTreeMap<String, String> {
        self.str_data_names(category, scope_id)
            .into_iter()
            .map(|name| {
                let value = self.get_str_data(category, &name, scope_id);
                (name, value)
            })
            .collect()
    }

    pub fn get_str_data2(
        &self,
        category: &str,
        key: &str,
        scope_id: u64,
        style_set_id: u64,
    ) -> String {
        self.scopes
            .get(&scope_id)
            .and_then(|sets| sets.iter().find(|set| set.uid() == style_set_id))
            .map(|set| set.get_str_data(category, key))
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

    pub fn get_str_data_defs_json2(
        &self,
        category: &str,
        scope_id: u64,
        style_set_id: u64,
    ) -> String {
        self.scopes
            .get(&scope_id)
            .and_then(|sets| sets.iter().find(|set| set.uid() == style_set_id))
            .map_or_else(|| "[]".to_string(), |set| set.str_data_names_json(category))
    }

    pub fn get_str_data_defs_json_2(
        &self,
        category: &str,
        scope_id: u64,
        style_set_id: u64,
    ) -> String {
        self.get_str_data_defs_json2(category, scope_id, style_set_id)
    }

    pub fn style_names_json(&self, scope_id: u64) -> String {
        let entries = self
            .scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .flat_map(style_info_entries)
            .collect::<Vec<_>>();
        format!("[{}]", entries.join(","))
    }

    pub fn get_style_names_json(&self, scope_id: u64) -> String {
        self.style_names_json(scope_id)
    }

    pub fn style_infos(&self, scope_id: u64) -> Vec<StyleInfo> {
        self.scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .flat_map(StyleSet::style_infos)
            .collect()
    }

    pub fn style_node(&self, scope_id: u64, style_id: &str) -> Option<&LDom2Node> {
        let lookup = |scope: u64| {
            self.scopes
                .get(&scope)?
                .iter()
                .find_map(|set| set.get_style(style_id))
        };
        lookup(scope_id).or_else(|| (scope_id != 0).then(|| lookup(0)).flatten())
    }

    pub fn get_material_string(
        &self,
        material_id: &str,
        renderer_type: &str,
        scope_id: u64,
    ) -> String {
        self.find_in_scope(scope_id, |set| {
            set.get_material(material_id)?
                .dependent_value(renderer_type)
                .map(str::to_string)
        })
        .or_else(|| {
            (scope_id != 0).then(|| {
                self.find_in_scope(0, |set| {
                    set.get_material(material_id)?
                        .dependent_value(renderer_type)
                        .map(str::to_string)
                })
            })?
        })
        .unwrap_or_default()
    }

    pub fn get_material_number(&self, material_id: &str, parameter: usize, scope_id: u64) -> f64 {
        self.find_in_scope(scope_id, |set| {
            set.get_material(material_id)?.system_value(parameter)
        })
        .or_else(|| {
            (scope_id != 0).then(|| {
                self.find_in_scope(0, |set| {
                    set.get_material(material_id)?.system_value(parameter)
                })
            })?
        })
        .unwrap_or(-1.0)
    }

    pub fn get_config(&self, renderer_type: &str, setting_name: &str, scope_id: u64) -> String {
        let key = str_data_key("cfg", setting_name, renderer_type);
        self.find_in_scope(scope_id, |set| set.strings.get(&key).cloned())
            .or_else(|| {
                (scope_id != 0)
                    .then(|| self.find_in_scope(0, |set| set.strings.get(&key).cloned()))?
            })
            .unwrap_or_default()
    }

    pub fn get_multi_path(&self, key: &str, scope_id: u64) -> Vec<String> {
        let mut paths = self
            .scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .map(|set| set.get_path(key))
            .filter(|path| !path.is_empty())
            .collect::<Vec<_>>();
        if scope_id != 0 {
            paths.extend(self.get_multi_path(key, 0));
        }
        paths
    }

    pub fn get_material_names_json(&self, scope_id: u64) -> String {
        let names = self
            .scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .flat_map(|set| set.materials.keys().map(String::as_str));
        json_names(names)
    }

    pub fn material_names(&self, scope_id: u64) -> Vec<String> {
        let mut names = self
            .scopes
            .get(&scope_id)
            .into_iter()
            .flatten()
            .flat_map(|set| set.materials.keys().cloned())
            .collect::<Vec<_>>();
        if scope_id != 0 {
            names.extend(self.material_names(0));
        }
        names.sort();
        names.dedup();
        names
    }

    pub fn get_material_names_json2(&self, scope_id: u64, style_set_id: u64) -> String {
        self.scopes
            .get(&scope_id)
            .and_then(|sets| sets.iter().find(|set| set.uid() == style_set_id))
            .map_or_else(|| "[]".to_string(), StyleSet::get_material_names_json)
    }

    pub fn get_material_names_json_2(&self, scope_id: u64, style_set_id: u64) -> String {
        self.get_material_names_json2(scope_id, style_set_id)
    }

    pub fn create_style_from_obj(
        &mut self,
        _scope_id: u64,
        style_set_id: u64,
        name: impl AsRef<str>,
        object: &Renderer,
    ) -> bool {
        let Some(set) = self.get_style_set_mut(style_set_id) else {
            return false;
        };
        if set.read_only() {
            return false;
        }
        let name = name.as_ref();
        if set.has_style(name) {
            set.remove_style(name);
        }
        set.set_style(name, object.to_style_node())
    }

    pub fn compile_color(&self, rep: &str, scope_id: u64) -> SolidColor {
        if let Some(color) = self.get_color(rep.trim(), scope_id) {
            return color.clone();
        }
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
        std::fs::write(path_name, style_set_to_xml(set)).is_ok()
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

    pub fn load_style_set_from_xml(
        &mut self,
        scope_id: u64,
        name: impl Into<String>,
        xml: &str,
        read_only: bool,
    ) -> u64 {
        if !parse_ldom2_xml(xml).is_ok_and(|tree| tree.top().tag_name() == "styles") {
            return INVALID_UID;
        }
        let uid = self.alloc_uid();
        let mut set = StyleSet::new(uid, name);
        populate_style_set_from_xml(&mut set, xml);
        set.set_read_only(read_only);
        set.set_modified(false);
        self.scopes.entry(scope_id).or_default().insert(0, set);
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

    pub fn style_child_node(
        &self,
        scope_id: u64,
        style_id: &str,
        child_tag: &str,
    ) -> Option<&LDom2Node> {
        self.style_node(scope_id, style_id)?.find_child(child_tag)
    }

    pub fn resolve_style_child_node(
        &self,
        scope_id: u64,
        style_list: &str,
        child_tag: &str,
    ) -> Option<&LDom2Node> {
        let mut result = None;
        for name in style_list
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            if let Some(node) = self.style_child_node(scope_id, name, child_tag) {
                result = Some(node);
            }
        }
        result
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

    /// The `type` of a preset's nested child element (e.g. `DefaultCPKColoring` →
    /// `<coloring type="CPKColoring">` → `"CPKColoring"`). `type` is an LDom2
    /// RESERVED attribute surfaced via `type_name()`, NOT `str_attr("type")`, so it
    /// needs its own resolver (the trap that `style_child_attr` cannot read).
    pub fn style_child_type(
        &self,
        scope_id: u64,
        style_id: &str,
        child_tag: &str,
    ) -> Option<String> {
        let lookup = |scope: u64| -> Option<String> {
            self.scopes.get(&scope)?.iter().find_map(|set| {
                let type_name = set
                    .get_style(style_id)?
                    .find_child(child_tag)?
                    .type_name()
                    .to_string();
                (!type_name.is_empty()).then_some(type_name)
            })
        };
        lookup(scope_id).or_else(|| (scope_id != 0).then(|| lookup(0)).flatten())
    }

    /// Resolve a `style="A,B"` list for a nested child element's `type` (later
    /// style wins) — e.g. the renderer's own `<coloring type="...">` scheme.
    pub fn resolve_style_child_type(
        &self,
        scope_id: u64,
        style_list: &str,
        child_tag: &str,
    ) -> Option<String> {
        let mut result = None;
        for name in style_list.split(',') {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            if let Some(type_name) = self.style_child_type(scope_id, name, child_tag) {
                result = Some(type_name);
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

#[derive(Debug)]
pub struct StyleNamedColorResolver {
    manager: RwLock<StyleManager>,
    context_id: AtomicU64,
    next_cache_id: AtomicU64,
    cached: Mutex<std::collections::BTreeSet<u64>>,
}

impl StyleNamedColorResolver {
    pub fn new(manager: StyleManager, context_id: u64) -> Self {
        Self {
            manager: RwLock::new(manager),
            context_id: AtomicU64::new(context_id),
            next_cache_id: AtomicU64::new(0),
            cached: Mutex::new(std::collections::BTreeSet::new()),
        }
    }

    pub fn install(self) -> Arc<Self> {
        let resolver = Arc::new(self);
        set_named_color_resolver(Some(resolver.clone()));
        resolver
    }

    pub fn set_context_id(&self, context_id: u64) {
        self.context_id.store(context_id, Ordering::SeqCst);
    }

    pub fn context_id(&self) -> u64 {
        self.context_id.load(Ordering::SeqCst)
    }

    pub fn with_manager<R>(&self, operation: impl FnOnce(&StyleManager) -> R) -> R {
        let manager = self
            .manager
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        operation(&manager)
    }

    pub fn with_manager_mut<R>(&self, operation: impl FnOnce(&mut StyleManager) -> R) -> R {
        let result = {
            let mut manager = self
                .manager
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            operation(&mut manager)
        };
        self.invalidate_cache();
        result
    }
}

impl NamedColorResolver for StyleNamedColorResolver {
    fn get_color(&self, name: &str, context_id: u64) -> Option<ColorPtr> {
        self.with_manager(|manager| {
            manager
                .get_color(name, context_id)
                .cloned()
                .map(|color| Arc::new(color) as ColorPtr)
        })
    }

    fn context_id(&self) -> u64 {
        self.context_id()
    }

    fn make_cache(&self) -> u64 {
        let cache_id = self.next_cache_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(cache_id);
        cache_id
    }

    fn is_cached(&self, cache_id: u64) -> bool {
        self.cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&cache_id)
    }

    fn set_cached(&self, cache_id: u64, cached: bool) {
        let mut cache = self
            .cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cached {
            cache.insert(cache_id);
        } else {
            cache.remove(&cache_id);
        }
    }

    fn invalidate_cache(&self) {
        self.cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

/// Scoped style-context switch that restores the previous context on drop.
pub struct AutoStyleContext<'a> {
    resolver: &'a StyleNamedColorResolver,
    previous_context_id: u64,
    active: bool,
}

impl<'a> AutoStyleContext<'a> {
    pub fn new(resolver: &'a StyleNamedColorResolver, context_id: u64) -> Self {
        let previous_context_id = resolver.context_id();
        let active = context_id != INVALID_UID;
        if active {
            resolver.set_context_id(context_id);
        }
        Self {
            resolver,
            previous_context_id,
            active,
        }
    }

    pub fn end_request(&mut self) {
        if self.active {
            self.resolver.set_context_id(self.previous_context_id);
            self.active = false;
        }
    }
}

impl Drop for AutoStyleContext<'_> {
    fn drop(&mut self) {
        self.end_request();
    }
}

pub type AutoStyleCtxt<'a> = AutoStyleContext<'a>;

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

fn style_set_to_xml(set: &StyleSet) -> String {
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    output.push_str("<styles id=\"");
    output.push_str(&escape_xml_attr(set.name()));
    output.push_str("\">\n");

    for (id, material) in &set.materials {
        output.push_str("  <material id=\"");
        output.push_str(&escape_xml_attr(id));
        output.push_str("\">\n");
        if material.has_system_values() {
            output.push_str("    <def");
            for (name, parameter) in [
                ("ambient", MATERIAL_AMBIENT),
                ("diffuse", MATERIAL_DIFFUSE),
                ("specular", MATERIAL_SPECULAR),
                ("shininess", MATERIAL_SHININESS),
                ("emission", MATERIAL_EMISSION),
            ] {
                let value = material.system_values[parameter];
                output.push(' ');
                output.push_str(name);
                output.push_str("=\"");
                output.push_str(&value.to_string());
                output.push('"');
            }
            output.push_str("/>\n");
        }
        for (renderer_type, value) in material.dependent_values() {
            output.push_str("    <def type=\"");
            output.push_str(&escape_xml_attr(renderer_type));
            output.push_str("\">");
            output.push_str(&escape_xml_text(value));
            output.push_str("</def>\n");
        }
        output.push_str("  </material>\n");
    }

    for (name, color) in &set.colors {
        output.push_str("  <color id=\"");
        output.push_str(&escape_xml_attr(name));
        output.push_str("\" value=\"");
        output.push_str(&escape_xml_attr(&color.script_string()));
        if color.material().is_empty() {
            output.push_str("\"/>\n");
        } else {
            output.push_str("\" material=\"");
            output.push_str(&escape_xml_attr(color.material()));
            output.push_str("\"/>\n");
        }
    }

    for (key, value) in &set.strings {
        let Some((database, category, id)) = decode_str_data_key(key) else {
            continue;
        };
        if database == "string" {
            output.push_str("  <");
            output.push_str(category);
            output.push_str(" id=\"");
            output.push_str(&escape_xml_attr(id));
            output.push_str("\" value=\"");
            output.push_str(&escape_xml_attr(value));
            output.push_str("\"/>\n");
        } else if database == "cfg" {
            output.push_str("  <setting type=\"");
            output.push_str(&escape_xml_attr(id));
            output.push_str("\"><");
            output.push_str(category);
            output.push('>');
            output.push_str(&escape_xml_text(value));
            output.push_str("</");
            output.push_str(category);
            output.push_str("></setting>\n");
        }
    }

    for (key, node) in &set.styles {
        let Some(id) = key.strip_suffix("\tstyle") else {
            continue;
        };
        write_style_xml_node(&mut output, node, 1, Some(id));
        output.push('\n');
    }
    output.push_str("</styles>\n");
    output
}

fn write_style_xml_node(
    output: &mut String,
    node: &LDom2Node,
    indent: usize,
    forced_id: Option<&str>,
) {
    output.push_str(&"  ".repeat(indent));
    output.push('<');
    output.push_str(node.tag_name());
    if let Some(id) = forced_id {
        output.push_str(" id=\"");
        output.push_str(&escape_xml_attr(id));
        output.push('"');
    }
    let type_name = if node.type_name().is_empty() {
        node.str_attr("type").to_string()
    } else {
        node.type_name().to_string()
    };
    if !type_name.is_empty() {
        output.push_str(" type=\"");
        output.push_str(&escape_xml_attr(&type_name));
        output.push('"');
    }
    if !node.value().is_empty() {
        output.push_str(" value=\"");
        output.push_str(&escape_xml_attr(node.value()));
        output.push('"');
    }
    for child in node.children().iter().filter(|child| child.is_attr()) {
        if child.tag_name() == "type" || (child.tag_name() == "id" && forced_id.is_some()) {
            continue;
        }
        output.push(' ');
        output.push_str(child.tag_name());
        output.push_str("=\"");
        output.push_str(&escape_xml_attr(child.value()));
        output.push('"');
    }
    let children = node
        .children()
        .iter()
        .filter(|child| !child.is_attr())
        .collect::<Vec<_>>();
    if children.is_empty() && node.contents().is_empty() {
        output.push_str("/>");
        return;
    }
    output.push('>');
    if !node.contents().is_empty() {
        output.push_str(&escape_xml_text(node.contents()));
    }
    if !children.is_empty() {
        output.push('\n');
        for child in children {
            write_style_xml_node(output, child, indent + 1, None);
            output.push('\n');
        }
        output.push_str(&"  ".repeat(indent));
    }
    output.push_str("</");
    output.push_str(node.tag_name());
    output.push('>');
}

fn escape_xml_attr(value: &str) -> String {
    escape_xml_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn decode_str_data_key(key: &str) -> Option<(&str, &str, &str)> {
    let (database, rest) = key.split_once('\t')?;
    let (category, id) = rest.split_once('\t')?;
    Some((database, category, id))
}

/// Parse an LDom2 style XML document (`<styles>`) into a [`StyleSet`], mirroring
/// C++ `StyleFile::loadNodes`: styles, palette entries, material definitions,
/// renderer settings, selection aliases, paths, and generic string data are all
/// retained. Returns the number of styles registered, or zero for invalid XML.
fn populate_style_set_from_xml(set: &mut StyleSet, xml: &str) -> usize {
    let Ok(tree) = parse_ldom2_xml(xml) else {
        return 0;
    };
    let root_id = tree.top().str_attr("id").to_string();
    if !root_id.is_empty() {
        set.set_name(root_id);
    }
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
                    let mut color = SolidColor::from(rgba);
                    let material = child.str_attr("material").to_string();
                    if !material.is_empty() {
                        color.set_material(material);
                    }
                    set.set_color(id, color);
                }
            }
            "material" => load_material_node(set, child),
            "setting" => load_setting_node(set, child),
            _ => load_string_node(set, child),
        }
    }
    style_count
}

fn load_material_node(set: &mut StyleSet, node: &LDom2Node) {
    let id = node.str_attr("id").to_string();
    if id.is_empty() {
        return;
    }
    for definition in node
        .children()
        .iter()
        .filter(|child| !child.is_attr() && child.tag_name() == "def")
    {
        let renderer_type = definition.type_name().to_string();
        if !renderer_type.is_empty() {
            let value = node_text(definition);
            set.put_material_string(id.clone(), renderer_type, value);
            continue;
        }
        for (name, parameter) in [
            ("ambient", MATERIAL_AMBIENT),
            ("diffuse", MATERIAL_DIFFUSE),
            ("specular", MATERIAL_SPECULAR),
            ("shininess", MATERIAL_SHININESS),
            ("emission", MATERIAL_EMISSION),
        ] {
            if let Ok(value) = definition.str_attr(name).to_string().trim().parse::<f64>() {
                set.put_material_number(id.clone(), parameter, value);
            }
        }
    }
}

fn load_setting_node(set: &mut StyleSet, node: &LDom2Node) {
    let renderer_type = node.type_name();
    if renderer_type.is_empty() {
        return;
    }
    for child in node.children().iter().filter(|child| !child.is_attr()) {
        set.strings.insert(
            str_data_key("cfg", child.tag_name(), renderer_type),
            node_text(child),
        );
        set.modified = true;
    }
}

fn load_string_node(set: &mut StyleSet, node: &LDom2Node) {
    let id = node.str_attr("id").to_string();
    if id.is_empty() {
        return;
    }
    set.strings.insert(
        str_data_key("string", node.tag_name(), &id),
        node_text(node),
    );
    set.modified = true;
}

fn node_text(node: &LDom2Node) -> String {
    if node.value().is_empty() {
        node.contents().trim().to_string()
    } else {
        node.value().to_string()
    }
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

fn style_info_entries(set: &StyleSet) -> Vec<String> {
    set.styles
        .iter()
        .filter_map(|(key, node)| {
            let name = key.strip_suffix("\tstyle")?;
            let description = node.str_attr("desc").to_string();
            let style_type = if node.type_name().is_empty() {
                node.str_attr("type").to_string()
            } else {
                node.type_name().to_string()
            };
            Some(format!(
                "{{\"name\":\"{}\",\"desc\":\"{}\",\"type\":\"{}\"}}",
                escape_json(name),
                escape_json(&description),
                escape_json(&style_type)
            ))
        })
        .collect()
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
        assert_eq!(set.src(), "");
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
        assert_eq!(
            set.style_names_json(),
            r#"[{"name":"Ribbon","desc":"","type":""}]"#
        );
        assert_eq!(
            set.style_infos(),
            [StyleInfo {
                name: "Ribbon".to_string(),
                description: String::new(),
                style_type: String::new(),
            }]
        );
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
    fn style_named_color_resolver_tracks_scope_and_invalidates_caches() {
        let mut manager = StyleManager::new();
        let set_id = manager.create_style_set("scene", 10);
        assert!(manager.set_color("accent", SolidColor::from_rgb_u8(255, 0, 0), 10, set_id));
        let resolver = StyleNamedColorResolver::new(manager, 10);

        assert_eq!(resolver.context_id(), 10);
        assert_eq!(
            resolver.get_color("accent", 10).unwrap().code(),
            0xffff_0000
        );
        let cache_id = resolver.make_cache();
        assert!(resolver.is_cached(cache_id));

        resolver.with_manager_mut(|manager| {
            manager.set_color("accent", SolidColor::from_rgb_u8(0, 0, 255), 10, set_id);
        });
        assert!(!resolver.is_cached(cache_id));
        assert_eq!(
            resolver.get_color("accent", 10).unwrap().code(),
            0xff00_00ff
        );
        resolver.set_context_id(11);
        assert_eq!(resolver.context_id(), 11);
    }

    #[test]
    fn auto_style_context_restores_the_previous_scope() {
        let resolver = StyleNamedColorResolver::new(StyleManager::new(), 10);
        {
            let mut context = AutoStyleContext::new(&resolver, 20);
            assert_eq!(resolver.context_id(), 20);
            context.end_request();
            assert_eq!(resolver.context_id(), 10);
        }
        assert_eq!(resolver.context_id(), 10);
        {
            let _context = AutoStyleContext::new(&resolver, 30);
            assert_eq!(resolver.context_id(), 30);
        }
        assert_eq!(resolver.context_id(), 10);
        {
            let _context = AutoStyleContext::new(&resolver, INVALID_UID);
            assert_eq!(resolver.context_id(), 10);
        }
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
        assert!(manager.get_color2("molcol", 10, uid).is_some());
        assert!(manager.get_color2("molcol", 11, uid).is_none());
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
        assert_eq!(
            manager.style_names_json(10),
            r#"[{"name":"Ribbon","desc":"","type":""}]"#
        );
        manager.fire_pending_events();
        assert_eq!(manager.pending_event_count(), 0);

        assert!(manager.destroy_style_set(10, uid));
        assert_eq!(manager.has_style_set("Default", 10), INVALID_UID);
    }

    #[test]
    fn string_data_lookup_falls_back_to_global_scope_and_lists_overrides() {
        let mut manager = StyleManager::new();
        let global = manager.create_style_set("global", 0);
        let local = manager.create_style_set("local", 42);
        assert!(manager.set_str_data("sel", "protein", "rprop type=prot", 0, global));
        assert!(manager.set_str_data("sel", "solvent", "resn HOH", 0, global));
        assert!(manager.set_str_data("sel", "protein", "chain A", 42, local));

        assert_eq!(manager.get_str_data("sel", "protein", 42), "chain A");
        assert_eq!(manager.get_str_data("sel", "solvent", 42), "resn HOH");
        assert_eq!(
            manager.str_data_names("sel", 42),
            vec!["protein".to_string(), "solvent".to_string()]
        );
        assert_eq!(
            manager.str_data_entries("sel", 42),
            BTreeMap::from([
                ("protein".to_string(), "chain A".to_string()),
                ("solvent".to_string(), "resn HOH".to_string()),
            ])
        );
    }

    #[test]
    fn compile_color_uses_scope_fallback_and_clear_scope_removes_local_state() {
        let mut manager = StyleManager::new();
        let global = manager.create_style_set("global", 0);
        let local = manager.create_style_set("local", 42);
        assert!(manager.set_color("accent", SolidColor::from_rgb_u8(255, 0, 0), 0, global));
        assert!(manager.set_color("accent", SolidColor::from_rgb_u8(0, 0, 255), 42, local));
        assert!(manager.set_color("global-only", SolidColor::from_rgb_u8(0, 255, 0), 0, global));

        assert_eq!(manager.compile_color("accent", 42).rgba8().b, 255);
        assert_eq!(manager.compile_color("global-only", 42).rgba8().g, 255);
        assert_eq!(manager.compile_color("#010203", 42).rgba8().r, 1);
        manager.set_style_value(42, "local", "Renderer.width", "2");
        assert_eq!(manager.pending_event_count(), 1);

        manager.clear_scope(42);
        assert_eq!(manager.has_style_set("local", 42), INVALID_UID);
        assert_eq!(manager.compile_color("accent", 42).rgba8().r, 255);
        assert_eq!(manager.pending_event_count(), 0);
        assert_ne!(manager.has_style_set("global", 0), INVALID_UID);
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
    fn style_manager_extracts_and_replaces_renderer_styles() {
        let mut manager = StyleManager::new();
        let uid = manager.create_style_set("Default", 10);
        let mut renderer = Renderer::new("simple");
        renderer.set_name("source");
        renderer.set_alpha(0.4);
        renderer.set_property_str("bondw", "0.2");

        assert!(manager.create_style_from_obj(10, uid, "Custom", &renderer));
        let style = manager
            .get_style_set(uid)
            .and_then(|set| set.get_style("Custom"))
            .unwrap();
        assert_eq!(style.type_name(), "renderer");
        assert_eq!(style.str_attr("alpha").to_string(), "0.4");
        assert_eq!(style.str_attr("bondw").to_string(), "0.2");
        assert!(style.str_attr("name").is_empty());

        renderer.set_property_str("bondw", "0.3");
        assert!(manager.create_style_from_obj(10, uid, "Custom", &renderer));
        let style = manager
            .get_style_set(uid)
            .and_then(|set| set.get_style("Custom"))
            .unwrap();
        assert_eq!(style.str_attr("bondw").to_string(), "0.3");

        manager.get_style_set_mut(uid).unwrap().set_read_only(true);
        assert!(!manager.create_style_from_obj(10, uid, "Blocked", &renderer));
        assert!(!manager.create_style_from_obj(10, INVALID_UID, "Missing", &renderer));
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
        assert_eq!(
            manager
                .style_attr(0, "DefaultBallStick", "bondw")
                .as_deref(),
            Some("0.2")
        );
        assert_eq!(
            manager.style_attr(0, "DefaultBallStick", "sphr").as_deref(),
            Some("0.3")
        );
        assert_eq!(
            manager.style_attr(0, "ThickBallStick", "bondw").as_deref(),
            Some("0.3")
        );
        assert_eq!(
            manager.style_attr(0, "ThickBallStick", "sphr").as_deref(),
            Some("0.5")
        );
        assert_eq!(
            manager.style_attr(0, "DefaultCPK", "detail").as_deref(),
            Some("10")
        );
        // An undefined attribute on a real style is None, not empty string.
        assert!(manager.style_attr(0, "DefaultCPK", "bondw").is_none());
        // An unknown style is None.
        assert!(manager.style_attr(0, "NoSuchStyle", "bondw").is_none());

        // The X11 named-colour table is populated (`<color id=.. value=..>`).
        assert!(set.has_color("Yellow"));
        assert!(set.has_color("SlateBlue"));

        // Non-style sections are part of the same StyleSet contract.
        assert_eq!(set.get_str_data("sel", "protein"), "rprop type=prot");
        assert_eq!(
            set.get_str_data("path", "icc_profile_dir"),
            "%%CONFDIR%%/icc_profiles"
        );
        assert!(manager
            .get_config("pov", "preamble", 0)
            .contains("colors.inc"));
        assert_eq!(
            manager.get_material_number("default", MATERIAL_AMBIENT, 0),
            0.2
        );
        assert_eq!(
            manager.get_material_number("default", MATERIAL_DIFFUSE, 0),
            0.8
        );
        assert!(manager
            .get_material_string("default", "pov", 0)
            .contains("pigment"));
        assert!(manager
            .get_material_names_json(0)
            .contains("\"nolighting\""));
    }

    #[test]
    fn style_file_roundtrips_all_database_sections() {
        let input = r##"<?xml version="1.0"?>
<styles id="roundtrip">
  <material id="custom">
    <def ambient="0.3" diffuse="0.6" specular="0.2" shininess="12" emission="0.1"/>
    <def type="pov">texture { pigment { color rgb @COLOR@ } }</def>
  </material>
  <setting type="pov"><preamble>#include "colors.inc"</preamble></setting>
  <path id="assets" value="%%CONFDIR%%/assets"/>
  <sel id="backbone" value="bymainch *"/>
  <color id="Accent" value="#123456" material="custom"/>
  <style id="MyStyle" type="renderer" desc="Custom" bondw="0.42">
    <coloring type="CPKColoring" col_C="#abcdef"/>
  </style>
</styles>"##;
        let dir = std::env::temp_dir().join("cuemol_style_roundtrip_test");
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.xml");
        let saved = dir.join("saved.xml");
        std::fs::write(&source, input).unwrap();

        let mut manager = StyleManager::new();
        let uid = manager.load_style_set_from_file(4, &source, false);
        assert!(manager.save_style_set_to_file(4, uid, &saved));

        let mut reloaded = StyleManager::new();
        let reloaded_uid = reloaded.load_style_set_from_file(9, &saved, false);
        let set = reloaded.get_style_set(reloaded_uid).unwrap();
        assert_eq!(set.name(), "roundtrip");
        assert_eq!(set.get_str_data("sel", "backbone"), "bymainch *");
        assert_eq!(set.get_str_data("path", "assets"), "%%CONFDIR%%/assets");
        assert_eq!(
            reloaded.get_config("pov", "preamble", 9),
            "#include \"colors.inc\""
        );
        assert_eq!(
            reloaded.get_material_number("custom", MATERIAL_EMISSION, 9),
            0.1
        );
        assert!(reloaded
            .get_material_string("custom", "pov", 9)
            .contains("@COLOR@"));
        assert_eq!(set.get_color("accent").unwrap().material(), "custom");
        assert_eq!(
            reloaded.style_attr(9, "MyStyle", "bondw").as_deref(),
            Some("0.42")
        );
        assert_eq!(
            reloaded
                .style_child_type(9, "MyStyle", "coloring")
                .as_deref(),
            Some("CPKColoring")
        );
        assert!(reloaded.style_names_json(9).contains(r#""desc":"Custom""#));

        std::fs::remove_file(&source).ok();
        std::fs::remove_file(&saved).ok();
    }

    #[test]
    fn resolve_style_attr_walks_the_comma_list_last_wins() {
        let mut manager = StyleManager::new();
        manager.load_builtin_default_styles(0);

        // `DefaultLabel` carries color/font; `DefaultAtomIntr` carries width —
        // the two contribute disjoint attributes, both resolvable from the list.
        assert_eq!(
            manager
                .resolve_style_attr(0, "DefaultLabel,DefaultAtomIntr", "width")
                .as_deref(),
            Some("1.0")
        );
        assert_eq!(
            manager
                .resolve_style_attr(0, "DefaultLabel,DefaultAtomIntr", "color")
                .as_deref(),
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
            manager
                .resolve_style_attr(0, " DefaultBallStick , ", "bondw")
                .as_deref(),
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
            manager
                .style_child_attr(0, "DefaultRibbon", "helix", "tuber")
                .as_deref(),
            Some("6.0")
        );
        assert_eq!(
            manager
                .style_child_attr(0, "DefaultRibbon", "helix", "width")
                .as_deref(),
            Some("0.2")
        );
        assert_eq!(
            manager
                .style_child_attr(0, "DefaultRibbon", "sheet", "tuber")
                .as_deref(),
            Some("7.0")
        );
        // No <coil> sub-element in DefaultRibbon.
        assert!(manager
            .style_child_attr(0, "DefaultRibbon", "coil", "width")
            .is_none());
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
        assert_eq!(
            manager.style_attr(5, "MyBallStick", "bondw").as_deref(),
            Some("0.42")
        );
        assert!(manager.get_style_set(uid).unwrap().has_color("Crimson"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn style_child_type_reads_the_reserved_type_attr() {
        let mut manager = StyleManager::new();
        manager.load_builtin_default_styles(0);
        // DefaultCPKColoring has <coloring type="CPKColoring" col_C="$molcol"/>.
        // This LDom2 parser stores `type` as a normal attr child, so style_child_type
        // resolves it (via type_name) to "CPKColoring".
        assert_eq!(
            manager
                .style_child_type(0, "DefaultCPKColoring", "coloring")
                .as_deref(),
            Some("CPKColoring"),
            "style_child_type reads the coloring scheme name"
        );

        // A custom style with a RainbowColoring scheme resolves through a style list.
        let dir = std::env::temp_dir().join("cuemol_style_child_type_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rainbow_style.xml");
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
        manager.load_style_set_from_file(0, &path, true);
        assert_eq!(
            manager
                .style_child_type(0, "MyRainbow", "coloring")
                .as_deref(),
            Some("RainbowColoring")
        );
        assert_eq!(
            manager
                .resolve_style_child_type(0, "DefaultCPKColoring,MyRainbow", "coloring")
                .as_deref(),
            Some("RainbowColoring"),
            "later style in the list wins"
        );
        std::fs::remove_file(&path).ok();
    }
}
