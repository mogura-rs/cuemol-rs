#![cfg(feature = "offscreen")]

use cuemol_core::{Quat, Vector4};
use cuemol_desktop_core::{
    offscreen::{encode_rgba_png, SceneRenderer},
    viewport::{atom_instance, bond_instance},
    DesktopAppState,
};
use cuemol_gfx::{ColorBase, ColorSpec};
use cuemol_qsys::Camera;
use cuemol_render::{
    MeshVertex, RenderAtom, RenderBond, RenderEdgeMode, RenderEdgeStyle, RenderMesh,
    RenderSceneSnapshot, RenderTransparencyGroup,
};
use serde_json::json;

fn solid() -> Option<ColorSpec> {
    Some(ColorSpec {
        base: ColorBase::Html {
            r: 153,
            g: 77,
            b: 26,
            a: 255,
        },
        modifiers: Vec::new(),
    })
}

fn plane(depth: f64, normal: Vector4) -> RenderSceneSnapshot {
    let mut snapshot = RenderSceneSnapshot::empty();
    snapshot.meshes.push(RenderMesh {
        vertices: [(-5.0, -5.0), (5.0, -5.0), (5.0, 5.0), (-5.0, 5.0)]
            .into_iter()
            .map(|(x, y)| MeshVertex {
                position: Vector4::new3(x, y, depth),
                normal,
                color: [0.6, 0.3, 0.1, 1.0],
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
    });
    snapshot
}

#[test]
fn viewport_lighting_stays_in_eye_space_and_fog_uses_the_scene_background() {
    let Some(renderer) = SceneRenderer::new() else {
        eprintln!("Skipping viewport appearance: no GPU adapter");
        return;
    };
    let mut camera = Camera::new();
    camera.set_perspective(false);
    camera.set_cam_dist(40.0);
    camera.set_slab_depth(40.0);
    camera.set_zoom(18.0);
    let background = [0.05, 0.08, 0.12, 1.0];
    let mut sphere = RenderSceneSnapshot::empty();
    sphere
        .atoms
        .push(RenderAtom::new(Vector4::ZERO, 3.0, solid()));
    let render = |snapshot: &RenderSceneSnapshot, camera: &Camera| {
        renderer
            .render_camera(snapshot, camera, 128, 128, background, 0)
            .unwrap()
    };
    let first = render(&sphere, &camera);
    let mut rotated = camera.clone();
    let half_angle = std::f64::consts::FRAC_PI_4;
    rotated.set_rotation(Quat::new(0.0, half_angle.sin(), 0.0, half_angle.cos()));
    let second = render(&sphere, &rotated);
    assert!(
        first.iter().zip(&second).all(|(a, b)| a.abs_diff(*b) <= 1),
        "a sphere's highlight moved with the world axes"
    );
    let fog = render(&plane(-10.0, Vector4::new3(0.0, 0.0, 1.0)), &camera);
    let center = &fog[(64 * 128 + 64) * 4..(64 * 128 + 64) * 4 + 4];
    // Independently sampled from the reference GLSL at half the fog interval.
    assert_eq!(center, &[82, 48, 28, 255]);
    let back = render(&plane(0.0, Vector4::new3(0.0, 0.0, -1.0)), &camera);
    assert_eq!(
        &back[(64 * 128 + 64) * 4..(64 * 128 + 64) * 4 + 4],
        &[61, 31, 10, 255]
    );

    let Some(directory) = std::env::var_os("CUEMOL_APPEARANCE_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    let mut cases = vec![
        ("sphere", sphere.clone(), camera.clone()),
        ("sphere-rotated", sphere.clone(), rotated.clone()),
        (
            "mesh-fog",
            plane(-10.0, Vector4::new3(0.0, 0.0, 1.0)),
            camera.clone(),
        ),
        (
            "mesh-back",
            plane(0.0, Vector4::new3(0.0, 0.0, -1.0)),
            camera.clone(),
        ),
    ];
    let mut perspective = camera.clone();
    perspective.set_perspective(true);
    cases.push(("sphere-perspective", sphere.clone(), perspective.clone()));
    sphere.atoms[0].edge = Some(RenderEdgeStyle {
        mode: RenderEdgeMode::Edges,
        width: 0.15,
        color: [0.1, 0.2, 0.3, 1.0],
    });
    cases.push(("sphere-outline", sphere, camera.clone()));
    for (name, start, end, view) in [
        (
            "cylinder",
            [-4.0, -2.0, 0.0],
            [4.0, 2.0, 0.0],
            camera.clone(),
        ),
        (
            "cylinder-tilted",
            [-4.0, -2.0, 3.0],
            [4.0, 2.0, -3.0],
            camera.clone(),
        ),
        (
            "cylinder-reversed",
            [4.0, 2.0, -3.0],
            [-4.0, -2.0, 3.0],
            camera.clone(),
        ),
        (
            "cylinder-perspective",
            [-4.0, -2.0, -3.0],
            [4.0, 2.0, 3.0],
            perspective,
        ),
        (
            "cylinder-rotated",
            [-4.0, -2.0, 3.0],
            [4.0, 2.0, -3.0],
            rotated,
        ),
    ] {
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.bonds.push(RenderBond::new(
            Vector4::new3(start[0], start[1], start[2]),
            Vector4::new3(end[0], end[1], end[2]),
            0.8,
            solid(),
        ));
        cases.push((name, snapshot, view));
    }
    for (name, stipple, perspective) in [
        ("line", false, false),
        ("line-stipple", true, false),
        ("line-perspective", false, true),
        ("line-perspective-stipple", true, true),
    ] {
        let mut snapshot = RenderSceneSnapshot::empty();
        let mut bond = RenderBond::new(
            Vector4::new3(-4.0, -2.0, 3.0),
            Vector4::new3(4.0, 2.0, -3.0),
            0.1,
            solid(),
        );
        bond.screen_width = Some(3.0);
        bond.screen_stipple = stipple;
        bond.end_color = Some(ColorSpec {
            base: ColorBase::Html {
                r: 26,
                g: 77,
                b: 153,
                a: 255,
            },
            modifiers: Vec::new(),
        });
        snapshot.bonds.push(bond);
        let mut view = camera.clone();
        view.set_perspective(perspective);
        cases.push((name, snapshot, view));
    }
    let mut mesh = plane(0.0, Vector4::new3(0.0, 0.0, 1.0));
    mesh.meshes[0].vertices[0].normal = Vector4::new3(-1.0, 0.0, 0.0);
    mesh.meshes[0].vertices[1].normal = Vector4::new3(0.0, 1.0, 1.0);
    cases.push(("mesh-gouraud", mesh, camera.clone()));
    let reference = std::env::var_os("CUEMOL2_REF_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2")
        });
    let pdb = reference.join("tests/test_data/1CRN.pdb");
    if pdb.is_file() {
        for representation in ["ribbon", "cartoon"] {
            let mut app = DesktopAppState::new();
            app.open_path(&pdb).unwrap();
            let scene_uid = app.active_scene().unwrap();
            let scene = app.scene_manager_mut().scene_mut(scene_uid).unwrap();
            let object_uid = scene.objects()[0].uid();
            let object = scene.object_mut(object_uid).unwrap();
            let renderer_ids = object
                .renderers()
                .iter()
                .map(|r| r.uid())
                .collect::<Vec<_>>();
            for id in renderer_ids {
                object.destroy_renderer(id);
            }
            object.add_renderer(representation, representation);
            let snapshot = app.active_render_snapshot("").unwrap();
            let mut view = camera.clone();
            view.set_center(Vector4::new3(9.3, 9.8, 7.0));
            view.set_zoom(42.0);
            view.set_cam_dist(60.0);
            view.set_slab_depth(80.0);
            cases.push((representation, snapshot, view));
        }
    }
    for (name, snapshot, camera) in cases {
        let rgba = render(&snapshot, &camera);
        std::fs::write(
            directory.join(format!("{name}-rust.png")),
            encode_rgba_png(&rgba, 128, 128),
        )
        .unwrap();
        let rotation = camera.rotation().to_matrix();
        let center = camera.center();
        let mut view = rotation.values;
        for row in 0..3 {
            view[row][3] =
                -(view[row][0] * center.x + view[row][1] * center.y + view[row][2] * center.z);
        }
        view[2][3] -= camera.cam_dist();
        let atoms=snapshot.atoms.iter().filter(|a| a.hit.is_none()).map(|a| {let a=atom_instance(a);json!({"center_radius":a.center_radius,"color":a.color,"edge":a.edge,"edge_color":a.edge_color})}).collect::<Vec<_>>();
        let bonds=snapshot.bonds.iter().map(|b| {let b=bond_instance(b);json!({"from_radius":b.from_radius,"to_edge":b.to_edge,"color":b.color,"end_color":b.end_color,"edge_mode":b.edge_mode,"edge_color":b.edge_color})}).collect::<Vec<_>>();
        let meshes=snapshot.meshes.iter().map(|m| json!({"vertices":m.vertices.iter().map(|v|vec![v.position.x,v.position.y,v.position.z,v.normal.x,v.normal.y,v.normal.z,v.color[0] as f64,v.color[1] as f64,v.color[2] as f64,v.color[3] as f64]).collect::<Vec<_>>(),"indices":m.indices})).collect::<Vec<_>>();
        let data = json!({"size":[128,128],"view":view,"zoom":camera.zoom(),"distance":camera.cam_dist(),"slab":camera.slab_depth(),"perspective":camera.is_perspective(),"background":background,"atoms":atoms,"bonds":bonds,"meshes":meshes});
        std::fs::write(
            directory.join(format!("{name}.json")),
            serde_json::to_vec(&data).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn viewport_composites_transparent_sections_after_opaque_geometry_and_depth_tests_lines() {
    let Some(renderer) = SceneRenderer::new() else {
        eprintln!("Skipping viewport compositing: no GPU adapter");
        return;
    };
    let mut camera = Camera::new();
    camera.set_perspective(false);
    camera.set_cam_dist(40.0);
    camera.set_slab_depth(40.0);
    camera.set_zoom(18.0);
    let background = [0.0, 0.0, 0.0, 1.0];
    let render = |snapshot: &RenderSceneSnapshot| {
        renderer
            .render_camera(snapshot, &camera, 128, 128, background, 0)
            .unwrap()
    };
    let center = |rgba: &[u8]| rgba[(64 * 128 + 64) * 4..(64 * 128 + 64) * 4 + 4].to_vec();
    let mut front = plane(1.0, Vector4::new3(0.0, 0.0, 1.0));
    for vertex in &mut front.meshes[0].vertices {
        vertex.color = [1.0, 0.0, 0.0, 0.5];
    }
    let mut opaque_front = front.clone();
    for vertex in &mut opaque_front.meshes[0].vertices {
        vertex.color[3] = 1.0;
    }
    let red = center(&render(&opaque_front));
    let mut back = plane(0.0, Vector4::new3(0.0, 0.0, 1.0));
    for vertex in &mut back.meshes[0].vertices {
        vertex.color = [0.0, 0.0, 1.0, 1.0];
    }
    let blue = center(&render(&back));
    front.meshes.extend(back.meshes);
    front.transparency_groups = vec![RenderTransparencyGroup {
        name: "front".into(),
        alpha: 0.5,
        material: "default".into(),
        edge_group: String::new(),
        edge_style: None,
        atom_range: 0..0,
        bond_range: 0..0,
        mesh_range: 0..1,
    }];
    let blended = center(&render(&front));
    assert!(
        blended[0].abs_diff(red[0] / 2) <= 2,
        "{blended:?} from {red:?}"
    );
    assert!(
        blended[2].abs_diff(blue[2] / 2) <= 2,
        "{blended:?} over {blue:?}"
    );
    assert_eq!(blended[3], 255);

    let mut scene = plane(0.0, Vector4::new3(0.0, 0.0, 1.0));
    let base = center(&render(&scene));
    let mut line = RenderBond::new(
        Vector4::new3(-4.0, 0.0, -1.0),
        Vector4::new3(4.0, 0.0, -1.0),
        0.1,
        solid(),
    );
    line.screen_width = Some(4.0);
    scene.bonds.push(line);
    assert_eq!(
        center(&render(&scene)),
        base,
        "a hidden screen line overlaid the foreground"
    );
    scene.bonds[0].from.z = 1.0;
    scene.bonds[0].to.z = 1.0;
    assert_eq!(center(&render(&scene)), vec![153, 77, 26, 255]);

    let mut outline = plane(1.0, Vector4::new3(0.0, 0.0, 1.0));
    for vertex in &mut outline.meshes[0].vertices {
        vertex.color = [1.0, 0.0, 0.0, 1.0];
        *vertex = vertex.clone().with_edge_outline(true);
    }
    scene.bonds.clear();
    scene.meshes.extend(outline.meshes);
    assert_eq!(
        center(&render(&scene)),
        base,
        "a silhouette overwrote a filled foreground surface"
    );
    let mut point = RenderBond::new(Vector4::new3(0.0, 0.0, 1.0), Vector4::ZERO, 0.1, solid());
    point.screen_width = Some(4.0);
    point.screen_point = true;
    point.screen_offset = [5.0, 3.0];
    scene.bonds.push(point);
    let shifted = render(&scene);
    assert_eq!(center(&shifted), base);
    assert_eq!(
        &shifted[(67 * 128 + 69) * 4..(67 * 128 + 69) * 4 + 4],
        &[153, 77, 26, 255]
    );
}
