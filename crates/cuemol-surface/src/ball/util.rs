//! Containers standing in for BALL's `HashGrid3`, `HashSet<Index>` and pointer graphs.

use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

macro_rules! define_id {
    ($($name:ident),* $(,)?) => {$(
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub(super) struct $name(pub(super) u32);
    )*};
}
pub(super) use define_id;

pub(super) trait ArenaId: Copy {
    fn from_index(index: usize) -> Self;
    fn index(self) -> usize;
}

macro_rules! impl_arena_id {
    ($($name:ident),* $(,)?) => {$(
        impl $crate::ball::util::ArenaId for $name {
            #[inline]
            fn from_index(index: usize) -> Self {
                Self(index as u32)
            }
            #[inline]
            fn index(self) -> usize {
                self.0 as usize
            }
        }
    )*};
}
pub(super) use impl_arena_id;

/// Append-only storage; deleted C++ objects become unreachable entries.
pub(super) struct Arena<I, T> {
    items: Vec<T>,
    marker: PhantomData<I>,
}

impl<I: ArenaId, T> Arena<I, T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            marker: PhantomData,
        }
    }
    pub fn push(&mut self, item: T) -> I {
        self.items.push(item);
        I::from_index(self.items.len() - 1)
    }
}

impl<I: ArenaId, T> Index<I> for Arena<I, T> {
    type Output = T;
    #[inline]
    fn index(&self, id: I) -> &T {
        &self.items[id.index()]
    }
}

impl<I: ArenaId, T> IndexMut<I> for Arena<I, T> {
    #[inline]
    fn index_mut(&mut self, id: I) -> &mut T {
        &mut self.items[id.index()]
    }
}

/// Insertion-ordered set used where BALL iterates a pointer-keyed `HashSet`.
#[derive(Clone, Debug, Default)]
pub(super) struct OrderedSet<T> {
    items: Vec<T>,
}

impl<T: Copy + PartialEq> OrderedSet<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    pub fn insert(&mut self, item: T) {
        if !self.items.contains(&item) {
            self.items.push(item);
        }
    }
    pub fn erase(&mut self, item: T) {
        if let Some(pos) = self.items.iter().position(|&x| x == item) {
            self.items.remove(pos);
        }
    }
    pub fn has(&self, item: T) -> bool {
        self.items.contains(&item)
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn items(&self) -> Vec<T> {
        self.items.clone()
    }
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.items.iter().copied()
    }
}

/// BALL `getNextPrime`.
fn next_prime(mut l: u32) -> u32 {
    if l <= 3 {
        return 3;
    }
    if l & 1 == 0 {
        l += 1;
    }
    let sqr = (l as f64).sqrt() as u32 + 1;
    loop {
        let mut div = 3;
        while div <= sqr && l % div != 0 {
            div += 2;
        }
        if div > sqr {
            return l;
        }
        l += 2;
    }
}

/// BALL `HashSet<Index>`: identity hash, chained buckets, newest node first.
pub(super) struct BallIntSet {
    size: u32,
    capacity: u32,
    buckets: Vec<Vec<i32>>,
}

impl BallIntSet {
    pub fn new() -> Self {
        Self {
            size: 0,
            capacity: 4,
            buckets: vec![Vec::new(); 3],
        }
    }
    fn bucket(&self, key: i32) -> usize {
        ((key as u32) % (self.buckets.len() as u32)) as usize
    }
    pub fn insert(&mut self, key: i32) {
        if self.buckets[self.bucket(key)].contains(&key) {
            return;
        }
        if self.size >= self.capacity {
            self.capacity = next_prime((self.buckets.len() as u32) << 1);
            let old =
                std::mem::replace(&mut self.buckets, vec![Vec::new(); self.capacity as usize]);
            for chain in old {
                // chains are stored oldest first; BALL walks them newest first
                for &value in chain.iter().rev() {
                    let b = self.bucket(value);
                    self.buckets[b].push(value);
                }
            }
        }
        let b = self.bucket(key);
        self.buckets[b].push(key);
        self.size += 1;
    }
    pub fn iter(&self) -> impl Iterator<Item = i32> + '_ {
        self.buckets
            .iter()
            .flat_map(|chain| chain.iter().rev().copied())
    }
}

/// BALL `HashGrid3` with single-precision boxes and BALL's neighbour visiting order.
pub(super) struct HashGrid<T> {
    origin: [f32; 3],
    unit: [f32; 3],
    dims: [u32; 3],
    boxes: std::collections::HashMap<[u32; 3], Vec<T>>,
}

