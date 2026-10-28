use cuemol_core::Vector4;
use cuemol_molstr::GridMap;
use cuemol_qsys::ScalarObject;
use cuemol_xtal::{DensityMap, MapRenderer};

fn map_case(kind: u8) -> DensityMap {
    if kind == 2 {
        return DensityMap::from_continuous_grid(GridMap {
            counts: [16; 3],
            origin: Vector4::ZERO,
            delta: [
                Vector4::new3(1., 0., 0.),
                Vector4::new3(0., 1., 0.),
                Vector4::new3(0., 0., 1.),
            ],
            values: (0..4096).map(|i| (i % 17 - 8) as f64 * 0.125).collect(),
        });
    }
    let bytes = (0..4096)
        .map(|i| match kind {
            3 => 0,
            4 => (i % 256) as u8,
            _ => {
                if i < 10 {
                    230 + i as u8
                } else {
                    0
                }
            }
        })
        .collect::<Vec<_>>();
    let (min, max, mean, sigma) = match kind {
        3 => (5., 5., 5., 0.),
        4 => (0., 2.56e-18, 1.27e-18, 7.4e-19),
        _ => (-2., 254., 0.5, 10.),
    };
    let mut map = DensityMap::new();
    map.set_map_byte_array(&bytes, [16; 3], min, max, mean, sigma);
    map.set_map_params([0; 3], [16; 3]);
    map.set_xtal_params(16., 16., 16., 90., 90., 90.);
    map.set_detected_map_type(if kind == 1 || kind == 3 { 2 } else { 1 });
    map
}

fn states(map: &DensityMap, kind: u8) -> Vec<Vec<f64>> {
    let mut renderer = MapRenderer::new();
    renderer.attach_client_scalar(map);
    let mut states = Vec::new();
    for phase in 0..5 {
        match phase {
            1 => renderer.set_siglevel(1.1),
            2 => renderer.reset_siglevel(),
            3 => renderer.set_level(if kind == 4 { 1e-18 } else { 42. }),
            4 => renderer.set_use_abslevel(true),
            _ => (),
        }
        assert_eq!(renderer.level(), renderer.level_from(map));
        states.push(vec![
            renderer.siglevel(),
            renderer.siglevel_effective(),
            renderer.level(),
            renderer.min_level(),
            renderer.max_level(),
            renderer.ui_abslevel_min(),
            renderer.ui_abslevel_max(),
            renderer.ui_level_min(),
            renderer.ui_level_max(),
            if renderer.siglevel_is_default() {
                1.
            } else {
                0.
            },
        ]);
    }
    states
}

#[test]
fn sparse_em_default_levels_differ_from_explicit_levels() {
    let map = map_case(1);
    let values = states(&map, 1);
    assert_eq!(values[0][1], 10. / 4096. * 100.);
    assert_eq!(values[0][2], 228.);
    assert_eq!(values[1][1], 1.1);
    assert_eq!(values[1][2], -2.);
    assert_eq!(values[2], values[0]);
    assert_eq!(values[0][3], -0.2);
    assert_eq!(values[0][4], 25.4);
    assert_eq!(values[0][7], 0.01);
    assert_eq!(map.quant_step(), 1.);
}

#[test]
fn map_levels_and_default_flags_match_cpp() {
    let Some(oracle) = std::env::var_os("CUEMOL_MAP_LEVEL_ORACLE") else {
        eprintln!(
            "Skipping C++ map-level oracle: set CUEMOL_MAP_LEVEL_ORACLE and CUEMOL_SCALAR_CONFIG"
        );
        return;
    };
    let config = std::env::var_os("CUEMOL_SCALAR_CONFIG").expect("oracle config");
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("levels.txt");
    assert!(std::process::Command::new(oracle)
        .args([config.as_os_str(), output.as_os_str()])
        .status()
        .unwrap()
        .success());
    let text = std::fs::read_to_string(output).unwrap();
    let expected = text
        .lines()
        .map(|line| {
            line.split_whitespace()
                .map(|x| x.parse::<f64>().unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let actual = (0..5)
        .flat_map(|kind| states(&map_case(kind), kind))
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), actual.len());
    for (row, (a, b)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(a.len(), b.len());
        for (column, (a, b)) in a.iter().zip(b).enumerate() {
            let scale = if row / 5 == 4 && [2, 5, 6].contains(&column) {
                1e-18
            } else {
                1.
            };
            assert!(
                (a - b).abs() < 1e-10 * b.abs().max(scale),
                "row {row} column {column}: Rust {a}, C++ {b}"
            );
        }
    }
    eprintln!("Compared 25 map-level states with C++ including explicit/reset EM defaults and continuous potentials");
}
