//! Analytic solvent-excluded surface (SES) primitives — port of C++
//! `surface/SurfParams` (`TorusParam`, `ConcSphParam`), the geometric foundation
//! of the reduced-surface (Connolly) SES that `RSCompBuilder` + `SESTgBuilder`
//! build on.
//!
//! These are the probe-placement primitives: the torus a probe sweeps between two
//! atoms, and the two reentrant-probe positions where one probe touches three
//! atoms. They are pure analytic geometry (no graph traversal) and each is
//! verified by its defining oracle — the probe surface touches every atom it rolls
//! against (distance `r_atom + r_probe`).
//!
//! The grid SES ([`crate::grid_surface::build_ses_surface`]) is the production
//! path; this analytic port provides the convex, saddle-torus, and concave patch
//! geometry used by [`build_analytic_ses`].

use cuemol_core::Vector4;

use crate::{SurfaceMesh, SurfaceVertex};

const EPS: f64 = 1.0e-4;

/// The torus swept by a probe of radius `r_p` rolling between two atoms — port of
/// C++ `TorusParam::calc`. The torus ring lies in the plane through `center`
/// perpendicular to `axis`, with radius `ring_radius`; a probe centred anywhere on
/// that ring touches both atom surfaces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TorusParam {
    /// Tij — torus centre, on the inter-atom axis.
    pub center: Vector4,
    /// Uij — unit torus axis, pointing a_i → a_j.
    pub axis: Vector4,
    /// rij — torus ring radius (locus of the probe centre).
    pub ring_radius: f64,
    /// |a_j − a_i| — inter-atom distance.
    pub dij: f64,
    /// rc_i / rc_j — contact-circle radii on each atom.
    pub contact_i: f64,
    pub contact_j: f64,
}

impl TorusParam {
    /// Compute the torus for atoms `(a_i, r_i)` and `(a_j, r_j)` with probe radius
    /// `r_p`. `None` when the atoms are coincident, too far apart to share a probe,
    /// or one engulfs the other.
    pub fn calc(a_i: Vector4, r_i: f64, a_j: Vector4, r_j: f64, r_p: f64) -> Option<TorusParam> {
        let vij = a_j - a_i;
        let dijsq = vij.sqlen3d();
        let dij = dijsq.sqrt();
        if dij < EPS {
            return None;
        }
        let axis = vij * (1.0 / dij);
        let rip = r_i + r_p;
        let rjp = r_j + r_p;
        let rat = (rip * rip - rjp * rjp) / dijsq;
        let center = (a_i + a_j + vij * rat) * 0.5;

        let rirj2rp = r_i + r_j + 2.0 * r_p;
        let rirj = r_i - r_j;
        let term1 = rirj2rp * rirj2rp - dijsq;
        if term1 < 0.0 {
            return None; // atoms too far apart to share a rolling probe
        }
        let term2 = dijsq - rirj * rirj;
        if term2 < 0.0 {
            return None; // one atom engulfs the other
        }
        let ring_radius = 0.5 * (term1 * term2).sqrt() / dij;
        Some(TorusParam {
            center,
            axis,
            ring_radius,
            dij,
            contact_i: ring_radius * r_i / (r_i + r_p),
            contact_j: ring_radius * r_j / (r_j + r_p),
        })
    }

    /// A probe-centre point on the torus ring, in the direction of `perp`
    /// (projected perpendicular to the axis).
    pub fn ring_point(&self, perp: Vector4) -> Vector4 {
        let d = perp - self.axis * perp.dot3d(self.axis);
        self.center + d.normalize3d().unwrap_or(d) * self.ring_radius
    }
}

/// The two reentrant-probe positions where a single probe touches three atoms —
/// port of C++ `ConcSphParam::calc`. The probe centres `probe_plus`/`probe_minus`
/// sit above/below the three-atom plane; each is at distance `r_atom + r_p` from
/// all three atoms (the defining property of a concave reentrant patch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConcSphParam {
    /// Pijk_p / Pijk_m — the two probe centres.
    pub probe_plus: Vector4,
    pub probe_minus: Vector4,
    /// Uijk — unit normal of the three-atom plane.
    pub normal: Vector4,
    /// Bijk — base point (foot of the probe centres on the plane).
    pub base: Vector4,
    /// h_ijk — distance of each probe centre from the plane.
    pub height: f64,
}

