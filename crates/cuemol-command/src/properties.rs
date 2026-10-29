use crate::{
    advanced::resolve_node,
    commands::{fit, renderer_owner, scene, text, uid},
    nodes,
};
use cuemol_core::{
    parse_ldom2_xml, write_ldom2_xml, CueMolError, LDom2Node, LDom2Tree, Matrix4, Quat, Result,
    Uid, Vector4,
};
use cuemol_desktop_core::{DesktopAppState, GuiWorkspaceObjectKind};
use cuemol_qsys::{Camera, View};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn call_tool(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: &Value,
) -> Result<Value> {
    match name {
        "list_node_props" | "set_node_prop" | "set_prop" | "get_prop" | "reset_prop" => {
            properties(state, camera, name, args)
        }
        "create_style_set" | "delete_style_set" | "set_style_readonly" | "load_style_file"
        | "save_style_file" | "define_color" | "define_selection" | "remove_color"
        | "remove_selection" | "remove_style" => styles(state, name, args),
        "focus_node" => {
            let id = uid(args, "nodeId")?;
            let object = if args["nodeType"] == "object" {
                id
            } else {
                renderer_owner(state, id)?
            };
            if state.density_map_summary(object).is_some() {
                let mut view = View::new(Uid::new(1), scene(state)?);
                view.set_camera(camera);
                view.set_view_size(800, 600);
                state.fit_density_map_view(object, &mut view)?;
                *camera = view.camera().clone();
            } else if state.scene_manager().object(object).is_some() {
                let selection = if id != object {
                    nodes::renderer_properties(state, id)?.get("sel").cloned()
                } else {
                    None
                };
                fit(state, camera, Some(object), selection.as_deref(), true)?;
            } else {
                let snapshot = state.workspace_object_render_snapshot(object)?;
                let (min, max) = snapshot
                    .bounds
                    .ok_or_else(|| CueMolError::not_found("object geometry"))?;
                let size = (max - min).length3d().max(2.);
                camera.set_center((min + max).scale(0.5));
                camera.set_zoom(size * 1.2);
                camera.set_cam_dist(size * 2.5);
                camera.set_slab_depth(size * 2.);
            }
            Ok(crate::view_json(camera))
        }
        "move_node" => move_node(state, args),
        "set_render_settings" => {
            let sid = scene(state)?;
            let mut settings = render_settings(state)?;
            let mut updates = Vec::new();
            for part in split_settings(text(args, "settings")?)?
                .into_iter()
                .map(str::trim)
                .filter(|p| !p.is_empty())
            {
                let (key, value) = part
                    .split_once('=')
                    .ok_or_else(|| CueMolError::invalid_argument("expected key=value"))?;
                let key = key.trim();
                let value = value.trim();
                let local_key = key
                    .strip_prefix("umbreon.")
                    .or_else(|| key.strip_prefix("umbreon_npr."))
                    .or_else(|| key.strip_prefix("povray."))
                    .unwrap_or(key);
                let schema = cuemol_qsys::SettingsClass::Render.property(key);
                if schema.is_none() && !RENDER_KEYS.contains(&local_key)
                    || (key != local_key
                        && matches!(
                            local_key,
                            "width" | "height" | "unit" | "dpi" | "backend" | "projection"
                        ))
                {
                    return Err(CueMolError::invalid_argument(format!(
                        "unknown render setting {key}"
                    )));
                }
                let parsed = serde_json::from_str::<Value>(value).unwrap_or_else(|_| json!(value));
                let valid = if let Some(property) = schema {
                    let text = parsed
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| parsed.to_string());
                    property.parse(&text).is_ok()
                        && match local_key {
                            "backend" => parsed
                                .as_str()
                                .is_some_and(|v| matches!(v, "" | "umbreon" | "umbreon_npr")),
                            "projection" => parsed
                                .as_str()
                                .is_some_and(|v| matches!(v, "perspective" | "orthographic")),
                            "unit" => parsed
                                .as_str()
                                .is_some_and(|v| matches!(v, "px" | "in" | "mm" | "cm")),
                            "width" | "height" | "dpi" => parsed.as_f64().is_some_and(|v| v > 0.0),
                            _ => true,
                        }
                } else {
                    match local_key {
                        "backend" => parsed
                            .as_str()
                            .is_some_and(|v| matches!(v, "" | "umbreon" | "umbreon_npr")),
                        "projection" => parsed
                            .as_str()
                            .is_some_and(|v| matches!(v, "perspective" | "orthographic")),
                        "unit" => parsed
                            .as_str()
                            .is_some_and(|v| matches!(v, "px" | "in" | "cm" | "mm")),
                        "aoGather" => parsed
                            .as_str()
                            .is_some_and(|v| matches!(v, "Per output pixel" | "Per shading hit")),
                        "aaMode" => parsed.as_i64().is_some_and(|v| (0..=1).contains(&v)),
                        "aaDepth" => parsed.as_i64().is_some_and(|v| (0..=128).contains(&v)),
                        "aoResDiv" => parsed.as_i64().is_some_and(|v| (-1..=128).contains(&v)),
                        "denoiser" => parsed.as_i64().is_some_and(|v| (0..=2).contains(&v)),
                        "denoise" => parsed.as_str().is_some_and(|v| {
                            matches!(v, "OIDN" | "Bilateral" | "A-trous" | "None")
                        }),
                        "giGroundColor" | "hatchInkColor" | "hatchPaperColor" | "hatchColoring"
                        | "hatchStyle" | "hatchLayersSpec" | "hatchToneSpec" => parsed.is_string(),
                        "transparent" | "transparentBg" | "perspective" | "useClipZ"
                        | "clipPlane" | "shadows" | "showEdgeLines" | "edgeLines" | "useGi"
                        | "useGI" | "giDenoise" | "aoEnabled" | "hatchCustomInk"
                        | "hatchCustomPaper" | "hatchDefaultEdges" | "aoMultiScale"
                        | "aoBentNormal" | "aoLowDiscrepancy" | "giSkyGradient"
                        | "perPixelBlend" | "contactEdges" => parsed.is_boolean(),
                        "width" | "height" | "dpi" | "hatchDensity" | "hatchWidthScale" => {
                            parsed.as_f64().is_some_and(|n| n.is_finite() && n > 0.)
                        }
                        _ => parsed.as_f64().is_some_and(f64::is_finite),
                    }
                };
                if !valid {
                    return Err(CueMolError::invalid_argument(format!(
                        "invalid value for render setting {key}"
                    )));
                }
                updates.push((key.to_string(), parsed.clone()));
                settings.insert(key.into(), parsed);
            }
            crate::commands::hatch_params(&settings, "umbreon_npr")?;
            crate::commands::rendering_params(&settings, "umbreon")?;
            crate::commands::rendering_params(&settings, "umbreon_npr")?;
            let mut typed = state
                .scene_manager()
                .scene(sid)
                .and_then(|scene| scene.app_data().get("render"))
                .cloned()
                .unwrap_or_default();
            let mut extensions = serde_json::Map::new();
            for (key, value) in &settings {
                if !store_render_setting(&mut typed, key, value)? {
                    extensions.insert(key.clone(), value.clone());
                }
            }
            for (key, value) in &updates {
                store_render_setting(&mut typed, key, value)?;
            }
            let target = state.scene_manager_mut().scene_mut(sid).unwrap();
            target.app_data_mut().insert("render", typed)?;
            if extensions.is_empty() {
                target.remove_scene_attribute("render_settings");
            } else {
                target.set_scene_attribute("render_settings", json!(extensions).to_string());
            }
            Ok(json!({"settings":render_settings(state)?}))
        }
        "export_image" | "export_scene" => export(state, camera, name, args),
        "superpose" => {
            let algorithm = args["algo"].as_str().unwrap_or("USalign");
            let matrix = state.superpose_objects_with_pivot(
                uid(args, "refId")?,
                args["refSel"]
                    .as_str()
                    .unwrap_or(if algorithm == "LSQ" { "name CA" } else { "*" }),
                uid(args, "movId")?,
                args["movSel"]
                    .as_str()
                    .unwrap_or(if algorithm == "LSQ" { "name CA" } else { "*" }),
                algorithm,
                args["keepAsProperty"] == true,
                args["pivot"].as_str().unwrap_or("auto"),
            )?;
            Ok(json!({"matrix":matrix,"algorithm":algorithm}))
        }
        _ => crate::animation::call_tool(state, camera, name, args),
    }
}

