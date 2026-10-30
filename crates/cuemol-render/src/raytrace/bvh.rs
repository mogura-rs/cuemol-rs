use super::*;

struct BoundsRay {
    origin: [f64; 3],
    inverse_direction: [f64; 3],
    parallel: [bool; 3],
    negative: [bool; 3],
    has_parallel: bool,
}

impl BoundsRay {
    fn new(ray: Ray) -> Self {
        let direction = [ray.direction.x, ray.direction.y, ray.direction.z];
        let parallel = direction.map(|value| value.abs() < 1e-30);
        Self {
            origin: [ray.origin.x, ray.origin.y, ray.origin.z],
            inverse_direction: std::array::from_fn(|axis| {
                if parallel[axis] {
                    0.
                } else {
                    1. / direction[axis]
                }
            }),
            parallel,
            negative: direction.map(|value| value < 0.),
            has_parallel: parallel.into_iter().any(|value| value),
        }
    }
}

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
    fn area(self) -> f64 {
        let d: [f64; 3] = std::array::from_fn(|i| (self.max[i] - self.min[i]).max(0.));
        2. * (d[0] * d[1] + d[1] * d[2] + d[2] * d[0])
    }
    fn intersects(self, ray: &BoundsRay, limit: f64) -> bool {
        self.entry_distance(ray, limit).is_some()
    }
    fn entry_distance(self, ray: &BoundsRay, limit: f64) -> Option<f64> {
        if !ray.has_parallel {
            let mut near = 0.0f64;
            let mut far = limit;
            for i in 0..3 {
                let (start, end) = if ray.negative[i] {
                    (self.max[i], self.min[i])
                } else {
                    (self.min[i], self.max[i])
                };
                near = near.max((start - ray.origin[i]) * ray.inverse_direction[i]);
                far = far.min((end - ray.origin[i]) * ray.inverse_direction[i]);
            }
            return (near <= far).then_some(near);
        }
        let mut near = 0.0f64;
        let mut far = limit;
        for i in 0..3 {
            if ray.parallel[i] {
                if ray.origin[i] < self.min[i] || ray.origin[i] > self.max[i] {
                    return None;
                }
                continue;
            }
            let a = (self.min[i] - ray.origin[i]) * ray.inverse_direction[i];
            let b = (self.max[i] - ray.origin[i]) * ray.inverse_direction[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
        Some(near)
    }
}
#[derive(Clone, Copy)]
enum Shape {
    Atom(usize),
    Bond(usize),
    Triangle(usize, usize),
    OutlineTriangle(usize, usize),
}
struct Primitive {
    shape: Shape,
    group: Option<usize>,
    tie_priority: usize,
}
#[derive(Clone, Copy)]
struct Node {
    bounds: Bounds,
    first: usize,
    count: usize,
}
struct Entry {
    id: usize,
    bounds: Bounds,
}

fn entry_bounds(entries: &[Entry]) -> Bounds {
    let mut bounds = Bounds::empty();
    for entry in entries {
        bounds.merge(entry.bounds);
    }
    bounds
}

fn median_partition(entries: &mut [Entry], bounds: Bounds) -> usize {
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
    middle
}

fn area_partition(entries: &mut [Entry], bounds: Bounds) -> Option<usize> {
    const BINS: usize = 12;
    let mut best_cost = bounds.area() * (entries.len() as f64 - 1.);
    let mut best = None;
    for axis in 0..3 {
        let center = |entry: &Entry| entry.bounds.min[axis] + entry.bounds.max[axis];
        let minimum = entries.iter().map(center).fold(f64::INFINITY, f64::min);
        let maximum = entries.iter().map(center).fold(f64::NEG_INFINITY, f64::max);
        if !(maximum > minimum && (maximum - minimum).is_finite()) {
            continue;
        }
        let scale = BINS as f64 / (maximum - minimum);
        let bin = |entry: &Entry| (((center(entry) - minimum) * scale) as usize).min(BINS - 1);
        let mut counts = [0usize; BINS];
        let mut boxes = [Bounds::empty(); BINS];
        for entry in entries.iter() {
            let i = bin(entry);
            counts[i] += 1;
            boxes[i].merge(entry.bounds);
        }
        let mut left_cost = [0.; BINS - 1];
        let mut combined = Bounds::empty();
        let mut count = 0;
        for i in 0..BINS - 1 {
            combined.merge(boxes[i]);
            count += counts[i];
            left_cost[i] = if count == 0 {
                f64::INFINITY
            } else {
                combined.area() * count as f64
            };
        }
        combined = Bounds::empty();
        count = 0;
        for i in (1..BINS).rev() {
            combined.merge(boxes[i]);
            count += counts[i];
            if count == 0 {
                continue;
            }
            let cost = left_cost[i - 1] + combined.area() * count as f64;
            if cost < best_cost {
                best_cost = cost;
                best = Some((axis, i, minimum, scale));
            }
        }
    }
    let (axis, split, minimum, scale) = best?;
    let mut middle = 0;
    for i in 0..entries.len() {
        let center = entries[i].bounds.min[axis] + entries[i].bounds.max[axis];
        let bin = (((center - minimum) * scale) as usize).min(BINS - 1);
        if bin < split {
            entries.swap(i, middle);
            middle += 1;
        }
    }
    (middle > 0 && middle < entries.len()).then_some(middle)
}

pub(super) struct RayIndex<'a> {
    snapshot: &'a RenderSceneSnapshot,
    primitives: Vec<Primitive>,
    nodes: Vec<Node>,
    order: Vec<usize>,
    cylinder_axes: Vec<CylinderAxis>,
    pub translucent_groups: Vec<(usize, f64)>,
    pub edges: super::edges::EdgeGroups,
    pub opaque_without_edges: bool,
}
impl<'a> RayIndex<'a> {
    pub fn geometry_id(&self, hit: Hit) -> (u8, usize) {
        match self.primitives.get(hit.order).map(|p| p.shape) {
            Some(Shape::Atom(id)) => (0, id),
            Some(Shape::Bond(id)) => (1, id),
            Some(Shape::Triangle(id, _)) => (2, id),
            Some(Shape::OutlineTriangle(id, _)) => (3, id),
            None => (4, hit.order),
        }
    }

