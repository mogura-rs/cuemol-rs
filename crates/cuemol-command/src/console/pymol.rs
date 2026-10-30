use super::{
    nodes, parser, pymol_analysis, pymol_io, pymol_selection, pymol_settings, pymol_view, Action,
    Completion, Session,
};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Parameter {
    name: String,
    default: Option<String>,
}

#[derive(Deserialize)]
struct Command {
    params: Vec<Parameter>,
    summary: String,
}

fn catalog() -> &'static BTreeMap<String, Command> {
    static CATALOG: OnceLock<BTreeMap<String, Command>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("pymol-catalog.json")).expect("valid PyMOL catalog")
    })
}

pub(super) fn help(topic: &str) -> Result<String, String> {
    if topic.is_empty() || topic == "commands" {
        return Ok(format!("PyMOL commands\n{}\n\nUse help <command> for arguments. Named selections store expressions and are evaluated for each molecule.", catalog().iter().map(|(name, command)| format!("{name:<18} {}", command.summary)).collect::<Vec<_>>().join("\n")));
    }
    let name = parser::lookup(topic, catalog().keys().map(String::as_str))?;
    let command = &catalog()[name];
    let params = command
        .params
        .iter()
        .map(|param| {
            param.default.as_ref().map_or_else(
                || param.name.clone(),
                |value| format!("[{}={value}]", param.name),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("{name} {params}\n{}", command.summary))
}

pub(super) fn tool(name: &str, args: Value) -> Result<Action, String> {
    crate::validate_args(name, &args)?;
    Ok(Action::Tool {
        name: name.into(),
        args,
    })
}

pub(super) fn number(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("Expected a finite number, not {value}"))
}

pub(super) fn boolean(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" => Ok(true),
        "0" | "false" | "off" | "no" => Ok(false),
        _ => Err(format!("Expected a boolean, not {value}")),
    }
}

pub(super) fn all(value: &str) -> bool {
    matches!(
        value.trim().trim_matches(['(', ')']).trim(),
        "" | "*" | "all"
    )
}

fn glob(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let mut prior = vec![false; pattern.len() + 1];
    prior[0] = true;
    for i in 0..pattern.len() {
        prior[i + 1] = prior[i] && pattern[i] == '*';
    }
    for ch in value.chars() {
        let mut next = vec![false; prior.len()];
        for i in 0..pattern.len() {
            next[i + 1] = if pattern[i] == '*' {
                next[i] || prior[i + 1]
            } else {
                prior[i] && (pattern[i] == '?' || pattern[i] == ch)
            };
        }
        prior = next;
    }
    *prior.last().unwrap()
}

pub(super) fn matching_nodes(
    state: &DesktopAppState,
    pattern: &str,
) -> Result<Vec<super::Node>, String> {
    let inventory = nodes(state)?;
    let exact = inventory
        .iter()
        .filter(|node| !all(pattern) && (node.name == pattern || node.path == pattern))
        .cloned()
        .collect::<Vec<_>>();
    let matches = if exact.is_empty() {
        inventory
            .into_iter()
            .filter(|node| {
                if all(pattern) {
                    node.kind == "object"
                } else {
                    glob(pattern, &node.name) || glob(pattern, &node.path)
                }
            })
            .collect()
    } else {
        exact
    };
    let objects = matches
        .iter()
        .filter(|node| node.kind == "object")
        .cloned()
        .collect::<Vec<_>>();
    Ok(if objects.is_empty() { matches } else { objects })
}

pub(super) struct MoleculeSelection {
    pub id: Uid,
    pub name: String,
    pub expression: String,
}

pub(super) fn named_selections(state: &DesktopAppState) -> Vec<(String, String, String)> {
    let Some(scene) = state
        .active_scene()
        .and_then(|id| state.scene_manager().scene(id))
    else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for style in scene.loaded_styles() {
        if style.node.str_attr("readonly").equals_ignore_case("true") {
            continue;
        }
        let tree = if style.node.children().iter().any(|node| !node.is_attr()) {
            Some(cuemol_core::LDom2Tree::from(style.node.clone()))
        } else {
            cuemol_core::parse_ldom2_xml(&style.data).ok()
        };
        if let Some(tree) = tree {
            for node in tree
                .top()
                .children()
                .iter()
                .filter(|node| node.tag_name() == "sel" && !node.is_attr())
            {
                result.push((
                    style.node.str_attr("id").to_string(),
                    node.str_attr("id").to_string(),
                    node.value().to_string(),
                ));
            }
        }
    }
    result
}

pub(super) fn selections(
    state: &DesktopAppState,
    source: &str,
) -> Result<(Vec<MoleculeSelection>, bool), String> {
    let molecules = nodes(state)?
        .into_iter()
        .filter(|node| {
            node.kind == "object" && (node.class == "MolCoord" || node.class.ends_with("Mol"))
        })
        .collect::<Vec<_>>();
    if molecules.is_empty() {
        return Err("No molecule in the active scene".into());
    }
    let parsed = pymol_selection::parse(if all(source) { "all" } else { source })?;
    let mut names = Vec::new();
    parsed.names(&mut names);
    let mentioned = molecules
        .iter()
        .filter(|node| names.contains(&node.name))
        .map(|node| node.name.clone())
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    for node in &molecules {
        if !mentioned.is_empty() && !mentioned.contains(&node.name) {
            continue;
        }
        let expression = parsed.emit(&|name| {
            molecules
                .iter()
                .any(|molecule| molecule.name == name)
                .then(|| if node.name == name { "*" } else { "none" }.into())
        });
        state
            .parse_scene_selector(state.active_scene().ok_or("No active scene")?, &expression)
            .map_err(|error| error.to_string())?;
        result.push(MoleculeSelection {
            id: Uid::new(node.id),
            name: node.name.clone(),
            expression,
        });
    }
    Ok((result, !mentioned.is_empty()))
}

