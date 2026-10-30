use super::{
    pymol::{boolean, matching_nodes, native_property, number, tool},
    Action, Session,
};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub(super) fn colors() -> &'static BTreeMap<String, String> {
    static COLORS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    COLORS.get_or_init(|| {
        serde_json::from_str(include_str!("pymol-colors.json")).expect("valid PyMOL colors")
    })
}

pub(super) fn rgb(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    if let Some(hex) = text.strip_prefix('#').or_else(|| text.strip_prefix("0x")) {
        if hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Ok(format!("#{}", hex.to_ascii_lowercase()));
        }
    }
    let values = text
        .trim_matches(['[', ']'])
        .split(',')
        .map(|value| number(value.trim()))
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != 3 || values.iter().any(|value| *value < 0.) {
        return Err("RGB requires three nonnegative values".into());
    }
    let scale = if values.iter().any(|value| *value > 1.) {
        255.
    } else {
        1.
    };
    Ok(format!(
        "#{}",
        values
            .iter()
            .map(|value| format!("{:02x}", ((value / scale).min(1.) * 255.).round() as u8))
            .collect::<String>()
    ))
}

pub(super) fn color(state: &DesktopAppState, raw: &str) -> Result<String, String> {
    if let Ok(rgb) = rgb(raw) {
        return Ok(rgb);
    }
    let key = raw.trim().to_ascii_lowercase();
    if let Some(known) = colors().get(&key) {
        return Ok(known.clone());
    }
    if let Some(level) = key
        .strip_prefix("grey")
        .or_else(|| key.strip_prefix("gray"))
    {
        if level.len() == 2 && level.chars().all(|ch| ch.is_ascii_digit()) {
            let value = (level.parse::<f64>().unwrap() / 99. * 255.).round() as u8;
            return Ok(format!("#{value:02x}{value:02x}{value:02x}"));
        }
    }
    let value = state
        .active_style_color(raw)
        .ok_or_else(|| format!("Unknown color: {raw}"))?;
    Ok(format!("#{:02x}{:02x}{:02x}", value.r, value.g, value.b))
}

struct RepresentationSetting {
    rep: &'static str,
    props: &'static [&'static str],
    kind: &'static str,
}

fn representation_setting(name: &str) -> Option<RepresentationSetting> {
    let (rep, props, kind): (_, &[_], _) = match name {
        "stick_radius" => ("sticks", &["bondw", "sphr"], "positive"),
        "stick_transparency" => ("sticks", &["alpha"], "transparency"),
        "line_width" => ("lines", &["width"], "positive"),
        "sphere_transparency" => ("spheres", &["alpha"], "transparency"),
        "cartoon_transparency" => ("cartoon", &["alpha"], "transparency"),
        "transparency" => ("surface", &["alpha"], "transparency"),
        "solvent_radius" => ("surface", &["proberad"], "positive"),
        "label_size" => ("labels", &["font_size"], "positive"),
        "label_color" => ("labels", &["color"], "color"),
        _ => return None,
    };
    Some(RepresentationSetting { rep, props, kind })
}

pub(super) fn remembered(
    session: &Session,
    state: &DesktopAppState,
    rep: &str,
    path: &str,
) -> Vec<Action> {
    let Some(scene) = state.active_scene() else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    for ((scope, name), value) in &session.remembered_settings {
        if *scope != scene.get() {
            continue;
        }
        if let Some(setting) = representation_setting(name) {
            if setting.rep == rep {
                actions.extend(
                    setting
                        .props
                        .iter()
                        .map(|prop| native_property(path, prop, value)),
                );
            }
        }
    }
    actions
}

