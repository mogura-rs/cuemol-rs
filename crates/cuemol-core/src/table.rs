use crate::{CueMolError, LString, Result};
use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// Ordered key/value table corresponding to `qlib::GenericTable`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenericTable<K, V> {
    values: BTreeMap<K, V>,
}

impl<K: Ord, V> GenericTable<K, V> {
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.values.contains_key(key)
    }

    /// Insert without replacing an existing entry.
    pub fn set(&mut self, key: K, value: V) -> bool {
        use std::collections::btree_map::Entry;
        match self.values.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    pub fn top_key(&self) -> Result<&K> {
        self.values
            .first_key_value()
            .map(|(key, _)| key)
            .ok_or_else(|| CueMolError::NotFound("table is empty".to_string()))
    }

    pub fn topkey(&self) -> Result<K>
    where
        K: Clone,
    {
        self.top_key().cloned()
    }

    pub fn get_ref(&self, key: &K) -> Option<&V> {
        self.values.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.values.get_mut(key)
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, K, V> {
        self.values.iter()
    }

    pub fn iter_mut(&mut self) -> std::collections::btree_map::IterMut<'_, K, V> {
        self.values.iter_mut()
    }

    pub fn keys(&self) -> std::collections::btree_map::Keys<'_, K, V> {
        self.values.keys()
    }

    pub fn values(&self) -> std::collections::btree_map::Values<'_, K, V> {
        self.values.values()
    }
}

impl<'a, K: Ord, V> IntoIterator for &'a GenericTable<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = std::collections::btree_map::Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}

/// Immediate-value table corresponding to `qlib::GenericValueTable`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenericValueTable<K, V> {
    table: GenericTable<K, V>,
}

impl<K: Ord, V> GenericValueTable<K, V> {
    pub fn new() -> Self {
        Self {
            table: GenericTable::new(),
        }
    }

    pub fn contains_value(&self, value: &V) -> bool
    where
        V: PartialEq,
    {
        self.table.values().any(|stored| stored == value)
    }

    /// Return a copied value, or `V::default()` when the key is absent.
    pub fn get(&self, key: &K) -> V
    where
        V: Clone + Default,
    {
        self.table.get_ref(key).cloned().unwrap_or_default()
    }

    pub fn force_set(&mut self, key: K, value: V) {
        self.table.values.insert(key, value);
    }

    pub fn remove(&mut self, key: &K) -> bool {
        self.table.values.remove(key).is_some()
    }
}

impl<K, V> Deref for GenericValueTable<K, V> {
    type Target = GenericTable<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

impl<K, V> DerefMut for GenericValueTable<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
    }
}

/// Owning pointer table corresponding to `qlib::GenericPtrTable`.
/// Removing an entry transfers ownership to the caller.
pub struct GenericPtrTable<K, V> {
    values: BTreeMap<K, Box<V>>,
}

impl<K, V> Default for GenericPtrTable<K, V> {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
}

