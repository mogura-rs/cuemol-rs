//! Pure-Rust port of the BALL solvent-excluded surface (reduced surface, SES and its
//! triangulation) vendored by CueMol2; derived from BALL, licensed under LGPL-2.1.

mod geom;
mod rs;
mod ses;
mod tses;
mod util;

pub(crate) type BallResult<T> = std::result::Result<T, String>;

/// Restores BALL's default `Constants::EPSILON` around one computation.
struct EpsilonScope(f64);

impl EpsilonScope {
    fn enter() -> Self {
        Self(geom::set_eps(1e-6))
    }
}

impl Drop for EpsilonScope {
    fn drop(&mut self) {
        geom::set_eps(self.0);
    }
}

/// `ball_surface` of CueMol's surface bridge: interleaved position/normal vertices and
/// triangle vertex indices. Retries with slightly changed probe radii like the original.
pub(crate) fn ball_surface(
    xyzr: &[f64],
    density: f64,
    probe: f64,
) -> BallResult<(Vec<f64>, Vec<u32>)> {
    let _scope = EpsilonScope::enter();
    let spheres: Vec<geom::Sphere> = xyzr
        .chunks_exact(4)
        .map(|a| geom::Sphere::new(geom::Vec3::new(a[0], a[1], a[2]), a[3]))
        .collect();
    if spheres.is_empty() {
        return Err("BALL needs at least one atom".to_string());
    }
    let step = if probe < 1.5 { 0.01 } else { -0.01 };
    for retry in 0..10u32 {
        geom::set_eps(1e-6);
        let mut reduced = rs::ReducedSurface::new(spheres.clone(), probe + retry as f64 * step);
        reduced.compute()?;
        let mut excluded = ses::Ses::new();
        ses::SesComputer::new(&mut reduced, &mut excluded).run()?;
        if !excluded.check() {
            continue;
        }
        let (vertices, triangles) = tses::triangulate(&mut reduced, &mut excluded, density)?;
        if vertices.is_empty() || triangles.is_empty() {
            return Err("BALL returned an empty surface".to_string());
        }
        return Ok((vertices, triangles));
    }
    Err("BALL could not construct a valid solvent-excluded surface".to_string())
}
