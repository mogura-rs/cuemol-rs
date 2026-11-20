//! Regression fixtures for the surface kernels, recorded from the original
//! C++ implementations (MeshMS, BALL, EDTSurf) before they were ported.

use crate::backends::kernel;
use std::path::{Path, PathBuf};

const PROBE: f64 = 1.4;
/// (kernel backend id, density/scale parameter)
const BACKENDS: [(u32, f64); 6] = [(1, 3.0), (2, 3.0), (3, 2.0), (4, 2.0), (5, 2.0), (6, 0.8)];

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

struct Lcg(u64);
impl Lcg {
    fn unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64) * 2.0 - 1.0
    }
}

/// Deterministic self-avoiding chain built with correctly rounded operations only.
fn chain(count: usize, seed: u64) -> Vec<f64> {
    const RADII: [f64; 4] = [1.7, 1.55, 1.52, 1.8];
    let mut rng = Lcg(seed);
    let mut atoms: Vec<[f64; 3]> = vec![[0.0; 3]];
    while atoms.len() < count {
        let [x, y, z] = [rng.unit(), rng.unit(), rng.unit()];
        let norm = (x * x + y * y + z * z).sqrt();
        if !(0.1..=1.0).contains(&norm) {
            continue;
        }
        let last = atoms[atoms.len() - 1];
        let next = [
            last[0] + 1.5 * x / norm,
            last[1] + 1.5 * y / norm,
            last[2] + 1.5 * z / norm,
        ];
        let clash = atoms[..atoms.len() - 1].iter().any(|a| {
            let d = [a[0] - next[0], a[1] - next[1], a[2] - next[2]];
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < 2.4 * 2.4
        });
        if !clash {
            atoms.push(next);
        }
    }
    atoms
        .iter()
        .enumerate()
        .flat_map(|(i, a)| [a[0], a[1], a[2], RADII[i % RADII.len()]])
        .collect()
}

fn read_xyzr(path: &Path) -> Vec<f64> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .split_whitespace()
        .map(|v| f64::from_bits(u64::from_str_radix(v, 16).unwrap()))
        .collect()
}

fn cases() -> Vec<(&'static str, Vec<f64>)> {
    vec![
        (
            "trio",
            vec![0.0, 0.0, 0.0, 1.7, 2.2, 0.3, 0.0, 1.55, 1.0, 1.9, 0.4, 1.52],
        ),
        ("chain40", chain(40, 7)),
        ("crn", read_xyzr(&golden_dir().join("crn.xyzr"))),
    ]
}

type KernelOutput = std::result::Result<(Vec<f64>, Vec<u32>, u32, Vec<u32>), String>;

fn run(xyzr: &[f64], backend: u32, density: f64) -> KernelOutput {
    kernel(xyzr, density, PROBE, backend).map_err(|e| e.to_string())
}

fn encode(output: &KernelOutput) -> Vec<u8> {
    let mut bytes = Vec::new();
    match output {
        Err(message) => {
            bytes.extend(b"CMSE");
            bytes.extend(message.as_bytes());
        }
        Ok((vertices, triangles, used, owners)) => {
            bytes.extend(b"CMSG");
            for n in [vertices.len() / 6, triangles.len() / 3, *used as usize] {
                bytes.extend((n as u32).to_le_bytes());
            }
            vertices.iter().for_each(|v| bytes.extend(v.to_le_bytes()));
            owners.iter().for_each(|v| bytes.extend(v.to_le_bytes()));
            triangles.iter().for_each(|v| bytes.extend(v.to_le_bytes()));
        }
    }
    bytes
}

fn decode(bytes: &[u8]) -> KernelOutput {
    let (magic, body) = bytes.split_at(4);
    if magic == b"CMSE" {
        return Err(String::from_utf8(body.to_vec()).unwrap());
    }
    assert_eq!(magic, b"CMSG");
    let u32_at = |offset: usize| u32::from_le_bytes(body[offset..offset + 4].try_into().unwrap());
    let (nv, nt, used) = (u32_at(0) as usize, u32_at(4) as usize, u32_at(8));
    let mut offset = 12;
    let vertices = (0..nv * 6)
        .map(|i| f64::from_le_bytes(body[offset + i * 8..offset + i * 8 + 8].try_into().unwrap()))
        .collect();
    offset += nv * 48;
    let owners = (0..nv).map(|i| u32_at(offset + i * 4)).collect();
    offset += nv * 4;
    let triangles = (0..nt * 3).map(|i| u32_at(offset + i * 4)).collect();
    Ok((vertices, triangles, used, owners))
}

