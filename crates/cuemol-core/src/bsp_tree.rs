use crate::{CueMolError, Result, Vector4};
use std::cmp::Ordering;

#[derive(Debug, Clone)]
struct Tuple<T> {
    position: [f64; 3],
    value: T,
}

#[derive(Debug, Clone)]
struct Node {
    start: usize,
    end: usize,
    direction: usize,
    divider: f64,
    minimum: Vector4,
    maximum: Vector4,
    lower: Option<Box<Node>>,
    upper: Option<Box<Node>>,
}

impl Node {
    fn leaf(start: usize, end: usize, minimum: Vector4, maximum: Vector4) -> Self {
        Self {
            start,
            end,
            direction: 0,
            divider: 0.0,
            minimum,
            maximum,
            lower: None,
            upper: None,
        }
    }

    fn is_leaf(&self) -> bool {
        self.lower.is_none()
    }
}

/// Three-dimensional binary spatial partition tree corresponding to
/// `qlib::BSPTree`.
#[derive(Debug, Clone)]
pub struct BSPTree<T> {
    root: Option<Box<Node>>,
    data: Vec<Tuple<T>>,
    granularity: usize,
    max_levels: usize,
}

impl<T> Default for BSPTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> BSPTree<T> {
    pub const fn new() -> Self {
        Self {
            root: None,
            data: Vec::new(),
            granularity: 10,
            max_levels: 1_000_000,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn is_built(&self) -> bool {
        self.root.is_some()
    }

    pub fn clear(&mut self) {
        self.root = None;
        self.data.clear();
    }

    pub fn granularity(&self) -> usize {
        self.granularity
    }

    pub fn set_granularity(&mut self, value: usize) -> Result<()> {
        if value < 3 {
            return Err(CueMolError::invalid_argument(
                "BSP granularity must be at least three",
            ));
        }
        self.granularity = value;
        self.root = None;
        Ok(())
    }

    pub fn set_max_levels(&mut self, value: usize) {
        self.max_levels = value;
        self.root = None;
    }

    pub fn set_at(&mut self, index: usize, position: Vector4, value: T) -> Result<()> {
        let tuple = self
            .data
            .get_mut(index)
            .ok_or_else(|| CueMolError::invalid_argument("BSP index out of range"))?;
        tuple.position = [position.x, position.y, position.z];
        tuple.value = value;
        self.root = None;
        Ok(())
    }

    pub fn push(&mut self, position: Vector4, value: T) {
        self.data.push(Tuple {
            position: [position.x, position.y, position.z],
            value,
        });
        self.root = None;
    }

    pub fn build(&mut self) {
        if self.data.is_empty() {
            self.root = Some(Box::new(Node::leaf(
                0,
                0,
                Vector4::new3(1.0e100, 1.0e100, 1.0e100),
                Vector4::new3(-1.0e100, -1.0e100, -1.0e100),
            )));
            return;
        }
        let (minimum, maximum) = bounds(&self.data, 0, self.data.len());
        let data_len = self.data.len();
        self.root = Some(Box::new(build_node(
            &mut self.data,
            0,
            data_len,
            minimum,
            maximum,
            self.granularity,
            self.max_levels,
            1,
        )));
    }

    /// Find values strictly closer than `radius`. The output vector follows the
    /// reference contract: it grows to at least the tree size and the return
    /// value identifies how many leading entries were written.
    pub fn find_around(&self, position: Vector4, radius: f64, output: &mut Vec<T>) -> usize
    where
        T: Clone,
    {
        let Some(root) = self.root.as_deref() else {
            return 0;
        };
        let minimum = Vector4::new3(
            position.x - radius,
            position.y - radius,
            position.z - radius,
        );
        let maximum = Vector4::new3(
            position.x + radius,
            position.y + radius,
            position.z + radius,
        );
        let mut indices = Vec::new();
        collect_box(root, minimum, maximum, &mut indices);
        if indices.is_empty() {
            return 0;
        }

        if output.len() < self.data.len() {
            let filler = self.data[indices[0]].value.clone();
            output.resize(self.data.len(), filler);
        }
        let radius_squared = radius * radius;
        let mut count = 0;
        for index in indices {
            let tuple = &self.data[index];
            let point = Vector4::new3(tuple.position[0], tuple.position[1], tuple.position[2]);
            if (position - point).sqlen() < radius_squared {
                output[count] = tuple.value.clone();
                count += 1;
            }
        }
        count
    }

    pub fn values_around(&self, position: Vector4, radius: f64) -> Vec<T>
    where
        T: Clone,
    {
        let mut values = Vec::new();
        let count = self.find_around(position, radius, &mut values);
        values.truncate(count);
        values
    }

    pub fn coll_chk(&self, position: Vector4, radius: f64) -> bool {
        let Some(root) = self.root.as_deref() else {
            return false;
        };
        let minimum = Vector4::new3(
            position.x - radius,
            position.y - radius,
            position.z - radius,
        );
        let maximum = Vector4::new3(
            position.x + radius,
            position.y + radius,
            position.z + radius,
        );
        let mut indices = Vec::new();
        collect_box(root, minimum, maximum, &mut indices);
        let radius_squared = radius * radius;
        indices.into_iter().any(|index| {
            let tuple = &self.data[index];
            let point = Vector4::new3(tuple.position[0], tuple.position[1], tuple.position[2]);
            (position - point).sqlen() < radius_squared
        })
    }
}

impl<T: Default> BSPTree<T> {
    pub fn alloc(&mut self, size: usize) {
        self.root = None;
        self.data = (0..size)
            .map(|_| Tuple {
                position: [0.0; 3],
                value: T::default(),
            })
            .collect();
    }
}

#[allow(clippy::too_many_arguments)]
fn build_node<T>(
    data: &mut [Tuple<T>],
    start: usize,
    end: usize,
    minimum: Vector4,
    maximum: Vector4,
    granularity: usize,
    max_levels: usize,
    level: usize,
) -> Node {
    let mut node = Node::leaf(start, end, minimum, maximum);
    let count = end - start;
    if count < granularity || level > max_levels {
        return node;
    }

    let range = maximum - minimum;
    let direction = if range.x > range.y && range.x > range.z {
        0
    } else if range.y > range.x && range.y > range.z {
        1
    } else {
        2
    };
    data[start..end].sort_by(|left, right| {
        left.position[direction]
            .partial_cmp(&right.position[direction])
            .unwrap_or(Ordering::Equal)
    });

    let middle = start + count / 2;
    let divider = (data[middle].position[direction] + data[middle + 1].position[direction]) * 0.5;
    let mut lower_maximum = maximum;
    let mut upper_minimum = minimum;
    match direction {
        0 => {
            lower_maximum.x = divider;
            upper_minimum.x = divider;
        }
        1 => {
            lower_maximum.y = divider;
            upper_minimum.y = divider;
        }
        _ => {
            lower_maximum.z = divider;
            upper_minimum.z = divider;
        }
    }

    node.direction = direction;
    node.divider = divider;
    node.lower = Some(Box::new(build_node(
        data,
        start,
        middle + 1,
        minimum,
        lower_maximum,
        granularity,
        max_levels,
        level + 1,
    )));
    node.upper = Some(Box::new(build_node(
        data,
        middle + 1,
        end,
        upper_minimum,
        maximum,
        granularity,
        max_levels,
        level + 1,
    )));
    node
}

fn bounds<T>(data: &[Tuple<T>], start: usize, end: usize) -> (Vector4, Vector4) {
    let mut minimum = Vector4::new3(1.0e100, 1.0e100, 1.0e100);
    let mut maximum = Vector4::new3(-1.0e100, -1.0e100, -1.0e100);
    for tuple in &data[start..end] {
        minimum.x = minimum.x.min(tuple.position[0]);
        minimum.y = minimum.y.min(tuple.position[1]);
        minimum.z = minimum.z.min(tuple.position[2]);
        maximum.x = maximum.x.max(tuple.position[0]);
        maximum.y = maximum.y.max(tuple.position[1]);
        maximum.z = maximum.z.max(tuple.position[2]);
    }
    (minimum, maximum)
}

fn collect_box(node: &Node, minimum: Vector4, maximum: Vector4, output: &mut Vec<usize>) {
    if !boxes_intersect(minimum, maximum, node.minimum, node.maximum) {
        return;
    }
    if node.is_leaf() {
        output.extend(node.start..node.end);
        return;
    }
    if let Some(lower) = node.lower.as_deref() {
        collect_box(lower, minimum, maximum, output);
    }
    if let Some(upper) = node.upper.as_deref() {
        collect_box(upper, minimum, maximum, output);
    }
}

fn boxes_intersect(
    first_minimum: Vector4,
    first_maximum: Vector4,
    second_minimum: Vector4,
    second_maximum: Vector4,
) -> bool {
    overlaps(
        first_minimum.x,
        first_maximum.x,
        second_minimum.x,
        second_maximum.x,
    ) && overlaps(
        first_minimum.y,
        first_maximum.y,
        second_minimum.y,
        second_maximum.y,
    ) && overlaps(
        first_minimum.z,
        first_maximum.z,
        second_minimum.z,
        second_maximum.z,
    )
}

fn overlaps(
    first_minimum: f64,
    first_maximum: f64,
    second_minimum: f64,
    second_maximum: f64,
) -> bool {
    first_maximum >= second_minimum && second_maximum >= first_minimum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bsp_tree_finds_strict_radius_values_across_split_planes() {
        let mut tree = BSPTree::new();
        for index in 0..30 {
            tree.push(Vector4::new3(index as f64, 0.0, 0.0), index);
        }
        tree.build();
        assert!(tree.is_built());
        let values = tree.values_around(Vector4::new3(15.0, 0.0, 0.0), 2.0);
        assert_eq!(values, vec![14, 15, 16]);
        assert!(tree.coll_chk(Vector4::new3(15.0, 1.9, 0.0), 2.0));
        assert!(!tree.coll_chk(Vector4::new3(15.0, 2.0, 0.0), 2.0));
    }

    #[test]
    fn bsp_tree_alloc_set_clear_and_unbuilt_behavior_match_contract() {
        let mut tree = BSPTree::<usize>::new();
        tree.alloc(12);
        for index in 0..12 {
            tree.set_at(index, Vector4::new3(0.0, index as f64, 0.0), 100 + index)
                .unwrap();
        }
        assert!(!tree.coll_chk(Vector4::ZERO, 1.0));
        tree.build();
        let mut output = Vec::new();
        let count = tree.find_around(Vector4::new3(0.0, 5.0, 0.0), 1.1, &mut output);
        assert_eq!(&output[..count], &[104, 105, 106]);
        assert!(output.len() >= tree.len());
        tree.clear();
        assert!(tree.is_empty());
        assert!(!tree.is_built());
    }

    #[test]
    fn empty_and_tied_extent_trees_are_queryable() {
        let mut empty = BSPTree::<i32>::new();
        empty.build();
        assert!(empty.values_around(Vector4::ZERO, 1.0).is_empty());

        let mut tied = BSPTree::new();
        for index in 0..16 {
            let coordinate = (index % 4) as f64;
            tied.push(Vector4::new3(coordinate, coordinate, coordinate), index);
        }
        tied.build();
        assert!(tied.coll_chk(Vector4::new3(2.0, 2.0, 2.0), 0.1));
    }
}
