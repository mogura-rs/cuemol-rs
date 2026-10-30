use super::*;
use crate::{MeshVertex, RenderAtom, RenderMesh, RenderTransparencyGroup};

fn receiver() -> Hit {
    Hit {
        order: usize::MAX,
        distance: 1.,
        position: Vector4::new3(0., 0., 0.),
        normal: Vector4::new3(0., 0., 1.),
        color: [0.5, 0.5, 0.5, 1.],
        unlit: false,
        is_volume: false,
        transparency_group: None,
    }
}
fn camera() -> Camera {
    let mut c = Camera::new();
    c.set_zoom(4.);
    c.set_cam_dist(10.);
    c.set_slab_depth(100.);
    c
}
fn ray() -> Ray {
    Ray {
        origin: Vector4::new3(0., 0., 10.),
        direction: Vector4::new3(0., 0., -1.),
    }
}

#[test]
fn transparent_background_fog_changes_coverage_without_revealing_hidden_geometry() {
    let mut snapshot = RenderSceneSnapshot::empty();
    for (z, color) in [(-2.5, [1., 0., 0., 1.]), (-4., [0., 1., 0., 1.])] {
        snapshot.meshes.push(RenderMesh {
            vertices: [(-2., -2.), (2., -2.), (0., 2.)]
                .into_iter()
                .map(|(x, y)| MeshVertex {
                    pick_atom_index: None,
                    position: Vector4::new3(x, y, z),
                    normal: Vector4::new3(0., 0., 1.),
                    color,
                })
                .collect(),
            indices: vec![0, 1, 2],
        });
    }
    snapshot.mark_renderer_section(1., "nolighting");
    let mut camera = camera();
    camera.set_slab_depth(10.);
    let params = UmbreonRenderParams {
        use_gi: false,
        transparent_background: true,
        show_edge_lines: false,
        ..Default::default()
    };
    let sample = trace_sample_pass(
        &snapshot,
        &RayIndex::new(&snapshot),
        ray(),
        &camera,
        [0., 0., 0., 1.],
        &params,
        None,
        None,
    )
    .0;
    assert_eq!(sample, [1., 0., 0., 0.5]);
    let hatch = npr::ResolvedHatch::new(
        &npr::HatchRenderParams {
            coloring: "flat color".into(),
            tone_strength: 0.,
            default_edges: false,
            ..Default::default()
        },
        1,
    )
    .unwrap();
    let hatch_sample = npr::trace_npr_pass(
        &snapshot,
        &RayIndex::new(&snapshot),
        ray(),
        &camera,
        [0., 0., 0., 1.],
        &params,
        &hatch,
        0.,
        0.,
        None,
        None,
    )
    .0;
    assert_eq!(hatch_sample, sample);
}

