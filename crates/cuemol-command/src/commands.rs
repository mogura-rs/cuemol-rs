use base64::Engine;
use cuemol_core::{CueMolError, Quat, Result, Uid, Vector4};
use cuemol_desktop_core::{DesktopAppState, GuiLoadedPath, GuiRendererSetup, GuiSceneSaveOptions};
use cuemol_molstr::parse_selector;
use cuemol_qsys::{Camera, View};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args[key]
        .as_str()
        .ok_or_else(|| CueMolError::invalid_argument(format!("missing string: {key}")))
}
fn uid(args: &Value, key: &str) -> Result<Uid> {
    args[key]
        .as_u64()
        .filter(|id| *id > 0)
        .map(Uid::new)
        .ok_or_else(|| CueMolError::invalid_argument(format!("invalid identifier: {key}")))
}
fn integer(args: &Value, key: &str, default: usize) -> Result<usize> {
    if args[key].is_null() {
        return Ok(default);
    }
    args[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| CueMolError::invalid_argument(format!("invalid nonnegative integer: {key}")))
}
fn scene(state: &DesktopAppState) -> Result<Uid> {
    state
        .active_scene()
        .ok_or_else(|| CueMolError::not_found("active scene"))
}
fn renderer_owner(state: &DesktopAppState, renderer: Uid) -> Result<Uid> {
    let scene = state
        .scene_manager()
        .scene(scene(state)?)
        .ok_or_else(|| CueMolError::not_found("scene"))?;
    scene
        .objects()
        .iter()
        .find(|object| object.renderer(renderer).is_some())
        .map(|object| object.uid())
        .ok_or_else(|| CueMolError::not_found("renderer in active scene"))
}

pub fn view_json(camera: &Camera) -> Value {
    let center = camera.center();
    let q = camera.rotation();
    json!({"zoom":camera.zoom(),"slab":camera.slab_depth(),"distance":camera.cam_dist(),
        "center":[center.x,center.y,center.z],"rotation":[q.x,q.y,q.z,q.w],"perspective":camera.is_perspective()})
}