impl ConcSphParam {
    /// Compute the concave-probe positions for three atoms with probe radius `r_p`.
    /// `None` when the atoms are collinear (no unique probe) or no probe can touch
    /// all three.
    #[allow(clippy::too_many_arguments)]
    pub fn calc(
        a_i: Vector4,
        r_i: f64,
        a_j: Vector4,
        r_j: f64,
        a_k: Vector4,
        r_k: f64,
        r_p: f64,
    ) -> Option<ConcSphParam> {
        let t_ij = TorusParam::calc(a_i, r_i, a_j, r_j, r_p)?;
        let t_ik = TorusParam::calc(a_i, r_i, a_k, r_k, r_p)?;
        let (uij, uik) = (t_ij.axis, t_ik.axis);
        let (tij, tik) = (t_ij.center, t_ik.center);

        let xu = uij.cross(uik);
        let sin_omg = xu.length3d();
        if sin_omg <= EPS {
            return None; // collinear atoms
        }
        let uijk = xu * (1.0 / sin_omg);
        let utb = uijk.cross(uij);
        let base = tij + utb * (uik.dot3d(tik - tij) / sin_omg);

        let rip = r_i + r_p;
        let hijksq = rip * rip - (base - a_i).sqlen3d();
        if hijksq <= 0.0 {
            return None;
        }
        let height = hijksq.sqrt();
        Some(ConcSphParam {
            probe_plus: base + uijk * height,
            probe_minus: base - uijk * height,
            normal: uijk,
            base,
            height,
        })
    }
}

/// Is a probe centred at `probe_pos` accessible — i.e. on the SES surface, with
/// its sphere not overlapping any atom? Port of `RSCompBuilder::collCheckProbe`:
/// an atom collides when its surface plus the probe radius reaches past the probe
/// centre (`r_atom + r_p > dist`). The atoms that FORM the probe sit exactly at
/// `r_atom + r_p` (just touching), so a small tolerance keeps them from
/// self-rejecting. `atoms` is `(position, radius)`.
pub fn probe_is_accessible(probe_pos: Vector4, atoms: &[(Vector4, f64)], r_p: f64) -> bool {
    for &(a, r) in atoms {
        let dist = (a - probe_pos).length3d();
        if r + r_p - dist > EPS {
            return false; // probe sphere overlaps this atom -> buried
        }
    }
    true
}

/// All concave-probe positions that actually lie on the SES surface, for a set of
/// `(position, radius)` atoms and probe radius `r_p`. For every atom triple, the
/// two `ConcSphParam` probe centres are kept only if accessible (no other atom
/// buries them) — the direct (non-recursive) equivalent of the reduced-surface
/// graph's set of concave faces.
pub fn accessible_concave_probes(atoms: &[(Vector4, f64)], r_p: f64) -> Vec<Vector4> {
    let n = atoms.len();
    let mut probes = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            for k in (j + 1)..n {
                let Some(c) = ConcSphParam::calc(
                    atoms[i].0, atoms[i].1, atoms[j].0, atoms[j].1, atoms[k].0, atoms[k].1, r_p,
                ) else {
                    continue;
                };
                for probe in [c.probe_plus, c.probe_minus] {
                    if probe_is_accessible(probe, atoms, r_p) {
                        probes.push(probe);
                    }
                }
            }
        }
    }
    probes
}

