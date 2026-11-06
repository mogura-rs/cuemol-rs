use super::{
    colors, display,
    execute::{tool, warnings, Context, Deferred, Target},
    palette, Command,
};
use crate::console::Action;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, String>;
pub(super) type Entry = (String, String);
pub(super) const POLYMER: &str = "(protein or nucleic)";
pub(super) fn residues(
    context: &Context<'_>,
    target: &Target,
) -> Result<Vec<(String, Vec<String>)>> {
    let object = context
        .state
        .scene_manager()
        .object(target.model.id)
        .ok_or("No molecule")?;
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut by_chain = BTreeMap::new();
    for index in context.indices(target)? {
        let a = &object.mol().atoms()[index];
        let res = format!(
            "{}{}",
            a.res_id,
            a.ins_code
                .filter(|c| !c.is_whitespace())
                .map(|c| c.to_string())
                .unwrap_or_default()
        );
        if !seen.insert((a.chain_id.clone(), res.clone())) {
            continue;
        }
        let index = *by_chain.entry(a.chain_id.as_str()).or_insert_with(|| {
            out.push((a.chain_id.clone(), Vec::new()));
            out.len() - 1
        });
        out[index].1.push(res);
    }
    Ok(out)
}
pub(super) fn element_entries(selection: &str, carbon: bool) -> Vec<Entry> {
    [
        "C", "N", "O", "S", "P", "H", "F", "Cl", "Br", "I", "Se", "Fe", "Zn", "Mg", "Ca", "Mn",
        "Cu", "Co", "Ni", "Na", "K",
    ]
    .into_iter()
    .filter(|e| carbon || *e != "C")
    .map(|e| {
        (
            display::intersect(selection, &format!("elem {e}")),
            colors::tables()
                .elements
                .get(e)
                .cloned()
                .unwrap_or_else(|| "#b4b4b4".into()),
        )
    })
    .collect()
}
pub(super) fn apply(
    context: &Context<'_>,
    target: &Target,
    parts: &str,
    entries: &[Entry],
    atoms_only: bool,
    actions: &mut Vec<Action>,
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let ids = display::owned(context, target.model.id);
    let mut renderers = Vec::new();
    if parts.contains(['a', 'b']) {
        renderers.extend(
            display::KINDS[..3]
                .iter()
                .filter_map(|k| ids.get(k.0))
                .copied(),
        );
    }
    if !atoms_only && parts.contains(['c', 'r']) {
        renderers.extend(
            display::KINDS[3..]
                .iter()
                .filter_map(|k| ids.get(k.0))
                .copied(),
        );
    }
    if parts.contains('s') {
        renderers.extend(
            ids.iter()
                .filter(|(name, _)| name.starts_with("chx:surface:"))
                .map(|(_, id)| *id),
        );
    }
    apply_ids(context, target, &renderers, entries, actions)
}
pub(super) fn apply_ids(
    context: &Context<'_>,
    target: &Target,
    renderers: &[cuemol_core::Uid],
    entries: &[Entry],
    actions: &mut Vec<Action>,
) -> Result<()> {
    for &renderer in renderers {
        if context
            .state
            .renderer_coloring_type(target.model.id, renderer)
            .as_deref()
            != Some("PaintColoring")
            && !actions.iter().any(|action| {
                matches!(action, Action::Tool { name, args }
                if name=="set_renderer_coloring" && args["nodeId"].as_u64()==Some(renderer.get()))
            })
        {
            actions.push(tool(
                "set_renderer_coloring",
                json!({"nodeId":renderer.get(),"nodeType":"renderer","mode":"paint-type-paint"}),
            )?);
        }
        for (selection, color) in entries {
            actions.push(tool("add_paint_at",json!({"nodeId":renderer.get(),"nodeType":"renderer","selection":selection,"color":color,"at":1}))?);
        }
    }
    Ok(())
}
pub(super) fn plan(
    context: &Context<'_>,
    command: &Command,
    args: &BTreeMap<String, String>,
    line: &str,
    aliases: &[String],
) -> Result<Vec<Action>> {
    let targets = context.targets(&args["objects"], true)?;
    let mut out = warnings(command, args);
    let parts = if args["target"].is_empty() {
        "acsbpf"
    } else {
        &args["target"]
    };
    let color = args.get("color").map(String::as_str).unwrap_or("");
    if command.name == "color" {
        if color.is_empty() {
            return Err("Error: color: give a colour or one of byhetero, byelement, bychain, bymodel, random".into());
        }
        let why = match color {
            "bypolymer" | "byidentity" => {
                Some("it needs ChimeraX's seeded random colour per sequence")
            }
            "bynucleotide" | "fromatoms" | "fromcartoons" | "fromribbons" => {
                Some("not supported yet")
            }
            "byatom" => Some("use byelement"),
            _ => None,
        };
        if let Some(why) = why {
            return Err(format!("Error: color: {color} is not supported ({why})"));
        }
    } else if args["level"] == "polymers" {
        return Err(
            "Error: rainbow: polymers is not supported (the console does not compare sequences)"
                .into(),
        );
    }
    let palette = if command.name == "rainbow" {
        Some(
            palette::Palette::read(if args["palette"].is_empty() {
                "rainbow"
            } else {
                &args["palette"]
            })
            .map_err(|e| format!("Error: rainbow: {e}"))?,
        )
    } else {
        None
    };
    let mut create = display::ensure(context, &targets)?;
    if !create.is_empty() {
        create.push(Action::ChimeraX(Box::new(Deferred::Line {
            text: line.into(),
            aliases: aliases.into(),
        })));
        return Ok(create);
    }
    let (color, alpha) = colors::split(color);
    if alpha.is_some_and(|a| a < 1.) {
        out.push(Action::Output(
            "Warning: color: transparency in a colour is not supported yet; drawn opaque".into(),
        ));
    }
    if color == "random" {
        out.push(Action::Output(
            "Warning: color: random gives each residue a colour (ChimeraX gives each atom one)"
                .into(),
        ));
    }
    let model_colors = palette
        .as_ref()
        .map(|p| p.colors(targets.len()))
        .unwrap_or_default();
    for (i, target) in targets.iter().enumerate() {
        let mut entries = Vec::new();
        if let Some(palette) = &palette {
            if args["level"] == "structures" {
                entries.push((target.selection.clone(), model_colors[i].clone()));
            } else {
                let polymer = Target {
                    selection: display::intersect(&target.selection, POLYMER),
                    ..target.clone()
                };
                let mut chains = residues(context, &polymer)?;
                if args["level"] == "chains" {
                    chains.sort_by(|a, b| a.0.cmp(&b.0));
                    let count = chains.len();
                    for ((chain, _), color) in chains.into_iter().zip(palette.colors(count)) {
                        entries.push((format!("chain {chain} and {POLYMER}"), color));
                    }
                } else {
                    for (chain, residues) in chains {
                        for (first, last, color) in palette.ranges(&residues) {
                            let resi = if first == last {
                                first
                            } else {
                                format!("{first}:{last}")
                            };
                            entries.push((
                                display::intersect(
                                    &target.selection,
                                    &format!("chain {chain} and resi {resi} and {POLYMER}"),
                                ),
                                color,
                            ));
                        }
                    }
                }
            }
        } else {
            match color {
                "byelement" | "byhetero" => {
                    entries = element_entries(&target.selection, color == "byelement")
                }
                "bymodel" => {
                    let (color, exact) = palette::model(target.model.number);
                    if !exact {
                        out.push(Action::Output(format!("Warning: color: #{} is past ChimeraX's model colours; its colour differs from ChimeraX's",target.model.number)));
                    }
                    entries.push((target.selection.clone(), color));
                }
                "bychain" => {
                    let mut chains = residues(context, target)?;
                    chains.sort_by(|a, b| a.0.cmp(&b.0));
                    for (chain, _) in chains {
                        let (color, exact) = palette::chain(&chain);
                        if !exact {
                            out.push(Action::Output(format!("Warning: color: chain {chain} has no colour in ChimeraX's table; its colour differs from ChimeraX's")));
                        }
                        entries.push((
                            display::intersect(&target.selection, &format!("chain {chain}")),
                            color,
                        ));
                    }
                }
                "random" => {
                    for (chain, residues) in residues(context, target)? {
                        for residue in residues {
                            entries.push((
                                display::intersect(
                                    &target.selection,
                                    &format!("chain {chain} and resi {residue}"),
                                ),
                                palette::random()?,
                            ));
                        }
                    }
                }
                _ => entries.push((target.selection.clone(), color.into())),
            }
        }
        apply(
            context,
            target,
            parts,
            &entries,
            matches!(color, "byelement" | "byhetero"),
            &mut out,
        )?;
    }
    Ok(out)
}
