//! Typed scene application data shared by scene IO and application frontends.

use cuemol_core::{CueMolError, LDom2Node, Result, Uid};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_APP_DATA_UID: AtomicU64 = AtomicU64::new(8_000_000_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsClass {
    Render,
    Pov,
    Umbreon,
    UmbreonNpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsKind {
    Boolean,
    Integer,
    Real,
    String,
}

#[derive(Debug, Clone, Copy)]
pub struct SettingsProperty {
    pub name: &'static str,
    pub kind: SettingsKind,
    pub default: &'static str,
}

macro_rules! properties {
    ($(($name:literal, $kind:ident, $default:literal)),* $(,)?) => {
        &[$(SettingsProperty { name: $name, kind: SettingsKind::$kind, default: $default }),*]
    };
}

const COMMON: &[SettingsProperty] = properties![
    ("backend", String, ""),
    ("width", Real, "1200"),
    ("height", Real, "1200"),
    ("unit", String, "px"),
    ("dpi", Real, "600"),
    ("transparentBg", Boolean, "false"),
    ("postBlend", Boolean, "true"),
    ("pixelLabels", Boolean, "false"),
    ("projection", String, "perspective"),
    ("stereoMode", String, "none"),
    ("stereoDepth", Real, "0.03"),
    ("clipPlane", Boolean, "true"),
    ("numThreads", Integer, "2"),
    ("edgeLines", Boolean, "true"),
];
const POV: &[SettingsProperty] = properties![
    ("radiosityMode", String, "Disable"),
    ("shadow", Boolean, "false"),
    ("lightDefault", Boolean, "true"),
    ("lightSpread", Integer, "1"),
    ("lightIntensity", Real, "1.3"),
    ("flashFraction", Real, "0.6"),
    ("ambientFraction", Real, "0"),
];
const UMBREON: &[SettingsProperty] = properties![
    ("supersample", Integer, "3"),
    ("aoEnabled", Boolean, "false"),
    ("aoSamples", Integer, "64"),
    ("aoDistance", Real, "0"),
    ("aoIntensity", Real, "1"),
    ("aoDiffuseFactor", Real, "1"),
    ("aoMultiScale", Boolean, "true"),
    ("aoBentNormal", Boolean, "true"),
    ("aoLowDiscrepancy", Boolean, "true"),
    ("aoGather", String, "Per output pixel"),
    ("shadows", Boolean, "false"),
    ("shadowSamples", Integer, "1"),
    ("lightRadius", Real, "0"),
    ("creaseLimit", Real, "-1"),
    ("edgeRise", Real, "0.5"),
    ("contactEdges", Boolean, "true"),
    ("outlineFarDepth", Real, "0.2"),
    ("perPixelBlend", Boolean, "true"),
    ("useGI", Boolean, "true"),
    ("giSamples", Integer, "32"),
    ("denoise", String, "OIDN"),
    ("giSkyGradient", Boolean, "true"),
    ("giGroundColor", String, "#666666"),
    ("lightIntensity", Real, "1.3"),
    ("flashFraction", Real, "0"),
    ("ambientFraction", Real, "0.6"),
];
const NPR: &[SettingsProperty] = properties![
    ("useGI", Boolean, "false"),
    ("lightIntensity", Real, "1.55"),
    ("flashFraction", Real, "0.6"),
    ("hatchStyle", String, "richardson"),
    ("hatchColoring", String, "Style default"),
    ("hatchDensity", Real, "1"),
    ("hatchWidthScale", Real, "1"),
    ("hatchCustomInk", Boolean, "false"),
    ("hatchInkColor", String, "#000000"),
    ("hatchCustomPaper", Boolean, "false"),
    ("hatchPaperColor", String, "#ffffff"),
    ("hatchDefaultEdges", Boolean, "true"),
    ("hatchLayersSpec", String, ""),
    ("hatchToneSpec", String, ""),
];

impl SettingsClass {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "RenderSettings" => Some(Self::Render),
            "PovRenderSettings" => Some(Self::Pov),
            "UmbreonRenderSettings" => Some(Self::Umbreon),
            "UmbreonNprRenderSettings" => Some(Self::UmbreonNpr),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Render => "RenderSettings",
            Self::Pov => "PovRenderSettings",
            Self::Umbreon => "UmbreonRenderSettings",
            Self::UmbreonNpr => "UmbreonNprRenderSettings",
        }
    }

    pub fn properties(self) -> Vec<SettingsProperty> {
        match self {
            Self::Render => COMMON.to_vec(),
            Self::Pov => POV.to_vec(),
            Self::Umbreon => UMBREON.to_vec(),
            Self::UmbreonNpr => UMBREON
                .iter()
                .filter(|property| !NPR.iter().any(|other| other.name == property.name))
                .chain(NPR)
                .copied()
                .collect(),
        }
    }

    pub fn children(self) -> &'static [(&'static str, Self)] {
        if self == Self::Render {
            &[
                ("povray", Self::Pov),
                ("umbreon", Self::Umbreon),
                ("umbreon_npr", Self::UmbreonNpr),
            ]
        } else {
            &[]
        }
    }

    pub fn property(self, path: &str) -> Option<SettingsProperty> {
        if let Some((child, path)) = path.split_once('.') {
            return self
                .children()
                .iter()
                .find(|(name, _)| *name == child)?
                .1
                .property(path);
        }
        self.properties()
            .into_iter()
            .find(|property| property.name == path)
    }
}

