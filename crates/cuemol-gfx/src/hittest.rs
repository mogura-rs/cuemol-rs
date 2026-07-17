use cuemol_core::{CueMolError, Result, Uid, Vector4};
use std::collections::BTreeMap;

use crate::DisplayContext;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitPoint {
    pub position: Vector4,
    pub id: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HittestList {
    points: Vec<HitPoint>,
}

impl HittestList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn draw_point_hit(&mut self, id: i32, mut position: Vector4) {
        position.w = 1.0;
        self.points.push(HitPoint { position, id });
    }

    pub fn points(&self) -> &[HitPoint] {
        &self.points
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    pub fn is_display_list(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HittestContext {
    records: Vec<HitRecord>,
    context: DisplayContext,
    names: Vec<i32>,
    current_renderer_id: Option<Uid>,
}

impl Default for HittestContext {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            context: DisplayContext::new(),
            names: vec![-1],
            current_renderer_id: None,
        }
    }
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

    pub fn context(&self) -> &DisplayContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut DisplayContext {
        &mut self.context
    }

    pub fn start_hit(&mut self, renderer_id: Uid) {
        self.current_renderer_id = Some(renderer_id);
    }

    pub fn end_hit(&mut self) {
        self.current_renderer_id = None;
    }

    pub fn load_name(&mut self, name: i32) {
        self.names[0] = name;
    }

    pub fn push_name(&mut self, name: i32) {
        self.names.insert(0, name);
    }

    pub fn pop_name(&mut self) -> Result<()> {
        if self.names.len() <= 1 {
            return Err(CueMolError::invalid_argument(
                "HittestContext name stack underflow",
            ));
        }
        self.names.remove(0);
        Ok(())
    }

    pub fn name_stack(&self) -> &[i32] {
        &self.names
    }

    pub fn call_display_list(&mut self, list: &HittestList) {
        let Some(renderer_id) = self.current_renderer_id else {
            return;
        };
        let transform = self
            .context
            .projection_matrix()
            .mul_mat(self.context.model_view_matrix());
        for point in list.points() {
            let projected = transform.mul_vec(point.position);
            if projected.w.abs() < f64::EPSILON {
                continue;
            }
            let projected = projected.scale(1.0 / projected.w);
            if !(-1.0..1.0).contains(&projected.x)
                || !(-1.0..1.0).contains(&projected.y)
                || !(-1.0..1.0).contains(&projected.z)
            {
                continue;
            }
            let mut names = self.names.iter().skip(1).copied().collect::<Vec<_>>();
            names.push(point.id);
            self.add_hit(renderer_id, projected.z, names);
        }
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

    #[test]
    fn cpu_context_projects_points_and_preserves_reference_name_order() {
        let mut list = HittestList::new();
        list.draw_point_hit(101, Vector4::new3(0.25, 0.5, -0.25));
        list.draw_point_hit(102, Vector4::new3(2.0, 0.0, 0.0));

        let mut context = HittestContext::new();
        context.start_hit(Uid::new(9));
        context.load_name(10);
        context.push_name(20);
        context.call_display_list(&list);
        assert_eq!(context.records().len(), 1);
        assert_eq!(context.records()[0].renderer_id, Uid::new(9));
        assert_eq!(context.records()[0].depth, -0.25);
        assert_eq!(context.records()[0].names, vec![10, 101]);
        context.pop_name().unwrap();
        assert!(context.pop_name().is_err());
    }

    #[test]
    fn cpu_context_applies_model_view_before_projection() {
        let mut list = HittestList::new();
        list.draw_point_hit(7, Vector4::ZERO);
        let mut context = HittestContext::new();
        context.start_hit(Uid::new(1));
        context
            .context_mut()
            .translate(Vector4::new3(0.0, 0.0, 0.75));
        context.call_display_list(&list);
        assert_eq!(context.records()[0].depth, 0.75);
    }
}
