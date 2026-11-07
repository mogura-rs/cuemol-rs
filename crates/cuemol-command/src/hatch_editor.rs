use cuemol_render::{HatchLook, HatchRenderParams};
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

pub type Values = Map<String, Value>;

pub fn catalog() -> &'static Value {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    CATALOG.get_or_init(|| serde_json::from_str(include_str!("hatch_editor_catalog.json")).unwrap())
}

#[derive(Clone)]
pub struct HatchEditor {
    pub layers: Vec<Values>,
    pub tone: Values,
    pub ink: Values,
    template: String,
}

#[derive(Clone, Copy)]
pub struct Environment {
    pub ao_enabled: bool,
    pub base_is_albedo: bool,
    pub density: f64,
    pub width_scale: f64,
    pub supersample: f64,
}

impl Environment {
    pub fn new(params: &HatchRenderParams, look: &HatchEditor, ao: bool, supersample: f64) -> Self {
        Self {
            ao_enabled: ao,
            base_is_albedo: if params.coloring == "Style default" || params.coloring.is_empty() {
                look.ink.get("base").and_then(Value::as_str) == Some("albedo")
            } else {
                params.coloring.contains("color fill")
            },
            density: params.density,
            width_scale: params.width_scale,
            supersample,
        }
    }
}

fn number(values: &Values, key: &str) -> f64 {
    values.get(key).and_then(Value::as_f64).unwrap_or(0.)
}

pub fn enabled(section: &str, key: &str, values: &Values, env: Environment) -> bool {
    match section {
        "layer" => {
            let kind = values.get("kind").and_then(Value::as_str).unwrap_or("line");
            match key {
                "tonelo" => {
                    kind == "stipple"
                        || number(values, "subdiv") > 0.
                        || number(values, "fade") <= 0.
                }
                "wobwave" => number(values, "wobble") > 0. || number(values, "wjitter") > 0.,
                "sgap" | "taper" | "anglejitter" | "lenjitter" => number(values, "slen") > 0.,
                "toothscale" => number(values, "tooth") > 0.,
                "dotangle" => !(number(values, "aspect") == 1. && number(values, "shape") == 2.),
                "seed" => {
                    kind == "stipple"
                        || number(values, "jitter") > 0.
                        || number(values, "tooth") > 0.
                        || (kind == "line"
                            && ["wobble", "wjitter", "slen"]
                                .iter()
                                .any(|key| number(values, key) > 0.))
                }
                _ => true,
            }
        }
        "tone" => match key {
            "hlsoft" => number(values, "hl") < 1.,
            "rimpow" | "rimbias" => number(values, "rim") > 0.,
            "contact" | "shape" => env.ao_enabled,
            _ => true,
        },
        "ink" if key == "albedoquant" => env.base_is_albedo,
        _ => true,
    }
}

