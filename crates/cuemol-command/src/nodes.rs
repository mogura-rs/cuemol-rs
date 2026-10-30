use crate::commands::{renderer_owner, scene, text, uid};
use cuemol_core::{CueMolError, Result, Uid};
use cuemol_desktop_core::DesktopAppState;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn edit_node(state: &mut DesktopAppState, name: &str, args: &Value) -> Result<Value> {
    let id = uid(args, "nodeId")?;
    let kind = text(args, "nodeType")?;
    let visible = args["visible"] == true;
    let label = args["name"].as_str().unwrap_or("");
    if kind == "object" {
        if state.density_map_summary(id).is_some() {
            match name {
                "set_visible" => state.set_density_map_visible(id, visible)?,
                "delete_node" => {
                    state.destroy_density_map(id)?;
                }
                _ => state.rename_density_map(id, label)?,
            }
        } else if state.surface_summary(id).is_some() {
            match name {
                "set_visible" => state.set_surface_visible(id, visible)?,
                "delete_node" => {
                    state.destroy_surface(id)?;
                }
                _ => state.rename_surface(id, label)?,
            }
        } else if state.lw_object_summary(id).is_some() {
            match name {
                "set_visible" => state.set_lw_object_visible(id, visible)?,
                "delete_node" => {
                    state.destroy_lw_object(id)?;
                }
                _ => state.rename_lw_object(id, label)?,
            }
        } else {
            match name {
                "set_visible" => state.set_object_visible(id, visible)?,
                "delete_node" => {
                    if !state.destroy_object(id)? {
                        return Err(CueMolError::not_found("object"));
                    }
                }
                _ => {
                    let object = state
                        .scene_manager_mut()
                        .object_mut(id)
                        .ok_or_else(|| CueMolError::not_found("object"))?;
                    object.set_name(label);
                    object.set_modified(true);
                }
            }
        }
    } else {
        let owner = renderer_owner(state, id)?;
        if kind == "rendGroup" {
            match name {
                "set_visible" => state.set_renderer_group_visible(owner, id, visible)?,
                "delete_node" => {
                    state.destroy_renderer_group_in_object(owner, id)?;
                }
                _ => state.rename_renderer_group(owner, id, label)?,
            }
        } else if state.density_map_summary(owner).is_some() {
            match name {
                "set_visible" => state.set_density_renderer_visible(owner, id, visible)?,
                "delete_node" => {
                    state.destroy_density_renderer(owner, id)?;
                }
                _ => state.rename_density_renderer(owner, id, label)?,
            }
        } else if state.surface_summary(owner).is_some() {
            match name {
                "set_visible" => state.set_surface_renderer_visible(owner, id, visible)?,
                "delete_node" => {
                    state.destroy_surface_renderer(owner, id)?;
                }
                _ => state.rename_surface_renderer(owner, id, label)?,
            }
        } else if state.lw_object_summary(owner).is_some() {
            match name {
                "set_visible" => state.set_lw_renderer_visible(owner, id, visible)?,
                "delete_node" => {
                    state.destroy_lw_renderer(owner, id)?;
                }
                _ => state.rename_lw_renderer(owner, id, label)?,
            }
        } else {
            match name {
                "set_visible" => state.set_renderer_visible(owner, id, visible)?,
                "delete_node" => {
                    state.destroy_renderer_in_object(owner, id)?;
                }
                _ => state
                    .scene_manager_mut()
                    .object_mut(owner)
                    .unwrap()
                    .renderer_mut(id)
                    .unwrap()
                    .set_name(label),
            }
        }
    }
    Ok(json!({"nodeId":id.get()}))
}

pub fn renderer_properties(state: &DesktopAppState, id: Uid) -> Result<BTreeMap<String, String>> {
    let owner = renderer_owner(state, id)?;
    if state.scene_manager().object(owner).is_none() {
        return state.auxiliary_renderer_properties(owner, id);
    }
    let r = state
        .scene_manager()
        .object(owner)
        .unwrap()
        .renderer(id)
        .unwrap();
    let mut result = state.molecule_renderer_properties(owner, id)?;
    for (k, v) in [
        ("name", r.name().to_string()),
        ("visible", r.is_visible().to_string()),
        ("locked", (r.qsc_attr("locked") == Some("true")).to_string()),
        ("sel", r.qsc_attr("sel").unwrap_or("").into()),
        ("style", r.style().unwrap_or("").into()),
        ("type_name", r.renderer_type().into()),
        ("uid", r.uid().get().to_string()),
        ("scene_uid", scene(state)?.get().to_string()),
        ("group", r.group().unwrap_or("").into()),
        ("ui_order", r.qsc_attr("ui_order").unwrap_or("0").into()),
        (
            "ui_collapsed",
            r.qsc_attr("ui_collapsed").unwrap_or("false").into(),
        ),
    ] {
        result.insert(k.into(), v);
    }
    Ok(result)
}

