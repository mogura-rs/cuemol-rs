use super::{catalog, spec, syntax, Command, Model};
use crate::console::{Action, Session};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

type Args = BTreeMap<String, String>;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    tables: BTreeMap<Uid, BTreeMap<Uid, u32>>,
    aliases: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Deferred {
    Line {
        text: String,
        aliases: Vec<String>,
    },
    Style {
        object: Uid,
        name: String,
        styles: String,
    },
    SelectedCount,
    Opened {
        before: Vec<Uid>,
    },
    AutoStyle {
        object: Uid,
    },
    Rmsd {
        mov: Uid,
        reference: Uid,
        mov_sel: String,
        ref_sel: String,
    },
    Properties {
        object: Uid,
        name: String,
        props: Vec<(String, String)>,
    },
}

pub(super) fn tool(name: &str, args: Value) -> Result<Action> {
    crate::validate_args(name, &args)?;
    Ok(Action::Tool {
        name: name.into(),
        args,
    })
}
fn output(text: impl Into<String>) -> Action {
    Action::Output(text.into())
}
fn deferred(step: Deferred) -> Action {
    Action::ChimeraX(Box::new(step))
}

impl State {
    pub(super) fn models(&mut self, state: &DesktopAppState) -> Result<Vec<Model>> {
        let Some(scene) = state.active_scene() else {
            return Ok(Vec::new());
        };
        let table = self.tables.entry(scene).or_default();
        let mut next = table.values().copied().max().unwrap_or(0) + 1;
        let mut result = Vec::new();
        let mut objects = crate::console::nodes(state)?
            .into_iter()
            .filter(|node| node.kind == "object")
            .collect::<Vec<_>>();
        objects.sort_by_cached_key(|object| {
            let id = Uid::new(object.id);
            let order = state
                .scene_manager()
                .object(id)
                .map(|o| o.ui_order())
                .or_else(|| state.density_map_summary(id).map(|o| o.ui_order))
                .or_else(|| state.surface_summary(id).map(|o| o.ui_order))
                .or_else(|| state.lw_object_summary(id).map(|o| o.ui_order))
                .unwrap_or(0);
            (order, object.id)
        });
        for object in objects {
            let id = Uid::new(object.id);
            let number = *table.entry(id).or_insert_with(|| {
                let number = next;
                next += 1;
                number
            });
            result.push(Model {
                id,
                number,
                name: object.name,
                molecule: object.class == "MolCoord" || object.class.ends_with("Mol"),
            });
        }
        result.sort_by_key(|model| model.number);
        Ok(result)
    }
}

pub(super) fn selection_names(state: &DesktopAppState) -> BTreeSet<String> {
    let (scene, global) = state.selection_definitions();
    scene.into_iter().chain(global).collect()
}

pub(super) struct Context<'a> {
    pub state: &'a DesktopAppState,
    pub models: Vec<Model>,
    pub names: BTreeSet<String>,
}
#[derive(Clone)]
pub(super) struct Target {
    pub model: Model,
    pub selection: String,
}
impl Context<'_> {
    pub fn targets(&self, text: &str, molecules: bool) -> Result<Vec<Target>> {
        let expr = if text.trim().is_empty() {
            None
        } else {
            Some(
                spec::parse(text, &self.names)
                    .map_err(|e| e.display(text))?
                    .ok_or_else(|| format!("Error: \"{text}\" is not an atom specification"))?
                    .0,
            )
        };
        let mols = self
            .models
            .iter()
            .filter(|m| m.molecule)
            .cloned()
            .collect::<Vec<_>>();
        let context = spec::Context {
            state: self.state,
            models: &mols,
            names: &self.names,
        };
        let mut targets = Vec::new();
        for model in self.models.iter().filter(|m| !molecules || m.molecule) {
            let selection = expr
                .as_ref()
                .map(|e| context.emit(e, model).map_err(|e| e.display(text)))
                .transpose()?
                .unwrap_or_else(|| "*".into());
            if selection != "none" {
                targets.push(Target {
                    model: model.clone(),
                    selection,
                });
            }
        }
        Ok(targets)
    }
    pub fn indices(&self, target: &Target) -> Result<Vec<usize>> {
        let Some(object) = self.state.scene_manager().object(target.model.id) else {
            return Ok(Vec::new());
        };
        let selector = self
            .state
            .parse_object_selector(target.model.id, &target.selection)
            .map_err(|e| e.to_string())?;
        Ok(object.mol().select_indices(&selector))
    }
}