pub(super) fn native_property(node_path: &str, prop: &str, value: &str) -> Action {
    Action::Native(format!(
        "set {}, {}",
        json!(format!("{node_path}.{prop}")),
        json!(value)
    ))
}

const REPRESENTATIONS: &[(&str, &str)] = &[
    ("lines", "simple"),
    ("sticks", "ballstick"),
    ("spheres", "cpk"),
    ("cartoon", "cartoon"),
    ("ribbon", "ribbon"),
    ("surface", "dsurface"),
    ("nonbonded", "simple"),
    ("nb_spheres", "cpk"),
];

fn representation_actions(
    session: &Session,
    state: &DesktopAppState,
    operation: &str,
    rep: &str,
    source: &str,
) -> Result<Vec<Action>, String> {
    let mut rep = rep;
    let mut source = source;
    if source.is_empty() && (all(rep) || rep.contains(['(', '/'])) {
        source = rep;
        rep = if operation == "hide" {
            "everything"
        } else {
            "wire"
        };
    }
    let accepted = REPRESENTATIONS.iter().map(|pair| pair.0).chain([
        "everything",
        "wire",
        "licorice",
        "labels",
        "mesh",
        "dots",
        "volume",
        "slice",
        "cell",
        "ellipsoids",
    ]);
    let mut reps = Vec::new();
    for word in rep.split_whitespace() {
        let resolved = parser::lookup(word, accepted.clone())?;
        match resolved {
            "everything" => {
                reps.extend(REPRESENTATIONS.iter().map(|pair| pair.0));
                reps.push("labels");
            }
            "wire" => reps.push("lines"),
            "licorice" => reps.push("sticks"),
            "labels" => reps.push("labels"),
            _ if REPRESENTATIONS.iter().any(|pair| pair.0 == resolved) => reps.push(resolved),
            _ => {
                return Err(format!(
                "{resolved} is not a molecular representation supported by the reference console"
            ))
            }
        }
    }
    if reps.is_empty() {
        return Err("A representation is required".into());
    }
    reps.sort_unstable();
    reps.dedup();
    let (items, _) = selections(state, source)?;
    let mut actions = Vec::new();
    for item in items {
        let renderers = state.renderer_summaries(item.id);
        if operation == "as" {
            for renderer in &renderers {
                if let Some(rep) = renderer.name.strip_prefix("pym:") {
                    if rep != "labels" && !reps.contains(&rep) {
                        actions.push(tool(
                            "set_renderer_selection",
                            json!({"rendId":renderer.uid.get(),"selection":"none"}),
                        )?);
                        actions.push(tool("set_visible", json!({"nodeId":renderer.uid.get(),"nodeType":"renderer","visible":false}))?);
                    }
                }
            }
        }
        for rep in &reps {
            let label = format!("pym:{rep}");
            let existing = renderers.iter().find(|renderer| renderer.name == label);
            if *rep == "labels" {
                if operation == "hide" && !all(source) {
                    actions.push(tool("set_atom_labels", json!({"objId":item.id.get(),"selection":item.expression,"format":null,"rendererName":"pym:labels"}))?);
                } else if let Some(renderer) = existing {
                    actions.push(tool("set_visible", json!({"nodeId":renderer.uid.get(),"nodeType":"renderer","visible":operation != "hide"}))?);
                }
                continue;
            }
            if let Some(renderer) = existing {
                let current = state
                    .renderer_property(item.id, renderer.uid, "sel")
                    .unwrap_or_default();
                let current = if current.trim().is_empty() {
                    "*"
                } else {
                    &current
                };
                let next = match operation {
                    "hide" if all(source) => "none".to_string(),
                    "hide" => format!("({current}) and not ({})", item.expression),
                    "as" => item.expression.clone(),
                    _ => format!("({current}) or ({})", item.expression),
                };
                actions.push(tool(
                    "set_renderer_selection",
                    json!({"rendId":renderer.uid.get(),"selection":next}),
                )?);
                actions.push(tool("set_visible", json!({"nodeId":renderer.uid.get(),"nodeType":"renderer","visible":!(operation == "hide" && all(source))}))?);
            } else if operation != "hide" {
                let kind = REPRESENTATIONS
                    .iter()
                    .find(|pair| pair.0 == *rep)
                    .unwrap()
                    .1;
                actions.push(tool("create_renderer", json!({"objId":item.id.get(),"rendererType":kind,"name":label,"selection":item.expression,"centerView":false}))?);
                let path = format!("{}/{}", item.name, label);
                if *rep == "sticks" {
                    actions.push(Action::Native(format!(
                        "set_renderer_styles {}, {}",
                        json!(path),
                        json!("StickBallStick,DefaultCPKColoring")
                    )));
                }
                actions.extend(pymol_settings::remembered(session, state, rep, &path));
            }
        }
    }
    Ok(actions)
}