const NEIGHBOUR_TABLE: [[i32; 3]; 27] = [
    [0, 0, 0],
    [0, 0, -1],
    [0, 0, 1],
    [0, -1, -1],
    [0, -1, 0],
    [0, -1, 1],
    [0, 1, -1],
    [0, 1, 0],
    [0, 1, 1],
    [-1, 0, -1],
    [-1, 0, 0],
    [-1, 0, 1],
    [-1, -1, -1],
    [-1, -1, 0],
    [-1, -1, 1],
    [-1, 1, -1],
    [-1, 1, 0],
    [-1, 1, 1],
    [1, 0, -1],
    [1, 0, 0],
    [1, 0, 1],
    [1, -1, -1],
    [1, -1, 0],
    [1, -1, 1],
    [1, 1, -1],
    [1, 1, 0],
    [1, 1, 1],
];

/// `Maths::floor<float>` (EPSILON-dependent) converted to `Index`.
fn ball_floor_f32(t: f32) -> i32 {
    let eps = super::geom::eps();
    let value = if (t - 0.0) as f64 >= eps {
        t
    } else if (t - (t as i64 as f32)).abs() as f64 >= eps {
        t - 1.0
    } else {
        t
    };
    value as i64 as i32
}

impl<T: Copy> HashGrid<T> {
    pub fn new(origin: [f32; 3], unit: [f32; 3], dims: [u32; 3]) -> Self {
        Self {
            origin,
            unit,
            dims,
            boxes: Default::default(),
        }
    }
    fn box_at(&self, x: u32, y: u32, z: u32) -> Option<[u32; 3]> {
        (x < self.dims[0] && y < self.dims[1] && z < self.dims[2]).then_some([x, y, z])
    }
    fn box_indices(&self, pos: [f32; 3]) -> [u32; 3] {
        let mut r = [0u32; 3];
        for i in 0..3 {
            r[i] = ball_floor_f32((pos[i] - self.origin[i]) / self.unit[i]) as u32;
        }
        r
    }
    pub fn insert(&mut self, pos: [f32; 3], item: T) {
        let [x, y, z] = self.box_indices(pos);
        if let Some(b) = self.box_at(x, y, z) {
            self.boxes.entry(b).or_default().push(item);
        }
    }
    /// Items of the box containing `pos` and its neighbours in `HashGridBox3` order;
    /// `None` if `pos` lies outside the grid.
    pub fn neighbourhood(&self, pos: [f32; 3]) -> Option<Vec<T>> {
        let [x, y, z] = self.box_indices(pos);
        self.box_at(x, y, z)?;
        let mut items = Vec::new();
        for offset in NEIGHBOUR_TABLE {
            let b = self.box_at(
                x.wrapping_add(offset[0] as u32),
                y.wrapping_add(offset[1] as u32),
                z.wrapping_add(offset[2] as u32),
            );
            if let Some(entries) = b.and_then(|key| self.boxes.get(&key)) {
                items.extend(entries.iter().rev().copied());
            }
        }
        Some(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_set_matches_ball_bucket_order() {
        let mut set = BallIntSet::new();
        for k in [5, 1, 9, 3] {
            set.insert(k);
        }
        // 3 buckets: 9,3 -> 0 (newest first), 1 -> 1, 5 -> 2
        assert_eq!(set.iter().collect::<Vec<_>>(), vec![3, 9, 1, 5]);
        set.insert(9);
        set.insert(7);
        // rehash to 7 buckets
        assert_eq!(set.iter().collect::<Vec<_>>(), vec![7, 1, 9, 3, 5]);
        assert_eq!(next_prime(6), 7);
        assert_eq!(next_prime(14), 17);
    }

    #[test]
    fn grid_visits_centre_box_first_and_newest_items_first() {
        let mut grid = HashGrid::new([0.0; 3], [1.0; 3], [3, 3, 3]);
        grid.insert([1.5, 1.5, 1.5], 1);
        grid.insert([1.5, 1.5, 1.5], 2);
        grid.insert([0.5, 1.5, 1.5], 3);
        grid.insert([1.5, 1.5, 0.5], 4);
        assert_eq!(
            grid.neighbourhood([1.2, 1.7, 1.1]).unwrap(),
            vec![2, 1, 4, 3]
        );
        assert!(grid.neighbourhood([-0.5, 1.0, 1.0]).is_none());
        assert_eq!(ball_floor_f32(-0.5), -1);
        assert_eq!(ball_floor_f32(2.75), 2);
    }
}
