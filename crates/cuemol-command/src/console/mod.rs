pub mod actions;
mod completion;
mod parser;
mod pymol;
mod pymol_analysis;
mod pymol_io;
mod pymol_label;
mod pymol_selection;
mod pymol_settings;
mod pymol_view;
mod qax;
mod refs;
mod runtime;
pub use runtime::Submission;

use cuemol_desktop_core::DesktopAppState;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use parser::{bind_arguments, head, lookup, parse_arguments, unquote};
pub use parser::{split_commands, CommandLine};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dialect {
    #[default]
    CueMol,
    PyMol,
    JavaScript,
}

impl Dialect {
    pub fn label(self) -> &'static str {
        match self {
            Self::CueMol => "CueMol",
            Self::PyMol => "PyMOL",
            Self::JavaScript => "JavaScript",
        }
    }

    pub fn prompt(self) -> &'static str {
        match self {
            Self::CueMol => "CueMol>",
            Self::PyMol => "PyM>",
            Self::JavaScript => "Js>",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Tool {
        name: String,
        args: Value,
    },
    Output(String),
    Echo(String),
    Source(String),
    Native(String),
    Qax(String),
    QaxOp(cuemol_core::LDom2Node),
    Transaction(String),
    RememberSetting {
        scene: u64,
        name: String,
        value: Option<String>,
    },
    Render {
        width: u32,
        height: u32,
        ray: bool,
        dpi: Option<f64>,
        path: Option<String>,
    },
    WriteFile {
        path: String,
        bytes: std::sync::Arc<Vec<u8>>,
        mime: String,
    },
    KeepChain {
        name: String,
        chain: String,
    },
    Clear,
    Quit,
}

#[derive(Clone, Debug, Deserialize)]
struct CommandSpec {
    tool: String,
    group: String,
    params: Vec<String>,
    #[serde(default)]
    semantics: BTreeMap<String, String>,
    #[serde(default)]
    defaults: BTreeMap<String, Value>,
    #[serde(default)]
    fixed: BTreeMap<String, Value>,
}

fn catalog() -> &'static BTreeMap<String, CommandSpec> {
    static CATALOG: OnceLock<BTreeMap<String, CommandSpec>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("catalog.json")).expect("valid console catalog")
    })
}

const BUILTINS: &[&str] = &[
    "cd",
    "clear",
    "close_log",
    "exit",
    "help",
    "ls",
    "open_log",
    "pwd",
    "quit",
    "run",
];

#[derive(Clone, Debug)]
pub struct Session {
    cwd: PathBuf,
    remembered_settings: BTreeMap<(u64, String), String>,
    command_log: Option<PathBuf>,
    qax_log: Option<qax::Log>,
    log_label: String,
    log_source: String,
    pub action_record: actions::Record,
    prepared_record: Option<cuemol_core::LDom2Node>,
    render: Option<RenderedImage>,
    revision: u64,
    pub trajectory_frames: BTreeMap<u64, usize>,
}

#[derive(Clone, Debug)]
struct RenderedImage {
    scene: Option<cuemol_core::Uid>,
    camera: cuemol_qsys::Camera,
    bytes: std::sync::Arc<Vec<u8>>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            remembered_settings: BTreeMap::new(),
            command_log: None,
            qax_log: None,
            log_label: String::new(),
            log_source: String::new(),
            action_record: actions::Record::default(),
            prepared_record: None,
            render: None,
            revision: 0,
            trajectory_frames: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub replacement: String,
    pub display: String,
}

#[derive(Clone, Debug)]
struct Node {
    id: u64,
    kind: &'static str,
    class: String,
    path: String,
    name: String,
}

