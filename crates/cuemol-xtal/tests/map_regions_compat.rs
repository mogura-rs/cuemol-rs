use cuemol_core::{Matrix4, Vector4};
use cuemol_xtal::{lod_step_for_budget, DensityMap, MapRegionMode, MapRenderer};

fn map_case(kind: usize) -> (DensityMap, Matrix4) {
    let mut map = DensityMap::new();
    map.set_map_byte_array(&vec![128; 33 * 35 * 37], [33, 35, 37], 0., 256., 128., 10.);
    map.set_map_params([2, -3, 5], [40; 3]);
    map.set_xtal_params(
        40.,
        48.,
        56.,
        if kind > 0 { 83. } else { 90. },
        if kind > 0 { 77. } else { 90. },
        if kind > 0 { 120. } else { 90. },
    );
    map.set_origin(Vector4::new3(7., -5., 3.));
    let transform = if kind == 2 {
        Matrix4 {
            values: [
                [0., -2., 0., 20.],
                [2., 0., 0., -10.],
                [0., 0., 1.5, 5.],
                [0., 0., 0., 1.],
            ],
        }
    } else {
        Matrix4::IDENTITY
    };
    (map, transform)
}

#[test]
fn full_map_regions_align_sampling_and_keep_stable_view_coverage() {
    let (map, _) = map_case(0);
    let mut renderer = MapRenderer::new();
    renderer.set_region_mode(MapRegionMode::Full);
    renderer.set_lod(4);
    renderer.set_view_box(Vector4::new3(20., 10., 24.), 5.);
    let current = renderer.full_region(&map, None).unwrap();
    assert_eq!(current.step, 4);
    assert!(!current.periodic);
    for i in 0..3 {
        assert_eq!(current.start[i] % 4, 0);
        assert_eq!((current.end[i] - 1) % 4, 0);
    }
    renderer.set_view_box(Vector4::new3(20.1, 10., 24.), 5.);
    assert!(!renderer.view_region_requires_rebuild(&map, None, current));
    renderer.set_view_box(Vector4::new3(35., 10., 24.), 5.);
    assert!(renderer.view_region_requires_rebuild(&map, None, current));
    renderer.set_view_box(Vector4::new3(1000., 1000., 1000.), 5.);
    assert!(!renderer.view_region_requires_rebuild(&map, None, current));
    renderer.set_zoom_refine(false);
    let full = renderer.full_region(&map, None).unwrap();
    assert_eq!(full.start, [0; 3]);
    assert_eq!(full.end, [33, 33, 37]);
    assert_eq!(lod_step_for_budget([512; 3], 16 << 20), 2);
}

#[test]
fn skewed_transformed_regions_and_lod_match_cpp() {
    let Some(oracle) = std::env::var_os("CUEMOL_MAP_REGION_ORACLE") else {
        eprintln!(
            "Skipping C++ map-region oracle: set CUEMOL_MAP_REGION_ORACLE and CUEMOL_SCALAR_CONFIG"
        );
        return;
    };
    let config = std::env::var_os("CUEMOL_SCALAR_CONFIG").expect("oracle config");
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("regions.txt");
    assert!(std::process::Command::new(oracle)
        .args([config.as_os_str(), output.as_os_str()])
        .status()
        .unwrap()
        .success());
    let expected = std::fs::read_to_string(output)
        .unwrap()
        .lines()
        .map(|l| {
            l.split_whitespace()
                .map(|x| x.parse::<isize>().unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut actual = Vec::new();
    for kind in 0..3 {
        let (map, transform) = map_case(kind);
        let mut renderer = MapRenderer::new();
        renderer.set_region_mode(MapRegionMode::Full);
        renderer.set_world_transform(transform);
        for (phase, half) in [100., 3., 8., 0., 5., 5., 5.].into_iter().enumerate() {
            renderer.set_view_box(
                if phase == 3 {
                    Vector4::new3(1000., 1000., 1000.)
                } else {
                    Vector4::new3(14., 10., 24.)
                },
                half,
            );
            renderer.set_zoom_refine(phase != 5);
            renderer.set_lod(if phase == 4 {
                4
            } else if phase == 6 {
                8
            } else {
                0
            });
            let region = renderer.full_region(&map, None).unwrap();
            actual.push(vec![
                region.start[0],
                region.end[0],
                region.start[1],
                region.end[1],
                region.start[2],
                region.end[2],
                region.step as isize,
            ]);
        }
    }
    for n in [1, 33, 256, 512, 10000] {
        for budget in [1, 1048576, 16777216] {
            actual.push(vec![lod_step_for_budget([n, n + 2, n + 4], budget) as isize]);
        }
    }
    assert_eq!(actual, expected);
    eprintln!("Compared 21 map regions and 15 LOD budgets with C++");
}
