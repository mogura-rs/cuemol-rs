use super::{
    colors, display,
    execute::{tool, warnings, Context, Deferred, Target},
    paint, surface, Command,
};
use crate::console::Action;
use cuemol_core::Uid;
use serde_json::json;
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;
pub(super) fn alpha(percent: f64) -> String {
    (1. - percent / 100.).clamp(0., 1.).to_string()
}
pub(super) fn properties(
    ids: &[Uid],
    prop: &str,
    value: &str,
    out: &mut Vec<Action>,
) -> Result<()> {
    for id in ids {
        out.push(tool(
            "set_node_prop",
            json!({"nodeType":"renderer","nodeId":id.get(),"prop":prop,"value":value}),
        )?);
    }
    Ok(())
}
fn repeat(mut actions: Vec<Action>, line: &str, aliases: &[String]) -> Vec<Action> {
    actions.push(Action::ChimeraX(Box::new(Deferred::Line {
        text: line.into(),
        aliases: aliases.into(),
    })));
    actions
}
pub(super) fn plan(
    context: &Context<'_>,
    command: &Command,
    args: &BTreeMap<String, String>,
    line: &str,
    aliases: &[String],
) -> Result<Vec<Action>> {
    let name = command.name.as_str();
    let mut out = warnings(command, args);
    match name {
        "surface" | "surface hidePatches" | "surface close" | "~surface" => {
            let enclose = name == "surface" && !args["enclose"].is_empty();
            let targets =
                context.targets(&args[if enclose { "enclose" } else { "atoms" }], true)?;
            if name == "surface" {
                let create = surface::ensure(context, &targets, enclose)?;
                if !create.is_empty() {
                    return Ok(repeat(create, line, aliases));
                }
            }
            for target in targets {
                match name {
                    "surface close" => surface::close(context, &target, &mut out)?,
                    "surface" => {
                        let ids = surface::show(context, &target, enclose, &mut out)?;
                        if !args["probeRadius"].is_empty() {
                            properties(&ids, "proberad", &args["probeRadius"], &mut out)?;
                        }
                        if !args["color"].is_empty() {
                            paint::apply_ids(
                                context,
                                &target,
                                &ids,
                                &[(
                                    target.selection.clone(),
                                    colors::split(&args["color"]).0.into(),
                                )],
                                &mut out,
                            )?;
                        }
                        if !args["transparency"].is_empty() {
                            properties(
                                &ids,
                                "alpha",
                                &alpha(args["transparency"].parse().unwrap()),
                                &mut out,
                            )?;
                        }
                    }
                    _ => surface::hide(context, &target, &mut out)?,
                }
            }
        }
        "transparency" => {
            if args["percent"].is_empty() {
                return Err("Error: transparency: give a percentage (0-100)".into());
            }
            let what = args["what"]
                .split(',')
                .map(|s| match s {
                    "all" => "acs",
                    "atoms" | "bonds" => "a",
                    "cartoons" | "ribbons" => "c",
                    "surfaces" => "s",
                    _ => "",
                })
                .collect::<String>();
            let parts = if !args["target"].is_empty() {
                &args["target"]
            } else if !what.is_empty() {
                &what
            } else {
                "s"
            };
            if let Some(bad) = parts.chars().find(|ch| !"abcrs".contains(*ch)) {
                return Err(format!(
                    "Error: transparency: target \"{bad}\" is not supported (a b c r s)"
                ));
            }
            let targets = context.targets(&args["objects"], true)?;
            if let Some(t) = targets.iter().find(|t| t.selection != "*") {
                return Err(format!("Error: transparency: \"{}\" is part of #{}; CueMol sets transparency per representation, so give whole models",args["objects"],t.model.number));
            }
            if parts.contains(['a', 'b', 'c', 'r']) {
                let create = display::ensure(context, &targets)?;
                if !create.is_empty() {
                    return Ok(repeat(create, line, aliases));
                }
            }
            for target in targets {
                let owned = display::owned(context, target.model.id);
                let mut ids = Vec::new();
                if parts.contains(['a', 'b']) {
                    ids.extend(
                        display::KINDS[..3]
                            .iter()
                            .filter_map(|k| owned.get(k.0))
                            .copied(),
                    );
                }
                if parts.contains(['c', 'r']) {
                    ids.extend(
                        display::KINDS[3..]
                            .iter()
                            .filter_map(|k| owned.get(k.0))
                            .copied(),
                    );
                }
                if parts.contains('s') {
                    ids.extend(
                        surface::surfaces(context, target.model.id)
                            .iter()
                            .map(|s| s.0),
                    );
                }
                properties(
                    &ids,
                    "alpha",
                    &alpha(args["percent"].parse().unwrap()),
                    &mut out,
                )?;
            }
        }
        _ => labels(context, name, args, &mut out)?,
    }
    Ok(out)
}
fn labels(
    context: &Context<'_>,
    name: &str,
    args: &BTreeMap<String, String>,
    out: &mut Vec<Action>,
) -> Result<()> {
    let deleting = name != "label";
    let kinds = match args["otype"].as_str() {
        "" if deleting => vec!["residues", "atoms"],
        "" => vec!["residues"],
        "residues" => vec!["residues"],
        "atoms" => vec!["atoms"],
        other => return Err(format!("Error: label: {other} labels are not supported")),
    };
    for kind in kinds {
        let format = if deleting {
            None
        } else {
            Some(label_format(kind, &args["text"], &args["attribute"])?)
        };
        for target in context.targets(&args["objects"], true)? {
            let renderer = format!("chx:label:{kind}");
            if deleting && !display::owned(context, target.model.id).contains_key(&renderer) {
                continue;
            }
            let selection = if kind == "residues" {
                let residues = Target {
                    selection: if target.selection == "*" {
                        "*".into()
                    } else {
                        format!("byres ({})", target.selection)
                    },
                    ..target.clone()
                };
                if deleting {
                    residues.selection
                } else {
                    let object = context
                        .state
                        .scene_manager()
                        .object(target.model.id)
                        .ok_or("No molecule")?;
                    let mut anchors: Vec<(usize, i32)> = Vec::new();
                    let mut by_residue = BTreeMap::new();
                    for index in context.indices(&residues)? {
                        let a = &object.mol().atoms()[index];
                        let key = (a.chain_id.as_str(), a.res_id, a.ins_code);
                        let rank = match a.name.as_str() {
                            "CA" => 0,
                            "C4'" => 1,
                            _ => 2,
                        };
                        let index = *by_residue.entry(key).or_insert_with(|| {
                            anchors.push((rank, a.serial));
                            anchors.len() - 1
                        });
                        if rank < anchors[index].0 {
                            anchors[index] = (rank, a.serial);
                        }
                    }
                    if anchors.is_empty() {
                        continue;
                    }
                    format!(
                        "aid {}",
                        anchors
                            .iter()
                            .map(|e| e.1.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
            } else {
                target.selection.clone()
            };
            out.push(tool("set_atom_labels",json!({"objId":target.model.id.get(),"selection":selection,"format":format,"rendererName":renderer}))?);
            if !deleting {
                let mut props = Vec::new();
                for (key, prop) in [
                    ("color", "color"),
                    ("size", "font_size"),
                    ("onTop", "label_on_top"),
                ] {
                    if !args[key].is_empty() {
                        props.push((
                            prop.into(),
                            if key == "color" {
                                colors::split(&args[key]).0.into()
                            } else {
                                args[key].clone()
                            },
                        ));
                    }
                }
                if !props.is_empty() {
                    out.push(Action::ChimeraX(Box::new(Deferred::Properties {
                        object: target.model.id,
                        name: renderer,
                        props,
                    })));
                }
            }
        }
    }
    Ok(())
}
fn label_format(kind: &str, text: &str, attribute: &str) -> Result<String> {
    if !text.is_empty() {
        return Ok(text.replace('{', "{{").replace('}', "}}"));
    }
    if attribute.is_empty() {
        return Ok(if kind == "atoms" {
            "{name}"
        } else {
            "{resn} {resi}"
        }
        .into());
    }
    let fields: &[(&str, &str)] = if kind == "atoms" {
        &[
            ("name", "{name}"),
            ("element", "{elem}"),
            ("bfactor", "{bfac:.3g}"),
            ("occupancy", "{occ:.3g}"),
            ("alt_loc", "{alt}"),
        ]
    } else {
        &[
            ("name", "{resn}"),
            ("number", "{resi}"),
            ("chain_id", "{chain}"),
        ]
    };
    fields
        .iter()
        .find(|e| e.0 == attribute)
        .map(|e| e.1.into())
        .ok_or_else(|| {
            format!(
                "Error: label: {kind} attribute \"{attribute}\" is not supported ({})",
                fields.iter().map(|e| e.0).collect::<Vec<_>>().join(", ")
            )
        })
}
