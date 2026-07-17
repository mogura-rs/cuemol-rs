use cuemol_core::Vector2;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// A vertex in a planar cut boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryElement {
    pub id: i32,
    pub position: Vector2,
}

impl BoundaryElement {
    pub const fn new(id: i32, position: Vector2) -> Self {
        Self { id, position }
    }
}

/// A closed planar boundary with optional nested contours.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Boundary {
    vertices: Vec<BoundaryElement>,
    insets: BoundarySet,
}

impl Boundary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: i32, position: Vector2) {
        self.vertices.push(BoundaryElement::new(id, position));
    }

    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    pub fn vertices(&self) -> &[BoundaryElement] {
        &self.vertices
    }

    pub fn iter(&self) -> impl Iterator<Item = &BoundaryElement> {
        self.vertices.iter()
    }

    pub fn vertex(&self, index: usize) -> Option<Vector2> {
        self.vertices.get(index).map(|element| element.position)
    }

    pub fn vertex_id(&self, index: usize) -> Option<i32> {
        self.vertices.get(index).map(|element| element.id)
    }

    pub fn circular_vertex(&self, index: usize) -> Option<Vector2> {
        (!self.vertices.is_empty()).then(|| self.vertices[index % self.vertices.len()].position)
    }

    pub fn insets(&self) -> &BoundarySet {
        &self.insets
    }

    pub fn insets_mut(&mut self) -> &mut BoundarySet {
        &mut self.insets
    }

    pub fn insert_inset(&mut self, boundary: Boundary) {
        self.insets.push(boundary);
    }

    pub fn is_enclosing(&self, position: Vector2, check_insets: bool) -> bool {
        self.enclosure_intersections(position, check_insets) % 2 == 1
    }

    pub fn enclosure_intersections(&self, position: Vector2, check_insets: bool) -> usize {
        if self.vertices.is_empty() {
            return 0;
        }

        let mut intersections = 0;
        let mut previous = self.vertices[self.vertices.len() - 1].position;
        for element in &self.vertices {
            let current = element.position;
            if (previous.x - position.x) * (current.x - position.x) < 0.0 {
                let y = (current.y - previous.y) * (position.x - previous.x)
                    / (current.x - previous.x)
                    + previous.y;
                if y < position.y {
                    intersections += 1;
                }
            }
            previous = current;
        }

        if check_insets {
            intersections += self
                .insets
                .iter()
                .map(|inset| inset.enclosure_intersections(position, true))
                .sum::<usize>();
        }
        intersections
    }

    fn absolute_area(&self) -> f64 {
        if self.vertices.len() < 3 {
            return 0.0;
        }
        let mut twice_area = 0.0;
        let mut previous = self.vertices[self.vertices.len() - 1].position;
        for element in &self.vertices {
            let current = element.position;
            twice_area += previous.cross(current);
            previous = current;
        }
        (twice_area * 0.5).abs()
    }

    fn insert_nested(&mut self, boundary: Boundary) {
        let Some(position) = boundary.vertex(0) else {
            return;
        };
        for inset in self.insets.iter_mut() {
            if inset.is_enclosing(position, false) {
                inset.insert_nested(boundary);
                return;
            }
        }
        self.insets.push(boundary);
    }
}

/// A collection of top-level planar boundaries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoundarySet {
    boundaries: Vec<Boundary>,
}

