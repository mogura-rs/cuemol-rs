use cuemol_core::Vector4;
use cuemol_qsys::ScalarObject;
use cuemol_xtal::{Ccp4MapReader, DensityMap};
use std::io::Write;

fn word(bytes: &mut [u8], index: usize, value: u32, be: bool) {
    let value = if be {
        value.to_be_bytes()
    } else {
        value.to_le_bytes()
    };
    bytes[index * 4..index * 4 + 4].copy_from_slice(&value);
}
fn float(bytes: &mut [u8], index: usize, value: f32, be: bool) {
    word(bytes, index, value.to_bits(), be);
}
fn fixture(mode: i32, be: bool, rotated: bool, kind: usize) -> Vec<u8> {
    let mut bytes = vec![0; 1024];
    for axis in 0..3 {
        word(&mut bytes, axis, 4, be);
        word(&mut bytes, axis + 4, (axis as i32 * 2 - 2) as u32, be);
        word(&mut bytes, axis + 7, 8, be);
        float(&mut bytes, axis + 10, (axis as f32 + 1.) * 8., be);
        float(
            &mut bytes,
            axis + 13,
            if axis == 2 && kind == 0 { 120. } else { 90. },
            be,
        );
        word(
            &mut bytes,
            axis + 16,
            if rotated {
                [2, 3, 1][axis]
            } else {
                axis as u32 + 1
            },
            be,
        );
    }
    word(&mut bytes, 3, mode as u32, be);
    word(&mut bytes, 22, if kind == 1 { 0 } else { 1 }, be);
    word(&mut bytes, 38, 1146047817, be);
    word(&mut bytes, 39, 1, be);
    bytes[208..212].copy_from_slice(b"MAP ");
    float(&mut bytes, 19, -10., be);
    float(&mut bytes, 20, 30., be);
    float(&mut bytes, 21, 2., be);
    float(&mut bytes, 54, 2., be);
    if kind == 2 {
        float(&mut bytes, 49, -11.5, be);
        float(&mut bytes, 50, 8.25, be);
        float(&mut bytes, 51, 3., be);
    }
    if kind == 3 {
        word(&mut bytes, 55, 1, be);
        bytes[224..234].copy_from_slice(b"CryoSPARC ");
    }
    for i in 0..64 {
        let value = (i % 16) as i16 - if mode == 6 { 0 } else { 8 };
        let half = if value == 0 {
            0
        } else {
            let f = f32::from(value).to_bits();
            (((f >> 16) & 0x8000) | ((((f >> 23) & 255) - 112) << 10) | ((f >> 13) & 1023)) as u16
        };
        match mode {
            0 => bytes.push(value as i8 as u8),
            1 | 6 | 12 => {
                let value = if mode == 12 { half } else { value as u16 };
                bytes.extend_from_slice(&if be {
                    value.to_be_bytes()
                } else {
                    value.to_le_bytes()
                });
            }
            _ => bytes.extend_from_slice(&if be {
                f32::from(value).to_be_bytes()
            } else {
                f32::from(value).to_le_bytes()
            }),
        }
    }
    bytes
}

fn summary(map: &DensityMap) -> Vec<f64> {
    let origin = map.origin();
    let corner = map.conv_to_orth(Vector4::new3(1., 2., 3.));
    let mut out = vec![
        map.col_no() as f64,
        map.row_no() as f64,
        map.sec_no() as f64,
        map.start_col() as f64,
        map.start_row() as f64,
        map.start_sec() as f64,
        origin.x,
        origin.y,
        origin.z,
        corner.x,
        corner.y,
        corner.z,
        f64::from(map.detected_map_type()),
        map.den_min(),
        map.den_max(),
        map.den_mean(),
        map.den_sigma(),
        map.level_base(),
        map.level_step(),
    ];
    out.extend([
        map.at_float(0, 0, 0),
        map.value_at(map.conv_to_orth(Vector4::new3(0.4, 0.6, 0.8))),
        map.quant_step(),
    ]);
    out.extend(map.byte_values().iter().map(|v| f64::from(*v)));
    out
}

