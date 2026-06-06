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

    pub fn contains(&self, value: i32) -> bool {
        self.ranges.iter().any(|range| range.contains(value))
    }

    pub fn contains_int(&self, value: i32) -> bool {
        self.contains(value)
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
}
