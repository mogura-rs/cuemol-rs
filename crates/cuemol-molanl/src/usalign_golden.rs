//! Regression fixtures for US-align, recorded from the original C++ implementation.

use crate::usalign::align_arrays;
use std::path::{Path, PathBuf};

struct Case {
    name: String,
    pivot: String,
    moving: (Vec<f64>, Vec<u8>),
    reference: (Vec<f64>, Vec<u8>),
    matrix: [f64; 12],
    scores: [f64; 6],
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/usalign.txt")
}

fn unhex(text: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(text, 16).unwrap())
}

fn read_cases() -> Vec<Case> {
    let text = std::fs::read_to_string(fixture()).unwrap();
    let mut lines = text.lines();
    let mut cases = Vec::new();
    let chain = |lines: &mut std::str::Lines, tag: &str| {
        let header = lines.next().unwrap();
        let count: usize = header.strip_prefix(tag).unwrap().trim().parse().unwrap();
        let mut xyz = Vec::new();
        let mut names = Vec::new();
        for _ in 0..count {
            let fields: Vec<_> = lines.next().unwrap().split(' ').collect();
            xyz.extend(fields[..3].iter().map(|v| unhex(v)));
            let record = fields[3].as_bytes();
            names.extend(
                record[..3]
                    .iter()
                    .map(|&b| if b == b'_' { b' ' } else { b }),
            );
            names.push(record[3] - b'0');
        }
        (xyz, names)
    };
    while let Some(header) = lines.next() {
        let fields: Vec<_> = header.split(' ').collect();
        assert_eq!(fields[0], "case");
        let moving = chain(&mut lines, "moving");
        let reference = chain(&mut lines, "reference");
        let values = |line: &str, tag: &str| -> Vec<f64> {
            line.strip_prefix(tag)
                .unwrap()
                .split_whitespace()
                .map(unhex)
                .collect()
        };
        let matrix = values(lines.next().unwrap(), "matrix").try_into().unwrap();
        let scores = values(lines.next().unwrap(), "scores").try_into().unwrap();
        cases.push(Case {
            name: fields[1].to_string(),
            pivot: fields[2].to_string(),
            moving,
            reference,
            matrix,
            scores,
        });
    }
    cases
}

#[test]
fn usalign_matches_golden() {
    for case in read_cases() {
        let (matrix, scores) = align_arrays(
            &case.moving.0,
            &case.moving.1,
            &case.reference.0,
            &case.reference.1,
            &case.pivot,
        )
        .unwrap();
        for (i, (e, a)) in case.matrix.iter().zip(&matrix).enumerate() {
            assert_eq!(
                e.to_bits(),
                a.to_bits(),
                "{}: matrix[{i}] {e} vs {a}",
                case.name
            );
        }
        for (i, (e, a)) in case.scores.iter().zip(&scores).enumerate() {
            assert_eq!(
                e.to_bits(),
                a.to_bits(),
                "{}: scores[{i}] {e} vs {a}",
                case.name
            );
        }
    }
}
