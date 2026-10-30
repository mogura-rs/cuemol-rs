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
}

impl GuiSequenceLayout {
    pub fn new(chains: &[GuiSeqChain]) -> Self {
        let mut minimum = 0;
        let mut maximum = 0;
        let mut insertions = BTreeSet::new();
        for residue in chains.iter().flat_map(|chain| &chain.residues) {
            minimum = minimum.min(residue.res_id);
            maximum = maximum.max(residue.res_id);
            let code = insertion_code(residue);
            if !code.is_empty() {
                insertions.insert((residue.res_id, code.to_string()));
            }
        }
        let columns = (i64::from(maximum) - i64::from(minimum)) as usize + insertions.len() + 10;
        let insertions = insertions
            .into_iter()
            .enumerate()
            .map(|(offset, (index, code))| {
                let column = (i64::from(index) - i64::from(minimum)) as usize + offset + 1;
                (index, code, column)
            })
            .collect();
        Self {
            minimum,
            columns,
            insertions,
        }
    }

    pub fn column(&self, residue: &GuiSeqResidue) -> usize {
        let key = (residue.res_id, insertion_code(residue));
        let inserted = self
            .insertions
            .partition_point(|(index, code, _)| (*index, code.as_str()) <= key);
        (i64::from(residue.res_id) - i64::from(self.minimum)) as usize + inserted
    }

    pub fn ruler_number(&self, column: usize) -> Option<i64> {
        let inserted = self
            .insertions
            .partition_point(|(_, _, position)| *position < column);
        if self
            .insertions
            .get(inserted)
            .is_some_and(|(_, _, position)| *position == column)
        {
            None
        } else {
            Some(i64::from(self.minimum) + column as i64 - inserted as i64)
        }
    }
}