pub(super) fn plan(
    session: &mut Session,
    state: &DesktopAppState,
    camera: &Camera,
    line: &str,
) -> Result<Vec<Action>, String> {
    if let Some(source) = line.trim().strip_prefix('@') {
        return session.read_script(source);
    }
    let (word, rest) = parser::head(line);
    let name = parser::lookup(word, catalog().keys().map(String::as_str))?;
    if rest == "?" {
        return Ok(vec![Action::Output(help(name)?)]);
    }
    let command = &catalog()[name];
    let params = command
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    let legacy = matches!(
        name,
        "select"
            | "color"
            | "set"
            | "isomesh"
            | "isosurface"
            | "isolevel"
            | "distance"
            | "angle"
            | "dihedral"
    );
    let mut args = parser::bind_pymol_arguments(&params, rest, legacy)?;
    for parameter in &command.params {
        if args
            .get(&parameter.name)
            .is_none_or(|value| value.is_empty())
        {
            if let Some(value) = &parameter.default {
                args.insert(parameter.name.clone(), value.clone());
            } else {
                return Err(format!("Missing argument: {}", parameter.name));
            }
        }
    }
    let arg = |key: &str| parser::unquote(args.get(key).map(String::as_str).unwrap_or(""));
    let mut warnings = argument_warnings(name, command, &args);
    let actions = (|| -> Result<Vec<Action>, String> {
        let mut actions = Vec::new();
        match name {
        "help" => actions.push(Action::Output(help(arg("command"))?)),
        "distance" | "angle" | "dihedral" => return pymol_analysis::measure(state, name, &arg),
        "align" | "super" | "pair_fit" => return pymol_analysis::fit(state, name, &arg),
        "label" => return pymol_analysis::label(session, state, arg("selection"), args.get("expression").map(String::as_str).unwrap_or("")),
        "isomesh" | "isosurface" | "isolevel" => return pymol_analysis::map(state, name, &arg),
        "spectrum" => return pymol_analysis::spectrum(state, &arg),
        "save" => return pymol_io::save(session, state, camera, &arg),
        "png" | "ray" => return pymol_io::image(session, state, camera, name, &arg),
        "log_open" => return session.open_log(arg("filename"), arg("mode")),
        "log_close" => return Ok(session.close_log()),
        "log" => session.log_line(arg("text"))?,
        "quit" | "exit" => actions.push(Action::Quit),
        "pwd" => actions.push(Action::Output(session.cwd().display().to_string())),
        "cd" | "ls" => return session.directory_command(name, arg(if name == "cd" { "dir" } else { "pattern" })),
        "run" => return session.read_script(arg("filename")),
        "undo" | "redo" => actions.push(tool(name, json!({}))?),
        "refresh" => actions.push(tool("get_view", json!({}))?),
        "select" | "indicate" | "deselect" => {
            let source = if name == "select" && arg("selection").is_empty() { arg("name") } else { arg("selection") };
            let (items, named) = selections(state, if name == "deselect" { "none" } else { source })?;
            let molecules = nodes(state)?.into_iter().filter(|node| node.kind == "object" && (node.class == "MolCoord" || node.class.ends_with("Mol")));
            for node in molecules {
                let expression = items.iter().find(|item| item.id.get() == node.id).map_or("", |item| item.expression.as_str());
                actions.push(tool("set_mol_selection", json!({"objId":node.id,"selection":expression}))?);
            }
            if name == "select" {
                let label = if arg("selection").is_empty() { "sele" } else { arg("name") };
                if named { actions.push(Action::Output(format!("{label} is shown but not stored: the expression names objects."))); }
                else { actions.push(tool("define_selection", json!({"name":label,"selection":items[0].expression}))?); }
            }
        }
        "count_atoms" => {
            let (items, _) = selections(state, arg("selection"))?;
            let mut total = 0;
            for item in &items {
                let selector = state.parse_scene_selector(state.active_scene().unwrap(), &item.expression).map_err(|error| error.to_string())?;
                let count = state.scene_manager().object(item.id).unwrap().mol().select_indices(&selector).len();
                total += count;
                if items.len() > 1 { actions.push(Action::Output(format!("{}: {count}", item.name))); }
            }
            actions.push(Action::Output(format!("count_atoms: {total} atoms")));
        }
        "enable" | "disable" | "delete" | "set_name" => {
            let pattern = arg(if name == "set_name" { "old_name" } else { "name" });
            let matches = matching_nodes(state, pattern)?;
            if matches.is_empty() || name == "delete" && all(pattern) {
                let stored = named_selections(state);
                let exact = stored.iter().any(|entry| entry.1 == pattern);
                for (set, label, expression) in stored.into_iter().filter(|entry| if exact { entry.1 == pattern } else { all(pattern) || glob(pattern, &entry.1) }) {
                    match name {
                        "delete" => actions.push(tool("remove_selection", json!({"styleSet":set,"name":label}))?),
                        "set_name" => {
                            actions.push(tool("define_selection", json!({"name":arg("new_name"),"selection":expression}))?);
                            actions.push(tool("remove_selection", json!({"styleSet":set,"name":label}))?);
                        }
                        _ => for object in state.active_object_summaries() {
                            actions.push(tool("set_mol_selection", json!({"objId":object.uid.get(),"selection":if name == "enable" { &expression } else { "" }}))?);
                        },
                    }
                }
                if matches.is_empty() {
                    if !actions.is_empty() || name == "delete" && all(pattern) { return Ok(actions); }
                    return Err(format!("No object, renderer or named selection: {pattern}"));
                }
            }
            if name == "set_name" && matches.len() != 1 { return Err("set_name requires one object or renderer".into()); }
            for node in matches {
                let action = match name {
                    "delete" => tool("delete_node", json!({"nodeId":node.id,"nodeType":node.kind})),
                    "set_name" => tool("rename_node", json!({"nodeId":node.id,"nodeType":node.kind,"name":arg("new_name")})),
                    _ => tool("set_visible", json!({"nodeId":node.id,"nodeType":node.kind,"visible":name == "enable"})),
                }?;
                actions.push(action);
            }
        }
        "get_names" => {
            let objects = ["objects", "public_objects", "nongroup_objects", "public_nongroup_objects", "all", "public"].contains(&arg("type"));
            let selections = ["selections", "public_selections", "all", "public"].contains(&arg("type"));
            if !objects && !selections { return Err(format!("Unknown name-list type: {}", arg("type"))); }
            let enabled = boolean(arg("enabled_only"))?;
            let mut names = if objects { state.active_object_summaries().iter().filter(|object| !enabled || object.visible).map(|object| object.name.clone()).collect::<Vec<_>>() } else { Vec::new() };
            if selections { names.extend(named_selections(state).into_iter().map(|entry| entry.1)); }
            actions.push(Action::Output(json!(names).to_string()));
        }
        "get_chains" => {
            let (items, _) = selections(state, arg("selection"))?;
            if items.len() != 1 { return Err("get_chains requires one molecule".into()); }
            actions.push(tool("list_chains", json!({"objId":items[0].id.get()}))?);
        }
        "show" | "hide" | "as" | "show_as" => return representation_actions(session, state, if name == "show_as" { "as" } else { name }, arg("representation"), arg("selection")),
        "color" => {
            let color = pymol_settings::color(state, arg("color"))?;
            let (items, _) = selections(state, arg("selection"))?;
            for item in items {
                for renderer in state.renderer_summaries(item.id).iter().filter(|renderer| renderer.name.starts_with("pym:")) {
                    if state.renderer_coloring_type(item.id, renderer.uid).as_deref() != Some("PaintColoring") {
                        actions.push(tool("set_renderer_coloring", json!({"nodeId":renderer.uid.get(),"nodeType":"renderer","mode":"paint-type-paint"}))?);
                    }
                    actions.push(tool("add_paint_at", json!({"nodeId":renderer.uid.get(),"nodeType":"renderer","selection":item.expression,"color":color,"at":1}))?);
                }
            }
            if actions.is_empty() { return Err("Show a representation before coloring it".into()); }
        }
        "bg_color" => actions.push(tool("set_node_prop", json!({"nodeType":"scene","prop":"bgcolor","value":pymol_settings::color(state, arg("color"))?}))?),
        "set_color" => {
            let name = arg("name");
            if !name.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_') || !name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') { return Err("Color names accept letters, digits and underscores".into()); }
            let color = pymol_settings::rgb(arg("rgb"))?;
            if pymol_settings::colors().contains_key(&name.to_ascii_lowercase()) { actions.push(Action::Output(format!("{name} is a built-in PyMOL color; its built-in value remains in use."))); }
            actions.push(tool("define_color", json!({"name":name,"color":color,"styleSet":"pym"}))?);
        }
        "set" | "get" | "unset" => return pymol_settings::plan(session, state, camera, name, arg("name"), arg("value"), arg("selection")),
        "zoom" | "center" | "reset" => {
            let (items, _) = selections(state, arg(if name == "reset" { "object" } else { "selection" }))?;
            if items.len() > 1 { actions.push(Action::Output(format!("Using {} only; name a molecule to choose another.", items[0].name))); }
            if name == "reset" { actions.push(tool("set_node_prop", json!({"nodeType":"view","prop":"rotation","value":cuemol_core::Quat::IDENTITY.to_string()}))?); }
            actions.push(tool("center_view", json!({"objId":items[0].id.get(),"selection":items[0].expression,"zoom":name != "center"}))?);
        }
        "orient" => {
            let (items, _) = selections(state, arg("selection"))?;
            let item = &items[0];
            if items.len() > 1 { actions.push(Action::Output(format!("Orienting {} only; name a molecule to choose another.", item.name))); }
            let selector = state.parse_scene_selector(state.active_scene().unwrap(), &item.expression).map_err(|error| error.to_string())?;
            let object = state.scene_manager().object(item.id).unwrap();
            let mut view = cuemol_qsys::View::new(Uid::new(1), state.active_scene().unwrap());
            view.set_camera(camera);
            view.set_view_size(state.runtime_value("viewport:width").parse().unwrap_or(800), state.runtime_value("viewport:height").parse().unwrap_or(600));
            if !object.mol().orient_view_with_xform(&mut view, Some(&selector), object.xform_matrix()) { return Err("The orientation selection matched no atoms".into()); }
            actions.extend(pymol_view::actions(&view.camera())?);
        }
        "get_view" => actions.push(Action::Output(pymol_view::format(camera))),
        "set_view" => actions.extend(pymol_view::actions(&pymol_view::parse(arg("view"), camera)?)?),
        "turn" => actions.push(tool("rotate_view", json!({"axis":arg("axis").to_ascii_lowercase(),"angle":number(arg("angle"))?}))?),
        "move" => {
            let axis = arg("axis").to_ascii_lowercase();
            if !matches!(axis.as_str(), "x" | "y" | "z") { return Err("Axis must be x, y or z".into()); }
            let distance = number(arg("distance"))?;
            actions.push(tool("pan_view", json!({"dx":if axis == "x" {distance} else {0.},"dy":if axis == "y" {distance} else {0.},"dz":if axis == "z" {distance} else {0.}}))?);
        }
        "view" => {
            let action = parser::lookup(arg("action"), ["store", "recall", "clear", "update"])?;
            if arg("key") == "*" {
                if action == "clear" {
                    for camera in state.active_camera_summaries() { actions.push(tool("delete_camera", json!({"name":camera.name}))?); }
                } else { actions.push(tool("list_cameras", json!({}))?); }
            } else {
                actions.push(tool(match action { "store" | "update" => "save_camera", "clear" => "delete_camera", _ => "apply_camera" }, json!({"name":arg("key")}))?);
            }
        }
        "load" | "fetch" => return pymol_io::load(session, name, &arg),
        "mplay" | "mstop" => actions.push(tool("animate", json!({"action":if name == "mplay" { "play" } else { "pause" }}))?),
        "frame" | "rewind" | "count_states" => {
            let objects = matching_nodes(state, if name == "count_states" { arg("selection") } else { "all" })?;
            let framed = objects.iter().filter(|node| node.kind == "object").map(|node| (node.id, state.trajectory_len(Uid::new(node.id)).max(state.morph_frame_summaries(Uid::new(node.id)).len()).max(session.trajectory_frames.get(&node.id).copied().unwrap_or(0)))).filter(|(_, frames)| *frames > 0).collect::<Vec<_>>();
            if name == "count_states" {
                if objects.is_empty() { return Err("No matching object".into()); }
                actions.push(Action::Output(format!("count_states: {} states", framed.iter().map(|(_,frames)| *frames).max().unwrap_or(1))));
            } else {
                let requested = if name == "rewind" { 0 } else { arg("frame").parse::<usize>().ok().and_then(|number| number.checked_sub(1)).ok_or("Frame must be a whole number from one")? };
                if name == "frame" && framed.is_empty() { return Err("No object has multiple states".into()); }
                for (id, frames) in framed {
                    let frame = requested.min(frames - 1);
                    if frame != requested { actions.push(Action::Output(format!("Object #{id} has {frames} states; showing {}",frame+1))); }
                    actions.push(tool("set_frame", json!({"objId":id,"frame":frame}))?);
                }
                if name == "rewind" { actions.push(tool("animate", json!({"action":"seek","timeMs":0.}))?); }
            }
        }
        _ => return Err(format!("PyMOL {name} is not implemented yet")),
    }
        Ok(actions)
    })()?;
    warnings.extend(actions);
    Ok(warnings)
}