/// Triangulate the concave reentrant spherical patch at `probe` (radius `r_p`)
/// bounded by the three contact points toward `a_i`/`a_j`/`a_k`, appending it to
/// `mesh`. The patch is the spherical triangle on the probe sphere facing the
/// three atoms; its outward (solvent-facing) normals point TOWARD the probe centre
/// (the patch is concave). Port of the `TgConcSphere` patch, tessellated directly.
pub fn add_concave_patch(
    mesh: &mut SurfaceMesh,
    probe: Vector4,
    r_p: f64,
    a_i: Vector4,
    a_j: Vector4,
    a_k: Vector4,
    ndiv: usize,
) {
    let ndiv = ndiv.max(1);
    let dir = |a: Vector4| {
        (a - probe)
            .normalize3d()
            .unwrap_or(Vector4::new3(0.0, 0.0, 1.0))
    };
    let (d_i, d_j, d_k) = (dir(a_i), dir(a_j), dir(a_k));

    // Triangular grid of barycentric points (a·d_i + b·d_j + c·d_k, a+b+c=ndiv),
    // each projected onto the probe sphere.
    let mut rows: Vec<Vec<u32>> = Vec::with_capacity(ndiv + 1);
    for i in 0..=ndiv {
        let mut row = Vec::with_capacity(ndiv - i + 1);
        for j in 0..=(ndiv - i) {
            let (a, b, c) = ((ndiv - i - j) as f64, i as f64, j as f64);
            let w = d_i * a + d_j * b + d_k * c;
            let normal_in = w.normalize3d().unwrap_or(d_i); // points outward from probe
            let position = probe + normal_in * r_p;
            row.push(mesh.vertex_count() as u32);
            mesh.push_vertex(SurfaceVertex {
                position,
                // Concave patch: solvent-facing normal points back toward the probe.
                normal: normal_in * -1.0,
                info: 0,
            });
        }
        rows.push(row);
    }

    for i in 0..ndiv {
        for j in 0..(ndiv - i) {
            let a = rows[i][j];
            let b = rows[i][j + 1];
            let c = rows[i + 1][j];
            mesh.push_triangle([a, b, c]);
            if j + 1 < rows[i + 1].len() {
                let d = rows[i + 1][j + 1];
                mesh.push_triangle([b, d, c]);
            }
        }
    }
}

/// Accessible concave-probe positions together with the indices of the three
/// atoms that form each — the input the concave-patch triangulation needs.
pub fn accessible_concave_probes_with_atoms(
    atoms: &[(Vector4, f64)],
    r_p: f64,
) -> Vec<(Vector4, [usize; 3])> {
    let n = atoms.len();
    let mut out = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            for k in (j + 1)..n {
                let Some(c) = ConcSphParam::calc(
                    atoms[i].0, atoms[i].1, atoms[j].0, atoms[j].1, atoms[k].0, atoms[k].1, r_p,
                ) else {
                    continue;
                };
                for probe in [c.probe_plus, c.probe_minus] {
                    if probe_is_accessible(probe, atoms, r_p) {
                        out.push((probe, [i, j, k]));
                    }
                }
            }
        }
    }
    out
}

/// Build the concave (reentrant) component of the analytic SES: a [`SurfaceMesh`]
/// of the spherical-triangle patches at every accessible concave-probe position.
pub fn build_concave_patches(atoms: &[(Vector4, f64)], r_p: f64, ndiv: usize) -> SurfaceMesh {
    let mut mesh = SurfaceMesh::new();
    for (probe, [i, j, k]) in accessible_concave_probes_with_atoms(atoms, r_p) {
        add_concave_patch(
            &mut mesh, probe, r_p, atoms[i].0, atoms[j].0, atoms[k].0, ndiv,
        );
    }
    mesh
}

