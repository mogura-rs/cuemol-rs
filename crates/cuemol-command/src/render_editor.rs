//! Shared values and edit transactions for the rendering settings editor.
use cuemol_qsys::{SettingsClass, SettingsKind};
use serde_json::{json, Map, Value};

pub fn defaults() -> Map<String, Value> {
    let mut values = Map::new();
    for (prefix, class) in std::iter::once(("", SettingsClass::Render))
        .chain(SettingsClass::Render.children().iter().copied())
    {
        for property in class.properties() {
            let key = if prefix.is_empty() {
                property.name.into()
            } else {
                format!("{prefix}.{}", property.name)
            };
            let value = if property.kind == SettingsKind::String {
                json!(property.default)
            } else {
                serde_json::from_str(property.default).unwrap()
            };
            values.insert(key, value);
        }
    }
    values
}

pub fn argument(values: &Map<String, Value>) -> String {
    values
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Debug, Default)]
pub struct RenderEditor {
    pub values: Map<String, Value>,
    scene: Option<u64>,
    known: Map<String, Value>,
    loaded: Map<String, Value>,
    changed_at: Option<f64>,
}

impl RenderEditor {
    pub fn sync(&mut self, scene: Option<u64>, stored: &Map<String, Value>) {
        if self.scene == scene
            && (self.changed_at.is_some() || self.known == *stored)
            && !self.values.is_empty()
        {
            return;
        }
        self.scene = scene;
        self.known = stored.clone();
        self.values = defaults();
        self.values.extend(stored.clone());
        self.changed_at = None;
        if !matches!(self.text("backend"), "umbreon" | "umbreon_npr") {
            self.values.insert("backend".into(), json!("umbreon"));
        }
        for prefix in ["umbreon", "umbreon_npr"] {
            if let Some(div) = stored
                .get(&format!("{prefix}.aoResDiv"))
                .or_else(|| stored.get("aoResDiv"))
                .and_then(Value::as_i64)
            {
                self.values.insert(
                    format!("{prefix}.aoGather"),
                    json!(if div < 0 || div > 1 {
                        "Per output pixel"
                    } else {
                        "Per shading hit"
                    }),
                );
            }
        }
        self.loaded = self.values.clone();
    }