#[test]
fn header_probe_sampling_and_limits_preserve_grid_placement() {
    let bytes = fixture(2, false, true, 2);
    let mut reader = Ccp4MapReader::new();
    let probe: serde_json::Value =
        serde_json::from_str(&reader.probe_header_bytes(&bytes[..1024]).unwrap()).unwrap();
    assert_eq!(probe["nvoxels"], 64);
    assert_eq!(probe["axis"], serde_json::json!([2, 3, 1]));
    assert_eq!(probe["start"], serde_json::json!([-2, 0, 2]));
    reader.set_max_voxels(8.);
    assert!(reader
        .read_density_map_bytes(&bytes[..1024])
        .unwrap_err()
        .to_string()
        .contains("max_voxels"));
    reader.set_subsample(2);
    let map = reader.read_density_map_bytes(&bytes).unwrap();
    assert_eq!([map.col_no(), map.row_no(), map.sec_no()], [2; 3]);
    assert_eq!([map.start_col(), map.start_row(), map.start_sec()], [0; 3]);
    assert_eq!(map.origin(), Vector4::new3(-11.5, 8.25, 3.));
    assert!(map.is_em());
    assert!((map.col_grid_size() - 2.).abs() < 1e-12);
    reader.set_subsample(3);
    assert!(reader
        .read_density_map_bytes(&bytes)
        .unwrap_err()
        .to_string()
        .contains("does not divide"));
    reader.set_subsample(2);
    assert!(reader
        .read_density_map_bytes(&bytes[..bytes.len() - 1])
        .is_err());
    let mut invalid = bytes.clone();
    word(&mut invalid, 3, 4, false);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&reader.probe_header_bytes(&invalid).unwrap())
            .unwrap()["supported"],
        false
    );
    assert!(reader.read_density_map_bytes(&invalid).is_err());
}

#[test]
fn streamed_modes_header_ranges_and_compressed_files_roundtrip() {
    let reader = Ccp4MapReader::new();
    let expected = summary(
        &reader
            .read_density_map_bytes(&fixture(1, false, false, 0))
            .unwrap(),
    );
    for mode in [0, 1, 2, 12] {
        for be in [false, true] {
            assert_eq!(
                summary(
                    &reader
                        .read_density_map_bytes(&fixture(mode, be, false, 0))
                        .unwrap()
                ),
                expected
            );
        }
    }
    let mut stale = fixture(2, false, false, 0);
    float(&mut stale, 19, -1., false);
    float(&mut stale, 20, 1., false);
    float(&mut stale, 21, 0., false);
    let measured = reader.read_density_map_bytes(&stale).unwrap();
    assert_eq!(measured.level_base(), -8.);
    assert_eq!(measured.level_step(), 15. / 256.);
    let mut reader = reader;
    reader.set_use_header_stats(false);
    assert_eq!(
        summary(
            &reader
                .read_density_map_bytes(&fixture(2, false, false, 0))
                .unwrap()
        ),
        summary(&measured)
    );
    let bytes = fixture(2, false, true, 3);
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gzip.write_all(&bytes).unwrap();
    let mut bzip = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
    bzip.write_all(&bytes).unwrap();
    let mut xz = Vec::new();
    lzma_rs::xz_compress(&mut std::io::Cursor::new(&bytes), &mut xz).unwrap();
    let dir = tempfile::tempdir().unwrap();
    for (i, encoded) in [
        bytes.clone(),
        gzip.finish().unwrap(),
        bzip.finish().unwrap(),
        xz,
    ]
    .iter()
    .enumerate()
    {
        let path = dir.path().join(format!("map-{i}"));
        std::fs::write(&path, encoded).unwrap();
        let from_file = reader.read_density_map_file(&path).unwrap();
        assert_eq!(from_file, reader.read_density_map_bytes(&bytes).unwrap());
        assert_eq!(from_file, reader.read_density_map_bytes(encoded).unwrap());
        assert_eq!(
            reader.probe_header(&path).unwrap(),
            reader.probe_header_bytes(&bytes).unwrap()
        );
    }
}