impl<K: Ord, V> GenericPtrTable<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.values.contains_key(key)
    }

    pub fn contains_value(&self, value: &V) -> bool {
        self.values
            .values()
            .any(|stored| std::ptr::eq(stored.as_ref(), value))
    }

    /// Insert a boxed value, returning it unchanged if the key already exists.
    pub fn set_box(&mut self, key: K, value: Box<V>) -> std::result::Result<(), Box<V>> {
        use std::collections::btree_map::Entry;
        match self.values.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(())
            }
            Entry::Occupied(_) => Err(value),
        }
    }

    /// Convenience insertion. A rejected duplicate value is dropped.
    pub fn set(&mut self, key: K, value: V) -> bool {
        self.set_box(key, Box::new(value)).is_ok()
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.values.get(key).map(Box::as_ref)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.values.get_mut(key).map(Box::as_mut)
    }

    /// Replace an entry, dropping the previous owned value.
    pub fn force_set_box(&mut self, key: K, value: Box<V>) {
        self.values.insert(key, value);
    }

    pub fn force_set(&mut self, key: K, value: V) {
        self.force_set_box(key, Box::new(value));
    }

    /// Remove and transfer ownership of the value.
    pub fn remove(&mut self, key: &K) -> Option<Box<V>> {
        self.values.remove(key)
    }

    pub fn remove_and_delete(&mut self, key: &K) -> bool {
        self.values.remove(key).is_some()
    }

    pub fn clear_and_delete(&mut self) {
        self.values.clear();
    }

    pub fn top_key(&self) -> Result<&K> {
        self.values
            .first_key_value()
            .map(|(key, _)| key)
            .ok_or_else(|| CueMolError::NotFound("table is empty".to_string()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.values.iter().map(|(key, value)| (key, value.as_ref()))
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for GenericPtrTable<K, V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_map().entries(self.values.iter()).finish()
    }
}

impl<K: Clone + Ord, V: Clone> Clone for GenericPtrTable<K, V> {
    fn clone(&self) -> Self {
        Self {
            values: self
                .values
                .iter()
                .map(|(key, value)| (key.clone(), Box::new((**value).clone())))
                .collect(),
        }
    }
}

pub type MapTable<V> = GenericValueTable<LString, V>;
pub type HashTable<V> = GenericValueTable<LString, V>;
pub type MapPtrTable<V> = GenericPtrTable<LString, V>;
pub type HashPtrTable<V> = GenericPtrTable<LString, V>;

/// Owning integer-key table with monotonic key allocation.
#[derive(Debug, Clone)]
pub struct IndexedTable<V> {
    table: GenericPtrTable<i32, V>,
    next_index: i32,
}

impl<V> Default for IndexedTable<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> IndexedTable<V> {
    pub fn new() -> Self {
        Self {
            table: GenericPtrTable::new(),
            next_index: 0,
        }
    }

    pub fn put(&mut self, value: V) -> i32 {
        self.put_box(Box::new(value))
    }

    pub fn put_box(&mut self, value: Box<V>) -> i32 {
        let key = self.next_index;
        self.next_index = self
            .next_index
            .checked_add(1)
            .expect("indexed table overflow");
        if self.table.set_box(key, value).is_err() {
            unreachable!("generated indexed table key is unique");
        }
        key
    }

    pub fn next_index(&self) -> i32 {
        self.next_index
    }
}

impl<V> Deref for IndexedTable<V> {
    type Target = GenericPtrTable<i32, V>;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

impl<V> DerefMut for IndexedTable<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn value_table_matches_insert_default_force_and_order_contracts() {
        let mut table = MapTable::<i32>::new();
        assert!(table.set("beta".into(), 2));
        assert!(table.set("alpha".into(), 1));
        assert!(!table.set("alpha".into(), 99));
        assert_eq!(table.get(&LString::from("alpha")), 1);
        assert_eq!(table.get(&LString::from("missing")), 0);
        assert_eq!(table.topkey().unwrap().as_str(), "alpha");
        assert!(table.contains_key(&LString::from("beta")));
        assert!(table.contains_value(&2));
        table.force_set("alpha".into(), 7);
        assert_eq!(table.get(&LString::from("alpha")), 7);
        assert!(table.remove(&LString::from("beta")));
        assert!(!table.remove(&LString::from("beta")));
    }

    #[derive(Debug)]
    struct DropValue {
        value: i32,
        drops: Rc<Cell<usize>>,
    }

    impl Drop for DropValue {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn pointer_table_transfers_or_deletes_owned_values() {
        let drops = Rc::new(Cell::new(0));
        let mut table = MapPtrTable::new();
        assert!(table.set(
            "a".into(),
            DropValue {
                value: 1,
                drops: drops.clone(),
            },
        ));
        let rejected = table
            .set_box(
                "a".into(),
                Box::new(DropValue {
                    value: 2,
                    drops: drops.clone(),
                }),
            )
            .unwrap_err();
        assert_eq!(rejected.value, 2);
        drop(rejected);
        assert_eq!(drops.get(), 1);

        let removed = table.remove(&LString::from("a")).unwrap();
        assert_eq!(removed.value, 1);
        assert_eq!(drops.get(), 1);
        drop(removed);
        assert_eq!(drops.get(), 2);

        table.set(
            "b".into(),
            DropValue {
                value: 3,
                drops: drops.clone(),
            },
        );
        assert!(table.remove_and_delete(&LString::from("b")));
        assert_eq!(drops.get(), 3);
    }

    #[test]
    fn indexed_table_never_reuses_removed_keys() {
        let mut table = IndexedTable::new();
        assert_eq!(table.put("zero".to_string()), 0);
        assert_eq!(table.put("one".to_string()), 1);
        assert_eq!(table.remove(&0).unwrap().as_str(), "zero");
        assert_eq!(table.put("two".to_string()), 2);
        assert_eq!(table.get(&1).map(String::as_str), Some("one"));
        assert_eq!(table.get(&2).map(String::as_str), Some("two"));
    }
}
