use base64::Engine;
use cuemol_core::{CueMolError, Quat, Result, Uid, Vector4};
use cuemol_desktop_core::{DesktopAppState, GuiLoadedPath};
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
    renderer_owner_in_scene(state, renderer, scene(state)?)
}

pub(crate) fn renderer_owner_in_scene(
    state: &DesktopAppState,
    renderer: Uid,
    sid: Uid,
) -> Result<Uid> {
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
    Err(CueMolError::not_found("renderer in scene"))
}

pub fn view_json(camera: &Camera) -> Value {
    let center = camera.center();
    let q = camera.rotation();
    json!({"zoom":camera.zoom(),"slab":camera.slab_depth(),"distance":camera.cam_dist(),
        "center":[center.x,center.y,center.z],"rotation":[q.x,q.y,q.z,q.w],"perspective":camera.is_perspective()})
}

fn fit_geometry_bounds(
    state: &DesktopAppState,
    camera: &mut Camera,
    min: Vector4,
    max: Vector4,
    zoom: bool,
) {
    if zoom {
        let points = [min.x, max.x].into_iter().flat_map(|x| {
            [min.y, max.y].into_iter().flat_map(move |y| {
                [min.z, max.z]
                    .into_iter()
                    .map(move |z| Vector4::new3(x, y, z))
            })
        });
        let aspect = state
            .active_view_summary()
            .and_then(|view| state.command_manager().view(view.uid))
            .filter(|view| view.width() > 0 && view.height() > 0)
            .map_or(1.0, |view| {
                f64::from(view.width()) / f64::from(view.height())
            });
        camera.fit_positions(points, aspect);
    } else {
        camera.set_center((min + max).scale(0.5));
    }
}

