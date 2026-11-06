use crate::animation_graph::{editor_time_ms, Graph};
use crate::commands::{scene, text};
use cuemol_anim::{AnimMgr, AnimMotion};
use cuemol_core::{
    time_from_milliseconds, time_to_milliseconds, CueMolError, LDom2Node, Result, Uid,
};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};

fn element(mgr: &AnimMgr, spec: &str) -> Result<usize> {
    let index = if let Some(uid) = spec.strip_prefix('#').and_then(|s| s.parse::<u64>().ok()) {
        mgr.motions()
            .iter()
            .position(|m| m.base().uid().get() == uid)
    } else if let Ok(index) = spec.parse::<usize>() {
        index.checked_sub(1).filter(|i| *i < mgr.size())
    } else {
        mgr.motions().iter().position(|m| m.base().name() == spec)
    };
    index.ok_or_else(|| CueMolError::not_found(format!("animation element {spec}")))
}

fn seed_start_camera(
    state: &mut DesktopAppState,
    camera: &Camera,
    sid: Uid,
    mgr: &mut AnimMgr,
) -> Result<()> {
    if state
        .scene_manager()
        .scene(sid)
        .and_then(|s| s.camera_by_name("__current"))
        .is_none()
    {
        let mut saved = camera.clone();
        saved.set_name("__current");
        state.save_scene_camera(sid, saved, false)?;
    }
    if mgr.startcam().is_empty() {
        mgr.set_startcam("__current");
    }
    Ok(())
}

pub fn initialize_animation_start_camera(
    state: &mut DesktopAppState,
    camera: &Camera,
) -> Result<()> {
    let sid = scene(state)?;
    let mut mgr = state.scene_animation(sid)?.unwrap_or_default();
    seed_start_camera(state, camera, sid, &mut mgr)?;
    state.set_scene_animation(sid, Some(&mgr))
}

/// Keep the reference's implicit camera initialization outside the element undo step.
pub fn retain_animation_seed_in_undo(
    before: &mut DesktopAppState,
    after: &DesktopAppState,
    tool: &str,
) {
    if tool != "add_anim" || before.active_scene() != after.active_scene() {
        return;
    }
    let Some(sid) = after.active_scene() else {
        return;
    };
    let Some(scene) = after.scene_manager().scene(sid) else {
        return;
    };
    let Some(camera) = scene.camera_by_name("__current") else {
        return;
    };
    let Ok(Some(current)) = after.scene_animation(sid) else {
        return;
    };
    let Ok(previous) = before.scene_animation(sid) else {
        return;
    };
    let mut previous = previous.unwrap_or_default();
    if before
        .scene_manager()
        .scene(sid)
        .and_then(|s| s.camera_by_name("__current"))
        .is_none()
    {
        if before
            .save_scene_camera(sid, camera.clone(), false)
            .is_err()
        {
            return;
        }
    }
    if previous.startcam().is_empty() && current.startcam() == "__current" {
        previous.set_startcam("__current");
        let _ = before.set_scene_animation(sid, Some(&previous));
    }
}
pub fn animation_timeline(state: &DesktopAppState) -> Result<Value> {
    let sid = scene(state)?;
    let mgr = state.scene_animation(sid)?.unwrap_or_default();
    Ok(timeline(state, &mgr, sid))
}

