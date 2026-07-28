use std::fs;
use std::process::Command;

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_name(output: &mut Vec<u8>, value: &str) {
    push_u32(output, value.len() as u32);
    output.extend_from_slice(value.as_bytes());
    output.resize(output.len().next_multiple_of(4), 0);
}

fn push_char_attribute(output: &mut Vec<u8>, name: &str, value: &str) {
    push_name(output, name);
    push_u32(output, 2);
    push_u32(output, value.len() as u32);
    output.extend_from_slice(value.as_bytes());
    output.resize(output.len().next_multiple_of(4), 0);
}

fn fixture() -> Vec<u8> {
    let atom_count = 2_usize;
    let frame_count = 3_usize;
    let mut output = b"CDF\x02".to_vec();
    push_u32(&mut output, frame_count as u32);

    push_u32(&mut output, 10);
    push_u32(&mut output, 5);
    for (name, size) in [
        ("spatial", 3),
        ("atom", atom_count as u32),
        ("frame", 0),
        ("cell_spatial", 3),
        ("cell_angular", 3),
    ] {
        push_name(&mut output, name);
        push_u32(&mut output, size);
    }
    push_u32(&mut output, 12);
    push_u32(&mut output, 1);
    push_char_attribute(&mut output, "Conventions", "AMBER");

    push_u32(&mut output, 11);
    push_u32(&mut output, 3);
    let mut begin_positions = Vec::new();
    for (name, dimensions, units, byte_size) in [
        (
            "coordinates",
            vec![2_u32, 1, 0],
            "angstrom",
            atom_count * 12,
        ),
        ("cell_lengths", vec![2, 3], "angstrom", 12),
        ("cell_angles", vec![2, 4], "degree", 12),
    ] {
        push_name(&mut output, name);
        push_u32(&mut output, dimensions.len() as u32);
        for dimension in dimensions {
            push_u32(&mut output, dimension);
        }
        push_u32(&mut output, 12);
        push_u32(&mut output, 1);
        push_char_attribute(&mut output, "units", units);
        push_u32(&mut output, 5);
        push_u32(&mut output, byte_size as u32);
        begin_positions.push(output.len());
        push_u64(&mut output, 0);
    }
    let header_size = output.len() as u64;
    let begins = [
        header_size,
        header_size + (atom_count * 12) as u64,
        header_size + (atom_count * 12 + 12) as u64,
    ];
    for (position, begin) in begin_positions.into_iter().zip(begins) {
        output[position..position + 8].copy_from_slice(&begin.to_be_bytes());
    }

    for frame in 0..frame_count {
        for atom in 0..atom_count {
            for axis in 0..3 {
                let value = (frame * 100 + atom * 10 + axis) as f32 / 10.0;
                output.extend_from_slice(&value.to_be_bytes());
            }
        }
        for value in [10.0_f32, 11.0, 12.0, 90.0, 90.0, 90.0] {
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
    output
}

#[test]
fn ambernetcdf_info_reports_filtered_trajectory_metadata() {
    let directory = std::env::temp_dir().join(format!("cuemol-cli-netcdf-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("trajectory.nc");
    fs::write(&path, fixture()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .arg("ambernetcdf-info")
        .arg(&path)
        .arg("2")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("frames=2"));
    assert!(stdout.contains("atoms=2"));
    assert!(stdout.contains("has_cell=true"));
    assert!(stdout.contains("first=0.000000,0.100000,0.200000"));

    fs::remove_dir_all(directory).unwrap();
}