pub fn hint(key: &str, value: f64, env: Environment) -> Option<String> {
    let format = |number: f64| {
        format!("{number:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    };
    match key {
        "spacing" => {
            let spacing = value / env.density.max(0.1);
            let mut parts = Vec::new();
            if env.density != 1. {
                parts.push(format!(
                    "Effective {} px at Mark density ×{}",
                    format(spacing),
                    format(env.density)
                ));
            }
            if spacing * env.supersample.max(1.) < 2. {
                parts.push(
                    "Below the 2 px minimum pitch on the supersample grid; increase Supersampling"
                        .into(),
                );
            }
            (!parts.is_empty()).then(|| parts.join("; "))
        }
        "width" | "dotscale" if env.width_scale != 1. => Some(format!(
            "Effective {}{} at Mark width ×{}",
            format(value * env.width_scale),
            if key == "width" { " px" } else { "" },
            format(env.width_scale),
        )),
        _ => None,
    }
}

fn defaults(section: &str) -> Values {
    catalog()[section]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| (field["key"].as_str().unwrap().into(), field["def"].clone()))
        .collect()
}

pub fn new_layer(kind: &str) -> Values {
    let mut values = defaults("layer");
    values.insert("kind".into(), json!(kind));
    values
}

impl HatchEditor {
    pub fn load(params: &HatchRenderParams) -> Result<Self, String> {
        let mut look = HatchLook::style(&params.style).map_err(|e| e.to_string())?;
        let template = look.spec();
        look.apply_spec(&params.layers_spec, 1)
            .and_then(|_| look.apply_spec(&params.tone_spec, 6))
            .map_err(|e| e.to_string())?;
        let mut editor = Self {
            layers: Vec::new(),
            tone: defaults("tone"),
            ink: defaults("ink"),
            template,
        };
        for line in look.spec().lines() {
            let Some((section, entries)) = line.split_once(':') else {
                continue;
            };
            let mut values = if section == "layer" {
                new_layer("line")
            } else {
                defaults(section)
            };
            for entry in entries.split(',') {
                let Some((key, raw)) = entry.split_once('=') else {
                    continue;
                };
                if section == "ink" && key == "levels" {
                    editor.tone.insert(
                        key.into(),
                        json!(raw.parse::<i32>().map_err(|e| e.to_string())?),
                    );
                    continue;
                }
                let value = match values.get(key) {
                    Some(Value::Bool(_)) => json!(raw == "true" || raw == "on" || raw == "1"),
                    Some(Value::Number(_)) => {
                        json!(raw.parse::<f64>().map_err(|e| e.to_string())?)
                    }
                    _ => json!(raw),
                };
                values.insert(key.into(), value);
            }
            match section {
                "layer" => editor.layers.push(values),
                "tone" => editor.tone = values,
                "ink" => editor.ink = values,
                _ => {}
            }
        }
        Ok(editor)
    }

    pub fn specs(&self) -> (String, String) {
        fn line(section: &str, values: &Values) -> String {
            let values = values
                .iter()
                .map(|(key, value)| {
                    let value = match value {
                        Value::String(value) => value.clone(),
                        Value::Number(value) => value.as_f64().unwrap().to_string(),
                        value => value.to_string(),
                    };
                    format!("{key}={value}")
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{section}:{values}")
        }
        (
            self.layers
                .iter()
                .map(|layer| line("layer", layer))
                .collect::<Vec<_>>()
                .join("\n"),
            format!("{}\n{}", line("tone", &self.tone), line("ink", &self.ink)),
        )
    }

    pub fn dirty(&self) -> bool {
        let (layers, tone) = self.specs();
        let mut look = HatchLook::default();
        look.apply_spec(&layers, 1)
            .and_then(|_| look.apply_spec(&tone, 6))
            .is_err()
            || look.spec() != self.template
    }

    pub fn fields(section: &str, values: &Values, env: Environment) -> Vec<Value> {
        catalog()[section]
            .as_array()
            .unwrap()
            .iter()
            .filter(|field| {
                field["type"] != "str"
                    && field
                        .get("kinds")
                        .is_none_or(|kinds| kinds.as_array().unwrap().contains(&values["kind"]))
            })
            .map(|field| {
                let mut field = field.clone();
                let key = field["key"].as_str().unwrap().to_string();
                field["value"] = values.get(&key).cloned().unwrap_or(Value::Null);
                field["enabled"] = json!(enabled(section, &key, values, env));
                field["hint"] = if section == "layer" {
                    json!(hint(&key, number(values, &key), env))
                } else {
                    Value::Null
                };
                field
            })
            .collect()
    }

    pub fn snapshot(&self, env: Environment) -> Value {
        json!({
            "dirty":self.dirty(),
            "layers":self.layers.iter().map(|layer| Self::fields("layer", layer, env)).collect::<Vec<_>>(),
            "tone":Self::fields("tone", &self.tone, env),
            "ink":Self::fields("ink", &self.ink, env),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hatch_editor_round_trips_every_style_and_keeps_tone_levels_with_tone() {
        for style in cuemol_render::HATCH_STYLES {
            let params = HatchRenderParams {
                style: style.into(),
                ..Default::default()
            };
            let mut editor = HatchEditor::load(&params).unwrap();
            assert!(!editor.dirty(), "{style}");
            let original = editor.layers.clone();
            editor.tone.insert("strength".into(), json!(1.5));
            editor.tone.insert("curve".into(), json!(0.75));
            editor.tone.insert("levels".into(), json!(4));
            editor.ink.insert("albedoquant".into(), json!(3));
            let (layers_spec, tone_spec) = editor.specs();
            assert!(tone_spec.lines().next().unwrap().contains("levels=4"));
            assert!(!tone_spec.lines().nth(1).unwrap().contains("levels="));
            let changed = HatchRenderParams {
                layers_spec,
                tone_spec,
                ..params.clone()
            };
            let restored = HatchEditor::load(&changed).unwrap();
            assert!(restored.dirty());
            assert_eq!(restored.layers, original);
            assert_eq!(restored.tone["strength"], 1.5);
            assert_eq!(restored.tone["curve"], 0.75);
            assert_eq!(restored.tone["levels"], 4);
            assert_eq!(restored.ink["albedoquant"], 3.);
            let mut rendered = HatchLook::style(style).unwrap();
            rendered.apply_spec(&changed.layers_spec, 1).unwrap();
            rendered.apply_spec(&changed.tone_spec, 6).unwrap();
            assert_eq!(rendered.tone.strength, 1.5);
            assert_eq!(rendered.levels, 4);
            assert!(!HatchEditor::load(&params).unwrap().dirty());
        }
    }

    #[test]
    fn hatch_editor_context_gates_inactive_fields_and_reports_effective_sizes() {
        let params = HatchRenderParams {
            density: 2.,
            width_scale: 3.,
            ..Default::default()
        };
        let editor = HatchEditor::load(&params).unwrap();
        let env = Environment::new(&params, &editor, false, 1.);
        let mut layer = new_layer("line");
        for key in ["seed", "wobwave", "toothscale", "sgap", "taper"] {
            assert!(!enabled("layer", key, &layer, env), "{key}");
        }
        layer.insert("slen".into(), json!(10));
        assert!(enabled("layer", "seed", &layer, env));
        assert!(enabled("layer", "sgap", &layer, env));
        assert!(!enabled("tone", "contact", &editor.tone, env));
        assert!(!enabled("ink", "albedoquant", &editor.ink, env));
        let hint = hint("spacing", 1., env).unwrap();
        assert!(hint.contains("Effective 0.5 px") && hint.contains("minimum pitch"));
        assert!(super::hint("width", 2., env)
            .unwrap()
            .contains("Effective 6 px"));
    }
}