fn timeline(state: &DesktopAppState, mgr: &AnimMgr, sid: Uid) -> Value {
    let play_state = if state.is_scene_animation_playing(sid) {
        "playing"
    } else if mgr.play_state() == cuemol_anim::AnimPlayState::Pause {
        "paused"
    } else {
        "stopped"
    };
    let graph = Graph::read(mgr);
    let elements = mgr.motions().iter().enumerate().map(|(index, motion)| {
        let base = motion.base();
        let node = &graph.nodes[index];
        let mut entry = json!({
            "number": index + 1, "uid": base.uid().get(), "name": base.name(),
            "type": motion.type_name(), "startMs": time_to_milliseconds(base.rel_start()),
            "endMs": time_to_milliseconds(base.rel_end()), "absStartMs": node.abs_start_ms.unwrap_or_else(|| time_to_milliseconds(base.abs_start())),
            "absEndMs": node.abs_end_ms.unwrap_or_else(|| time_to_milliseconds(base.abs_end())), "timeRefName": base.time_ref_name(),
            "disabled": base.disabled(), "quadric": base.quadric(), "timeRefState": node.state
        });
        if let Some(error) = &node.error { entry["resolveError"] = json!(error); }
        entry
    }).collect::<Vec<_>>();
    let mut data = json!({"sceneId":sid.get(), "fps":30, "cameras":state.camera_summaries(sid).into_iter().map(|camera| camera.name).collect::<Vec<_>>(), "mgr": {"lengthMs": time_to_milliseconds(mgr.length()), "loop": mgr.loop_enabled(),
        "startcam": mgr.startcam(), "playState": play_state,
        "elapsedMs": state.scene_animation_elapsed_ms(sid)}, "elements": elements});
    if let Some(error) = graph.error() {
        data["resolveError"] = json!(error);
    }
    data
}