/// Triangulate the convex (exposed van der Waals) part of atom `idx`'s sphere,
/// appending it to `mesh`. A surface point is exposed when a probe touching the
/// atom there does not bury into any other atom (reuses [`probe_is_accessible`]);
/// outward normals point away from the atom centre. This is the convex component
/// of the analytic SES (the part the rolling probe cannot reach behind).
pub fn add_convex_cap(
    mesh: &mut SurfaceMesh,
    atoms: &[(Vector4, f64)],
    idx: usize,
    r_p: f64,
    stacks: usize,
    slices: usize,
) {
    let (center, r) = atoms[idx];
    let stacks = stacks.max(2);
    let slices = slices.max(3);
    let exposed = |dir: Vector4| probe_is_accessible(center + dir * (r + r_p), atoms, r_p);

    // UV-sphere grid; assign a mesh index only to exposed vertices.
    let mut grid: Vec<Vec<Option<u32>>> = Vec::with_capacity(stacks + 1);
    for i in 0..=stacks {
        let theta = std::f64::consts::PI * i as f64 / stacks as f64;
        let (st, ct) = (theta.sin(), theta.cos());
        let mut row = Vec::with_capacity(slices + 1);
        for j in 0..=slices {
            let phi = std::f64::consts::TAU * j as f64 / slices as f64;
            let dir = Vector4::new3(st * phi.cos(), st * phi.sin(), ct);
            if exposed(dir) {
                row.push(Some(mesh.vertex_count() as u32));
                mesh.push_vertex(SurfaceVertex {
                    position: center + dir * r,
                    normal: dir, // convex: outward normal points away from the atom
                    info: idx as u32,
                });
            } else {
                row.push(None);
            }
        }
        grid.push(row);
    }

    for i in 0..stacks {
        for j in 0..slices {
            // A quad's two triangles are emitted only where all corners are exposed.
            let (a, b, c, d) = (
                grid[i][j],
                grid[i][j + 1],
                grid[i + 1][j],
                grid[i + 1][j + 1],
            );
            if let (Some(a), Some(b), Some(c)) = (a, b, c) {
                mesh.push_triangle([a, c, b]);
            }
            if let (Some(b), Some(c), Some(d)) = (b, c, d) {
                mesh.push_triangle([b, c, d]);
            }
        }
    }
}

/// Build the convex component of the analytic SES — every atom's exposed van der
/// Waals cap.
pub fn build_convex_caps(
    atoms: &[(Vector4, f64)],
    r_p: f64,
    stacks: usize,
    slices: usize,
) -> SurfaceMesh {
    let mut mesh = SurfaceMesh::new();
    for idx in 0..atoms.len() {
        add_convex_cap(&mut mesh, atoms, idx, r_p, stacks, slices);
    }
    mesh
}

/// Spherical linear interpolation between two unit vectors.
fn slerp(a: Vector4, b: Vector4, t: f64) -> Vector4 {
    let dot = a.dot3d(b).clamp(-1.0, 1.0);
    let omega = dot.acos();
    if omega < EPS {
        return a;
    }
    let s = omega.sin();
    a * (((1.0 - t) * omega).sin() / s) + b * ((t * omega).sin() / s)
}