#[test]
fn direct_light_energy_and_shadowless_headlight_follow_the_settings() {
    let s = RenderSceneSnapshot::empty();
    let index = RayIndex::new(&s);
    let hit = receiver();
    let mut p = UmbreonRenderParams {
        light_intensity: 1.,
        flash_fraction: 1.,
        ambient_fraction: 0.,
        show_edge_lines: false,
        ..Default::default()
    };
    let first = lighting::shade(&s, &index, hit, ray(), &camera(), &p, None, None);
    p.light_intensity = 2.;
    let second = lighting::shade(&s, &index, hit, ray(), &camera(), &p, None, None);
    for c in 0..3 {
        assert!((first[c] - 0.5).abs() < 1e-12);
        assert!((second[c] - 0.9).abs() < 1e-12);
    }
    p.shadows = true;
    p.light_radius = 20.;
    p.shadow_samples = 8;
    assert_eq!(
        second,
        lighting::shade(&s, &index, hit, ray(), &camera(), &p, None, None)
    );
    p.flash_fraction = 0.;
    assert!(lighting::shade(&s, &index, hit, ray(), &camera(), &p, None, None)[0] < second[0]);
}
#[test]
fn diffuse_gather_conserves_uniform_environment_and_transfers_wall_color() {
    let mut s = RenderSceneSnapshot::empty();
    let mut p = UmbreonRenderParams {
        use_gi: true,
        light_intensity: 1.3,
        ambient_fraction: 0.6,
        gi_env_intensity: 1.,
        gi_intensity: 1.,
        gi_sky_gradient: false,
        gi_samples: 256,
        ..Default::default()
    };
    let uniform = lighting::indirect(&s, &RayIndex::new(&s), receiver(), &camera(), &p, None);
    for c in uniform {
        assert!((c - 0.78).abs() < 1e-12);
    }
    s.meshes.push(RenderMesh {
        vertices: [(-5., 0.), (5., 0.), (5., 10.), (-5., 10.)]
            .into_iter()
            .map(|(y, z)| MeshVertex {
                pick_atom_index: None,
                position: Vector4::new3(-1., y, z),
                normal: Vector4::new3(1., 0., 0.),
                color: [1., 0.02, 0.01, 1.],
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
    });
    p.gi_env_intensity = 0.;
    let bounced = lighting::indirect(&s, &RayIndex::new(&s), receiver(), &camera(), &p, None);
    assert!(bounced[0] > 0.01, "{bounced:?}");
    assert!(bounced[0] > bounced[1] * 40.);
    assert!(bounced[0] > bounced[2] * 80.);
    p.gi_intensity = 0.;
    assert_eq!(
        lighting::indirect(&s, &RayIndex::new(&s), receiver(), &camera(), &p, None),
        [0.; 3]
    );
}
#[test]
fn ao_separates_contact_and_shape_with_bounded_radii() {
    let mut s = RenderSceneSnapshot::empty();
    s.atoms
        .push(RenderAtom::new(Vector4::new3(0., 0., 0.), 4., None));
    let index = RayIndex::new(&s);
    let mut p = UmbreonRenderParams {
        ao_samples: 256,
        ao_distance: 2.,
        ao_multi_scale: false,
        ..Default::default()
    };
    assert_eq!(
        lighting::occlusion(&s, &index, receiver(), &p, None).open,
        1.
    );
    p.ao_distance = 8.;
    assert!(lighting::occlusion(&s, &index, receiver(), &p, None).open < 1e-12);
    p.ao_multi_scale = true;
    let ao = lighting::occlusion(&s, &index, receiver(), &p, None);
    assert!((ao.open - 0.9).abs() < 1e-12);
    assert_eq!(ao.contact, 1.);
    assert!((ao.shape - 7. / 9.).abs() < 1e-12);
    assert!((ao.bent.length3d() - 1.).abs() < 1e-12);
    p.ao_falloff_power = 2.;
    assert!(lighting::occlusion(&s, &index, receiver(), &p, None).open > ao.open);
}
fn group(alpha: f32) -> RenderTransparencyGroup {
    RenderTransparencyGroup {
        picking: None,
        name: "veil".into(),
        alpha,
        material: "default".into(),
        edge_group: String::new(),
        edge_style: None,
        atom_range: 0..0,
        bond_range: 0..0,
        mesh_range: 0..0,
    }
}
#[test]
fn per_pixel_veil_weights_stay_nonnegative_and_equal_alphas_share_a_pass() {
    let mut s = RenderSceneSnapshot::empty();
    s.transparency_groups = vec![group(0.75), group(0.6), group(0.75)];
    let index = RayIndex::new(&s);
    assert_eq!(index.translucent_groups.len(), 2);
    let p = UmbreonRenderParams::default();
    let composite = blend_sample(&index, &p, |group| match group {
        None => ([1.; 4], 10.),
        Some(0) => ([1., 0., 0., 1.], 5.),
        Some(_) => ([0., 0., 1., 1.], 6.),
    });
    for (a, b) in composite
        .into_iter()
        .zip([0.318_546_778, 0.010_022_826, 0.214_041_14, 1.])
    {
        assert!((a - b).abs() < 1e-6);
    }
    let no_coverage = blend_sample(&index, &p, |_| ([0.3, 0.4, 0.5, 1.], 10.));
    assert_eq!(no_coverage, [0.3, 0.4, 0.5, 1.]);
    let layers = blend_sample(
        &index,
        &UmbreonRenderParams {
            per_pixel_blend: false,
            ..p
        },
        |group| match group {
            None => ([1.; 4], 10.),
            Some(0) => ([1., 0., 0., 1.], 5.),
            Some(_) => ([0., 0., 1., 1.], 6.),
        },
    );
    for (a, b) in layers
        .into_iter()
        .zip([0.132_868_322, 0., 0.050_876_088, 1.])
    {
        assert!((a - b).abs() < 1e-6);
    }
}
#[test]
fn nearest_bvh_hit_respects_distance_and_filter_without_allocating_hits() {
    let mut s = RenderSceneSnapshot::empty();
    for z in [0., -3., -6.] {
        s.atoms
            .push(RenderAtom::new(Vector4::new3(0., 0., z), 1., None));
    }
    let index = RayIndex::new(&s);
    let r = ray();
    assert!(index.nearest(r, 8., None).is_none());
    assert_eq!(index.nearest(r, 20., None).unwrap().distance, 9.);
    assert_eq!(
        index
            .nearest_where(r, 20., None, |h| h.position.z < 0.)
            .unwrap()
            .distance,
        12.
    );
}
#[test]
fn crease_angle_controls_fold_contours_without_inking_a_smooth_plane() {
    let mut s = RenderSceneSnapshot::empty();
    for (lo, hi, n) in [
        (-2., 0., Vector4::new3(0., 0., 1.)),
        (0., 2., Vector4::new3(0.8, 0., 0.6)),
    ] {
        s.meshes.push(RenderMesh {
            vertices: [(lo, -2.), (hi, -2.), (hi, 2.), (lo, 2.)]
                .into_iter()
                .map(|(x, y)| MeshVertex {
                    pick_atom_index: None,
                    position: Vector4::new3(x, y, 0.),
                    normal: n,
                    color: [1.; 4],
                })
                .collect(),
            indices: vec![0, 1, 2, 0, 2, 3],
        });
    }
    let r = Ray {
        origin: Vector4::new3(-0.01, 0., 10.),
        ..ray()
    };
    let index = RayIndex::new(&s);
    let h = index.nearest(r, 20., None).unwrap();
    let mut p = UmbreonRenderParams {
        width: 100,
        height: 100,
        perspective: false,
        crease_limit: 45.,
        contact_edges: false,
        ..Default::default()
    };
    assert!(edge_at(&index, h, r, &camera(), &p, None, false));
    p.crease_limit = 90.;
    assert!(!edge_at(&index, h, r, &camera(), &p, None, false));
    p.show_edge_lines = false;
    assert!(!edge_at(&index, h, r, &camera(), &p, None, true));
}
