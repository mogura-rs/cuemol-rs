use cuemol_core::Vector4;
use cuemol_surface::SurfaceMesh;
use cuemol_xtal::{DensityMap, MapCapMode, MapRegionMode, MapSurfRenderer};

fn fixture(mask: usize) -> DensityMap {
    let values = (0..8)
        .map(|i| if mask & (1 << i) != 0 { 64 } else { 192 })
        .collect::<Vec<_>>();
    let mut map = DensityMap::new();
    map.set_map_byte_array(&values, [2; 3], 0., 256., 128., 64.);
    map.set_map_params([0; 3], [2; 3]);
    map.set_xtal_params(2., 2., 2., 90., 90., 90.);
    map
}
fn renderer() -> MapSurfRenderer {
    let mut renderer = MapSurfRenderer::new();
    renderer.map_mut().set_region_mode(MapRegionMode::Full);
    renderer.map_mut().set_use_abslevel(true);
    renderer.map_mut().set_siglevel(2.);
    renderer
}
fn area(mesh: &SurfaceMesh) -> f64 {
    mesh.triangles()
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| mesh.vertices()[i as usize].position);
            (b - a).cross(c - a).length3d() * 0.5
        })
        .sum()
}
#[test]
fn caps_follow_region_policy_and_preserve_boundary_clipping() {
    let map = fixture(0);
    let mut r = renderer();
    let cube = r.display_surface(&map, None);
    assert_eq!(cube.mesh().triangle_count(), 12);
    assert!((area(cube.mesh()) - 6.).abs() < 1e-10);
    let center = Vector4::new3(0.5, 0.5, 0.5);
    for v in cube.mesh().vertices() {
        assert!(v.normal.dot3d(v.position - center) > 0.49);
    }
    r.map_mut().set_region_mode(MapRegionMode::Box);
    r.map_mut().set_use_pbc(false);
    assert_eq!(r.display_surface(&map, None).mesh().triangle_count(), 0);
    assert_eq!(r.generate_surf_obj_from(&map).mesh().triangle_count(), 12);
    r.set_cap_mode(MapCapMode::On);
    assert_eq!(r.display_surface(&map, None).mesh().triangle_count(), 12);
    r.set_cap_mode(MapCapMode::Off);
    assert_eq!(r.generate_surf_obj_from(&map).mesh().triangle_count(), 0);
    r.set_cap_mode(MapCapMode::On);
    r.map_mut().set_bndry_rng(0.1);
    assert_eq!(
        r.display_surface(&map, Some(&[Vector4::ZERO]))
            .mesh()
            .triangle_count(),
        0
    );
    // Exercise indexed boundary lookup with the same nearest-distance condition.
    let points = (0..64)
        .map(|i| Vector4::new3(i as f64 * 10., 0., 0.))
        .collect::<Vec<_>>();
    assert_eq!(
        r.display_surface(&map, Some(&points))
            .mesh()
            .triangle_count(),
        0
    );
    r.map_mut().set_bndry_rng(2.);
    assert_eq!(
        r.display_surface(&map, Some(&points))
            .mesh()
            .triangle_count(),
        12
    );
}
#[test]
fn all_cube_cap_counts_and_areas_match_cpp() {
    let Some(oracle) = std::env::var_os("CUEMOL_MAP_CAP_ORACLE") else {
        eprintln!(
            "Skipping C++ map-cap oracle: set CUEMOL_MAP_CAP_ORACLE and CUEMOL_SCALAR_CONFIG"
        );
        return;
    };
    let config = std::env::var_os("CUEMOL_SCALAR_CONFIG").expect("oracle config");
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("caps.txt");
    assert!(std::process::Command::new(oracle)
        .args([config.as_os_str(), output.as_os_str()])
        .status()
        .unwrap()
        .success());
    let expected = std::fs::read_to_string(output).unwrap();
    assert_eq!(expected.lines().count(), 256);
    for (mask, line) in expected.lines().enumerate() {
        let numbers = line
            .split_whitespace()
            .map(|x| x.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        let map = fixture(mask);
        let mut r = renderer();
        r.set_cap_mode(MapCapMode::Off);
        let open = r.generate_surf_obj_from(&map);
        r.set_cap_mode(MapCapMode::On);
        let closed = r.generate_surf_obj_from(&map);
        assert_eq!(
            closed.mesh().triangle_count() - open.mesh().triangle_count(),
            numbers[0] as usize,
            "mask {mask}"
        );
        assert!(
            (area(closed.mesh()) - area(open.mesh()) - numbers[1]).abs() < 1e-6,
            "mask {mask}"
        );
    }
}
