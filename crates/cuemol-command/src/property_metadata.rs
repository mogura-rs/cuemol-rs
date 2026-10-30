use super::*;
use cuemol_desktop_core::property_catalog::{
    map_renderer_attr_property, renderer_attr_properties, renderer_common_or_child_property,
    renderer_property_allowed, renderer_selection_readonly,
};

fn scalar_type(key: &str, value: &str) -> &'static str {
    match key {
        "sel" | "selection" | "anchor_sel" => "selection",
        "center" | "viewcenter" | "up_vec" | "right_vec" | "forward_vec" => "vector",
        "rotation" | "rotQuat" => "quaternion",
        "xformMat" => "matrix",
        "bgcolor" | "defaultcolor" | "egcolor" | "color" => "color",
        "uid" | "scene_uid" | "ui_order" | "width" | "height" | "aaJitterLevel" | "aoSlices"
        | "aoSteps" | "activeObjID" | "activeRendID" | "activeViewID" => "integer",
        "name" | "style" | "material" | "group" | "egroup" | "src" | "srctype" | "alt_src"
        | "type_name" | "onload" | "icc_filename" | "basePath" | "obj_uids" | "view_uids" => {
            "string"
        }
        _ if matches!(value, "true" | "false") => "boolean",
        _ if value.parse::<f64>().is_ok() => "real",
        _ => "string",
    }
}

fn category(key: &str) -> &str {
    if let Some((child, _)) = key.split_once('.') {
        return child;
    }
    if matches!(
        key,
        "name" | "visible" | "locked" | "sel" | "group" | "style" | "ui_order" | "ui_collapsed"
    ) {
        "Common"
    } else if key.starts_with("ao") {
        "Ambient occlusion"
    } else if key.starts_with("aa") {
        "Anti-aliasing"
    } else if key.starts_with("eg")
        || matches!(key, "alpha" | "material" | "defaultcolor" | "color")
    {
        "Appearance"
    } else if matches!(
        key,
        "src" | "alt_src" | "srctype" | "uid" | "scene_uid" | "type_name" | "modified"
    ) {
        "Identity"
    } else {
        "Geometry"
    }
}

pub(super) fn rows(
    state: &DesktopAppState,
    camera: &Camera,
    kind: &str,
    id: Option<Uid>,
) -> Result<Vec<Value>> {
    let sid = scene(state)?;
    let values = node_property_values(state, camera, kind, id)?;
    let owner = if matches!(kind, "renderer" | "rendGroup") {
        Some(renderer_owner(
            state,
            id.ok_or_else(|| CueMolError::invalid_argument("nodeId is required"))?,
        )?)
    } else {
        None
    };
    let renderer = owner.and_then(|owner| state.scene_manager().object(owner)?.renderer(id?));
    let auxiliary_explicit = if renderer.is_none() {
        owner
            .zip(id)
            .map(|(owner, id)| state.auxiliary_renderer_explicit_properties(owner, id))
            .transpose()?
    } else {
        None
    };
    let renderer_type = renderer
        .map(|r| r.renderer_type())
        .unwrap_or_else(|| values.get("type_name").map(String::as_str).unwrap_or(""))
        .to_string();
    let current_scene = state.scene_manager().scene(sid).unwrap();
    let object = if kind == "object" {
        id.and_then(|id| state.scene_manager().object(id))
    } else {
        None
    };
    let specs = renderer_attr_properties(&renderer_type)
        .into_iter()
        .map(|p| (p.key, p))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::with_capacity(values.len());
    for (key, value) in values {
        let spec = renderer_common_or_child_property(&renderer_type, &key)
            .or_else(|| specs.get(key.as_str()).copied())
            .or_else(|| map_renderer_attr_property(&renderer_type, &key));
        let mut default = spec.map(|p| p.default.to_string());
        let mut ty = spec
            .map(|p| p.kind.type_name())
            .unwrap_or_else(|| scalar_type(&key, &value));
        let mut allowed = renderer_property_allowed(&renderer_type, &key);
        if allowed.is_empty() {
            allowed = property_allowed(&key);
        }
        if !allowed.is_empty() {
            ty = "enum";
        }
        if kind == "object" {
            default = match key.as_str() {
                "visible" => Some("true".into()),
                "locked" | "ui_collapsed" => Some("false".into()),
                "defaultcolor" => Some("#ffffff".into()),
                "xformMat" => Some(Matrix4::IDENTITY.to_script_string()),
                "map_type" => Some("auto".into()),
                _ => None,
            };
        } else if kind == "scene" {
            default = VIEW_EFFECT_DEFAULTS
                .iter()
                .find_map(|(k, v)| (*k == key).then(|| v.to_string()));
            if key == "bgcolor" {
                default = Some("black".into());
            }
        } else if kind == "view" {
            default = None;
        }
        if matches!(
            key.as_str(),
            "name" | "sel" | "selection" | "style" | "ui_order"
        ) {
            default = None;
        }
        if map_renderer_attr_property(&renderer_type, &key).is_some()
            && matches!(key.as_str(), "level" | "defaultcolor")
        {
            default = None;
        }
        let readonly = matches!(
            key.as_str(),
            "uid"
                | "scene_uid"
                | "type_name"
                | "src"
                | "alt_src"
                | "srctype"
                | "modified"
                | "map_type_resolved"
        ) || kind == "scene"
            && matches!(key.as_str(), "name" | "basePath" | "obj_uids" | "view_uids")
            || spec.is_some_and(|property| !property.writable)
            || matches!(kind, "renderer" | "rendGroup")
                && (key == "style"
                    || cuemol_desktop_core::auxiliary_property_readonly(&key)
                    || matches!(key.as_str(), "sel" | "selection")
                        && renderer_selection_readonly(&renderer_type))
            || kind == "view"
                && matches!(
                    key.as_str(),
                    "width"
                        | "height"
                        | "hasHWStereo"
                        | "up_vec"
                        | "right_vec"
                        | "forward_vec"
                        | "style"
                );
        let hasdefault = default.is_some() && !readonly;
        let isdefault = hasdefault
            && if let Some(renderer) = renderer {
                if let Some((child, attr)) = key.split_once('.') {
                    renderer.qsc_child_is_default(child, attr)
                } else {
                    !renderer.qsc_attrs().contains_key(&key)
                }
            } else if let Some(explicit) = &auxiliary_explicit {
                !explicit.contains(key.as_str())
            } else if kind == "scene" {
                !current_scene.scene_attributes().contains_key(&key)
            } else if let Some(object) = object {
                !object.qsc_extra_attrs().contains_key(&key)
                    && default.as_deref() == Some(value.as_str())
            } else {
                default.as_deref() == Some(value.as_str())
            };
        let mut row = json!({"key":key, "value":value, "type":ty, "readonly":readonly,
            "hasdefault":hasdefault, "isdefault":isdefault, "depth":usize::from(key.contains('.')), "category":category(&key)});
        if let Some(default) = default {
            row["defaultValue"] = json!(default);
        }
        if !allowed.is_empty() {
            row["allowed"] = json!(allowed);
        }
        rows.push(row);
    }
    let colors = state.active_style_colors(
        rows.iter()
            .filter(|row| row["type"] == "color")
            .filter_map(|row| row["value"].as_str()),
    );
    for (row, color) in rows
        .iter_mut()
        .filter(|row| row["type"] == "color")
        .zip(colors)
    {
        if let Some(color) = color {
            row["rgba"] = json!([color.r, color.g, color.b, color.a]);
        }
    }
    Ok(rows)
}