fn nodes(state: &DesktopAppState) -> Result<Vec<Node>, String> {
    if state.active_scene().is_none() {
        return Ok(Vec::new());
    }
    let scene = state
        .scene_manager()
        .scene(state.active_scene().unwrap())
        .ok_or("No active scene")?;
    let mut result = Vec::new();
    for object in scene.objects() {
        let name = object.name();
        result.push(Node {
            id: object.uid().get(),
            kind: "object",
            class: object.object_type().into(),
            path: name.into(),
            name: name.into(),
        });
        for renderer in object.renderers() {
            result.push(Node {
                id: renderer.uid().get(),
                kind: if ["*group", "rendGroup"].contains(&renderer.renderer_type()) {
                    "rendGroup"
                } else {
                    "renderer"
                },
                class: renderer.renderer_type().into(),
                path: format!("{name}/{}", renderer.name()),
                name: renderer.name().into(),
            });
        }
    }
    let data =
        json!({"objects":crate::nodes::extra_scene_objects(state).map_err(|e| e.to_string())?});
    for object in data["objects"].as_array().into_iter().flatten() {
        let name = object["name"].as_str().unwrap_or("").to_string();
        result.push(Node {
            id: object["id"].as_u64().unwrap(),
            kind: "object",
            class: object["className"].as_str().unwrap_or("").into(),
            path: name.clone(),
            name: name.clone(),
        });
        for renderer in object["renderers"].as_array().into_iter().flatten() {
            let label = renderer["name"].as_str().unwrap_or("").to_string();
            result.push(Node {
                id: renderer["id"].as_u64().unwrap(),
                kind: if renderer["type"] == "*group" || renderer["type"] == "rendGroup" {
                    "rendGroup"
                } else {
                    "renderer"
                },
                class: renderer["type"].as_str().unwrap_or("").into(),
                path: format!("{name}/{label}"),
                name: label,
            });
        }
    }
    Ok(result)
}

fn node_accepts(node: &Node, semantic: &str) -> bool {
    match semantic {
        "renderer" => node.kind != "object",
        "node" => true,
        "object" => node.kind == "object",
        kind if kind.ends_with(" object") => {
            node.kind == "object"
                && (node.class == kind.trim_end_matches(" object")
                    || kind == "MolCoord object"
                        && matches!(node.class.as_str(), "AnimMol" | "MorphMol" | "Trajectory"))
        }
        _ => false,
    }
}

fn resolve_node<'a>(nodes: &'a [Node], word: &str, semantic: &str) -> Result<&'a Node, String> {
    refs::resolve(nodes, word, semantic)
}

fn tool_schema(name: &str) -> &'static Value {
    &crate::tools_list()
        .iter()
        .find(|tool| tool["name"] == name)
        .expect("console tool exists")["inputSchema"]
}

fn typed_value(text: &str, spec: &Value) -> Result<Value, String> {
    let decoded = serde_json::from_str::<String>(text).ok();
    let value = decoded.as_deref().unwrap_or_else(|| unquote(text));
    let kind = spec["type"]
        .as_str()
        .or_else(|| {
            spec["type"]
                .as_array()?
                .iter()
                .find_map(|v| v.as_str().filter(|s| *s != "null"))
        })
        .unwrap_or("string");
    match kind {
        "string" => Ok(json!(value)),
        "boolean" => match value.to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => Ok(json!(true)),
            "false" | "off" | "no" | "0" => Ok(json!(false)),
            _ => Err(format!("Expected true or false, not {value}")),
        },
        "integer" => value
            .parse::<i64>()
            .map(|v| json!(v))
            .map_err(|_| format!("Expected an integer, not {value}")),
        "number" => value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| json!(v))
            .ok_or_else(|| format!("Expected a finite number, not {value}")),
        "array" | "object" => {
            serde_json::from_str(value).map_err(|e| format!("Expected {kind}: {e}"))
        }
        _ => Err(format!("Unsupported argument type: {kind}")),
    }
}

impl Session {
    pub fn plan_dialect(
        &mut self,
        state: &DesktopAppState,
        camera: &cuemol_qsys::Camera,
        dialect: Dialect,
        line: &str,
    ) -> Result<Vec<Action>, String> {
        match dialect {
            Dialect::CueMol => self.plan(state, line),
            Dialect::PyMol => pymol::plan(self, state, camera, line),
            Dialect::JavaScript => Ok(vec![Action::Tool {
                name: "execute_script".into(),
                args: json!({"source": line}),
            }]),
        }
    }