fn argument_warnings(
    name: &str,
    command: &Command,
    args: &BTreeMap<String, String>,
) -> Vec<Action> {
    let used = match name {
        "load" => "filename object format zoom",
        "fetch" => "code name type zoom",
        "save" => "filename selection format",
        "png" => "filename width height dpi ray",
        "ray" => "width height",
        "select" => "name selection",
        "count_atoms" | "get_chains" | "orient" | "indicate" => "selection",
        "enable" | "disable" | "delete" => "name",
        "get_names" => "type enabled_only",
        "show" | "hide" | "as" | "show_as" => "representation selection",
        "color" => "color selection",
        "set_color" => "name rgb",
        "set" => "name value selection",
        "get" | "unset" => "name selection",
        "zoom" | "center" => "selection",
        "view" => "key action",
        "get_view" => "",
        "set_view" => "view",
        "frame" => "frame",
        "count_states" => "selection",
        "align" | "super" => "mobile target transform",
        "pair_fit" => "mobile target",
        "distance" => "name selection1 selection2 cutoff mode",
        "angle" => "name selection1 selection2 selection3",
        "dihedral" => "name selection1 selection2 selection3 selection4",
        "label" => "selection expression",
        "isomesh" | "isosurface" => "name map level selection buffer carve",
        "isolevel" => "name level query",
        "spectrum" => "expression palette selection minimum maximum",
        "log" => "text",
        "run" => "filename",
        _ => return Vec::new(),
    };
    command
        .params
        .iter()
        .filter_map(|parameter| {
            if parameter.name == "quiet" || used.split_whitespace().any(|key| key == parameter.name)
            {
                return None;
            }
            let default = parameter.default.as_deref()?;
            let value = parser::unquote(args.get(&parameter.name)?);
            if value == default
                || number(value)
                    .ok()
                    .zip(number(default).ok())
                    .is_some_and(|(a, b)| a == b)
            {
                return None;
            }
            Some(Action::Output(format!(
                "{name}: {} is ignored because the reference console does not support it.",
                parameter.name
            )))
        })
        .collect()
}

