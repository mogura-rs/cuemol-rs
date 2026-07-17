const JITTER_2: [[i8; 2]; 2] = [[0, 0], [-4, -4]];
const JITTER_4: [[i8; 2]; 4] = [[0, 0], [6, -2], [-6, 2], [2, 6]];
const JITTER_8: [[i8; 2]; 8] = [
    [0, 0],
    [-1, 3],
    [5, 1],
    [-3, -5],
    [-5, 5],
    [-7, -1],
    [3, 7],
    [7, -7],
];
const JITTER_16: [[i8; 2]; 16] = [
    [0, 0],
    [-1, -3],
    [-3, 2],
    [4, -1],
    [-5, -2],
    [2, 5],
    [5, 3],
    [3, -5],
    [-2, 6],
    [0, -7],
    [-4, -6],
    [-6, 4],
    [-8, 0],
    [7, -4],
    [6, 7],
    [-7, -8],
];
const JITTER_32: [[i8; 2]; 32] = [
    [0, 0],
    [-7, -5],
    [-3, -5],
    [-5, -4],
    [-1, -4],
    [-2, -2],
    [-6, -1],
    [-4, 0],
    [-7, 1],
    [-1, 2],
    [-6, 3],
    [-3, 3],
    [-7, 6],
    [-3, 6],
    [-5, 7],
    [-1, 7],
    [5, -7],
    [1, -6],
    [6, -5],
    [4, -4],
    [2, -3],
    [7, -2],
    [1, -1],
    [4, -1],
    [2, 1],
    [6, 2],
    [0, 4],
    [4, 4],
    [2, 5],
    [7, 5],
    [5, 6],
    [3, 7],
];

pub fn jitter_sample_count(level: i32) -> usize {
    if level <= 0 {
        1
    } else {
        1_usize << level.min(5)
    }
}

pub fn jitter_offset(level: i32, index: i32) -> (f64, f64) {
    let table: &[[i8; 2]] = match level {
        1 => &JITTER_2,
        2 => &JITTER_4,
        3 => &JITTER_8,
        4 => &JITTER_16,
        5 => &JITTER_32,
        _ => return (0.0, 0.0),
    };
    let Ok(index) = usize::try_from(index) else {
        return (0.0, 0.0);
    };
    let Some([x, y]) = table.get(index).copied() else {
        return (0.0, 0.0);
    };
    (f64::from(x) / 16.0, f64::from(y) / 16.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_counts_are_clamped_to_reference_levels() {
        assert_eq!(jitter_sample_count(-1), 1);
        assert_eq!(jitter_sample_count(0), 1);
        assert_eq!(jitter_sample_count(1), 2);
        assert_eq!(jitter_sample_count(5), 32);
        assert_eq!(jitter_sample_count(8), 32);
    }

    #[test]
    fn offsets_match_reference_tables_and_reject_invalid_indices() {
        assert_eq!(jitter_offset(2, 1), (0.375, -0.125));
        assert_eq!(jitter_offset(4, 15), (-0.4375, -0.5));
        assert_eq!(jitter_offset(5, 31), (0.1875, 0.4375));
        assert_eq!(jitter_offset(0, 0), (0.0, 0.0));
        assert_eq!(jitter_offset(3, -1), (0.0, 0.0));
        assert_eq!(jitter_offset(3, 8), (0.0, 0.0));
    }
}
