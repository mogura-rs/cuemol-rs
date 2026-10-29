use super::*;

pub(super) fn apply(snapshot: &mut RenderSceneSnapshot, matrix: &Matrix4) {
    if *matrix == Matrix4::IDENTITY {
        return;
    }
    let m = matrix.values;
    let columns = [
        Vector4::new3(m[0][0], m[1][0], m[2][0]),
        Vector4::new3(m[0][1], m[1][1], m[2][1]),
        Vector4::new3(m[0][2], m[1][2], m[2][2]),
    ];
    let cofactors = [
        columns[1].cross(columns[2]),
        columns[2].cross(columns[0]),
        columns[0].cross(columns[1]),
    ];
    let determinant = columns[0].dot3d(cofactors[0]);
    let length_sq = columns[0].dot3d(columns[0]);
    let tolerance = length_sq * 1.0e-10;
    let similarity = length_sq > 0.0
        && columns
            .iter()
            .all(|column| (column.dot3d(*column) - length_sq).abs() <= tolerance)
        && columns[0].dot3d(columns[1]).abs() <= tolerance
        && columns[1].dot3d(columns[2]).abs() <= tolerance
        && columns[2].dot3d(columns[0]).abs() <= tolerance;
    if similarity {
        let scale = length_sq.sqrt() as f32;
        for atom in &mut snapshot.atoms {
            atom.radius *= scale;
            if let Some(edge) = &mut atom.edge {
                edge.width *= scale;
            }
        }
        for bond in &mut snapshot.bonds {
            if bond.screen_width.is_none() {
                bond.radius *= scale;
                if let Some(edge) = &mut bond.edge {
                    edge.width *= scale;
                }
            }
        }
        for group in &mut snapshot.transparency_groups {
            if let Some(edge) = &mut group.edge_style {
                edge.width *= scale;
            }
        }
    } else {
        tessellate_primitives(snapshot);
    }
    for group in &mut snapshot.transparency_groups {
        if let Some(picking) = &mut group.picking {
            for atoms in [&mut picking.atoms, &mut picking.gpu_atoms] {
                for atom in std::sync::Arc::make_mut(atoms) {
                    matrix.xform3d(&mut atom.position);
                }
            }
        }
    }
    for volume in &mut snapshot.volumes {
        volume.apply_xform(matrix);
    }
    for atom in &mut snapshot.atoms {
        matrix.xform3d(&mut atom.center);
    }
    for bond in &mut snapshot.bonds {
        matrix.xform3d(&mut bond.from);
        matrix.xform3d(&mut bond.to);
    }
    for mesh in &mut snapshot.meshes {
        for vertex in &mut mesh.vertices {
            matrix.xform3d(&mut vertex.position);
            let normal = vertex.normal;
            let transformed = cofactors[0].scale(normal.x)
                + cofactors[1].scale(normal.y)
                + cofactors[2].scale(normal.z);
            vertex.normal = transformed
                .scale(if determinant < 0.0 { -1.0 } else { 1.0 })
                .normalize3d()
                .unwrap_or(normal);
            vertex.normal.w = normal.w;
        }
        if determinant < 0.0 {
            for face in mesh.indices.chunks_exact_mut(3) {
                face.swap(1, 2);
            }
        }
    }
    snapshot.recompute_bounds();
}

struct Primitives {
    atoms: Vec<Option<RenderAtom>>,
    bonds: Vec<Option<RenderBond>>,
    meshes: Vec<Option<RenderMesh>>,
}