fn split_settings(input: &str) -> Result<Vec<&str>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && c == '\\' {
            escaped = true;
            continue;
        }
        if c == '"' {
            quoted = !quoted;
        }
        if c == ';' && !quoted {
            parts.push(&input[start..i]);
            start = i + 1;
        }
    }
    if quoted {
        return Err(CueMolError::invalid_argument(
            "unterminated render setting string",
        ));
    }
    parts.push(&input[start..]);
    Ok(parts)
}

const RENDER_KEYS: &[&str] = &[
    "aaMode",
    "aaDepth",
    "aoResDiv",
    "aoDiffuseFactor",
    "aoMultiScale",
    "aoBentNormal",
    "aoLowDiscrepancy",
    "aoFalloffPower",
    "aoGather",
    "giSkyGradient",
    "giGroundColor",
    "giBounces",
    "perPixelBlend",
    "contactEdges",
    "outlineFarDepth",
    "hatchStyle",
    "hatchLayersSpec",
    "hatchToneSpec",
    "hatchDefaultEdges",
    "hatchToneStrength",
    "hatchToneCurve",
    "lightIntensity",
    "flashFraction",
    "ambientFraction",
    "backend",
    "projection",
    "transparentBg",
    "clipPlane",
    "edgeLines",
    "useGI",
    "denoise",
    "aoEnabled",
    "hatchDensity",
    "hatchWidthScale",
    "hatchInkColor",
    "hatchPaperColor",
    "hatchColoring",
    "hatchCustomInk",
    "hatchCustomPaper",
    "width",
    "height",
    "unit",
    "dpi",
    "transparent",
    "perspective",
    "useClipZ",
    "supersample",
    "aoSamples",
    "aoDistance",
    "aoIntensity",
    "shadows",
    "shadowSamples",
    "lightRadius",
    "showEdgeLines",
    "creaseLimit",
    "edgeRise",
    "useGi",
    "giSamples",
    "giIntensity",
    "giEnvIntensity",
    "giDenoise",
    "denoiser",
];
fn canonical_render_setting(key: &str, value: &Value) -> (String, Value) {
    let (prefix, key) = key
        .rsplit_once('.')
        .map_or(("", key), |(prefix, key)| (prefix, key));
    let (key, value) = match key {
        "transparent" => ("transparentBg", value.clone()),
        "useClipZ" => ("clipPlane", value.clone()),
        "showEdgeLines" => ("edgeLines", value.clone()),
        "useGi" => ("useGI", value.clone()),
        "perspective" => (
            "projection",
            json!(if value == true {
                "perspective"
            } else {
                "orthographic"
            }),
        ),
        "giDenoise" => (
            "denoise",
            json!(if value == true { "OIDN" } else { "None" }),
        ),
        "denoiser" => (
            "denoise",
            json!(match value.as_i64() {
                Some(1) => "A-trous",
                Some(2) => "None",
                _ => "OIDN",
            }),
        ),
        _ => (key, value.clone()),
    };
    (
        if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{key}")
        },
        value,
    )
}

