use crate::grid_surface::build_ses_surface;
use crate::{SurfSphere, SurfaceMesh, SurfaceVertex};
use cuemol_core::{BSPTree, CueMolError, Result, Vector4};
use cuemol_molstr::{toppar, MolCoord, Selector};

const PATH_SLICE_COUNT: usize = 1_000;
const PATH_STEP: f64 = 0.25;
const MIN_HOLE_RADIUS: f64 = 1.0;
const MAX_HOLE_RADIUS: f64 = 5.0;
const MONTE_CARLO_STEPS: usize = 1_000;
const MAX_MONTE_CARLO_MOVE: f64 = 0.3;
const TEMPERATURE_SCALE: f64 = 0.9;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HolePathPoint {
    pub center: Vector4,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy)]
struct AtomSphere {
    center: Vector4,
    radius: f64,
}

struct HoleAtomIndex {
    atoms: Vec<AtomSphere>,
    tree: BSPTree<usize>,
}

impl HoleAtomIndex {
    fn new(molecule: &MolCoord, selector: &Selector) -> Result<Self> {
        let atoms: Vec<_> = molecule
            .atoms()
            .iter()
            .filter(|atom| selector.matches_in_molecule(atom, molecule))
            .filter(|atom| {
                atom.position.x.is_finite()
                    && atom.position.y.is_finite()
                    && atom.position.z.is_finite()
            })
            .map(|atom| AtomSphere {
                center: atom.position,
                radius: toppar::vdw_radius_for_atom(atom).max(0.1),
            })
            .collect();
        if atoms.is_empty() {
            return Err(CueMolError::invalid_argument(
                "hole search requires at least one selected atom",
            ));
        }

        let mut tree = BSPTree::new();
        for (index, atom) in atoms.iter().enumerate() {
            tree.push(atom.center, index);
        }
        tree.build();
        Ok(Self { atoms, tree })
    }

