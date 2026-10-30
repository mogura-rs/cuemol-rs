use cuemol_core::{CueMolError, Result, Uid};
use cuemol_desktop_core::DesktopAppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Keyword {
    pub key: &'static str,
    pub label: &'static str,
    pub emit: &'static str,
    pub kind: &'static str,
    pub candidates: Option<&'static str>,
}

pub const KEYWORDS: &[Keyword] = &[
    Keyword {
        key: "named",
        label: "Named",
        emit: "",
        kind: "named",
        candidates: None,
    },
    Keyword {
        key: "history",
        label: "History",
        emit: "",
        kind: "history",
        candidates: None,
    },
    Keyword {
        key: "hierarchical",
        label: "hier",
        emit: "",
        kind: "hierarchical",
        candidates: None,
    },
    Keyword {
        key: "chain",
        label: "chain",
        emit: "chain",
        kind: "nameList",
        candidates: Some("chain"),
    },
    Keyword {
        key: "resi",
        label: "resid",
        emit: "resid",
        kind: "numList",
        candidates: None,
    },
    Keyword {
        key: "all",
        label: "all",
        emit: "all",
        kind: "none",
        candidates: None,
    },
    Keyword {
        key: "none",
        label: "none",
        emit: "none",
        kind: "none",
        candidates: None,
    },
    Keyword {
        key: "resn",
        label: "resn",
        emit: "resn",
        kind: "nameList",
        candidates: Some("resname"),
    },
    Keyword {
        key: "name",
        label: "name",
        emit: "name",
        kind: "nameList",
        candidates: Some("aname"),
    },
    Keyword {
        key: "elem",
        label: "elem",
        emit: "elem",
        kind: "nameList",
        candidates: Some("elem"),
    },
    Keyword {
        key: "alt",
        label: "Altloc",
        emit: "alt",
        kind: "nameList",
        candidates: None,
    },
    Keyword {
        key: "bfac",
        label: "bfac",
        emit: "bfac",
        kind: "compare",
        candidates: None,
    },
    Keyword {
        key: "rprop",
        label: "rprop",
        emit: "rprop",
        kind: "nameValue",
        candidates: None,
    },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Draft {
    pub keyword: String,
    pub fields: BTreeMap<String, String>,
    pub picked: String,
    pub distance: String,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            keyword: "hierarchical".into(),
            fields: BTreeMap::new(),
            picked: String::new(),
            distance: "5".into(),
        }
    }
}

impl Draft {
    pub fn definition(&self) -> Option<&'static Keyword> {
        KEYWORDS.iter().find(|item| item.key == self.keyword)
    }

    pub fn set_keyword(&mut self, keyword: &str) {
        if self.keyword == keyword {
            return;
        }
        self.keyword = keyword.into();
        self.fields.clear();
        self.picked.clear();
        if keyword == "bfac" {
            self.fields.insert("op".into(), "<".into());
        }
    }

    pub fn term(&self) -> Option<String> {
        let definition = self.definition()?;
        let field = |key: &str| {
            self.fields
                .get(key)
                .map(String::as_str)
                .unwrap_or("")
                .trim()
        };
        let value = field("value");
        let term = match definition.kind {
            "none" => definition.emit.into(),
            "named" | "history" => self.picked.trim().into(),
            "nameList" => {
                let value = name_list(value, self.keyword == "chain");
                if value.is_empty() {
                    return None;
                }
                format!("{} {value}", definition.emit)
            }
            "numList" => {
                let value = number_list(value);
                if value.is_empty() {
                    return None;
                }
                format!("{} {value}", definition.emit)
            }
            "compare" => {
                let op = field("op");
                if op.is_empty() || value.is_empty() {
                    return None;
                }
                format!("{} {op} {value}", definition.emit)
            }
            "nameValue" => {
                let name = field("name");
                if name.is_empty() || value.is_empty() {
                    return None;
                }
                format!("{} {name}={value}", definition.emit)
            }
            "hierarchical" => {
                let mut parts = [
                    name_list(field("chain"), true),
                    number_list(field("resid")),
                    name_list(field("aname"), false),
                ];
                if parts.iter().all(String::is_empty) {
                    return None;
                }
                for part in &mut parts {
                    if part.is_empty() {
                        *part = "*".into();
                    }
                }
                parts.join(".")
            }
            _ => return None,
        };
        (!term.is_empty()).then_some(term)
    }

    pub fn apply(&self, current: &str, operation: &str) -> Option<String> {
        let current = current.trim();
        if operation != "set" && current.is_empty() {
            return None;
        }
        if matches!(operation, "set" | "add" | "sub" | "intersect") {
            let term = self.term()?;
            return Some(match operation {
                "set" => term,
                "add" => format!("({current}) or ({term})"),
                "sub" => format!("({current}) and not ({term})"),
                _ => format!("({current}) and ({term})"),
            });
        }
        let prefix = match operation {
            "not" => "not",
            "byres" => "byres",
            "sidechain" => "bysidech",
            "mainchain" => "bymainch",
            "around" | "expand" => {
                let distance = self.distance.trim();
                distance
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && *v >= 0.)?;
                return Some(format!("({current}) {operation} {distance}"));
            }
            _ => return None,
        };
        Some(format!("{prefix} ({current})"))
    }
}