fn golden_path(case: &str, backend: u32) -> PathBuf {
    golden_dir().join(format!("{case}.b{backend}.bin"))
}

/// Records the fixtures; run with the native C++ kernels still linked.
#[test]
#[ignore]
fn record_surface_golden() {
    let dir = golden_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let reference = std::env::var_os("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let molecule =
        cuemol_molstr::read_pdb_file(reference.join("tests/test_data/1CRN.pdb")).unwrap();
    let crn = molecule
        .atoms()
        .iter()
        .filter(|atom| atom.alt_loc.is_none_or(|alt| alt == 'A' || alt == ' '))
        .take(120)
        .flat_map(|atom| {
            [
                atom.position.x,
                atom.position.y,
                atom.position.z,
                cuemol_molstr::toppar::vdw_radius_for_atom(atom).max(0.1),
            ]
        })
        .map(|v| format!("{:016x}", v.to_bits()))
        .collect::<Vec<_>>();
    let text = crn
        .chunks(4)
        .map(|c| c.join(" "))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.join("crn.xyzr"), text + "\n").unwrap();
    for (name, xyzr) in cases() {
        for (backend, density) in BACKENDS {
            let output = run(&xyzr, backend, density);
            std::fs::write(golden_path(name, backend), encode(&output)).unwrap();
        }
    }
}

/// BALL emits triangles in pointer-hash order, so compare oriented triangles as a set.
fn canonical(triangles: &[u32]) -> Vec<[u32; 3]> {
    let mut faces: Vec<[u32; 3]> = triangles
        .chunks_exact(3)
        .map(|t| {
            let k = (0..3).min_by_key(|&i| t[i]).unwrap();
            [t[k], t[(k + 1) % 3], t[(k + 2) % 3]]
        })
        .collect();
    faces.sort_unstable();
    faces
}

fn compare(label: &str, expected: &KernelOutput, actual: &KernelOutput) {
    match (expected, actual) {
        (Err(_), Err(_)) => {}
        (Ok(e), Ok(a)) => {
            assert_eq!(e.2, a.2, "{label}: backend");
            assert_eq!(e.0.len(), a.0.len(), "{label}: vertex count");
            assert_eq!(e.1.len(), a.1.len(), "{label}: triangle count");
            let mismatch =
                e.0.iter()
                    .zip(&a.0)
                    .position(|(x, y)| x.to_bits() != y.to_bits());
            assert!(
                mismatch.is_none(),
                "{label}: vertex value {} differs: {} vs {}",
                mismatch.unwrap(),
                e.0[mismatch.unwrap()],
                a.0[mismatch.unwrap()]
            );
            assert_eq!(e.3, a.3, "{label}: owners");
            assert_eq!(canonical(&e.1), canonical(&a.1), "{label}: triangles");
        }
        _ => panic!(
            "{label}: status differs: {:?} vs {:?}",
            expected.as_ref().err(),
            actual.as_ref().err()
        ),
    }
}

fn check_backend(backend: u32) {
    for (name, xyzr) in cases() {
        let density = BACKENDS.iter().find(|(b, _)| *b == backend).unwrap().1;
        let expected = decode(&std::fs::read(golden_path(name, backend)).unwrap());
        compare(
            &format!("{name}/b{backend}"),
            &expected,
            &run(&xyzr, backend, density),
        );
    }
}

#[test]
fn meshms_matches_golden() {
    check_backend(1);
    check_backend(6);
}

#[test]
fn ball_matches_golden() {
    check_backend(2);
}

#[test]
fn edtsurf_matches_golden() {
    for backend in 3..=5 {
        check_backend(backend);
    }
}
