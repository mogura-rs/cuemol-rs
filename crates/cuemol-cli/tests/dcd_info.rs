use std::fs;
use std::process::Command;

fn push_record(output: &mut Vec<u8>, payload: &[u8]) {
    output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    output.extend_from_slice(payload);
    output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
}

fn dcd_fixture() -> Vec<u8> {
    let mut output = Vec::new();
    let mut header = vec![0_u8; 84];
    header[..4].copy_from_slice(b"CORD");
    header[4..8].copy_from_slice(&2_u32.to_le_bytes());
    push_record(&mut output, &header);

    let mut title = vec![0_u8; 84];
    title[..4].copy_from_slice(&1_u32.to_le_bytes());
    push_record(&mut output, &title);
    push_record(&mut output, &2_u32.to_le_bytes());

    for frame in 0..2 {
        for axis in 0..3 {
            let values = [
                (frame * 100 + axis * 10) as f32,
                (frame * 100 + axis * 10 + 1) as f32,
            ];
            let mut payload = Vec::new();
            for value in values {
                payload.extend_from_slice(&value.to_le_bytes());
            }
            push_record(&mut output, &payload);
        }
    }
    output
}

#[test]
fn dcd_info_reports_filtered_trajectory_metadata() {
    let path = std::env::temp_dir().join(format!("cuemol-cli-{}.dcd", std::process::id()));
    fs::write(&path, dcd_fixture()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .arg("dcd-info")
        .arg(&path)
        .arg("2")
        .output()
        .unwrap();
    let _ = fs::remove_file(path);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        "frames=1\natoms=2\nhas_cell=false\nfirst=0.000000,10.000000,20.000000\n"
    );
}
