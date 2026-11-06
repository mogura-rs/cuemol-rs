use cuemol_core::{Matrix4, Uid, Vector4};
use cuemol_qsys::View;
use cuemol_xtal::DensityMap;

#[test]
fn map_fit_boxes_match_cpp_with_origins_skew_cells_and_rotations() {
    let Some(oracle) = std::env::var_os("CUEMOL_MAP_FIT_BOX_ORACLE") else {
        eprintln!("skipping map fit-box oracle: run scripts/check-reference-camera-fit.py");
        return;
    };
    let config = std::env::var_os("CUEMOL_CAMERA_FIT_CONFIG").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("boxes.jsonl");
    assert!(std::process::Command::new(oracle)
        .arg(config)
        .arg(&output)
        .status()
        .unwrap()
        .success());
    let expected = std::fs::read_to_string(output).unwrap();
    let lines: Vec<_> = expected.lines().collect();
    assert_eq!(lines.len(), 16);
    for (i, line) in lines.into_iter().enumerate() {
        let mut map = DensityMap::new();
        map.set_map_byte_array(&[0; 8], [2; 3], 0., 1., 0.5, 0.25);
        map.set_map_params([-3, 2, 4], [10, 12, 14]);
        map.set_xtal_params(
            10.,
            12.,
            14.,
            90.,
            if i & 1 != 0 { 110. } else { 90. },
            if i & 1 != 0 { 80. } else { 90. },
        );
        if i & 2 != 0 {
            map.set_origin(Vector4::new3(3., -2., 5.));
        }
        let transform = if i & 4 != 0 {
            Matrix4::make_trans_mat(Vector4::new3(4., 5., 6.))
        } else {
            Matrix4::IDENTITY
        };
        let mut view = View::new(Uid::new(1), Uid::new(2));
        view.set_view_size(100, 200);
        if i & 8 != 0 {
            view.rotate_view(17., -31., 23.);
        }
        let before = view.camera();
        let (min, max) = map.get_fit_box_with_xform(&view, transform);
        assert_eq!(view.camera(), before);
        let reference: serde_json::Value = serde_json::from_str(line).unwrap();
        for (key, actual) in [("min", min), ("max", max)] {
            for (actual, reference) in [actual.x, actual.y, actual.z]
                .into_iter()
                .zip(reference[key].as_array().unwrap())
            {
                assert!(
                    (actual - reference.as_f64().unwrap()).abs() < 1e-10,
                    "case {i}: {key} {actual} != {reference}"
                );
            }
        }
    }
    eprintln!("compared 16 C++ density-map fit boxes");
}