pub fn call_tool(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    name: &str,
    args: &Value,
) -> Result<Value> {
    if !matches!(
        name,
        "list_anims"
            | "add_anim"
            | "remove_anim"
            | "move_anim"
            | "set_anim_time"
            | "set_anim_prop"
            | "set_anim_generic_prop"
            | "reset_anim_prop"
            | "set_anim_options"
            | "animate"
    ) {
        return crate::file_commands::call_tool(state, camera, name, args);
    }
    let sid = scene(state)?;
    let mut mgr = state.scene_animation(sid)?.unwrap_or_default();
    if name == "list_anims" {
        return Ok(timeline(state, &mgr, sid));
    }
    if name == "animate" {
        let action = text(args, "action")?;
        if !matches!(action, "play" | "pause" | "stop" | "seek") {
            return Err(CueMolError::invalid_argument("unknown animation action"));
        }
        if action == "play" && mgr.length() <= 0 {
            return Err(CueMolError::invalid_argument("animation has no duration"));
        }
        if action == "seek"
            && !args["timeMs"]
                .as_f64()
                .is_some_and(|n| n.is_finite() && n >= 0.)
        {
            return Err(CueMolError::invalid_argument(
                "seek requires a nonnegative timeMs",
            ));
        }
        if matches!(action, "play" | "seek") {
            if let Some(error) = Graph::read(&mgr).error() {
                return Err(CueMolError::invalid_argument(error));
            }
        }
        advance_animation(state, camera)?;
        mgr = state.scene_animation(sid)?.unwrap_or_default();
        if mgr.startcam().is_empty() && matches!(action, "play" | "seek") {
            let mut start = mgr
                .initial_camera()
                .cloned()
                .unwrap_or_else(|| camera.clone());
            start.set_name("__animation_start");
            state.save_scene_camera(sid, start, true)?;
            mgr.set_startcam("__animation_start");
            state.set_scene_animation(sid, Some(&mgr))?;
        }
        match action {
            "play" => {
                state.set_active_view_camera(camera)?;
                let view = state
                    .active_view_summary()
                    .ok_or_else(|| CueMolError::not_found("view"))?;
                state.play_scene_animation(sid, view.uid)?;
            }
            "pause" => {
                if let Some(updated) = state.pause_scene_animation(sid)? {
                    *camera = updated;
                }
            }
            "stop" => {
                let view = state
                    .active_view_summary()
                    .ok_or_else(|| CueMolError::not_found("view"))?;
                *camera = state.stop_scene_animation(sid, view.uid)?;
            }
            "seek" => {
                let time = args["timeMs"]
                    .as_f64()
                    .ok_or_else(|| CueMolError::invalid_argument("seek requires timeMs"))?;
                if !time.is_finite() || time < 0. {
                    return Err(CueMolError::invalid_argument("invalid animation time"));
                }
                apply_time(
                    state,
                    camera,
                    sid,
                    time.min(time_to_milliseconds(mgr.length())),
                )?;
            }
            _ => return Err(CueMolError::invalid_argument("unknown animation action")),
        }
        mgr = state.scene_animation(sid)?.unwrap_or_default();
        return Ok(timeline(state, &mgr, sid));
    }
    if name == "set_anim_options" {
        if let Some(value) = args["loop"].as_bool() {
            mgr.set_loop(value);
        }
        if let Some(value) = args["startCamera"].as_str() {
            if !value.is_empty()
                && state
                    .scene_manager()
                    .scene(sid)
                    .unwrap()
                    .camera_by_name(value)
                    .is_none()
            {
                return Err(CueMolError::not_found("start camera"));
            }
            mgr.set_startcam(value);
        }
    } else if name == "add_anim" {
        let kind = text(args, "type")?;
        let class = match kind {
            "spin" => "SimpleSpin",
            "camera" => "CamMotion",
            "show" | "hide" => "ShowHideAnim",
            "slidein" | "slideout" => "SlideInOutAnim",
            "mol" => "MolAnim",
            "wait" => "NoopAnimObj",
            _ => return Err(CueMolError::invalid_argument("unknown animation type")),
        };
        let label = args["name"]
            .as_str()
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| {
                (0..)
                    .map(|i| format!("{class}{i}"))
                    .find(|n| mgr.motions().iter().all(|m| m.base().name() != n))
                    .unwrap()
            });
        if label.is_empty() || mgr.motions().iter().any(|m| m.base().name() == label) {
            return Err(CueMolError::invalid_argument(
                "animation name is empty or already used",
            ));
        }
        let before = args["before"]
            .as_str()
            .map(|s| element(&mgr, s))
            .transpose()?
            .unwrap_or(mgr.size());
        let mut node = LDom2Node::new();
        node.set_tag_name("motion");
        node.set_type_name(class);
        node.set_str_attr("name", &label);
        node.set_str_attr("end", time_from_milliseconds(1000.).to_string());
        let reference = Graph::read(&mgr).auto_chain_name(before);
        if !reference.is_empty() {
            node.set_str_attr("timeRefName", reference);
        }
        if kind == "spin" {
            node.set_str_attr("angle", "360");
        } else if kind == "mol" {
            node.set_str_attr("prop", "frame");
            node.set_str_attr("startValue", "0");
            node.set_str_attr("endValue", "1");
        }
        if matches!(kind, "hide" | "show" | "slideout" | "slidein") {
            node.set_str_attr("hide", (kind == "hide" || kind == "slideout").to_string());
        }
        mgr.insert_motion_before(before, AnimMotion::from_ldom2_node(&node)?);
        seed_start_camera(state, camera, sid, &mut mgr)?;
    } else {
        let index = element(&mgr, text(args, "element")?)?;
        if name == "remove_anim" {
            let graph = Graph::read(&mgr);
            graph.detach_dependents(&mut mgr, &[index]);
            mgr.remove_at(index);
        } else if name == "move_anim" {
            let to = crate::nodes::move_target(args, index, mgr.size())?;
            let motion = mgr.motions()[index].clone();
            mgr.remove_at(index);
            mgr.insert_motion_before(to, motion);
        } else {
            let mut node = mgr.motions()[index].to_ldom2_node();
            if name == "set_anim_time" {
                let start = args["startMs"].as_f64().unwrap();
                let end = args["endMs"].as_f64().unwrap();
                if end < start {
                    return Err(CueMolError::invalid_argument(
                        "animation end must not be before the start",
                    ));
                }
                node.set_str_attr("start", editor_time_ms(start).to_string());
                node.set_str_attr("end", editor_time_ms(end).to_string());
            } else if name == "reset_anim_prop" {
                let properties = text(args, "props")?
                    .split(',')
                    .map(crate::animation_value::trim)
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>();
                if properties.is_empty() {
                    return Err(CueMolError::invalid_argument("no properties to reset"));
                }
                for prop in properties {
                    if mgr.motions()[index]
                        .property_spec(prop)
                        .is_none_or(|spec| spec.default.is_none())
                    {
                        return Err(CueMolError::invalid_argument(format!(
                            "{prop} has no writable default"
                        )));
                    }
                    node.remove_child(prop);
                }
            } else {
                let requested = text(args, "prop")?;
                let prop = if name == "set_anim_prop" && requested == "tgtAlpha" {
                    "tgt_alpha"
                } else {
                    requested
                };
                let value = text(args, "value")?;
                let structured = name == "set_anim_prop";
                if matches!(prop, "start" | "end") {
                    return Err(CueMolError::invalid_argument(
                        "start and end are edited together through set_anim_time",
                    ));
                }
                if matches!(prop, "name" | "timeRefName") {
                    let converted = crate::animation_value::guarded(value, structured);
                    let value = converted.as_str();
                    let graph = Graph::read(&mgr);
                    let original = &graph.nodes[index];
                    if prop == "name" {
                        let name = graph
                            .check_name(index, value)
                            .map_err(CueMolError::invalid_argument)?;
                        if name == original.name {
                            return Ok(timeline(state, &mgr, sid));
                        }
                        if !original.name.is_empty()
                            && graph.first.get(&original.name) == Some(&index)
                        {
                            for dependent in &graph.nodes {
                                if dependent.index != index
                                    && dependent.time_ref_name == original.name
                                {
                                    mgr.motions_mut()[dependent.index]
                                        .base_mut()
                                        .set_time_ref_name(&name);
                                }
                            }
                        }
                        node.set_str_attr("name", name);
                    } else {
                        let span = graph
                            .rebase_span(index, value)
                            .map_err(CueMolError::invalid_argument)?;
                        if value == original.time_ref_name {
                            return Ok(timeline(state, &mgr, sid));
                        }
                        if let Some((relative_start, relative_end)) = span {
                            node.set_str_attr("start", editor_time_ms(relative_start).to_string());
                            node.set_str_attr("end", editor_time_ms(relative_end).to_string());
                        }
                        node.set_str_attr("timeRefName", value);
                    }
                } else {
                    let spec = mgr.motions()[index].property_spec(prop).ok_or_else(|| {
                        CueMolError::invalid_argument(format!("unknown property {prop}"))
                    })?;
                    if spec.read_only {
                        return Err(CueMolError::invalid_argument(format!(
                            "{prop} is read-only"
                        )));
                    }
                    let value = crate::animation_value::property(
                        &mgr.motions()[index],
                        spec,
                        value,
                        structured,
                    )?;
                    node.set_str_attr(prop, value);
                    if let Some(child) = node.find_child_mut(prop) {
                        child.set_default_flag(false);
                    }
                }
            }
            let mut motion = AnimMotion::from_ldom2_node(&node)?;
            let previous = mgr.motions()[index].base();
            motion.base_mut().set_abs_start(previous.abs_start());
            motion.base_mut().set_abs_end(previous.abs_end());
            mgr.motions_mut()[index] = motion;
        }
    }
    save_edited_animation(state, camera, sid, &mut mgr, name != "set_anim_options")?;
    Ok(timeline(state, &mgr, sid))
}