/// Triangulate the saddle (toroidal reentrant) patch swept by the probe rolling
/// between atoms `i` and `j`, appending it to `mesh`. Port of the `TgTorus` patch,
/// tessellated directly: for each accessible ring angle the probe centre `C(θ)`
/// sits on the torus ring, and the reentrant surface is the probe-sphere arc
/// (slerp) between the two atom contacts — a concave band whose normals point back
/// toward the rolling probe. Ring angles where the probe would bury into another
/// atom are skipped (the band breaks into arcs, the concave patches fill the gaps).
pub fn add_saddle_torus(
    mesh: &mut SurfaceMesh,
    atoms: &[(Vector4, f64)],
    i: usize,
    j: usize,
    r_p: f64,
    n_theta: usize,
    n_t: usize,
) {
    let (a_i, r_i) = atoms[i];
    let (a_j, r_j) = atoms[j];
    let Some(tp) = TorusParam::calc(a_i, r_i, a_j, r_j, r_p) else {
        return;
    };
    // Frame perpendicular to the torus axis.
    let axis = tp.axis;
    let seed = if axis.x.abs() < 0.9 {
        Vector4::new3(1.0, 0.0, 0.0)
    } else {
        Vector4::new3(0.0, 1.0, 0.0)
    };
    let e1 = axis
        .cross(seed)
        .normalize3d()
        .unwrap_or(Vector4::new3(0.0, 1.0, 0.0));
    let e2 = axis
        .cross(e1)
        .normalize3d()
        .unwrap_or(Vector4::new3(0.0, 0.0, 1.0));

    let n_theta = n_theta.max(3);
    let n_t = n_t.max(1);
    let mut prev: Option<Vec<u32>> = None;
    let mut first: Option<Vec<u32>> = None;
    for s in 0..=n_theta {
        let theta = std::f64::consts::TAU * s as f64 / n_theta as f64;
        let c = tp.center + (e1 * theta.cos() + e2 * theta.sin()) * tp.ring_radius;
        // Only where the rolling probe is on the surface (not buried in a 3rd atom).
        if !probe_is_accessible(c, atoms, r_p) {
            prev = None;
            continue;
        }
        let u_i = (a_i - c).normalize3d().unwrap_or(e1);
        let u_j = (a_j - c).normalize3d().unwrap_or(e2);
        let mut col = Vec::with_capacity(n_t + 1);
        for q in 0..=n_t {
            let t = q as f64 / n_t as f64;
            let dir = slerp(u_i, u_j, t);
            col.push(mesh.vertex_count() as u32);
            mesh.push_vertex(SurfaceVertex {
                position: c + dir * r_p,
                normal: dir * -1.0, // reentrant: normal points back toward the probe
                info: 0,
            });
        }
        if let Some(prev) = &prev {
            for q in 0..n_t {
                mesh.push_triangle([prev[q], prev[q + 1], col[q]]);
                mesh.push_triangle([col[q], prev[q + 1], col[q + 1]]);
            }
        }
        if first.is_none() {
            first = Some(col.clone());
        }
        prev = Some(col);
    }
    // (The wrap-around seam at θ=2π reuses the s==n_theta column, so no explicit
    // stitch to `first` is needed.)
    let _ = first;
}