    pub fn scene(&self) -> Option<u64> {
        self.scene
    }
    pub fn backend(&self) -> &str {
        self.text("backend")
    }
    pub fn text(&self, key: &str) -> &str {
        self.values.get(key).and_then(Value::as_str).unwrap_or("")
    }
    pub fn number(&self, key: &str) -> f64 {
        self.values.get(key).and_then(Value::as_f64).unwrap_or(0.)
    }
    pub fn flag(&self, key: &str) -> bool {
        self.values
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
    pub fn key(&self, local: &str) -> String {
        format!("{}.{}", self.backend(), local)
    }
    pub fn pending(&self) -> bool {
        self.changed_at.is_some()
    }
    pub fn edited(&mut self, now: f64) {
        if self.scene.is_some() {
            self.changed_at = Some(now);
        }
    }
    pub fn due(&self, scene: Option<u64>, now: f64, force: bool) -> bool {
        self.changed_at
            .is_some_and(|t| force || self.scene != scene || now - t >= 0.3)
    }
    pub fn saved(&mut self, stored: &Map<String, Value>) {
        self.changed_at = None;
        self.known = stored.clone();
        self.values.extend(stored.clone());
        self.loaded = self.values.clone();
    }
    pub fn discard_pending(&mut self) {
        self.changed_at = None;
        self.known.clear();
    }
    pub fn snapshot(&self) -> Map<String, Value> {
        let prefix = format!("{}.", self.backend());
        self.values
            .iter()
            .filter(|(key, _)| {
                (!key.contains('.')
                    || key.starts_with(&prefix)
                    || self.loaded.get(*key) != self.values.get(*key))
                    && !key.ends_with("aoResDiv")
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
    pub fn settings_argument(&self) -> String {
        let mut values = self.snapshot();
        for key in ["aaMode", "aaDepth"] {
            let path = self.key(key);
            if values.contains_key(&path) {
                values.insert(path, json!(0));
            }
        }
        for prefix in ["umbreon", "umbreon_npr"] {
            let gather = format!("{prefix}.aoGather");
            let legacy = format!("{prefix}.aoResDiv");
            if let Some(previous) = self
                .values
                .get(&legacy)
                .or_else(|| self.values.get("aoResDiv"))
            {
                if let Some(current) = values.get(&gather) {
                    let value = if self.loaded.get(&gather) == Some(current) {
                        previous.clone()
                    } else {
                        json!(if current == "Per output pixel" { -1 } else { 0 })
                    };
                    values.insert(legacy, value);
                }
            }
        }
        argument(&values)
    }
    pub fn scale(&self) -> f64 {
        let dpi = self.number("dpi");
        match self.text("unit") {
            "in" => dpi,
            "mm" => dpi / 25.4,
            "cm" => dpi / 2.54,
            _ => 1.,
        }
    }
    pub fn pixels(&self) -> [u32; 2] {
        ["width", "height"].map(|key| (self.number(key) * self.scale()).round().max(1.) as u32)
    }
    pub fn set_unit(&mut self, unit: &str) {
        let pixels = [
            self.number("width") * self.scale(),
            self.number("height") * self.scale(),
        ];
        self.values.insert("unit".into(), json!(unit));
        let scale = self.scale();
        for (key, px) in ["width", "height"].into_iter().zip(pixels) {
            self.values.insert(
                key.into(),
                json!(if unit == "px" { px.round() } else { px / scale }),
            );
        }
    }
    pub fn set_size(&mut self, size: [u32; 2], dpi: Option<f64>) {
        if let Some(dpi) = dpi {
            self.values.insert("dpi".into(), json!(dpi));
        }
        self.values.insert("unit".into(), json!("px"));
        self.values.insert("width".into(), json!(size[0]));
        self.values.insert("height".into(), json!(size[1]));
    }
    pub fn lighting(&self) -> &'static str {
        if self.backend() != "umbreon_npr" && self.flag(&self.key("useGI")) {
            "gi"
        } else if self.flag(&self.key("aoEnabled")) {
            "ao"
        } else {
            "none"
        }
    }
    pub fn apply_lighting(&mut self, lighting: &str) {
        let ao_step = self.quality_step("ao").to_string();
        self.patch(json!({"aoEnabled": lighting == "ao", "useGI": lighting == "gi" && self.backend() != "umbreon_npr"}));
        if lighting == "gi" && self.backend() != "umbreon_npr" {
            self.apply_quality("giLighting", "4");
        } else {
            self.patch(json!({"lightIntensity":1.55,"flashFraction":0.6}));
            if self.backend() == "umbreon" {
                self.patch(json!({"ambientFraction":0.16}));
            }
            if lighting == "ao" {
                self.apply_quality(
                    "ao",
                    if ao_step == "custom" {
                        "medium"
                    } else {
                        &ao_step
                    },
                );
            }
        }
    }
    pub fn quality_step(&self, axis: &str) -> &'static str {
        quality_steps(axis)
            .into_iter()
            .find(|(_, _, patch)| {
                patch.as_object().unwrap().iter().all(|(key, value)| {
                    self.values
                        .get(&self.key(key))
                        .is_some_and(|actual| equal(actual, value))
                })
            })
            .map_or("custom", |(id, _, _)| id)
    }
    pub fn apply_quality(&mut self, axis: &str, step: &str) {
        if let Some((_, _, patch)) = quality_steps(axis)
            .into_iter()
            .find(|(id, _, _)| *id == step)
        {
            self.patch(patch);
        }
    }
    fn patch(&mut self, patch: Value) {
        for (key, value) in patch.as_object().unwrap() {
            self.values.insert(self.key(key), value.clone());
        }
    }
}

fn equal(a: &Value, b: &Value) -> bool {
    a == b || a.as_f64().zip(b.as_f64()).is_some_and(|(a, b)| a == b)
}

pub fn quality_steps(axis: &str) -> Vec<(&'static str, &'static str, Value)> {
    match axis {
        "aa" => vec![
            ("low", "1x (off)", json!({"supersample":1})),
            ("medium", "2x", json!({"supersample":2})),
            ("high", "3x", json!({"supersample":3})),
            ("ultra", "4x", json!({"supersample":4})),
        ],
        "ao" => vec![
            (
                "low",
                "Low",
                json!({"aoSamples":16,"aoGather":"Per output pixel","aoLowDiscrepancy":true,"aoMultiScale":false,"aoBentNormal":false,"aoDiffuseFactor":1.0}),
            ),
            (
                "medium",
                "Medium",
                json!({"aoSamples":64,"aoGather":"Per output pixel","aoLowDiscrepancy":true,"aoMultiScale":true,"aoBentNormal":true,"aoDiffuseFactor":1.0}),
            ),
            (
                "high",
                "High",
                json!({"aoSamples":256,"aoGather":"Per shading hit","aoLowDiscrepancy":true,"aoMultiScale":true,"aoBentNormal":true,"aoDiffuseFactor":1.0}),
            ),
        ],
        "giLighting" => vec![
            (
                "0",
                "0 (raytrace match)",
                json!({"lightIntensity":1.55,"flashFraction":0.6,"ambientFraction":0.16}),
            ),
            (
                "1",
                "1",
                json!({"lightIntensity":1.43,"flashFraction":0.45,"ambientFraction":0.34}),
            ),
            (
                "2",
                "2",
                json!({"lightIntensity":1.37,"flashFraction":0.3,"ambientFraction":0.46}),
            ),
            (
                "3",
                "3",
                json!({"lightIntensity":1.32,"flashFraction":0.15,"ambientFraction":0.54}),
            ),
            (
                "4",
                "4 (max GI)",
                json!({"lightIntensity":1.3,"flashFraction":0,"ambientFraction":0.6}),
            ),
        ],
        "shadows" => vec![
            (
                "off",
                "Off",
                json!({"shadows":false,"shadowSamples":1,"lightRadius":0}),
            ),
            (
                "hard",
                "Hard",
                json!({"shadows":true,"shadowSamples":1,"lightRadius":0}),
            ),
            (
                "soft",
                "Soft",
                json!({"shadows":true,"shadowSamples":16,"lightRadius":3}),
            ),
            (
                "softer",
                "Very soft",
                json!({"shadows":true,"shadowSamples":32,"lightRadius":5}),
            ),
        ],
        _ => vec![],
    }
}

/// Display metadata; values and defaults come from the typed scene schema.
pub fn catalog() -> &'static Value {
    static CATALOG: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    CATALOG
        .get_or_init(|| serde_json::from_str(include_str!("render_editor_catalog.json")).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{call_tool, render_settings, scene_render_settings};
    use cuemol_desktop_core::DesktopAppState;
    use cuemol_qsys::Camera;

    #[test]
    fn backend_blocks_quality_and_units_follow_scene_values() {
        let mut e = RenderEditor::default();
        e.sync(Some(1), &Map::new());
        assert_eq!(e.pixels(), [1200, 1200]);
        assert_eq!(e.lighting(), "gi");
        assert_eq!(e.quality_step("giLighting"), "4");
        e.apply_quality("aa", "medium");
        e.apply_quality("shadows", "soft");
        e.apply_lighting("ao");
        assert_eq!(e.lighting(), "ao");
        assert_eq!(e.quality_step("ao"), "medium");
        assert_eq!(e.quality_step("aa"), "medium");
        assert_eq!(e.quality_step("shadows"), "soft");
        e.apply_quality("ao", "high");
        e.apply_lighting("none");
        assert_eq!(e.number("umbreon.aoSamples"), 256.);
        e.apply_lighting("ao");
        assert_eq!(e.quality_step("ao"), "high");
        e.apply_lighting("gi");
        assert_eq!(e.quality_step("giLighting"), "4");
        e.values.insert("umbreon.lightIntensity".into(), json!(1.2));
        assert_eq!(e.quality_step("giLighting"), "custom");
        e.values.insert("backend".into(), json!("umbreon_npr"));
        assert_eq!(e.lighting(), "none");
        assert_eq!(e.quality_step("aa"), "high");
        assert_eq!(e.quality_step("shadows"), "off");
        assert_eq!(e.snapshot()["umbreon.aoSamples"], 256);
        assert!(!e.snapshot().contains_key("umbreon.giSamples"));
        e.set_size([600, 300], Some(300.));
        e.set_unit("in");
        assert_eq!(e.number("width"), 2.);
        assert_eq!(e.pixels(), [600, 300]);
        e.values.insert("dpi".into(), json!(600));
        assert_eq!(e.pixels(), [1200, 600]);
        e.set_unit("mm");
        assert!((e.number("width") - 50.8).abs() < 1e-10);
        e.set_unit("px");
        assert_eq!(e.pixels(), [1200, 600]);
    }

    #[test]
    fn edit_transactions_keep_target_and_reload_external_writes() {
        let mut e = RenderEditor::default();
        e.sync(Some(10), &Map::new());
        assert!(!e.pending());
        e.values.insert("dpi".into(), json!(300));
        e.edited(1.);
        assert!(!e.due(Some(10), 1.2, false));
        e.edited(1.2);
        assert!(!e.due(Some(10), 1.4, false));
        assert!(e.due(Some(10), 1.6, false));
        assert!(e.due(Some(20), 1.2, false));
        assert_eq!(e.scene(), Some(10));
        e.saved(&e.values.clone());
        assert!(!e.pending());
        e.sync(Some(10), &Map::new());
        assert_eq!(e.number("dpi"), 600.);
        e.sync(
            Some(20),
            &json!({"dpi":72,"backend":"umbreon_npr"})
                .as_object()
                .unwrap()
                .clone(),
        );
        assert_eq!(e.number("dpi"), 72.);
        assert_eq!(e.backend(), "umbreon_npr");
    }

    #[test]
    fn scoped_write_is_atomic_and_preserves_other_scene_and_backend() {
        let mut state = DesktopAppState::new();
        let first = state.new_scene("first", true).result_scene;
        let mut camera = Camera::new();
        call_tool(
            &mut state,
            &mut camera,
            "set_render_settings",
            &json!({"settings":"umbreon_npr.hatchDensity=2; umbreon.aoResDiv=0"}),
        )
        .unwrap();
        let mut editor = RenderEditor::default();
        editor.sync(Some(first.get()), &render_settings(&state).unwrap());
        assert_eq!(editor.text("umbreon.aoGather"), "Per shading hit");
        editor.apply_lighting("ao");
        editor.apply_quality("ao", "medium");
        editor.values.insert("dpi".into(), json!(300));
        editor.values.insert("backend".into(), json!("umbreon_npr"));
        let second = state.new_scene("second", true).result_scene;
        call_tool(
            &mut state,
            &mut camera,
            "set_render_settings",
            &json!({"sceneId":first.get(),"settings":editor.settings_argument()}),
        )
        .unwrap();
        assert_eq!(state.active_scene(), Some(second));
        assert!(render_settings(&state).unwrap().is_empty());
        let stored = scene_render_settings(&state, first).unwrap();
        assert_eq!(stored["dpi"], 300.);
        assert_eq!(stored["umbreon_npr.hatchDensity"], 2.);
        assert_eq!(stored["umbreon.aoSamples"], 64);
        assert_eq!(
            crate::rendering_params(&stored, "umbreon")
                .unwrap()
                .ao_res_div,
            -1
        );
        let before = state.clone();
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_render_settings",
            &json!({"sceneId":first.get(),"settings":"dpi=72; width=-1"})
        )
        .is_err());
        assert!(before.same_scene_undo_state(&state));
        assert!(call_tool(
            &mut state,
            &mut camera,
            "set_render_settings",
            &json!({"sceneId":u64::MAX,"settings":"dpi=72"})
        )
        .is_err());
        assert!(before.same_scene_undo_state(&state));
    }