#[test]
fn half_subnormals_unsigned_bytes_and_zero_rms_are_supported() {
    let mut bytes = fixture(12, false, false, 0);
    bytes[1024..1026].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        Ccp4MapReader::new().read_bytes(&bytes).unwrap().grid.values[0],
        2f64.powi(-24)
    );
    bytes = fixture(0, false, false, 0);
    word(&mut bytes, 39, 0, false);
    let map = Ccp4MapReader::new().read_bytes(&bytes).unwrap();
    assert_eq!(map.grid.values[0], 248.);
    let mut reader = Ccp4MapReader::new();
    reader.set_normalize(true);
    float(&mut bytes, 54, 0., false);
    assert_eq!(reader.read_bytes(&bytes).unwrap().grid.values[0], 248.);
}

#[test]
fn saved_header_quantization_preserves_density_values() {
    for use_header in [false, true] {
        let mut reader = Ccp4MapReader::new();
        reader.set_use_header_stats(use_header);
        let map = reader
            .read_density_map_bytes(&fixture(2, false, true, 2))
            .unwrap();
        let restored =
            cuemol_xtal::read_qdf_denmap_bytes(&cuemol_xtal::write_qdf_denmap_bytes(&map)).unwrap();
        assert_eq!(map.level_base(), restored.level_base());
        assert_eq!(map.level_step(), restored.level_step());
        assert_eq!(map.byte_values(), restored.byte_values());
        assert_eq!(map.origin(), restored.origin());
    }
}

#[test]
fn ccp4_streamed_values_match_cpp_oracle() {
    let Ok(oracle) = std::env::var("CUEMOL_CCP4_ORACLE") else {
        eprintln!("Skipping C++ comparison: set CUEMOL_CCP4_ORACLE and CUEMOL_CCP4_CONFIG");
        return;
    };
    let config = std::env::var("CUEMOL_CCP4_CONFIG").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.map");
    let output = dir.path().join("output.txt");
    let mut cases = 0;
    for mode in [0, 1, 2, 6, 12] {
        for be in [false, true] {
            for variant in 0..8 {
                let mut bytes = fixture(mode, be, variant & 1 != 0, variant % 4);
                let stride = if variant >= 4 { 2 } else { 1 };
                let use_header = variant % 3 != 0;
                if variant == 6 {
                    float(&mut bytes, 19, -1., be);
                    float(&mut bytes, 20, 1., be);
                    float(&mut bytes, 21, 0., be);
                }
                let transform = variant % 4;
                let mut reader = Ccp4MapReader::new();
                reader.set_subsample(stride);
                reader.set_use_header_stats(use_header);
                reader.set_normalize(transform & 1 != 0);
                reader.set_truncate_min(transform & 2 != 0);
                reader.set_truncate_max(transform & 2 != 0);
                reader.set_min(-0.5);
                reader.set_max(1.5);
                std::fs::write(&input, &bytes).unwrap();
                let result = std::process::Command::new(&oracle)
                    .arg(&config)
                    .arg(&input)
                    .arg(&output)
                    .args([
                        stride.to_string(),
                        usize::from(use_header).to_string(),
                        transform.to_string(),
                        "0".into(),
                    ])
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                let text = std::fs::read_to_string(&output).unwrap();
                let actual = summary(&reader.read_density_map_bytes(&bytes).unwrap());
                assert!(
                    text.starts_with("ok\n"),
                    "mode={mode},be={be},variant={variant}: {text}"
                );
                let expected = text
                    .split_whitespace()
                    .skip(1)
                    .map(|v| v.parse::<f64>().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(actual.len(), expected.len());
                for (i, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                    assert!(
                        (actual - expected).abs() <= 1e-10 * expected.abs().max(1.),
                        "mode={mode},be={be},variant={variant},index={i}: {actual} != {expected}"
                    );
                }
                cases += 1;
            }
        }
    }
    eprintln!("Compared {cases} CCP4/MRC cases with C++ (all stored voxels, statistics, type, and coordinates)");
}
