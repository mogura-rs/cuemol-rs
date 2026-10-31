use super::{catalog, nodes, resolve_node, tool_schema, Action, Session};
use cuemol_core::{parse_ldom2_xml, LDom2Node};
use cuemol_desktop_core::DesktopAppState;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(super) struct Log {
    #[cfg(not(target_arch = "wasm32"))]
    path: PathBuf,
    document: LDom2Node,
    pending: Vec<LDom2Node>,
}

pub(super) fn elements(node: &LDom2Node) -> impl Iterator<Item = &LDom2Node> {
    node.children().iter().filter(|child| !child.is_attr())
}

fn validate_items(parent: &LDom2Node, parameters: &BTreeSet<String>) -> Result<(), String> {
    for item in elements(parent) {
        match item.tag_name() {
            "param" if parent.tag_name() == "macro" => {}
            "cmd" | "note" => {}
            "op" => {
                if item.str_attr("name").is_empty() {
                    return Err("A QAX op needs a name".into());
                }
                let mut names = BTreeSet::new();
                for arg in elements(item) {
                    if arg.tag_name() != "arg" {
                        return Err(format!("Unexpected QAX op element: {}", arg.tag_name()));
                    }
                    let name = arg.str_attr("name").to_string();
                    if name.is_empty() || !names.insert(name) {
                        return Err("QAX arguments require unique names".into());
                    }
                    let parameter = arg.str_attr("param").to_string();
                    if !parameter.is_empty() && !parameters.contains(&parameter) {
                        return Err(format!("Unknown QAX parameter: {parameter}"));
                    }
                    if !arg.str_attr("sub").is_empty() && parameter.is_empty() {
                        return Err("A QAX sub argument needs a parameter".into());
                    }
                    if elements(arg).any(|child| child.tag_name() != "atom") {
                        return Err("Only atom elements are allowed inside a QAX argument".into());
                    }
                }
            }
            other => return Err(format!("Unexpected QAX item: {other}")),
        }
    }
    Ok(())
}

pub(super) fn parse(source: &str) -> Result<LDom2Node, String> {
    let tree = parse_ldom2_xml(source).map_err(|error| error.to_string())?;
    let root = tree.top();
    if root.tag_name() != "cuemol-actions" || root.str_attr("version").to_string() != "1" {
        return Err("Expected a cuemol-actions document with version 1".into());
    }
    let mut macros = BTreeSet::new();
    for action in elements(root) {
        if !matches!(action.tag_name(), "action" | "macro") {
            return Err(format!("Unexpected QAX element: {}", action.tag_name()));
        }
        let mut parameters = BTreeSet::new();
        if action.tag_name() == "macro" {
            let name = action.str_attr("name").to_string();
            if name.is_empty() || !macros.insert(name) {
                return Err("QAX macros require unique names".into());
            }
            let mut body = false;
            for item in elements(action) {
                if item.tag_name() == "param" {
                    let name = item.str_attr("name").to_string();
                    if body
                        || name.is_empty()
                        || item.str_attr("type").is_empty()
                        || !parameters.insert(name)
                    {
                        return Err(
                            "QAX macro parameters need unique names and types before the body"
                                .into(),
                        );
                    }
                } else {
                    body = true;
                }
            }
        }
        validate_items(action, &parameters)?;
    }
    Ok(root.clone())
}

pub(super) fn actions(source: &str, separate: bool) -> Result<Vec<Action>, String> {
    let document = parse(source)?;
    let mut actions = Vec::new();
    for group in elements(&document).filter(|child| child.tag_name() == "action") {
        if separate {
            actions.push(Action::Transaction(group.str_attr("label").to_string()));
        }
        for item in elements(group) {
            match item.tag_name() {
                "cmd" => actions.push(Action::Native(item.contents().into())),
                "note" => {}
                "op" => actions.push(Action::QaxOp(item.clone())),
                _ => unreachable!("QAX items were validated"),
            }
        }
    }
    Ok(actions)
}

fn argument_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