pub(crate) fn resume(
    session: &mut Session,
    state: &DesktopAppState,
    camera: &Camera,
    action: Deferred,
) -> Result<Vec<Action>> {
    match action {
        Deferred::Rmsd {
            mov,
            reference,
            mov_sel,
            ref_sel,
        } => {
            let context = Context {
                state,
                models: session.chimerax.models(state)?,
                names: selection_names(state),
            };
            super::analysis::report(&context, mov, reference, &mov_sel, &ref_sel)
        }
        Deferred::Opened { before } => {
            let models = session.chimerax.models(state)?;
            let mut out = Vec::new();
            for model in models.into_iter().filter(|m| !before.contains(&m.id)) {
                out.push(output(format!(
                    "Opened {} as #{}",
                    model.name, model.number
                )));
                if model.molecule {
                    out.push(deferred(Deferred::AutoStyle { object: model.id }));
                }
            }
            Ok(out)
        }
        Deferred::AutoStyle { object } => {
            let context = Context {
                state,
                models: session.chimerax.models(state)?,
                names: selection_names(state),
            };
            let model = context
                .models
                .iter()
                .find(|m| m.id == object)
                .ok_or("Opened model no longer exists")?;
            super::auto_style::apply(&context, model)
        }
        Deferred::Properties {
            object,
            name,
            props,
        } => {
            let id = state
                .renderer_summaries(object)
                .into_iter()
                .find(|r| r.name == name)
                .ok_or_else(|| format!("Error: could not make {name}"))?
                .uid;
            let mut out = Vec::new();
            for (prop, value) in props {
                super::representation::properties(&[id], &prop, &value, &mut out)?;
            }
            Ok(out)
        }
        Deferred::Line { text, aliases } => plan(session, state, camera, &text, &aliases),
        Deferred::Style {
            object,
            name,
            styles,
        } => {
            let renderer = state
                .renderer_summaries(object)
                .into_iter()
                .find(|r| r.name == name)
                .ok_or_else(|| format!("Error: could not make {name}"))?;
            Ok(vec![tool(
                "set_renderer_styles",
                json!({"rendId":renderer.uid.get(),"styles":styles}),
            )?])
        }
        Deferred::SelectedCount => {
            let mut total = 0;
            for object in state.active_object_summaries() {
                let text = state
                    .molecule_selection_text(object.uid)
                    .map_err(|e| e.to_string())?;
                if text.trim().is_empty() || text == "none" {
                    continue;
                }
                let selector = state
                    .parse_object_selector(object.uid, text)
                    .map_err(|e| e.to_string())?;
                total += state
                    .scene_manager()
                    .object(object.uid)
                    .unwrap()
                    .mol()
                    .atom_sel_count(&selector);
            }
            Ok(vec![output(if total == 0 {
                "Nothing selected".into()
            } else {
                format!("{total} atoms selected")
            })])
        }
    }
}

