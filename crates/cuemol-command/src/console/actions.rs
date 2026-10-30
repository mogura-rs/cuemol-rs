use super::{catalog, nodes, qax, resolve_node, Action, Session};
use cuemol_core::LDom2Node;
use cuemol_desktop_core::DesktopAppState;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;

pub const PRESETS: &str = include_str!("default_actions.qax");
const MAX_RECORD_BYTES: usize = 1_048_576;

pub fn targets(state: &DesktopAppState, selected: &[u64]) -> Vec<(u64, String)> {
    let inventory = nodes(state).unwrap_or_default();
    let objects: Vec<_> = inventory
        .into_iter()
        .filter(|node| node.kind == "object")
        .collect();
    let single = selected.is_empty() && objects.len() == 1;
    objects
        .into_iter()
        .filter(|node| single || selected.contains(&node.id))
        .map(|node| (node.id, node.name))
        .collect()
}

#[derive(Clone, Debug)]
pub struct Record(Arc<Mutex<RecordData>>);

#[derive(Debug)]
struct RecordData {
    id: usize,
    label: String,
    source: String,
    items: Vec<LDom2Node>,
    bytes: usize,
    truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntryView {
    pub command: String,
    pub args: Vec<(String, String)>,
    pub note: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordView {
    pub id: usize,
    pub label: String,
    pub source: String,
    pub entries: Vec<EntryView>,
}

impl Default for Record {
    fn default() -> Self {
        Self::new("Edit scene", "gui")
    }
}

impl Record {
    pub fn new(label: &str, source: &str) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        Self(Arc::new(Mutex::new(RecordData {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            label: label.chars().take(200).collect(),
            source: source.into(),
            items: Vec::new(),
            bytes: 0,
            truncated: false,
        })))
    }

    pub fn id(&self) -> usize {
        self.0.lock().unwrap().id
    }

    pub fn entry_count(&self) -> usize {
        self.0.lock().unwrap().items.len()
    }

    pub fn label(&self) -> String {
        self.0.lock().unwrap().label.clone()
    }

    pub fn append(&self, item: LDom2Node) {
        let mut data = self.0.lock().unwrap();
        if data.truncated {
            return;
        }
        let bytes = qax::serialize(&item).len();
        if data.bytes.saturating_add(bytes) > MAX_RECORD_BYTES {
            let mut note = LDom2Node::new();
            note.set_tag_name("note");
            note.set_str_attr("incomplete", "true");
            note.set_contents(
                "Recording exceeded the size limit; the remaining operations were not recorded.",
            );
            data.items.push(note);
            data.truncated = true;
        } else {
            data.bytes += bytes;
            data.items.push(item);
        }
    }

    pub fn view(&self) -> RecordView {
        let data = self.0.lock().unwrap();
        RecordView {
            id: data.id,
            label: data.label.clone(),
            source: data.source.clone(),
            entries: data.items.iter().map(entry_view).collect(),
        }
    }

    fn node(&self) -> LDom2Node {
        let data = self.0.lock().unwrap();
        let mut node = LDom2Node::new();
        node.set_tag_name("action");
        node.set_str_attr("label", &data.label);
        node.set_str_attr("source", &data.source);
        if data.items.is_empty() {
            node.append_child("note")
                .set_contents("No operation was recorded for this edit.");
        } else {
            for item in &data.items {
                node.append_child_node(item.clone());
            }
        }
        node
    }

    pub fn operation(state: &DesktopAppState, name: &str, args: &serde_json::Value) -> Self {
        let label = match name {
            "rename_node" => format!("Rename to {}", args["name"].as_str().unwrap_or("")),
            "set_node_prop" => format!("Change {}", args["prop"].as_str().unwrap_or("property")),
            "set_visible" => if args["visible"] == false {
                "Hide selection"
            } else {
                "Show selection"
            }
            .into(),
            _ => {
                let mut label = name.replace('_', " ");
                if let Some(first) = label.get_mut(..1) {
                    first.make_ascii_uppercase();
                }
                label
            }
        };
        let record = Self::new(&label, "gui");
        if let Some(op) = Session::record_operation(state, name, args) {
            record.append(op);
        }
        record
    }
}

fn argument_value(arg: &LDom2Node) -> String {
    if !arg.str_attr("path").is_empty() {
        return arg.str_attr("path").to_string();
    }
    if !arg.str_attr("uid").is_empty() {
        return format!("#{}", arg.str_attr("uid"));
    }
    if qax::elements(arg).next().is_some() {
        return qax::elements(arg)
            .map(|atom| {
                format!(
                    "{}.{}.{}",
                    atom.str_attr("chain"),
                    atom.str_attr("resid"),
                    atom.str_attr("name")
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
    }
    arg.contents().into()
}

fn entry_view(item: &LDom2Node) -> EntryView {
    match item.tag_name() {
        "op" => EntryView {
            command: item.str_attr("name").to_string(),
            args: qax::elements(item)
                .map(|arg| (arg.str_attr("name").to_string(), argument_value(arg)))
                .collect(),
            note: false,
        },
        "cmd" => EntryView {
            command: item.contents().into(),
            args: Vec::new(),
            note: false,
        },
        _ => EntryView {
            command: item.contents().into(),
            args: Vec::new(),
            note: true,
        },
    }
}

fn document() -> LDom2Node {
    let mut doc = LDom2Node::new();
    doc.set_tag_name("cuemol-actions");
    doc.set_str_attr("version", "1");
    doc
}

pub fn export(records: &[Record]) -> String {
    let mut doc = document();
    for record in records {
        doc.append_child_node(record.node());
    }
    qax::serialize(&doc)
}

pub fn commands(records: &[Record]) -> String {
    let mut lines = Vec::new();
    for record in records {
        let node = record.node();
        lines.push(format!(
            "# {}",
            node.str_attr("label")
                .to_string()
                .replace(['\r', '\n'], " ")
        ));
        for item in qax::elements(&node) {
            if item.tag_name() == "cmd" {
                lines.push(item.contents().into());
                continue;
            }
            if item.tag_name() != "op" {
                lines.extend(item.contents().lines().map(|line| format!("# {line}")));
                continue;
            }
            let tool = item.str_attr("name").to_string();
            let command = catalog()
                .iter()
                .find(|(_, spec)| spec.tool == tool && spec.fixed.is_empty())
                .map(|(name, _)| name.as_str())
                .unwrap_or(&tool);
            let args = qax::elements(item)
                .map(|arg| {
                    let key = arg.str_attr("name").to_string();
                    let text = argument_value(arg);
                    let value = serde_json::to_string(&text).unwrap();
                    format!("{key}={value}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("{command} {args}").trim_end().into());
        }
    }
    lines.join("\n")
}

#[derive(Clone, Debug, Serialize)]
pub struct MacroInfo {
    pub name: String,
    pub label: String,
    pub description: String,
    pub target_type: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LibraryFile {
    pub key: String,
    pub name: String,
    pub source: String,
    pub bundled: bool,
    pub macros: Vec<MacroInfo>,
    pub action_count: usize,
    pub error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    path: Option<std::path::PathBuf>,
}

impl LibraryFile {
    pub fn parse(name: &str, source: String, bundled: bool) -> Self {
        let mut file = Self {
            key: name.into(),
            name: name.into(),
            source,
            bundled,
            macros: Vec::new(),
            action_count: 0,
            error: None,
            #[cfg(not(target_arch = "wasm32"))]
            path: None,
        };
        match qax::parse(&file.source) {
            Err(error) => file.error = Some(error),
            Ok(doc) => {
                for item in qax::elements(&doc) {
                    if item.tag_name() == "action" {
                        file.action_count += 1;
                        continue;
                    }
                    let params = qax::elements(item)
                        .filter(|node| node.tag_name() == "param")
                        .collect::<Vec<_>>();
                    let name = item.str_attr("name").to_string();
                    let label = item.str_attr("label").to_string();
                    file.macros.push(MacroInfo {
                        label: if label.is_empty() {
                            name.clone()
                        } else {
                            label
                        },
                        name,
                        description: item.str_attr("description").to_string(),
                        target_type: params.first().map(|node| node.str_attr("type").to_string()),
                        error: (params.len() > 1).then(|| {
                            "Only macros with zero or one object parameter can run here.".into()
                        }),
                    });
                }
            }
        }
        file
    }

    pub fn presets() -> Self {
        Self::parse("Presets", PRESETS.into(), true)
    }

    pub fn copy_macro(&self, name: &str) -> Result<String, String> {
        let doc = qax::parse(&self.source)?;
        let item = qax::elements(&doc)
            .find(|node| node.tag_name() == "macro" && node.str_attr("name").equals(name))
            .ok_or("Macro not found")?;
        let mut copy = document();
        copy.append_child_node(item.clone());
        Ok(qax::serialize(&copy))
    }
}

pub(super) fn macro_node(source: &str, name: &str) -> Result<LDom2Node, String> {
    let doc = qax::parse(source)?;
    let result = qax::elements(&doc)
        .find(|node| node.tag_name() == "macro" && node.str_attr("name").equals(name))
        .cloned()
        .ok_or_else(|| format!("Macro not found: {name}"));
    result
}

pub(super) fn bind_macro(
    state: &DesktopAppState,
    mac: &LDom2Node,
    target: Option<u64>,
) -> Result<Vec<Action>, String> {
    let params = qax::elements(mac)
        .filter(|node| node.tag_name() == "param")
        .collect::<Vec<_>>();
    if params.len() > 1 {
        return Err("Only one object parameter can be bound.".into());
    }
    let path = if let Some(param) = params.first() {
        let id = target.ok_or("Select an object for this macro.")?;
        let inventory = nodes(state)?;
        let node = resolve_node(&inventory, &format!("#{id}"), "object")?;
        let class = param.str_attr("type").to_string();
        let semantic = if matches!(class.as_str(), "Object" | "object") {
            "object".into()
        } else {
            format!("{class} object")
        };
        if !super::node_accepts(node, &semantic) {
            return Err(format!("{} is not a {class} object.", node.name));
        }
        if inventory
            .iter()
            .filter(|other| other.kind == "object" && other.path == node.path)
            .count()
            != 1
        {
            return Err(format!("{} has no unique object name.", node.name));
        }
        Some(node.path.clone())
    } else {
        None
    };
    let mut actions = Vec::new();
    for item in qax::elements(mac).filter(|node| node.tag_name() != "param") {
        match item.tag_name() {
            "note" => {}
            "cmd" => return Err("Command lines in QAX files run only from the Console.".into()),
            "op" => {
                let mut op = item.clone();
                for arg in op
                    .children_mut()
                    .iter_mut()
                    .filter(|child| !child.is_attr())
                {
                    if arg.str_attr("param").is_empty() {
                        continue;
                    }
                    let root = path.as_ref().ok_or("The macro parameter was not bound.")?;
                    let sub = arg.str_attr("sub").to_string();
                    let bound = if sub.is_empty() {
                        root.clone()
                    } else {
                        format!("{root}/{sub}")
                    };
                    let name = arg.str_attr("name").to_string();
                    *arg = LDom2Node::new();
                    arg.set_tag_name("arg");
                    arg.set_str_attr("name", name);
                    arg.set_str_attr("path", bound);
                }
                actions.push(Action::QaxOp(op));
            }
            _ => unreachable!("QAX macro was validated"),
        }
    }
    Ok(actions)
}

fn plain_path(path: &str) -> &str {
    let path = path
        .rsplit_once('#')
        .filter(|(_, tail)| tail.parse::<u64>().is_ok())
        .map_or(path, |(head, _)| head);
    path.rsplit_once('@').map_or(path, |(head, _)| head)
}

pub fn make_macro(
    records: &[Record],
    name: &str,
    label: &str,
    state: &DesktopAppState,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Enter a macro name.".into());
    }
    let mut body = Vec::new();
    let mut target: Option<String> = None;
    let mut created = BTreeSet::new();
    for record in records {
        let node = record.node();
        for item in qax::elements(&node) {
            if item.bool_attr("incomplete") {
                return Err("This action was too large to record completely.".into());
            }
            let mut op = item.clone();
            if op.tag_name() == "op" {
                let creating = ["create_renderer", "create_preset_renderer", "create_group"]
                    .contains(&op.str_attr("name").to_string().as_str());
                let new_name = qax::elements(&op)
                    .find(|arg| arg.str_attr("name").equals("name"))
                    .map(|arg| arg.contents().to_string());
                let owner = qax::elements(&op)
                    .find(|arg| arg.str_attr("name").equals("objId"))
                    .map(|arg| plain_path(&arg.str_attr("path").to_string()).to_string());
                for arg in op
                    .children_mut()
                    .iter_mut()
                    .filter(|child| !child.is_attr())
                {
                    if arg.str_attr("path").is_empty() && arg.str_attr("uid").is_empty() {
                        continue;
                    }
                    if arg.str_attr("path").is_empty() {
                        return Err("A recorded node had no portable path.".into());
                    }
                    let path = plain_path(&arg.str_attr("path").to_string()).to_string();
                    let (object, renderer) = path
                        .split_once('/')
                        .map_or((path.as_str(), None), |(obj, rend)| (obj, Some(rend)));
                    if target.as_ref().is_some_and(|prior| prior != object) {
                        return Err("These steps refer to more than one object.".into());
                    }
                    target = Some(object.into());
                    if renderer.is_some() && !created.contains(&path) {
                        return Err(format!("Include the step that creates {path}."));
                    }
                    let name = arg.str_attr("name").to_string();
                    *arg = LDom2Node::new();
                    arg.set_tag_name("arg");
                    arg.set_str_attr("name", name);
                    arg.set_str_attr("param", "target");
                    if let Some(renderer) = renderer {
                        arg.set_str_attr("sub", renderer);
                    }
                }
                if creating {
                    if let (Some(owner), Some(name)) = (owner, new_name) {
                        if !owner.is_empty() && !name.is_empty() {
                            created.insert(format!("{owner}/{name}"));
                        }
                    }
                }
            }
            body.push(op);
        }
    }
    let mut doc = document();
    let mac = doc.append_child("macro");
    mac.set_str_attr("name", name.trim());
    mac.set_str_attr("label", label.trim());
    if let Some(target) = target {
        let class = nodes(state)?
            .into_iter()
            .find(|node| node.kind == "object" && node.path == target)
            .map(|node| {
                if super::node_accepts(&node, "MolCoord object") {
                    "MolCoord".into()
                } else {
                    node.class
                }
            })
            .unwrap_or("Object".into());
        let param = mac.append_child("param");
        param.set_str_attr("name", "target");
        param.set_str_attr("type", class);
        param.set_str_attr("label", "Target");
    }
    for item in body {
        mac.append_child_node(item);
    }
    Ok(qax::serialize(&doc))
}

pub fn validate_file_name(name: &str) -> Result<String, String> {
    let name = name.trim().trim_end_matches(".qax");
    if name.is_empty()
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
    {
        return Err("Enter a file name without path separators or reserved characters.".into());
    }
    Ok(format!("{name}.qax"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_library(directory: &std::path::Path) -> Vec<LibraryFile> {
    let mut paths = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("qax"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut files = vec![LibraryFile::presets()];
    for path in paths {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut file = match std::fs::read_to_string(&path) {
            Ok(source) => LibraryFile::parse(&name, source, false),
            Err(error) => {
                let mut file = LibraryFile::parse(&name, String::new(), false);
                file.error = Some(error.to_string());
                file
            }
        };
        file.key = format!(
            "file:{}",
            path.file_name()
                .unwrap_or_default()
                .as_encoded_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        file.path = Some(path);
        files.push(file);
    }
    files
}

#[cfg(not(target_arch = "wasm32"))]
pub fn resolve_library_file(
    directory: &std::path::Path,
    key: &str,
    bundled: bool,
) -> Result<LibraryFile, String> {
    let files = read_library(directory);
    if let Some(file) = files
        .iter()
        .find(|file| file.bundled == bundled && file.key == key)
    {
        return Ok(file.clone());
    }
    let mut matching = files
        .into_iter()
        .filter(|file| file.bundled == bundled && file.name == key);
    let file = matching.next().ok_or("Action file not found.")?;
    if matching.next().is_some() {
        return Err("More than one action file has that name. Select a library entry.".into());
    }
    Ok(file)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn delete_library_file(directory: &std::path::Path, key: &str) -> Result<(), String> {
    let file = resolve_library_file(directory, key, false)?;
    std::fs::remove_file(file.path.ok_or("Presets are read-only.")?)
        .map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn rename_library_file(
    directory: &std::path::Path,
    key: &str,
    name: &str,
) -> Result<(), String> {
    let file = resolve_library_file(directory, key, false)?;
    let old = file.path.as_ref().ok_or("Presets are read-only.")?;
    let new = directory.join(validate_file_name(name)?);
    if *old == new {
        return Ok(());
    }
    save_library(directory, name, &file.source)?;
    if let Err(error) = std::fs::remove_file(old) {
        if let Err(cleanup) = std::fs::remove_file(&new) {
            return Err(format!("Could not remove the old action: {error}; could not remove its new copy: {cleanup}"));
        }
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_library(directory: &std::path::Path, name: &str, source: &str) -> Result<(), String> {
    use std::io::Write;
    let name = validate_file_name(name)?;
    qax::parse(source)?;
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
    temporary
        .write_all(source.as_bytes())
        .map_err(|error| error.to_string())?;
    temporary
        .persist_noclobber(directory.join(name))
        .map_err(|error| error.to_string())?;
    Ok(())
}
