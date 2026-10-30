use super::pymol::{
    all, boolean, matching_nodes, native_property, number, selections, tool, MoleculeSelection,
};
use super::{nodes, pymol_label, pymol_settings, Action, Session};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use serde_json::json;

pub(super) fn atom_count(
    state: &DesktopAppState,
    item: &MoleculeSelection,
) -> Result<usize, String> {
    let selector = state
        .parse_scene_selector(
            state.active_scene().ok_or("No active scene")?,
            &item.expression,
        )
        .map_err(|e| e.to_string())?;
    Ok(state
        .scene_manager()
        .object(item.id)
        .ok_or("Molecule is no longer available")?
        .mol()
        .select_indices(&selector)
        .len())
}

fn first_selection(
    state: &DesktopAppState,
    source: &str,
    actions: &mut Vec<Action>,
) -> Result<MoleculeSelection, String> {
    let (mut items, _) = selections(state, source)?;
    if items.len() > 1 {
        actions.push(Action::Output(format!(
            "Using {} only; name a molecule to choose another.",
            items[0].name
        )));
    }
    if items.is_empty() {
        return Err("No molecule matches the selection".into());
    }
    Ok(items.remove(0))
}

pub(super) fn measure<'a>(
    state: &DesktopAppState,
    name: &str,
    arg: &dyn Fn(&str) -> &'a str,
) -> Result<Vec<Action>, String> {
    let count = match name {
        "distance" => 2,
        "angle" => 3,
        _ => 4,
    };
    let mut actions = Vec::new();
    let label = if arg("name").trim().is_empty() {
        "measure"
    } else {
        arg("name").trim()
    };
    let mut per_point = Vec::new();
    let mut named = std::collections::BTreeSet::new();
    for point in 1..=count {
        let source = arg(&format!("selection{point}"));
        if source.is_empty() {
            return Err(format!("{name} requires {count} selections"));
        }
        let (items, explicit) = selections(state, source)?;
        if explicit {
            named.extend(items.iter().map(|item| item.id));
        }
        per_point.push(items);
    }
    if name == "distance" {
        let mode = arg("mode").trim();
        if !["", "0", "2", "3", "4"].contains(&mode) {
            return Err("Distance modes are 0, 2, 3 and 4".into());
        }
        let cutoff = if arg("cutoff").is_empty() {
            None
        } else {
            let value = number(arg("cutoff"))?;
            if value <= 0. {
                return Err("Distance cutoff must be positive".into());
            }
            Some(value)
        };
        if cutoff.is_some() || matches!(mode, "0" | "2" | "3") {
            let one = first_selection(state, arg("selection1"), &mut actions)?;
            let two = first_selection(state, arg("selection2"), &mut actions)?;
            actions.push(tool("analyze_interactions", json!({"objId":one.id.get(),"selection":one.expression,"obj2Id":if one.id == two.id {None} else {Some(two.id.get())},"selection2":two.expression,"minDist":0.,"maxDist":cutoff.unwrap_or(if mode == "2" {3.6} else {4.}),"includeCarbon":mode != "2","maxLabels":100,"labelSet":label}))?);
            return Ok(actions);
        }
    }
    if named.len() > 1 {
        return Err(format!(
            "{name} between selection centroids requires one molecule"
        ));
    }
    let chosen = named.first().copied().unwrap_or(per_point[0][0].id);
    if named.is_empty() && per_point[0].len() > 1 {
        actions.push(Action::Output(format!(
            "Measuring {} only; name a molecule to choose another.",
            per_point[0][0].name
        )));
    }
    let expressions = per_point
        .iter()
        .map(|items| {
            items
                .iter()
                .find(|item| item.id == chosen)
                .map(|item| item.expression.as_str())
                .ok_or_else(|| format!("{name} between selection centroids requires one molecule"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    actions.push(tool(
        "measure_selections",
        json!({"objId":chosen.get(),"selections":expressions.join(";"),"label":label}),
    )?);
    Ok(actions)
}

pub(super) fn fit<'a>(
    state: &DesktopAppState,
    name: &str,
    arg: &dyn Fn(&str) -> &'a str,
) -> Result<Vec<Action>, String> {
    let mut actions = Vec::new();
    let moving = first_selection(state, arg("mobile"), &mut actions)?;
    let reference = first_selection(state, arg("target"), &mut actions)?;
    if moving.id == reference.id {
        return Err("The moving and reference molecules must differ".into());
    }
    if name == "pair_fit" {
        let a = atom_count(state, &moving)?;
        let b = atom_count(state, &reference)?;
        if a == 0 || b == 0 {
            return Err("A fit selection matched no atoms".into());
        }
        if a != b {
            return Err(format!(
                "pair_fit requires equal atom counts ({a} versus {b})"
            ));
        }
    }
    actions.push(tool("superpose", json!({"movId":moving.id.get(),"refId":reference.id.get(),"movSel":moving.expression,"refSel":reference.expression,"algo":if name == "pair_fit" {"LSQ"} else {"USalign"},"transform":name == "pair_fit" || boolean(arg("transform"))?,"keepAsProperty":false}))?);
    Ok(actions)
}

pub(super) fn label(
    session: &Session,
    state: &DesktopAppState,
    source: &str,
    expression: &str,
) -> Result<Vec<Action>, String> {
    let format = pymol_label::translate(expression)?;
    let (items, _) = selections(state, source)?;
    let mut targets = Vec::new();
    let mut total = 0;
    for item in items {
        let count = atom_count(state, &item)?;
        if count > 0 {
            total += count;
            targets.push(item);
        }
    }
    if total == 0 {
        return Err("The label selection matched no atoms".into());
    }
    if !format.is_empty() && total > 2000 {
        return Err(format!(
            "{total} atoms selected; label at most 2000 at once"
        ));
    }
    let mut actions = Vec::new();
    for item in targets {
        let existing = state
            .renderer_summaries(item.id)
            .into_iter()
            .find(|renderer| renderer.name == "pym:labels");
        if !format.is_empty() {
            if let Some(renderer) = &existing {
                actions.push(tool(
                    "set_visible",
                    json!({"nodeId":renderer.uid.get(),"nodeType":"renderer","visible":true}),
                )?);
            }
        }
        actions.push(tool("set_atom_labels", json!({"objId":item.id.get(),"selection":item.expression,"format":if format.is_empty() {None} else {Some(&format)},"rendererName":"pym:labels"}))?);
        if existing.is_none() && !format.is_empty() {
            actions.extend(pymol_settings::remembered(
                session,
                state,
                "labels",
                &format!("{}/pym:labels", item.name),
            ));
        }
    }
    Ok(actions)
}

pub(super) fn map<'a>(
    state: &DesktopAppState,
    name: &str,
    arg: &dyn Fn(&str) -> &'a str,
) -> Result<Vec<Action>, String> {
    let inventory = nodes(state)?;
    let label = arg("name").trim();
    if label.is_empty() {
        return Err("A map renderer name is required".into());
    }
    let mut actions = Vec::new();
    if name == "isolevel" {
        let renderers = inventory
            .iter()
            .filter(|node| {
                node.kind != "object"
                    && ["contour", "isosurf", "gpu_mapmesh", "gpu_mapvol"]
                        .contains(&node.class.as_str())
                    && node.name == label
            })
            .collect::<Vec<_>>();
        if renderers.is_empty() {
            return Err(format!("No map renderer named {label}"));
        }
        if boolean(arg("query"))? {
            for renderer in renderers {
                let props = crate::properties::node_property_values(
                    state,
                    &cuemol_qsys::Camera::new(),
                    "renderer",
                    Some(Uid::new(renderer.id)),
                )
                .map_err(|e| e.to_string())?;
                actions.push(Action::Output(format!(
                    "isolevel: {} at {}",
                    renderer.path,
                    props
                        .get("siglevel")
                        .map(String::as_str)
                        .unwrap_or("unknown")
                )));
            }
        } else {
            let level = number(arg("level"))?.to_string();
            for renderer in renderers {
                actions.push(native_property(&renderer.path, "siglevel", &level));
            }
        }
        return Ok(actions);
    }
    let level = number(arg("level"))?.to_string();
    let kind = if name == "isomesh" {
        "contour"
    } else {
        "isosurf"
    };
    let map = matching_nodes(state, arg("map"))?
        .into_iter()
        .find(|node| {
            node.kind == "object" && matches!(node.class.as_str(), "DensityMap" | "ElePotMap")
        })
        .ok_or("The requested object is not a density map")?;
    let carve = if arg("carve").is_empty() {
        None
    } else {
        Some(number(arg("carve"))?)
    };
    let buffer = if arg("buffer").is_empty() || number(arg("buffer"))? == 0. {
        None
    } else {
        Some(number(arg("buffer"))?)
    };
    let range = carve.or(buffer);
    let boundary = if arg("selection").is_empty() {
        None
    } else {
        Some(first_selection(state, arg("selection"), &mut actions)?)
    };
    if carve.is_some() && buffer.is_some() {
        actions.push(Action::Output(
            "buffer is ignored because carve supplies the boundary radius".into(),
        ));
    }
    if range.is_some() && boundary.is_none() {
        actions.push(Action::Output(
            "carve and buffer need a molecular selection".into(),
        ));
    }
    if boundary.is_some() && carve.is_none() {
        actions.push(Action::Output(
            "The map region is also carved around the selected atoms.".into(),
        ));
    }
    let path = format!("{}/{label}", map.name);
    let existing = inventory
        .iter()
        .find(|node| node.kind != "object" && node.path == path && node.class == kind);
    if existing.is_none() {
        actions.push(tool(
            "create_renderer",
            json!({"objId":map.id,"rendererType":kind,"name":label,"centerView":false}),
        )?);
        if boundary.is_none() && map.class == "DensityMap" {
            actions.push(Action::Native(format!("recenter_map {}", json!(path))));
        }
    }
    actions.push(native_property(&path, "siglevel", &level));
    if let Some(item) = boundary {
        actions.push(native_property(&path, "bndry_molname", &item.name));
        actions.push(native_property(&path, "bndry_sel", &item.expression));
        if let Some(range) = range {
            actions.push(native_property(&path, "bndry_rng", &range.to_string()));
        }
    }
    Ok(actions)
}

pub(super) fn spectrum<'a>(
    state: &DesktopAppState,
    arg: &dyn Fn(&str) -> &'a str,
) -> Result<Vec<Action>, String> {
    let expr = arg("expression").to_ascii_lowercase();
    let palette = arg("palette").to_ascii_lowercase();
    let mut properties = Vec::<(&str, String)>::new();
    let kind = match expr.as_str() {
        "b" | "q" => {
            let colors = palette
                .split(|ch: char| ch == '_' || ch.is_whitespace())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if colors.len() != 2
                || colors
                    .iter()
                    .any(|name| !pymol_settings::colors().contains_key(*name))
            {
                return Err(
                    "B-factor and occupancy spectra require two built-in colors, such as blue_red"
                        .into(),
                );
            }
            properties.extend([
                ("mode", if expr == "b" { "bfac" } else { "occ" }.into()),
                ("lowcol", pymol_settings::color(state, colors[0])?),
                ("highcol", pymol_settings::color(state, colors[1])?),
            ]);
            let fixed = !arg("minimum").is_empty() || !arg("maximum").is_empty();
            properties.push(("auto", if fixed { "none" } else { "mol" }.into()));
            for (key, prop) in [("minimum", "lowpar"), ("maximum", "highpar")] {
                if !arg(key).is_empty() {
                    properties.push((prop, number(arg(key))?.to_string()));
                }
            }
            "paint-type-bfac"
        }
        "count" | "resi" => {
            if !["rainbow", "rainbow_rev"].contains(&palette.as_str()) {
                return Err("Residue spectra use rainbow or rainbow_rev".into());
            }
            properties.extend([
                ("incr_mode", "resid".into()),
                (
                    "start_hue",
                    if palette == "rainbow" { "240" } else { "0" }.into(),
                ),
                (
                    "end_hue",
                    if palette == "rainbow" { "0" } else { "240" }.into(),
                ),
            ]);
            "paint-type-rainbow"
        }
        _ => return Err("Spectrum expressions are b, q, count and resi".into()),
    };
    let objects = matching_nodes(
        state,
        if all(arg("selection")) {
            "all"
        } else {
            arg("selection")
        },
    )?
    .into_iter()
    .filter(|node| node.kind == "object" && node.class == "MolCoord")
    .collect::<Vec<_>>();
    if objects.is_empty() {
        return Err("Spectrum colors whole representations; give a molecule name or all".into());
    }
    let mut actions = Vec::new();
    if matches!(expr.as_str(), "count" | "resi")
        && (!arg("minimum").is_empty() || !arg("maximum").is_empty())
    {
        actions.push(Action::Output(
            "Spectrum minimum and maximum apply only to b and q.".into(),
        ));
    }
    let mut painted = 0;
    for object in objects {
        for renderer in state
            .renderer_summaries(Uid::new(object.id))
            .iter()
            .filter(|renderer| renderer.name.starts_with("pym:"))
        {
            actions.push(tool(
                "set_renderer_coloring",
                json!({"nodeId":renderer.uid.get(),"nodeType":"renderer","mode":kind}),
            )?);
            for (prop, value) in &properties {
                actions.push(tool("set_coloring_prop", json!({"nodeId":renderer.uid.get(),"nodeType":"renderer","prop":prop,"value":value}))?);
            }
            painted += 1;
        }
    }
    if painted == 0 {
        return Err("Show a representation before coloring it".into());
    }
    Ok(actions)
}
