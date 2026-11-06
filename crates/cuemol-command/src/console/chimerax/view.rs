use super::{
    colors,
    execute::{tool, Context, Target},
    Command,
};
use crate::console::Action;
use cuemol_core::{Matrix3, Quat, Uid, Vector4};
use cuemol_qsys::{Camera, View};
use serde_json::{json, Value};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;

pub(super) fn current_view(context: &Context<'_>, camera: &Camera) -> View {
    let mut view = View::new(
        Uid::new(0),
        context.state.active_scene().unwrap_or(Uid::new(0)),
    );
    view.set_camera(camera);
    let (w, h) = context
        .state
        .active_view_summary()
        .and_then(|s| context.state.command_manager().view(s.uid))
        .map_or((800, 600), |v| (v.width().max(1), v.height().max(1)));
    view.set_view_size(w, h);
    view
}
pub(super) fn framing(
    context: &Context<'_>,
    camera: &Camera,
    targets: &[Target],
) -> Result<Option<Value>> {
    let view = current_view(context, camera);
    let mut bounds: Option<(Vector4, Vector4)> = None;
    let rotation = camera.rotation().to_matrix();
    for target in targets {
        let Some(object) = context.state.scene_manager().object(target.model.id) else {
            continue;
        };
        for index in context.indices(target)? {
            let mut world = object.mol().atoms()[index].position;
            object.xform_matrix().xform3d(&mut world);
            let p = rotation.mul_vec(world);
            bounds = Some(match bounds {
                None => (p, p),
                Some((lo, hi)) => (
                    Vector4::new3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)),
                    Vector4::new3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)),
                ),
            });
        }
    }
    let Some((lo, hi)) = bounds else {
        return Ok(None);
    };
    let grow = (hi - lo).scale(0.2);
    let mut fitted = camera.clone();
    fitted.fit_to_box(
        lo - grow,
        hi + grow,
        f64::from(view.width()) / f64::from(view.height().max(1)),
    );
    let c = fitted.center();
    Ok(Some(
        json!({"center":[c.x,c.y,c.z],"zoom":fitted.zoom(),"slab":fitted.slab_depth()}),
    ))
}
fn axis(text: &str) -> Option<Vector4> {
    let sign = if text.starts_with('-') { -1. } else { 1. };
    let v = match text.trim_start_matches('-').to_ascii_lowercase().as_str() {
        "x" => Vector4::new3(sign, 0., 0.),
        "y" => Vector4::new3(0., sign, 0.),
        "z" => Vector4::new3(0., 0., sign),
        _ => {
            let n = text
                .split(',')
                .map(str::parse::<f64>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .ok()?;
            if n.len() != 3 || n.iter().any(|v| !v.is_finite()) {
                return None;
            }
            Vector4::new3(n[0], n[1], n[2])
        }
    };
    let n = v.length3d();
    (n > 0.).then(|| v.scale(1. / n))
}
pub(super) fn precision(value: f64, digits: usize) -> String {
    if value == 0. {
        return "0".into();
    }
    let scientific = format!("{value:.precision$e}", precision = digits - 1);
    let (mantissa, exp) = scientific.split_once('e').unwrap();
    let exp = exp.parse::<i32>().unwrap();
    let trim = |s: String| {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    };
    if exp < -6 || exp >= digits as i32 {
        format!(
            "{}e{}{:02}",
            trim(mantissa.into()),
            if exp < 0 { "-" } else { "+" },
            exp.abs()
        )
    } else {
        trim(format!(
            "{value:.precision$}",
            precision = (digits as i32 - exp - 1).max(0) as usize
        ))
    }
}
fn refuse(command: &str, args: &BTreeMap<String, String>, keys: &[&str], why: &str) -> Result<()> {
    if let Some(key) = keys
        .iter()
        .find(|key| args.get(**key).is_some_and(|v| !v.is_empty()))
    {
        Err(format!("Error: {command}: {key} is not supported ({why})"))
    } else {
        Ok(())
    }
}
fn rotation(q: Quat) -> Value {
    json!([q.w, q.x, q.y, q.z])
}
fn vec3(v: Vector4) -> Value {
    json!([v.x, v.y, v.z])
}
fn output(text: impl Into<String>) -> Action {
    Action::Output(text.into())
}

pub(super) fn plan(
    context: &Context<'_>,
    camera: &Camera,
    command: &Command,
    args: &BTreeMap<String, String>,
) -> Result<Vec<Action>> {
    let name = command.name.as_str();
    let view = current_view(context, camera);
    let value = |key: &str| args.get(key).map(String::as_str).unwrap_or("");
    let mut out = Vec::new();
    let change = match name {
        "turn" | "move" => {
            refuse(
                name,
                args,
                if name == "turn" {
                    &[
                        "center",
                        "coordinateSystem",
                        "rock",
                        "wobble",
                        "wobbleAspect",
                        "models",
                        "atoms",
                    ]
                } else {
                    &["coordinateSystem", "models", "atoms"]
                },
                if name == "turn" {
                    "the camera turns about the view centre, and models are not moved"
                } else {
                    "only the camera is moved"
                },
            )?;
            let raw = if name == "turn" && value("axis").is_empty() {
                "y"
            } else {
                value("axis")
            };
            let axis = axis(raw).ok_or_else(|| {
                format!("Error: {name}: \"{raw}\" is not an axis (x, y, z or three numbers)")
            })?;
            let frames = if value("frames").is_empty() {
                1.
            } else {
                value("frames").parse::<f64>().unwrap_or(f64::NAN)
            };
            if !frames.is_finite() || frames <= 0. || frames.fract() != 0. {
                return Err(format!(
                    "Error: {name}: frames must be a whole number (forever is not supported)"
                ));
            }
            if !value("frames").is_empty() {
                out.push(output(format!(
                    "Warning: {name}: frames: {} at once, not animated",
                    if name == "turn" { "turned" } else { "moved" }
                )));
            }
            if name == "turn" {
                let angle = value("angle").parse::<f64>().unwrap_or(90.) * frames;
                let q =
                    Quat::from_axis_angle(axis, -angle.to_radians()).map_err(|e| e.to_string())?;
                json!({"rotation":rotation(camera.rotation().mul_quat(q))})
            } else {
                let d = value("distance").parse::<f64>().unwrap_or(1.) * frames;
                let shift = view.right_vector().scale(axis.x)
                    + view.up_vector().scale(axis.y)
                    + view.forward_vector().scale(axis.z);
                json!({"center":vec3(camera.center()-shift.scale(d))})
            }
        }
        "cofr" => {
            if !value("objects").is_empty()
                || !value("pivot").is_empty()
                || !value("coordinateSystem").is_empty()
                || !matches!(value("method"), "" | "centerOfView")
            {
                return Err("Error: cofr: CueMol turns about the view centre, so moving the centre of rotation would move the view; use \"view <spec>\" to centre on something".into());
            }
            if !value("showPivot").is_empty() {
                out.push(output(
                    "Warning: cofr: showPivot is not supported yet; ignored",
                ));
            }
            if value("method").is_empty() {
                let c = camera.center();
                out.push(output(format!(
                    "Center of rotation: {} {} {}  center of view",
                    precision(c.x, 5),
                    precision(c.y, 5),
                    precision(c.z, 5)
                )));
            }
            return Ok(out);
        }
        "zoom" => {
            let pixel = camera.zoom() / f64::from(view.height().max(1));
            if value("factor").is_empty() && value("pixelSize").is_empty() {
                return Ok(vec![output(format!(
                    "Pixel size at center of rotation is {}",
                    precision(pixel, 3)
                ))]);
            }
            if !value("frames").is_empty() {
                out.push(output(
                    "Warning: zoom: frames: zoomed at once, not animated",
                ));
            }
            let mut factor = value("factor").parse::<f64>().unwrap_or(1.);
            if !value("pixelSize").is_empty() {
                factor *= pixel
                    / value("pixelSize")
                        .parse::<f64>()
                        .map_err(|e| e.to_string())?;
            }
            if factor <= 0. || factor.is_nan() {
                return Err("Error: zoom: the factor must be positive".into());
            }
            json!({"zoom":camera.zoom()/factor})
        }
        "view list" => {
            let mut names = context
                .state
                .active_camera_summaries()
                .into_iter()
                .map(|c| c.name)
                .collect::<Vec<_>>();
            names.sort();
            return Ok(vec![output(if names.is_empty() {
                "No named views.".into()
            } else {
                format!("Named views: {}", names.join(", "))
            })]);
        }
        "view name" => {
            if let Some(clash) = [
                "clip", "cofr", "delete", "frames", "initial", "list", "matrix", "orient",
                "zalign", "pad", "position",
            ]
            .into_iter()
            .find(|r| r.starts_with(value("name")))
            {
                return Err(format!(
                    "Error: view name \"{}\" conflicts with \"{clash}\" view option",
                    value("name")
                ));
            }
            return Ok(vec![tool("save_camera", json!({"name":value("name")}))?]);
        }
        "view delete" => {
            let names = context
                .state
                .active_camera_summaries()
                .into_iter()
                .filter(|c| value("name") == "all" || c.name == value("name"))
                .map(|c| c.name);
            return names
                .map(|name| tool("delete_camera", json!({"name":name})))
                .collect();
        }
        "view" => {
            if value("objects").is_empty()
                && !value("frames").is_empty()
                && !re!(r"^\d+$").is_match(value("frames"))
            {
                if context
                    .state
                    .active_camera_summaries()
                    .iter()
                    .any(|c| c.name == value("frames"))
                {
                    return Ok(vec![tool("apply_camera", json!({"name":value("frames")}))?]);
                }
                return Err(format!(
                    "Error: view: \"{}\" is neither an atom specification nor a named view",
                    value("frames")
                ));
            }
            framing(context, camera, &context.targets(value("objects"), true)?)?
                .ok_or("Error: view: nothing to show")?
        }
        "view matrix" => {
            refuse(
                name,
                args,
                &["models", "coordinateSystem"],
                "CueMol objects have no positions of their own",
            )?;
            if value("camera").is_empty() {
                let eye = camera.center() + view.forward_vector().scale(camera.cam_dist());
                let cols = [
                    view.right_vector(),
                    view.up_vector(),
                    view.forward_vector(),
                    eye,
                ];
                let mut values = Vec::new();
                for component in 0..3 {
                    for col in &cols {
                        values.push(precision(
                            match component {
                                0 => col.x,
                                1 => col.y,
                                _ => col.z,
                            },
                            5,
                        ));
                    }
                }
                return Ok(vec![output(format!(
                    "view matrix camera {}",
                    values.join(",")
                ))]);
            }
            let values = value("camera")
                .split(',')
                .map(|v| v.trim().parse::<f64>())
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| "Error: view matrix: camera takes 12 numbers")?;
            if values.len() != 12 || values.iter().any(|n| !n.is_finite()) {
                return Err("Error: view matrix: camera takes 12 numbers".into());
            }
            let column = |j| Vector4::new3(values[j], values[4 + j], values[8 + j]);
            let unit = |v: Vector4| {
                let n = v.length3d();
                v.scale(1. / if n == 0. { 1. } else { n })
            };
            let (right, up, back, eye) =
                (unit(column(0)), unit(column(1)), unit(column(2)), column(3));
            let mut distance = camera.cam_dist();
            if let Some(fit) = framing(context, camera, &context.targets("", true)?)? {
                let c = &fit["center"];
                let center = Vector4::new3(
                    c[0].as_f64().unwrap(),
                    c[1].as_f64().unwrap(),
                    c[2].as_f64().unwrap(),
                );
                let ahead = -(center - eye).dot(back);
                if ahead > 0. {
                    distance = ahead;
                }
            }
            let q = Quat::from_rot_matrix(Matrix3::new([
                [right.x, right.y, right.z],
                [up.x, up.y, up.z],
                [back.x, back.y, back.z],
            ]))
            .map_err(|e| e.to_string())?;
            let aspect = f64::from(view.width()) / f64::from(view.height().max(1));
            out.push(tool("set_view",json!({"rotation":rotation(q),"center":vec3(eye-back.scale(distance)),"zoom":2.*distance*15_f64.to_radians().tan()/aspect,"distance":distance}))?);
            out.push(tool("set_view", json!({"fitSlab":true}))?);
            out.push(tool("set_projection", json!({"perspective":true}))?);
            return Ok(out);
        }
        "set" | "graphics silhouettes" => {
            if name == "set" {
                if args.values().all(String::is_empty) {
                    let props = crate::properties::node_property_values(
                        context.state,
                        camera,
                        "scene",
                        None,
                    )
                    .map_err(|e| e.to_string())?;
                    return Ok(vec![output(format!(
                        "Background color: {}",
                        props.get("bgcolor").map(String::as_str).unwrap_or("")
                    ))]);
                }
                if !value("bgColor").is_empty() {
                    out.push(tool("set_node_prop",json!({"nodeType":"scene","prop":"bgcolor","value":colors::split(value("bgColor")).0}))?);
                }
                if ["silhouettes", "silhouetteWidth", "silhouetteColor"]
                    .into_iter()
                    .all(|k| value(k).is_empty())
                {
                    return Ok(out);
                }
            }
            let (enabled, width, color) = if name == "set" {
                (
                    value("silhouettes"),
                    value("silhouetteWidth"),
                    value("silhouetteColor"),
                )
            } else {
                (value("enable"), value("width"), value("color"))
            };
            let pixel = camera.zoom() / f64::from(view.height().max(1));
            let mut rends = Vec::new();
            for node in crate::console::nodes(context.state)?
                .into_iter()
                .filter(|n| {
                    n.kind == "renderer"
                        && !matches!(
                            n.class.as_str(),
                            "simple"
                                | "trace"
                                | "spline"
                                | "contour"
                                | "gpu_mapmesh"
                                | "*selection"
                                | "*unitcell"
                                | "*namelabel"
                        )
                })
            {
                let props = crate::properties::node_property_values(
                    context.state,
                    camera,
                    "renderer",
                    Some(Uid::new(node.id)),
                )
                .map_err(|e| e.to_string())?;
                if props.contains_key("egtype") {
                    rends.push((node.id, props));
                }
            }
            if enabled.is_empty() && width.is_empty() && color.is_empty() {
                let enabled = rends.iter().find(|(_, p)| p["egtype"] == "silhouette");
                let first = enabled.or(rends.first());
                let width = first
                    .and_then(|(_, p)| p.get("eglinew"))
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.)
                    / pixel;
                out.push(output(format!(
                    "Silhouettes: {}, width {}, color {}",
                    enabled.is_some(),
                    precision(width, 3),
                    first
                        .and_then(|(_, p)| p.get("egcolor"))
                        .map(String::as_str)
                        .unwrap_or("")
                )));
                return Ok(out);
            }
            if !width.is_empty() {
                out.push(output(format!("Warning: {name}: the width is converted at the current zoom (CueMol's outline width is in angstroms, so it grows as you zoom in)")));
            }
            for (id, _) in rends {
                let mut props = Vec::new();
                if !enabled.is_empty() {
                    props.push((
                        "egtype",
                        if enabled == "true" {
                            "silhouette"
                        } else {
                            "none"
                        }
                        .into(),
                    ));
                }
                if !width.is_empty() {
                    props.push((
                        "eglinew",
                        (width.parse::<f64>().map_err(|e| e.to_string())? * pixel).to_string(),
                    ));
                }
                if !color.is_empty() {
                    props.push(("egcolor", colors::split(color).0.into()));
                }
                for (prop, value) in props {
                    out.push(tool(
                        "set_node_prop",
                        json!({"nodeType":"renderer","nodeId":id,"prop":prop,"value":value}),
                    )?);
                }
            }
            return Ok(out);
        }
        _ => return Err(format!("Unknown view command: {name}")),
    };
    out.push(tool("set_view", change)?);
    Ok(out)
}
