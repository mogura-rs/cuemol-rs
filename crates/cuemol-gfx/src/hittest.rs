use cuemol_core::Uid;
use std::collections::BTreeMap;

pub trait RawHitData {
    fn data_size(&self, renderer_id: Uid) -> usize;
    fn data_at(&self, renderer_id: Uid, index: usize, sub_index: usize) -> Option<i32>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct HitRecord {
    pub renderer_id: Uid,
    pub depth: f64,
    pub names: Vec<i32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HittestContext {
    records: Vec<HitRecord>,
}

impl HittestContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_hit(&mut self, renderer_id: Uid, depth: f64, names: impl Into<Vec<i32>>) {
        self.records.push(HitRecord {
            renderer_id,
            depth,
            names: names.into(),
        });
    }

    pub fn records(&self) -> &[HitRecord] {
        &self.records
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitEntry {
    pub renderer_id: Uid,
    indices: Vec<usize>,
    data: Vec<i32>,
}

impl HitEntry {
    pub fn hit_count(&self) -> usize {
        self.indices.len()
    }

    fn push(&mut self, names: &[i32]) {
        self.indices.push(self.data.len());
        self.data.extend_from_slice(names);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitData {
    entries: BTreeMap<Uid, HitEntry>,
    nearest_renderer_id: Option<Uid>,
}

impl HitData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.nearest_renderer_id = None;
    }

    pub fn create_nearest(&mut self, context: &HittestContext) {
        self.clear();
        let Some(record) = context
            .records()
            .iter()
            .filter(|record| record.depth.is_finite())
            .min_by(|left, right| left.depth.total_cmp(&right.depth))
        else {
            return;
        };
        self.entry_mut(record.renderer_id).push(&record.names);
        self.nearest_renderer_id = Some(record.renderer_id);
    }

    pub fn create_all(&mut self, context: &HittestContext) {
        self.clear();
        let mut nearest: Option<(f64, Uid)> = None;
        for record in context
            .records()
            .iter()
            .filter(|record| record.depth.is_finite())
        {
            if nearest.is_none_or(|(depth, _)| record.depth < depth) {
                nearest = Some((record.depth, record.renderer_id));
            }
            self.entry_mut(record.renderer_id).push(&record.names);
        }
        self.nearest_renderer_id = nearest.map(|(_, renderer_id)| renderer_id);
    }

    pub fn nearest_renderer_id(&self) -> Option<Uid> {
        self.nearest_renderer_id
    }

    pub fn renderer_count(&self) -> usize {
        self.entries.len()
    }

    pub fn renderer_ids(&self) -> Vec<Uid> {
        self.entries.keys().copied().collect()
    }

    pub fn entry(&self, renderer_id: Uid) -> Option<&HitEntry> {
        self.entries.get(&renderer_id)
    }

    fn entry_mut(&mut self, renderer_id: Uid) -> &mut HitEntry {
        self.entries.entry(renderer_id).or_insert(HitEntry {
            renderer_id,
            indices: Vec::new(),
            data: Vec::new(),
        })
    }
}

impl RawHitData for HitData {
    fn data_size(&self, renderer_id: Uid) -> usize {
        self.entry(renderer_id).map_or(0, HitEntry::hit_count)
    }

    fn data_at(&self, renderer_id: Uid, index: usize, sub_index: usize) -> Option<i32> {
        let entry = self.entry(renderer_id)?;
        let start = *entry.indices.get(index)?;
        let end = entry
            .indices
            .get(index + 1)
            .copied()
            .unwrap_or(entry.data.len());
        (start + sub_index < end).then(|| entry.data[start + sub_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_hit_keeps_only_the_lowest_depth_record() {
        let mut context = HittestContext::new();
        context.add_hit(Uid::new(2), 0.8, vec![20, 21]);
        context.add_hit(Uid::new(1), 0.2, vec![10]);
        context.add_hit(Uid::new(3), f64::NAN, vec![30]);
        let mut data = HitData::new();
        data.create_nearest(&context);

        assert_eq!(data.nearest_renderer_id(), Some(Uid::new(1)));
        assert_eq!(data.renderer_ids(), vec![Uid::new(1)]);
        assert_eq!(data.data_size(Uid::new(1)), 1);
        assert_eq!(data.data_at(Uid::new(1), 0, 0), Some(10));
    }

    #[test]
    fn all_hits_group_names_by_renderer_and_preserve_hit_boundaries() {
        let mut context = HittestContext::new();
        context.add_hit(Uid::new(2), 0.8, vec![20]);
        context.add_hit(Uid::new(1), 0.3, vec![10, 11]);
        context.add_hit(Uid::new(1), 0.4, vec![12]);
        let mut data = HitData::new();
        data.create_all(&context);

        assert_eq!(data.nearest_renderer_id(), Some(Uid::new(1)));
        assert_eq!(data.renderer_ids(), vec![Uid::new(1), Uid::new(2)]);
        assert_eq!(data.data_size(Uid::new(1)), 2);
        assert_eq!(data.data_at(Uid::new(1), 0, 0), Some(10));
        assert_eq!(data.data_at(Uid::new(1), 0, 1), Some(11));
        assert_eq!(data.data_at(Uid::new(1), 0, 2), None);
        assert_eq!(data.data_at(Uid::new(1), 1, 0), Some(12));
    }
}