impl Session {
    pub(super) fn qax_op(
        &self,
        state: &DesktopAppState,
        node: &LDom2Node,
    ) -> Result<Action, String> {
        let name = node.str_attr("name").to_string();
        let spec = catalog()
            .get(&name)
            .filter(|spec| spec.tool == name)
            .or_else(|| {
                catalog()
                    .values()
                    .find(|spec| spec.tool == name && spec.fixed.is_empty())
            })
            .ok_or_else(|| format!("Unknown QAX operation: {name}"))?;
        let schema = tool_schema(&name);
        let mut bound = BTreeMap::new();
        let mut atoms = BTreeMap::new();
        for arg in elements(node) {
            let key = arg.str_attr("name").to_string();
            if schema["properties"][&key].is_null() {
                return Err(format!("Unknown {name} argument: {key}"));
            }
            if !arg.str_attr("param").is_empty() {
                return Err("Macro arguments require a bound macro".into());
            }
            if elements(arg).next().is_some()
                || spec.semantics.get(&key).is_some_and(|kind| kind == "atoms")
                    && arg.contents().is_empty()
            {
                atoms.insert(key, json!(elements(arg).map(|atom| json!({"chain":atom.str_attr("chain").to_string(),"resid":atom.str_attr("resid").to_string(),"atomName":atom.str_attr("name").to_string()})).collect::<Vec<_>>()));
            } else if !arg.str_attr("path").is_empty() {
                bound.insert(key, json!(arg.str_attr("path").to_string()).to_string());
            } else if !arg.str_attr("uid").is_empty() {
                let uid = arg
                    .str_attr("uid")
                    .to_string()
                    .parse::<u64>()
                    .map_err(|_| "QAX UIDs must be nonnegative whole numbers")?;
                bound.insert(key, format!("#{uid}"));
            } else {
                let text = arg.contents();
                let kind = &schema["properties"][&key]["type"];
                let text = if kind == "string"
                    || kind
                        .as_array()
                        .is_some_and(|types| types.contains(&json!("string")))
                {
                    json!(text).to_string()
                } else {
                    text.into()
                };
                bound.insert(key, text);
            }
        }
        let mut args = self.bind_tool(state, spec, &bound)?;
        for (key, value) in atoms {
            args[key] = value;
        }
        crate::validate_args(&name, &args)?;
        if super::runtime::outside_transaction(&name, &args) {
            return Err(format!("{name} cannot run inside a QAX action"));
        }
        Ok(Action::Tool { name, args })
    }

    pub fn begin_submission(&mut self, label: &str, dialect: super::Dialect) {
        self.log_label = label.chars().take(200).collect();
        self.log_source = if dialect == super::Dialect::PyMol {
            "pym"
        } else {
            "cmd"
        }
        .into();
        self.action_record = super::actions::Record::new(&self.log_label, &self.log_source);
        self.prepared_record = None;
        if let Some(log) = &mut self.qax_log {
            log.pending.clear();
        }
    }

    pub fn begin_transaction(&mut self, label: &str) -> Result<(), String> {
        self.finish_submission()?;
        self.begin_submission(label, super::Dialect::CueMol);
        self.log_source = "qax".into();
        Ok(())
    }

    pub fn prepare_record(&mut self, state: &DesktopAppState, name: &str, args: &Value) {
        self.prepared_record = Self::record_operation(state, name, args);
    }

