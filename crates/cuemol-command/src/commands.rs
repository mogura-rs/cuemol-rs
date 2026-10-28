use base64::Engine;
use cuemol_core::{CueMolError, Quat, Result, Uid, Vector4};
use cuemol_desktop_core::{DesktopAppState, GuiLoadedPath, GuiSceneSaveOptions};
use cuemol_qsys::{Camera, View};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args[key]
        .as_str()
        .ok_or_else(|| CueMolError::invalid_argument(format!("missing string: {key}")))
}
pub(crate) fn uid(args: &Value, key: &str) -> Result<Uid> {
    args[key]
        .as_u64()
        .filter(|id| *id > 0)
        .map(Uid::new)
        .ok_or_else(|| CueMolError::invalid_argument(format!("invalid identifier: {key}")))
}
pub(crate) fn integer(args: &Value, key: &str, default: usize) -> Result<usize> {
    if args[key].is_null() {
        return Ok(default);
    }
    args[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| CueMolError::invalid_argument(format!("invalid nonnegative integer: {key}")))
}
pub(crate) fn scene(state: &DesktopAppState) -> Result<Uid> {
    state
        .active_scene()
        .ok_or_else(|| CueMolError::not_found("active scene"))
}
pub(crate) fn renderer_owner(state: &DesktopAppState, renderer: Uid) -> Result<Uid> {
    let sid = scene(state)?;
    let current = state
        .scene_manager()
        .scene(sid)
        .ok_or_else(|| CueMolError::not_found("scene"))?;
    if let Some(object) = current
        .objects()
        .iter()
        .find(|object| object.renderer(renderer).is_some())
    {
        return Ok(object.uid());
    }
    for object in state.density_map_summaries(sid) {
        if object.renderers.iter().any(|r| r.uid == renderer)
            || object.renderer_groups.iter().any(|r| r.uid == renderer)
        {
            return Ok(object.uid);
        }
    }
    for object in state.surface_summaries(sid) {
        if object.renderers.iter().any(|r| r.uid == renderer)
            || object.renderer_groups.iter().any(|r| r.uid == renderer)
        {
            return Ok(object.uid);
        }
    }
    for object in state.lw_object_summaries(sid) {
        if object.renderers.iter().any(|r| r.uid == renderer)
            || object.renderer_groups.iter().any(|r| r.uid == renderer)
        {
            return Ok(object.uid);
        }
    }
    Err(CueMolError::not_found("renderer in active scene"))
}

pub fn view_json(camera: &Camera) -> Value {
    let center = camera.center();
    let q = camera.rotation();
    json!({"zoom":camera.zoom(),"slab":camera.slab_depth(),"distance":camera.cam_dist(),
        "center":[center.x,center.y,center.z],"rotation":[q.x,q.y,q.z,q.w],"perspective":camera.is_perspective()})
}

pub(crate) fn fit(
    state: &DesktopAppState,
    camera: &mut Camera,
    object: Option<Uid>,
    selection: Option<&str>,
    zoom: bool,
) -> Result<()> {
    let scene_uid = scene(state)?;
    let ids = object.map(|id| vec![id]).unwrap_or_else(|| {
        state
            .object_summaries(scene_uid)
            .iter()
            .map(|o| o.uid)
            .collect()
    });
    let selector = selection
        .map(|s| state.parse_scene_selector(scene_uid, s))
        .transpose()?;
    let objects = ids
        .into_iter()
        .map(|id| {
            state
                .scene_manager()
                .object(id)
                .ok_or_else(|| CueMolError::not_found("molecule"))
        })
        .collect::<Result<Vec<_>>>()?;
    let positions = objects.into_iter().flat_map(|object| {
        let selection = selector
            .as_ref()
            .unwrap_or_else(|| object.mol().selection());
        object
            .mol()
            .select_indices(selection)
            .into_iter()
            .map(move |index| {
                let mut position = object.mol().atoms()[index].position;
                object.xform_matrix().xform3d(&mut position);
                position
            })
    });
    let found = if zoom {
        let aspect = state
            .active_view_summary()
            .and_then(|view| state.command_manager().view(view.uid))
            .filter(|view| view.width() > 0 && view.height() > 0)
            .map_or(1.0, |view| {
                f64::from(view.width()) / f64::from(view.height())
            });
        camera.fit_positions(positions, aspect)
    } else {
        let (total, count) = positions.fold((Vector4::ZERO, 0usize), |(total, count), point| {
            (total + point, count + 1)
        });
        if count > 0 {
            camera.set_center(total.scale(1.0 / count as f64));
        }
        count > 0
    };
    if !found {
        return Err(CueMolError::not_found("atoms matching selection"));
    }
    Ok(())
}