/// Build the complete analytic SES mesh: convex van-der-Waals caps + saddle tori
/// (between every atom pair) + concave reentrant patches, in one [`SurfaceMesh`]
/// (the same type the grid SES produces, OBJ-exportable). The analytic counterpart
/// of [`crate::grid_surface::build_ses_surface`].
pub fn build_analytic_ses(atoms: &[(Vector4, f64)], r_p: f64, detail: usize) -> SurfaceMesh {
    let detail = detail.max(2);
    let mut mesh = SurfaceMesh::new();
    for idx in 0..atoms.len() {
        add_convex_cap(&mut mesh, atoms, idx, r_p, detail * 2, detail * 2);
    }
    for i in 0..atoms.len() {
        for j in (i + 1)..atoms.len() {
            add_saddle_torus(&mut mesh, atoms, i, j, r_p, detail * 3, detail);
        }
    }
    for (probe, [i, j, k]) in accessible_concave_probes_with_atoms(atoms, r_p) {
        add_concave_patch(
            &mut mesh, probe, r_p, atoms[i].0, atoms[j].0, atoms[k].0, detail,
        );
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64, z: f64) -> Vector4 {
        Vector4::new3(x, y, z)
    }

    #[test]
    fn torus_ring_probe_touches_both_atoms() {
        let (r_i, r_j, r_p) = (1.5, 1.5, 1.4);
        let a_i = v(0.0, 0.0, 0.0);
        let a_j = v(3.0, 0.0, 0.0);
        let t = TorusParam::calc(a_i, r_i, a_j, r_j, r_p).unwrap();

        // Symmetric atoms -> torus centre at the midpoint, axis along the bond.
        assert!((t.center - v(1.5, 0.0, 0.0)).length3d() < 1e-9);
        assert!((t.axis - v(1.0, 0.0, 0.0)).length3d() < 1e-9);

        // The DEFINING oracle: a probe centred on the ring touches both atom
        // surfaces (distance = r_atom + r_probe), for several ring directions.
        for perp in [v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0), v(0.0, 1.0, 1.0)] {
            let p = t.ring_point(perp);
            assert!(((p - a_i).length3d() - (r_i + r_p)).abs() < 1e-9, "touch i");
            assert!(((p - a_j).length3d() - (r_j + r_p)).abs() < 1e-9, "touch j");
        }

        // Asymmetric radii: the centre shifts toward the larger atom's side.
        let t2 = TorusParam::calc(a_i, 2.0, a_j, 1.0, r_p).unwrap();
        let p = t2.ring_point(v(0.0, 1.0, 0.0));
        assert!(((p - a_i).length3d() - (2.0 + r_p)).abs() < 1e-9);
        assert!(((p - a_j).length3d() - (1.0 + r_p)).abs() < 1e-9);
    }

    #[test]
    fn torus_rejects_degenerate_atom_pairs() {
        // Too far apart to share a probe.
        assert!(TorusParam::calc(v(0.0, 0.0, 0.0), 1.5, v(100.0, 0.0, 0.0), 1.5, 1.4).is_none());
        // Coincident atoms.
        assert!(TorusParam::calc(v(0.0, 0.0, 0.0), 1.5, v(0.0, 0.0, 0.0), 1.5, 1.4).is_none());
        // One atom engulfs the other (far larger, very close).
        assert!(TorusParam::calc(v(0.0, 0.0, 0.0), 5.0, v(0.5, 0.0, 0.0), 0.5, 1.4).is_none());
    }

    #[test]
    fn concave_probe_touches_all_three_atoms() {
        let r_p = 1.4;
        // Three atoms forming a triangle in the z=0 plane.
        let a_i = v(0.0, 0.0, 0.0);
        let a_j = v(2.8, 0.0, 0.0);
        let a_k = v(1.4, 2.4, 0.0);
        let (r_i, r_j, r_k) = (1.6, 1.5, 1.7);
        let c = ConcSphParam::calc(a_i, r_i, a_j, r_j, a_k, r_k, r_p).unwrap();

        // Both probe positions touch all three atoms (distance = r_atom + r_p).
        for probe in [c.probe_plus, c.probe_minus] {
            assert!(
                ((probe - a_i).length3d() - (r_i + r_p)).abs() < 1e-7,
                "touch i"
            );
            assert!(
                ((probe - a_j).length3d() - (r_j + r_p)).abs() < 1e-7,
                "touch j"
            );
            assert!(
                ((probe - a_k).length3d() - (r_k + r_p)).abs() < 1e-7,
                "touch k"
            );
        }
        // The two positions are mirror images across the atom plane (z = 0 here).
        assert!((c.probe_plus.z + c.probe_minus.z).abs() < 1e-9);
        assert!(c.probe_plus.z.abs() > 1e-3, "probe is off the plane");

        // Collinear atoms -> no unique probe.
        assert!(ConcSphParam::calc(a_i, r_i, a_j, r_j, v(1.4, 0.0, 0.0), r_k, r_p).is_none());
    }

    #[test]
    fn accessible_probes_filter_buried_positions() {
        let r_p = 1.4;
        let atoms = vec![
            (v(0.0, 0.0, 0.0), 1.6),
            (v(2.8, 0.0, 0.0), 1.5),
            (v(1.4, 2.4, 0.0), 1.7),
        ];
        // Three atoms -> both reentrant probe positions (above/below) accessible.
        assert_eq!(accessible_concave_probes(&atoms, r_p).len(), 2);

        let c = ConcSphParam::calc(
            atoms[0].0, atoms[0].1, atoms[1].0, atoms[1].1, atoms[2].0, atoms[2].1, r_p,
        )
        .unwrap();
        // A probe touching its three atoms is accessible; an atom sitting on it
        // buries it (overlaps the probe sphere).
        assert!(probe_is_accessible(c.probe_plus, &atoms, r_p));
        assert!(!probe_is_accessible(
            c.probe_plus,
            &[(c.probe_plus, 0.5)],
            r_p
        ));

        // Burying the +z probe with a 4th atom removes it from the surface set.
        let mut buried = atoms.clone();
        buried.push((c.probe_plus, 0.5));
        let probes = accessible_concave_probes(&buried, r_p);
        assert!(
            probes.iter().all(|p| (*p - c.probe_plus).length3d() > 1e-3),
            "the buried probe is no longer on the surface"
        );
    }

    #[test]
    fn concave_patch_lies_on_the_probe_sphere_with_inward_normals() {
        let r_p = 1.4;
        let atoms = vec![
            (v(0.0, 0.0, 0.0), 1.6),
            (v(2.8, 0.0, 0.0), 1.5),
            (v(1.4, 2.4, 0.0), 1.7),
        ];
        let probes = accessible_concave_probes_with_atoms(&atoms, r_p);
        assert_eq!(probes.len(), 2);
        let (probe, [i, j, k]) = probes[0];

        let mut mesh = SurfaceMesh::new();
        add_concave_patch(&mut mesh, probe, r_p, atoms[i].0, atoms[j].0, atoms[k].0, 4);
        assert!(mesh.triangle_count() > 0);
        for vtx in mesh.vertices() {
            // Every patch vertex lies exactly on the probe sphere.
            assert!(
                ((vtx.position - probe).length3d() - r_p).abs() < 1e-9,
                "vertex on probe sphere"
            );
            // The patch is concave: its outward normal points back toward the probe.
            let to_probe = (probe - vtx.position).normalize3d().unwrap();
            assert!(
                vtx.normal.dot3d(to_probe) > 0.999,
                "inward (concave) normal"
            );
        }
        // The whole-molecule build covers both probe patches.
        let full = build_concave_patches(&atoms, r_p, 4);
        assert_eq!(full.triangle_count(), 2 * mesh.triangle_count());
    }

    #[test]
    fn convex_cap_is_full_sphere_isolated_and_clipped_when_crowded() {
        let r_p = 1.4;
        // An isolated atom's convex cap is its whole van der Waals sphere.
        let solo = [(v(0.0, 0.0, 0.0), 1.6)];
        let mut mesh = SurfaceMesh::new();
        add_convex_cap(&mut mesh, &solo, 0, r_p, 12, 16);
        for vtx in mesh.vertices() {
            assert!(
                (vtx.position.length3d() - 1.6).abs() < 1e-9,
                "on the vdW sphere"
            );
            let outward = vtx.position.normalize3d().unwrap();
            assert!(vtx.normal.dot3d(outward) > 0.999, "outward (convex) normal");
        }
        let full = mesh.triangle_count();
        assert!(full > 0);

        // Two overlapping atoms: each cap loses the region facing the other.
        let pair = [(v(0.0, 0.0, 0.0), 1.6), (v(2.0, 0.0, 0.0), 1.6)];
        let caps = build_convex_caps(&pair, r_p, 12, 16);
        assert!(caps.triangle_count() < 2 * full, "facing caps are clipped");
        assert!(
            caps.triangle_count() > full,
            "but most of each sphere survives"
        );
    }

    #[test]
    fn saddle_torus_vertices_sit_on_probes_touching_both_atoms() {
        let r_p = 1.4;
        let atoms = [(v(0.0, 0.0, 0.0), 1.6), (v(2.5, 0.0, 0.0), 1.5)];
        let mut mesh = SurfaceMesh::new();
        add_saddle_torus(&mut mesh, &atoms, 0, 1, r_p, 24, 4);
        assert!(mesh.triangle_count() > 0);
        for vtx in mesh.vertices() {
            // Reconstruct the rolling-probe centre (the normal points toward it):
            // C = P + r_p·normal. C must touch BOTH atoms (distance r_atom + r_p).
            let c = vtx.position + vtx.normal * r_p;
            assert!(
                ((c - atoms[0].0).length3d() - (atoms[0].1 + r_p)).abs() < 1e-6,
                "probe touches atom i"
            );
            assert!(
                ((c - atoms[1].0).length3d() - (atoms[1].1 + r_p)).abs() < 1e-6,
                "probe touches atom j"
            );
        }
        // The full analytic SES of two atoms = convex caps + the saddle torus
        // (no 3-atom concave patches), assembled into one mesh.
        let ses = build_analytic_ses(&atoms, r_p, 4);
        assert!(ses.triangle_count() > mesh.triangle_count());
    }
}
