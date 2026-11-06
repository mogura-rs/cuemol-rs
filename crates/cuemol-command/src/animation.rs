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
        if kind == "hide" || kind == "slideout" {
            node.set_str_attr("hide", "true");
        }
        mgr.insert_motion_before(before, AnimMotion::from_ldom2_node(&node)?);
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
                if start < 0. || end < start {
                    return Err(CueMolError::invalid_argument(
                        "animation end must follow a nonnegative start",
                    ));
                }
                node.set_str_attr("start", editor_time_ms(start).to_string());
                node.set_str_attr("end", editor_time_ms(end).to_string());
            } else if name == "reset_anim_prop" {
                for prop in text(args, "props")?
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|p| !p.is_empty())
                {
                    if matches!(
                        prop,
                        "name" | "uid" | "timeRefName" | "start" | "end" | "absStart" | "absEnd"
                    ) {
                        return Err(CueMolError::invalid_argument(format!(
                            "{prop} has no writable default"
                        )));
                    }
                    if !node.children().iter().any(|child| child.tag_name() == prop) {
                        return Err(CueMolError::invalid_argument(format!(
                            "unknown property {prop}"
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
                if matches!(prop, "start" | "end") {
                    return Err(CueMolError::invalid_argument(
                        "start and end are edited together through set_anim_time",
                    ));
                }
                if matches!(prop, "name" | "timeRefName") {
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
                    if !node.children().iter().any(|child| child.tag_name() == prop)
                        && !matches!(
                            prop,
                            "axis"
                                | "name"
                                | "timeRefName"
                                | "endcam"
                                | "rend"
                                | "mol"
                                | "disabled"
                                | "quadric"
                        )
                    {
                        return Err(CueMolError::invalid_argument(format!(
                            "unknown property {prop}"
                        )));
                    }
                    if prop == "uid" {
                        return Err(CueMolError::invalid_argument("identity is read-only"));
                    }
                    node.set_str_attr(prop, value);
                }
            }
            let mut motion = AnimMotion::from_ldom2_node(&node)?;
            let previous = mgr.motions()[index].base();
            motion.base_mut().set_abs_start(previous.abs_start());
            motion.base_mut().set_abs_end(previous.abs_end());
            mgr.motions_mut()[index] = motion;
        }
    }
    if let Err(error) = mgr.try_resolve_rel_time() {
        let graph = Graph::read(&mgr);
        if graph.error().is_none() {
            return Err(error);
        }
    }
    let duration = mgr
        .motions()
        .iter()
        .map(|m| m.base().abs_end())
        .max()
        .unwrap_or(0);
    mgr.set_length(duration);
    if name != "set_anim_options" {
        let active = state.has_scene_animation_player(sid)
            || state
                .scene_animation(sid)?
                .is_some_and(|manager| manager.initial_camera().is_some());
        if active {
            let view = state
                .active_view_summary()
                .ok_or_else(|| CueMolError::not_found("view"))?;
            *camera = state.stop_scene_animation(sid, view.uid)?;
        }
        mgr.discard_playback_state();
    }
    state.set_scene_animation(sid, Some(&mgr))?;
    Ok(timeline(state, &mgr, sid))
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
