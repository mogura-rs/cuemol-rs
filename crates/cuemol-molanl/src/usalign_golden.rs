//! Regression fixtures for US-align, recorded from the original C++ implementation.

use crate::usalign::{align_arrays, pivot_arrays};
use cuemol_molstr::Selector;
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

fn hex(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

fn unhex(text: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(text, 16).unwrap())
}

fn write_chain(out: &mut String, tag: &str, (xyz, names): &(Vec<f64>, Vec<u8>)) {
    out.push_str(&format!("{tag} {}\n", xyz.len() / 3));
    for (p, n) in xyz.chunks(3).zip(names.chunks(4)) {
        let name: String = n[..3]
            .iter()
            .map(|&b| if b == b' ' { '_' } else { b as char })
            .collect();
        out.push_str(&format!(
            "{} {} {} {}{}\n",
            hex(p[0]),
            hex(p[1]),
            hex(p[2]),
            name,
            n[3]
        ));
    }
}

fn read_cases() -> Vec<Case> {
    let text = std::fs::read_to_string(fixture()).unwrap();
    let mut lines = text.lines();
    let mut cases = Vec::new();
    let mut chain = |lines: &mut std::str::Lines, tag: &str| {
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

/// Records the fixtures; run with the native C++ implementation still linked.
#[test]
#[ignore]
fn record_usalign_golden() {
    let reference = std::env::var_os("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let data = reference.join("tests/test_data");
    let load = |name: &str| {
        let path = data.join(name);
        cuemol_molstr::read_pdb_file(path).unwrap()
    };
    let arrays =
        |name: &str, pivot: &str| pivot_arrays(&load(name), &Selector::All, pivot).unwrap();
    let crn = arrays("1CRN.pdb", "");
    let slice = |(xyz, names): &(Vec<f64>, Vec<u8>), from: usize, to: usize| {
        (
            xyz[from * 3..to * 3].to_vec(),
            names[from * 4..to * 4].to_vec(),
        )
    };
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    let mut noise = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        ((rng >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 0.6
    };
    // Rotation by a rational orthogonal matrix (3-4-5 triangle) plus translation and noise.
    let moved = (
        crn.0
            .chunks(3)
            .flat_map(|p| {
                [
                    0.6 * p[0] - 0.8 * p[1] + 12.5 + noise(),
                    0.8 * p[0] + 0.6 * p[1] - 3.25 + noise(),
                    p[2] + 7.0 + noise(),
                ]
            })
            .collect::<Vec<_>>(),
        crn.1.clone(),
    );
    let reversed = (
        crn.0.chunks(3).rev().flatten().copied().collect::<Vec<_>>(),
        crn.1.chunks(4).rev().flatten().copied().collect::<Vec<_>>(),
    );
    let inputs = vec![
        ("crn_af2", "C3'", arrays("1crn_af2_1.pdb", ""), crn.clone()),
        (
            "af2_af2",
            "C3'",
            arrays("1crn_af2_2.pdb", ""),
            arrays("1crn_af2_1.pdb", ""),
        ),
        ("crn_rev", "C3'", reversed.clone(), crn.clone()),
        ("rna_crn", "C3'", arrays("1ehz_rna.pdb", ""), crn.clone()),
        (
            "rna_rna",
            "C3'",
            arrays("3cw5_rna.pdb", ""),
            arrays("1ehz_rna.pdb", ""),
        ),
        ("crn_part", "C3'", slice(&crn, 4, 40), crn.clone()),
        ("crn_moved", "C3'", moved, crn.clone()),
        (
            "crn_n",
            "N",
            arrays("1crn_af2_1.pdb", "N"),
            arrays("1CRN.pdb", "N"),
        ),
    ];
    let mut out = String::new();
    for (name, pivot, moving, reference) in inputs {
        let Ok((matrix, scores)) =
            align_arrays(&moving.0, &moving.1, &reference.0, &reference.1, pivot)
        else {
            eprintln!("skipping {name}: alignment failed");
            continue;
        };
        out.push_str(&format!("case {name} {pivot}\n"));
        write_chain(&mut out, "moving", &moving);
        write_chain(&mut out, "reference", &reference);
        out.push_str(&format!(
            "matrix {}\n",
            matrix.iter().map(|v| hex(*v)).collect::<Vec<_>>().join(" ")
        ));
        out.push_str(&format!(
            "scores {}\n",
            scores.iter().map(|v| hex(*v)).collect::<Vec<_>>().join(" ")
        ));
    }
    std::fs::create_dir_all(fixture().parent().unwrap()).unwrap();
    std::fs::write(fixture(), out).unwrap();
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
