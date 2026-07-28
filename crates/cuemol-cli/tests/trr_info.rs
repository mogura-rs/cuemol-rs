use std::fs;
use std::process::Command;

fn push_i32(output: &mut Vec<u8>, value: i32) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_gmx_string(output: &mut Vec<u8>, value: &str) {
    push_i32(output, value.len() as i32 + 1);
    push_i32(output, value.len() as i32);
    output.extend_from_slice(value.as_bytes());
    output.resize(output.len().next_multiple_of(4), 0);
}

fn push_frame(output: &mut Vec<u8>, frame: usize) {
    push_i32(output, 1993);
    push_gmx_string(output, "GMX_trn_file");
    for size in [0, 0, 36, 0, 0, 0, 0, 24, 0, 0] {
        push_i32(output, size);
    }
    push_i32(output, 2);
    push_i32(output, frame as i32);
    push_i32(output, 0);
    output.extend_from_slice(&(frame as f32).to_be_bytes());
    output.extend_from_slice(&0.0_f32.to_be_bytes());
    for value in [1.0_f32, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0] {
        output.extend_from_slice(&value.to_be_bytes());
    }
    for atom in 0..2 {
        for axis in 0..3 {
            let value = (frame * 100 + atom * 10 + axis) as f32 / 10.0;
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
}

fn trr_fixture() -> Vec<u8> {
    let mut output = Vec::new();
    push_frame(&mut output, 0);
    push_frame(&mut output, 1);
    output
}

#[test]
fn trr_info_reports_filtered_trajectory_metadata() {
    let path = std::env::temp_dir().join(format!("cuemol-cli-{}.trr", std::process::id()));
    fs::write(&path, trr_fixture()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .arg("trr-info")
        .arg(&path)
        .arg("2")
        .output()
        .unwrap();
    let _ = fs::remove_file(path);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        "frames=1\natoms=2\nhas_cell=true\nfirst=0.000000,1.000000,2.000000\n"
    );
}
