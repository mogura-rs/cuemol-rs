use cuemol_core::Uid;
use cuemol_qsys::View;

fn values(view: &View) -> Vec<f64> {
    let center = view.center();
    vec![
        view.distance(),
        view.slab(),
        view.zoom(),
        center.x,
        center.y,
        center.z,
    ]
}

#[test]
fn view_transform_matches_reference_distance_slab_zoom_and_pan() {
    let mut actual = Vec::new();
    for distance in [0., 0.05, 0.1, 10., 10000., 20000.] {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_distance(200.);
        view.set_slab(300.);
        view.set_distance(distance);
        assert_eq!(view.slab(), 300., "distance alone must not resize the slab");
        actual.push(values(&view));
        view.set_slab(400.);
        actual.push(values(&view));
        view.set_slab(25000.);
        assert!(view.slab() <= 10000.);
        actual.push(values(&view));
    }
    for zoom in [-1., 0., 0.005, 0.01, 100.] {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_zoom(zoom);
        actual.push(values(&view));
    }
    for axis in 0..3 {
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_view_size(800, 200);
        view.set_zoom(50.);
        view.rotate_view(30., 45., 90.);
        view.translate_view(
            if axis == 0 { 4. } else { 0. },
            if axis == 1 { 4. } else { 0. },
            if axis == 2 { 4. } else { 0. },
        );
        actual.push(values(&view));
    }
    let Some(oracle) = std::env::var_os("CUEMOL_VIEW_XFORM_ORACLE") else {
        eprintln!("C++ view transform oracle not configured; Rust regression ran");
        return;
    };
    let config = std::env::var_os("CUEMOL_CAMERA_FIT_CONFIG").expect("oracle configuration");
    let output = std::env::temp_dir().join(format!("cuemol-view-xform-{}.txt", std::process::id()));
    assert!(std::process::Command::new(oracle)
        .arg(config)
        .arg(&output)
        .status()
        .unwrap()
        .success());
    let text = std::fs::read_to_string(&output).unwrap();
    std::fs::remove_file(&output).unwrap();
    let expected: Vec<Vec<f64>> = text
        .lines()
        .map(|line| {
            line.split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect()
        })
        .collect();
    assert_eq!(actual.len(), expected.len());
    for (case, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1e-9,
                "case {case}: {actual} != {expected}"
            );
        }
    }
}