impl Primitives {
    fn append_section(
        &mut self,
        target: &mut RenderSceneSnapshot,
        atoms: Range<usize>,
        bonds: Range<usize>,
        meshes: Range<usize>,
    ) {
        let mut converted = RenderMesh::new();
        for atom in self.atoms[atoms].iter_mut().filter_map(Option::take) {
            if atom.hit.is_some() {
                target.atoms.push(atom);
                continue;
            }
            let start = converted.vertices.len();
            if let Some(edge) = atom.edge.filter(|edge| edge.width > 0.0) {
                add_sphere_edge(
                    &mut converted,
                    atom.center,
                    f64::from(atom.radius),
                    atom.detail,
                    edge,
                );
            }
            let (rings, segments) = atom
                .detail
                .map(sphere_detail_to_rings_segs)
                .unwrap_or((DEFAULT_SPHERE_RINGS, DEFAULT_SPHERE_SEGS));
            add_sphere_detailed(
                &mut converted,
                atom.center,
                f64::from(atom.radius),
                snapshot_color_rgba(&atom.color),
                rings,
                segments,
            );
            set_owner(&mut converted, start, atom.atom_index);
        }
        for bond in self.bonds[bonds].iter_mut().filter_map(Option::take) {
            if bond.screen_width.is_some() {
                target.bonds.push(bond);
                continue;
            }
            let midpoint = (bond.from + bond.to).scale(0.5);
            let owners = bond
                .atom_indices
                .map(|(a, b)| [Some(a), Some(b)])
                .unwrap_or([None; 2]);
            let intervals = if owners[0] == owners[1] || bond.screen_point {
                vec![(bond.from, bond.to, owners[0])]
            } else {
                vec![
                    (bond.from, midpoint, owners[0]),
                    (midpoint, bond.to, owners[1]),
                ]
            };
            for (from, to, owner) in intervals {
                let start = converted.vertices.len();
                if let Some(edge) = bond.edge.filter(|edge| edge.width > 0.0) {
                    if bond.screen_point {
                        add_sphere_edge(&mut converted, from, f64::from(bond.radius), None, edge);
                    } else {
                        add_cylinder_edge(&mut converted, from, to, f64::from(bond.radius), edge);
                    }
                }
                if bond.screen_point {
                    add_sphere(
                        &mut converted,
                        from,
                        f64::from(bond.radius),
                        snapshot_color_rgba(&bond.color),
                    );
                } else {
                    add_cylinder(
                        &mut converted,
                        from,
                        to,
                        f64::from(bond.radius),
                        snapshot_color_rgba(&bond.color),
                    );
                }
                set_owner(&mut converted, start, owner);
            }
        }
        target
            .meshes
            .extend(self.meshes[meshes].iter_mut().filter_map(Option::take));
        if !converted.vertices.is_empty() {
            target.meshes.push(converted);
        }
    }
}

fn set_owner(mesh: &mut RenderMesh, start: usize, index: Option<usize>) {
    let index = index.and_then(|index| u32::try_from(index).ok());
    for vertex in &mut mesh.vertices[start..] {
        vertex.pick_atom_index = index;
    }
}

