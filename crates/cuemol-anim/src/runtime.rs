use super::{primary_property_motions, renderer_target_ids, AnimMotion};
use cuemol_core::{Matrix4, Uid};
use cuemol_qsys::Scene;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
enum PropertyValue {
    Visible(bool),
    Alpha(f64),
    Transform(Matrix4),
    Other,
}

#[derive(Clone, Debug, PartialEq)]
struct SavedProperty {
    owner: Option<Uid>,
    value: PropertyValue,
    attributes: Vec<(String, Option<String>)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SavedProperties {
    values: BTreeMap<(Uid, String), SavedProperty>,
}

impl SavedProperties {
    pub fn capture<T>(&mut self, scene: &Scene<T>, motions: &[AnimMotion]) {
        for ((target, property), (_, owner)) in primary_property_motions(scene, motions) {
            self.capture_property(scene, target, owner, &property);
        }
        for motion in motions.iter().filter(|motion| !motion.base().disabled()) {
            let (targets, property) = match motion {
                AnimMotion::ShowHide(motion) if motion.fade() => (motion.rend(), "alpha"),
                AnimMotion::Slide(motion) => (motion.rend(), "xformMat"),
                _ => continue,
            };
            for (owner, target) in renderer_target_ids(scene, targets) {
                self.capture_property(scene, target, Some(owner), property);
            }
        }
    }

    fn capture_property<T>(
        &mut self,
        scene: &Scene<T>,
        target: Uid,
        owner: Option<Uid>,
        property: &str,
    ) {
        let (value, attributes) = if let Some(owner) = owner {
            let Some(renderer) = scene
                .object(owner)
                .and_then(|object| object.renderer(target))
            else {
                return;
            };
            let value = match property {
                "visible" => PropertyValue::Visible(renderer.visible()),
                "alpha" | "default_alpha" => PropertyValue::Alpha(renderer.alpha()),
                "xformMat" => PropertyValue::Transform(renderer.xform_matrix()),
                _ => PropertyValue::Other,
            };
            let names: &[&str] = if matches!(property, "alpha" | "default_alpha") {
                &["alpha", "default_alpha"]
            } else {
                &[property]
            };
            let attributes = names
                .iter()
                .map(|name| {
                    (
                        name.to_string(),
                        renderer.property_str(name).map(str::to_string),
                    )
                })
                .collect();
            (value, attributes)
        } else {
            let Some(object) = scene.object(target) else {
                return;
            };
            let value = match property {
                "visible" => PropertyValue::Visible(object.visible()),
                "xformMat" => PropertyValue::Transform(object.xform_matrix()),
                _ => PropertyValue::Other,
            };
            (
                value,
                vec![(
                    property.to_string(),
                    object.property_str(property).map(str::to_string),
                )],
            )
        };
        self.values
            .entry((target, property.to_string()))
            .or_insert(SavedProperty {
                owner,
                value,
                attributes,
            });
    }

    pub fn restore<T>(&mut self, scene: &mut Scene<T>) {
        for (
            (target, _),
            SavedProperty {
                owner,
                value,
                attributes,
            },
        ) in std::mem::take(&mut self.values)
        {
            if let Some(owner) = owner {
                let Some(renderer) = scene
                    .object_mut(owner)
                    .and_then(|object| object.renderer_mut(target))
                else {
                    continue;
                };
                match value {
                    PropertyValue::Visible(value) => renderer.set_visible(value),
                    PropertyValue::Alpha(value) => renderer.set_alpha(value),
                    PropertyValue::Transform(value) => renderer.set_xform_matrix(value),
                    PropertyValue::Other => {}
                }
                for (name, value) in attributes {
                    if let Some(value) = value {
                        renderer.set_property_str(name, value);
                    } else {
                        renderer.remove_property_str(&name);
                    }
                }
            } else {
                let Some(object) = scene.object_mut(target) else {
                    continue;
                };
                match value {
                    PropertyValue::Visible(value) => object.set_visible(value),
                    PropertyValue::Transform(value) => object.set_xform_matrix(value),
                    PropertyValue::Other => {}
                    PropertyValue::Alpha(_) => unreachable!(),
                }
                for (name, value) in attributes {
                    if let Some(value) = value {
                        object.set_property_str(name, value);
                    } else {
                        object.remove_property_str(&name);
                    }
                }
            }
        }
    }
}
