use cuemol_mdtools::{read_amber_netcdf_file, read_trr_file, read_xtc_file};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn reads_binary_trajectories_written_by_mdconvert() {
    if Command::new("mdconvert").arg("--help").output().is_err() {
        return;
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("cuemol-gromacs-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let pdb_path = directory.join("input.pdb");
    let xtc_path = directory.join("output.xtc");
    let trr_path = directory.join("output.trr");
    let netcdf_path = directory.join("output.nc");

    fs::write(&pdb_path, TEN_ATOM_PDB).unwrap();

    for output_path in [&xtc_path, &trr_path] {
        let output = Command::new("mdconvert")
            .arg("-o")
            .arg(output_path)
            .arg(&pdb_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new("mdconvert")
        .arg("-o")
        .arg(&netcdf_path)
        .arg("-t")
        .arg(&pdb_path)
        .arg(&pdb_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let xtc = read_xtc_file(&xtc_path).unwrap();
    let trr = read_trr_file(&trr_path).unwrap();
    let netcdf = read_amber_netcdf_file(&netcdf_path).unwrap();
    assert_eq!(xtc.atom_count(), 10);
    assert_eq!(trr.atom_count(), 10);
    assert_eq!(netcdf.atom_count(), 10);
    assert_eq!(xtc.frame_count(), 1);
    assert_eq!(trr.frame_count(), 1);
    assert_eq!(netcdf.frame_count(), 1);
    for (actual, expected) in xtc.frame(0).unwrap()[..3].iter().zip([17.05, 14.1, 3.63]) {
        assert!((actual - expected).abs() < 1.0e-5);
    }
    for (actual, expected) in trr.frame(0).unwrap()[..3]
        .iter()
        .zip([17.047, 14.099, 3.625])
    {
        assert!((actual - expected).abs() < 1.0e-5);
    }
    for (actual, expected) in netcdf.frame(0).unwrap()[..3]
        .iter()
        .zip([17.047, 14.099, 3.625])
    {
        assert!((actual - expected).abs() < 1.0e-5);
    }

    fs::remove_dir_all(directory).unwrap();
}

const TEN_ATOM_PDB: &str = "\
ATOM      1  N   THR A   1      17.047  14.099   3.625  1.00 13.79           N
ATOM      2  CA  THR A   1      16.967  12.784   4.338  1.00 10.80           C
ATOM      3  C   THR A   1      15.685  12.755   5.133  1.00  9.19           C
ATOM      4  O   THR A   1      15.268  13.825   5.594  1.00  9.85           O
ATOM      5  CB  THR A   1      18.170  12.703   5.337  1.00 13.02           C
ATOM      6  OG1 THR A   1      19.334  12.829   4.463  1.00 15.06           O
ATOM      7  CG2 THR A   1      18.150  11.546   6.304  1.00 14.23           C
ATOM      8  N   THR A   2      15.115  11.555   5.265  1.00  7.81           N
ATOM      9  CA  THR A   2      13.856  11.469   6.066  1.00  8.31           C
ATOM     10  C   THR A   2      14.164  10.785   7.379  1.00  5.80           C
END
";

#[test]
fn indexed_random_frames_match_external_mdtraj_coordinates() {
    if Command::new("mdconvert").arg("--help").output().is_err() {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let pdb = directory.path().join("models.pdb");
    let mut models = String::new();
    for frame in 0..9 {
        models.push_str(&format!("MODEL     {:4}\n", frame + 1));
        for line in TEN_ATOM_PDB.lines().filter(|l| l.starts_with("ATOM")) {
            let x: f64 = line[30..38].trim().parse().unwrap();
            models.push_str(&format!(
                "{}{:8.3}{}\n",
                &line[..30],
                x + frame as f64 * 0.125,
                &line[38..]
            ));
        }
        models.push_str("ENDMDL\n");
    }
    fs::write(&pdb, models).unwrap();
    for extension in ["dcd", "xtc", "trr", "nc"] {
        let path = directory.path().join(format!("trajectory.{extension}"));
        let output = Command::new("mdconvert")
            .arg("-o")
            .arg(&path)
            .arg("-t")
            .arg(&pdb)
            .arg(&pdb)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let indexed = cuemol_mdtools::IndexedTrajectory::open(&path, 2, Some(10)).unwrap();
        assert_eq!(indexed.frame_count(), 5);
        for frame in [4, 0, 2, 1, 3] {
            let actual = indexed.read_frame(frame).unwrap();
            assert_eq!(actual.coordinates.len(), 30);
            let expected = 17.047 + frame as f32 * 0.25;
            let tolerance = if extension == "xtc" { 0.006 } else { 0.0001 };
            assert!(
                (actual.coordinates[0] - expected).abs() < tolerance,
                "{extension} frame {frame}: {} != {expected}",
                actual.coordinates[0]
            );
        }
    }
}
