use crate::{grid_surface, MolCoord, Selector, SurfSphere, SurfaceMesh, SurfaceVertex};
use cuemol_core::{CueMolError, Result, Vector4};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SesBackend {
    Auto,
    MeshMs,
    Ball,
    Rust,
}
impl SesBackend {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "meshms" => Ok(Self::MeshMs),
            "ball" => Ok(Self::Ball),
            "rust" => Ok(Self::Rust),
            _ => Err(CueMolError::invalid_argument(format!(
                "unknown SES backend: {value}"
            ))),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::MeshMs => "meshms",
            Self::Ball => "ball",
            Self::Rust => "rust",
        }
    }
}

pub fn build_molecular_surface(
    molecule: &MolCoord,
    selection: &Selector,
    density: f64,
    probe: f64,
    backend: SesBackend,
) -> Result<(SurfaceMesh, SesBackend)> {
    let spheres: Vec<_> = molecule
        .atoms()
        .iter()
        .filter(|atom| selection.matches_in_molecule(atom, molecule))
        .filter(|atom| atom.alt_loc.is_none_or(|alt| alt == 'A' || alt == ' '))
        .map(|atom| SurfSphere {
            center: atom.position,
            radius: cuemol_molstr::toppar::vdw_radius_for_atom(atom).max(0.1),
        })
        .collect();
    build_surface(&spheres, density, probe, backend)
}

pub fn build_surface(
    spheres: &[SurfSphere],
    density: f64,
    probe: f64,
    backend: SesBackend,
) -> Result<(SurfaceMesh, SesBackend)> {
    if spheres.is_empty()
        || !density.is_finite()
        || density <= 0.
        || !probe.is_finite()
        || probe < 0.
    {
        return Err(CueMolError::invalid_argument(
            "SES needs atoms, positive density, and a nonnegative probe radius",
        ));
    }
    let mut xyzr = Vec::with_capacity(spheres.len() * 4);
    for sphere in spheres {
        let values = [
            sphere.center.x,
            sphere.center.y,
            sphere.center.z,
            sphere.radius,
        ];
        if values.iter().any(|value| !value.is_finite()) || sphere.radius <= 0. {
            return Err(CueMolError::invalid_argument(
                "SES atom coordinates and radii must be finite with positive radii",
            ));
        }
        xyzr.extend(values);
    }
    let backend = if backend == SesBackend::Auto {
        std::env::var("CUEMOL_SES_BACKEND")
            .ok()
            .and_then(|s| SesBackend::parse(&s).ok())
            .filter(|b| *b != SesBackend::Auto)
            .unwrap_or(SesBackend::MeshMs)
    } else {
        backend
    };
    if backend == SesBackend::Rust || probe == 0. {
        let mut mesh = SurfaceMesh::new();
        grid_surface::build_ses_surface(
            &mut mesh,
            spheres,
            probe,
            (1.0 / density).clamp(0.25, 1.5),
        );
        return Ok((mesh, SesBackend::Rust));
    }
    let (coordinates, triangles, used, _) = kernel(
        &xyzr,
        density,
        probe,
        if backend == SesBackend::Ball { 2 } else { 1 },
    )?;
    let vertices: Vec<_> = coordinates
        .chunks_exact(6)
        .map(|v| {
            SurfaceVertex::new(
                Vector4::new3(v[0], v[1], v[2]),
                Vector4::new3(v[3], v[4], v[5]),
            )
        })
        .collect();
    if vertices.is_empty()
        || triangles.is_empty()
        || coordinates.len() % 6 != 0
        || triangles.len() % 3 != 0
        || coordinates.iter().any(|v| !v.is_finite())
        || triangles.iter().any(|&i| i as usize >= vertices.len())
    {
        return Err(CueMolError::invalid_argument(
            "invalid SES mesh returned by the calculation kernel",
        ));
    }
    let triangles = triangles
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    Ok((
        SurfaceMesh {
            vertices,
            triangles,
        },
        if used == 2 {
            SesBackend::Ball
        } else {
            SesBackend::MeshMs
        },
    ))
}

