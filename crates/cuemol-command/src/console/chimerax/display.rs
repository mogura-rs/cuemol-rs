use super::execute::{tool, warnings, Context, Deferred, Target};
use super::Command;
use crate::console::Action;
use cuemol_core::Uid;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, String>;
pub(super) const KINDS: &[(&str, &str, &str, &str)] = &[
    (
        "chx:stick",
        "ballstick",
        "StickBallStick,DefaultCPKColoring",
        "*",
    ),
    (
        "chx:ball",
        "ballstick",
        "DefaultBallStick,DefaultCPKColoring",
        "*",
    ),
    ("chx:sphere", "cpk", "DefaultCPK,DefaultCPKColoring", "*"),
    (
        "chx:cartoon",
        "ribbon",
        "DefaultRibbon,DefaultHSCPaint",
        "protein",
    ),
    ("chx:nucl", "nucl", "DefaultNucl", "nucleic"),
];
pub(super) fn union(a: &str, b: &str) -> String {
    if a == "*" || b == "*" {
        "*".into()
    } else if a == "none" {
        b.into()
    } else if b == "none" {
        a.into()
    } else {
        format!("({a}) or ({b})")
    }
}
pub(super) fn minus(a: &str, b: &str) -> String {
    if a == "none" || b == "none" {
        a.into()
    } else if b == "*" {
        "none".into()
    } else {
        format!("({a}) and not ({b})")
    }
}
pub(super) fn intersect(a: &str, b: &str) -> String {
    if a == "none" || b == "none" {
        "none".into()
    } else if a == "*" {
        b.into()
    } else if b == "*" {
        a.into()
    } else {
        format!("({a}) and ({b})")
    }
}
pub(super) fn owned(context: &Context<'_>, object: Uid) -> BTreeMap<String, Uid> {
    context
        .state
        .renderer_summaries(object)
        .into_iter()
        .filter(|r| r.name.starts_with("chx:"))
        .map(|r| (r.name, r.uid))
        .collect()
}
pub(super) fn ensure(context: &Context<'_>, targets: &[Target]) -> Result<Vec<Action>> {
    let mut actions = Vec::new();
    for target in targets {
        let ids = owned(context, target.model.id);
        for &(name, kind, styles, _) in KINDS {
            if ids.contains_key(name) {
                continue;
            }
            actions.push(tool("create_renderer",json!({"objId":target.model.id.get(),"rendererType":kind,"name":name,"selection":"none","centerView":false}))?);
            actions.push(Action::ChimeraX(Box::new(Deferred::Style {
                object: target.model.id,
                name: name.into(),
                styles: styles.into(),
            })));
        }
    }
    Ok(actions)
}
pub(super) fn read(context: &Context<'_>, object: Uid, renderer: Uid) -> String {
    context
        .state
        .scene_manager()
        .object(object)
        .and_then(|o| o.renderer(renderer))
        .and_then(|r| r.qsc_attr("sel"))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("*")
        .into()
}
pub(super) fn write(
    context: &Context<'_>,
    object: Uid,
    renderer: Uid,
    next: &str,
    actions: &mut Vec<Action>,
) -> Result<()> {
    let next = crate::selection::simplify_expression(next);
    if next != read(context, object, renderer) {
        actions.push(tool(
            "set_renderer_selection",
            json!({"rendId":renderer.get(),"selection":next}),
        )?);
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
    let name = command.name.as_str();
    let cartoon = name.starts_with("cartoon");
    let targets = context.targets(&args[if cartoon { "atoms" } else { "objects" }], true)?;
    let mut parts = BTreeSet::new();
    if cartoon {
        parts.insert("cartoons");
    } else if name != "style" {
        for what in args["what"].split(',').filter(|s| !s.is_empty()) {
            parts.insert(match what {
                "atoms" | "bonds" => "atoms",
                "cartoons" | "ribbons" => "cartoons",
                "models" => "models",
                "surfaces" => "surfaces",
                _ => "pseudobonds",
            });
        }
        for letter in args["target"].chars() {
            parts.insert(match letter {
                'a' | 'b' => "atoms",
                'c' | 'r' => "cartoons",
                'm' => "models",
                's' => "surfaces",
                'p' => "pseudobonds",
                _ => {
                    return Err(format!(
                        "Error: \"{letter}\" in target is not one of a b c m p r s"
                    ))
                }
            });
        }
        if parts.is_empty() {
            parts.insert("atoms");
        }
    }
    if name == "style" && args["atomStyle"].is_empty() {
        return Err("Error: style: give stick, ball or sphere".into());
    }
    if parts.contains("surfaces") && name == "show" {
        let mut create = super::surface::ensure(context, &targets, false)?;
        if !create.is_empty() {
            create.push(Action::ChimeraX(Box::new(Deferred::Line {
                text: line.into(),
                aliases: aliases.into(),
            })));
            return Ok(create);
        }
    }
    if name == "style" || parts.contains("atoms") || parts.contains("cartoons") {
        let mut create = ensure(context, &targets)?;
        if !create.is_empty() {
            create.push(Action::ChimeraX(Box::new(Deferred::Line {
                text: line.into(),
                aliases: aliases.into(),
            })));
            return Ok(create);
        }
    }
    let mut actions = warnings(command, args);
    if parts.contains("pseudobonds") {
        actions.push(Action::Output(format!(
            "Warning: {name}: pseudobonds are not supported yet; ignored"
        )));
    }
    let show = matches!(name, "show" | "cartoon");
    for target in targets {
        let object = target.model.id;
        let ids = owned(context, object);
        if parts.contains("surfaces") {
            if show {
                super::surface::show(context, &target, false, &mut actions)?;
            } else {
                super::surface::hide(context, &target, &mut actions)?;
            }
        }
        if parts.contains("models") {
            actions.push(tool(
                "set_visible",
                json!({"nodeId":object.get(),"nodeType":"object","visible":show}),
            )?);
        }
        if parts.contains("atoms") || name == "style" {
            let atom_ids = [ids["chx:stick"], ids["chx:ball"], ids["chx:sphere"]];
            let sels = atom_ids.map(|id| read(context, object, id));
            if name == "style" {
                let to = ["stick", "ball", "sphere"]
                    .iter()
                    .position(|s| *s == args["atomStyle"])
                    .ok_or("Unknown atom style")?;
                let others = (0..3).filter(|i| *i != to).collect::<Vec<_>>();
                let moved = intersect(
                    &target.selection,
                    &union(&sels[others[0]], &sels[others[1]]),
                );
                write(
                    context,
                    object,
                    atom_ids[to],
                    &union(&sels[to], &moved),
                    &mut actions,
                )?;
                for i in others {
                    write(
                        context,
                        object,
                        atom_ids[i],
                        &minus(&sels[i], &target.selection),
                        &mut actions,
                    )?;
                }
            } else if show {
                let next = union(
                    &sels[0],
                    &minus(&target.selection, &union(&sels[1], &sels[2])),
                );
                write(context, object, atom_ids[0], &next, &mut actions)?;
            } else {
                for (id, sel) in atom_ids.iter().zip(sels) {
                    write(
                        context,
                        object,
                        *id,
                        &minus(&sel, &target.selection),
                        &mut actions,
                    )?;
                }
            }
        }
        if parts.contains("cartoons") {
            for &(name, _, _, scope) in &KINDS[3..] {
                let id = ids[name];
                let now = read(context, object, id);
                let next = if show {
                    union(&now, &intersect(&target.selection, scope))
                } else {
                    minus(&now, &target.selection)
                };
                write(context, object, id, &next, &mut actions)?;
            }
        }
    }
    Ok(actions)
}
