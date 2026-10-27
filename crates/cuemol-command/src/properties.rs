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
            for part in text(args, "settings")?
                .split(';')
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
                    .unwrap_or(key);
                if !RENDER_KEYS.contains(&local_key)
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
                let valid = match local_key {
                    "backend" => parsed
                        .as_str()
                        .is_some_and(|v| matches!(v, "" | "umbreon" | "umbreon_npr")),
                    "projection" => parsed
                        .as_str()
                        .is_some_and(|v| matches!(v, "perspective" | "orthographic")),
                    "unit" => parsed
                        .as_str()
                        .is_some_and(|v| matches!(v, "px" | "in" | "cm" | "mm")),
                    "denoise" => parsed
                        .as_str()
                        .is_some_and(|v| matches!(v, "Bilateral" | "A-trous" | "None")),
                    "hatchInkColor" | "hatchPaperColor" | "hatchColoring" => parsed.is_string(),
                    "transparent" | "transparentBg" | "perspective" | "useClipZ" | "clipPlane"
                    | "shadows" | "showEdgeLines" | "edgeLines" | "useGi" | "useGI"
                    | "giDenoise" | "aoEnabled" | "hatchCustomInk" | "hatchCustomPaper" => {
                        parsed.is_boolean()
                    }
                    "width" | "height" | "dpi" | "hatchDensity" | "hatchWidthScale" => {
                        parsed.as_f64().is_some_and(|n| n.is_finite() && n > 0.)
                    }
                    _ => parsed.as_f64().is_some_and(f64::is_finite),
                };
                if !valid {
                    return Err(CueMolError::invalid_argument(format!(
                        "invalid value for render setting {key}"
                    )));
                }
                settings.insert(key.into(), parsed);
            }
            state
                .scene_manager_mut()
                .scene_mut(sid)
                .unwrap()
                .set_scene_attribute("render_settings", json!(settings).to_string());
            Ok(json!({"settings":settings}))
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

const RENDER_KEYS: &[&str] = &[
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
pub fn render_settings(state: &DesktopAppState) -> Result<serde_json::Map<String, Value>> {
    let s = state.scene_manager().scene(scene(state)?).unwrap();
    Ok(s.scene_attribute("render_settings")
        .and_then(|v| serde_json::from_str::<Value>(v).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default())
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
            "view" => {
                let number = || {
                    value
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite() && *v > 0.)
                        .ok_or_else(|| {
                            CueMolError::invalid_argument("expected positive finite number")
                        })
                };
                match prop.as_str() {
                    "zoom" => camera.set_zoom(number()?),
                    "slab" | "slabdepth" => camera.set_slab_depth(number()?),
                    "distance" | "camdist" => camera.set_cam_dist(number()?),
                    "center" | "viewcenter" => {
                        camera.set_center(Vector4::from_script_string(value)?)
                    }
                    "rotation" | "rotQuat" => camera.set_rotation(parse_rotation(value)?),
                    "perspective" => camera.set_perspective(
                        value
                            .parse()
                            .map_err(|_| CueMolError::invalid_argument("expected true or false"))?,
                    ),
                    _ => {
                        state.put_config_value(&format!("view.{prop}"), value);
                    }
                }
            }
            "scene" => {
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
                if prop == "*" {
                    let keys = state
                        .scene_manager()
                        .object(owner)
                        .and_then(|o| o.renderer(id))
                        .ok_or_else(|| {
                            CueMolError::invalid_argument("reset-all requires molecule renderer")
                        })?
                        .qsc_attrs()
                        .keys()
                        .filter(|k| !matches!(k.as_str(), "name" | "type" | "ui_order"))
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
            "object" => {
                let object = state
                    .scene_manager_mut()
                    .object_mut(id.unwrap())
                    .ok_or_else(|| CueMolError::not_found("molecule"))?;
                object.remove_qsc_extra_attr(&prop);
            }
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
            v
        }
        "view" => BTreeMap::from([
            ("zoom".into(), camera.zoom().to_string()),
            ("slab".into(), camera.slab_depth().to_string()),
            ("distance".into(), camera.cam_dist().to_string()),
            ("center".into(), camera.center().to_string()),
            ("rotation".into(), camera.rotation().to_string()),
            ("perspective".into(), camera.is_perspective().to_string()),
        ]),
        _ => return Err(CueMolError::invalid_argument("unknown node type")),
    };
    if name == "list_node_props" {
        Ok(
            json!({"properties":values.iter().map(|(key,value)|json!({"key":key,"value":value,"readonly":false})).collect::<Vec<_>>()}),
        )
    } else {
        Ok(json!({"prop":prop,"value":values.remove(&prop)}))
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
    p.insert("name".into(), name);
    p.insert("visible".into(), visible.to_string());
    p.insert("locked".into(), locked.to_string());
    Ok(p)
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
        "xformMat" => state
            .scene_manager_mut()
            .object_mut(id)
            .ok_or_else(|| CueMolError::not_found("molecule"))?
            .set_xform_matrix(Matrix4::from_script_string(value)?),
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
    let snapshot = state.active_render_snapshot("")?;
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