    #[test]
    fn legacy_global_ao_does_not_override_editor_and_invalid_targets_do_not_fall_back() {
        let mut state = DesktopAppState::new();
        state.new_scene("target", true);
        let mut camera = Camera::new();
        call_tool(
            &mut state,
            &mut camera,
            "set_render_settings",
            &json!({"settings":"aoResDiv=0; umbreon.aoEnabled=true"}),
        )
        .unwrap();
        let mut editor = RenderEditor::default();
        editor.sync(
            state.active_scene().map(|id| id.get()),
            &render_settings(&state).unwrap(),
        );
        assert_eq!(editor.text("umbreon.aoGather"), "Per shading hit");
        editor.apply_quality("ao", "medium");
        call_tool(
            &mut state,
            &mut camera,
            "set_render_settings",
            &json!({"settings":editor.settings_argument()}),
        )
        .unwrap();
        let values = render_settings(&state).unwrap();
        assert_eq!(
            crate::rendering_params(&values, "umbreon")
                .unwrap()
                .ao_res_div,
            -1
        );
        assert_eq!(
            crate::rendering_params(&values, "umbreon_npr")
                .unwrap()
                .ao_res_div,
            0
        );
        let before = state.clone();
        for id in [-1, 0] {
            assert!(call_tool(
                &mut state,
                &mut camera,
                "set_render_settings",
                &json!({"sceneId":id,"settings":"dpi=72"})
            )
            .is_err());
            assert!(before.same_scene_undo_state(&state));
        }
    }

    #[test]
    fn catalog_rows_have_typed_defaults_and_valid_enum_values() {
        let d = defaults();
        for (block, rows) in catalog().as_object().unwrap() {
            for row in rows.as_array().unwrap() {
                let key = row["key"].as_str().unwrap();
                let path = if block == "common" {
                    key.into()
                } else {
                    format!("{block}.{key}")
                };
                let value = d.get(&path).unwrap_or_else(|| panic!("missing {path}"));
                if row["type"] == "enum" {
                    let choices = row["enumValues"]
                        .as_array()
                        .or_else(|| row["options"].as_array())
                        .unwrap();
                    assert!(choices.iter().any(|v| equal(value, v)), "{path}: {value}");
                }
            }
        }
    }
}