pub(crate) fn scene_state(state: &DesktopAppState) -> Result<Value> {
    let id = scene(state)?;
    let mut objects: Vec<_> = state.object_summaries(id).into_iter().map(|object| {
        let renderers: Vec<_> = state.renderer_summaries(object.uid).into_iter().filter(|renderer|renderer.renderer_type != "*selection").map(|renderer| json!({
            "id":renderer.uid.get(),"name":renderer.name,"type":renderer.renderer_type,"visible":renderer.visible,"group":renderer.group
        })).collect();
        json!({"id":object.uid.get(),"name":object.name,"className":"MolCoord","visible":object.visible,
            "atoms":object.atom_count,"residues":object.residue_count,"bonds":object.bond_count,"renderers":renderers})
    }).collect();
    objects.extend(crate::nodes::extra_scene_objects(state)?);
    let maps: Vec<_> = state
        .density_map_summaries(id)
        .into_iter()
        .map(|map| json!({"id":map.uid.get(),"name":map.name,"visible":map.visible}))
        .collect();
    Ok(json!({"sceneId":id.get(),"objects":objects,"maps":maps,"namedSelections":[]}))
}

pub(crate) fn loaded_result(loaded: GuiLoadedPath) -> Value {
    match loaded {
        GuiLoadedPath::Object {
            scene_uid,
            object_uid,
        } => json!({"sceneId":scene_uid.get(),"objectId":object_uid.get()}),
        GuiLoadedPath::Map { scene_uid, map_uid } => {
            json!({"sceneId":scene_uid.get(),"objectId":map_uid.get()})
        }
        GuiLoadedPath::Scene { scene_uid } => json!({"sceneId":scene_uid.get()}),
    }
}

