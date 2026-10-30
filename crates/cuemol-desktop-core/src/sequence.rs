use crate::{GuiSeqChain, GuiSeqResidue};
use std::collections::BTreeSet;

pub fn sequence_residue_number(index: &str) -> Option<i32> {
    let index = index.trim_start();
    let sign = usize::from(index.starts_with(['+', '-']));
    let digits = index[sign..].bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    index[..sign + digits].parse().ok()
}

fn insertion_code(residue: &GuiSeqResidue) -> &str {
    let index = residue.residue_index.trim_start();
    let start = usize::from(index.starts_with(['+', '-']));
    let end = start
        + index[start..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
    &index[end..]
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSequenceLayout {
    pub minimum: i32,
    pub columns: usize,
    pub insertions: Vec<(i32, String, usize)>,
    pub gaps: Vec<GuiSequenceGap>,
}

#[derive(Debug, serde::Serialize)]
pub struct GuiSequenceGap {
    pub first: i32,
    pub last: i32,
    pub column: usize,
    pub removed: u64,
}

impl GuiSequenceLayout {
    pub fn new(chains: &[GuiSeqChain]) -> Self {
        let mut minimum = 0;
        let mut maximum = 0;
        let mut insertions = BTreeSet::new();
        let mut numbers = BTreeSet::from([0]);
        for residue in chains.iter().flat_map(|chain| &chain.residues) {
            numbers.insert(residue.res_id);
            minimum = minimum.min(residue.res_id);
            maximum = maximum.max(residue.res_id);
            let code = insertion_code(residue);
            if !code.is_empty() {
                insertions.insert((residue.res_id, code.to_string()));
            }
        }
        let mut gaps = Vec::new();
        if i64::from(maximum) - i64::from(minimum) > 100_000 {
            let mut previous = None;
            let mut removed = 0;
            for number in numbers {
                if let Some(before) = previous {
                    let missing = i64::from(number) - i64::from(before) - 1;
                    if missing > 8 {
                        removed += (missing - 1) as u64;
                        gaps.push(GuiSequenceGap {
                            first: before + 1,
                            last: number - 1,
                            column: 0,
                            removed,
                        });
                    }
                }
                previous = Some(number);
            }
        }
        let base_column = |index| {
            let prior = gaps.partition_point(|gap| gap.last < index);
            let removed = prior.checked_sub(1).map_or(0, |index| gaps[index].removed);
            ((i64::from(index) - i64::from(minimum)) as u64 - removed) as usize
        };
        let columns = base_column(maximum)
            .saturating_add(insertions.len())
            .saturating_add(10);
        let insertions: Vec<_> = insertions
            .into_iter()
            .enumerate()
            .map(|(offset, (index, code))| {
                let column = base_column(index) + offset + 1;
                (index, code, column)
            })
            .collect();
        for index in 0..gaps.len() {
            let prior_removed = index.checked_sub(1).map_or(0, |prior| gaps[prior].removed);
            let first = gaps[index].first;
            gaps[index].column = ((i64::from(first) - i64::from(minimum)) as u64 - prior_removed)
                as usize
                + insertions.partition_point(|(number, _, _)| *number < first);
        }
        Self {
            minimum,
            columns,
            insertions,
            gaps,
        }
    }

    pub fn column(&self, residue: &GuiSeqResidue) -> usize {
        let key = (residue.res_id, insertion_code(residue));
        let inserted = self
            .insertions
            .partition_point(|(index, code, _)| (*index, code.as_str()) <= key);
        let prior = self.gaps.partition_point(|gap| gap.last < residue.res_id);
        let removed = prior
            .checked_sub(1)
            .map_or(0, |index| self.gaps[index].removed);
        ((i64::from(residue.res_id) - i64::from(self.minimum)) as u64 - removed) as usize + inserted
    }

    pub fn is_gap(&self, column: usize) -> bool {
        let index = self.gaps.partition_point(|gap| gap.column < column);
        self.gaps.get(index).is_some_and(|gap| gap.column == column)
    }

    pub fn ruler_number(&self, column: usize) -> Option<i64> {
        let inserted = self
            .insertions
            .partition_point(|(_, _, position)| *position < column);
        if self.is_gap(column)
            || self
                .insertions
                .get(inserted)
                .is_some_and(|(_, _, position)| *position == column)
        {
            None
        } else {
            let prior = self.gaps.partition_point(|gap| gap.column < column);
            let removed = prior
                .checked_sub(1)
                .map_or(0, |index| self.gaps[index].removed);
            let number = i64::from(self.minimum) + column as i64 - inserted as i64 + removed as i64;
            i32::try_from(number).ok().map(i64::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_full_integer_span_keeps_residues_and_insertions_reachable() {
        let residues = [
            (i32::MIN, ""),
            (i32::MIN, "A"),
            (0, ""),
            (i32::MAX, ""),
            (i32::MAX, "B"),
        ]
        .map(|(number, code)| GuiSeqResidue {
            letter: "G".into(),
            res_id: number,
            residue_index: format!("{number}{code}"),
            res_name: "GLY".into(),
            selection: format!("'A'.{number}{code}.*"),
            selected: false,
        });
        let rows = [GuiSeqChain {
            object_uid: cuemol_core::scene::Uid::new(1),
            object_name: "sparse".into(),
            chain_id: "A".into(),
            residues: residues.to_vec(),
        }];
        let layout = GuiSequenceLayout::new(&rows);
        assert!(layout.columns < 20);
        assert_eq!(
            residues.each_ref().map(|residue| layout.column(residue)),
            [0, 1, 3, 5, 6]
        );
        assert_eq!(layout.gaps.len(), 2);
        assert_eq!(
            (layout.gaps[0].first, layout.gaps[0].last),
            (i32::MIN + 1, -1)
        );
        assert_eq!(
            (layout.gaps[1].first, layout.gaps[1].last),
            (1, i32::MAX - 1)
        );
        for (column, expected) in [
            (0, Some(i64::from(i32::MIN))),
            (1, None),
            (2, None),
            (3, Some(0)),
            (4, None),
            (5, Some(i64::from(i32::MAX))),
            (6, None),
            (7, None),
            (10, None),
        ] {
            assert_eq!(layout.ruler_number(column), expected);
        }
        assert!(layout.is_gap(2));
        assert!(layout.is_gap(4));
        let dto = serde_json::to_value(&layout).unwrap();
        assert_eq!(dto["gaps"][1]["column"], 4);
        assert_eq!(dto["gaps"][1]["removed"], 4_294_967_291_u64);
    }
}