pub(crate) fn plan(
    session: &mut Session,
    state: &DesktopAppState,
    camera: &Camera,
    text: &str,
    expanding: &[String],
) -> Result<Vec<Action>> {
    let Some(first) = syntax::next_token(text) else {
        return Ok(Vec::new());
    };
    if let Some(body) = session
        .chimerax
        .aliases
        .get(first.value)
        .filter(|_| !expanding.iter().any(|name| name == first.value))
        .cloned()
    {
        if expanding.len() >= 10 {
            return Err(format!("Error: {}: aliases nested too deeply", first.value));
        }
        let mut words = Vec::new();
        let mut rest = first.rest;
        while let Some(token) = syntax::next_token(rest) {
            words.push(token.value);
            rest = token.rest;
        }
        let text = re!(r"\$(\*|[1-9])").replace_all(&body, |captures: &regex::Captures<'_>| {
            if &captures[1] == "*" {
                first.rest.trim()
            } else {
                words
                    .get(captures[1].parse::<usize>().unwrap() - 1)
                    .copied()
                    .unwrap_or("")
            }
        });
        let lines = syntax::split_lines(&text);
        let names = selection_names(state);
        for line in &lines {
            if let Ok((command, _)) = syntax::resolve(line, catalog()) {
                if let Ok(args) = syntax::bind(command, line, &names) {
                    if outside(command, &args) {
                        return Err(format!("Error: alias {}: {} cannot run inside an alias; run it on its own line", first.value, command.name));
                    }
                }
            }
        }
        let mut aliases = expanding.to_vec();
        aliases.push(first.value.into());
        return Ok(lines
            .into_iter()
            .map(|text| {
                deferred(Deferred::Line {
                    text,
                    aliases: aliases.clone(),
                })
            })
            .collect());
    }
    let (command, _) = syntax::resolve(text, catalog())?;
    let context = Context {
        state,
        models: session.chimerax.models(state)?,
        names: selection_names(state),
    };
    let args = syntax::bind(command, text, &context.names)?;
    let mut actions = match command.name.as_str() {
        "help" => vec![output(super::help(&args["topic"])?)],
        "open" | "close" | "delete" | "save" => {
            super::files::plan(session, &context, camera, command, &args)?
        }
        "distance" | "distance delete" | "~distance" | "align" | "matchmaker" | "mmaker" => {
            super::analysis::plan(&context, command, &args)?
        }
        "usage" => vec![output(super::usage(&args["commandName"])?)],
        "undo" | "redo" => vec![tool(&command.name, json!({}))?],
        "alias" | "alias list" | "alias delete" | "~alias" => {
            aliases(&mut session.chimerax, &command.name, &args)?
        }
        "select" | "select add" | "select subtract" | "select intersect" | "select clear"
        | "~select" => select(&context, &command.name, &args)?,
        "name" | "name frozen" | "name delete" | "name list" => {
            names(&context, &command.name, &args)?
        }
        "info" | "info models" | "info chains" | "info residues" | "info atoms" => {
            info(&context, &command.name, &args)?
        }
        "view"
        | "view list"
        | "view name"
        | "view delete"
        | "view matrix"
        | "turn"
        | "move"
        | "cofr"
        | "zoom"
        | "set"
        | "graphics silhouettes" => super::view::plan(&context, camera, command, &args)?,
        "color" | "rainbow" => {
            return super::paint::plan(&context, command, &args, text, expanding)
        }
        "surface"
        | "surface hidePatches"
        | "surface close"
        | "~surface"
        | "transparency"
        | "label"
        | "label delete"
        | "~label" => {
            return super::representation::plan(&context, command, &args, text, expanding)
        }
        "show" | "hide" | "style" | "cartoon" | "cartoon hide" => {
            return super::display::plan(&context, command, &args, text, expanding)
        }
        _ => return Err(format!("Unknown ChimeraX command: {}", command.name)),
    };
    let mut warnings = warnings(command, &args);
    warnings.append(&mut actions);
    Ok(warnings)
}

pub(super) fn outside(command: &Command, args: &Args) -> bool {
    let qsc = |name: &str| {
        args.get(name)
            .is_some_and(|value| value.to_ascii_lowercase().ends_with(".qsc"))
    };
    matches!(command.name.as_str(), "undo" | "redo")
        || command.name == "open" && qsc("names")
        || command.name == "save"
            && (qsc("filename")
                || args.get("format").is_some_and(|s| {
                    s.eq_ignore_ascii_case("qsc") || s.eq_ignore_ascii_case("session")
                }))
}
pub(super) fn warnings(command: &Command, args: &Args) -> Vec<Action> {
    command
        .args
        .iter()
        .filter(|arg| arg.ignored && args.get(&arg.name).is_some_and(|value| !value.is_empty()))
        .map(|arg| {
            output(format!(
                "Warning: {}: {} is not supported yet; ignored",
                command.name, arg.name
            ))
        })
        .collect()
}
fn aliases(state: &mut State, command: &str, args: &Args) -> Result<Vec<Action>> {
    let mut out = Vec::new();
    match command {
        "alias list" => out.push(output(if state.aliases.is_empty() {
            "No aliases.".into()
        } else {
            format!(
                "{} aliases: {}",
                state.aliases.len(),
                state.aliases.keys().cloned().collect::<Vec<_>>().join(", ")
            )
        })),
        "alias delete" | "~alias" => {
            if args["name"] == "all" {
                state.aliases.clear();
            } else if state.aliases.remove(&args["name"]).is_none() {
                return Err(format!("Error: no alias named \"{}\"", args["name"]));
            }
        }
        _ if args["name"].is_empty() => out.extend(
            state
                .aliases
                .iter()
                .map(|(name, text)| output(format!("{name}: {text}"))),
        ),
        _ if args["text"].is_empty() => {
            let text = state
                .aliases
                .get(&args["name"])
                .ok_or_else(|| format!("Error: no alias named \"{}\"", args["name"]))?;
            out.push(output(format!("{}: {text}", args["name"])));
        }
        _ => {
            state
                .aliases
                .insert(args["name"].clone(), args["text"].clone());
        }
    }
    Ok(out)
}

