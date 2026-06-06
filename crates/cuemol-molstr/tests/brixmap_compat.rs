use cuemol_molstr::read_brix_map_with_header_bytes;

#[test]
fn brix_map_reader_reads_ascii_header_and_density_block() {
    let map = read_brix_map_with_header_bytes(&brix_bytes()).unwrap();

    assert_eq!(map.grid.counts, [2, 1, 1]);
    assert_eq!(map.header.start, [0, 0, 0]);
    assert_eq!(map.header.end, [2, 1, 1]);
    assert_eq!(map.header.prod, 2.0);
    assert_eq!(map.header.plus, 10.0);
    assert!(!map.header.dsn6);
    assert_eq!(map.grid.values, vec![0.0, 1.0]);
    assert!((map.grid.delta[0].x - 1.0).abs() < 1.0e-6);
}

fn brix_bytes() -> Vec<u8> {
    let header =
        ":-) origin 0 0 0 extent 2 1 1 grid 2 1 1 cell 2 1 1 90 90 90 prod 2 plus 10 sigma 1";
    let mut bytes = vec![b' '; 512];
    bytes[..header.len()].copy_from_slice(header.as_bytes());
    let mut block = vec![0_u8; 512];
    block[0] = 10;
    block[1] = 12;
    bytes.extend_from_slice(&block);
    bytes
}