impl BoundarySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.boundaries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.boundaries.is_empty()
    }

    pub fn as_slice(&self) -> &[Boundary] {
        &self.boundaries
    }

    pub fn iter(&self) -> impl Iterator<Item = &Boundary> {
        self.boundaries.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Boundary> {
        self.boundaries.iter_mut()
    }

    pub fn push(&mut self, boundary: Boundary) {
        self.boundaries.push(boundary);
    }

    /// Build boundaries from the directed edge map used by `CutByPlane2`.
    pub fn build<F>(&mut self, edges: &mut BTreeMap<i32, i32>, mut project: F)
    where
        F: FnMut(i32) -> Vector2,
    {
        let mut boundaries = Vec::new();
        while let Some((&first, &next)) = edges.iter().next() {
            edges.remove(&first);
            let mut boundary = Boundary::new();
            boundary.insert(first, project(first));

            let mut current = next;
            while current != first {
                let Some(next) = edges.remove(&current) else {
                    break;
                };
                boundary.insert(current, project(current));
                current = next;
            }
            boundaries.push(boundary);
        }
        *self = Self::from_boundaries(boundaries);
    }

    pub fn from_boundaries(mut boundaries: Vec<Boundary>) -> Self {
        boundaries.retain(|boundary| !boundary.is_empty());
        boundaries.sort_by(|left, right| {
            right
                .absolute_area()
                .partial_cmp(&left.absolute_area())
                .unwrap_or(Ordering::Equal)
        });

        let mut roots = Vec::<Boundary>::new();
        for boundary in boundaries {
            let Some(position) = boundary.vertex(0) else {
                continue;
            };
            if let Some(root) = roots
                .iter_mut()
                .find(|root| root.is_enclosing(position, false))
            {
                root.insert_nested(boundary);
            } else {
                roots.push(boundary);
            }
        }
        Self { boundaries: roots }
    }
}

impl IntoIterator for BoundarySet {
    type Item = Boundary;
    type IntoIter = std::vec::IntoIter<Boundary>;

    fn into_iter(self) -> Self::IntoIter {
        self.boundaries.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(first_id: i32, half_size: f64) -> Boundary {
        let mut boundary = Boundary::new();
        boundary.insert(first_id, Vector2::new(-half_size, -half_size));
        boundary.insert(first_id + 1, Vector2::new(half_size, -half_size));
        boundary.insert(first_id + 2, Vector2::new(half_size, half_size));
        boundary.insert(first_id + 3, Vector2::new(-half_size, half_size));
        boundary
    }

    #[test]
    fn boundary_tests_planar_enclosure() {
        let boundary = square(0, 2.0);
        assert!(boundary.is_enclosing(Vector2::ZERO, false));
        assert!(!boundary.is_enclosing(Vector2::new(3.0, 0.0), false));
        assert_eq!(boundary.circular_vertex(4), boundary.vertex(0));
    }

    #[test]
    fn nested_boundaries_apply_the_even_odd_rule() {
        let island = square(20, 0.25);
        let mut hole = square(10, 1.0);
        hole.insert_inset(island);
        let mut outer = square(0, 3.0);
        outer.insert_inset(hole);

        assert!(outer.is_enclosing(Vector2::new(2.0, 0.0), true));
        assert!(!outer.is_enclosing(Vector2::new(0.5, 0.0), true));
        assert!(outer.is_enclosing(Vector2::ZERO, true));
        assert!(outer.is_enclosing(Vector2::new(0.5, 0.0), false));
    }

    #[test]
    fn boundary_set_builds_and_nests_directed_loops() {
        let positions = BTreeMap::from([
            (0, Vector2::new(-1.0, -1.0)),
            (1, Vector2::new(1.0, -1.0)),
            (2, Vector2::new(1.0, 1.0)),
            (3, Vector2::new(-1.0, 1.0)),
            (10, Vector2::new(-3.0, -3.0)),
            (11, Vector2::new(3.0, -3.0)),
            (12, Vector2::new(3.0, 3.0)),
            (13, Vector2::new(-3.0, 3.0)),
        ]);
        let mut edges = BTreeMap::from([
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (10, 11),
            (11, 12),
            (12, 13),
            (13, 10),
        ]);
        let mut boundaries = BoundarySet::new();
        boundaries.build(&mut edges, |id| positions[&id]);

        assert!(edges.is_empty());
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries.as_slice()[0].insets().len(), 1);
        assert!(!boundaries.as_slice()[0].is_enclosing(Vector2::ZERO, true));
    }
}