fn name_list(value: &str, quote: bool) -> String {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            if quote
                && s != "null"
                && !s.starts_with(['/', '\'', '"'])
                && s.chars().any(|c| c.is_ascii_alphabetic())
            {
                format!("'{s}'")
            } else {
                s.into()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn number_list(value: &str) -> String {
    value
        .split(',')
        .map(|part| {
            part.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub molecule_id: Option<u64>,
    pub current: String,
    pub scene: Vec<String>,
    pub global: Vec<String>,
    pub history: Vec<String>,
}

pub fn property_molecule(state: &DesktopAppState, kind: &str, id: Option<u64>) -> Option<Uid> {
    let scene = state.scene_manager().scene(state.active_scene()?)?;
    let id = Uid::new(id?);
    scene
        .objects()
        .iter()
        .find(|object| {
            if kind == "object" {
                object.uid() == id
            } else if matches!(kind, "renderer" | "rendGroup") {
                object.renderer(id).is_some()
            } else {
                false
            }
        })
        .map(|object| object.uid())
}

pub fn context(state: &DesktopAppState, molecule: Option<Uid>) -> Context {
    let (scene, global) = state.selection_definitions();
    Context {
        molecule_id: molecule.map(Uid::get),
        current: molecule
            .and_then(|id| state.molecule_selection_text(id).ok())
            .unwrap_or("")
            .into(),
        scene,
        global,
        history: state.selection_history(),
    }
}

pub fn definition_style_set(state: &DesktopAppState) -> String {
    let Some(scene) = state
        .active_scene()
        .and_then(|id| state.scene_manager().scene(id))
    else {
        return "default".into();
    };
    if let Some(style) = scene
        .loaded_styles()
        .iter()
        .find(|style| !style.node.str_attr("readonly").equals_ignore_case("true"))
    {
        return style.node.str_attr("id").to_string();
    }
    let names = scene
        .loaded_styles()
        .iter()
        .map(|style| style.node.str_attr("id").to_string())
        .collect::<BTreeSet<_>>();
    if !names.contains("default") {
        return "default".into();
    }
    (0..)
        .map(|index| format!("default_{index}"))
        .find(|name| !names.contains(name))
        .unwrap()
}

pub fn values(state: &DesktopAppState, molecule: Option<Uid>, kind: &str) -> Vec<String> {
    let Some(object) = molecule.and_then(|id| state.scene_manager().object(id)) else {
        return Vec::new();
    };
    let atoms = object.mol().atoms();
    let mut values = BTreeSet::new();
    let mut sampled = BTreeSet::new();
    if matches!(kind, "aname" | "elem") {
        for atom in atoms {
            sampled.insert((&atom.chain_id, atom.res_id, atom.ins_code));
            if sampled.len() > 40 {
                sampled.pop_last();
            }
        }
    }
    for atom in atoms {
        let value = match kind {
            "chain" => &atom.chain_id,
            "resname" => &atom.res_name,
            "aname" | "elem" => {
                let residue = (&atom.chain_id, atom.res_id, atom.ins_code);
                if !sampled.contains(&residue) {
                    continue;
                }
                if kind == "aname" {
                    &atom.name
                } else {
                    &atom.element
                }
            }
            _ => return Vec::new(),
        };
        if !value.is_empty() {
            values.insert(value.clone());
        }
    }
    values.into_iter().collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct Evaluation {
    pub valid: bool,
    pub count: Option<usize>,
    pub error: Option<String>,
}

pub fn evaluate(state: &DesktopAppState, molecule: Option<Uid>, expression: &str) -> Evaluation {
    let result = state
        .active_scene()
        .ok_or_else(|| CueMolError::not_found("active scene"))
        .and_then(|scene| state.parse_scene_selector(scene, expression));
    match result {
        Ok(selector) => Evaluation {
            valid: true,
            count: molecule
                .and_then(|id| state.scene_manager().object(id))
                .map(|object| object.mol().atom_sel_count(&selector)),
            error: None,
        },
        Err(error) => Evaluation {
            valid: false,
            count: None,
            error: Some(error.to_string()),
        },
    }
}

pub fn request(state: &DesktopAppState, args: &Value) -> Result<Value> {
    if !args.is_object() {
        return Err(CueMolError::invalid_argument(
            "Selection arguments must be an object",
        ));
    }
    let molecule = if let Some(id) = args["moleculeId"].as_u64() {
        let id = Uid::new(id);
        if state.scene_manager().scene_uid_for_object(id) != state.active_scene() {
            return Err(CueMolError::not_found("molecule in active scene"));
        }
        Some(id)
    } else {
        property_molecule(
            state,
            args["nodeType"].as_str().unwrap_or(""),
            args["nodeId"].as_u64(),
        )
    };
    Ok(match crate::commands::text(args, "action")? {
        "context" => {
            let mut context = json!(context(state, molecule));
            context["keywords"] = json!(KEYWORDS);
            context
        }
        "values" => json!(values(
            state,
            molecule,
            crate::commands::text(args, "kind")?
        )),
        "evaluate" => json!(evaluate(
            state,
            molecule,
            crate::commands::text(args, "expression")?
        )),
        "compose" => {
            let draft: Draft = serde_json::from_value(args["draft"].clone())
                .map_err(|error| CueMolError::invalid_argument(error.to_string()))?;
            json!({"expression":draft.apply(crate::commands::text(args, "current")?, crate::commands::text(args, "operation")?)})
        }
        _ => return Err(CueMolError::invalid_argument("Unknown selection operation")),
    })
}

pub fn remember_property_edit(state: &mut DesktopAppState, tool: &str, args: &Value) -> bool {
    let updates: &[Value] = match tool {
        "set_node_prop" => std::slice::from_ref(args),
        "set_node_props" => args["updates"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default(),
        _ => return false,
    };
    let mut recorded = false;
    for update in updates {
        if matches!(
            update["prop"].as_str(),
            Some("sel" | "selection" | "anchor_sel" | "showsel" | "bndry_sel")
        ) {
            if let Some(value) = update["value"].as_str() {
                state.remember_selection(value);
                recorded = true;
            }
        }
    }
    recorded
}