    pub fn record_operation(
        state: &DesktopAppState,
        name: &str,
        args: &Value,
    ) -> Option<LDom2Node> {
        let mut normalized;
        let args = if name == "set_node_prop" {
            normalized = args.clone();
            if matches!(args["nodeType"].as_str(), Some("scene" | "view")) {
                normalized["nodeId"] = Value::Null;
            }
            let group = args["nodeType"] == "rendGroup"
                || args["nodeType"] == "renderer"
                    && args["nodeId"].as_u64().is_some_and(|id| {
                        crate::nodes::renderer_properties(state, cuemol_core::Uid::new(id))
                            .is_ok_and(|props| {
                                props.get("type_name").is_some_and(|kind| kind == "*group")
                            })
                    });
            if group && args["prop"] == "name" {
                return Self::record_operation(
                    state,
                    "rename_node",
                    &json!({"nodeType":"rendGroup","nodeId":args["nodeId"],"name":args["value"]}),
                );
            }
            if group && args["prop"] == "visible" && args["cascadeGroupVisibility"] == true {
                return Self::record_operation(
                    state,
                    "set_visible",
                    &json!({"nodeType":"rendGroup","nodeId":args["nodeId"],"visible":args["value"] == "true"}),
                );
            }
            if normalized["nodeType"] == "rendGroup" {
                normalized["nodeType"] = json!("renderer");
            }
            normalized.as_object_mut()?.remove("cascadeGroupVisibility");
            &normalized
        } else if matches!(
            name,
            "define_color"
                | "define_selection"
                | "remove_color"
                | "remove_selection"
                | "remove_style"
        ) && !args["styleId"].is_null()
        {
            if args["scope"] != "scene" {
                return None;
            }
            let scene = cuemol_core::Uid::new(args["sceneId"].as_u64()?);
            let style_id = args["styleId"].as_u64()?;
            let set_name = args["styleSet"].as_str()?;
            let matching: Vec<_> = state
                .style_summaries(scene)
                .into_iter()
                .filter(|summary| {
                    summary.scene_uid == scene
                        && state
                            .style_name(summary.target)
                            .is_ok_and(|name| name == set_name)
                })
                .collect();
            if matching.len() != 1 || matching[0].target.runtime_id() != style_id {
                return None;
            }
            normalized = args.clone();
            for key in ["styleId", "sceneId"] {
                normalized.as_object_mut()?.remove(key);
            }
            &normalized
        } else {
            args
        };
        if name == "set_prop" {
            let (kind, id, prop) =
                crate::properties::resolve_property_path(state, args["path"].as_str()?).ok()?;
            return Self::record_operation(
                state,
                "set_node_prop",
                &json!({"nodeType":kind,"nodeId":id.map(|id| id.get()),"prop":prop,"value":args["value"]}),
            );
        }
        if !crate::tool_records_history(name) {
            return None;
        }
        let Some(spec) = catalog()
            .get(name)
            .filter(|spec| spec.tool == name)
            .or_else(|| {
                catalog()
                    .values()
                    .find(|spec| spec.tool == name && spec.fixed.is_empty())
            })
        else {
            return None;
        };
        let inventory = nodes(state).unwrap_or_default();
        let mut op = LDom2Node::new();
        op.set_tag_name("op");
        op.set_str_attr("name", name);
        for (key, value) in args.as_object().into_iter().flatten() {
            if value.is_null() {
                continue;
            }
            let arg = op.append_child("arg");
            arg.set_str_attr("name", key);
            let semantic = spec.semantics.get(key).map(String::as_str).unwrap_or("");
            if semantic == "node"
                || semantic == "renderer"
                || (semantic == "object" || semantic.ends_with(" object"))
            {
                if let Some(id) = value.as_u64() {
                    arg.set_str_attr("uid", id.to_string());
                    if let Ok(node) = resolve_node(&inventory, &format!("#{id}"), semantic) {
                        arg.set_str_attr(
                            "path",
                            format!("{}@{}#{}", node.path, node.class, node.id),
                        );
                    }
                    continue;
                }
            }
            if semantic == "atoms" {
                for atom in value.as_array().into_iter().flatten() {
                    let child = arg.append_child("atom");
                    for (output, input) in
                        [("chain", "chain"), ("resid", "resid"), ("name", "atomName")]
                    {
                        child.set_str_attr(output, argument_text(&atom[input]));
                    }
                }
            } else if semantic == "x, y, z" {
                arg.set_contents(
                    value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            } else {
                arg.set_contents(argument_text(value));
            }
        }
        Some(op)
    }

    pub fn complete_record(&mut self, changed: bool) {
        if let Some(op) = self.prepared_record.take().filter(|_| changed) {
            self.action_record.append(op.clone());
            if let Some(log) = &mut self.qax_log {
                log.pending.push(op);
            }
        }
    }

    pub fn finish_submission(&mut self) -> Result<(), String> {
        self.prepared_record = None;
        let Some(log) = &mut self.qax_log else {
            return Ok(());
        };
        if log.pending.is_empty() {
            return Ok(());
        }
        let action = log.document.append_child("action");
        action.set_str_attr("label", &self.log_label);
        action.set_str_attr("source", &self.log_source);
        for op in log.pending.drain(..) {
            action.append_child_node(op);
        }
        let result = write_log(log);
        if let Err(error) = result {
            self.qax_log = None;
            return Err(format!("QAX log stopped: {error}"));
        }
        Ok(())
    }

    pub(super) fn open_qax_log(
        &mut self,
        filename: &str,
        mode: &str,
    ) -> Result<Vec<Action>, String> {
        if !["w", "a"].contains(&mode) {
            return Err("Log mode must be w or a".into());
        }
        self.finish_submission()?;
        #[cfg(target_arch = "wasm32")]
        {
            let _ = filename;
            Err("QAX log files require the native application".into())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.resolve_path(filename);
            let document = if mode == "a" && path.exists() {
                parse(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)?
            } else {
                let mut root = LDom2Node::new();
                root.set_tag_name("cuemol-actions");
                root.set_str_attr("version", "1");
                root
            };
            let log = Log {
                path: path.clone(),
                document,
                pending: Vec::new(),
            };
            write_log(&log)?;
            self.qax_log = Some(log);
            Ok(vec![Action::Output(format!(
                "Logging actions to {}",
                path.display()
            ))])
        }
    }

    pub(super) fn close_qax_log(&mut self) -> Result<Vec<Action>, String> {
        self.finish_submission()?;
        self.qax_log = None;
        Ok(vec![Action::Output("QAX log closed.".into())])
    }
}

pub(super) fn serialize(root: &LDom2Node) -> String {
    fn escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\r', "&#13;")
            .replace('\n', "&#10;")
            .replace('\t', "&#9;")
    }
    fn node(output: &mut String, value: &LDom2Node, depth: usize) {
        let pad = "  ".repeat(depth);
        output.push_str(&format!("{pad}<{}", value.tag_name()));
        for attr in value.children().iter().filter(|child| child.is_attr()) {
            output.push_str(&format!(
                " {}=\"{}\"",
                attr.tag_name(),
                escape(attr.value())
            ));
        }
        let children = elements(value).collect::<Vec<_>>();
        if children.is_empty() {
            if value.contents().is_empty() {
                output.push_str("/>\n");
            } else {
                output.push_str(&format!(
                    ">{}</{}>\n",
                    escape(value.contents()),
                    value.tag_name()
                ));
            }
        } else {
            output.push_str(">\n");
            for child in children {
                node(output, child, depth + 1);
            }
            output.push_str(&format!("{pad}</{}>\n", value.tag_name()));
        }
    }
    let mut output = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n".to_string();
    node(&mut output, root, 0);
    output
}

