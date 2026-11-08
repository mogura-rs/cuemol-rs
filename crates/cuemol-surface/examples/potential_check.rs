use cuemol_surface::{qdf::read_qdf_potential_bytes, OpenDxPotReader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        return Err("usage: potential_check INPUT_DX REFERENCE_QDF".into());
    }
    let map = OpenDxPotReader::new().read_file(&arguments[0])?;
    let reference = read_qdf_potential_bytes(&std::fs::read(&arguments[1])?)?;
    assert_eq!(map.col_no(), reference.col_no());
    assert_eq!(map.row_no(), reference.row_no());
    assert_eq!(map.sec_no(), reference.sec_no());
    for (value, expected) in [
        (map.col_grid_size(), reference.col_grid_size()),
        (map.row_grid_size(), reference.row_grid_size()),
        (map.sec_grid_size(), reference.sec_grid_size()),
        (map.origin().x, reference.origin().x),
        (map.origin().y, reference.origin().y),
        (map.origin().z, reference.origin().z),
    ] {
        assert_eq!((value as f32).to_bits(), (expected as f32).to_bits());
    }
    assert_eq!(map.value_count(), reference.value_count());
    let mut mismatches = 0;
    for (index, (value, expected)) in map.values().iter().zip(reference.values()).enumerate() {
        if (*value as f32).to_bits() != (*expected as f32).to_bits() {
            if mismatches < 4 {
                eprintln!("voxel {index}: Rust={value}, C++={expected}");
            }
            mismatches += 1;
        }
    }
    println!(
        "{{\"values\":{},\"mismatches\":{mismatches},\"matches_reference_f32\":{}}}",
        map.value_count(),
        mismatches == 0
    );
    if mismatches != 0 {
        return Err("potential values differ from the reference".into());
    }
    Ok(())
}