fn select(context: &Context<'_>, command: &str, args: &Args) -> Result<Vec<Action>> {
    let expression = args.get("objects").map(String::as_str).unwrap_or("");
    let residues = args.get("residues").is_some_and(|v| v == "true");
    let subtract = matches!(command, "select subtract" | "select clear" | "~select");
    let targets = context.targets(expression, true)?;
    let mut actions = Vec::new();
    for model in context.models.iter().filter(|m| m.molecule) {
        let mut term = targets
            .iter()
            .find(|t| t.model.id == model.id)
            .map(|t| t.selection.clone())
            .unwrap_or_else(|| "none".into());
        if subtract && expression.trim().is_empty() {
            term = "*".into();
        }
        if residues && term != "none" && term != "*" {
            term = format!("byres ({term})");
        }
        let now = context
            .state
            .molecule_selection_text(model.id)
            .unwrap_or("");
        let now = if now.trim().is_empty() { "none" } else { now };
        let next = match command {
            "select add" => super::display::union(now, &term),
            "select intersect" => super::display::intersect(now, &term),
            _ if subtract => super::display::minus(now, &term),
            _ => term,
        };
        if next != now {
            actions.push(tool(
                "set_mol_selection",
                json!({"objId":model.id.get(),"selection":if next=="none" { "" } else { &next }}),
            )?);
        }
    }
    actions.push(deferred(Deferred::SelectedCount));
    Ok(actions)
}
fn names(context: &Context<'_>, command: &str, args: &Args) -> Result<Vec<Action>> {
    let definitions = crate::console::pymol::named_selections(context.state);
    let name = args.get("name").map(String::as_str).unwrap_or("");
    match command {
        "name frozen" => Err("Error: name frozen is not supported (a CueMol named selection is an expression, re-evaluated where it is used)".into()),
        "name list" => Ok(if definitions.is_empty() { vec![output("There are no user-defined specifier names.")] } else { definitions.iter().map(|(_,name,text)|output(format!("{name}: {text}"))).collect() }),
        "name delete" => {
            let found = definitions.iter().filter(|(_,n,_)|name=="all"||n==name).collect::<Vec<_>>();
            if found.is_empty() && name!="all" { return Err(format!("Error: no name \"{name}\"")) }
            found.into_iter().map(|(style,name,_)|tool("remove_selection",json!({"name":name,"styleSet":style}))).collect()
        }
        _ if args["text"].trim().is_empty() => {
            let def = definitions.iter().find(|(_,n,_)|n==name).ok_or_else(||format!("Error: no name \"{name}\""))?;
            Ok(vec![output(format!("{name}: {}",def.2))])
        }
        _ => {
            if !re!(r"^[_a-zA-Z0-9][*'_a-zA-Z0-9]*$").is_match(name) || matches!(name,"all"|"sel") { return Err(format!("Error: name: \"{name}\" cannot be a name (letters, digits and _ only; not all or sel)")) }
            let targets = context.targets(&args["text"],true)?;
            let expressions = context.models.iter().filter(|m|m.molecule).map(|m|targets.iter().find(|t|t.model.id==m.id).map(|t|t.selection.as_str()).unwrap_or("none")).collect::<BTreeSet<_>>();
            if expressions.len()>1 { return Err("Error: name: the specification differs between models (it names a model, or reaches across them), which a CueMol named selection cannot keep".into()) }
            Ok(vec![tool("define_selection",json!({"name":name,"selection":expressions.first().copied().unwrap_or("none")}))?])
        }
    }
}

