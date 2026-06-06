use cuemol_molstr::read_ccp4_map_with_header_bytes;

#[test]
fn ccp4_map_reader_reads_float_map_values() {
    let map = read_ccp4_map_with_header_bytes(&ccp4_f32_bytes(
        [2, 1, 1],
        [2, 1, 1],
        [1, 2, 3],
        [1.0, 2.0],
        false,
    ))
    .unwrap();

    assert_eq!(map.grid.counts, [2, 1, 1]);
    assert_eq!(map.grid.value_count(), 2);
    assert_eq!(map.grid.values, vec![1.0, 2.0]);
    assert!((map.grid.delta[0].x - 1.0).abs() < 1.0e-6);
    assert_eq!(map.header.mode, 2);
    assert!(!map.header.byte_swapped);
}

#[test]
fn ccp4_map_reader_reads_big_endian_values() {
    let map = read_ccp4_map_with_header_bytes(&ccp4_f32_bytes(
        [1, 1, 1],
        [1, 1, 1],
        [1, 2, 3],
        [3.5],
        true,
    ))
    .unwrap();

    assert_eq!(map.grid.values, vec![3.5]);
    assert!(map.header.byte_swapped);
}

#[test]
fn ccp4_map_reader_rotates_axis_order_like_reference() {
    let map = read_ccp4_map_with_header_bytes(&ccp4_f32_bytes(
        [1, 2, 1],
        [2, 1, 1],
        [2, 1, 3],
        [10.0, 20.0],
        false,
    ))
    .unwrap();

    assert_eq!(map.header.raw_counts, [1, 2, 1]);
    assert_eq!(map.grid.counts, [2, 1, 1]);
    assert_eq!(map.grid.values, vec![10.0, 20.0]);
}

fn ccp4_f32_bytes(
    raw_counts: [i32; 3],
    grid_intervals: [i32; 3],
    axis_order: [i32; 3],
    values: impl IntoIterator<Item = f32>,
    swapped: bool,
) -> Vec<u8> {
    let values = values.into_iter().collect::<Vec<_>>();
    let mut bytes = vec![0_u8; 1024];
    write_i32(&mut bytes, 0, raw_counts[0], swapped);
    write_i32(&mut bytes, 1, raw_counts[1], swapped);
    write_i32(&mut bytes, 2, raw_counts[2], swapped);
    write_i32(&mut bytes, 3, 2, swapped);
    write_i32(&mut bytes, 7, grid_intervals[0], swapped);
    write_i32(&mut bytes, 8, grid_intervals[1], swapped);
    write_i32(&mut bytes, 9, grid_intervals[2], swapped);
    write_f32(&mut bytes, 10, 2.0, swapped);
    write_f32(&mut bytes, 11, 1.0, swapped);
    write_f32(&mut bytes, 12, 1.0, swapped);
    write_f32(&mut bytes, 13, 90.0, swapped);
    write_f32(&mut bytes, 14, 90.0, swapped);
    write_f32(&mut bytes, 15, 90.0, swapped);
    write_i32(&mut bytes, 16, axis_order[0], swapped);
    write_i32(&mut bytes, 17, axis_order[1], swapped);
    write_i32(&mut bytes, 18, axis_order[2], swapped);
    let min = values.iter().copied().fold(f32::INFINITY, f32::min);
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    write_f32(&mut bytes, 19, min, swapped);
    write_f32(&mut bytes, 20, max, swapped);
    write_f32(&mut bytes, 21, mean, swapped);
    write_i32(&mut bytes, 22, 1, swapped);
    write_i32(&mut bytes, 23, 0, swapped);
    bytes[208..212].copy_from_slice(b"MAP ");
    write_f32(&mut bytes, 54, 0.0, swapped);

    for value in values {
        if swapped {
            bytes.extend_from_slice(&value.to_be_bytes());
        } else {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

fn write_i32(bytes: &mut [u8], word: usize, value: i32, swapped: bool) {
    let encoded = if swapped {
        value.to_be_bytes()
    } else {
        value.to_le_bytes()
    };
    bytes[word * 4..word * 4 + 4].copy_from_slice(&encoded);
}

fn write_f32(bytes: &mut [u8], word: usize, value: f32, swapped: bool) {
    let encoded = if swapped {
        value.to_be_bytes()
    } else {
        value.to_le_bytes()
    };
    bytes[word * 4..word * 4 + 4].copy_from_slice(&encoded);
}
