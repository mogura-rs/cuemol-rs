use super::{
    display,
    execute::{tool, Context, Deferred, Target},
};
use crate::console::Action;
use cuemol_core::Uid;
use serde_json::json;
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, String>;
const PREFIX: &str = "chx:surface:";
const ENCLOSE: &str = "chx:surface:enclose";
const POLYMER: &str = "(protein or nucleic)";

pub(super) fn surfaces(context: &Context<'_>, object: Uid) -> Vec<(Uid, Option<String>)> {
    display::owned(context, object)
        .into_iter()
        .filter_map(|(name, id)| {
            name.strip_prefix(PREFIX).map(|chain| {
                (
                    id,
                    if name == ENCLOSE {
                        None
                    } else {
                        Some(chain.into())
                    },
                )
            })
        })
        .collect()
}
pub(super) fn chains(context: &Context<'_>, target: &Target) -> Result<BTreeSet<String>> {
    let object = context
        .state
        .scene_manager()
        .object(target.model.id)
        .ok_or("No molecule")?;
    Ok(context
        .indices(target)?
        .into_iter()
        .map(|i| object.mol().atoms()[i].chain_id.clone())
        .collect())
}
pub(super) fn show_selection(context: &Context<'_>, object: Uid, id: Uid) -> String {
    context
        .state
        .scene_manager()
        .object(object)
        .and_then(|o| o.renderer(id))
        .and_then(|r| r.qsc_attr("showsel"))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("*")
        .into()
}
fn write(
    context: &Context<'_>,
    target: &Target,
    id: Uid,
    text: &str,
    out: &mut Vec<Action>,
) -> Result<()> {
    let value = crate::selection::simplify_expression(text);
    if show_selection(context, target.model.id, id) != value {
        out.push(tool(
            "set_node_prop",
            json!({"nodeType":"renderer","nodeId":id.get(),"prop":"showsel","value":value}),
        )?);
    }
    Ok(())
}
pub(super) fn ensure(
    context: &Context<'_>,
    targets: &[Target],
    enclose: bool,
) -> Result<Vec<Action>> {
    let mut out = Vec::new();
    for target in targets {
        let have = display::owned(context, target.model.id);
        let needed = if enclose {
            vec![(ENCLOSE.into(), target.selection.clone())]
        } else {
            let polymer = Target {
                selection: display::intersect(&target.selection, POLYMER),
                ..target.clone()
            };
            chains(context, &polymer)?
                .into_iter()
                .map(|c| (format!("{PREFIX}{c}"), format!("(chain {c}) and {POLYMER}")))
                .collect()
        };
        for (name, selection) in needed {
            if have.contains_key(&name) {
                continue;
            }
            out.push(tool("create_renderer",json!({"objId":target.model.id.get(),"rendererType":"dsurface","name":name,"selection":selection,"centerView":false}))?);
            out.push(Action::ChimeraX(Box::new(Deferred::Properties {
                object: target.model.id,
                name,
                props: vec![("showsel".into(), "none".into())],
            })));
        }
    }
    Ok(out)
}
pub(super) fn show(
    context: &Context<'_>,
    target: &Target,
    enclose: bool,
    out: &mut Vec<Action>,
) -> Result<Vec<Uid>> {
    let mut ids = Vec::new();
    if enclose {
        if let Some((id, _)) = surfaces(context, target.model.id)
            .into_iter()
            .find(|s| s.1.is_none())
        {
            display::write(context, target.model.id, id, &target.selection, out)?;
            write(context, target, id, "*", out)?;
            ids.push(id);
        }
    } else {
        let polymer = Target {
            selection: display::intersect(&target.selection, POLYMER),
            ..target.clone()
        };
        let hit = chains(context, &polymer)?;
        for (id, chain) in surfaces(context, target.model.id) {
            let Some(chain) = chain.filter(|c| hit.contains(c)) else {
                continue;
            };
            let selection = display::union(
                &show_selection(context, target.model.id, id),
                &display::intersect(&target.selection, &format!("chain {chain}")),
            );
            write(context, target, id, &selection, out)?;
            ids.push(id);
        }
    }
    Ok(ids)
}
pub(super) fn hide(context: &Context<'_>, target: &Target, out: &mut Vec<Action>) -> Result<()> {
    for (id, _) in surfaces(context, target.model.id) {
        write(
            context,
            target,
            id,
            &display::minus(
                &show_selection(context, target.model.id, id),
                &target.selection,
            ),
            out,
        )?;
    }
    Ok(())
}
pub(super) fn close(context: &Context<'_>, target: &Target, out: &mut Vec<Action>) -> Result<()> {
    let hit = chains(context, target)?;
    for (id, chain) in surfaces(context, target.model.id) {
        if chain.is_none_or(|c| hit.contains(&c)) {
            out.push(tool(
                "delete_node",
                json!({"nodeId":id.get(),"nodeType":"renderer"}),
            )?);
        }
    }
    Ok(())
}
