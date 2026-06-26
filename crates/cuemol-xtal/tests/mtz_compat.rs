use cuemol_xtal::{read_mtz_bytes, Mtz2MapReader};

#[test]
fn mtz_reader_parses_header_records() {
    let mtz = read_mtz_bytes(&mtz_bytes()).unwrap();

    assert_eq!(mtz.header_start_word, 21);
    assert_eq!(mtz.column_count, 5);
    assert_eq!(mtz.reflection_count, 10);
    assert_eq!(mtz.columns.len(), 5);
    assert_eq!(mtz.columns[0].name, "H");
    assert_eq!(mtz.columns[3].kind, 'F');
    assert_eq!(mtz.cell.unwrap(), [10.0, 11.0, 12.0, 90.0, 90.0, 90.0]);
    assert_eq!(mtz.space_group, Some(1));
    assert!((mtz.resolution.unwrap().max - 2.0).abs() < 1.0e-6);
    assert!(mtz.reflections.is_empty());
}

#[test]
fn mtz_reader_reads_reflection_body_records() {
    let mtz = read_mtz_bytes(&mtz_bytes_with_body()).unwrap();

    assert_eq!(mtz.header_start_word, 31);
    assert_eq!(mtz.column_count, 5);
    assert_eq!(mtz.reflection_count, 2);
    assert_eq!(mtz.reflections.len(), 2);
    assert_eq!(mtz.reflections[0].values, vec![1.0, 2.0, 3.0, 10.0, 90.0]);
    assert_eq!(mtz.reflections[1].values, vec![4.0, 5.0, 6.0, 20.0, 180.0]);
    assert_eq!(mtz.column_index("fwt"), Some(3));
    assert_eq!(mtz.reflection_hkl(1), Some((4, 5, 6)));
    assert_eq!(mtz.reflection_value(0, "PHWT"), Some(90.0));
    assert_eq!(mtz.column_values("FWT"), Some(vec![10.0, 20.0]));
}

#[test]
fn mtz2_map_reader_state_and_column_info_match_reference_shape() {
    let mut reader = Mtz2MapReader::new();
    assert_eq!(reader.name(), "mtzmap");
    assert_eq!(reader.type_descr(), "MTZ Structure Factor (*.mtz)");
    assert_eq!(reader.file_ext(), "*.mtz");
    assert_eq!(reader.grid_size(), 0.33);
    assert_eq!(reader.resolution(), -1.0);
    assert_eq!(reader.column_info_json(), "[]");

    reader.read_bytes(&mtz_bytes_with_body()).unwrap();
    assert_eq!(reader.min_res(), 0.0);
    assert_eq!(reader.max_res(), 0.0);
    assert_eq!(reader.selected_f(), Some(3));
    assert_eq!(reader.selected_phi(), Some(4));
    assert_eq!(
        reader.column_info_json(),
        "[{\"nid\":0, \"name\": \"H\", \"type\": \"H\"},{\"nid\":1, \"name\": \"K\", \"type\": \"H\"},{\"nid\":2, \"name\": \"L\", \"type\": \"H\"},{\"nid\":3, \"name\": \"FWT\", \"type\": \"F\"},{\"nid\":4, \"name\": \"PHWT\", \"type\": \"P\"}]"
    );
}

#[test]
fn mtz2_map_reader_honors_requested_columns() {
    let mut reader = Mtz2MapReader::new();
    reader.set_column_f("FWT");
    reader.set_column_phi("PHWT");
    reader.set_column_weight("FOM");
    reader.read_bytes(&mtz_bytes_with_weight()).unwrap();

    assert_eq!(reader.selected_f(), Some(3));
    assert_eq!(reader.selected_phi(), Some(4));
    assert_eq!(reader.selected_weight(), Some(5));
}

#[test]
fn build_density_map_synthesizes_finite_map_from_synthetic_mtz() {
    // Exercise the real public pipeline end-to-end: read_bytes -> column
    // selection (guess_fft_columns) -> build_density_map. The standalone
    // density_map_from_mtz unit tests feed a hand-built MtzFile and never reach
    // this entry point, so this is the only coverage of read_bytes + build wired
    // together. mtz_bytes_with_body() already carries a CELL record (required;
    // build_density_map errors on a missing cell) and no SYMINF, so the space
    // group falls back to P1.
    let mut reader = Mtz2MapReader::new();
    reader.read_bytes(&mtz_bytes_with_body()).unwrap();

    let map = reader.build_density_map().unwrap();

    // Stats must be finite, and the synthesized map must actually carry signal:
    // two reflections with non-trivial phases give a non-constant density, so
    // den_max strictly exceeds den_min and the RMSD is positive. den_sigma == 0
    // would mean a degenerate/empty grid slipped through.
    assert!(map.den_min().is_finite());
    assert!(map.den_max().is_finite());
    assert!(map.den_mean().is_finite());
    assert!(map.den_sigma().is_finite());
    assert!(map.den_max() > map.den_min());
    assert!(map.den_sigma() > 0.0);
}

fn mtz_bytes() -> Vec<u8> {
    let mut bytes = vec![0_u8; 80];
    bytes[0..4].copy_from_slice(b"MTZ ");
    bytes[4..8].copy_from_slice(&(21_i32).to_le_bytes());
    bytes[8..12].copy_from_slice(&[0x44, 0x44, 0, 0]);
    for record in [
        "NCOL 5 10 0",
        "CELL 10 11 12 90 90 90",
        "COL H H",
        "COL K H",
        "COL L H",
        "COL FWT F",
        "COL PHWT P",
        "SYMINF 1 1 P 1 'P 1'",
        "RESO 0.010000 0.250000",
        "END",
    ] {
        append_record(&mut bytes, record);
    }
    bytes
}

fn mtz_bytes_with_body() -> Vec<u8> {
    let mut bytes = vec![0_u8; 80];
    bytes[0..4].copy_from_slice(b"MTZ ");
    bytes[4..8].copy_from_slice(&(31_i32).to_le_bytes());
    bytes[8..12].copy_from_slice(&[0x44, 0x44, 0, 0]);
    for value in [1.0_f32, 2.0, 3.0, 10.0, 90.0, 4.0, 5.0, 6.0, 20.0, 180.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for record in [
        "NCOL 5 2 0",
        "CELL 10 11 12 90 90 90",
        "COL H H",
        "COL K H",
        "COL L H",
        "COL FWT F",
        "COL PHWT P",
        "END",
    ] {
        append_record(&mut bytes, record);
    }
    bytes
}

fn mtz_bytes_with_weight() -> Vec<u8> {
    let mut bytes = vec![0_u8; 80];
    bytes[0..4].copy_from_slice(b"MTZ ");
    bytes[4..8].copy_from_slice(&(21_i32).to_le_bytes());
    bytes[8..12].copy_from_slice(&[0x44, 0x44, 0, 0]);
    for record in [
        "NCOL 6 0 0",
        "COL H H",
        "COL K H",
        "COL L H",
        "COL FWT F",
        "COL PHWT P",
        "COL FOM W",
        "END",
    ] {
        append_record(&mut bytes, record);
    }
    bytes
}

fn append_record(bytes: &mut Vec<u8>, record: &str) {
    let mut padded = [b' '; 80];
    padded[..record.len()].copy_from_slice(record.as_bytes());
    bytes.extend_from_slice(&padded);
}
