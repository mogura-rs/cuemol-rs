use super::*;

#[derive(Clone, Copy)]
struct Bounds {
    min: [f64; 3],
    max: [f64; 3],
}
impl Bounds {
    fn empty() -> Self {
        Self {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }
    fn include(&mut self, p: Vector4, r: f64) {
        for (i, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            self.min[i] = self.min[i].min(v - r);
            self.max[i] = self.max[i].max(v + r);
        }
    }
    fn merge(&mut self, b: Self) {
        for i in 0..3 {
            self.min[i] = self.min[i].min(b.min[i]);
            self.max[i] = self.max[i].max(b.max[i]);
        }
    }
    fn intersects(self, ray: Ray, limit: f64) -> bool {
        let mut near = 0.0f64;
        let mut far = limit;
        let origin = [ray.origin.x, ray.origin.y, ray.origin.z];
        let direction = [ray.direction.x, ray.direction.y, ray.direction.z];
        for i in 0..3 {
            if direction[i].abs() < 1e-30 {
                if origin[i] < self.min[i] || origin[i] > self.max[i] {
                    return false;
                }
                continue;
            }
            let a = (self.min[i] - origin[i]) / direction[i];
            let b = (self.max[i] - origin[i]) / direction[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return false;
            }
        }
        true
    }
}
#[derive(Clone, Copy)]
enum Shape {
    Atom(usize),
    Bond(usize),
    Triangle(usize, usize),
}
struct Primitive {
    shape: Shape,
    group: Option<usize>,
}
struct Node {
    bounds: Bounds,
    first: usize,
    count: usize,
    right: usize,
}
struct Entry {
    id: usize,
    bounds: Bounds,
}

pub(super) struct RayIndex<'a> {
    snapshot: &'a RenderSceneSnapshot,
    primitives: Vec<Primitive>,
    nodes: Vec<Node>,
    order: Vec<usize>,
    pub translucent_groups: Vec<(usize, f64)>,
}
impl<'a> RayIndex<'a> {
    pub fn new(snapshot: &'a RenderSceneSnapshot) -> Self {
        let mut index = Self {
            snapshot,
            primitives: Vec::new(),
            nodes: Vec::new(),
            order: Vec::new(),
            translucent_groups: snapshot
                .transparency_groups
                .iter()
                .enumerate()
                .filter(|(_, g)| g.alpha < 1.0 - 1e-4)
                .map(|(i, g)| (i, f64::from(g.alpha)))
                .collect(),
        };
        let mut entries = Vec::new();
        let mut add = |shape, group, bounds| {
            entries.push(Entry {
                id: index.primitives.len(),
                bounds,
            });
            index.primitives.push(Primitive { shape, group });
        };
        for (id, atom) in snapshot
            .atoms
            .iter()
            .enumerate()
            .filter(|(_, a)| a.hit.is_none())
        {
            let mut bounds = Bounds::empty();
            bounds.include(
                atom.center,
                f64::from(atom.radius.abs() + atom.edge.map(|e| e.width.max(0.)).unwrap_or(0.))
                    + 1e-7,
            );
            add(
                Shape::Atom(id),
                transparency_group_for_atom(snapshot, id),
                bounds,
            );
        }
        for (id, bond) in snapshot.bonds.iter().enumerate() {
            let mut bounds = Bounds::empty();
            let radius = f64::from(
                bond.radius.abs()
                    + bond
                        .edge
                        .filter(|_| bond.screen_width.is_none())
                        .map(|e| e.width.max(0.))
                        .unwrap_or(0.),
            ) + 1e-7;
            bounds.include(bond.from, radius);
            if !bond.screen_point {
                bounds.include(bond.to, radius);
            }
            add(
                Shape::Bond(id),
                transparency_group_for_bond(snapshot, id),
                bounds,
            );
        }
        for (id, mesh) in snapshot.meshes.iter().enumerate() {
            let group = transparency_group_for_mesh(snapshot, id);
            for (triangle, vertices) in mesh.indices.chunks_exact(3).enumerate() {
                let mut bounds = Bounds::empty();
                let mut valid = true;
                for vertex in vertices {
                    if let Some(v) = mesh.vertices.get(*vertex as usize) {
                        bounds.include(v.position, 1e-7);
                    } else {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    add(Shape::Triangle(id, triangle), group, bounds);
                }
            }
        }
        if !entries.is_empty() {
            index.build(&mut entries, 0);
        }
        index.order = entries.into_iter().map(|e| e.id).collect();
        index
    }
    fn build(&mut self, entries: &mut [Entry], offset: usize) -> usize {
        let mut bounds = Bounds::empty();
        for e in entries.iter() {
            bounds.merge(e.bounds);
        }
        let id = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            first: offset,
            count: entries.len(),
            right: 0,
        });
        if entries.len() > 8 {
            let axis = (0..3)
                .max_by(|a, b| {
                    (bounds.max[*a] - bounds.min[*a]).total_cmp(&(bounds.max[*b] - bounds.min[*b]))
                })
                .unwrap();
            let middle = entries.len() / 2;
            entries.select_nth_unstable_by(middle, |a, b| {
                (a.bounds.min[axis] + a.bounds.max[axis])
                    .total_cmp(&(b.bounds.min[axis] + b.bounds.max[axis]))
                    .then(a.id.cmp(&b.id))
            });
            let (left, right) = entries.split_at_mut(middle);
            let first = self.build(left, offset);
            let right = self.build(right, offset + middle);
            self.nodes[id].first = first;
            self.nodes[id].right = right;
            self.nodes[id].count = 0;
        }
        id
    }
    fn visit(&self, ray: Ray, limit: f64, mut visitor: impl FnMut(usize) -> bool) {
        if self.nodes.is_empty() {
            return;
        }
        // Median splits bound depth by log2(primitive count), below usize::BITS.
        let mut stack = [0usize; usize::BITS as usize + 1];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let node = &self.nodes[stack[top]];
            if !node.bounds.intersects(ray, limit) {
                continue;
            }
            if node.count != 0 {
                for &id in &self.order[node.first..node.first + node.count] {
                    if visitor(id) {
                        return;
                    }
                }
            } else {
                stack[top] = node.first;
                stack[top + 1] = node.right;
                top += 2;
            }
        }
    }
    fn hit(&self, id: usize, ray: Ray, visible_group: Option<usize>) -> Option<Hit> {
        let primitive = &self.primitives[id];
        let group = primitive.group;
        if let Some(g) = group {
            if self.snapshot.transparency_groups[g].alpha < 1. - 1e-4
                && group != visible_group
                && visible_group != Some(usize::MAX)
            {
                return None;
            }
        }
        let mut result = match primitive.shape {
            Shape::Atom(i) => {
                let atom = &self.snapshot.atoms[i];
                if let Some((distance, normal)) =
                    ray_sphere(ray, atom.center, f64::from(atom.radius))
                {
                    Hit {
                        order: id,
                        distance,
                        position: ray.origin + ray.direction * distance,
                        normal,
                        color: color_from_spec(atom.color.as_ref()),
                        unlit: false,
                        transparency_group: group,
                    }
                } else {
                    let edge = atom.edge.filter(|e| e.width > 0.)?;
                    let (distance, normal) =
                        ray_sphere(ray, atom.center, f64::from(atom.radius + edge.width))?;
                    primitive_edge_hit(ray, distance, normal, edge, group)
                }
            }
            Shape::Bond(i) => {
                let bond = &self.snapshot.bonds[i];
                let intersect = |radius| {
                    if bond.screen_point {
                        ray_sphere(ray, bond.from, radius)
                    } else {
                        ray_cylinder(ray, bond.from, bond.to, radius)
                    }
                };
                if let Some((distance, normal)) = intersect(f64::from(bond.radius)) {
                    Hit {
                        order: id,
                        distance,
                        position: ray.origin + ray.direction * distance,
                        normal,
                        color: color_from_spec(bond.color.as_ref()),
                        unlit: false,
                        transparency_group: group,
                    }
                } else {
                    let edge = bond
                        .edge
                        .filter(|e| e.width > 0. && bond.screen_width.is_none())?;
                    let (distance, normal) = intersect(f64::from(bond.radius + edge.width))?;
                    primitive_edge_hit(ray, distance, normal, edge, group)
                }
            }
            Shape::Triangle(mesh, triangle) => {
                let mesh = &self.snapshot.meshes[mesh];
                let triangle = &mesh.indices[triangle * 3..triangle * 3 + 3];
                let v0 = &mesh.vertices[triangle[0] as usize];
                let v1 = &mesh.vertices[triangle[1] as usize];
                let v2 = &mesh.vertices[triangle[2] as usize];
                let (distance, u, v) = ray_triangle(
                    ray,
                    [v0.position, v1.position, v2.position],
                    v0.culls_back_faces(),
                )?;
                let w = 1. - u - v;
                let normal = normalize(v0.normal * w + v1.normal * u + v2.normal * v);
                let color = std::array::from_fn(|c| {
                    f64::from(v0.color[c]) * w
                        + f64::from(v1.color[c]) * u
                        + f64::from(v2.color[c]) * v
                });
                Hit {
                    order: id,
                    distance,
                    position: ray.origin + ray.direction * distance,
                    normal: if normal.dot3d(ray.direction) > 0. {
                        normal * -1.
                    } else {
                        normal
                    },
                    color,
                    unlit: v0.is_edge_outline(),
                    transparency_group: group,
                }
            }
        };
        result.order = id;
        Some(result)
    }
    pub fn hits(&self, ray: Ray, group: Option<usize>) -> Vec<Hit> {
        let mut hits = Vec::new();
        self.visit(ray, f64::INFINITY, |id| {
            if let Some(hit) = self.hit(id, ray, group) {
                hits.push(hit);
            }
            false
        });
        hits
    }
    pub fn occluded(&self, ray: Ray, limit: f64, group: Option<usize>) -> bool {
        let mut found = false;
        self.visit(ray, limit, |id| {
            found = self
                .hit(id, ray, group)
                .is_some_and(|h| h.distance > 1e-4 && (!limit.is_finite() || h.distance <= limit));
            found
        });
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MeshVertex, RenderAtom, RenderBond, RenderEdgeMode, RenderEdgeStyle, RenderMesh};
    #[test]
    fn indexed_hits_match_brute_force_at_boundaries_and_for_shadow_rays() {
        let mut scene = RenderSceneSnapshot::empty();
        for i in 0..73 {
            let mut atom = RenderAtom::new(
                Vector4::new3(
                    (i % 9) as f64 * 2. - 8.,
                    (i / 9) as f64 * 1.5 - 5.,
                    (i % 5) as f64 * 0.3,
                ),
                0.5,
                None,
            );
            atom.edge = Some(RenderEdgeStyle {
                mode: RenderEdgeMode::Silhouette,
                width: 0.1,
                color: [0., 0., 0., 1.],
            });
            scene.atoms.push(atom);
        }
        scene.bonds.push(RenderBond::new(
            Vector4::new3(-3., 0., 1.),
            Vector4::new3(3., 0., 1.),
            0.25,
            None,
        ));
        scene.meshes.push(RenderMesh {
            vertices: [[-5., -5., -1.], [5., -5., -1.], [0., 5., -1.]]
                .map(|p| MeshVertex {
                    position: Vector4::new3(p[0], p[1], p[2]),
                    normal: Vector4::new3(0., 0., 1.),
                    color: [1., 0., 0., 0.5],
                })
                .to_vec(),
            indices: vec![0, 1, 2],
        });
        let index = RayIndex::new(&scene);
        for i in 0..1000 {
            let ray = Ray {
                origin: Vector4::new3((i % 37) as f64 * 0.51 - 9., (i / 37) as f64 * 0.48 - 6., 8.),
                direction: normalize(Vector4::new3(if i % 2 == 0 { 0. } else { 0.1 }, 0., -1.)),
            };
            let mut actual = index.hits(ray, None);
            let mut expected = scene_hits(&scene, ray, None);
            let sort = |a: &Hit, b: &Hit| {
                a.distance
                    .total_cmp(&b.distance)
                    .then(a.order.cmp(&b.order))
            };
            actual.sort_by(sort);
            expected.sort_by(sort);
            assert_eq!(actual.len(), expected.len());
            for (a, b) in actual.iter().zip(&expected) {
                assert_eq!(a.distance, b.distance);
                assert_eq!(a.color, b.color);
                assert_eq!(a.position, b.position);
                assert_eq!(a.normal, b.normal);
            }
            for max in [0.01, 7., 12., f64::INFINITY] {
                assert_eq!(
                    index.occluded(ray, max, None),
                    expected
                        .iter()
                        .any(|h| h.distance > 1e-4 && (!max.is_finite() || h.distance <= max))
                );
            }
        }
    }
}