pub(super) fn plan(
    session: &Session,
    state: &DesktopAppState,
    camera: &Camera,
    command: &str,
    name: &str,
    raw: &str,
    target: &str,
) -> Result<Vec<Action>, String> {
    let scene = state.active_scene().ok_or("No active scene")?;
    let mut actions = Vec::new();
    if let Some(setting) = representation_setting(name) {
        let objects = matching_nodes(state, target)?
            .into_iter()
            .filter(|node| node.kind == "object")
            .collect::<Vec<_>>();
        if !target.is_empty() && objects.is_empty() {
            return Err(format!("No molecule named {target}"));
        }
        let renderers = objects
            .iter()
            .flat_map(|node| state.renderer_summaries(Uid::new(node.id)))
            .filter(|renderer| renderer.name == format!("pym:{}", setting.rep))
            .collect::<Vec<_>>();
        if command == "get" {
            let global = target
                .is_empty()
                .then(|| session.remembered_settings.get(&(scene.get(), name.into())))
                .flatten();
            let value = if let Some(value) = global {
                value.clone()
            } else {
                let first = renderers
                    .first()
                    .ok_or_else(|| format!("No {} representation is shown", setting.rep))?;
                crate::nodes::renderer_properties(state, first.uid)
                    .map_err(|error| error.to_string())?
                    .get(setting.props[0])
                    .cloned()
                    .ok_or_else(|| format!("No property {}", setting.props[0]))?
            };
            let value = if setting.kind == "transparency" {
                format!("{:.6}", 1. - number(&value)?)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            } else {
                value
            };
            return Ok(vec![Action::Output(format!("get: {name} = {value}"))]);
        }
        let value = if command == "unset" {
            None
        } else if setting.kind == "color" {
            Some(color(state, raw)?)
        } else {
            let number = number(raw)?;
            if setting.kind == "positive" && number <= 0.
                || setting.kind == "transparency" && !(0.0..=1.0).contains(&number)
            {
                return Err(format!("Invalid value for {name}: {raw}"));
            }
            Some(
                if setting.kind == "transparency" {
                    1. - number
                } else {
                    number
                }
                .to_string(),
            )
        };
        for renderer in &renderers {
            for prop in setting.props {
                actions.push(if let Some(value) = &value {
                    tool(
                        "set_renderer_property",
                        json!({"rendId":renderer.uid.get(),"prop":prop,"value":value}),
                    )?
                } else {
                    tool(
                        "reset_prop",
                        json!({"path":format!("#{}.{}",renderer.uid.get(),prop)}),
                    )?
                });
            }
        }
        if target.is_empty() {
            actions.push(Action::RememberSetting {
                scene: scene.get(),
                name: name.into(),
                value,
            });
        }
        return Ok(actions);
    }
    let prop = if name == "bg_rgb" {
        "bgcolor"
    } else if name == "orthoscopic" {
        "perspective"
    } else {
        name
    };
    let (kind, id) = if name == "orthoscopic" || target == "view" {
        ("view", None)
    } else if target.is_empty() {
        ("scene", None)
    } else {
        let nodes = matching_nodes(state, target)?;
        if nodes.len() != 1 {
            return Err(format!("Expected one object or renderer: {target}"));
        }
        (nodes[0].kind, Some(Uid::new(nodes[0].id)))
    };
    let values = crate::properties::node_property_values(state, camera, kind, id)
        .map_err(|error| error.to_string())?;
    let current = values
        .get(prop)
        .ok_or_else(|| format!("Unknown setting: {name}"))?;
    if command == "get" {
        let shown = if name == "orthoscopic" {
            (!boolean(current)?).to_string()
        } else {
            current.clone()
        };
        return Ok(vec![Action::Output(format!("get: {name} = {shown}"))]);
    }
    if command == "unset" {
        if kind == "view" {
            let defaults =
                crate::properties::node_property_values(state, &Camera::new(), "view", None)
                    .map_err(|error| error.to_string())?;
            let default = defaults
                .get(prop)
                .ok_or_else(|| format!("No default for {name}"))?;
            return Ok(vec![tool(
                "set_node_prop",
                json!({"nodeType":"view","prop":prop,"value":default}),
            )?]);
        }
        let path = id.map_or_else(|| prop.to_string(), |id| format!("#{}.{prop}", id.get()));
        return Ok(vec![tool("reset_prop", json!({"path":path}))?]);
    }
    let value = if prop == "bgcolor" || prop == "color" {
        color(state, raw)?
    } else if name == "orthoscopic" {
        (!boolean(raw)?).to_string()
    } else if current == "true" || current == "false" {
        boolean(raw)?.to_string()
    } else {
        raw.to_string()
    };
    actions.push(tool(
        "set_node_prop",
        json!({"nodeType":kind,"nodeId":id.map(Uid::get),"prop":prop,"value":value}),
    )?);
    Ok(actions)
}