    pub fn snapshot(&self) -> &'a RenderSceneSnapshot {
        self.snapshot
    }

    pub fn volume_hits(&self, ray: Ray, camera: &Camera, clip: bool, far: f64) -> Vec<Hit> {
        let mut hits = Vec::new();
        if self.snapshot.volumes.is_empty() {
            return hits;
        }
        let rotation = camera
            .rotation()
            .inv()
            .unwrap_or(cuemol_core::Quat::IDENTITY)
            .to_matrix();
        let front = transform_direction(rotation, Vector4::new3(0., 0., 1.));
        let eye = camera.center() + front * camera.cam_dist();
        for (id, volume) in self.snapshot.volumes.iter().enumerate() {
            let Some(samples) = volume.ray_samples(ray.origin, ray.direction, eye, front, far)
            else {
                continue;
            };
            for (distance, color) in samples {
                let position = ray.origin + ray.direction * distance;
                if clip
                    && (position - camera.center()).dot3d(front).abs() > camera.slab_depth() * 0.5
                {
                    continue;
                }
                hits.push(Hit {
                    order: self.primitives.len() + id,
                    distance,
                    position,
                    normal: Vector4::ZERO,
                    color,
                    unlit: true,
                    is_volume: true,
                    transparency_group: None,
                });
            }
        }
        hits
    }

    #[cfg(test)]
    pub fn new(snapshot: &'a RenderSceneSnapshot) -> Self {
        Self::with_pixel_scale(snapshot, 0.0)
    }

    pub fn with_pixel_scale(snapshot: &'a RenderSceneSnapshot, pixel_scale: f32) -> Self {
        let mut index = Self {
            snapshot,
            primitives: Vec::new(),
            nodes: Vec::new(),
            order: Vec::new(),
            cylinder_axes: snapshot
                .bonds
                .iter()
                .map(|bond| CylinderAxis::new(bond.from, bond.to))
                .collect(),
            translucent_groups: Vec::new(),
            edges: super::edges::EdgeGroups::new(snapshot, pixel_scale),
            opaque_without_edges: snapshot.volumes.is_empty()
                && snapshot
                    .transparency_groups
                    .iter()
                    .all(|group| group.alpha >= 1. && group.edge_style.is_none())
                && snapshot.atoms.iter().all(|atom| {
                    atom.edge.is_none() && color_from_spec(atom.color.as_ref())[3] >= 1.
                })
                && snapshot.bonds.iter().all(|bond| {
                    bond.edge.is_none() && color_from_spec(bond.color.as_ref())[3] >= 1.
                })
                && snapshot
                    .meshes
                    .iter()
                    .flat_map(|mesh| &mesh.vertices)
                    .all(|vertex| !vertex.is_edge_outline() && vertex.color[3] >= 1.),
        };
        for (i, group) in snapshot.transparency_groups.iter().enumerate() {
            if group.alpha < 1. - 1e-4
                && !index
                    .translucent_groups
                    .iter()
                    .any(|(_, a)| (*a - group.alpha as f64).abs() <= 1e-4)
            {
                index
                    .translucent_groups
                    .push((i, group.alpha.clamp(0., 1.) as f64));
            }
        }
        let mut entries = Vec::new();
        let mut add = |shape, group, bounds| {
            entries.push(Entry {
                id: index.primitives.len(),
                bounds,
            });
            index.primitives.push(Primitive {
                shape,
                group,
                tie_priority: 0,
            });
        };
        for (id, atom) in snapshot
            .atoms
            .iter()
            .enumerate()
            .filter(|(_, a)| a.hit.is_none())
        {
            let group = transparency_group_for_atom(snapshot, id);
            let edge = index.edges.style(group).or(atom.edge);
            let mut bounds = Bounds::empty();
            bounds.include(
                atom.center,
                f64::from(atom.radius.abs() + edge.map(|e| e.width.max(0.)).unwrap_or(0.)) + 1e-7,
            );
            add(Shape::Atom(id), group, bounds);
        }
        for (id, bond) in snapshot.bonds.iter().enumerate() {
            let group = transparency_group_for_bond(snapshot, id);
            let edge = index.edges.style(group).or(bond.edge);
            let mut bounds = Bounds::empty();
            let radius = f64::from(
                bond.radius.abs()
                    + edge
                        .filter(|_| bond.screen_width.is_none())
                        .map(|e| e.width.max(0.))
                        .unwrap_or(0.),
            ) + 1e-7;
            bounds.include(bond.from, radius);
            if !bond.screen_point {
                bounds.include(bond.to, radius);
            }
            add(Shape::Bond(id), group, bounds);
        }
        for (id, mesh) in snapshot.meshes.iter().enumerate() {
            let group = transparency_group_for_mesh(snapshot, id);
            let edge = index.edges.style(group);
            let outline = mesh
                .vertices
                .first()
                .is_some_and(|vertex| vertex.is_edge_outline());
            if outline && edge.is_some() {
                continue;
            }
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
                    if let Some(edge) = edge.filter(|_| !outline) {
                        let mut bounds = Bounds::empty();
                        for &vertex in vertices {
                            let vertex = &mesh.vertices[vertex as usize];
                            bounds.include(vertex.position, f64::from(edge.width) + 1e-7);
                        }
                        add(Shape::OutlineTriangle(id, triangle), group, bounds);
                    }
                }
            }
        }
        if !entries.is_empty() {
            index.assign_tie_priorities(&mut entries, &mut 0);
            index.nodes.push(Node {
                bounds: Bounds::empty(),
                first: 0,
                count: 0,
            });
            index.build(&mut entries, 0, 0, 0);
        }
        index.order = entries.into_iter().map(|e| e.id).collect();
        index
    }
    fn assign_tie_priorities(&mut self, entries: &mut [Entry], priority: &mut usize) {
        // Preserve coincident-surface choices from the original median tree.
        if entries.len() > 8 {
            let middle = median_partition(entries, entry_bounds(entries));
            let (left, right) = entries.split_at_mut(middle);
            self.assign_tie_priorities(right, priority);
            self.assign_tie_priorities(left, priority);
        } else {
            for entry in entries {
                self.primitives[entry.id].tie_priority = *priority;
                *priority += 1;
            }
        }
    }
    fn build(&mut self, entries: &mut [Entry], offset: usize, depth: usize, id: usize) {
        let bounds = entry_bounds(entries);
        self.nodes[id] = Node {
            bounds,
            first: offset,
            count: entries.len(),
        };
        if entries.len() <= 4 || depth >= usize::BITS as usize - 2 {
            return;
        }
        let Some(middle) = area_partition(entries, bounds) else {
            return;
        };
        let (left, right) = entries.split_at_mut(middle);
        let first = self.nodes.len();
        self.nodes.extend(
            [Node {
                bounds,
                first: 0,
                count: 0,
            }; 2],
        );
        self.build(left, offset, depth + 1, first);
        self.build(right, offset + middle, depth + 1, first + 1);
        self.nodes[id].first = first;
        self.nodes[id].count = 0;
    }
    fn visit(&self, ray: Ray, limit: f64, mut visitor: impl FnMut(usize) -> bool) {
        if self.nodes.is_empty() {
            return;
        }
        let bounds_ray = BoundsRay::new(ray);
        // Construction caps depth below the fixed traversal stack capacity.
        let mut stack = [0usize; usize::BITS as usize + 1];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let node = &self.nodes[stack[top]];
            if !node.bounds.intersects(&bounds_ray, limit) {
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
                stack[top + 1] = node.first + 1;
                top += 2;
            }
        }
    }
    fn hit(&self, id: usize, ray: Ray, visible_group: Option<usize>) -> Option<Hit> {
        let primitive = &self.primitives[id];
        let group = primitive.group;
        if let Some(g) = group {
            if self.snapshot.transparency_groups[g].alpha < 1. - 1e-4
                && !visible_group.is_some_and(|selected| {
                    selected == usize::MAX
                        || (self.snapshot.transparency_groups[selected].alpha
                            - self.snapshot.transparency_groups[g].alpha)
                            .abs()
                            <= 1e-4
                })
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
                        is_volume: false,
                        transparency_group: group,
                    }
                } else {
                    let edge = self.edge_style(group, atom.edge).filter(|e| e.width > 0.)?;
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
                        self.cylinder_axes[i].intersect(ray, bond.from, bond.to, radius)
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
                        is_volume: false,
                        transparency_group: group,
                    }
                } else {
                    let edge = self
                        .edge_style(group, bond.edge)
                        .filter(|e| e.width > 0. && bond.screen_width.is_none())?;
                    let (distance, normal) = intersect(f64::from(bond.radius + edge.width))?;
                    primitive_edge_hit(ray, distance, normal, edge, group)
                }
            }
            Shape::OutlineTriangle(mesh, triangle) => {
                let style = self.edge_style(group, None)?;
                let mesh = &self.snapshot.meshes[mesh];
                let ids = &mesh.indices[triangle * 3..triangle * 3 + 3];
                let vertices = [ids[0], ids[2], ids[1]].map(|id| mesh.vertices[id as usize]);
                let positions = vertices.map(|vertex| {
                    vertex.position + normalize(vertex.normal) * f64::from(style.width)
                });
                let (distance, u, v) = ray_triangle(ray, positions, true)?;
                let normal = normalize(
                    vertices[0].normal * (1.0 - u - v)
                        + vertices[1].normal * u
                        + vertices[2].normal * v,
                );
                primitive_edge_hit(ray, distance, normal, style, group)
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
                    is_volume: false,
                    transparency_group: group,
                }
            }
        };
        result.order = id;
        Some(result)
    }
    fn edge_style(
        &self,
        section: Option<usize>,
        fallback: Option<crate::RenderEdgeStyle>,
    ) -> Option<crate::RenderEdgeStyle> {
        self.edges
            .style(section)
            .map(|mut style| {
                if let Some(section) = section {
                    style.color[3] *= self.snapshot.transparency_groups[section].alpha;
                }
                style
            })
            .or(fallback)
    }
    pub fn geometric_normal(&self, hit: Hit) -> Vector4 {
        let Some(primitive) = self.primitives.get(hit.order) else {
            return hit.normal;
        };
        let Shape::Triangle(mesh, triangle) = primitive.shape else {
            return hit.normal;
        };
        let mesh = &self.snapshot.meshes[mesh];
        let ids = &mesh.indices[triangle * 3..triangle * 3 + 3];
        let positions = [ids[0], ids[1], ids[2]].map(|id| mesh.vertices[id as usize].position);
        let normal = cross(positions[1] - positions[0], positions[2] - positions[0])
            .normalize3d()
            .unwrap_or(hit.normal);
        if normal.dot3d(hit.normal) < 0. {
            normal * -1.
        } else {
            normal
        }
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
    pub fn nearest(&self, ray: Ray, limit: f64, group: Option<usize>) -> Option<Hit> {
        self.nearest_where(ray, limit, group, |_| true)
    }
    pub fn nearest_where(
        &self,
        ray: Ray,
        limit: f64,
        group: Option<usize>,
        accept: impl Fn(Hit) -> bool,
    ) -> Option<Hit> {
        self.nearest_impl::<false>(ray, limit, group, accept)
    }

    pub fn nearest_visible(&self, ray: Ray, accept: impl Fn(Hit) -> bool) -> Option<Hit> {
        self.nearest_impl::<true>(ray, f64::INFINITY, None, accept)
            .map(|hit| self.primary_hit(hit, ray))
    }

    pub fn primary_hit(&self, mut hit: Hit, ray: Ray) -> Hit {
        if !hit.unlit {
            let mut geometric = self.geometric_normal(hit);
            if geometric.dot3d(ray.direction) > 0. {
                geometric = geometric * -1.;
            }
            if hit.normal.dot3d(geometric) < 0. {
                hit.normal = hit.normal * -1.;
            }
        }
        hit
    }

    fn nearest_impl<const SOURCE_ORDER: bool>(
        &self,
        ray: Ray,
        mut limit: f64,
        group: Option<usize>,
        accept: impl Fn(Hit) -> bool,
    ) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let bounds_ray = BoundsRay::new(ray);
        let near = self.nodes[0].bounds.entry_distance(&bounds_ray, limit)?;
        let mut result: Option<Hit> = None;
        let mut stack = [(0usize, 0.0); usize::BITS as usize + 1];
        stack[0] = (0, near);
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let (node_id, near) = stack[top];
            if near > limit {
                continue;
            }
            let node = &self.nodes[node_id];
            if node.count != 0 {
                for &id in &self.order[node.first..node.first + node.count] {
                    if let Some(hit) = self.hit(id, ray, group) {
                        if (SOURCE_ORDER || hit.distance > 1e-4)
                            && hit.distance <= limit
                            && accept(hit)
                            && result.is_none_or(|previous| {
                                hit.distance < previous.distance
                                    || if SOURCE_ORDER {
                                        hit.order < previous.order
                                    } else {
                                        self.primitives[hit.order].tie_priority
                                            >= self.primitives[previous.order].tie_priority
                                    }
                            })
                        {
                            limit = hit.distance;
                            result = Some(hit);
                        }
                    }
                }
            } else {
                let left = self.nodes[node.first]
                    .bounds
                    .entry_distance(&bounds_ray, limit);
                let right = self.nodes[node.first + 1]
                    .bounds
                    .entry_distance(&bounds_ray, limit);
                match (left, right) {
                    (Some(left), Some(right)) => {
                        let (near, far) = if left < right {
                            ((node.first, left), (node.first + 1, right))
                        } else {
                            ((node.first + 1, right), (node.first, left))
                        };
                        stack[top] = far;
                        stack[top + 1] = near;
                        top += 2;
                    }
                    (Some(near), None) | (None, Some(near)) => {
                        stack[top] = (
                            if left.is_some() {
                                node.first
                            } else {
                                node.first + 1
                            },
                            near,
                        );
                        top += 1;
                    }
                    (None, None) => {}
                }
            }
        }
        result
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
    fn grazing_primary_normals_follow_the_geometric_front_face() {
        let normal = normalize(Vector4::new3(1., 0., 0.05));
        let mut scene = RenderSceneSnapshot::empty();
        scene.meshes.push(RenderMesh {
            vertices: [(-5., -5.), (5., -5.), (0., 5.)]
                .map(|(x, y)| MeshVertex {
                    position: Vector4::new3(x, y, 0.),
                    normal,
                    color: [1.; 4],
                    pick_atom_index: None,
                })
                .to_vec(),
            indices: vec![0, 1, 2],
        });
        let index = RayIndex::new(&scene);
        for x in [-0.1, 0.1] {
            let ray = Ray {
                origin: Vector4::new3(0., 0., 1.),
                direction: normalize(Vector4::new3(x, 0., -1.)),
            };
            let primary = index.nearest_visible(ray, |_| true).unwrap();
            assert!((primary.normal - normal).length3d() < 1e-12);
            let secondary = index.nearest(ray, f64::INFINITY, None).unwrap();
            assert!(secondary.normal.dot3d(ray.direction) <= 0.);
            assert_eq!(index.primary_hit(secondary, ray).normal, primary.normal);
        }
    }

    #[test]
    fn bounds_rays_keep_parallel_boundary_and_distance_limits() {
        let bounds = Bounds {
            min: [-1.; 3],
            max: [1.; 3],
        };
        for (origin, direction, limit, expected) in [
            ([1., 0., 3.], [0., -0., -1.], 2., true),
            ([1., 0., 3.], [0., 0., -1.], 1.999, false),
            ([1.000001, 0., 3.], [0., 0., -1.], 4., false),
            ([1.000001, 0., 3.], [-1e-31, 0., -1.], 4., false),
            ([0., 0., 0.], [1., 0., 0.], 0., true),
            ([3., 0., 0.], [1., 0., 0.], f64::INFINITY, false),
            ([3., 0., 0.], [-1., 0., 0.], 2., true),
        ] {
            let ray = BoundsRay::new(Ray {
                origin: Vector4::new3(origin[0], origin[1], origin[2]),
                direction: Vector4::new3(direction[0], direction[1], direction[2]),
            });
            assert_eq!(bounds.intersects(&ray, limit), expected);
        }
    }

    #[test]
    fn nearest_keeps_traversal_ties_for_coincident_surfaces() {
        let mut scene = RenderSceneSnapshot::empty();
        for _ in 0..37 {
            scene.atoms.push(RenderAtom::new(Vector4::ZERO, 1., None));
        }
        let index = RayIndex::new(&scene);
        let ray = Ray {
            origin: Vector4::new3(0., 0., 4.),
            direction: Vector4::new3(0., 0., -1.),
        };
        assert_eq!(index.nearest(ray, f64::INFINITY, None).unwrap().order, 3);
        assert_eq!(
            index
                .nearest_where(ray, f64::INFINITY, None, |h| h.order % 2 == 0)
                .unwrap()
                .order,
            2
        );
        assert_eq!(index.nearest_visible(ray, |_| true).unwrap().order, 0);
    }

    #[test]
    fn primary_hits_keep_near_surfaces_and_source_order_with_clipping() {
        let mut scene = RenderSceneSnapshot::empty();
        for i in 0..37 {
            scene.atoms.push(RenderAtom::new(
                Vector4::new3(0., 0., if i < 20 { 0. } else { -3. }),
                1.,
                None,
            ));
        }
        let index = RayIndex::new(&scene);
        for z in [4., 1.00001] {
            let ray = Ray {
                origin: Vector4::new3(0., 0., z),
                direction: Vector4::new3(0., 0., -1.),
            };
            for clip in [false, true] {
                let accept = |hit: Hit| !clip || hit.position.z < -1.;
                let expected = index
                    .hits(ray, None)
                    .into_iter()
                    .filter(|hit| accept(*hit))
                    .min_by(|a, b| {
                        a.distance
                            .total_cmp(&b.distance)
                            .then(a.order.cmp(&b.order))
                    });
                let actual = index.nearest_visible(ray, accept);
                assert_eq!(
                    actual.map(|hit| (hit.order, hit.distance)),
                    expected.map(|hit| (hit.order, hit.distance))
                );
                if z < 2. && !clip {
                    assert!(actual.unwrap().distance < 1e-4);
                }
            }
        }
    }

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
                    pick_atom_index: None,
                    position: Vector4::new3(p[0], p[1], p[2]),
                    normal: normalize(Vector4::new3(p[0] * 0.03, p[1] * 0.04, 1.)),
                    color: [
                        (p[0] + 5.) as f32 / 10.,
                        0.25,
                        (p[1] + 5.) as f32 / 10.,
                        0.5,
                    ],
                })
                .to_vec(),
            indices: vec![0, 1, 2],
        });
        let index = RayIndex::new(&scene);
        for i in 0..1000 {
            let ray = Ray {
                origin: Vector4::new3((i % 37) as f64 * 0.51 - 9., (i / 37) as f64 * 0.48 - 6., 8.),
                direction: normalize(Vector4::new3(
                    if i % 2 == 0 { 0. } else { 0.1 },
                    if i % 3 == 0 { 0. } else { -0.05 },
                    -1.,
                )),
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
                let nearest = index.nearest(ray, max, None);
                let brute = expected
                    .iter()
                    .find(|h| h.distance > 1e-4 && h.distance <= max);
                assert_eq!(nearest.map(|h| h.distance), brute.map(|h| h.distance));
                if let (Some(actual), Some(expected)) = (nearest, brute) {
                    assert_eq!(actual.position, expected.position);
                    assert_eq!(actual.normal, expected.normal);
                    assert_eq!(actual.color, expected.color);
                    assert_eq!(actual.unlit, expected.unlit);
                }
                let accepted = index.nearest_where(ray, max, None, |h| h.position.x > 0.);
                let brute = expected
                    .iter()
                    .find(|h| h.distance > 1e-4 && h.distance <= max && h.position.x > 0.);
                assert_eq!(accepted.map(|h| h.distance), brute.map(|h| h.distance));
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