pub fn call_tool(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: &Value,
) -> Result<Value> {
    crate::protocol::validate_args(name, args).map_err(CueMolError::invalid_argument)?;
    match name {
        "clear_undo" => Ok(json!({"cleared":true})),
        "list_scenes" => Ok(json!(state.scene_summaries().into_iter().map(|s| json!({"sceneId":s.uid.get(),"name":s.name,"active":s.active,"objects":s.object_count,"tabs":1})).collect::<Vec<_>>())),
        "create_scene" => {
            let made=state.new_scene(args["name"].as_str().unwrap_or("Untitled"),true);
            *camera=Camera::new();
            Ok(json!({"sceneId":made.result_scene.get(),"active":true}))
        }
        "switch_scene" => {
            let id=uid(args,"sceneId")?;
            if !state.set_active_scene(Some(id)) { return Err(CueMolError::not_found("scene")); }
            *camera=state.active_view_camera().unwrap_or_default();
            Ok(json!({"sceneId":id.get(),"active":true}))
        }
        "close_scene" => {
            let id=if args["sceneId"].is_null() {scene(state)?} else {uid(args,"sceneId")?};
            let target=state.scene_manager().scene(id).ok_or_else(|| CueMolError::not_found("scene"))?;
            if args["discardChanges"] != true && target.objects().iter().any(|object| object.modified()) {
                return Err(CueMolError::invalid_argument("Scene has unsaved changes; save it or set discardChanges"));
            }
            state.destroy_scene(id);
            Ok(json!({"closed":id.get()}))
        }
        "get_scene_state" => scene_state(state),
        "load_file" => crate::file_commands::load_file(state,camera,name,args),
        "fetch_pdb" | "fetch_url" => {
            #[cfg(not(target_arch="wasm32"))]
            { return crate::transport::fetch_structure(state,camera,name,args); }
            #[cfg(target_arch="wasm32")]
            { Err(CueMolError::invalid_argument("Use the browser file or download controls")) }
        }
        "list_renderer_types" => {
            let id=uid(args,"objId")?;
            let types=if state.density_map_summary(id).is_some() { state.density_map_renderer_setup_options() }
                else { state.scene_manager().object(id).ok_or_else(||CueMolError::not_found("object"))?; state.molecule_renderer_setup_options(scene(state)?)? };
            Ok(json!({"rendererTypes":types.iter().filter(|t|!t.preset).map(|t|&t.name).collect::<Vec<_>>(),"presetTypes":types.iter().filter(|t|t.preset).map(|t|&t.name).collect::<Vec<_>>()}))
        }
        "create_renderer" | "create_preset_renderer" => {
            let object=uid(args,"objId")?;
            let kind=text(args,if name=="create_renderer" {"rendererType"} else {"preset"})?;
            let label=args["name"].as_str().map(str::to_string).unwrap_or(state.suggest_renderer_name(scene(state)?,kind)?);
            let renderer=if state.density_map_summary(object).is_some() {state.add_density_renderer(object,kind)?}
                else {state.new_renderer_from_setup(scene(state)?,object,kind,&label,args["selection"].as_str())?};
            if args["centerView"] == true { fit(state,camera,Some(object),args["selection"].as_str(),true)?; }
            Ok(json!({"objectId":object.get(),"rendererId":renderer.get(),"name":label}))
        }
        "set_renderer_type" => {
            let id=uid(args,"rendId")?;let object=renderer_owner(state,id)?;
            let changed=state.change_renderer_type_in_object(object,id,text(args,"rendererType")?)?;
            Ok(json!({"rendererId":changed.get()}))
        }
        "set_renderer_selection" => {
            let id=uid(args,"rendId")?;let object=renderer_owner(state,id)?;
            state.set_renderer_selection_text(object,id,text(args,"selection")?)?;
            Ok(json!({"rendererId":id.get()}))
        }
        "set_visible" | "delete_node" | "rename_node" => crate::nodes::edit_node(state,name,args),
        "count_selection" | "list_chains" | "list_residues" => {
            let object=state.scene_manager().object(uid(args,"objId")?).ok_or_else(||CueMolError::not_found("molecule"))?;
            match name {
                "count_selection" => {let count=object.mol().select_indices(&state.parse_scene_selector(scene(state)?,text(args,"selection")?)?).len();Ok(json!({"valid":true,"atomCount":count,"count":count}))},
                "list_chains" => Ok(json!({"chains":object.mol().chain_ids()})),
                _ => {
                    let chain=text(args,"chain")?;let offset=integer(args,"offset",0)?;let limit=integer(args,"limit",100)?.min(1000);
                    let mut seen=BTreeSet::new();
                    let residues:Vec<_>=object.mol().atoms().iter().filter(|atom|atom.chain_id==chain)
                        .filter(|atom|seen.insert((atom.res_id,atom.ins_code))).map(|atom|json!({"index":atom.res_id,"name":atom.res_name,"insertionCode":atom.ins_code})).collect();
                    Ok(json!({"total":residues.len(),"offset":offset,"residues":residues.into_iter().skip(offset).take(limit).collect::<Vec<_>>()}))
                }
            }
        }
        "set_mol_selection" => {state.set_molecule_selection_text(uid(args,"objId")?,text(args,"selection")?)?;Ok(json!({"selection":args["selection"]}))}
        "center_view" => {fit(state,camera,Some(uid(args,"objId")?),args["selection"].as_str(),args["zoom"]==true)?;Ok(view_json(camera))}
        "get_view" => Ok(view_json(camera)),
        "set_view" => {
            for key in ["zoom","slab","distance"] {if let Some(n)=args[key].as_f64() {
                if n<=0.0 || !n.is_finite() {return Err(CueMolError::invalid_argument(format!("{key} must be positive")));}
                match key {"zoom"=>camera.set_zoom(n),"slab"=>camera.set_slab_depth(n),_=>camera.set_cam_dist(n)}
            }}
            if let Some(center)=args["center"].as_array() {camera.set_center(Vector4::new3(center[0].as_f64().unwrap(),center[1].as_f64().unwrap(),center[2].as_f64().unwrap()));}
            if args["fitSlab"]==true {
                if !args["slab"].is_null() {return Err(CueMolError::invalid_argument("fitSlab and slab are mutually exclusive"));}
                let mut fitted=camera.clone();fit(state,&mut fitted,args["objId"].as_u64().map(Uid::new),None,true)?;camera.set_slab_depth(fitted.slab_depth());
            }
            Ok(view_json(camera))
        }
        "rotate_view" => {
            let axis=match text(args,"axis")? {"x"=>Vector4::new3(1.,0.,0.),"y"=>Vector4::new3(0.,1.,0.),_=>Vector4::new3(0.,0.,1.)};
            camera.set_rotation(camera.rotation().mul_quat(Quat::from_axis_angle(axis,args["angle"].as_f64().unwrap().to_radians())?).normalize()?);
            Ok(view_json(camera))
        }
        "pan_view" => {
            let mut view=View::new(Uid::new(1),scene(state)?);view.set_camera(camera);
            view.set_view_size(state.runtime_value("viewport:width").parse::<f64>().unwrap_or(800.0) as u32,
                state.runtime_value("viewport:height").parse::<f64>().unwrap_or(600.0) as u32);
            view.translate_view(args["dx"].as_f64().unwrap(),args["dy"].as_f64().unwrap(),args["dz"].as_f64().unwrap_or(0.0));
            *camera=view.camera();Ok(view_json(camera))
        }
        "set_projection" => {if let Some(value)=args["perspective"].as_bool(){camera.set_perspective(value);}Ok(json!({"perspective":camera.is_perspective()}))}
        "list_cameras" => Ok(json!({"cameras":state.active_camera_summaries().iter().map(|c|json!({"name":c.name})).collect::<Vec<_>>()})),
        "save_camera" => {let mut saved=camera.clone();saved.set_name(text(args,"name")?);state.save_scene_camera(scene(state)?,saved,args["withVisibility"]==true)?;Ok(json!({"name":args["name"]}))}
        "apply_camera" => {*camera=state.load_scene_camera(scene(state)?,text(args,"name")?,args["withVisibility"]==true)?;Ok(view_json(camera))}
        "delete_camera" => {if !state.destroy_scene_camera(scene(state)?,text(args,"name")?)?{return Err(CueMolError::not_found("camera"));}Ok(json!({"deleted":args["name"]}))}
        "load_camera_file" => Ok(json!({"name":state.load_scene_camera_file(scene(state)?,text(args,"path")?)?})),
        "save_camera_file" => {
            let name=text(args,"name")?;
            let path=args["path"].as_str().map(str::to_string).or_else(||state.active_camera_summaries().into_iter().find(|c|c.name==name).map(|c|c.source)).filter(|p|!p.is_empty()).ok_or_else(||CueMolError::invalid_argument("camera file path is required"))?;
            state.save_scene_camera_file(scene(state)?,name,&path)?;Ok(json!({"path":path}))
        }
        "reload_camera" => {state.reload_scene_camera_file(scene(state)?,text(args,"name")?)?;Ok(json!({"name":args["name"]}))}
        "save_scene" => {state.set_active_view_camera(camera)?;state.save_active_scene_qsc_with_options(text(args,"path")?,GuiSceneSaveOptions{embed_all:true,..Default::default()})?;Ok(json!({"path":args["path"]}))}
        "render_image" | "get_screenshot" | "capture_view" => render(state,camera,name,args),
        "execute_script" => {
            #[cfg(feature="scripting")]
            {state.set_active_view_camera(camera)?;let (log,selection)=state.run_script_js(text(args,"source")?)?;if let Some(updated)=state.active_view_camera(){*camera=updated;}Ok(json!({"log":log,"selection":selection}))}
            #[cfg(not(feature="scripting"))]
            {Err(CueMolError::invalid_argument("This build does not include the scripting feature"))}
        }
        "list_trajectories" => Ok(json!({"trajectories":state.active_trajectory_summaries().iter().map(|t|json!({"objectId":t.object_uid.get(),"name":t.object_name,"frames":t.frame_count,"frame":t.current_frame,"blocks":t.blocks.iter().map(|b|json!({"id":b.uid,"source":b.source,"format":b.format,"frames":b.frame_count})).collect::<Vec<_>>()})).collect::<Vec<_>>()})),
        "load_trajectory" => {
            if state.active_scene().is_none(){state.new_scene("Untitled",true);}
            let paths=args["paths"].as_array().unwrap().iter().map(|v|v.as_str().unwrap().to_string()).collect::<Vec<_>>();
            let loaded=state.load_md_trajectory(scene(state)?,text(args,"topology")?,&paths,integer(args,"stride",1)?,"auto")?;
            fit(state,camera,Some(loaded.object_uid),None,true)?;
            Ok(json!({"objectId":loaded.object_uid.get(),"frames":loaded.frame_count}))
        }
        "append_trajectory" => {let result=state.append_trajectory_block(uid(args,"objId")?,text(args,"path")?,integer(args,"stride",1)?)?;Ok(json!({"objectId":result.object_uid.get(),"frames":result.frame_count}))}
        "set_frame" => {state.apply_trajectory_frame(uid(args,"objId")?,integer(args,"frame",0)?)?;Ok(json!({"frame":args["frame"]}))}
        "get_renderer_property" | "set_renderer_property" => {
            let id=uid(args,"rendId")?;let prop=text(args,"prop")?;
            if name=="set_renderer_property" {crate::nodes::set_renderer_property(state,id,prop,text(args,"value")?)?;}
            Ok(json!({"prop":prop,"value":crate::nodes::renderer_properties(state,id)?.get(prop)}))
        }
        "list_coloring_styles" => Ok(json!({"styles":["DefaultCPKColoring","DefaultHSCPaint","DefaultRainbowColoring","DefaultBfacColoring"]})),
        "set_renderer_coloring" => set_coloring(state,args),
        _ => crate::advanced::call_tool(state,camera,name,args),
    }
}