fn store_render_setting(
    settings: &mut cuemol_qsys::RenderSettings,
    key: &str,
    value: &Value,
) -> Result<bool> {
    let (key, value) = canonical_render_setting(key, value);
    let text = value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string());
    if settings.class().property(&key).is_some() {
        settings.set(&key, &text)?;
        return Ok(true);
    }
    if !key.contains('.') {
        let mut found = false;
        for prefix in ["umbreon", "umbreon_npr"] {
            let path = format!("{prefix}.{key}");
            if settings.class().property(&path).is_some() {
                settings.set(&path, &text)?;
                found = true;
            }
        }
        return Ok(found);
    }
    Ok(false)
}

pub fn render_settings(state: &DesktopAppState) -> Result<serde_json::Map<String, Value>> {
    let scene = state.scene_manager().scene(scene(state)?).unwrap();
    let legacy = scene
        .scene_attribute("render_settings")
        .and_then(|v| serde_json::from_str::<Value>(v).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if scene.app_data().get("render").is_none() && legacy.is_empty() {
        return Ok(legacy);
    }
    let mut migrated = cuemol_qsys::RenderSettings::default();
    let mut result = serde_json::Map::new();
    for (key, value) in legacy {
        if !store_render_setting(&mut migrated, &key, &value)? {
            result.insert(key, value);
        }
    }
    let settings = scene.app_data().get("render").unwrap_or(&migrated);
    let mut add = |class: cuemol_qsys::SettingsClass, prefix: &str| {
        for property in class.properties() {
            let path = format!("{prefix}{}", property.name);
            let text = settings.get(&path).unwrap();
            let value = match property.kind {
                cuemol_qsys::SettingsKind::String => json!(text),
                _ => serde_json::from_str(text).unwrap(),
            };
            result.insert(path, value);
        }
    };
    add(cuemol_qsys::SettingsClass::Render, "");
    for (name, class) in cuemol_qsys::SettingsClass::Render.children() {
        add(*class, &format!("{name}."));
    }
    Ok(result)
}

const VIEW_EFFECT_DEFAULTS: &[(&str, &str)] = &[
    ("aoEnabled", "false"),
    ("aoRadius", "4"),
    ("aoIntensity", "2.2"),
    ("aoSlices", "9"),
    ("aoSteps", "3"),
    ("aoHalfRes", "false"),
    ("aa_method", "fxaa"),
    ("aaJitterLevel", "0"),
    ("aaSmaaThreshold", "0.05"),
];

fn validate_view_effect_property(key: &str, value: &str) -> Result<()> {
    let valid = match key {
        "aoEnabled" | "aoHalfRes" => value.parse::<bool>().is_ok(),
        "aoRadius" | "aoIntensity" => value
            .parse::<f64>()
            .is_ok_and(|n| n.is_finite() && n >= 0.0),
        "aaSmaaThreshold" => value
            .parse::<f64>()
            .is_ok_and(|n| n.is_finite() && (0.0..=1.0).contains(&n)),
        "aoSlices" | "aoSteps" => value.parse::<i32>().is_ok_and(|n| (1..=64).contains(&n)),
        "aaJitterLevel" => value.parse::<i32>().is_ok_and(|n| (0..=5).contains(&n)),
        "aa_method" => matches!(value, "none" | "fxaa" | "smaa"),
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(CueMolError::invalid_argument(format!(
            "invalid value for {key}: {value}"
        )))
    }
}

fn properties(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: &Value,
) -> Result<Value> {
    let (kind, id, prop) = if matches!(name, "get_prop" | "set_prop" | "reset_prop") {
        let path = text(args, "path")?;
        if let Some((node, prop)) = path.split_once('.') {
            if node == "view" {
                ("view".to_string(), None, prop.to_string())
            } else {
                let (id, object) = resolve_node(state, node)?;
                (
                    (if object { "object" } else { "renderer" }).into(),
                    Some(id),
                    prop.into(),
                )
            }
        } else {
            ("scene".into(), None, path.into())
        }
    } else {
        (
            text(args, "nodeType")?.into(),
            args["nodeId"].as_u64().map(Uid::new),
            args["prop"].as_str().unwrap_or("").into(),
        )
    };
    let sid = scene(state)?;
    if name == "set_node_prop" || name == "set_prop" {
        let value = text(args, "value")?;
        if property_readonly(state, &kind, id, &prop) {
            return Err(CueMolError::invalid_argument(format!(
                "{prop} is read-only"
            )));
        }
        match kind.as_str() {
            "renderer" | "rendGroup" => nodes::set_renderer_property(
                state,
                id.ok_or_else(|| CueMolError::invalid_argument("nodeId is required"))?,
                &prop,
                value,
            )?,
            "object" => set_object_property(
                state,
                id.ok_or_else(|| CueMolError::invalid_argument("nodeId is required"))?,
                &prop,
                value,
            )?,
            "view" => set_view_property(state, camera, &prop, value)?,
            "scene" => {
                validate_view_effect_property(&prop, value)?;
                if prop == "bgcolor" {
                    cuemol_gfx::parse_color(value)?;
                }
                let s = state.scene_manager_mut().scene_mut(sid).unwrap();
                if prop == "name" {
                    s.set_name(value);
                } else {
                    s.set_scene_attribute(prop.clone(), value);
                }
            }
            _ => return Err(CueMolError::invalid_argument("unknown node type")),
        }
    } else if name == "reset_prop" {
        match kind.as_str() {
            "renderer" | "rendGroup" => {
                let id = id.unwrap();
                let owner = renderer_owner(state, id)?;
                if state.scene_manager().object(owner).is_none() {
                    state.reset_auxiliary_renderer_property(owner, id, &prop)?;
                } else if prop == "*" {
                    let keys = state
                        .scene_manager()
                        .object(owner)
                        .and_then(|o| o.renderer(id))
                        .ok_or_else(|| {
                            CueMolError::invalid_argument("reset-all requires molecule renderer")
                        })?
                        .qsc_attrs()
                        .keys()
                        .filter(|k| {
                            !matches!(k.as_str(), "name" | "type" | "ui_order" | "style" | "group")
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    for key in keys {
                        state.reset_renderer_property(owner, id, &key)?;
                    }
                } else if let Some((child, key)) = prop.split_once('.') {
                    state.reset_renderer_child_property(owner, id, child, key)?;
                } else {
                    state.reset_renderer_property(owner, id, &prop)?;
                }
            }
            "scene" => {
                state
                    .scene_manager_mut()
                    .scene_mut(sid)
                    .unwrap()
                    .remove_scene_attribute(&prop);
            }
            "object" => reset_object_property(state, id.unwrap(), &prop)?,
            _ => {
                return Err(CueMolError::invalid_argument(
                    "this property has no default",
                ))
            }
        }
    }
    let mut values = match kind.as_str() {
        "renderer" | "rendGroup" => nodes::renderer_properties(
            state,
            id.ok_or_else(|| CueMolError::invalid_argument("nodeId is required"))?,
        )?,
        "object" => object_properties(
            state,
            id.ok_or_else(|| CueMolError::invalid_argument("nodeId is required"))?,
        )?,
        "scene" => {
            let s = state.scene_manager().scene(sid).unwrap();
            let mut v = s.scene_attributes().clone();
            v.insert("name".into(), s.name().into());
            v.entry("bgcolor".into()).or_insert_with(|| "black".into());
            for (key, value) in VIEW_EFFECT_DEFAULTS {
                v.entry((*key).into()).or_insert_with(|| (*value).into());
            }
            v
        }
        "view" => view_properties(state, camera),
        _ => return Err(CueMolError::invalid_argument("unknown node type")),
    };
    if name == "list_node_props" {
        Ok(json!({"properties":values.iter().map(|(key,value)| {
                let readonly = property_readonly(state, &kind, id, key);
                let allowed = property_allowed(key);
                let mut row = json!({"key":key,"value":value,"readonly":readonly});
                if !allowed.is_empty() { row["allowed"] = json!(allowed); }
                row
            }).collect::<Vec<_>>()}))
    } else {
        let value = if prop == "*" {
            Value::Null
        } else {
            json!(values
                .remove(&prop)
                .ok_or_else(|| CueMolError::not_found(format!("{kind} property {prop}")))?)
        };
        Ok(json!({"prop":prop,"value":value}))
    }
}

fn object_properties(state: &DesktopAppState, id: Uid) -> Result<BTreeMap<String, String>> {
    let mut p = BTreeMap::new();
    let (name, visible, locked) = if let Some(o) = state.scene_manager().object(id) {
        p.extend(o.qsc_extra_attrs().clone());
        p.insert("sel".into(), state.molecule_selection_text(id)?.into());
        p.insert("xformMat".into(), o.xform_matrix().to_script_string());
        (o.name().into(), o.is_visible(), o.is_ui_locked())
    } else if let Some(o) = state.density_map_summary(id) {
        (o.name, o.visible, o.locked)
    } else if let Some(o) = state.surface_summary(id) {
        (o.name, o.visible, o.locked)
    } else if let Some(o) = state.lw_object_summary(id) {
        (o.name, o.visible, o.locked)
    } else {
        return Err(CueMolError::not_found("object"));
    };
    p.insert(
        "xformMat".into(),
        state.workspace_object_transform(id)?.to_script_string(),
    );
    p.insert("name".into(), name);
    p.insert("visible".into(), visible.to_string());
    p.insert("locked".into(), locked.to_string());
    Ok(p)
}

fn reset_object_property(state: &mut DesktopAppState, id: Uid, prop: &str) -> Result<()> {
    let keys = if prop == "*" {
        vec!["name", "visible", "locked", "xformMat"]
    } else {
        vec![prop]
    };
    for key in keys {
        match key {
            "name" => set_object_property(state, id, key, "")?,
            "visible" => set_object_property(state, id, key, "true")?,
            "locked" => set_object_property(state, id, key, "false")?,
            "xformMat" => {
                state.set_workspace_object_transform(id, &Matrix4::IDENTITY.to_script_string())?
            }
            "sel" => state.set_molecule_selection_text(id, "")?,
            _ => {
                let object = state.scene_manager_mut().object_mut(id).ok_or_else(|| {
                    CueMolError::invalid_argument(format!("{key} has no default"))
                })?;
                if !object.remove_qsc_extra_attr(key) {
                    return Err(CueMolError::invalid_argument(format!(
                        "{key} has no default"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn property_allowed(key: &str) -> &'static [&'static str] {
    match key {
        "stereoMode" => &["none", "para", "cross", "hardware"],
        "centerMark" => &["none", "crosshair", "axis"],
        "egtype" => &["none", "edges", "silhouette"],
        "drawmode" => &["fill", "line", "point"],
        "region_mode" => &["auto", "box", "full"],
        "lod" => &["auto", "step1", "step2", "step4", "step8"],
        "cap_mode" => &["auto", "on", "off"],
        "aa_method" => &["none", "fxaa", "smaa"],
        _ => &[],
    }
}

fn property_readonly(state: &DesktopAppState, kind: &str, id: Option<Uid>, key: &str) -> bool {
    if matches!(
        key,
        "uid" | "scene_uid" | "type_name" | "src" | "alt_src" | "srctype" | "modified"
    ) {
        return true;
    }
    if kind == "view" {
        return matches!(
            key,
            "width" | "height" | "hasHWStereo" | "up_vec" | "right_vec" | "forward_vec" | "style"
        );
    }
    if matches!(kind, "renderer" | "rendGroup") {
        if cuemol_desktop_core::auxiliary_property_readonly(key) {
            return true;
        }
        if let Some(renderer) = id.and_then(|id| {
            renderer_owner(state, id)
                .ok()
                .and_then(|owner| state.scene_manager().object(owner)?.renderer(id))
        }) {
            return cuemol_desktop_core::property_catalog::renderer_attr_properties(
                renderer.renderer_type(),
            )
            .iter()
            .any(|property| property.key == key && !property.writable);
        }
    }
    false
}

fn view_properties(state: &DesktopAppState, camera: &Camera) -> BTreeMap<String, String> {
    use cuemol_qsys::{CenterMark, StereoMode};
    let mut values = BTreeMap::from([
        ("zoom".into(), camera.zoom().to_string()),
        ("slab".into(), camera.slab_depth().to_string()),
        ("distance".into(), camera.cam_dist().to_string()),
        ("center".into(), camera.center().to_string()),
        ("rotation".into(), camera.rotation().to_string()),
        ("perspective".into(), camera.is_perspective().to_string()),
        (
            "stereoMode".into(),
            match camera.stereo_mode() {
                StereoMode::None => "none",
                StereoMode::Parallel => "para",
                StereoMode::Cross => "cross",
                StereoMode::Hardware => "hardware",
            }
            .into(),
        ),
        ("stereoDist".into(), camera.stereo_distance().to_string()),
        (
            "centerMark".into(),
            match camera.center_mark() {
                CenterMark::None => "none",
                CenterMark::Cross => "crosshair",
                CenterMark::Axis => "axis",
            }
            .into(),
        ),
    ]);
    for (alias, key) in [
        ("slabdepth", "slab"),
        ("camdist", "distance"),
        ("viewcenter", "center"),
        ("rotQuat", "rotation"),
    ] {
        values.insert(alias.into(), values[key].clone());
    }
    let view = state
        .active_view_summary()
        .and_then(|summary| state.command_manager().view(summary.uid));
    let defaults = View::new(Uid::new(0), Uid::new(0));
    let mut current = view.unwrap_or(&defaults).clone();
    current.set_camera(camera);
    for (key, value) in [
        ("name", view.map_or("", View::name).to_string()),
        ("active", current.active().to_string()),
        ("width", current.width().to_string()),
        ("height", current.height().to_string()),
        ("trans_mms", current.trans_mms().to_string()),
        ("rot_mms", current.rot_mms().to_string()),
        ("swapStereoEyes", current.swap_stereo_eyes().to_string()),
        ("hasHWStereo", current.has_hw_stereo().to_string()),
        ("up_vec", current.up_vec().to_string()),
        ("right_vec", current.right_vec().to_string()),
        ("forward_vec", current.forward_vec().to_string()),
        ("style", current.style().into()),
    ] {
        values.insert(key.into(), value);
    }
    values
}

fn set_view_property(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    key: &str,
    value: &str,
) -> Result<()> {
    use cuemol_qsys::{CenterMark, StereoMode};
    let invalid = || CueMolError::invalid_argument(format!("invalid view property {key}: {value}"));
    let number = || {
        value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(invalid)
    };
    let positive = || number().and_then(|v| if v > 0.0 { Ok(v) } else { Err(invalid()) });
    let boolean = || value.parse::<bool>().map_err(|_| invalid());
    match key {
        "zoom" => camera.set_zoom(positive()?),
        "slab" | "slabdepth" => camera.set_slab_depth(positive()?),
        "distance" | "camdist" => camera.set_cam_dist(positive()?),
        "center" | "viewcenter" => {
            let center = Vector4::from_script_string(value)?;
            if ![center.x, center.y, center.z].iter().all(|v| v.is_finite()) {
                return Err(invalid());
            }
            camera.set_center(center);
        }
        "rotation" | "rotQuat" => camera.set_rotation(parse_rotation(value)?),
        "perspective" => camera.set_perspective(boolean()?),
        "stereoMode" => camera.set_stereo_mode(match value {
            "none" => StereoMode::None,
            "para" => StereoMode::Parallel,
            "cross" => StereoMode::Cross,
            "hardware" => StereoMode::Hardware,
            _ => return Err(invalid()),
        }),
        "stereoDist" => {
            let distance = number()?;
            if distance < 0.0 {
                return Err(invalid());
            }
            camera.set_stereo_distance(distance);
        }
        "centerMark" => camera.set_center_mark(match value {
            "none" => CenterMark::None,
            "crosshair" => CenterMark::Cross,
            "axis" => CenterMark::Axis,
            _ => return Err(invalid()),
        }),
        "name" | "active" | "trans_mms" | "rot_mms" | "swapStereoEyes" => {
            let flag = if key == "name" { false } else { boolean()? };
            let id = state
                .active_view_summary()
                .ok_or_else(|| CueMolError::not_found("active view"))?
                .uid;
            let view = state.command_manager_mut().view_mut(id).unwrap();
            match key {
                "name" => view.set_name(value),
                "active" => view.set_active(flag),
                "trans_mms" => view.set_trans_mms(flag),
                "rot_mms" => view.set_rot_mms(flag),
                _ => view.set_swap_stereo_eyes(flag),
            }
        }
        _ => {
            return Err(CueMolError::invalid_argument(format!(
                "unknown or read-only view property {key}"
            )))
        }
    }
    if state.active_view_summary().is_some() {
        state.set_active_view_camera(camera)?;
    }
    Ok(())
}

fn set_object_property(
    state: &mut DesktopAppState,
    id: Uid,
    prop: &str,
    value: &str,
) -> Result<()> {
    match prop {
        "name" => {
            nodes::edit_node(
                state,
                "rename_node",
                &json!({"nodeId":id.get(),"nodeType":"object","name":value}),
            )?;
        }
        "visible" => {
            let visible = value
                .parse::<bool>()
                .map_err(|_| CueMolError::invalid_argument("expected true or false"))?;
            nodes::edit_node(
                state,
                "set_visible",
                &json!({"nodeId":id.get(),"nodeType":"object","visible":visible}),
            )?;
        }
        "locked" => {
            let locked = value
                .parse::<bool>()
                .map_err(|_| CueMolError::invalid_argument("expected true or false"))?;
            match nodes::object_kind(state, id)? {
                GuiWorkspaceObjectKind::Molecule => state.set_object_locked(id, locked)?,
                GuiWorkspaceObjectKind::DensityMap => state.set_density_map_locked(id, locked)?,
                GuiWorkspaceObjectKind::Surface => state.set_surface_locked(id, locked)?,
                GuiWorkspaceObjectKind::Lightweight => state.set_lw_object_locked(id, locked)?,
            }
        }
        "sel" => state.set_molecule_selection_text(id, value)?,
        "xformMat" => state.set_workspace_object_transform(id, value)?,
        _ => {
            let object = state
                .scene_manager_mut()
                .object_mut(id)
                .ok_or_else(|| CueMolError::not_found("molecule"))?;
            object.set_qsc_extra_attr(prop, value);
        }
    }
    Ok(())
}

fn style_index(state: &DesktopAppState, name: &str) -> Result<usize> {
    let s = state.scene_manager().scene(scene(state)?).unwrap();
    s.loaded_styles()
        .iter()
        .position(|style| style.node.str_attr("id").as_str() == name)
        .ok_or_else(|| CueMolError::not_found(format!("scene style set {name}")))
}
fn styles(state: &mut DesktopAppState, name: &str, args: &Value) -> Result<Value> {
    let sid = scene(state)?;
    if name == "create_style_set" {
        return Ok(json!({"index":state.create_scene_style(sid,text(args,"name")?)?}));
    }
    if name == "load_style_file" {
        return Ok(json!({"index":state.load_scene_style_file(sid,text(args,"path")?,true)?}));
    }
    let set = args["styleSet"].as_str().unwrap_or("default");
    let index = if name == "define_color" || name == "define_selection" {
        style_index(state, set).or_else(|_| state.create_scene_style(sid, set))?
    } else {
        style_index(state, set)?
    };
    if name == "delete_style_set" {
        return Ok(json!({"removed":state.destroy_scene_style(sid,index)?}));
    }
    let style = state.scene_manager().scene(sid).unwrap().loaded_styles()[index].clone();
    if name == "save_style_file" {
        let path = args["path"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| style.node.str_attr("src").to_string());
        if path.is_empty() {
            return Err(CueMolError::invalid_argument(
                "style has no source file; specify path",
            ));
        }
        state.save_scene_style_file(sid, index, &path)?;
        return Ok(json!({"path":path}));
    }
    let mut tree = if style.node.children().iter().any(|n| !n.is_attr()) {
        LDom2Tree::from(style.node.clone())
    } else {
        parse_ldom2_xml(&style.data)?
    };
    let readonly = tree.top().str_attr("readonly").as_str() == "true";
    if name == "set_style_readonly" {
        state.update_scene_style(
            sid,
            index,
            set,
            args["readonly"] == true,
            &write_ldom2_xml(&tree),
        )?;
        return Ok(json!({"readonly":args["readonly"]}));
    }
    if readonly {
        return Err(CueMolError::invalid_argument("style set is read-only"));
    }
    let tag = match name {
        "define_color" | "remove_color" => "color",
        "define_selection" | "remove_selection" => "sel",
        _ => "style",
    };
    let entry = text(args, "name")?;
    tree.top_mut()
        .retain_children(|n| n.tag_name() != tag || n.str_attr("id").as_str() != entry);
    if name == "define_color" || name == "define_selection" {
        let value = text(args, if tag == "color" { "color" } else { "selection" })?;
        if tag == "color" {
            cuemol_gfx::parse_color(value)?;
        } else {
            state.parse_scene_selector(sid, value)?;
        }
        let mut node = LDom2Node::new();
        node.set_tag_name(tag);
        node.set_str_attr("id", entry);
        node.set_value(value);
        tree.top_mut().append_child_node(node);
    }
    state.update_scene_style(sid, index, set, false, &write_ldom2_xml(&tree))?;
    Ok(json!({"styleSet":set,"name":entry}))
}

fn move_node(state: &mut DesktopAppState, args: &Value) -> Result<Value> {
    let sid = scene(state)?;
    let id = uid(args, "nodeId")?;
    if args["nodeType"] == "object" {
        let mut list = state
            .object_summaries(sid)
            .iter()
            .map(|o| (o.uid, o.ui_order, GuiWorkspaceObjectKind::Molecule))
            .collect::<Vec<_>>();
        list.extend(
            state
                .density_map_summaries(sid)
                .iter()
                .map(|o| (o.uid, o.ui_order, GuiWorkspaceObjectKind::DensityMap)),
        );
        list.extend(
            state
                .surface_summaries(sid)
                .iter()
                .map(|o| (o.uid, o.ui_order, GuiWorkspaceObjectKind::Surface)),
        );
        list.extend(
            state
                .lw_object_summaries(sid)
                .iter()
                .map(|o| (o.uid, o.ui_order, GuiWorkspaceObjectKind::Lightweight)),
        );
        list.sort_by_key(|e| e.1);
        let from = list
            .iter()
            .position(|e| e.0 == id)
            .ok_or_else(|| CueMolError::not_found("object"))?;
        let to = nodes::move_target(args, from, list.len())?;
        state.move_workspace_object(sid, list[from].2, id, list[to].2, list[to].0, to > from)?;
    } else {
        let owner = renderer_owner(state, id)?;
        let mut entries = if let Some(o) = state.density_map_summary(owner) {
            o.renderers
                .iter()
                .map(|r| (r.uid, r.ui_order))
                .collect::<Vec<_>>()
        } else if let Some(o) = state.surface_summary(owner) {
            o.renderers.iter().map(|r| (r.uid, r.ui_order)).collect()
        } else if let Some(o) = state.lw_object_summary(owner) {
            o.renderers.iter().map(|r| (r.uid, r.ui_order)).collect()
        } else {
            state
                .renderer_summaries(owner)
                .iter()
                .filter(|r| r.renderer_type != "*selection")
                .map(|r| (r.uid, r.ui_order))
                .collect()
        };
        entries.sort_by_key(|e| e.1);
        let from = entries
            .iter()
            .position(|e| e.0 == id)
            .ok_or_else(|| CueMolError::not_found("renderer"))?;
        let to = nodes::move_target(args, from, entries.len())?;
        state.move_renderer_in_object(
            owner,
            id,
            entries[to].0,
            if to > from { "after" } else { "before" },
        )?;
    }
    Ok(json!({"nodeId":id.get()}))
}

fn export(state: &DesktopAppState, camera: &Camera, name: &str, args: &Value) -> Result<Value> {
    let path = text(args, "path")?;
    let format = args["format"]
        .as_str()
        .or_else(|| {
            std::path::Path::new(path)
                .extension()
                .and_then(|s| s.to_str())
        })
        .unwrap_or("png");
    if name == "export_image" || format == "png" {
        let mut values = args.clone();
        values["fileName"] = json!(path);
        return crate::commands::render(state, camera, "render_image", &values);
    }
    if format == "pov" {
        let count =
            state.export_active_scene_pov_with_camera(path, "", camera, Default::default())?;
        return Ok(json!({"path":path,"primitives":count}));
    }
    let mut snapshot = state.active_render_snapshot("")?;
    state.apply_scene_color_proof(&mut snapshot)?;
    match format {
        "stl" => {
            let exporter = cuemol_render::StlSceneExporter::new();
            let mesh = cuemol_render::tessellate_snapshot(&snapshot);
            std::fs::write(
                path,
                exporter.write_binary_stl_bytes_with_camera(&mesh, camera),
            )?;
        }
        "mqo" => {
            let exporter = cuemol_render::MqoSceneExporter::new();
            std::fs::write(
                path,
                exporter.write_snapshot_string_with_camera("scene", &snapshot, camera),
            )?;
        }
        _ => {
            return Err(CueMolError::invalid_argument(
                "unsupported scene export format",
            ))
        }
    }
    Ok(json!({"path":path}))
}

fn parse_rotation(value: &str) -> Result<Quat> {
    let parts = value
        .trim()
        .trim_matches(|c| c == '(' || c == ')' || c == '[' || c == ']')
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(str::parse::<f64>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| CueMolError::invalid_argument("invalid quaternion"))?;
    if parts.len() != 4 || parts.iter().any(|n| !n.is_finite()) {
        return Err(CueMolError::invalid_argument(
            "quaternion requires four finite components",
        ));
    }
    let norm = parts.iter().map(|n| n * n).sum::<f64>().sqrt();
    if norm < 1e-12 {
        return Err(CueMolError::invalid_argument("zero quaternion"));
    }
    Ok(Quat::new(
        parts[0] / norm,
        parts[1] / norm,
        parts[2] / norm,
        parts[3] / norm,
    ))
}