pub fn set_renderer_property(
    state: &mut DesktopAppState,
    id: Uid,
    key: &str,
    value: &str,
) -> Result<()> {
    let owner = renderer_owner(state, id)?;
    let boolean = || {
        value
            .parse::<bool>()
            .map_err(|_| CueMolError::invalid_argument("expected true or false"))
    };
    let number = || {
        value
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .ok_or_else(|| CueMolError::invalid_argument("expected finite number"))
    };
    if state.scene_manager().object(owner).is_none() {
        return state.set_auxiliary_renderer_property(owner, id, key, value);
    }
    let renderer_type = state
        .scene_manager()
        .object(owner)
        .unwrap()
        .renderer(id)
        .unwrap()
        .renderer_type();
    let allowed =
        cuemol_desktop_core::property_catalog::renderer_property_allowed(renderer_type, key);
    if !allowed.is_empty() && !allowed.contains(&value) {
        return Err(CueMolError::invalid_argument(format!(
            "invalid {key}: {value}"
        )));
    }
    if key == "group" {
        let object = state.scene_manager().object(owner).unwrap();
        let group = value.trim();
        if !group.is_empty()
            && (renderer_type == "*group"
                || !object.renderers().iter().any(|renderer| {
                    renderer.renderer_type() == "*group" && renderer.name() == group
                }))
        {
            return Err(CueMolError::invalid_argument(
                "group must name an existing group on the same object",
            ));
        }
        return state.set_workspace_renderer_group_name(owner, id, Some(group));
    }
    match key {
        "name" => {
            state
                .scene_manager_mut()
                .object_mut(owner)
                .unwrap()
                .renderer_mut(id)
                .unwrap()
                .set_name(value);
            Ok(())
        }
        "sel" => state.set_renderer_selection_text(owner, id, value),
        "visible" => state.set_renderer_visible(owner, id, boolean()?),
        "locked" => state.set_renderer_locked(owner, id, boolean()?),
        "style" => state.set_renderer_styles(owner, id, value),
        _ => {
            let renderer = state
                .scene_manager()
                .object(owner)
                .unwrap()
                .renderer(id)
                .unwrap();
            if let Some(prop) = cuemol_desktop_core::property_catalog::renderer_attr_properties(
                renderer.renderer_type(),
            )
            .iter()
            .find(|p| p.key == key)
            {
                use cuemol_desktop_core::property_catalog::RendererAttrKind;
                if !prop.writable {
                    return Err(CueMolError::invalid_argument("property is read-only"));
                }
                match prop.kind {
                    RendererAttrKind::Boolean => {
                        boolean()?;
                    }
                    RendererAttrKind::Integer => {
                        value
                            .parse::<i64>()
                            .map_err(|_| CueMolError::invalid_argument("expected integer"))?;
                    }
                    RendererAttrKind::Real => {
                        number()?;
                    }
                    RendererAttrKind::Color => {
                        cuemol_gfx::parse_color(value)?;
                    }
                    RendererAttrKind::Vector => {
                        cuemol_core::Vector4::from_script_string(value)?;
                    }
                    RendererAttrKind::Selection => {
                        cuemol_molstr::parse_selector(value)?;
                    }
                    _ => {}
                }
            }
            if let Some((child, attr)) = key.split_once('.') {
                state.set_renderer_child_property(owner, id, child, attr, value)
            } else {
                state.set_renderer_property(owner, id, key, value)
            }
        }
    }
}

pub fn move_target(args: &Value, from: usize, count: usize) -> Result<usize> {
    if count == 0 {
        return Err(CueMolError::not_found("list entry"));
    }
    if ["by", "to", "end"]
        .iter()
        .filter(|k| !args[**k].is_null())
        .count()
        != 1
    {
        return Err(CueMolError::invalid_argument(
            "give exactly one of by, to, or end",
        ));
    }
    if let Some(by) = args["by"].as_i64() {
        return Ok((from as i64 + by).clamp(0, count as i64 - 1) as usize);
    }
    if let Some(to) = args["to"].as_u64().filter(|n| *n > 0 && *n <= count as u64) {
        return Ok(to as usize - 1);
    }
    match args["end"].as_str() {
        Some("top") => Ok(0),
        Some("bottom") => Ok(count - 1),
        _ => Err(CueMolError::invalid_argument("invalid list destination")),
    }
}

pub fn extra_scene_objects(state: &DesktopAppState) -> Result<Vec<Value>> {
    let scene = scene(state)?;
    let mut objects = Vec::new();
    for map in state.density_map_summaries(scene) {
        objects.push(json!({"id":map.uid.get(),"name":map.name,"className":map.class_name,"visible":map.visible,"renderers":map.renderers.into_iter().map(|r|json!({"id":r.uid.get(),"name":r.name,"type":r.renderer_type,"visible":r.visible,"group":r.group})).collect::<Vec<_>>()}));
    }
    for surface in state.surface_summaries(scene) {
        objects.push(json!({"id":surface.uid.get(),"name":surface.name,"className":surface.class_name,"visible":surface.visible,"renderers":surface.renderers.into_iter().map(|r|json!({"id":r.uid.get(),"name":r.name,"type":r.renderer_type,"visible":r.visible,"group":r.group})).collect::<Vec<_>>()}));
    }
    for object in state.lw_object_summaries(scene) {
        objects.push(json!({"id":object.uid.get(),"name":object.name,"className":object.class_name,"visible":object.visible,"renderers":object.renderers.into_iter().map(|r|json!({"id":r.uid.get(),"name":r.name,"type":r.renderer_type,"visible":r.visible,"group":r.group})).collect::<Vec<_>>()}));
    }
    Ok(objects)
}

pub fn object_kind(
    state: &DesktopAppState,
    id: Uid,
) -> Result<cuemol_desktop_core::GuiWorkspaceObjectKind> {
    use cuemol_desktop_core::GuiWorkspaceObjectKind as K;
    if state.scene_manager().object(id).is_some() {
        Ok(K::Molecule)
    } else if state.density_map_summary(id).is_some() {
        Ok(K::DensityMap)
    } else if state.surface_summary(id).is_some() {
        Ok(K::Surface)
    } else if state.lw_object_summary(id).is_some() {
        Ok(K::Lightweight)
    } else {
        Err(CueMolError::not_found("object"))
    }
}
