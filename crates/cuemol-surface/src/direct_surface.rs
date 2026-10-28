use crate::{
    backends, DirectSurfRenderer, DistFieldSurfBuilder, MolCoord, SurfSphere, SurfaceMesh,
};
use cuemol_core::Vector4;

pub(super) fn voxel_size(detail: i32) -> f64 {
    1.0 / (1.0 + 0.2 * (i64::from(detail) - 1).clamp(0, 99) as f64)
}

fn bounded_spacing(spheres: &[(SurfSphere, i32)], probe: f64, spacing: f64) -> f64 {
    let Some((first, _)) = spheres.first() else {
        return spacing;
    };
    let mut lo = first.center;
    let mut hi = first.center;
    let mut radius: f64 = 0.0;
    for (sphere, _) in spheres {
        lo.x = lo.x.min(sphere.center.x);
        lo.y = lo.y.min(sphere.center.y);
        lo.z = lo.z.min(sphere.center.z);
        hi.x = hi.x.max(sphere.center.x);
        hi.y = hi.y.max(sphere.center.y);
        hi.z = hi.z.max(sphere.center.z);
        radius = radius.max(sphere.radius);
    }
    let size = hi - lo;
    let cells = |step: f64| {
        let padding = radius + 2.0 * probe + 3.0 * step;
        [size.x, size.y, size.z]
            .into_iter()
            .map(|length| ((length + 2.0 * padding) / step).ceil() + 1.0)
            .product::<f64>()
    };
    const MAX_CELLS: f64 = 64_000_000.0;
    if cells(spacing) <= MAX_CELLS {
        return spacing;
    }
    let mut upper = 4.0_f64.max(spacing);
    while cells(upper) > MAX_CELLS {
        upper *= 2.0;
    }
    let mut lower = 0.0;
    for _ in 0..40 {
        let middle = (upper + lower) * 0.5;
        if cells(middle) > MAX_CELLS {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    upper
}

pub(super) fn render(renderer: &DirectSurfRenderer, molecule: &MolCoord) -> SurfaceMesh {
    let spheres = molecule
        .atoms()
        .iter()
        .filter(|atom| renderer.selection.matches_in_molecule(atom, molecule))
        .filter(|atom| atom.alt_loc.is_none_or(|alt| alt == 'A' || alt == ' '))
        .map(|atom| {
            (
                SurfSphere {
                    center: Vector4::new3(
                        atom.position.x as f32 as f64,
                        atom.position.y as f32 as f64,
                        atom.position.z as f32 as f64,
                    ),
                    radius: renderer.vdw_radii.radius_for_element(&atom.element),
                },
                atom.serial,
            )
        })
        .collect::<Vec<_>>();
    if spheres.is_empty()
        || spheres.iter().any(|(sphere, _)| {
            !sphere.radius.is_finite()
                || sphere.radius <= 0.0
                || [sphere.center.x, sphere.center.y, sphere.center.z]
                    .iter()
                    .any(|value| !value.is_finite())
        })
    {
        return SurfaceMesh::new();
    }
    let probe = renderer.probe_radius.max(0.0);
    if !probe.is_finite() {
        return SurfaceMesh::new();
    }
    let voxel = voxel_size(renderer.detail);
    let kernel = match renderer.surface_algorithm {
        DirectSurfRenderer::DS_EDTSURF => {
            Some((1.0 / voxel, 3 + renderer.surface_type.clamp(0, 2) as u32))
        }
        DirectSurfRenderer::DS_MESHMS if renderer.surface_type == DirectSurfRenderer::DS_SES => {
            let min_size = (12.0 * spheres.len() as f64 / 16_000_000.0).sqrt();
            let size = (voxel * 1.61).max(min_size);
            Some((size, 6))
        }
        _ => None,
    };
    if let Some((density, backend)) = kernel {
        if let Ok(mut mesh) = backends::direct_surface_kernel(&spheres, density, probe, backend) {
            if !mesh.is_empty() {
                assign_missing_atom_ids(&mut mesh);
                return mesh;
            }
        }
    }
    let spacing = bounded_spacing(
        &spheres,
        if renderer.surface_type == DirectSurfRenderer::DS_VDW {
            0.0
        } else {
            probe
        },
        renderer.grid_spacing(),
    );
    let mut builder = DistFieldSurfBuilder::new();
    builder.set_probe_radius(probe);
    builder.set_grid_spacing(spacing);
    builder.set_surface_type(renderer.surface_type);
    for (sphere, id) in spheres {
        builder.add_atom(sphere.center, sphere.radius, id);
    }
    builder.build();
    let mut mesh = builder.into_mesh();
    for vertex in &mut mesh.vertices {
        vertex.position = Vector4::new3(
            vertex.position.x as f32 as f64,
            vertex.position.y as f32 as f64,
            vertex.position.z as f32 as f64,
        );
        vertex.normal = Vector4::new3(
            vertex.normal.x as f32 as f64,
            vertex.normal.y as f32 as f64,
            vertex.normal.z as f32 as f64,
        );
    }
    assign_missing_atom_ids(&mut mesh);
    mesh
}

fn assign_missing_atom_ids(mesh: &mut SurfaceMesh) {
    for _ in 0..3 {
        let mut changed = false;
        for face in &mesh.triangles {
            let known = face
                .iter()
                .map(|&i| mesh.vertices[i as usize].info)
                .find(|&id| id != u32::MAX);
            if let Some(known) = known {
                for &index in face {
                    if mesh.vertices[index as usize].info == u32::MAX {
                        mesh.vertices[index as usize].info = known;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_calibration_and_grid_budget_match_the_reference() {
        assert_eq!(voxel_size(1), 1.0);
        assert_eq!(voxel_size(6), 0.5);
        assert_eq!(voxel_size(16), 0.25);
        let spheres = [
            (
                SurfSphere {
                    center: Vector4::ZERO,
                    radius: 1.7,
                },
                1,
            ),
            (
                SurfSphere {
                    center: Vector4::new3(120.0, 120.0, 120.0),
                    radius: 1.7,
                },
                2,
            ),
        ];
        let step = bounded_spacing(&spheres, 1.4, 0.15);
        let pad = 1.7 + 2.8 + 3.0 * step;
        assert!((((120.0 + 2.0 * pad) / step).ceil() + 1.0).powi(3) <= 64_000_000.0);
        assert_eq!(step, bounded_spacing(&spheres, 1.4, 0.17));
    }

    #[test]
    fn algorithms_preserve_atom_ids_selection_and_non_ses_fallback() {
        let mol = cuemol_molstr::read_pdb_str("ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nATOM      2  O   GLY A   1       2.000   0.000   0.000  1.00 10.00           O\n").unwrap();
        let mut renderer = DirectSurfRenderer::default();
        let distance = renderer.render_mesh(&mol);
        assert!(!distance.is_empty());
        for algorithm in [
            DirectSurfRenderer::DS_EDTSURF,
            DirectSurfRenderer::DS_MESHMS,
        ] {
            renderer.set_surface_algorithm(algorithm);
            let mesh = renderer.render_mesh(&mol);
            assert!(!mesh.is_empty());
            assert!(mesh.vertices.iter().all(|v| [1, 2].contains(&v.info)));
            assert_ne!(mesh, distance);
        }
        renderer.set_surface_type(DirectSurfRenderer::DS_SAS);
        let fallback = renderer.render_mesh(&mol);
        renderer.set_surface_algorithm(DirectSurfRenderer::DS_DISTFIELD);
        assert_eq!(fallback, renderer.render_mesh(&mol));
        renderer.set_selection(cuemol_molstr::Selector::None);
        assert!(renderer.render_mesh(&mol).is_empty());
    }
}
