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
    let (coordinates, triangles, used) = kernel(
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

#[cfg(not(target_arch = "wasm32"))]
fn kernel(
    xyzr: &[f64],
    density: f64,
    probe: f64,
    backend: u32,
) -> Result<(Vec<f64>, Vec<u32>, u32)> {
    use std::ffi::{c_char, c_void, CStr};
    unsafe extern "C" {
        fn cuemol_surface_build(
            xyzr: *const f64,
            count: u32,
            density: f64,
            probe: f64,
            backend: u32,
            error: *mut c_char,
            error_length: u32,
        ) -> *mut c_void;
        fn cuemol_surface_destroy(surface: *mut c_void);
        fn cuemol_surface_vertex_count(surface: *const c_void) -> u32;
        fn cuemol_surface_triangle_count(surface: *const c_void) -> u32;
        fn cuemol_surface_backend(surface: *const c_void) -> u32;
        fn cuemol_surface_vertices(surface: *const c_void) -> *const f64;
        fn cuemol_surface_triangles(surface: *const c_void) -> *const u32;
    }
    struct Handle(*mut c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                cuemol_surface_destroy(self.0);
            }
        }
    }
    let count = u32::try_from(xyzr.len() / 4)
        .map_err(|_| CueMolError::invalid_argument("too many SES atoms"))?;
    let mut error = [0; 512];
    let raw = unsafe {
        cuemol_surface_build(
            xyzr.as_ptr(),
            count,
            density,
            probe,
            backend,
            error.as_mut_ptr(),
            error.len() as u32,
        )
    };
    if raw.is_null() {
        return Err(CueMolError::invalid_argument(
            unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy(),
        ));
    }
    let handle = Handle(raw);
    unsafe {
        let vertices = cuemol_surface_vertex_count(handle.0) as usize;
        let triangles = cuemol_surface_triangle_count(handle.0) as usize;
        Ok((
            std::slice::from_raw_parts(cuemol_surface_vertices(handle.0), vertices * 6).to_vec(),
            std::slice::from_raw_parts(cuemol_surface_triangles(handle.0), triangles * 3).to_vec(),
            cuemol_surface_backend(handle.0),
        ))
    }
}

#[cfg(target_arch = "wasm32")]
fn kernel(
    xyzr: &[f64],
    density: f64,
    probe: f64,
    backend: u32,
) -> Result<(Vec<f64>, Vec<u32>, u32)> {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = cuemolKernels, js_name = surface, catch)]
        fn surface(
            xyzr: &[f64],
            density: f64,
            probe: f64,
            backend: u32,
        ) -> std::result::Result<js_sys::Array, JsValue>;
    }
    let result = surface(xyzr, density, probe, backend)
        .map_err(|e| CueMolError::invalid_argument(format!("SES generation failed: {e:?}")))?;
    let vertices = result
        .get(0)
        .dyn_into::<js_sys::Float64Array>()
        .map_err(|_| CueMolError::invalid_argument("invalid SES vertices"))?
        .to_vec();
    let triangles = result
        .get(1)
        .dyn_into::<js_sys::Uint32Array>()
        .map_err(|_| CueMolError::invalid_argument("invalid SES triangles"))?
        .to_vec();
    Ok((
        vertices,
        triangles,
        result.get(2).as_f64().unwrap_or(0.) as u32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
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