fn tessellate_primitives(snapshot: &mut RenderSceneSnapshot) {
    let mut original = Primitives {
        atoms: std::mem::take(&mut snapshot.atoms)
            .into_iter()
            .map(Some)
            .collect(),
        bonds: std::mem::take(&mut snapshot.bonds)
            .into_iter()
            .map(Some)
            .collect(),
        meshes: std::mem::take(&mut snapshot.meshes)
            .into_iter()
            .map(Some)
            .collect(),
    };
    let groups = std::mem::take(&mut snapshot.transparency_groups);
    for mut group in groups {
        let start = (
            snapshot.atoms.len(),
            snapshot.bonds.len(),
            snapshot.meshes.len(),
        );
        original.append_section(
            snapshot,
            group.atom_range.clone(),
            group.bond_range.clone(),
            group.mesh_range.clone(),
        );
        group.atom_range = start.0..snapshot.atoms.len();
        group.bond_range = start.1..snapshot.bonds.len();
        group.mesh_range = start.2..snapshot.meshes.len();
        snapshot.transparency_groups.push(group);
    }
    let counts = (
        original.atoms.len(),
        original.bonds.len(),
        original.meshes.len(),
    );
    original.append_section(snapshot, 0..counts.0, 0..counts.1, 0..counts.2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_scaling_keeps_primitives_and_pixel_widths() {
        let mut scene = RenderSceneSnapshot::empty();
        scene
            .atoms
            .push(RenderAtom::new(Vector4::new3(1., 2., 3.), 0.5, None));
        scene.bonds.push(RenderBond::new(
            Vector4::ZERO,
            Vector4::new3(1., 0., 0.),
            0.2,
            None,
        ));
        scene.bonds.push(
            RenderBond::new(Vector4::ZERO, Vector4::new3(0., 1., 0.), 0.1, None)
                .with_screen_style(7., true),
        );
        scene.mark_named_renderer_section("opaque", 0.7, "glass");
        let transform = Matrix4::from_script_string("(0,3,0,0,-3,0,0,0,0,0,3,0,4,5,6,1)").unwrap();
        scene.apply_xform(&transform);
        assert_eq!(scene.atoms.len(), 1);
        assert!(scene.meshes.is_empty());
        assert_eq!(scene.atoms[0].center, Vector4::new3(-2., 8., 15.));
        assert_eq!(scene.atoms[0].radius, 1.5);
        assert!((scene.bonds[0].radius - 0.6).abs() < 1e-6);
        assert_eq!(scene.bonds[1].screen_width, Some(7.));
        assert_eq!(scene.bonds[1].radius, 0.1);
        assert!(scene.bonds[1].screen_stipple);
        assert_eq!(scene.transparency_groups[0].atom_range, 0..1);
    }

    #[test]
    fn nonuniform_shapes_keep_sections_colors_and_picking_owners() {
        let mut scene = RenderSceneSnapshot::empty();
        scene.atoms.push(
            RenderAtom::new(
                Vector4::ZERO,
                1.,
                Some(cuemol_gfx::parse_color("#ff0000").unwrap()),
            )
            .with_picking(Some(7), Some(2), false),
        );
        scene.bonds.push(
            RenderBond::new(Vector4::ZERO, Vector4::new3(0., 2., 0.), 0.3, None).with_picking(
                Some(7),
                Some((2, 3)),
                false,
            ),
        );
        scene.mark_named_renderer_section("first", 0.4, "glass");
        scene.atoms.push(
            RenderAtom::new(
                Vector4::new3(10., 0., 0.),
                2.,
                Some(cuemol_gfx::parse_color("#0000ff").unwrap()),
            )
            .with_picking(Some(9), Some(8), false),
        );
        let mut second = scene.transparency_groups[0].clone();
        second.name = "second".into();
        second.atom_range = 1..2;
        second.bond_range = 1..1;
        scene.transparency_groups.push(second);
        scene.bonds.push(
            RenderBond::new(Vector4::ZERO, Vector4::new3(1., 1., 0.), 0.1, None)
                .with_screen_style(5., false),
        );
        let transform =
            Matrix4::from_script_string("(-2,0,0,0,0.4,3,0,0,0,0.2,1,0,4,5,6,1)").unwrap();
        let inverse = transform.invert().unwrap();
        scene.apply_xform(&transform);
        assert!(scene.atoms.is_empty());
        assert_eq!(scene.bonds.len(), 1);
        assert_eq!(scene.bonds[0].screen_width, Some(5.));
        assert_eq!(scene.meshes.len(), 2);
        assert_eq!(scene.transparency_groups[0].mesh_range, 0..1);
        assert_eq!(scene.transparency_groups[1].mesh_range, 1..2);
        assert_eq!(scene.transparency_groups[0].alpha, 0.4);
        assert_eq!(scene.transparency_groups[0].material, "glass");
        assert_eq!(scene.transparency_groups[1].name, "second");
        let owners: std::collections::BTreeSet<_> = scene.meshes[0]
            .vertices
            .iter()
            .map(|v| v.pick_atom_index)
            .collect();
        assert_eq!(owners, [Some(2), Some(3)].into_iter().collect());
        for vertex in &scene.meshes[1].vertices {
            let mut original = vertex.position;
            inverse.xform3d(&mut original);
            assert!(((original - Vector4::new3(10., 0., 0.)).length3d() - 2.).abs() < 1e-8);
            assert_eq!(vertex.pick_atom_index, Some(8));
            assert_eq!(vertex.color, [0., 0., 1., 1.]);
        }
    }

    #[test]
    fn normals_follow_transformed_tangents_with_reflection_and_shear() {
        let points = [
            Vector4::ZERO,
            Vector4::new3(1., 1., 0.),
            Vector4::new3(0., 1., 1.),
        ];
        let normal = (points[1] - points[0])
            .cross(points[2] - points[0])
            .normalize3d()
            .unwrap();
        let mut scene = RenderSceneSnapshot::empty();
        scene.meshes.push(RenderMesh {
            vertices: points
                .into_iter()
                .map(|position| {
                    MeshVertex {
                        position,
                        normal,
                        color: [1.; 4],
                        pick_atom_index: Some(4),
                    }
                    .with_back_face_culling(true)
                })
                .collect(),
            indices: vec![0, 1, 2],
        });
        scene.apply_xform(
            &Matrix4::from_script_string("(-2,0,0,0,0.4,3,0,0,0,0.2,1,0,4,5,6,1)").unwrap(),
        );
        let mesh = &scene.meshes[0];
        let [a, b, c] = std::array::from_fn(|i| mesh.vertices[mesh.indices[i] as usize].position);
        let face = (b - a).cross(c - a).normalize3d().unwrap();
        for vertex in &mesh.vertices {
            assert!((vertex.normal.dot3d(face) - 1.).abs() < 1e-10);
            assert!(vertex.culls_back_faces());
            assert_eq!(vertex.pick_atom_index, Some(4));
        }
    }
}
