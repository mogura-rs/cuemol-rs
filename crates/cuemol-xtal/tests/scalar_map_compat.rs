use cuemol_core::Vector4;
use cuemol_molstr::GridMap;
use cuemol_qsys::ScalarObject;
use cuemol_xtal::DensityMap;

#[test]
fn continuous_potential_samples_and_histograms_match_cpp() {
    let Some(oracle) = std::env::var_os("CUEMOL_SCALAR_ORACLE") else {
        eprintln!("Skipping C++ scalar oracle: set CUEMOL_SCALAR_ORACLE and CUEMOL_SCALAR_CONFIG");
        return;
    };
    let config = std::env::var_os("CUEMOL_SCALAR_CONFIG").expect("oracle config");
    let dir = tempfile::tempdir().unwrap();
    for case in 0..5 {
        let counts = [4, 3, 5];
        let values: Vec<f64> = (0..60)
            .map(|i| match case {
                0 => (i as f64 / 7.).sin() * 4. - 0.3,
                1 => 0.125,
                2 => {
                    if i == 0 {
                        1e8
                    } else {
                        i as f64 * 0.125 - 1.
                    }
                }
                3 => i as f64 * 0.125 - 3.25,
                _ => i as f64 * 1e-20 - 1e-19,
            })
            .collect();
        let map = DensityMap::from_continuous_grid(GridMap {
            counts,
            origin: Vector4::new3(1., 2., 3.),
            delta: [
                Vector4::new3(0.5, 0., 0.),
                Vector4::new3(0., 2., 0.),
                Vector4::new3(0., 0., 3.),
            ],
            values: values.clone(),
        });
        let input = dir.path().join(format!("case-{case}.txt"));
        let output = dir.path().join(format!("case-{case}.out"));
        std::fs::write(
            &input,
            format!(
                "4 3 5\n{}",
                values
                    .iter()
                    .map(|v| format!("{}", *v as f32))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        )
        .unwrap();
        let status = std::process::Command::new(&oracle)
            .args([config.as_os_str(), input.as_os_str(), output.as_os_str()])
            .status()
            .unwrap();
        assert!(status.success());
        let result = std::fs::read_to_string(&output).unwrap();
        let mut lines = result.lines();
        let mut expected = Vec::new();
        for _ in 0..4 {
            expected.extend(
                lines
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .map(|v| v.parse::<f64>().unwrap()),
            );
        }
        let mut actual = vec![
            map.den_min(),
            map.den_max(),
            map.den_mean(),
            map.den_sigma(),
            map.level_base(),
            map.level_step(),
            map.quant_step(),
        ];
        actual.extend(map.continuous_values().unwrap().iter().map(|v| *v as f64));
        for z in 0..counts[2] {
            for y in 0..counts[1] {
                for x in 0..counts[0] {
                    actual.push(map.at_byte(x as isize, y as isize, z as isize) as f64);
                }
            }
        }
        for z in [-0.2, 0.4, 1.2, 4.] {
            for y in [-0.1, 0.7, 1.1, 2.] {
                for x in [-1.1, -0.2, 0.3, 3.] {
                    actual.push(map.value_at(map.conv_to_orth(Vector4::new3(x, y, z))));
                }
            }
        }
        assert_eq!(expected.len(), actual.len());
        let scale = values
            .iter()
            .copied()
            .map(f64::abs)
            .fold(1e-30_f64, f64::max);
        for (i, (a, b)) in actual.iter().zip(&expected).enumerate() {
            if case == 1 && i == 3 && b.is_nan() {
                assert_eq!(
                    *a, 0.,
                    "uniform-map variance is clamped before taking its square root"
                );
                continue;
            }
            assert!(
                (a - b).abs() <= 1e-10 * b.abs().max(scale),
                "case {case}, sample {i}: Rust {a}, C++ {b}"
            );
        }
        let expected_json = lines.collect::<Vec<_>>().join("\n");
        let expected_json = if case == 1 {
            expected_json.replace("-nan", "0.0").replace("nan", "0.0")
        } else {
            expected_json
        };
        let expected: serde_json::Value = serde_json::from_str(&expected_json).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&map.get_histogram_json(
            map.den_min() - 0.1,
            map.den_max() + 0.1,
            31,
        ))
        .unwrap();
        assert_eq!(actual, expected, "case {case} histogram");
    }
    eprintln!("Compared 5 continuous-potential cases with C++ (samples, interpolation, statistics, histogram)");
}