fn set_coloring(state: &mut DesktopAppState, args: &Value) -> Result<Value> {
    let id = uid(args, "nodeId")?;
    let target = text(args, "nodeType")?;
    let mode = text(args, "mode")?;
    let class = match mode.trim_start_matches("paint-type-") {
        "cpk" => Some("CPKColoring"),
        "rainbow" => Some("RainbowColoring"),
        "bfac" => Some("BfacColoring"),
        "solid" => Some("SolidColoring"),
        "paint" => Some("PaintColoring"),
        "resetdef" => None,
        "multigrad" => {
            let object = renderer_owner(state, id)?;
            let nodes = vec![
                (-1.0, "blue".into()),
                (0.0, "white".into()),
                (1.0, "red".into()),
            ];
            if state.density_map_summary(object).is_some() {
                state.set_density_renderer_coloring(
                    object,
                    id,
                    "multigrad",
                    &BTreeMap::new(),
                    &nodes,
                )?;
            } else {
                state.set_surface_renderer_coloring(
                    object,
                    id,
                    "multigrad",
                    &BTreeMap::new(),
                    &nodes,
                )?;
            }
            return Ok(json!({"nodeId":id.get()}));
        }
        "style" => {
            if target != "renderer" {
                return Err(CueMolError::invalid_argument(
                    "Coloring styles require a renderer",
                ));
            }
            let object = renderer_owner(state, id)?;
            state.apply_renderer_style_option(object, id, text(args, "styleName")?, "coloring")?;
            return Ok(json!({"nodeId":id.get()}));
        }
        _ => {
            return Err(CueMolError::invalid_argument(
                "Unsupported coloring mode for this node",
            ))
        }
    }
    .map(str::to_string);
    let paint = if mode == "paint-type-paint" {
        let object = if target == "object" {
            id
        } else {
            renderer_owner(state, id)?
        };
        let scene = state
            .scene_manager()
            .scene_uid_for_object(object)
            .ok_or_else(|| CueMolError::NotFound(format!("object {}", object.get())))?;
        state.default_molecule_paint(scene, object)?
    } else {
        vec![]
    };
    if target == "object" {
        state.set_object_coloring_details(id, class, BTreeMap::new(), paint, None)?;
    } else {
        let object = renderer_owner(state, id)?;
        state.set_renderer_coloring_details(object, id, class, BTreeMap::new(), paint, None)?;
    }
    Ok(json!({"nodeId":id.get()}))
}