    pub fn help_dialect(&self, dialect: Dialect, topic: &str) -> Result<String, String> {
        match dialect {
            Dialect::CueMol => self.help(topic),
            Dialect::PyMol => pymol::help(topic),
            Dialect::JavaScript => Ok("JavaScript: use cuemol, scene and view. Enter runs the input; Shift+Enter inserts a line; Up/Down recalls history.".into()),
        }
    }

    pub fn complete_dialect(
        &self,
        state: &DesktopAppState,
        dialect: Dialect,
        source: &str,
    ) -> Vec<Completion> {
        match dialect {
            Dialect::CueMol => self.complete(state, source),
            Dialect::PyMol => pymol::complete(self, state, source),
            Dialect::JavaScript => Vec::new(),
        }
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn resolve_path(&self, text: &str) -> PathBuf {
        let decoded = serde_json::from_str::<String>(text).ok();
        let text = decoded.as_deref().unwrap_or_else(|| unquote(text));
        let path = if text == "~" || text.starts_with("~/") {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .map(|home| home.join(text.trim_start_matches('~').trim_start_matches('/')))
                .unwrap_or_else(|| PathBuf::from(text))
        } else {
            PathBuf::from(text)
        };
        if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        }
    }

    pub fn plan(&mut self, state: &DesktopAppState, line: &str) -> Result<Vec<Action>, String> {
        let (word, rest) = head(line);
        if let Some(path) = line.trim().strip_prefix('@') {
            return self.read_script(path);
        }
        if word.starts_with('/') {
            return Err("Python expressions are not available in the CueMol command console; use the Python script editor".into());
        }
        let name = lookup(
            word,
            catalog()
                .keys()
                .map(String::as_str)
                .chain(BUILTINS.iter().copied()),
        )?;
        if name == "help" || rest.trim() == "?" {
            return Ok(vec![Action::Output(self.help(if name == "help" {
                unquote(rest)
            } else {
                name
            })?)]);
        }
        match name {
            "clear" => return no_args(rest, Action::Clear),
            "quit" | "exit" => return no_args(rest, Action::Quit),
            "pwd" => return no_args(rest, Action::Output(self.cwd.display().to_string())),
            "run" => {
                let args = bind_arguments(&["file".into()], &parse_arguments(rest, false)?)?;
                return self.read_script(args.get("file").ok_or("A script file is required")?);
            }
            "open_log" => {
                let args = bind_arguments(
                    &["file".into(), "mode".into()],
                    &parse_arguments(rest, false)?,
                )?;
                return self.open_qax_log(
                    args.get("file").map(|v| unquote(v)).unwrap_or("log.qax"),
                    args.get("mode").map(|v| unquote(v)).unwrap_or("w"),
                );
            }
            "close_log" => {
                if !rest.is_empty() {
                    return Err("close_log takes no arguments".into());
                }
                return self.close_qax_log();
            }
            "cd" | "ls" => {
                let args = bind_arguments(&["dir".into()], &parse_arguments(rest, false)?)?;
                return self
                    .directory_command(name, args.get("dir").map(String::as_str).unwrap_or(""));
            }
            _ => {}
        }
        let spec = &catalog()[name];
        let args = parse_arguments(rest, false)?;
        let mut params = spec.params.clone();
        let schema = tool_schema(&spec.tool);
        for key in schema["properties"].as_object().unwrap().keys() {
            if !params.contains(key)
                && !spec.fixed.contains_key(key)
                && !(key == "sceneId" && params.iter().any(|p| p == "scene"))
            {
                params.push(key.clone());
            }
        }
        let bound = bind_arguments(&params, &args)?;
        let (bindings, mut actions) = refs::expand(self, &nodes(state)?, spec, &bound)?;
        for bound in bindings {
            let values = self.bind_tool(state, spec, &bound)?;
            crate::validate_args(&spec.tool, &values)?;
            actions.push(Action::Tool {
                name: spec.tool.clone(),
                args: values,
            });
        }
        Ok(actions)
    }

