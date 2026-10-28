use crate::{RenderEdgeStyle, RenderSceneSnapshot};
use std::collections::BTreeMap;

pub(super) struct EdgeGroups {
    sections: Vec<usize>,
    styles: Vec<Option<RenderEdgeStyle>>,
}

impl EdgeGroups {
    pub fn new(snapshot: &RenderSceneSnapshot, pixel_scale: f32) -> Self {
        let mut keys = BTreeMap::new();
        let mut sections = Vec::new();
        let mut styles = Vec::new();
        for section in &snapshot.transparency_groups {
            let style = section.edge_style.map(|mut style| {
                style.color[3] = 1.0;
                style.width = style.width.max(pixel_scale);
                style
            });
            let key = if section.edge_group.is_empty() {
                match style {
                    Some(style) => format!(
                        "style:{:?}:{:.4}:{:.4}:{:.4}:{:.4}:{:.4}",
                        style.mode,
                        if pixel_scale > 0.0 {
                            style.width / pixel_scale
                        } else {
                            style.width
                        },
                        style.color[0],
                        style.color[1],
                        style.color[2],
                        style.color[3]
                    ),
                    None => "style:none".to_string(),
                }
            } else {
                format!("name:{}", section.edge_group)
            };
            let next = styles.len();
            let group = *keys.entry(key).or_insert_with(|| {
                styles.push(None);
                next
            });
            if styles[group].is_none() {
                styles[group] = style;
            }
            sections.push(group);
        }
        Self { sections, styles }
    }

    pub fn same(&self, first: Option<usize>, second: Option<usize>) -> bool {
        first.and_then(|id| self.sections.get(id)) == second.and_then(|id| self.sections.get(id))
    }

    pub fn style(&self, section: Option<usize>) -> Option<RenderEdgeStyle> {
        section
            .and_then(|id| self.sections.get(id))
            .and_then(|id| self.styles.get(*id))
            .copied()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raytrace::{render_umbreon_snapshot, UmbreonRenderParams};
    use crate::{MeshVertex, RenderAtom, RenderEdgeMode, RenderMesh, RenderTransparencyGroup};
    use cuemol_core::Vector4;
    use cuemol_qsys::Camera;

    fn style(red: bool) -> RenderEdgeStyle {
        RenderEdgeStyle {
            mode: RenderEdgeMode::Edges,
            width: 0.1,
            color: if red {
                [1., 0., 0., 1.]
            } else {
                [0., 0., 1., 1.]
            },
        }
    }

    fn section(name: &str, edge: Option<RenderEdgeStyle>) -> RenderTransparencyGroup {
        RenderTransparencyGroup {
            name: name.into(),
            alpha: 1.,
            material: "matte".into(),
            edge_group: name.into(),
            edge_style: edge,
            atom_range: 0..0,
            bond_range: 0..0,
            mesh_range: 0..0,
        }
    }

    #[test]
    fn style_groups_ignore_section_names_and_alpha_and_named_groups_choose_first_enabled_style() {
        let mut scene = RenderSceneSnapshot::empty();
        scene.transparency_groups = vec![
            section("", Some(style(true))),
            section("", Some(style(true))),
            section("", Some(style(false))),
            section("joined", None),
            section("joined", Some(style(false))),
            section("joined", Some(style(true))),
        ];
        scene.transparency_groups[1].name = "other object".into();
        scene.transparency_groups[1].alpha = 0.3;
        let groups = EdgeGroups::new(&scene, 0.01);
        assert!(groups.same(Some(0), Some(1)));
        assert!(!groups.same(Some(0), Some(2)));
        assert!(!groups.same(Some(2), Some(4)));
        for id in 3..6 {
            assert_eq!(groups.style(Some(id)), Some(style(false)));
        }
        let groups = EdgeGroups::new(&scene, 0.5);
        assert_eq!(groups.style(Some(0)).unwrap().width, 0.5);
    }

    #[test]
    fn named_style_outlines_members_without_their_own_edges() {
        let mut scene = RenderSceneSnapshot::empty();
        scene.atoms = vec![
            RenderAtom::new(Vector4::ZERO, 1., None),
            RenderAtom::new(Vector4::new3(4., 0., 0.), 1., None),
        ];
        scene.transparency_groups = vec![
            section("joined", None),
            section("joined", Some(style(true))),
        ];
        scene.transparency_groups[0].atom_range = 0..1;
        scene.transparency_groups[1].atom_range = 1..2;
        let index = super::super::RayIndex::new(&scene);
        for x in [0., 4.] {
            let ray = super::super::Ray {
                origin: Vector4::new3(x, 1.05, 5.),
                direction: Vector4::new3(0., 0., -1.),
            };
            let hit = index
                .nearest(ray, 10., None)
                .expect("shared outline covers this ray");
            assert!(hit.unlit);
            assert_eq!(hit.color, [1., 0., 0., 1.]);
        }
    }

    #[test]
    fn grouping_removes_contact_ink_but_retains_outer_contours() {
        let mut scene = RenderSceneSnapshot::empty();
        for (id, left, right) in [(0, -2., 0.), (1, 0., 2.)] {
            scene.meshes.push(RenderMesh {
                vertices: [(left, -2.), (right, -2.), (right, 2.), (left, 2.)]
                    .into_iter()
                    .map(|(x, y)| MeshVertex {
                        position: Vector4::new3(x, y, 0.),
                        normal: Vector4::new3(0., 0., 1.),
                        color: [1.; 4],
                    })
                    .collect(),
                indices: vec![0, 1, 2, 0, 2, 3],
            });
            let mut section = section(if id == 0 { "left" } else { "right" }, Some(style(true)));
            section.mesh_range = id..id + 1;
            scene.transparency_groups.push(section);
        }
        let mut camera = Camera::new();
        camera.set_zoom(6.);
        camera.set_cam_dist(10.);
        camera.set_slab_depth(20.);
        let params = UmbreonRenderParams {
            width: 64,
            height: 64,
            supersample: 1,
            use_gi: false,
            gi_denoise: false,
            perspective: false,
            contact_edges: true,
            crease_limit: -1.,
            ..Default::default()
        };
        let separate = render_umbreon_snapshot(&scene, &camera, [1.; 4], &params).unwrap();
        scene.transparency_groups[1].edge_group = "left".into();
        let joined = render_umbreon_snapshot(&scene, &camera, [1.; 4], &params).unwrap();
        let pixel = |frame: &crate::UmbreonFrame, x: usize, y: usize| {
            let offset = (y * 64 + x) * frame.components as usize;
            frame.pixels[offset..offset + 3].to_vec()
        };
        assert_eq!(pixel(&separate, 31, 32), [255, 0, 0]);
        let center = pixel(&joined, 31, 32);
        assert!(center.iter().all(|channel| *channel > 100), "{center:?}");
        assert!(joined
            .pixels
            .chunks_exact(joined.components as usize)
            .any(|pixel| pixel[..3] == [255, 0, 0]));
    }
}