fn fit(
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
    let selector = selection.map(parse_selector).transpose()?;
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
            for (i, v) in [atom.position.x, atom.position.y, atom.position.z]
                .into_iter()
                .enumerate()
            {
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

fn scene_state(state: &DesktopAppState) -> Result<Value> {
    let id = scene(state)?;
    let objects: Vec<_> = state.object_summaries(id).into_iter().map(|object| {
        let renderers: Vec<_> = state.renderer_summaries(object.uid).into_iter().filter(|renderer|renderer.renderer_type != "*selection").map(|renderer| json!({
            "id":renderer.uid.get(),"name":renderer.name,"type":renderer.renderer_type,"visible":renderer.visible,"group":renderer.group
        })).collect();
        json!({"id":object.uid.get(),"name":object.name,"className":"MolCoord","visible":object.visible,
            "atoms":object.atom_count,"residues":object.residue_count,"bonds":object.bond_count,"renderers":renderers})
    }).collect();
    let maps: Vec<_> = state
        .density_map_summaries(id)
        .into_iter()
        .map(|map| json!({"id":map.uid.get(),"name":map.name,"visible":map.visible}))
        .collect();
    Ok(json!({"sceneId":id.get(),"objects":objects,"maps":maps,"namedSelections":[]}))
}

fn loaded_result(loaded: GuiLoadedPath) -> Value {
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
        "load_file" => {
            let path=text(args,"path")?;
            let loaded=if let Some(kind)=args["preset"].as_str().or_else(|| args["rendererType"].as_str()) {
                let fallback=std::path::Path::new(path).file_stem().and_then(|s|s.to_str()).unwrap_or("molecule");
                state.open_path_with_renderer_setup(path,&GuiRendererSetup {
                    object_name:args["name"].as_str().unwrap_or(fallback).into(), renderer_type:kind.into(),
                    renderer_name:args["rendererName"].as_str().unwrap_or(kind).into(),
                    selection:args["selection"].as_str().map(str::to_string), recenter:args["centerView"] != false,
                })?
            } else { state.open_path(path)? };
            if let GuiLoadedPath::Object {object_uid,..} = &loaded {
                if let Some(name)=args["name"].as_str() {
                    state.scene_manager_mut().object_mut(*object_uid).unwrap().set_name(name);
                }
                if args["centerView"] != false { fit(state,camera,Some(*object_uid),args["selection"].as_str(),true)?; }
            } else if let Some(saved)=state.active_view_camera() { *camera=saved; }
            Ok(loaded_result(loaded))
        }
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
        "set_visible" | "delete_node" | "rename_node" => {
            let id=uid(args,"nodeId")?;let kind=text(args,"nodeType")?;
            if kind=="object" {
                match name {
                    "set_visible" => {state.set_object_visible(id,args["visible"]==true)?;}
                    "delete_node" => {if !state.destroy_object(id)? {return Err(CueMolError::not_found("object"));}}
                    _ => {let object=state.scene_manager_mut().object_mut(id).ok_or_else(||CueMolError::not_found("object"))?;object.set_name(text(args,"name")?);object.set_modified(true);}
                }
            } else {
                let object=renderer_owner(state,id)?;
                match name {
                    "set_visible" => {state.set_renderer_visible(object,id,args["visible"]==true)?;}
                    "delete_node" => {state.destroy_renderer_in_object(object,id)?;}
                    _ => {state.scene_manager_mut().object_mut(object).unwrap().renderer_mut(id).unwrap().set_name(text(args,"name")?);}
                }
            }
            Ok(json!({"nodeId":id.get()}))
        }
        "count_selection" | "list_chains" | "list_residues" => {
            let object=state.scene_manager().object(uid(args,"objId")?).ok_or_else(||CueMolError::not_found("molecule"))?;
            match name {
                "count_selection" => Ok(json!({"count":object.mol().select_indices(&parse_selector(text(args,"selection")?)?).len()})),
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
            {let (log,selection)=state.run_script_js(text(args,"source")?)?;Ok(json!({"log":log,"selection":selection}))}
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
            let id=uid(args,"rendId")?;let object=renderer_owner(state,id)?;let prop=text(args,"prop")?;
            if name=="set_renderer_property" {state.set_renderer_property(object,id,prop,text(args,"value")?)?;}
            Ok(json!({"prop":prop,"value":state.renderer_property(object,id,prop)}))
        }
        "list_coloring_styles" => Ok(json!({"styles":["DefaultCPKColoring","DefaultHSCPaint","DefaultRainbowColoring","DefaultBfacColoring"]})),
        "set_renderer_coloring" => set_coloring(state,args),
        _ => Err(CueMolError::not_found(format!("tool {name}"))),
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
        "style" => {
            if target != "renderer" {
                return Err(CueMolError::invalid_argument(
                    "Coloring styles require a renderer",
                ));
            }
            let object = renderer_owner(state, id)?;
            state.set_renderer_property(object, id, "style", text(args, "styleName")?)?;
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

fn render(state: &DesktopAppState, camera: &Camera, name: &str, args: &Value) -> Result<Value> {
    let width = if name == "capture_view" {
        integer(args, "longSide", 640)?.clamp(64, 2048)
    } else {
        integer(args, "width", 640)?
    };
    let height = if name == "capture_view" {
        width * 3 / 4
    } else {
        integer(args, "height", 480)?
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
    let params = cuemol_render::UmbreonRenderParams {
        width: width as i32,
        height: height as i32,
        perspective: camera.is_perspective(),
        supersample: 1,
        use_gi: args["style"] == "gi",
        gi_samples: 8,
        ..Default::default()
    };
    let frame =
        cuemol_render::render_umbreon_snapshot(&snapshot, camera, [0., 0., 0., 1.], &params)?;
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
    if name == "render_image" {
        std::fs::write(text(args, "fileName")?, &png)?;
        Ok(json!({"path":args["fileName"],"width":width,"height":height}))
    } else {
        Ok(
            json!({"width":width,"height":height,"_image":base64::engine::general_purpose::STANDARD.encode(png)}),
        )
    }
}