pub(super) fn complete(
    session: &Session,
    state: &DesktopAppState,
    source: &str,
) -> Vec<Completion> {
    use super::completion::Context;
    let initial = Context::new(source, &[]);
    let (word, rest) = (initial.command, initial.rest);
    if !source.ends_with(char::is_whitespace) && rest.is_empty() {
        let prefix = &source[..source.len() - word.len()];
        return catalog()
            .keys()
            .filter(|name| parser::shortcut_matches(word, name))
            .map(|name| Completion {
                replacement: format!("{prefix}{name} "),
                display: name.clone(),
            })
            .collect();
    }
    let Ok(name) = parser::lookup(word, catalog().keys().map(String::as_str)) else {
        return initial.files(session, false, false);
    };
    let params = catalog()[name]
        .params
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>();
    let context = Context::new(source, &params);
    let key = context.key.as_str();
    let mut results = context.parameters(&params);
    if matches!(key, "filename" | "file" | "path" | "directory")
        || matches!(name, "cd" | "ls" | "run" | "log_open")
    {
        results.extend(context.files(session, name == "cd", false));
        return results;
    }
    let mut candidates = Vec::new();
    if name == "help" {
        candidates.extend(catalog().keys().cloned());
    } else if key == "representation" {
        candidates.extend(REPRESENTATIONS.iter().map(|p| p.0.into()));
    } else if key == "color" || name == "bg_color" {
        candidates.extend(pymol_settings::colors().keys().cloned());
        candidates.extend(state.active_style_names("color"));
    } else if matches!(name, "set" | "get" | "unset") && key == "name" {
        candidates.extend(
            [
                "orthoscopic",
                "bg_rgb",
                "stick_radius",
                "stick_transparency",
                "line_width",
                "sphere_transparency",
                "cartoon_transparency",
                "transparency",
                "solvent_radius",
                "label_size",
                "label_color",
            ]
            .into_iter()
            .map(str::to_string),
        );
        if let Ok(values) =
            crate::properties::node_property_values(state, &Camera::new(), "scene", None)
        {
            candidates.extend(
                values
                    .into_keys()
                    .filter(|key| !crate::properties::property_readonly(state, "scene", None, key)),
            );
        }
    } else if name == "view" && key == "key" {
        candidates.extend(
            state
                .active_camera_summaries()
                .into_iter()
                .map(|camera| camera.name),
        );
    } else if name == "view" && key == "action" {
        candidates.extend(["store", "recall", "clear"].into_iter().map(str::to_string));
    } else if matches!(name, "set" | "get" | "unset") && key == "value" {
        let property = context
            .bound
            .get("name")
            .map(|s| parser::unquote(s))
            .unwrap_or("");
        if property == "orthoscopic" {
            candidates.extend(["on".into(), "off".into()]);
        } else if let Ok(values) =
            crate::properties::node_property_values(state, &Camera::new(), "scene", None)
        {
            if values
                .get(property)
                .is_some_and(|value| value == "true" || value == "false")
            {
                candidates.extend(["on".into(), "off".into()]);
            }
            candidates.extend(
                crate::properties::property_allowed(property)
                    .iter()
                    .map(|s| s.to_string()),
            );
        }
    } else if key == "selection"
        || key.starts_with("selection")
        || matches!(key, "mobile" | "target" | "name" | "map")
    {
        candidates.extend(
            nodes(state)
                .unwrap_or_default()
                .into_iter()
                .filter(|node| {
                    (node.kind == "object" || name == "delete" || name == "isolevel")
                        && !node.name.is_empty()
                })
                .map(|node| node.name),
        );
        candidates.extend(state.active_style_names("sel"));
        if key.contains("selection") || matches!(key, "mobile" | "target") {
            candidates.extend(
                [
                    "all",
                    "none",
                    "polymer",
                    "protein",
                    "nucleic",
                    "solvent",
                    "organic",
                    "inorganic",
                    "hydro",
                    "hetatm",
                    "backbone",
                    "sidechain",
                    "chain",
                    "resi",
                    "resn",
                    "name",
                    "elem",
                    "and",
                    "or",
                    "not",
                    "byres",
                ]
                .into_iter()
                .map(str::to_string),
            );
        }
    }
    let open = key.contains("selection") || matches!(key, "mobile" | "target");
    let suffix = if open
        || params
            .iter()
            .all(|p| p == key || context.bound.contains_key(p))
    {
        ""
    } else {
        ", "
    };
    let mut offers = context.offers(candidates.clone(), suffix, false);
    if offers.is_empty() && open {
        offers = context.offers(candidates, suffix, true);
    }
    if offers.is_empty() && candidates_for_file_fallback(name, key) {
        results.extend(context.files(session, false, false));
    }
    results.extend(offers);
    results
}

