use cuemol_mdtools::{
    read_amber_netcdf_bytes_with_options, read_dcd_bytes_with_options, read_trr_bytes_with_options,
    read_xtc_bytes_with_options, AmberNetCDFReadOptions, DcdReadOptions, TrrReadOptions,
    XtcReadOptions,
};
use std::error::Error;
use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 6 {
        return Err(
            "usage: trajectory_reader_bench INPUT ATOMS ITERATIONS all|selected|error SKIP".into(),
        );
    }
    let path = Path::new(&args[1]);
    let atoms: usize = args[2].parse()?;
    let iterations: usize = args[3].parse()?;
    let mode = args[4].as_str();
    let skip: usize = args[5].parse()?;
    if atoms == 0 || iterations == 0 || skip == 0 || !matches!(mode, "all" | "selected" | "error") {
        return Err("counts must be positive and mode must be all, selected or error".into());
    }
    let input = std::fs::read(path)?;
    let selected = (mode == "selected").then(|| vec![atoms - 1, 0, atoms / 2, 0]);
    for _ in 0..iterations {
        let start = Instant::now();
        let result = match path.extension().and_then(|value| value.to_str()) {
            Some("dcd") => read_dcd_bytes_with_options(
                &input,
                &DcdReadOptions {
                    skip_no: skip,
                    expected_atom_count: Some(atoms),
                    atom_indices: selected.clone(),
                },
            ),
            Some("xtc") => read_xtc_bytes_with_options(
                &input,
                &XtcReadOptions {
                    skip_no: skip,
                    expected_atom_count: Some(atoms),
                    atom_indices: selected.clone(),
                },
            ),
            Some("trr") => read_trr_bytes_with_options(
                &input,
                &TrrReadOptions {
                    skip_no: skip,
                    expected_atom_count: Some(atoms),
                    atom_indices: selected.clone(),
                },
            ),
            Some("nc") => read_amber_netcdf_bytes_with_options(
                &input,
                &AmberNetCDFReadOptions {
                    skip_no: skip,
                    expected_atom_count: Some(atoms),
                    atom_indices: selected.clone(),
                },
            ),
            _ => return Err("expected a DCD, XTC, TRR or NetCDF input".into()),
        };
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if mode == "error" {
            assert!(result.is_err(), "malformed input was accepted");
            println!(
                "{{\"read_ms\":{elapsed},\"error\":true,\"hash\":\"0\",\"atoms\":0,\"frames\":0}}"
            );
            continue;
        }
        let block = result?;
        let mut hash = 0xcbf29ce484222325_u64;
        let mut add = |value: u64| {
            hash = (hash ^ value).wrapping_mul(0x100000001b3);
        };
        for frame in 0..block.frame_count() {
            for value in block.frame(frame).unwrap() {
                add(u64::from(value.to_bits()));
            }
            if let Some(cell) = block.cell(frame) {
                add(1);
                for value in cell {
                    add(value.to_bits());
                }
            } else {
                add(0);
            }
        }
        println!(
            "{{\"read_ms\":{elapsed},\"error\":false,\"hash\":\"{hash:016x}\",\"atoms\":{},\"frames\":{}}}",
            block.atom_count(),
            block.frame_count(),
        );
    }
    Ok(())
}
