use std::collections::{BTreeMap, BTreeSet};

/// Apply one recorded value change without replacing an unrelated current value.
pub fn apply_value_change<T: Clone + PartialEq>(current: &mut T, from: &T, to: &T) {
    if from != to {
        current.clone_from(to);
    }
}

pub fn apply_map_changes<K: Clone + Ord, V: Clone + PartialEq>(
    current: &mut BTreeMap<K, V>,
    from: &BTreeMap<K, V>,
    to: &BTreeMap<K, V>,
) {
    for key in from.keys().filter(|key| !to.contains_key(*key)) {
        current.remove(key);
    }
    for (key, value) in to {
        if from.get(key) != Some(value) {
            current.insert(key.clone(), value.clone());
        }
    }
}

pub fn apply_map_changes_with<K: Clone + Ord, V: Clone + PartialEq>(
    current: &mut BTreeMap<K, V>,
    from: &BTreeMap<K, V>,
    to: &BTreeMap<K, V>,
    mut update: impl FnMut(&mut V, &V, &V),
) {
    for key in from.keys().filter(|key| !to.contains_key(*key)) {
        current.remove(key);
    }
    for (key, next) in to {
        match from.get(key) {
            Some(previous) if previous != next => {
                if let Some(value) = current.get_mut(key) {
                    update(value, previous, next);
                }
            }
            None => {
                current.insert(key.clone(), next.clone());
            }
            _ => {}
        }
    }
}

/// Apply changes to keyed entries, retaining unrelated entries and their order.
pub fn apply_sequence_changes<T: Clone + PartialEq, K: Ord + Clone>(
    current: &mut Vec<T>,
    from: &[T],
    to: &[T],
    key: impl Fn(&T) -> K,
    mut update: impl FnMut(&mut T, &T, &T),
) {
    if from == to {
        return;
    }
    let previous: BTreeMap<_, _> = from.iter().map(|value| (key(value), value)).collect();
    let desired: BTreeMap<_, _> = to.iter().map(|value| (key(value), value)).collect();
    current.retain(|value| {
        let id = key(value);
        !previous.contains_key(&id) || desired.contains_key(&id)
    });
    for value in current.iter_mut() {
        let id = key(value);
        if let (Some(before), Some(after)) = (previous.get(&id), desired.get(&id)) {
            if before != after {
                update(value, before, after);
            }
        }
    }
    let mut current_keys: BTreeSet<_> = current.iter().map(&key).collect();
    for (index, value) in to.iter().enumerate() {
        let id = key(value);
        if !previous.contains_key(&id) && current_keys.insert(id) {
            current.insert(index.min(current.len()), value.clone());
        }
    }
    let before_order: Vec<_> = from
        .iter()
        .map(&key)
        .filter(|id| desired.contains_key(id))
        .collect();
    let after_order: Vec<_> = to
        .iter()
        .map(&key)
        .filter(|id| previous.contains_key(id))
        .collect();
    if before_order != after_order {
        let slots: Vec<_> = current
            .iter()
            .enumerate()
            .filter_map(|(index, value)| desired.contains_key(&key(value)).then_some(index))
            .collect();
        let values: BTreeMap<_, _> = slots
            .iter()
            .map(|index| (key(&current[*index]), current[*index].clone()))
            .collect();
        let ordered = to.iter().filter_map(|value| values.get(&key(value)));
        for (index, value) in slots.into_iter().zip(ordered) {
            current[index] = value.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_changes_preserve_later_unrelated_edits() {
        let from = BTreeMap::from([(1, 10), (2, 20)]);
        let to = BTreeMap::from([(1, 11), (2, 20), (3, 30)]);
        let mut current = BTreeMap::from([(1, 10), (2, 22), (4, 40)]);
        apply_map_changes(&mut current, &from, &to);
        assert_eq!(
            current,
            BTreeMap::from([(1, 11), (2, 22), (3, 30), (4, 40)])
        );
        apply_map_changes(&mut current, &to, &from);
        assert_eq!(current, BTreeMap::from([(1, 10), (2, 22), (4, 40)]));
        current.remove(&1);
        apply_map_changes(&mut current, &from, &to);
        assert_eq!(current[&1], 11);
    }

    #[test]
    fn keyed_changes_restore_membership_and_order_without_reviving_unrelated_deletions() {
        let from = vec![(1, 10), (2, 20), (3, 30)];
        let to = vec![(3, 31), (1, 10), (2, 20), (4, 40)];
        let mut current = vec![(1, 11), (3, 30), (5, 50)];
        apply_sequence_changes(
            &mut current,
            &from,
            &to,
            |item| item.0,
            |item, before, after| {
                apply_value_change(&mut item.1, &before.1, &after.1);
            },
        );
        assert_eq!(current, vec![(3, 31), (1, 11), (5, 50), (4, 40)]);
        apply_sequence_changes(
            &mut current,
            &to,
            &from,
            |item| item.0,
            |item, before, after| {
                apply_value_change(&mut item.1, &before.1, &after.1);
            },
        );
        assert_eq!(current, vec![(1, 11), (3, 30), (5, 50)]);
    }
}
