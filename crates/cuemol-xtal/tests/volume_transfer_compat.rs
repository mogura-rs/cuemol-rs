use cuemol_xtal::{DensityMap, GlslMapVolRenderer};

#[test]
fn volume_transfer_tables_match_the_cpp_texture_upload() {
    let Some(oracle) = std::env::var_os("CUEMOL_VOLUME_ORACLE") else {
        eprintln!("Skipping C++ transfer oracle: run scripts/check-reference-volume.py");
        return;
    };
    let path =
        std::env::temp_dir().join(format!("cuemol-volume-oracle-{}.txt", std::process::id()));
    assert!(std::process::Command::new(oracle)
        .arg(&path)
        .status()
        .unwrap()
        .success());
    let output = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    let expected = output
        .split_whitespace()
        .map(|v| v.parse::<u8>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 3 * 5 * 256 * 4);
    let mut map = DensityMap::new();
    map.set_map_byte_array(
        &(0..=255).collect::<Vec<_>>(),
        [256, 1, 1],
        0.,
        256.,
        127.5,
        1.,
    );
    let mut renderer = GlslMapVolRenderer::new();
    let mut offset = 0;
    for mode in 0..3 {
        renderer.set_xferfn_type(mode);
        for level in [0, 64, 128, 254, 255] {
            renderer.set_siglevel(f64::from(level));
            for (index, rgba) in renderer.transfer_function(&map).iter().enumerate() {
                assert_eq!(
                    rgba.as_slice(),
                    &expected[offset..offset + 4],
                    "mode {mode}, level {level}, byte {index}"
                );
                offset += 4;
            }
        }
    }
    eprintln!("Compared all 256 colors in 15 transfer tables with unchanged C++ function output");
}
