use crate::commands::{scene, text};
use cuemol_anim::{AnimMgr, AnimMotion};
use cuemol_core::clock::Instant;
use cuemol_core::{
    time_from_milliseconds, time_to_milliseconds, CueMolError, LDom2Node, Result, Uid,
};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::sync::OnceLock;

fn clock_ms() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.
}
fn key(scene: Uid, part: &str) -> String {
    format!("animation:{}:{part}", scene.get())
}
fn runtime_number(state: &DesktopAppState, scene: Uid, part: &str) -> f64 {
    state.runtime_value(&key(scene, part)).parse().unwrap_or(0.)
}
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
    json!({"mgr":{"lengthMs":time_to_milliseconds(mgr.length()),"loop":mgr.loop_enabled(),"startcam":mgr.startcam(),"playState":if state.runtime_value(&key(sid,"playing"))=="true"{"playing"}else{"paused"},"elapsedMs":runtime_number(state,sid,"time")},"elements":mgr.motions().iter().enumerate().map(|(i,m)|{let b=m.base();json!({"number":i+1,"uid":b.uid().get(),"name":b.name(),"type":m.type_name(),"startMs":time_to_milliseconds(b.rel_start()),"endMs":time_to_milliseconds(b.rel_end()),"absStartMs":time_to_milliseconds(b.abs_start()),"absEndMs":time_to_milliseconds(b.abs_end()),"timeRefName":b.time_ref_name(),"disabled":b.disabled()})}).collect::<Vec<_>>()})
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
        advance_animation(state, camera)?;
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
        if mgr.startcam().is_empty() {
            let mut start = camera.clone();
            start.set_name("__animation_start");
            state.save_scene_camera(sid, start, true)?;
            mgr.set_startcam("__animation_start");
            state.set_scene_animation(sid, Some(&mgr))?;
        }
        match action {
            "play" => {
                let duration = time_to_milliseconds(mgr.length());
                if duration <= 0. {
                    return Err(CueMolError::invalid_argument("animation has no duration"));
                }
                let mut time = runtime_number(state, sid, "time");
                if time >= duration {
                    time = 0.;
                }
                state.put_runtime_value(key(sid, "time"), time.to_string());
                state.put_runtime_value(key(sid, "offset"), time.to_string());
                state.put_runtime_value(key(sid, "epoch"), clock_ms().to_string());
                state.put_runtime_value(key(sid, "playing"), "true");
            }
            "pause" => state.put_runtime_value(key(sid, "playing"), "false"),
            "stop" | "seek" => {
                let time = if action == "stop" {
                    0.
                } else {
                    args["timeMs"]
                        .as_f64()
                        .ok_or_else(|| CueMolError::invalid_argument("seek requires timeMs"))?
                };
                if !time.is_finite() || time < 0. {
                    return Err(CueMolError::invalid_argument("invalid animation time"));
                }
                state.put_runtime_value(key(sid, "playing"), "false");
                apply_time(
                    state,
                    camera,
                    sid,
                    time.min(time_to_milliseconds(mgr.length())),
                )?;
            }
            _ => return Err(CueMolError::invalid_argument("unknown animation action")),
        }
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
        let label = args["name"].as_str().map(str::to_owned).unwrap_or_else(|| {
            (1..)
                .map(|i| format!("{kind}{i}"))
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
        if before > 0 {
            node.set_str_attr("timeRefName", mgr.motions()[before - 1].base().name());
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
            let removed = mgr.motions()[index].base().name().to_string();
            let prior = mgr.motions()[index].base().time_ref_name().to_string();
            mgr.remove_at(index);
            for motion in mgr.motions_mut() {
                if motion.base().time_ref_name() == removed {
                    motion.base_mut().set_time_ref_name(prior.clone());
                }
            }
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
                node.set_str_attr("start", time_from_milliseconds(start).to_string());
                node.set_str_attr("end", time_from_milliseconds(end).to_string());
            } else if name == "reset_anim_prop" {
                for prop in text(args, "props")?
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|p| !p.is_empty())
                {
                    if prop == "name" || prop == "uid" {
                        return Err(CueMolError::invalid_argument("identity cannot be reset"));
                    }
                    node.remove_child(prop);
                }
            } else {
                let prop = text(args, "prop")?;
                let value = text(args, "value")?;
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
            let motion = AnimMotion::from_ldom2_node(&node)?;
            mgr.motions_mut()[index] = motion;
        }
    }
    mgr.try_resolve_rel_time()?;
    let duration = mgr
        .motions()
        .iter()
        .map(|m| m.base().abs_end())
        .max()
        .unwrap_or(0);
    mgr.set_length(duration);
    state.set_scene_animation(sid, Some(&mgr))?;
    Ok(timeline(state, &mgr, sid))
}

fn apply_time(state: &mut DesktopAppState, camera: &mut Camera, sid: Uid, time: f64) -> Result<()> {
    let view = state
        .active_view_summary()
        .ok_or_else(|| CueMolError::not_found("view"))?;
    *camera = state.apply_scene_animation_time(sid, time_from_milliseconds(time), view.uid)?;
    state.put_runtime_value(key(sid, "time"), time.to_string());
    Ok(())
}

pub fn advance_animation(state: &mut DesktopAppState, camera: &mut Camera) -> Result<bool> {
    let Some(sid) = state.active_scene() else {
        return Ok(false);
    };
    if state.runtime_value(&key(sid, "playing")) != "true" {
        return Ok(false);
    }
    let Some(mgr) = state.scene_animation(sid)? else {
        return Ok(false);
    };
    let duration = time_to_milliseconds(mgr.length());
    if duration <= 0. {
        state.put_runtime_value(key(sid, "playing"), "false");
        return Ok(false);
    }
    let elapsed = (clock_ms() - runtime_number(state, sid, "epoch")
        + runtime_number(state, sid, "offset"))
    .max(0.);
    let time = if mgr.loop_enabled() {
        elapsed % duration
    } else {
        elapsed.min(duration)
    };
    if elapsed >= duration && !mgr.loop_enabled() {
        state.put_runtime_value(key(sid, "playing"), "false");
    }
    apply_time(state, camera, sid, time)?;
    Ok(true)
}
