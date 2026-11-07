use crate::commands::{scene, uid, view_json};
use cuemol_core::{CueMolError, Quat, Result, Vector4};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::Value;

pub struct PreparedCurrentView {
    scene: cuemol_core::Uid,
    camera: Camera,
    pub xml: String,
}

pub fn prepare_current_view(
    state: &DesktopAppState,
    camera: &Camera,
) -> Result<PreparedCurrentView> {
    let scene = scene(state)?;
    let mut camera = camera.clone();
    camera.set_name("__current");
    camera.set_source("");
    camera.set_alt_source("");
    camera.set_ui_order(-1);
    camera.clear_visibility_settings();
    Ok(PreparedCurrentView {
        scene,
        xml: camera.to_xml_string(),
        camera,
    })
}

impl PreparedCurrentView {
    pub fn finish(self, state: &mut DesktopAppState) -> Result<()> {
        let scene = state
            .scene_manager_mut()
            .scene_mut(self.scene)
            .ok_or_else(|| CueMolError::not_found("view source scene"))?;
        scene.set_camera(self.camera);
        Ok(())
    }
}

fn numbers<const N: usize>(args: &Value, key: &str) -> Result<Option<[f64; N]>> {
    if args[key].is_null() {
        return Ok(None);
    }
    let values = args[key]
        .as_array()
        .filter(|values| values.len() == N)
        .ok_or_else(|| CueMolError::invalid_argument(format!("{key} needs {N} numbers")))?;
    let mut result = [0.; N];
    for (output, value) in result.iter_mut().zip(values) {
        *output = value
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| CueMolError::invalid_argument(format!("{key} needs finite numbers")))?;
    }
    Ok(Some(result))
}

fn number(args: &Value, key: &str) -> Result<Option<f64>> {
    if args[key].is_null() {
        return Ok(None);
    }
    args[key]
        .as_f64()
        .filter(|n| n.is_finite())
        .map(Some)
        .ok_or_else(|| CueMolError::invalid_argument(format!("{key} must be a finite number")))
}

fn fitted_slab(state: &DesktopAppState, camera: &Camera, args: &Value) -> Result<f64> {
    let current = state
        .scene_manager()
        .scene(scene(state)?)
        .ok_or_else(|| CueMolError::not_found("scene"))?;
    let target = if args["objId"].is_null() {
        None
    } else {
        Some(uid(args, "objId")?)
    };
    let rotation = camera.rotation().to_matrix();
    let center_z = rotation.mul_vec(camera.center()).z;
    let mut far: Option<f64> = None;
    for object in current
        .objects()
        .iter()
        .filter(|object| target.is_none_or(|id| object.uid() == id))
    {
        let mut bounds: Option<(Vector4, Vector4)> = None;
        for atom in object.mol().atoms() {
            let mut position = atom.position;
            object.xform_matrix().xform3d(&mut position);
            bounds = Some(match bounds {
                None => (position, position),
                Some((lo, hi)) => (
                    Vector4::new3(
                        lo.x.min(position.x),
                        lo.y.min(position.y),
                        lo.z.min(position.z),
                    ),
                    Vector4::new3(
                        hi.x.max(position.x),
                        hi.y.max(position.y),
                        hi.z.max(position.z),
                    ),
                ),
            });
        }
        let Some((lo, hi)) = bounds else { continue };
        for x in [lo.x, hi.x] {
            for y in [lo.y, hi.y] {
                for z in [lo.z, hi.z] {
                    let distance = (rotation.mul_vec(Vector4::new3(x, y, z)).z - center_z).abs();
                    far = Some(far.unwrap_or(0.).max(distance));
                }
            }
        }
    }
    far.map(|far| 2.2 * far).ok_or_else(|| {
        CueMolError::invalid_argument(if target.is_some() {
            "That object has no atoms to fit."
        } else {
            "There are no MolCoord objects to fit."
        })
    })
}

pub(crate) fn set_view(
    state: &DesktopAppState,
    camera: &mut Camera,
    args: &Value,
) -> Result<Value> {
    let zoom = number(args, "zoom")?;
    let distance = number(args, "distance")?;
    let mut slab = number(args, "slab")?;
    let center = numbers::<3>(args, "center")?;
    let rotation = numbers::<4>(args, "rotation")?;
    if args["fitSlab"] == true {
        if slab.is_some() {
            return Err(CueMolError::invalid_argument(
                "fitSlab and slab are mutually exclusive",
            ));
        }
        slab = Some(fitted_slab(state, camera, args)?);
    }
    let mut updated = camera.clone();
    if let Some([w, x, y, z]) = rotation {
        let q = Quat::new(x, y, z, w);
        let length = q.sqlen();
        updated.set_rotation(if !length.is_finite() || length < 1e-4 {
            Quat::IDENTITY
        } else if (length - 1.).abs() < 1e-8 {
            q
        } else {
            q.normalize()?
        });
    }
    if let Some([x, y, z]) = center {
        updated.set_center(Vector4::new3(x, y, z));
    }
    if let Some(value) = distance {
        updated.set_cam_dist(value.max(0.));
    }
    if let Some(value) = slab {
        updated.set_slab_depth(value.clamp(0.1, updated.cam_dist() * 2.));
    }
    if let Some(value) = zoom {
        updated.set_zoom(value.max(0.01));
    }
    *camera = updated;
    Ok(view_json(camera))
}

pub(crate) fn focus_selection(
    state: &DesktopAppState,
    camera: &mut Camera,
    args: &Value,
    orient: bool,
) -> Result<Value> {
    let id = uid(args, "objId")?;
    let current = state
        .scene_manager()
        .scene(scene(state)?)
        .ok_or_else(|| CueMolError::not_found("scene"))?;
    let object = current
        .object(id)
        .ok_or_else(|| CueMolError::not_found("No object with that id in this scene."))?;
    let text = args["selection"]
        .as_str()
        .unwrap_or_else(|| object.mol().selection_text());
    if text.trim().is_empty() {
        return Err(CueMolError::invalid_argument(
            if args["selection"].is_null() {
                "The object has no current selection."
            } else {
                "The selection did not compile."
            },
        ));
    }
    let selector = state.parse_object_selector(id, text)?;
    if object.mol().get_atom_sel_size(&selector) == 0 {
        return Err(CueMolError::invalid_argument(
            "The selection matches no atoms.",
        ));
    }
    let mut view = crate::properties::current_fit_view(state, camera)?;
    if orient {
        object
            .mol()
            .orient_view_with_xform(&mut view, Some(&selector), object.xform_matrix());
    } else if args["zoom"] != false {
        object
            .mol()
            .fit_view_with_xform(&mut view, Some(&selector), object.xform_matrix());
    } else {
        let (sum, count) = object.mol().select_indices(&selector).into_iter().fold(
            (Vector4::ZERO, 0usize),
            |(sum, count), index| {
                let mut position = object.mol().atoms()[index].position;
                object.xform_matrix().xform3d(&mut position);
                (sum + position, count + 1)
            },
        );
        view.set_center(sum.scale(1. / count as f64));
    }
    *camera = view.camera();
    Ok(view_json(camera))
}