    fn bind_tool(
        &self,
        state: &DesktopAppState,
        spec: &CommandSpec,
        bound: &BTreeMap<String, String>,
    ) -> Result<Value, String> {
        let schema = tool_schema(&spec.tool);
        let mut values = Map::new();
        for (key, value) in spec.defaults.iter().chain(&spec.fixed) {
            values.insert(key.clone(), value.clone());
        }
        let inventory = if spec.semantics.values().any(|kind| {
            kind == "node" || kind == "renderer" || (kind == "object" || kind.ends_with(" object"))
        }) {
            nodes(state)?
        } else {
            Vec::new()
        };
        for (key, raw) in bound {
            if raw.trim().is_empty() {
                continue;
            }
            if raw.trim() == "null" {
                values.insert(key.clone(), Value::Null);
                continue;
            }
            let semantic = spec.semantics.get(key).map(String::as_str).unwrap_or("");
            if semantic == "scene" {
                let word = unquote(raw);
                let scenes = state.scene_summaries();
                let found = if let Some(id) =
                    word.strip_prefix('#').and_then(|id| id.parse::<u64>().ok())
                {
                    scenes.iter().find(|scene| scene.uid.get() == id)
                } else if let Ok(index) = word.parse::<usize>() {
                    index.checked_sub(1).and_then(|index| scenes.get(index))
                } else {
                    let matching = scenes
                        .iter()
                        .filter(|scene| scene.name == word)
                        .collect::<Vec<_>>();
                    if matching.len() != 1 {
                        return Err(format!("Scene name is missing or ambiguous: {word}"));
                    }
                    matching.first().copied()
                }
                .ok_or_else(|| format!("No scene: {word}"))?;
                values.insert("sceneId".into(), json!(found.uid.get()));
                continue;
            }
            if semantic == "node"
                || semantic == "renderer"
                || (semantic == "object" || semantic.ends_with(" object"))
            {
                let word = unquote(raw);
                let type_spec = &schema["properties"]["nodeType"];
                if semantic == "node"
                    && matches!(word, "scene" | "view")
                    && resolve_node(&inventory, word, semantic).is_err()
                    && type_spec["enum"]
                        .as_array()
                        .is_some_and(|types| types.contains(&json!(word)))
                {
                    values.insert(key.clone(), Value::Null);
                    values.entry("nodeType").or_insert(json!(word));
                } else {
                    let node = resolve_node(&inventory, raw, semantic)?;
                    values.insert(key.clone(), json!(node.id));
                    if semantic == "node" && !type_spec.is_null() && !bound.contains_key("nodeType")
                    {
                        let kind = if node.kind == "rendGroup"
                            && !type_spec["enum"]
                                .as_array()
                                .is_some_and(|types| types.contains(&json!("rendGroup")))
                        {
                            "renderer"
                        } else {
                            node.kind
                        };
                        values.insert("nodeType".into(), json!(kind));
                    }
                }
            } else if semantic == "path" {
                values.insert(key.clone(), json!(self.resolve_path(raw).to_string_lossy()));
            } else if semantic == "atoms" {
                let atoms = unquote(raw)
                    .split_whitespace()
                    .map(|word| {
                        let parts = word.split('.').collect::<Vec<_>>();
                        if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
                            return Err(format!("Expected chain.residue.atom, not {word}"));
                        }
                        Ok(json!({"chain":parts[0],"resid":parts[1],"atomName":parts[2]}))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                values.insert(key.clone(), json!(atoms));
            } else if semantic == "x, y, z" {
                let text = unquote(raw);
                let coordinates = if text.starts_with('[') {
                    serde_json::from_str(text).map_err(|e| format!("Expected x y z: {e}"))?
                } else {
                    Value::Array(
                        text.split_whitespace()
                            .map(|n| typed_value(n, &json!({"type":"number"})))
                            .collect::<Result<_, _>>()?,
                    )
                };
                values.insert(key.clone(), coordinates);
            } else {
                let property = &schema["properties"][key];
                if property.is_null() {
                    return Err(format!("Unknown argument: {key}"));
                }
                values.insert(
                    key.clone(),
                    typed_value(raw, property).map_err(|error| format!("{key}: {error}"))?,
                );
            }
        }
        if schema["properties"]["nodeType"]["enum"]
            .as_array()
            .is_some_and(|types| types.contains(&json!("scene")))
            && !values.contains_key("nodeType")
        {
            values.insert("nodeType".into(), json!("scene"));
        }
        Ok(Value::Object(values))
    }

    fn read_script(&self, text: &str) -> Result<Vec<Action>, String> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = text;
            Err(
                "Open a local script with the browser file picker, then paste its commands here"
                    .into(),
            )
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.resolve_path(text);
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
                return Err("Console scripts are limited to 4 MiB".into());
            }
            let qax = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("qax"));
            let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            if qax {
                return Ok(vec![Action::Qax(source)]);
            }
            Ok(vec![Action::Source(source)])
        }
    }

    fn directory_command(&mut self, name: &str, text: &str) -> Result<Vec<Action>, String> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (name, text);
            Err("The browser has no working-directory access".into())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.resolve_path(if text.is_empty() {
                if name == "cd" {
                    "~"
                } else {
                    "."
                }
            } else {
                text
            });
            if name == "cd" {
                if !path.is_dir() {
                    return Err(format!("No such directory: {}", path.display()));
                }
                self.cwd = path.canonicalize().map_err(|e| e.to_string())?;
                return Ok(vec![Action::Output(self.cwd.display().to_string())]);
            }
            let mut entries = std::fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .map(|entry| {
                    let entry = entry.map_err(|e| e.to_string())?;
                    Ok(format!(
                        "{}{}",
                        entry.file_name().to_string_lossy(),
                        if entry.path().is_dir() { "/" } else { "" }
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            entries.sort();
            Ok(vec![Action::Output(entries.join("\n"))])
        }
    }

    pub fn help(&self, topic: &str) -> Result<String, String> {
        if topic.is_empty() || topic == "commands" {
            let mut groups = BTreeMap::<&str, usize>::new();
            for spec in catalog().values() {
                *groups.entry(&spec.group).or_default() += 1;
            }
            let rows = groups
                .into_iter()
                .map(|(group, count)| format!("help {group:<14} {count} commands"))
                .collect::<Vec<_>>();
            return Ok(format!("CueMol commands\n{}\n\nUse help <command> or <command> ? for arguments.\nArguments are comma separated; names accept object/renderer or #uid.\nEnter runs the input. Shift+Enter inserts a line. Tab completes a name.", rows.join("\n")));
        }
        if topic == "syntax" {
            return Ok("Arguments are comma separated: zoom protein, chain A and resid 10:20\nNamed arguments follow positional arguments: create_renderer protein, cartoon, name=main\nQuote names containing spaces or commas. Use object/renderer or #uid.\nSeparate commands with semicolons or newlines. A trailing backslash continues a line.\nA line beginning with # is a comment. @file reads a command script.\nSelect JavaScript to run cuemol, scene and view scripts.".into());
        }
        let grouped = catalog()
            .iter()
            .filter(|(_, spec)| spec.group == topic)
            .map(|(name, spec)| {
                let tool = crate::tools_list()
                    .iter()
                    .find(|tool| tool["name"] == spec.tool)
                    .unwrap();
                format!("{name:<24} {}", tool["description"].as_str().unwrap_or(""))
            })
            .collect::<Vec<_>>();
        if !grouped.is_empty() {
            return Ok(grouped.join("\n"));
        }
        if BUILTINS.contains(&topic) {
            return Ok(match topic {
                "cd" => "cd [directory] — change the console working directory",
                "ls" => "ls [directory] — list files in a directory",
                "pwd" => "pwd — show the console working directory",
                "run" => "run file — replay QAX actions or a text file of CueMol commands",
                "open_log" => "open_log [file=log.qax], [mode=w] — record completed edits as native QAX operations",
                "close_log" => "close_log — stop recording QAX operations",
                "quit" | "exit" => "quit — close the application",
                "clear" => "clear — clear the console output",
                _ => "help [command or subject] — show commands or their arguments",
            }
            .into());
        }
        let name = lookup(topic, catalog().keys().map(String::as_str))?;
        let spec = &catalog()[name];
        let schema = tool_schema(&spec.tool);
        let parameters = spec
            .params
            .iter()
            .map(|key| {
                let optional = !schema["required"]
                    .as_array()
                    .is_some_and(|required| required.contains(&json!(key)))
                    || spec.defaults.contains_key(key);
                if optional {
                    format!("[{key}]")
                } else {
                    key.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let tool = crate::tools_list()
            .iter()
            .find(|tool| tool["name"] == spec.tool)
            .unwrap();
        Ok(format!(
            "{name} {parameters}\n{}",
            tool["description"].as_str().unwrap_or("")
        ))
    }

    pub fn complete(&self, state: &DesktopAppState, source: &str) -> Vec<Completion> {
        completion::native(self, state, source)
    }
}

fn no_args(text: &str, action: Action) -> Result<Vec<Action>, String> {
    if text.trim().is_empty() {
        Ok(vec![action])
    } else {
        Err("This command takes no arguments".into())
    }
}

pub fn format_output(value: &Value) -> String {
    if let Some(log) = value["log"].as_str() {
        log.to_string()
    } else if value["_image"].is_string() {
        format!("PNG image: {} x {}", value["width"], value["height"])
    } else {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    }
}

pub fn same_camera_view(a: &cuemol_qsys::Camera, b: &cuemol_qsys::Camera) -> bool {
    a.zoom() == b.zoom()
        && a.slab_depth() == b.slab_depth()
        && a.cam_dist() == b.cam_dist()
        && a.center() == b.center()
        && a.rotation() == b.rotation()
        && a.is_perspective() == b.is_perspective()
        && a.stereo_mode() == b.stereo_mode()
        && a.stereo_distance() == b.stereo_distance()
        && a.center_mark() == b.center_mark()
        && a.visibility_settings().eq(b.visibility_settings())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::Vector4;
    use cuemol_qsys::Camera;

    pub(super) fn state() -> DesktopAppState {
        let mut state = DesktopAppState::new();
        state.open_bytes("protein.pdb", b"ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2       2.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let id = state.active_object_summaries()[0].uid;
        state
            .scene_manager_mut()
            .object_mut(id)
            .unwrap()
            .set_name("protein");
        state
    }

    #[test]
    fn catalog_targets_and_arguments_exist() {
        for (name, spec) in catalog() {
            let schema = tool_schema(&spec.tool);
            for param in spec.params.iter().chain(spec.fixed.keys()) {
                assert!(
                    schema["properties"].get(param).is_some()
                        || param == "scene" && schema["properties"].get("sceneId").is_some(),
                    "{name}.{param}"
                );
            }
        }
    }

    #[test]
    fn native_aliases_resolve_names_and_use_the_shared_commands() {
        let mut state = state();
        let mut camera = Camera::new();
        let mut session = Session::default();
        for source in [
            "select protein, resid 2",
            "hide protein",
            "show protein",
            "rename protein, renamed",
            "create_renderer renamed, cartoon, name=main",
            "set renamed/main.visible, false",
            "slab 20",
            "view center=1 2 3",
        ] {
            for action in session.plan(&state, source).unwrap() {
                if let Action::Tool { name, args } = action {
                    crate::call_tool(&mut state, &mut camera, &name, &args).unwrap();
                }
            }
        }
        let object = &state.active_object_summaries()[0];
        assert_eq!(object.name, "renamed");
        assert!(object.visible);
        assert_eq!(
            state.molecule_selection_text(object.uid).unwrap(),
            "resid 2"
        );
        assert_eq!(camera.slab_depth(), 20.);
        assert_eq!(camera.center(), Vector4::new3(1., 2., 3.));
        assert!(
            !state
                .renderer_summaries(object.uid)
                .iter()
                .find(|r| r.name == "main")
                .unwrap()
                .visible
        );
        let completion = session.complete(&state, "hide rena");
        assert!(completion.iter().any(|c| c.replacement == "hide renamed "));
        assert!(session.plan(&state, "select missing, *").is_err());
        assert!(session
            .plan(&state, "create_renderer renamed, cartoon, nam=wrong")
            .is_err());
    }
}