fn info(context: &Context<'_>, command: &str, args: &Args) -> Result<Vec<Action>> {
    let targets = context.targets(&args["objects"], true)?;
    let attribute = args.get("attribute").map(String::as_str).unwrap_or("");
    let supported: &[&str] = match command {
        "info models" | "info residues" => &["name"],
        "info chains" => &["chain_id"],
        "info atoms" => &["name", "element", "bfactor", "occupancy"],
        _ => &[],
    };
    if !attribute.is_empty() && !supported.contains(&attribute) {
        return Err(format!(
            "Error: {command}: attribute \"{attribute}\" is not supported ({})",
            supported.join(", ")
        ));
    }
    let mut out = Vec::new();
    if command == "info" {
        let shown = context
            .models
            .iter()
            .filter(|model| {
                args["objects"].trim().is_empty() || targets.iter().any(|t| t.model.id == model.id)
            })
            .collect::<Vec<_>>();
        out.push(output(format!("{} models", shown.len())));
        for model in shown {
            let object = context.state.scene_manager().object(model.id);
            out.push(output(format!(
                "#{}, {}, {}",
                model.number,
                model.name,
                if crate::properties::node_property_values(
                    context.state,
                    &Camera::new(),
                    "object",
                    Some(model.id)
                )
                .ok()
                .and_then(|props| props.get("visible").cloned())
                .as_deref()
                    != Some("false")
                {
                    "shown"
                } else {
                    "hidden"
                }
            )));
            if let Some(object) = object {
                let atoms = object.mol().atoms();
                let residues = atoms
                    .iter()
                    .map(|a| (&a.chain_id, a.res_id, a.ins_code))
                    .collect::<BTreeSet<_>>();
                let mut chains = Vec::new();
                for atom in atoms {
                    if !chains.contains(&atom.chain_id) {
                        chains.push(atom.chain_id.clone());
                    }
                }
                out.push(output(format!(
                    "{} atoms, {} residues, {} chains ({})",
                    atoms.len(),
                    residues.len(),
                    chains.len(),
                    chains.join(",")
                )));
            }
        }
        return Ok(out);
    }
    for target in targets {
        if command == "info models" {
            let class = context
                .state
                .scene_manager()
                .object(target.model.id)
                .map(|o| o.object_type())
                .unwrap_or("MolCoord");
            out.push(output(format!(
                "model id #{} type {class} name {}",
                target.model.number, target.model.name
            )));
            continue;
        }
        let object = context
            .state
            .scene_manager()
            .object(target.model.id)
            .ok_or("No molecule")?;
        let mut seen = BTreeSet::new();
        for index in context.indices(&target)? {
            let atom = &object.mol().atoms()[index];
            let insertion = atom
                .ins_code
                .filter(|c| !c.is_whitespace())
                .map(|c| c.to_string())
                .unwrap_or_default();
            let residue = format!(
                "#{}/{}:{}{insertion}",
                target.model.number, atom.chain_id, atom.res_id
            );
            match command {
                "info chains" => {
                    if seen.insert(atom.chain_id.clone()) {
                        out.push(output(format!(
                            "chain id #{}/{} chain_id {}",
                            target.model.number, atom.chain_id, atom.chain_id
                        )));
                    }
                }
                "info residues" => {
                    if seen.insert(residue.clone()) {
                        out.push(output(format!(
                            "residue id {residue} name {}",
                            atom.res_name
                        )));
                    }
                }
                _ => {
                    let value = match attribute {
                        "name" => atom.name.clone(),
                        "element" => atom.element.clone(),
                        "bfactor" => super::view::precision(atom.b_factor as f64, 6)
                            .parse::<f64>()
                            .unwrap_or(atom.b_factor as f64)
                            .to_string(),
                        "occupancy" => super::view::precision(atom.occupancy as f64, 6)
                            .parse::<f64>()
                            .unwrap_or(atom.occupancy as f64)
                            .to_string(),
                        _ => String::new(),
                    };
                    out.push(output(format!(
                        "atom id {residue}@{}{}",
                        atom.name,
                        if attribute.is_empty() {
                            String::new()
                        } else {
                            format!(" {attribute} {value}")
                        }
                    )));
                }
            }
        }
    }
    Ok(out)
}