/// Apply a preview without history, or restore its anchor before a normal commit.
pub fn prepare_animation_edit(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    args: &Value,
) -> Result<Option<(&'static str, Value)>> {
    let mode = text(args, "mode")?;
    if !matches!(mode, "preview" | "commit" | "abort") {
        return Err(CueMolError::invalid_argument("unknown animation edit mode"));
    }
    let sid = crate::commands::uid(args, "sceneId")?;
    let uid = crate::commands::uid(args, "uid")?;
    if mode != "abort" && state.active_scene() != Some(sid) {
        return Err(CueMolError::invalid_argument("the inspected scene changed"));
    }
    let mut mgr = state
        .scene_animation(sid)?
        .ok_or_else(|| CueMolError::not_found("animation"))?;
    let index = element(&mgr, &format!("#{}", uid.get()))?;
    let prop = text(args, "prop")?;
    let current = &mgr.motions()[index];
    let motion_for = |value: &Value, was_default: bool| -> Result<AnimMotion> {
        let mut node = current.to_ldom2_node();
        if prop == "timing" {
            let (start, end) = value["startMs"]
                .as_f64()
                .zip(value["endMs"].as_f64())
                .filter(|(start, end)| start.is_finite() && end.is_finite())
                .ok_or_else(|| {
                    CueMolError::invalid_argument("timing requires two finite numbers")
                })?;
            node.set_str_attr("start", editor_time_ms(start.min(end)).to_string());
            node.set_str_attr("end", editor_time_ms(start.max(end)).to_string());
        } else {
            if !matches!(
                prop,
                "quadric"
                    | "angle"
                    | "tgt_alpha"
                    | "direction"
                    | "distance"
                    | "startValue"
                    | "endValue"
            ) || !node.children().iter().any(|child| child.tag_name() == prop)
            {
                return Err(CueMolError::invalid_argument(
                    "animation previews require a numeric property or timing",
                ));
            }
            let number = value
                .as_str()
                .and_then(|value| value.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .ok_or_else(|| {
                    CueMolError::invalid_argument("numeric edits require finite values")
                })?;
            node.set_str_attr(prop, number.to_string());
            node.find_child_mut(prop).unwrap().set_default_flag(false);
            if was_default
                && current
                    .property_spec(prop)
                    .is_some_and(|spec| spec.default.is_some())
            {
                node.remove_child(prop);
            }
        }
        let mut motion = AnimMotion::from_ldom2_node(&node)?;
        motion.base_mut().set_abs_start(current.base().abs_start());
        motion.base_mut().set_abs_end(current.base().abs_end());
        Ok(motion)
    };
    let value = motion_for(&args["value"], false)?;
    let original = motion_for(
        &args["originalValue"],
        args["originalWasDefault"].as_bool().unwrap_or(false),
    )?;
    let unchanged = if prop == "timing" {
        (value.base().rel_start(), value.base().rel_end())
            == (original.base().rel_start(), original.base().rel_end())
    } else {
        value
            .to_ldom2_node()
            .str_attr(prop)
            .as_str()
            .parse::<f64>()
            .ok()
            == original
                .to_ldom2_node()
                .str_attr(prop)
                .as_str()
                .parse::<f64>()
                .ok()
    };
    mgr.motions_mut()[index] = if mode == "preview" {
        value.clone()
    } else {
        original
    };
    save_edited_animation(state, camera, sid, &mut mgr, true)?;
    if mode != "commit" || unchanged {
        return Ok(None);
    }
    let element = format!("#{}", uid.get());
    Ok(Some(if prop == "timing" {
        (
            "set_anim_time",
            json!({"element":element,"startMs":time_to_milliseconds(value.base().rel_start()),"endMs":time_to_milliseconds(value.base().rel_end())}),
        )
    } else {
        (
            "set_anim_generic_prop",
            json!({"element":element,"prop":prop,"value":args["value"]}),
        )
    }))
}

fn save_edited_animation(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    sid: Uid,
    mgr: &mut AnimMgr,
    stop_playback: bool,
) -> Result<()> {
    if let Err(error) = mgr.try_resolve_rel_time() {
        if Graph::read(mgr).error().is_none() {
            return Err(error);
        }
    }
    mgr.set_length(
        mgr.motions()
            .iter()
            .map(|motion| motion.base().abs_end())
            .max()
            .unwrap_or(0),
    );
    if stop_playback {
        let active = state.has_scene_animation_player(sid)
            || state
                .scene_animation(sid)?
                .is_some_and(|manager| manager.initial_camera().is_some());
        if active {
            let view = state
                .scene_manager()
                .scene(sid)
                .and_then(|scene| scene.active_view_uid())
                .ok_or_else(|| CueMolError::not_found("view"))?;
            let restored = state.stop_scene_animation(sid, view)?;
            if state.active_scene() == Some(sid) {
                *camera = restored;
            }
        }
        mgr.discard_playback_state();
    }
    state.set_scene_animation(sid, Some(mgr))
}

fn apply_time(state: &mut DesktopAppState, camera: &mut Camera, sid: Uid, time: f64) -> Result<()> {
    let view = state
        .active_view_summary()
        .ok_or_else(|| CueMolError::not_found("view"))?;
    *camera = state.apply_scene_animation_time(sid, time_from_milliseconds(time), view.uid)?;
    Ok(())
}

pub fn advance_animation(state: &mut DesktopAppState, camera: &mut Camera) -> Result<bool> {
    let Some(scene) = state.active_scene() else {
        return Ok(false);
    };
    if let Some(updated) = state.advance_scene_animation(scene)? {
        *camera = updated;
        Ok(true)
    } else {
        Ok(false)
    }
}
pub fn animation_element_detail(state: &DesktopAppState, uid: u64) -> Result<Value> {
    let sid = scene(state)?;
    let manager = state.scene_animation(sid)?.unwrap_or_default();
    let index = element(&manager, &format!("#{uid}"))?;
    let motion = &manager.motions()[index];
    let node = motion.to_ldom2_node();
    let mut properties: serde_json::Map<String, Value> = node
        .children()
        .iter()
        .filter(|child| child.is_attr())
        .map(|child| (child.tag_name().into(), json!(child.value())))
        .collect();
    let graph = Graph::read(&manager);
    for (name, nanos) in [
        ("start", motion.base().rel_start()),
        ("end", motion.base().rel_end()),
        ("absStart", motion.base().abs_start()),
        ("absEnd", motion.base().abs_end()),
    ] {
        let millis = if name == "absStart" {
            graph.nodes[index]
                .abs_start_ms
                .unwrap_or_else(|| time_to_milliseconds(nanos))
        } else if name == "absEnd" {
            graph.nodes[index]
                .abs_end_ms
                .unwrap_or_else(|| time_to_milliseconds(nanos))
        } else {
            time_to_milliseconds(nanos)
        };
        let mut time = cuemol_core::LScrTime::new();
        time.set_milli_sec_real(millis);
        properties.insert(name.into(), json!(time.script_string()));
    }
    let metadata: serde_json::Map<String, Value> = motion.property_specs().map(|spec| (
        spec.name.into(), json!({"type":spec.type_name,"readonly":spec.read_only || matches!(spec.name,"start"|"end"),"hasDefault":spec.default.is_some(),"isDefault":motion.is_property_default(spec.name),"defaultValue":spec.default})
    )).collect();
    let references = graph
        .first
        .keys()
        .map(|name| json!({"name":name,"error":graph.check_reference(index,name).err()}))
        .collect::<Vec<_>>();
    let mut renderers = Vec::new();
    let mut molecules = Vec::new();
    if let Some(scene) = state.scene_manager().scene(sid) {
        for object in scene.objects() {
            if object.object_type() == "MorphMol" {
                molecules.push(object.name().to_string());
            }
            for renderer in object.renderers() {
                if !renderer.name().is_empty() {
                    renderers.push(json!({"object":object.name(),"name":renderer.name()}));
                }
            }
        }
    }
    for object in state.density_map_summaries(sid) {
        for renderer in object.renderers {
            if !renderer.name.is_empty() {
                renderers.push(json!({"object":object.name,"name":renderer.name}));
            }
        }
    }
    for object in state.surface_summaries(sid) {
        for renderer in object.renderers {
            if !renderer.name.is_empty() {
                renderers.push(json!({"object":object.name,"name":renderer.name}));
            }
        }
    }
    for object in state.lw_object_summaries(sid) {
        for renderer in object.renderers {
            if !renderer.name.is_empty() {
                renderers.push(json!({"object":object.name,"name":renderer.name}));
            }
        }
    }
    Ok(
        json!({"sceneId":sid.get(),"uid":uid,"type":motion.type_name(),"properties":properties,"metadata":metadata,
        "startMs":time_to_milliseconds(motion.base().rel_start()),"endMs":time_to_milliseconds(motion.base().rel_end()),
        "resolveError":graph.nodes[index].error,"references":references,
        "cameras":state.camera_summaries(sid).into_iter().map(|c|c.name).collect::<Vec<_>>(),
        "renderers":renderers,"molecules":molecules}),
    )
}