pub fn prepare_render(
    state: &DesktopAppState,
    camera: &Camera,
    name: &str,
    args: &Value,
) -> Result<PreparedRender> {
    let settings = crate::properties::render_settings(state)?;
    let size = |key: &str, default: usize| -> Result<usize> {
        if !args[key].is_null() {
            return integer(args, key, default);
        }
        let value = settings
            .get(key)
            .and_then(Value::as_f64)
            .unwrap_or(default as f64);
        let dpi = settings.get("dpi").and_then(Value::as_f64).unwrap_or(300.);
        let scale = match settings.get("unit").and_then(Value::as_str).unwrap_or("px") {
            "px" => 1.,
            "in" => dpi,
            "cm" => dpi / 2.54,
            "mm" => dpi / 25.4,
            _ => {
                return Err(CueMolError::invalid_argument(
                    "render unit must be px, in, cm, or mm",
                ))
            }
        };
        if !value.is_finite() || value <= 0. || !scale.is_finite() || scale <= 0. {
            return Err(CueMolError::invalid_argument(
                "render dimensions and DPI must be positive",
            ));
        }
        Ok((value * scale).round() as usize)
    };
    let width = if name == "capture_view" {
        integer(args, "longSide", 1024)?.clamp(256, 1568)
    } else {
        size("width", 640)?
    };
    let height = if name == "capture_view" {
        width * 3 / 4
    } else {
        size("height", 480)?
    };
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || width.saturating_mul(height) > 32_000_000
    {
        return Err(CueMolError::invalid_argument(
            "render size must be positive and at most 32 megapixels",
        ));
    }
    let mut snapshot = state.active_render_snapshot("")?;
    state.apply_scene_color_proof(&mut snapshot)?;
    let npr = args["style"]
        .as_str()
        .map(|style| style == "npr")
        .unwrap_or(settings.get("backend").is_some_and(|v| v == "umbreon_npr"));
    let prefix = if npr { "umbreon_npr" } else { "umbreon" };
    let mut params = rendering_params(&settings, prefix)?;
    params.width = width as i32;
    params.height = height as i32;
    params.perspective = settings
        .get("projection")
        .and_then(Value::as_str)
        .map(|v| v == "perspective")
        .unwrap_or(camera.is_perspective());
    if name != "render_image"
        && !settings.get("backend").is_some_and(|v| v == "umbreon")
        && settings.get("useGI").is_none()
        && settings.get("umbreon.useGI").is_none()
    {
        params.use_gi = false;
        if settings.get("flashFraction").is_none()
            && settings.get(&format!("{prefix}.flashFraction")).is_none()
        {
            params.flash_fraction = 0.6;
        }
        if settings.get("ambientFraction").is_none()
            && settings.get(&format!("{prefix}.ambientFraction")).is_none()
        {
            params.ambient_fraction = 0.16;
        }
    }
    let background = state.active_background_color()?;
    let hatch = if npr {
        Some(hatch_params(&settings, prefix)?)
    } else {
        None
    };
    let output = if name == "render_image" {
        Some(text(args, "fileName")?.to_string())
    } else {
        None
    };
    Ok(PreparedRender {
        snapshot,
        camera: camera.clone(),
        background,
        params,
        hatch,
        output,
    })
}

