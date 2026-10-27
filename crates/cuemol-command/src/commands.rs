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
    let ids = object.map(|id| vec![id]).unwrap_or_else(|| {
        state
            .object_summaries(state.active_scene().unwrap())
            .iter()
            .map(|o| o.uid)
            .collect()
    });
    let selector = selection
        .map(|s| state.parse_scene_selector(scene(state)?, s))
        .transpose()?;
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut found = false;
    for id in ids {
        let object = state
            .scene_manager()
            .object(id)
            .ok_or_else(|| CueMolError::not_found("molecule"))?;
        for atom in object.mol().atoms() {
            if selector
                .as_ref()
                .is_some_and(|sel| !sel.matches_in_molecule(atom, object.mol()))
            {
                continue;
            }
            let position = object.xform_matrix().transform_point(atom.position);
            for (i, v) in [position.x, position.y, position.z].into_iter().enumerate() {
                min[i] = min[i].min(v);
                max[i] = max[i].max(v);
            }
            found = true;
        }
    }
    if !found {
        return Err(CueMolError::not_found("atoms matching selection"));
    }
    camera.set_center(Vector4::new3(
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ));
    if zoom {
        let size =
            ((max[0] - min[0]).powi(2) + (max[1] - min[1]).powi(2) + (max[2] - min[2]).powi(2))
                .sqrt()
                .max(2.0);
        camera.set_zoom(size * 1.2);
        camera.set_cam_dist(size * 2.5);
        camera.set_slab_depth(size * 2.0);
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
            camera.set_rotation(Quat::from_axis_angle(axis,args["angle"].as_f64().unwrap().to_radians())?.mul_quat(camera.rotation()));
            Ok(view_json(camera))
        }
        "pan_view" => {
            let mut view=View::new(Uid::new(1),scene(state)?);view.set_camera(camera);view.set_view_size(800,600);
            let delta=Vector4::new3(args["dx"].as_f64().unwrap(),args["dy"].as_f64().unwrap(),args["dz"].as_f64().unwrap_or(0.));
            let scale=camera.zoom()/600.;
            let mut world=delta.scale(scale);camera.rotation().to_matrix().xform3d(&mut world);
            camera.set_center(camera.center()-world);Ok(view_json(camera))
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
        vec![
            ("sheet".into(), "SteelBlue".into()),
            ("helix".into(), "khaki".into()),
            ("*".into(), "FloralWhite".into()),
        ]
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
    let snapshot = state.active_render_snapshot("")?;
    let npr = args["style"]
        .as_str()
        .map(|style| style == "npr")
        .unwrap_or(settings.get("backend").is_some_and(|v| v == "umbreon_npr"));
    let prefix = if npr { "umbreon_npr" } else { "umbreon" };
    let setting = |key: &str| {
        settings
            .get(&format!("{prefix}.{key}"))
            .or_else(|| settings.get(key))
    };
    let number = |key: &str, default: f64| setting(key).and_then(Value::as_f64).unwrap_or(default);
    let flag = |key: &str, default: bool| setting(key).and_then(Value::as_bool).unwrap_or(default);
    let defaults = cuemol_render::UmbreonRenderParams::default();
    let params = cuemol_render::UmbreonRenderParams {
        width: width as i32,
        height: height as i32,
        perspective: settings
            .get("projection")
            .and_then(Value::as_str)
            .map(|p| p == "perspective")
            .unwrap_or(flag("perspective", camera.is_perspective())),
        use_clip_z: flag("clipPlane", flag("useClipZ", defaults.use_clip_z)),
        supersample: number("supersample", 1.).clamp(1., 4.) as i32,
        ao_samples: if flag("aoEnabled", setting("aoSamples").is_some()) {
            number("aoSamples", 64.).clamp(0., 256.) as i32
        } else {
            0
        },
        ao_distance: number("aoDistance", defaults.ao_distance),
        ao_intensity: number("aoIntensity", defaults.ao_intensity),
        shadows: flag("shadows", defaults.shadows),
        shadow_samples: number("shadowSamples", defaults.shadow_samples as f64).clamp(0., 256.)
            as i32,
        light_radius: number("lightRadius", defaults.light_radius),
        show_edge_lines: flag("edgeLines", flag("showEdgeLines", defaults.show_edge_lines)),
        crease_limit: number("creaseLimit", defaults.crease_limit),
        edge_rise: number("edgeRise", defaults.edge_rise),
        transparent_background: flag("transparentBg", flag("transparent", false)),
        use_gi: flag("useGI", flag("useGi", name == "render_image" && !npr)),
        gi_samples: number("giSamples", 8.).clamp(1., 256.) as i32,
        gi_intensity: number("giIntensity", defaults.gi_intensity),
        gi_env_intensity: number("giEnvIntensity", defaults.gi_env_intensity),
        gi_denoise: setting("denoise")
            .and_then(Value::as_str)
            .map(|v| v == "Bilateral")
            .unwrap_or(flag("giDenoise", defaults.gi_denoise)),
        denoiser: setting("denoise")
            .and_then(Value::as_str)
            .map(|v| i32::from(v == "A-trous"))
            .unwrap_or(number("denoiser", defaults.denoiser as f64) as i32),
    };
    let background = state.active_background_color()?;
    let hatch = if npr {
        let parse_color = |key: &str, default: [u8; 3]| -> Result<[u8; 3]> {
            let Some(value) = setting(key).and_then(Value::as_str) else {
                return Ok(default);
            };
            let hex = value.strip_prefix('#').unwrap_or(value);
            if hex.len() != 6 {
                return Err(CueMolError::invalid_argument("hatch color must be #rrggbb"));
            }
            let packed = u32::from_str_radix(hex, 16)
                .map_err(|_| CueMolError::invalid_argument("invalid hatch color"))?;
            Ok([(packed >> 16) as u8, (packed >> 8) as u8, packed as u8])
        };
        Some(cuemol_render::HatchRenderParams {
            density: number("hatchDensity", 1.),
            width_scale: number("hatchWidthScale", 1.),
            ink: if flag("hatchCustomInk", true) {
                parse_color("hatchInkColor", [0; 3])?
            } else {
                [0; 3]
            },
            paper: if flag("hatchCustomPaper", true) {
                parse_color("hatchPaperColor", [255; 3])?
            } else {
                [255; 3]
            },
            colored: setting("hatchColoring")
                .and_then(Value::as_str)
                .is_some_and(|v| v.eq_ignore_ascii_case("colored")),
        })
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

pub struct PreparedRender {
    snapshot: cuemol_render::RenderSceneSnapshot,
    camera: Camera,
    background: [f32; 4],
    params: cuemol_render::UmbreonRenderParams,
    hatch: Option<cuemol_render::HatchRenderParams>,
    output: Option<String>,
}
impl PreparedRender {
    pub fn run(self) -> Result<Value> {
        let width = self.params.width;
        let height = self.params.height;
        let frame = if let Some(hatch) = self.hatch {
            cuemol_render::render_umbreon_npr_snapshot(
                &self.snapshot,
                &self.camera,
                self.background,
                &self.params,
                &hatch,
            )?
        } else {
            cuemol_render::render_umbreon_snapshot(
                &self.snapshot,
                &self.camera,
                self.background,
                &self.params,
            )?
        };
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
        if let Some(path) = self.output {
            std::fs::write(&path, &png)?;
            Ok(json!({"path":path,"width":width,"height":height}))
        } else {
            Ok(
                json!({"width":width,"height":height,"_image":base64::engine::general_purpose::STANDARD.encode(png)}),
            )
        }
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
