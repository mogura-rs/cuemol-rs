use crate::{CueMolError, Result};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntRange {
    pub start: i32,
    pub end: i32,
}

impl IntRange {
    pub fn new(start: i32, end: i32) -> Self {
        if start <= end {
            Self { start, end }
        } else {
            Self {
                start: end,
                end: start,
            }
        }
    }

    pub fn contains(self, value: i32) -> bool {
        self.start <= value && value <= self.end
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RangeSet {
    ranges: Vec<IntRange>,
}

impl RangeSet {
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    pub fn ranges(&self) -> &[IntRange] {
        &self.ranges
    }

    pub fn insert_value(&mut self, value: i32) {
        self.insert_range(IntRange::new(value, value));
    }

    pub fn append_int(&mut self, value: i32) {
        self.insert_value(value);
    }

    pub fn insert_range(&mut self, range: IntRange) {
        self.ranges.push(range);
        self.normalize();
    }

    pub fn append_interval(&mut self, start: i32, end: i32) -> Result<()> {
        if end < start {
            return Err(CueMolError::invalid_argument(format!(
                "invalid range: start({start}) > end({end})"
            )));
        }
        if start < end {
            self.insert_range(IntRange::new(start, end - 1));
        }
        Ok(())
    }

    pub fn append(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for range in &other.ranges {
            result.insert_range(*range);
        }
        result
    }

    pub fn scr_append(&self, other: &Self) -> Self {
        self.append(other)
    }

    pub fn scr_append_int(&self, start: i32, end: i32) -> Result<Self> {
        let mut result = self.clone();
        result.append_interval(start, end)?;
        Ok(result)
    }

    pub fn contains(&self, value: i32) -> bool {
        self.ranges.iter().any(|range| range.contains(value))
    }

    pub fn contains_int(&self, value: i32) -> bool {
        self.contains(value)
    }

    pub fn contains_range(&self, start: i32, end: i32) -> bool {
        if end < start {
            return false;
        }
        self.ranges.iter().any(|range| {
            if start == end {
                range.start <= start && start <= range.end.saturating_add(1)
            } else {
                range.start <= start && end - 1 <= range.end
            }
        })
    }

    pub fn remove_int(&mut self, value: i32) -> bool {
        let mut changed = false;
        let mut next = Vec::new();
        for range in self.ranges.drain(..) {
            if !range.contains(value) {
                next.push(range);
                continue;
            }
            changed = true;
            if range.start < value {
                next.push(IntRange::new(range.start, value - 1));
            }
            if value < range.end {
                next.push(IntRange::new(value + 1, range.end));
            }
        }
        self.ranges = next;
        changed
    }

    pub fn remove_interval(&mut self, start: i32, end: i32) -> Result<bool> {
        if end < start {
            return Err(CueMolError::invalid_argument(format!(
                "invalid range: start({start}) > end({end})"
            )));
        }
        if start == end {
            return Ok(false);
        }
        let remove_end = end - 1;
        let mut changed = false;
        let mut next = Vec::new();
        for range in self.ranges.drain(..) {
            if range.end < start || remove_end < range.start {
                next.push(range);
                continue;
            }
            changed = true;
            if range.start < start {
                next.push(IntRange::new(range.start, start - 1));
            }
            if remove_end < range.end {
                next.push(IntRange::new(end, range.end));
            }
        }
        self.ranges = next;
        Ok(changed)
    }

    pub fn remove(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for range in &other.ranges {
            let end = range.end.saturating_add(1);
            let _ = result.remove_interval(range.start, end);
        }
        result
    }

    pub fn scr_remove(&self, other: &Self) -> Self {
        self.remove(other)
    }

    pub fn scr_remove_int(&self, start: i32, end: i32) -> Result<Self> {
        let mut result = self.clone();
        result.remove_interval(start, end)?;
        Ok(result)
    }

    pub fn negate(&self) -> Self {
        let universe_end = i32::MAX - 1;
        let mut result = Self::new();
        let mut start = i32::MIN;
        for range in &self.ranges {
            if start < range.start {
                result.insert_range(IntRange::new(start, range.start - 1));
            }
            start = range.end.saturating_add(1);
        }
        if start <= universe_end {
            result.insert_range(IntRange::new(start, universe_end));
        }
        result
    }

    pub fn clear(&mut self) {
        self.ranges.clear();
    }

    pub fn from_string(&mut self, source: &str) -> Result<()> {
        let mut parsed = Self::new();
        if source.trim().is_empty() {
            self.clear();
            return Ok(());
        }
        for token in source.split(',') {
            let token = token.trim();
            let (start, end) = if let Some((start, end)) = token.split_once(':') {
                let start = parse_int(start)?;
                let end = parse_int(end)?;
                if end < start {
                    return Err(CueMolError::invalid_argument("invalid range string"));
                }
                (start, end)
            } else {
                let value = parse_int(token)?;
                (value, value)
            };
            parsed.insert_range(IntRange::new(start, end));
        }
        *self = parsed;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    fn normalize(&mut self) {
        self.ranges.sort_by_key(|range| (range.start, range.end));
        let mut merged: Vec<IntRange> = Vec::new();
        for range in self.ranges.drain(..) {
            if let Some(last) = merged.last_mut() {
                if range.start <= last.end.saturating_add(1) {
                    last.end = last.end.max(range.end);
                    continue;
                }
            }
            merged.push(range);
        }
        self.ranges = merged;
    }
}

fn parse_int(value: &str) -> Result<i32> {
    value
        .trim()
        .parse()
        .map_err(|_| CueMolError::invalid_argument("invalid range string"))
}

impl fmt::Display for RangeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = self
            .ranges
            .iter()
            .map(|range| {
                if range.start == range.end {
                    range.start.to_string()
                } else {
                    format!("{}:{}", range.start, range.end)
                }
            })
            .collect::<Vec<_>>();
        write!(f, "{}", parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_adjacent_integer_ranges() {
        let mut ranges = RangeSet::new();
        for value in [1, 3, 2, 5, 4] {
            ranges.insert_value(value);
        }
        assert_eq!(ranges.to_string(), "1:5");
    }

    #[test]
    fn script_range_operations_use_exclusive_endpoints() {
        let mut ranges = RangeSet::new();
        ranges.from_string("1:3,5").unwrap();
        assert_eq!(ranges.to_string(), "1:3,5");
        assert!(ranges.contains_range(1, 4));
        assert!(ranges.contains_range(4, 4));
        assert!(!ranges.contains_range(3, 6));

        let appended = ranges.scr_append_int(4, 5).unwrap();
        assert_eq!(ranges.to_string(), "1:3,5");
        assert_eq!(appended.to_string(), "1:5");
        let removed = appended.scr_remove_int(2, 5).unwrap();
        assert_eq!(removed.to_string(), "1,5");

        let mut invalid_target = ranges.clone();
        assert!(invalid_target.from_string("9:2").is_err());
        assert_eq!(invalid_target, ranges);
        invalid_target.clear();
        assert!(invalid_target.is_empty());
    }

    #[test]
    fn negate_matches_reference_integer_universe() {
        let mut ranges = RangeSet::new();
        ranges.from_string("0:2").unwrap();
        let negated = ranges.negate();
        assert!(negated.contains(-1));
        assert!(negated.contains(3));
        assert!(!negated.contains(0));
        assert!(!negated.contains(2));
        assert!(!negated.contains(i32::MAX));
    }
}