fn candidates_for_file_fallback(command: &str, key: &str) -> bool {
    matches!(key, "format" | "value") && command != "get_view"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::{Dialect, Submission};

    fn run(
        state: &mut DesktopAppState,
        camera: &mut Camera,
        session: &mut Session,
        source: &str,
    ) -> Result<Vec<String>, String> {
        let mut run = Submission::new(source, Dialect::PyMol)?;
        let mut output = Vec::new();
        while let Some(action) = run.next(session, state, camera)? {
            match action {
                Action::Tool { name, args } => {
                    crate::call_tool(state, camera, &name, &args)
                        .map_err(|error| format!("{name} {args}: {error}"))?;
                }
                Action::Output(text) => output.push(text),
                Action::Echo(_) => {}
                other => panic!("Unexpected action {other:?}"),
            }
        }
        Ok(output)
    }

    #[test]
    fn pymol_representation_ranges_and_named_selections_use_current_molecules() {
        let mut state = super::super::tests::state();
        let id = state.active_object_summaries()[0].uid;
        let mut camera = Camera::new();
        let mut session = Session::default();
        run(&mut state, &mut camera, &mut session, "show sticks, resi 1; show sticks, resi 2; hide sticks, resi 1; select chosen, resi 2; bg_color white; turn y, 30; view saved, store").unwrap();
        let renderer = state
            .renderer_summaries(id)
            .into_iter()
            .find(|renderer| renderer.name == "pym:sticks")
            .unwrap();
        let expression = state.renderer_property(id, renderer.uid, "sel").unwrap();
        let selector = state
            .parse_scene_selector(state.active_scene().unwrap(), &expression)
            .unwrap();
        let mol = state.scene_manager().object(id).unwrap().mol();
        assert_eq!(mol.select_indices(&selector), [1]);
        assert_eq!(
            run(&mut state, &mut camera, &mut session, "count_atoms chosen").unwrap(),
            ["count_atoms: 1 atoms"]
        );
        assert_eq!(state.active_camera_summaries()[0].name, "saved");
        run(&mut state, &mut camera, &mut session, "hide sticks; show sticks, resi 1; as cartoon, protein; disable protein; enable protein; set_name protein, changed").unwrap();
        assert!(state.active_object_summaries()[0].visible);
        assert_eq!(state.active_object_summaries()[0].name, "changed");
        assert!(
            !state
                .renderer_summaries(id)
                .iter()
                .find(|renderer| renderer.name == "pym:sticks")
                .unwrap()
                .visible
        );
        assert!(state
            .renderer_summaries(id)
            .iter()
            .any(|renderer| renderer.name == "pym:cartoon"));
    }

    #[test]
    fn pymol_object_names_limit_selection_and_globs_resolve_visible_nodes() {
        let mut state = super::super::tests::state();
        let first_id = state.active_object_summaries()[0].uid;
        state
            .scene_manager_mut()
            .object_mut(first_id)
            .unwrap()
            .set_name("protein1");
        state.open_bytes("other.pdb", b"ATOM      1  CA  GLY A   1       8.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
        let mut camera = Camera::new();
        let mut session = Session::default();
        let output = run(
            &mut state,
            &mut camera,
            &mut session,
            "select shown, protein1 and resi 2; disable other*; count_atoms protein1 or other.pdb",
        )
        .unwrap();
        let objects = state.active_object_summaries();
        assert!(state
            .molecule_selection_text(objects[1].uid)
            .unwrap()
            .is_empty());
        assert!(!objects[1].visible);
        assert_eq!(output.last().unwrap(), "count_atoms: 3 atoms");
        assert!(glob("other?*", "other.pdb"));
        assert!(!glob("other?", "other.pdb"));
    }

    #[test]
    fn pymol_settings_apply_to_later_representations_and_colors_keep_paint_order() {
        let mut state = super::super::tests::state();
        let id = state.active_object_summaries()[0].uid;
        let mut camera = Camera::new();
        let mut session = Session::default();
        run(&mut state, &mut camera, &mut session, "set stick_radius, 0.45; show sticks; select selected, resi 2; color aquamarine; color tv_red, resi 1; set_color custom, [255,128,0]; color custom, resi 2; set orthoscopic, on").unwrap();
        let renderer = state
            .renderer_summaries(id)
            .into_iter()
            .find(|renderer| renderer.name == "pym:sticks")
            .unwrap();
        let props = crate::nodes::renderer_properties(&state, renderer.uid).unwrap();
        assert_eq!(props["bondw"], "0.45");
        assert_eq!(props["sphr"], "0.45");
        let (_, _, paint) = state.renderer_coloring_state(id, renderer.uid).unwrap();
        assert_eq!(
            paint
                .iter()
                .take(3)
                .map(|entry| entry.1.as_str())
                .collect::<Vec<_>>(),
            ["#ff8000", "#ff3333", "#80ffff"]
        );
        assert_eq!(paint.len(), 7);
        assert_eq!(state.molecule_selection_text(id).unwrap(), "resi 2");
        assert!(!camera.is_perspective());
        assert_eq!(
            run(&mut state, &mut camera, &mut session, "get stick_radius").unwrap(),
            ["get: stick_radius = 0.45"]
        );
        run(
            &mut state,
            &mut camera,
            &mut session,
            "unset stick_radius; set stick_transparency, 0.25; get stick_transparency",
        )
        .unwrap();
        assert_eq!(
            state.renderer_property(id, renderer.uid, "alpha").unwrap(),
            "0.75"
        );
    }

    #[test]
    fn pymol_measurements_labels_and_spectrum_keep_live_scene_properties() {
        let mut state = super::super::tests::state();
        let id = state.active_object_summaries()[0].uid;
        let mut camera = Camera::new();
        let mut session = Session::default();
        let warnings = run(&mut state, &mut camera, &mut session,
            "select chosen=resi 1; show sticks; label name CA, \"%s%s: %.2f\" % (resn, resi, b); distance separation, resi 1, resi 2; spectrum b, blue_red, all, 0, 40; select chosen, resi 2, enable=0").unwrap();
        assert!(warnings
            .iter()
            .any(|line| line.contains("enable is ignored")));
        let object = state.scene_manager().object(id).unwrap();
        let labels = object
            .renderers()
            .iter()
            .find(|renderer| renderer.name() == "pym:labels")
            .unwrap();
        let entries = object.name_label_entries(labels);
        assert_eq!(entries[0].1, "ALA1: 10.00");
        assert_eq!(entries[1].1, "GLY2: 10.00");
        let measurement = object
            .renderers()
            .iter()
            .find(|renderer| renderer.name() == "separation")
            .unwrap();
        assert_eq!(measurement.atom_intr_lines().len(), 1);
        assert!(measurement.atom_intr_lines()[0].positions().is_empty());
        let renderer = state
            .renderer_summaries(id)
            .into_iter()
            .find(|renderer| renderer.name == "pym:sticks")
            .unwrap();
        let (kind, props, _) = state.renderer_coloring_state(id, renderer.uid).unwrap();
        assert_eq!(kind, "BfacColoring");
        assert_eq!(props["mode"], "bfac");
        assert_eq!(props["lowpar"], "0");
        assert_eq!(props["highpar"], "40");
        assert!(run(
            &mut state,
            &mut camera,
            &mut session,
            "spectrum count, rainbow, resi 1"
        )
        .is_err());
        run(
            &mut state,
            &mut camera,
            &mut session,
            "hide labels; label resi 1, name; label resi 2, \"\"",
        )
        .unwrap();
        let object = state.scene_manager().object(id).unwrap();
        let labels = object
            .renderers()
            .iter()
            .find(|renderer| renderer.name() == "pym:labels")
            .unwrap();
        assert!(labels.is_visible());
        assert_eq!(object.name_label_entries(labels).len(), 1);
    }

    #[test]
    fn pymol_map_reuse_boundary_and_level_query_preserve_renderer_identity() {
        let mut state = super::super::tests::state();
        let grid = cuemol_xtal::read_opendx_str("object 1 class gridpositions counts 2 2 2\norigin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 2 2 2\nobject 3 class array type double rank 0 items 8 data follows\n0 1 2 3 4 5 6 7\n").unwrap();
        let id = state.add_density_map(
            state.active_scene().unwrap(),
            "density",
            cuemol_xtal::DensityMap::from_grid_map(grid),
        );
        let mut camera = Camera::new();
        camera.set_center(cuemol_core::Vector4::new3(12., 8., 4.));
        let mut session = Session::default();
        run(&mut state, &mut camera, &mut session, "isomesh mesh, density, 1.5; isosurface solid, density, 2.0, resi 1, carve=3; isomesh mesh, density, 2.5").unwrap();
        let maps = state.density_map_summaries(state.active_scene().unwrap());
        let map = maps.iter().find(|map| map.uid == id).unwrap();
        assert_eq!(map.renderers.len(), 2);
        let mesh = map
            .renderers
            .iter()
            .find(|renderer| renderer.name == "mesh")
            .unwrap();
        let props = crate::nodes::renderer_properties(&state, mesh.uid).unwrap();
        assert_eq!(props["siglevel"], "2.5");
        let solid = map
            .renderers
            .iter()
            .find(|renderer| renderer.name == "solid")
            .unwrap();
        let props = crate::nodes::renderer_properties(&state, solid.uid).unwrap();
        assert_eq!(props["bndry_molname"], "protein");
        assert_eq!(props["bndry_rng"], "3");
        assert_eq!(
            run(
                &mut state,
                &mut camera,
                &mut session,
                "isolevel mesh, query=1"
            )
            .unwrap(),
            ["isolevel: density/mesh at 2.5"]
        );
    }

    #[test]
    fn pymol_save_exports_only_selected_world_coordinates_without_relinking_source() {
        let mut state = super::super::tests::state();
        let id = state.active_object_summaries()[0].uid;
        state
            .scene_manager_mut()
            .object_mut(id)
            .unwrap()
            .set_xform_matrix(cuemol_core::Matrix4::make_trans_mat(
                cuemol_core::Vector4::new3(5., 0., 0.),
            ));
        let before = state.clone();
        let mut session = Session::default();
        let actions = session
            .plan_dialect(
                &state,
                &Camera::new(),
                Dialect::PyMol,
                "save chosen.pdb, resi 2",
            )
            .unwrap();
        let bytes = actions
            .iter()
            .find_map(|action| {
                if let Action::WriteFile { bytes, .. } = action {
                    Some(bytes)
                } else {
                    None
                }
            })
            .unwrap();
        let molecule = cuemol_molstr::read_pdb_str(std::str::from_utf8(bytes).unwrap()).unwrap();
        assert_eq!(molecule.atom_count(), 1);
        assert_eq!(molecule.atoms()[0].res_id, 2);
        assert_eq!(molecule.atoms()[0].position.x, 7.);
        assert!(before.same_undo_state(&state));
        let actions = session
            .plan_dialect(
                &state,
                &Camera::new(),
                Dialect::PyMol,
                "png figure, width=2in, dpi=100",
            )
            .unwrap();
        assert!(matches!(
            actions.last(),
            Some(Action::Render {
                width: 200,
                height: 150,
                ..
            })
        ));
    }
}