pub(crate) fn fit(
    state: &DesktopAppState,
    camera: &mut Camera,
    object: Option<Uid>,
    selection: Option<&str>,
    zoom: bool,
) -> Result<()> {
    let scene_uid = scene(state)?;
    if object.is_some_and(|id| state.scene_manager().object(id).is_none())
        || (object.is_none()
            && state
                .object_summaries(scene_uid)
                .iter()
                .all(|object| object.atom_count == 0))
    {
        let snapshot = if let Some(id) = object {
            state.workspace_object_render_snapshot(id)?
        } else {
            state.active_render_snapshot("")?
        };
        let (min, max) = snapshot
            .bounds
            .ok_or_else(|| CueMolError::not_found("object geometry"))?;
        fit_geometry_bounds(state, camera, min, max, zoom);
        return Ok(());
    }
    let ids = object.map(|id| vec![id]).unwrap_or_else(|| {
        state
            .object_summaries(scene_uid)
            .iter()
            .map(|o| o.uid)
            .collect()
    });
    let objects = ids
        .into_iter()
        .map(|id| {
            let object = state
                .scene_manager()
                .object(id)
                .ok_or_else(|| CueMolError::not_found("molecule"))?;
            let selector = state.parse_object_selector(
                id,
                selection.unwrap_or_else(|| object.mol().selection_text()),
            )?;
            Ok((object, selector))
        })
        .collect::<Result<Vec<_>>>()?;
    let positions = objects.into_iter().flat_map(|(object, selection)| {
        object
            .mol()
            .select_indices(&selection)
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
        let class = state.scene_manager().object(object.uid).map_or("MolCoord", |object|object.object_type());
        json!({"id":object.uid.get(),"name":object.name,"className":class,"visible":object.visible,
            "atoms":object.atom_count,"residues":object.residue_count,"bonds":object.bond_count,"renderers":renderers})
    }).collect();
    objects.extend(crate::nodes::extra_scene_objects(state)?);
    let maps: Vec<_> = state
        .density_map_summaries(id)
        .into_iter()
        .map(|map| json!({"id":map.uid.get(),"name":map.name,"visible":map.visible}))
        .collect();
    let (scene_selections, mut named_selections) = state.selection_definitions();
    named_selections.extend(scene_selections);
    Ok(json!({"sceneId":id.get(),"objects":objects,"maps":maps,"namedSelections":named_selections}))
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
        "clear_undo" => {state.clear_all_scene_modified();Ok(json!({"cleared":true}))},
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
            state.scene_manager().scene(id).ok_or_else(|| CueMolError::not_found("scene"))?;
            if args["discardChanges"] != true && state.scene_is_modified(id) {
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
            let types=cuemol_desktop_core::renderer_setup::Target::for_object(state,id)?.options;
            Ok(json!({"rendererTypes":types.iter().filter(|t|!t.preset).map(|t|&t.name).collect::<Vec<_>>(),"presetTypes":types.iter().filter(|t|t.preset).map(|t|&t.name).collect::<Vec<_>>()}))
        }
        "create_renderer" | "create_preset_renderer" => {
            crate::file_commands::map_center_policy(args)?;
            let object=uid(args,"objId")?;
            let kind=text(args,if name=="create_renderer" {"rendererType"} else {"preset"})?;
            let target=cuemol_desktop_core::renderer_setup::Target::for_object(state,object)?;
            let preset=target.options.iter().find(|option|option.name==kind).map(|option|option.preset)
                .or_else(|| (name=="create_renderer" && target.kind==cuemol_desktop_core::renderer_setup::Kind::Map && matches!(kind,"*unitcell"|"unitcell"|"gpu_mapmesh")).then_some(false))
                .ok_or_else(||CueMolError::invalid_argument("incompatible renderer type"))?;
            let group=args["group"].as_str().map(str::trim).filter(|name|!name.is_empty());
            if let Some(group)=group {
                if preset {return Err(CueMolError::invalid_argument("preset renderers cannot be nested in another group"));}
                if !target.groups.iter().any(|name|name==group) {return Err(CueMolError::not_found("renderer group in target object"));}
            }
            if !preset && target.kind==cuemol_desktop_core::renderer_setup::Kind::Molecule {
                if let Some(selection)=args["selection"].as_str() {state.parse_object_selector(object,selection)?;}
            }
            let label=args["name"].as_str().map(str::to_string).unwrap_or(state.suggest_renderer_name(target.scene,kind)?);
            let renderer=if state.density_map_summary(object).is_some() {
                let id=state.add_density_renderer(object,kind)?;
                state.rename_density_renderer(object,id,&label)?; id
            } else if state.surface_summary(object).is_some() {
                if kind!="molsurf" { return Err(CueMolError::invalid_argument("surface objects require a molsurf renderer")); }
                let id=state.add_surface_renderer(object)?;
                state.rename_surface_renderer(object,id,&label)?; id
            } else if state.lw_object_summary(object).is_some() {
                if kind!="lwrend" { return Err(CueMolError::invalid_argument("lightweight objects require a lwrend renderer")); }
                let id=state.add_lw_renderer(object)?;
                state.rename_lw_renderer(object,id,&label)?; id
            } else {state.new_renderer_from_setup(target.scene,object,kind,&label,args["selection"].as_str())?};
            if let Some(selection)=args["selection"].as_str().filter(|_| state.scene_manager().object(object).is_none()) {
                state.set_auxiliary_renderer_property(object,renderer,"sel",selection)?;
            }
            if group.is_some() {state.set_workspace_renderer_group_name(object,renderer,group)?;}
            if state.density_map_summary(object).is_some() {
                crate::file_commands::apply_map_renderer_center_policy(state,camera,object,&[renderer],args)?;
            } else if args["centerView"] == true {
                if target.kind==cuemol_desktop_core::renderer_setup::Kind::Molecule {
                    let mut view=crate::properties::current_fit_view(state,camera)?;
                    state.fit_molecule_view(object,Some(renderer),&mut view)?;
                    *camera=view.camera();
                } else {
                    let has_data=state.scene_manager().scene(target.scene)
                        .and_then(|scene|scene.loaded_lw_object(object))
                        .is_none_or(|object| !object.raw_data.is_empty());
                    if has_data {
                        if let Some((min,max))=state.workspace_object_render_snapshot(object)?.bounds {
                            fit_geometry_bounds(state,camera,min,max,true);
                        }
                    }
                }
            }
            Ok(json!({"objectId":object.get(),"rendererId":renderer.get(),"name":label}))
        }
        "set_renderer_type" => {
            let id=uid(args,"rendId")?;let object=renderer_owner(state,id)?;
            let changed=state.change_renderer_type_in_object(object,id,text(args,"rendererType")?)?;
            Ok(json!({"rendererId":changed.get()}))
        }
        "set_renderer_selection" => {
            let id=uid(args,"rendId")?;let object=renderer_owner(state,id)?;
            if state.scene_manager().object(object).is_some() {
                state.set_renderer_selection_text(object,id,text(args,"selection")?)?;
            } else { state.set_auxiliary_renderer_property(object,id,"sel",text(args,"selection")?)?; }
            Ok(json!({"rendererId":id.get()}))
        }
        "set_visible" | "delete_node" | "rename_node" => crate::nodes::edit_node(state,name,args),
        "count_selection" | "list_chains" | "list_residues" => {
            let object=state.scene_manager().object(uid(args,"objId")?).ok_or_else(||CueMolError::not_found("molecule"))?;
            match name {
                "count_selection" => {let count=object.mol().atom_sel_count(&state.parse_object_selector(object.uid(),text(args,"selection")?)?);Ok(json!({"valid":true,"atomCount":count,"count":count}))},
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
        "focus_selection" | "orient_view" => crate::view_commands::focus_selection(state, camera, args, name == "orient_view"),
        "center_view" => {fit(state,camera,Some(uid(args,"objId")?),args["selection"].as_str(),args["zoom"]==true)?;Ok(view_json(camera))}
        "get_view" => Ok(view_json(camera)),
        "set_view" => crate::view_commands::set_view(state,camera,args),
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
        "save_scene" => {let options=crate::file_commands::scene_save_options(state,args)?;state.save_active_scene_qsc_with_camera(text(args,"path")?,camera.clone(),Some(options))?;Ok(json!({"path":args["path"]}))}
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
        "set_frame" => {let object=uid(args,"objId")?;let frame=integer(args,"frame",0)?;if state.object_has_trajectory(object){state.apply_trajectory_frame(object,frame)?;}else{state.apply_morph_frame(object,frame as f64)?;}Ok(json!({"frame":args["frame"]}))}
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

/// Resolve a render view without changing the application's active scene or camera.
pub fn render_target(
    state: &DesktopAppState,
    active_camera: &Camera,
    args: &Value,
) -> Result<(cuemol_core::Uid, cuemol_core::Uid, Camera)> {
    let uid = |key: &str| -> Result<Option<cuemol_core::Uid>> {
        if args[key].is_null() {
            return Ok(None);
        }
        args[key]
            .as_u64()
            .filter(|id| *id > 0)
            .map(cuemol_core::Uid::new)
            .map(Some)
            .ok_or_else(|| {
                CueMolError::invalid_argument(format!("{key} must be a positive integer"))
            })
    };
    let requested_scene = uid("sceneId")?;
    let requested_view = uid("viewId")?;
    let view_scene = requested_view
        .map(|id| {
            state
                .command_manager()
                .view(id)
                .map(|view| view.scene_uid())
                .ok_or_else(|| CueMolError::not_found("render view"))
        })
        .transpose()?;
    if requested_scene.zip(view_scene).is_some_and(|(a, b)| a != b) {
        return Err(CueMolError::invalid_argument(
            "render view does not belong to the requested scene",
        ));
    }
    let scene_uid = requested_scene
        .or(view_scene)
        .or(state.active_scene())
        .ok_or_else(|| CueMolError::not_found("render scene"))?;
    let scene = state
        .scene_manager()
        .scene(scene_uid)
        .ok_or_else(|| CueMolError::not_found("render scene"))?;
    let view_uid = requested_view
        .or(scene.active_view_uid())
        .ok_or_else(|| CueMolError::not_found("render view"))?;
    let camera = if state
        .active_view_summary()
        .is_some_and(|view| view.uid == view_uid)
    {
        active_camera.clone()
    } else {
        state
            .view_camera(view_uid)
            .ok_or_else(|| CueMolError::not_found("render camera"))?
    };
    Ok((scene_uid, view_uid, camera))
}

pub fn prepare_render(
    state: &DesktopAppState,
    camera: &Camera,
    name: &str,
    args: &Value,
) -> Result<PreparedRender> {
    prepare_render_inner(state, camera, name, args, None)
}

pub fn prepare_umbreon_export(
    state: &DesktopAppState,
    camera: &Camera,
    target: &Value,
    options: cuemol_desktop_core::GuiPngExportOptions,
) -> Result<PreparedRender> {
    crate::image_export::ImageExportTarget::capture(state, camera, target)?
        .prepare_umbreon(state, options)
}

pub(crate) fn prepare_umbreon_export_target(
    state: &DesktopAppState,
    target: &crate::image_export::ImageExportTarget,
    options: cuemol_desktop_core::GuiPngExportOptions,
) -> Result<PreparedRender> {
    options.validate()?;
    let mut args = target.args();
    args["width"] = json!(options.width);
    args["height"] = json!(options.height);
    args["style"] = json!("umbreon");
    prepare_render_inner(
        state,
        &target.camera,
        "export_umbreon",
        &args,
        Some((options, target)),
    )
}

fn prepare_render_inner(
    state: &DesktopAppState,
    camera: &Camera,
    name: &str,
    args: &Value,
    image_export: Option<(
        cuemol_desktop_core::GuiPngExportOptions,
        &crate::image_export::ImageExportTarget,
    )>,
) -> Result<PreparedRender> {
    let (scene_uid, view_uid, camera) = render_target(state, camera, args)?;
    let camera = image_export.map_or(camera, |(_, target)| target.camera.clone());
    let settings = crate::properties::scene_render_settings(state, scene_uid)?;
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
    let output = if name == "render_image" {
        Some(text(args, "fileName")?.to_string())
    } else {
        None
    };
    let dpi = image_export
        .map(|(options, _)| options.resolution_dpi)
        .unwrap_or_else(|| {
            settings
                .get("dpi")
                .and_then(Value::as_f64)
                .unwrap_or(600.)
                .clamp(1., 4800.)
        });
    if args["style"].is_null()
        && settings
            .get("backend")
            .is_some_and(|value| value == "povray")
    {
        #[cfg(not(target_arch = "wasm32"))]
        return Ok(PreparedRender {
            backend: PreparedBackend::Pov(crate::pov_render::PreparedPovRender::prepare(
                state,
                scene_uid,
                &camera,
                &settings,
                [width as u32, height as u32],
            )?),
            output,
            dpi,
        });
        #[cfg(target_arch = "wasm32")]
        return Err(CueMolError::invalid_argument("POV-Ray runs in the native application; choose Umbreon or Umbreon (NPR) in the browser"));
    }
    let mut snapshot = state.render_snapshot(scene_uid, "")?;
    state.apply_scene_color_proof_for(scene_uid, &mut snapshot)?;
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
    if let Some((options, _)) = image_export {
        params.perspective = camera.is_perspective();
        params.transparent_background = options.transparent;
    }
    if image_export.is_none()
        && name != "render_image"
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
    params.swap_stereo_eyes = state
        .command_manager()
        .view(view_uid)
        .is_some_and(|view| view.swap_stereo_eyes());
    if let Some((_, target)) = image_export {
        params.swap_stereo_eyes = target.swap_stereo_eyes;
    }
    let background = state.scene_background_color(scene_uid)?;
    let hatch = if npr {
        Some(hatch_params(&settings, prefix)?)
    } else {
        None
    };
    Ok(PreparedRender {
        backend: PreparedBackend::Umbreon {
            snapshot,
            camera,
            background,
            params,
            hatch,
        },
        output,
        dpi,
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
    params.gi_bounces = number("giBounces", params.gi_bounces as f64).clamp(1., 8.) as i32;
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

enum PreparedBackend {
    Umbreon {
        snapshot: cuemol_render::RenderSceneSnapshot,
        camera: Camera,
        background: [f32; 4],
        params: cuemol_render::UmbreonRenderParams,
        hatch: Option<cuemol_render::HatchRenderParams>,
    },
    #[cfg(not(target_arch = "wasm32"))]
    Pov(crate::pov_render::PreparedPovRender),
}

pub struct PreparedRender {
    backend: PreparedBackend,
    output: Option<String>,
    dpi: f64,
}
impl PreparedRender {
    pub fn in_memory(mut self) -> Self {
        self.output = None;
        self
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn to_file(mut self, path: &std::path::Path) -> Self {
        self.output = Some(path.to_string_lossy().into_owned());
        self
    }
    pub fn with_dpi(mut self, dpi: f64) -> Self {
        self.dpi = dpi.clamp(1., 4800.);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn pov_progress(&self) -> Option<crate::pov::PovProgress> {
        match &self.backend {
            PreparedBackend::Pov(render) => Some(render.progress()),
            _ => None,
        }
    }

    pub fn run_with_cancel(self, cancel: &std::sync::atomic::AtomicBool) -> Result<Value> {
        let frame = self.render_frame(cancel)?;
        self.encode(frame)
    }

    pub fn run_png_with_cancel(self, cancel: &std::sync::atomic::AtomicBool) -> Result<Vec<u8>> {
        let frame = self.render_frame(cancel)?;
        self.encode_png(&frame)
    }

    fn render_frame(
        &self,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<cuemol_render::UmbreonFrame> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(CueMolError::invalid_argument("Render cancelled"));
        }
        let frame = match &self.backend {
            PreparedBackend::Umbreon {
                snapshot,
                camera,
                background,
                params,
                hatch,
            } => cuemol_render::render_umbreon_snapshot_with_cancel(
                snapshot,
                camera,
                *background,
                params,
                hatch.as_ref(),
                cancel,
            )?,
            #[cfg(not(target_arch = "wasm32"))]
            PreparedBackend::Pov(render) => render.run(cancel)?,
        };
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(CueMolError::invalid_argument("Render cancelled"));
        }
        Ok(frame)
    }

    pub fn run(self) -> Result<Value> {
        self.run_with_cancel(&std::sync::atomic::AtomicBool::new(false))
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
        let PreparedBackend::Umbreon {
            snapshot,
            camera,
            background,
            params,
            hatch,
        } = &self.backend;
        let frame = cuemol_render::render_umbreon_snapshot_async(
            snapshot,
            camera,
            *background,
            params,
            hatch.as_ref(),
            cancel,
        )
        .await?;
        self.encode(frame)
    }
    fn encode_png(&self, frame: &cuemol_render::UmbreonFrame) -> Result<Vec<u8>> {
        let width = frame.width;
        let height = frame.height;
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, width as u32, height as u32);
            encoder.set_color(if frame.components == 4 {
                png::ColorType::Rgba
            } else {
                png::ColorType::Rgb
            });
            encoder.set_depth(png::BitDepth::Eight);
            let pixels_per_meter = (self.dpi / 0.0254).round() as u32;
            encoder.set_pixel_dims(Some(png::PixelDimensions {
                xppu: pixels_per_meter,
                yppu: pixels_per_meter,
                unit: png::Unit::Meter,
            }));
            let mut writer = encoder
                .write_header()
                .map_err(|e| CueMolError::Io(e.to_string()))?;
            writer
                .write_image_data(&frame.pixels)
                .map_err(|e| CueMolError::Io(e.to_string()))?;
        }
        Ok(png)
    }

    fn encode(self, frame: cuemol_render::UmbreonFrame) -> Result<Value> {
        let width = frame.width;
        let height = frame.height;
        let png = self.encode_png(&frame)?;
        #[cfg(not(target_arch = "wasm32"))]
        let log = self.pov_progress().map(|progress| progress.text());
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = self.output {
            std::fs::write(&path, &png)?;
            Ok(json!({"path":path,"width":width,"height":height}))
        } else {
            Ok(
                json!({"width":width,"height":height,"_image":base64::engine::general_purpose::STANDARD.encode(png),"_log":log}),
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

#[cfg(test)]
mod render_target_tests {
    use super::*;
    use cuemol_core::Uid;

    #[test]
    fn umbreon_image_export_overrides_backend_projection_and_output_without_editing_settings() {
        let mut state = DesktopAppState::new();
        state.ensure_active_scene();
        let mut camera = Camera::new();
        camera.set_perspective(false);
        call_tool(&mut state, &mut camera, "set_render_settings", &json!({"settings":"backend=povray; width=200; height=100; dpi=600; projection=perspective; transparent=false; umbreon.useGI=false; umbreon.supersample=1"})).unwrap();
        let before = state.clone();
        let options = cuemol_desktop_core::GuiPngExportOptions {
            width: 32,
            height: 24,
            resolution_dpi: 150.,
            transparent: true,
        };
        let render = prepare_umbreon_export(&state, &camera, &json!({}), options).unwrap();
        let PreparedBackend::Umbreon { params, hatch, .. } = &render.backend else {
            panic!("exporter must be Umbreon")
        };
        assert_eq!((params.width, params.height, render.dpi), (32, 24, 150.));
        assert!(!params.perspective);
        assert!(params.transparent_background);
        assert!(!params.use_gi);
        assert!(hatch.is_none());
        let result = render.run().unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(result["_image"].as_str().unwrap())
            .unwrap();
        let reader = png::Decoder::new(bytes.as_slice()).read_info().unwrap();
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
        assert_eq!(reader.info().pixel_dims.unwrap().xppu, 5906);
        assert!(state.same_undo_state(&before));
    }

    #[test]
    fn selected_render_scene_owns_geometry_settings_background_proof_and_camera() {
        let mut state = DesktopAppState::new();
        state.open_bytes("first.pdb", b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n").unwrap();
        let first = state.active_scene().unwrap();
        let first_view = state.active_view_summary().unwrap().uid;
        let mut camera = Camera::new();
        camera.set_zoom(8.);
        state.set_active_view_camera(&camera).unwrap();
        call_tool(&mut state, &mut camera, "set_render_settings", &json!({"settings":"width=64; height=48; unit=px; dpi=72; backend=umbreon; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1"})).unwrap();
        let scene = state.scene_manager_mut().scene_mut(first).unwrap();
        scene.set_scene_attribute("bgcolor", "#ff0000");
        scene.set_scene_attribute("use_colproof", "true");
        let mut expected = state.render_snapshot(first, "").unwrap();
        state.apply_scene_color_proof(&mut expected).unwrap();
        let second = state.new_scene("other", true).result_scene;
        let second_view = state.active_view_summary().unwrap().uid;
        camera.set_zoom(300.);
        state.set_active_view_camera(&camera).unwrap();
        let before = state.clone();
        let args = json!({"sceneId":first.get(), "viewId":first_view.get()});
        let render = prepare_render(&state, &camera, "get_screenshot", &args).unwrap();
        let PreparedBackend::Umbreon {
            snapshot,
            camera: saved_camera,
            background,
            params,
            ..
        } = &render.backend
        else {
            panic!("unexpected backend")
        };
        assert_eq!(*snapshot, expected);
        assert_eq!(saved_camera.zoom(), 8.);
        assert_eq!(*background, [1., 0., 0., 1.]);
        assert_eq!((params.width, params.height, render.dpi), (64, 48, 72.));
        assert_eq!(state.active_scene(), Some(second));
        assert!(before.same_undo_state(&state));
        assert_eq!(
            crate::render_history::RenderSource::capture_scene(&state, first)
                .unwrap()
                .settings["dpi"],
            72.
        );
        let default = prepare_render(&state, &camera, "get_screenshot", &json!({})).unwrap();
        let PreparedBackend::Umbreon {
            snapshot,
            camera: render_camera,
            ..
        } = &default.backend
        else {
            panic!("unexpected backend")
        };
        assert_eq!(render_camera.zoom(), 300.);
        assert_eq!(snapshot.atom_count(), 0);
        for bad in [
            json!({"sceneId":first.get(),"viewId":second_view.get()}),
            json!({"sceneId":0}),
            json!({"viewId":-1}),
            json!({"viewId":"1"}),
            json!({"sceneId":u64::MAX}),
            json!({"viewId":u64::MAX}),
        ] {
            assert!(prepare_render(&state, &camera, "get_screenshot", &bad).is_err());
        }
        assert_eq!(
            render_target(&state, &camera, &json!({"viewId":first_view.get()}))
                .unwrap()
                .0,
            first
        );
        assert_eq!(
            render_target(&state, &camera, &json!({"sceneId":first.get()}))
                .unwrap()
                .1,
            first_view
        );
        assert_eq!(
            render_target(&state, &camera, &json!({"viewId":second_view.get()}))
                .unwrap()
                .2
                .zoom(),
            300.
        );
        assert!(render_target(&state, &camera, &json!({"sceneId":Uid::new(0).get()})).is_err());
    }
}