pub fn rendering_params(
    settings: &serde_json::Map<String, Value>,
    prefix: &str,
) -> Result<cuemol_render::UmbreonRenderParams> {
    let get = |key: &str| {
        settings
            .get(&format!("{prefix}.{key}"))
            .or_else(|| settings.get(key))
    };
    if let Some(color) = get("giGroundColor").and_then(Value::as_str) {
        let hex = color.strip_prefix('#').unwrap_or(color);
        if hex.len() != 6 || u32::from_str_radix(hex, 16).is_err() {
            return Err(CueMolError::invalid_argument(
                "GI ground color must be #rrggbb",
            ));
        }
    }
    let mut typed = cuemol_qsys::RenderSettings::default();
    let class = if prefix == "umbreon_npr" {
        cuemol_qsys::SettingsClass::UmbreonNpr
    } else {
        cuemol_qsys::SettingsClass::Umbreon
    };
    for (block, class) in [("", cuemol_qsys::SettingsClass::Render), (prefix, class)] {
        for property in class.properties() {
            let value = get(property.name).or_else(|| match property.name {
                "useGI" => get("useGi"),
                "clipPlane" => get("useClipZ"),
                "edgeLines" => get("showEdgeLines"),
                "transparentBg" => get("transparent"),
                _ => None,
            });
            if let Some(value) = value {
                let path = if block.is_empty() {
                    property.name.into()
                } else {
                    format!("{block}.{}", property.name)
                };
                let value = value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string());
                typed.set(&path, &value)?;
            }
        }
    }
    if get("aoEnabled").is_none() && get("aoSamples").is_some() {
        typed.set(&format!("{prefix}.aoEnabled"), "true")?;
    }
    let mut exporter = cuemol_render::UmbreonSceneExporter::new();
    exporter.apply_render_settings(&typed, prefix)?;
    let mut params = exporter.params().clone();
    let number = |key: &str, fallback: f64| get(key).and_then(Value::as_f64).unwrap_or(fallback);
    params.aa_mode = number("aaMode", 0.) as i32;
    params.aa_depth = number("aaDepth", 0.) as i32;
    params.ao_res_div = number("aoResDiv", params.ao_res_div as f64) as i32;
    params.ao_falloff_power = number("aoFalloffPower", 0.);
    params.gi_bounces = number("giBounces", 4.).clamp(1., 8.) as i32;
    params.gi_intensity = number("giIntensity", 1.);
    params.gi_env_intensity = number("giEnvIntensity", 1.);
    if get("denoise").is_none() {
        params.gi_denoise = get("giDenoise")
            .and_then(Value::as_bool)
            .unwrap_or(params.gi_denoise);
        params.denoiser = number("denoiser", 0.) as i32;
    }
    Ok(params)
}
pub fn rendering_settings(p: &cuemol_render::UmbreonRenderParams, prefix: &str) -> String {
    let ground = p
        .gi_ground_color
        .map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
    let settings = json!({"supersample":p.supersample,"aaMode":p.aa_mode,"aaDepth":p.aa_depth,"aoResDiv":p.ao_res_div,"aoEnabled":p.ao_samples>0,"aoSamples":p.ao_samples,"aoDistance":p.ao_distance,"aoIntensity":p.ao_intensity,
        "aoDiffuseFactor":p.ao_diffuse_factor,"aoMultiScale":p.ao_multi_scale,"aoBentNormal":p.ao_bent_normal,"aoLowDiscrepancy":p.ao_low_discrepancy,"aoFalloffPower":p.ao_falloff_power,
        "aoGather":if p.ao_res_div < 0 || p.ao_res_div > 1 {"Per output pixel"} else {"Per shading hit"},"shadows":p.shadows,"shadowSamples":p.shadow_samples,"lightRadius":p.light_radius,
        "lightIntensity":p.light_intensity,"flashFraction":p.flash_fraction,"ambientFraction":p.ambient_fraction,"edgeLines":p.show_edge_lines,"creaseLimit":p.crease_limit,"edgeRise":p.edge_rise,
        "contactEdges":p.contact_edges,"outlineFarDepth":p.outline_far_depth,"perPixelBlend":p.per_pixel_blend,"useGI":p.use_gi,"giSamples":p.gi_samples,"giBounces":p.gi_bounces,
        "giIntensity":p.gi_intensity,"giEnvIntensity":p.gi_env_intensity,"giSkyGradient":p.gi_sky_gradient,"giGroundColor":format!("#{:02x}{:02x}{:02x}",ground[0],ground[1],ground[2]),
        "denoise":if p.denoiser==2 || (p.gi_denoise && p.gi_denoiser==2) {"OIDN"} else if p.denoiser==1 {"A-trous"} else if p.gi_denoise {"Bilateral"} else {"None"}});
    settings
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| format!("{prefix}.{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn hatch_params(
    settings: &serde_json::Map<String, Value>,
    prefix: &str,
) -> Result<cuemol_render::HatchRenderParams> {
    let setting = |key: &str| {
        settings
            .get(&format!("{prefix}.{key}"))
            .or_else(|| settings.get(key))
    };
    let number = |key: &str, default: f64| setting(key).and_then(Value::as_f64).unwrap_or(default);
    let flag = |key: &str, default: bool| setting(key).and_then(Value::as_bool).unwrap_or(default);
    let text = |key: &str, default: &str| {
        setting(key)
            .and_then(Value::as_str)
            .unwrap_or(default)
            .to_string()
    };
    let color = |key: &str, default: [u8; 3]| -> Result<[u8; 3]> {
        let Some(value) = setting(key).and_then(Value::as_str) else {
            return Ok(default);
        };
        let hex = value.strip_prefix('#').unwrap_or(value);
        if hex.len() != 6 {
            return Err(CueMolError::invalid_argument("hatch color must be #rrggbb"));
        }
        let v = u32::from_str_radix(hex, 16)
            .map_err(|_| CueMolError::invalid_argument("invalid hatch color"))?;
        Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
    };
    let params = cuemol_render::HatchRenderParams {
        density: number("hatchDensity", 1.),
        width_scale: number("hatchWidthScale", 1.),
        ink: color("hatchInkColor", [0; 3])?,
        paper: color("hatchPaperColor", [255; 3])?,
        style: text("hatchStyle", "richardson"),
        coloring: text("hatchColoring", "Style default"),
        layers_spec: text("hatchLayersSpec", ""),
        tone_spec: text("hatchToneSpec", ""),
        custom_ink: flag("hatchCustomInk", setting("hatchInkColor").is_some()),
        custom_paper: flag("hatchCustomPaper", setting("hatchPaperColor").is_some()),
        default_edges: flag("hatchDefaultEdges", true),
        tone_strength: number("hatchToneStrength", 1.),
        tone_curve: number("hatchToneCurve", 1.),
        colored: false,
        base: String::new(),
        ink_model: String::new(),
    };
    cuemol_render::validate_hatch_params(&params)?;
    Ok(params)
}

pub fn hatch_settings(p: &cuemol_render::HatchRenderParams) -> String {
    let color = |v: [u8; 3]| format!("#{:02x}{:02x}{:02x}", v[0], v[1], v[2]);
    let settings = json!({"hatchStyle":p.style,"hatchColoring":p.coloring,"hatchLayersSpec":p.layers_spec,
        "hatchToneSpec":p.tone_spec,"hatchDensity":p.density,"hatchWidthScale":p.width_scale,
        "hatchCustomInk":p.custom_ink,"hatchCustomPaper":p.custom_paper,"hatchInkColor":color(p.ink),
        "hatchPaperColor":color(p.paper),"hatchDefaultEdges":p.default_edges,"hatchToneStrength":p.tone_strength,"hatchToneCurve":p.tone_curve});
    settings
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| format!("umbreon_npr.{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub struct PreparedRender {
    snapshot: cuemol_render::RenderSceneSnapshot,
    camera: Camera,
    background: [f32; 4],
    params: cuemol_render::UmbreonRenderParams,
    hatch: Option<cuemol_render::HatchRenderParams>,
    output: Option<String>,
}
impl PreparedRender {
    pub fn run_with_cancel(self, cancel: &std::sync::atomic::AtomicBool) -> Result<Value> {
        let frame = cuemol_render::render_umbreon_snapshot_with_cancel(
            &self.snapshot,
            &self.camera,
            self.background,
            &self.params,
            self.hatch.as_ref(),
            cancel,
        )?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(CueMolError::invalid_argument("Render cancelled"));
        }
        self.encode(frame)
    }

    pub fn run(self) -> Result<Value> {
        let frame = if let Some(hatch) = &self.hatch {
            cuemol_render::render_umbreon_npr_snapshot(
                &self.snapshot,
                &self.camera,
                self.background,
                &self.params,
                hatch,
            )?
        } else {
            cuemol_render::render_umbreon_snapshot(
                &self.snapshot,
                &self.camera,
                self.background,
                &self.params,
            )?
        };
        self.encode(frame)
    }
    #[cfg(target_arch = "wasm32")]
    pub async fn run_async(self) -> Result<Value> {
        self.run_async_with_cancel(None).await
    }
    #[cfg(target_arch = "wasm32")]
    pub async fn run_async_with_cancel(
        self,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<Value> {
        let frame = cuemol_render::render_umbreon_snapshot_async(
            &self.snapshot,
            &self.camera,
            self.background,
            &self.params,
            self.hatch.as_ref(),
            cancel,
        )
        .await?;
        self.encode(frame)
    }
    fn encode(self, frame: cuemol_render::UmbreonFrame) -> Result<Value> {
        let width = self.params.width;
        let height = self.params.height;
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, width as u32, height as u32);
            encoder.set_color(if frame.components == 4 {
                png::ColorType::Rgba
            } else {
                png::ColorType::Rgb
            });
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|e| CueMolError::Io(e.to_string()))?;
            writer
                .write_image_data(&frame.pixels)
                .map_err(|e| CueMolError::Io(e.to_string()))?;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = self.output {
            std::fs::write(&path, &png)?;
            Ok(json!({"path":path,"width":width,"height":height}))
        } else {
            Ok(
                json!({"width":width,"height":height,"_image":base64::engine::general_purpose::STANDARD.encode(png)}),
            )
        }
        #[cfg(target_arch = "wasm32")]
        Ok(
            json!({"width":width,"height":height,"fileName":self.output,"_image":base64::engine::general_purpose::STANDARD.encode(png)}),
        )
    }
}

pub(crate) fn render(
    state: &DesktopAppState,
    camera: &Camera,
    name: &str,
    args: &Value,
) -> Result<Value> {
    prepare_render(state, camera, name, args)?.run()
}