fn write_log(log: &Log) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = log;
        Err("QAX log files require the native application".into())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Write;
        let bytes = serialize(&log.document);
        let mut temporary = tempfile::NamedTempFile::new_in(
            log.path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
        )
        .map_err(|e| e.to_string())?;
        temporary
            .write_all(bytes.as_bytes())
            .map_err(|e| e.to_string())?;
        temporary.persist(&log.path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::{Dialect, Submission};
    use cuemol_qsys::Camera;

    #[test]
    fn qax_refuses_scene_io_and_history_operations_before_execution() {
        let state = super::super::tests::state();
        let session = Session::default();
        for name in ["load_file", "save_scene", "clear_undo", "undo", "redo"] {
            let args = if matches!(name, "load_file" | "save_scene") {
                "<arg name=\"path\">scene.qsc</arg>"
            } else {
                ""
            };
            let source = format!("<cuemol-actions version=\"1\"><action><op name=\"{name}\">{args}</op></action></cuemol-actions>");
            let Action::QaxOp(op) = actions(&source, false).unwrap().remove(0) else {
                panic!("Expected a QAX operation")
            };
            assert!(session
                .qax_op(&state, &op)
                .unwrap_err()
                .contains("QAX action"));
        }
    }

    #[test]
    fn published_qax_node_paths_override_recorded_uids_and_preserve_action_boundaries() {
        let source = r#"<cuemol-actions version="1"><action label="Hide"><op name="set_visible"><arg name="nodeId" path="protein" uid="987654321"/><arg name="nodeType">object</arg><arg name="visible">false</arg></op></action><action label="Rename"><cmd>rename protein, renamed</cmd></action></cuemol-actions>"#;
        let mut state = super::super::tests::state();
        let id = state.active_object_summaries()[0].uid;
        let mut camera = Camera::new();
        let mut session = Session::default();
        let mut boundaries = Vec::new();
        for action in actions(source, true).unwrap() {
            match action {
                Action::Transaction(label) => boundaries.push(label),
                Action::QaxOp(op) => {
                    let Action::Tool { name, args } = session.qax_op(&state, &op).unwrap() else {
                        panic!("Expected an operation")
                    };
                    crate::call_tool(&mut state, &mut camera, &name, &args).unwrap();
                }
                Action::Native(text) => {
                    let mut run = Submission::new(&text, Dialect::CueMol).unwrap();
                    while let Some(action) = run.next(&mut session, &state, &camera).unwrap() {
                        if let Action::Tool { name, args } = action {
                            crate::call_tool(&mut state, &mut camera, &name, &args).unwrap();
                        }
                    }
                }
                _ => panic!("Unexpected QAX item"),
            }
        }
        assert_eq!(boundaries, ["Hide", "Rename"]);
        assert_eq!(state.scene_manager().object(id).unwrap().name(), "renamed");
        assert!(!state.scene_manager().object(id).unwrap().is_visible());
        assert!(!actions(source, false)
            .unwrap()
            .iter()
            .any(|action| matches!(action, Action::Transaction(_))));
    }

    #[test]
    fn qax_log_records_completed_mutations_and_escapes_xml_text() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        session.cwd = directory.path().to_owned();
        session.open_qax_log("actions.qax", "w").unwrap();
        let mut state = super::super::tests::state();
        let id = state.active_object_summaries()[0].uid;
        let args = json!({"nodeId":id.get(),"nodeType":"object","name":"renamed & < ]]>"});
        session.begin_submission("Rename & <", Dialect::CueMol);
        session.prepare_record(&state, "rename_node", &args);
        crate::call_tool(&mut state, &mut Camera::new(), "rename_node", &args).unwrap();
        session.complete_record(true);
        session.finish_submission().unwrap();
        session.begin_submission("Read", Dialect::CueMol);
        session.prepare_record(&state, "get_scene_state", &json!({}));
        session.complete_record(false);
        session.finish_submission().unwrap();
        session.close_qax_log().unwrap();
        let text = std::fs::read_to_string(directory.path().join("actions.qax")).unwrap();
        let document = parse(&text).unwrap();
        assert_eq!(elements(&document).count(), 1);
        let action = elements(&document).next().unwrap();
        assert_eq!(action.str_attr("label").to_string(), "Rename & <");
        let op = elements(action).next().unwrap();
        assert_eq!(
            elements(op)
                .find(|arg| arg.str_attr("name").to_string() == "name")
                .unwrap()
                .contents(),
            "renamed & < ]]>"
        );
        assert!(text.contains("&amp;") && text.contains("&lt;"));
        session.open_qax_log("actions.qax", "a").unwrap();
        session.close_qax_log().unwrap();
        assert_eq!(
            elements(
                &parse(&std::fs::read_to_string(directory.path().join("actions.qax")).unwrap())
                    .unwrap()
            )
            .count(),
            1
        );
    }
}