    fn clearance(&self, position: Vector4) -> f64 {
        let mut search_radius = 5.0;
        loop {
            let candidates = self.tree.values_around(position, search_radius);
            if !candidates.is_empty() {
                let nearest = candidates
                    .into_iter()
                    .min_by(|lhs, rhs| {
                        let lhs_distance = (self.atoms[*lhs].center - position).sqlen();
                        let rhs_distance = (self.atoms[*rhs].center - position).sqlen();
                        lhs_distance.total_cmp(&rhs_distance)
                    })
                    .expect("non-empty candidate list");
                let atom = self.atoms[nearest];
                return (atom.center - position).length3d() - atom.radius;
            }
            search_radius *= 2.0;
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_f64(&mut self) -> f64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let mantissa = self.state >> 11;
        mantissa as f64 / ((1_u64 << 53) as f64)
    }
}

fn normalized_direction(direction: Vector4) -> Result<Vector4> {
    if !direction.x.is_finite()
        || !direction.y.is_finite()
        || !direction.z.is_finite()
        || direction.length3d() <= f64::EPSILON
    {
        return Err(CueMolError::invalid_argument(
            "hole direction must be a finite non-zero vector",
        ));
    }
    direction.normalize3d()
}

fn perpendicular_basis(normal: Vector4) -> (Vector4, Vector4) {
    let reference = if normal.x.abs() <= normal.y.abs() && normal.x.abs() <= normal.z.abs() {
        Vector4::new3(1.0, 0.0, 0.0)
    } else if normal.y.abs() <= normal.z.abs() {
        Vector4::new3(0.0, 1.0, 0.0)
    } else {
        Vector4::new3(0.0, 0.0, 1.0)
    };
    let first = reference.cross(normal).normalize3d().unwrap_or(reference);
    let second = first.cross(normal).normalize3d().unwrap_or(reference);
    (first, second)
}

fn random_in_plane_direction(normal: Vector4, rng: &mut DeterministicRng) -> Vector4 {
    let random = Vector4::new3(rng.next_f64(), rng.next_f64(), rng.next_f64());
    let projected = random - normal * normal.dot3d(random);
    projected
        .normalize3d()
        .unwrap_or_else(|_| perpendicular_basis(normal).0)
}

fn optimize_slice(
    index: &HoleAtomIndex,
    direction: Vector4,
    start_temperature: f64,
    start_position: Vector4,
    rng: &mut DeterministicRng,
) -> HolePathPoint {
    let mut position = start_position;
    let mut radius = -1.0;
    let mut temperature = start_temperature;

    for step in 0..MONTE_CARLO_STEPS {
        let mut trial = position;
        if step > 0 {
            trial = trial
                + random_in_plane_direction(direction, rng)
                    * (rng.next_f64() * MAX_MONTE_CARLO_MOVE);
        }
        let trial_radius = index.clearance(trial);
        let accept = if trial_radius > radius {
            true
        } else {
            let probability = if temperature > 0.0 {
                ((trial_radius - radius) / temperature).exp()
            } else {
                0.0
            };
            rng.next_f64() < probability
        };
        if accept {
            position = trial;
            radius = trial_radius;
        }
        temperature *= TEMPERATURE_SCALE;
    }

    HolePathPoint {
        center: position,
        radius,
    }
}

fn find_path(
    index: &HoleAtomIndex,
    direction: Vector4,
    start_position: Vector4,
    start_temperature: f64,
    rng: &mut DeterministicRng,
) -> (Vec<HolePathPoint>, f64) {
    let mut position = start_position;
    let mut score = 0.0;
    let mut path = Vec::new();
    let advance = direction * PATH_STEP;

    for _ in 0..PATH_SLICE_COUNT {
        let point = optimize_slice(index, direction, start_temperature, position, rng);
        path.push(point);
        position = point.center + advance;
        score += point.radius * PATH_STEP;
        if !(MIN_HOLE_RADIUS..=MAX_HOLE_RADIUS).contains(&point.radius) {
            break;
        }
    }
    (path, score)
}

pub fn trace_hole_path(
    molecule: &MolCoord,
    selector: &Selector,
    direction: Vector4,
    start_position: Vector4,
) -> Result<Vec<HolePathPoint>> {
    let direction = normalized_direction(direction)?;
    if !start_position.x.is_finite()
        || !start_position.y.is_finite()
        || !start_position.z.is_finite()
    {
        return Err(CueMolError::invalid_argument(
            "hole start position must be finite",
        ));
    }
    let index = HoleAtomIndex::new(molecule, selector)?;
    let mut rng = DeterministicRng::new(0x4355_454d_4f4c_3252);
    let mut previous_score = -1.0;
    let mut best_path = Vec::new();
    for temperature in [0.0, 0.1, 0.2] {
        let (path, score) = find_path(&index, direction, start_position, temperature, &mut rng);
        if score < previous_score {
            break;
        }
        previous_score = score;
        best_path = path;
    }
    Ok(best_path)
}

pub fn build_hole_ses(
    mesh: &mut SurfaceMesh,
    path: &[HolePathPoint],
    density: f64,
    probe_radius: f64,
) -> Result<()> {
    if !density.is_finite() || density <= 0.0 {
        return Err(CueMolError::invalid_argument(
            "surface density must be positive",
        ));
    }
    if !probe_radius.is_finite() || probe_radius < 0.0 {
        return Err(CueMolError::invalid_argument(
            "surface probe radius must be non-negative",
        ));
    }
    let pore_count = path.len() / 2;
    let spheres: Vec<_> = (0..pore_count)
        .map(|index| {
            let point = path[index * 2];
            SurfSphere {
                center: point.center,
                radius: point.radius,
            }
        })
        .collect();
    mesh.clear();
    build_ses_surface(
        mesh,
        &spheres,
        probe_radius,
        (1.0 / density).clamp(0.25, 1.5),
    );
    Ok(())
}

pub fn build_hole_test_tube(
    mesh: &mut SurfaceMesh,
    molecule: &MolCoord,
    direction: Vector4,
    start_position: Vector4,
) -> Result<Vec<HolePathPoint>> {
    const SLICE_COUNT: usize = 50;
    const CIRCLE_DIVISIONS: usize = 40;

    let direction = normalized_direction(direction)?;
    let index = HoleAtomIndex::new(molecule, &Selector::All)?;
    let mut rng = DeterministicRng::new(0x484f_4c45_5445_5354);
    let mut position = start_position;
    let mut radius = -1.0;
    let advance = direction * PATH_STEP;
    let mut path = Vec::with_capacity(SLICE_COUNT);

    while path.len() < SLICE_COUNT && radius < MAX_HOLE_RADIUS {
        let point = optimize_slice(&index, direction, 0.000_01, position, &mut rng);
        position = point.center + advance;
        radius = point.radius;
        path.push(point);
    }
    while path.len() < SLICE_COUNT {
        path.push(HolePathPoint {
            center: position,
            radius,
        });
        position = position + advance;
    }

    mesh.clear();
    let (basis_x, basis_y) = perpendicular_basis(direction);
    let angle_step = std::f64::consts::TAU / CIRCLE_DIVISIONS as f64;
    for point in &path {
        for division in 0..CIRCLE_DIVISIONS {
            let angle = division as f64 * angle_step;
            let normal = basis_x * angle.cos() + basis_y * angle.sin();
            mesh.push_vertex(SurfaceVertex {
                position: point.center + normal * point.radius,
                normal,
                info: 0,
            });
        }
    }
    for slice in 0..SLICE_COUNT - 1 {
        let base = slice * CIRCLE_DIVISIONS;
        for division in 0..CIRCLE_DIVISIONS {
            let next = (division + 1) % CIRCLE_DIVISIONS;
            mesh.push_triangle([
                (base + next) as u32,
                (base + division) as u32,
                (base + CIRCLE_DIVISIONS + division) as u32,
            ]);
            mesh.push_triangle([
                (base + CIRCLE_DIVISIONS + division) as u32,
                (base + CIRCLE_DIVISIONS + next) as u32,
                (base + next) as u32,
            ]);
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_molstr::Atom;
    use std::collections::BTreeMap;

    fn channel_molecule() -> MolCoord {
        let mut molecule = MolCoord::new();
        let mut serial = 1;
        for z in [-2.0, 0.0, 2.0, 4.0, 6.0] {
            for division in 0..12 {
                let angle = division as f64 * std::f64::consts::TAU / 12.0;
                molecule.push_atom(Atom {
                    serial,
                    name: "C".to_string(),
                    alt_loc: None,
                    res_name: "LIG".to_string(),
                    chain_id: "A".to_string(),
                    res_id: 1,
                    ins_code: None,
                    position: Vector4::new3(4.0 * angle.cos(), 4.0 * angle.sin(), z),
                    occupancy: 1.0,
                    b_factor: 0.0,
                    element: "C".to_string(),
                    formal_charge: None,
                    int_props: BTreeMap::new(),
                    real_props: BTreeMap::new(),
                });
                serial += 1;
            }
        }
        molecule
    }

    #[test]
    fn traced_path_is_reproducible_and_advances_through_channel() {
        let molecule = channel_molecule();
        let first = trace_hole_path(
            &molecule,
            &Selector::All,
            Vector4::new3(0.0, 0.0, 1.0),
            Vector4::ZERO,
        )
        .unwrap();
        let second = trace_hole_path(
            &molecule,
            &Selector::All,
            Vector4::new3(0.0, 0.0, 1.0),
            Vector4::ZERO,
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(first.len() > 8);
        assert!(first.last().unwrap().center.z > first[0].center.z);
        assert!((1.0..=5.0).contains(&first[0].radius));
    }

    #[test]
    fn debug_tube_matches_reference_mesh_dimensions() {
        let mut mesh = SurfaceMesh::new();
        let path = build_hole_test_tube(
            &mut mesh,
            &channel_molecule(),
            Vector4::new3(0.0, 0.0, 1.0),
            Vector4::ZERO,
        )
        .unwrap();
        assert_eq!(path.len(), 50);
        assert_eq!(mesh.vertex_count(), 2_000);
        assert_eq!(mesh.triangle_count(), 3_920);
    }
}