impl SettingsProperty {
    pub fn parse(self, value: &str) -> Result<String> {
        let parsed = match self.kind {
            SettingsKind::String => Some(value.to_string()),
            SettingsKind::Boolean => value.parse::<bool>().ok().map(|v| v.to_string()),
            SettingsKind::Integer => value.parse::<i32>().ok().map(|v| v.to_string()),
            SettingsKind::Real => value
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map(|v| v.to_string()),
        };
        parsed.ok_or_else(|| {
            CueMolError::invalid_argument(format!("invalid {} value: {value}", self.name))
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSettings {
    uid: Uid,
    class: SettingsClass,
    overrides: BTreeMap<String, String>,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self::new(SettingsClass::Render)
    }
}

impl RenderSettings {
    pub fn new(class: SettingsClass) -> Self {
        Self {
            uid: Uid::new(NEXT_APP_DATA_UID.fetch_add(1, Ordering::Relaxed)),
            class,
            overrides: BTreeMap::new(),
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }
    pub fn class(&self) -> SettingsClass {
        self.class
    }
    pub fn overrides(&self) -> &BTreeMap<String, String> {
        &self.overrides
    }

    pub fn get(&self, path: &str) -> Option<&str> {
        self.class.property(path).map(|property| {
            self.overrides
                .get(path)
                .map_or(property.default, String::as_str)
        })
    }

    pub fn set(&mut self, path: &str, value: &str) -> Result<()> {
        let property = self
            .class
            .property(path)
            .ok_or_else(|| CueMolError::not_found(format!("render setting {path}")))?;
        let value = property.parse(value)?;
        if value == property.parse(property.default)? {
            self.overrides.remove(path);
        } else {
            self.overrides.insert(path.to_string(), value);
        }
        Ok(())
    }

    pub fn reset(&mut self, path: &str) -> Result<()> {
        let property = self
            .class
            .property(path)
            .ok_or_else(|| CueMolError::not_found(format!("render setting {path}")))?;
        self.set(path, property.default)
    }

    pub fn is_default(&self, path: &str) -> bool {
        self.class.property(path).is_some() && !self.overrides.contains_key(path)
    }

    pub fn to_node(&self, id: &str) -> LDom2Node {
        let mut node = LDom2Node::new();
        node.set_tag_name("appdata");
        node.set_str_attr("id", id);
        node.set_type_name(self.class.name());
        for (path, value) in &self.overrides {
            if let Some((child, name)) = path.split_once('.') {
                if node.find_child(child).is_none() {
                    node.append_child(child);
                }
                node.find_child_mut(child)
                    .unwrap()
                    .set_str_attr(name, value);
            } else {
                node.set_str_attr(path, value);
            }
        }
        node
    }

    pub fn read_node(&mut self, node: &LDom2Node) -> Vec<String> {
        fn read(
            settings: &mut RenderSettings,
            node: &LDom2Node,
            prefix: &str,
            warnings: &mut Vec<String>,
        ) {
            for child in node.children() {
                if prefix.is_empty() && matches!(child.tag_name(), "id" | "type") {
                    continue;
                }
                let path = format!("{prefix}{}", child.tag_name());
                if !child.is_attr() && !child.children().is_empty() {
                    read(settings, child, &format!("{path}."), warnings);
                } else {
                    let value = if child.is_attr() || !child.value().is_empty() {
                        child.value()
                    } else {
                        child.contents()
                    };
                    if self::RenderSettings::set(settings, &path, value).is_err() {
                        warnings.push(format!(
                            "appdata property {path} ignored (unknown or invalid)"
                        ));
                    }
                }
            }
        }
        let mut warnings = Vec::new();
        read(self, node, "", &mut warnings);
        warnings
    }
}

#[derive(Debug, Clone, PartialEq)]
enum AppDataEntry {
    Render(RenderSettings),
    Unknown(LDom2Node),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneAppData {
    entries: BTreeMap<String, AppDataEntry>,
    warnings: Vec<String>,
}

impl SceneAppData {
    pub fn iter(&self) -> impl Iterator<Item = (&str, &RenderSettings)> {
        self.entries.iter().filter_map(|(id, entry)| match entry {
            AppDataEntry::Render(settings) => Some((id.as_str(), settings)),
            AppDataEntry::Unknown(_) => None,
        })
    }

    pub fn get(&self, id: &str) -> Option<&RenderSettings> {
        match self.entries.get(id)? {
            AppDataEntry::Render(settings) => Some(settings),
            AppDataEntry::Unknown(_) => None,
        }
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut RenderSettings> {
        match self.entries.get_mut(id)? {
            AppDataEntry::Render(settings) => Some(settings),
            AppDataEntry::Unknown(_) => None,
        }
    }

    pub fn get_or_create(
        &mut self,
        id: &str,
        class_name: &str,
    ) -> Result<Option<&mut RenderSettings>> {
        if self.get(id).is_none() {
            match SettingsClass::from_name(class_name) {
                Some(SettingsClass::Render) => {
                    self.entries.insert(
                        id.to_string(),
                        AppDataEntry::Render(RenderSettings::default()),
                    );
                }
                Some(_) => {
                    return Err(CueMolError::invalid_argument(format!(
                        "class {class_name} is not a SceneAppData"
                    )))
                }
                None => return Ok(None),
            }
        }
        Ok(self.get_mut(id))
    }

    pub fn insert(&mut self, id: impl Into<String>, settings: RenderSettings) -> Result<()> {
        if settings.class() != SettingsClass::Render {
            return Err(CueMolError::invalid_argument(
                "only RenderSettings can be scene app data",
            ));
        }
        self.entries
            .insert(id.into(), AppDataEntry::Render(settings));
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> bool {
        self.entries.remove(id).is_some()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.warnings.clear();
    }
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn nodes(&self) -> Vec<LDom2Node> {
        self.entries
            .iter()
            .map(|(id, entry)| match entry {
                AppDataEntry::Render(settings) => settings.to_node(id),
                AppDataEntry::Unknown(node) => node.clone(),
            })
            .collect()
    }

    pub fn read_node(&mut self, node: &LDom2Node) -> Vec<String> {
        let warnings = self.read_entry(node);
        self.warnings.extend(warnings.iter().cloned());
        warnings
    }

    fn read_entry(&mut self, node: &LDom2Node) -> Vec<String> {
        let id = node.str_attr("id").to_string();
        let class = if node.type_name().is_empty() {
            node.str_attr("type").to_string()
        } else {
            node.type_name().to_string()
        };
        if id.is_empty() || class.is_empty() {
            return vec!["appdata element without id/type ignored".to_string()];
        }
        match SettingsClass::from_name(&class) {
            Some(SettingsClass::Render) => {
                let mut settings = RenderSettings::default();
                let warnings = settings.read_node(node);
                self.entries.insert(id, AppDataEntry::Render(settings));
                warnings
            }
            Some(_) => vec![format!("appdata {id}: class {class} is not a SceneAppData")],
            None => {
                self.entries
                    .insert(id.clone(), AppDataEntry::Unknown(node.clone()));
                vec![format!(
                    "appdata {id}: class {class} unavailable (kept as-is)"
                )]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_keep_backend_defaults_and_omit_unchanged_values() {
        let mut settings = RenderSettings::default();
        assert_eq!(settings.get("width"), Some("1200"));
        assert_eq!(settings.get("dpi"), Some("600"));
        assert_eq!(settings.get("umbreon.useGI"), Some("true"));
        assert_eq!(settings.get("umbreon_npr.useGI"), Some("false"));
        assert_eq!(settings.get("umbreon_npr.ambientFraction"), Some("0.6"));
        settings.set("width", "1200.0").unwrap();
        settings.set("umbreon.giSamples", "64").unwrap();
        settings
            .set(
                "umbreon_npr.hatchLayersSpec",
                "layer:kind=line\nlayer:kind=dot",
            )
            .unwrap();
        assert!(settings.set("umbreon.giSamples", "2.5").is_err());
        assert!(settings.set("width", "NaN").is_err());
        assert!(settings.set("umbreon", "false").is_err());
        let node = settings.to_node("render");
        assert!(node.find_child("width").is_none());
        assert!(node.find_child("povray").is_none());
        let mut store = SceneAppData::default();
        assert!(store.read_node(&node).is_empty());
        assert_eq!(
            store.get("render").unwrap().overrides(),
            settings.overrides()
        );
        settings.reset("umbreon.giSamples").unwrap();
        assert!(settings.to_node("render").find_child("umbreon").is_none());
    }

    #[test]
    fn unknown_app_data_survives_while_invalid_known_properties_are_reported() {
        let mut node = LDom2Node::new();
        node.set_tag_name("appdata");
        node.set_str_attr("id", "extension");
        node.set_type_name("FutureSettings");
        node.append_child("nested").set_str_attr("custom", "keep");
        let mut store = SceneAppData::default();
        assert_eq!(store.read_node(&node).len(), 1);
        assert!(store.get("extension").is_none());
        assert_eq!(store.nodes(), [node]);
        assert!(store
            .get_or_create("extension", "MissingClass")
            .unwrap()
            .is_none());
        let uid = store
            .get_or_create("extension", "RenderSettings")
            .unwrap()
            .unwrap()
            .uid();
        assert_eq!(
            store
                .get_or_create("extension", "MissingClass")
                .unwrap()
                .unwrap()
                .uid(),
            uid
        );
        assert!(store.get_or_create("invalid", "PovRenderSettings").is_err());
        let mut invalid = RenderSettings::default().to_node("render");
        invalid.set_str_attr("width", "bad");
        invalid.set_str_attr("future", "value");
        invalid
            .append_child("umbreon")
            .set_str_attr("giSamples", "128");
        assert_eq!(store.read_node(&invalid).len(), 2);
        assert_eq!(store.get("render").unwrap().get("width"), Some("1200"));
        assert_eq!(
            store.get("render").unwrap().get("umbreon.giSamples"),
            Some("128")
        );
        assert_eq!(store.warnings().len(), 3);
        assert!(store.remove("extension"));
        assert!(!store.remove("extension"));
    }
}
