use super::*;

fn atom_scene(pdb: &str, kind: &str) -> (DesktopAppState, Uid, Uid) {
    let mut app = DesktopAppState::new();
    let scene = app.ensure_active_scene();
    let object = app
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("atoms", read_pdb_str(pdb).unwrap());
    let renderer = app
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer(kind, "geometry");
    (app, object, renderer)
}

#[test]
fn object_scale_moves_centers_but_renderer_scale_changes_glyphs() {
    let pdb =
        "ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n";
    let (mut app, object, renderer) = atom_scene(pdb, "cpk");
    let original = app.active_render_snapshot("").unwrap();
    let transform = Matrix4::from_script_string("(2,0,0,0,0,3,0,0,0,0,4,0,5,6,7,1)").unwrap();
    app.scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .set_xform_matrix(transform);
    let moved = app.active_render_snapshot("").unwrap();
    assert_eq!(moved.atoms.len(), 1);
    assert_eq!(moved.atoms[0].center, Vector4::new3(7., 6., 7.));
    assert_eq!(moved.atoms[0].radius, original.atoms[0].radius);
    assert_eq!(
        app.scene_manager().object(object).unwrap().mol().atoms()[0].position,
        Vector4::new3(1., 0., 0.)
    );
    let renderer_transform = "(2,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)";
    app.scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .renderer_mut(renderer)
        .unwrap()
        .set_qsc_attr("xformMat", renderer_transform);
    let stretched = app.active_render_snapshot("").unwrap();
    assert!(stretched.atoms.is_empty());
    assert_eq!(stretched.meshes.len(), 1);
    let group = &stretched.transparency_groups[0];
    assert_eq!(group.mesh_range, 0..1);
    assert_eq!(
        group.picking.as_ref().unwrap().atoms[0].position,
        Vector4::new3(14., 6., 7.)
    );
    for vertex in &stretched.meshes[0].vertices {
        let offset = vertex.position - Vector4::new3(14., 6., 7.);
        let radius = Vector4::new3(offset.x / 2., offset.y, offset.z).length3d();
        assert!((radius - f64::from(original.atoms[0].radius)).abs() < 1e-6);
        assert_eq!(vertex.pick_atom_index, Some(0));
    }
}

#[test]
fn object_rotation_preserves_anisotropic_tensor_axes() {
    let mut pdb =
        "ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\n"
            .to_string();
    let mut line = "ANISOU    1  CA  ALA A   1".to_string();
    while line.len() < 28 {
        line.push(' ');
    }
    for value in [1000, 4000, 9000, 0, 0, 0] {
        line.push_str(&format!("{value:>7}"));
    }
    pdb.push_str(&line);
    pdb.push_str("\nEND\n");
    let (mut app, object, renderer) = atom_scene(&pdb, "anisou");
    app.scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .renderer_mut(renderer)
        .unwrap()
        .set_qsc_attr("drawdisc", "false");
    let original = app.active_render_snapshot("").unwrap();
    assert!(!original.meshes[0].vertices.is_empty());
    let transform = Matrix4::from_script_string("(0,1,0,0,-1,0,0,0,0,0,1,0,3,2,1,1)").unwrap();
    app.scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .set_xform_matrix(transform);
    let moved = app.active_render_snapshot("").unwrap();
    for (before, after) in original
        .meshes
        .iter()
        .flat_map(|m| &m.vertices)
        .zip(moved.meshes.iter().flat_map(|m| &m.vertices))
    {
        assert!((after.position - before.position - Vector4::new3(2., 3., 1.)).length3d() < 1e-7);
        assert_eq!(before.normal, after.normal);
    }
    assert_eq!(
        original.meshes[0].triangle_count(),
        moved.meshes[0].triangle_count()
    );
}