pub(crate) fn direct_surface_kernel(
    spheres: &[(SurfSphere, i32)],
    density: f64,
    probe: f64,
    backend: u32,
) -> Result<SurfaceMesh> {
    let xyzr = spheres
        .iter()
        .flat_map(|(s, _)| [s.center.x, s.center.y, s.center.z, s.radius])
        .collect::<Vec<_>>();
    let (coords, faces, _, owners) = kernel(&xyzr, density, probe, backend)?;
    if coords.len() % 6 != 0
        || faces.len() % 3 != 0
        || coords.iter().any(|v| !v.is_finite())
        || faces.iter().any(|&i| i as usize >= coords.len() / 6)
    {
        return Err(CueMolError::invalid_argument("invalid direct surface mesh"));
    }
    let vertices = coords
        .chunks_exact(6)
        .enumerate()
        .map(|(i, v)| {
            SurfaceVertex::with_info(
                Vector4::new3(v[0] as f32 as f64, v[1] as f32 as f64, v[2] as f32 as f64),
                Vector4::new3(v[3] as f32 as f64, v[4] as f32 as f64, v[5] as f32 as f64),
                owners
                    .get(i)
                    .and_then(|&index| spheres.get(index as usize))
                    .map_or(u32::MAX, |(_, id)| *id as u32),
            )
        })
        .collect();
    Ok(SurfaceMesh {
        vertices,
        triangles: faces.chunks_exact(3).map(|f| [f[0], f[1], f[2]]).collect(),
    })
}

/// Kernel mesh: interleaved position/normal vertices, triangles, backend id, owner atom indices.
pub type KernelMesh = (Vec<f64>, Vec<u32>, u32, Vec<u32>);

/// Builds a packed MeshMS (1, 6), BALL (2), or EDTSurf (3–5) mesh.
pub fn kernel(xyzr: &[f64], density: f64, probe: f64, backend: u32) -> Result<KernelMesh> {
    if xyzr.is_empty()
        || !xyzr.len().is_multiple_of(4)
        || !density.is_finite()
        || density <= 0.0
        || !probe.is_finite()
        || probe < 0.0
        || xyzr
            .chunks_exact(4)
            .any(|a| a.iter().any(|v| !v.is_finite()) || a[3] <= 0.0)
    {
        return Err(CueMolError::invalid_argument(
            "invalid surface coordinates, radii, probe, or density",
        ));
    }
    let (vertices, triangles, used, mut owners) = match backend {
        3..=5 => crate::edtsurf::edtsurf_mesh(xyzr, density, probe, backend - 3)?,
        6 => crate::meshms::surface_kernel(xyzr, density, probe)?,
        2 => ball_kernel(xyzr, density, probe)?,
        _ => match crate::meshms::surface_kernel(xyzr, 1.18 / density.sqrt(), probe) {
            Ok(mesh) => mesh,
            Err(_) => ball_kernel(xyzr, density, probe)?,
        },
    };
    if vertices.iter().any(|v| !v.is_finite()) {
        return Err(CueMolError::invalid_argument(
            "Surface contains a non-finite vertex",
        ));
    }
    owners.resize(vertices.len() / 6, u32::MAX);
    Ok((vertices, triangles, used, owners))
}

fn ball_kernel(xyzr: &[f64], density: f64, probe: f64) -> Result<KernelMesh> {
    let (vertices, faces) =
        crate::ball::ball_surface(xyzr, density, probe).map_err(CueMolError::invalid_argument)?;
    Ok((vertices, faces, 2, Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_mesh_subdivision_returns_an_error() {
        let atoms = [0.0, 0.0, 0.0, 1.7, 2.0, 0.0, 0.0, 1.5];
        assert!(kernel(&atoms, 1e-12, 1.4, 6).is_err());
        assert!(kernel(&atoms, 1e24, 1.4, 2).is_err());
    }

    #[test]
    fn analytic_backends_reproduce_a_sphere_and_validate_inputs() {
        let radius = 1.7;
        let spheres = [SurfSphere {
            center: Vector4::new3(1., 2., 3.),
            radius,
        }];
        for backend in [SesBackend::MeshMs, SesBackend::Ball] {
            let (mesh, used) = build_surface(&spheres, 8., 1.4, backend).unwrap();
            assert_eq!(used, backend);
            let mut area = 0.;
            for vertex in mesh.vertices() {
                assert!(((vertex.position - spheres[0].center).length() - radius).abs() < 1e-5);
                assert!((vertex.normal.length() - 1.).abs() < 1e-5);
            }
            for face in mesh.triangles() {
                let a = mesh.vertices()[face[0] as usize].position;
                let b = mesh.vertices()[face[1] as usize].position;
                let c = mesh.vertices()[face[2] as usize].position;
                area += (b - a).cross(c - a).length() * 0.5;
            }
            assert!(
                (area / (4. * std::f64::consts::PI * radius * radius) - 1.).abs() < 0.06,
                "{backend:?}: {area}"
            );
        }
        assert!(build_surface(&[], 2., 1.4, SesBackend::Auto).is_err());
        assert!(build_surface(&spheres, f64::NAN, 1.4, SesBackend::Auto).is_err());
        assert!(SesBackend::parse("invalid").is_err());
    }
}
