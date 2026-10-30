use cuemol_gfx::SolidColor;
use cuemol_qsys::StyleManager;
use serde_json::json;
use std::{env, hint::black_box, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: style_bench COLORS SNAPSHOTS".into());
    }
    let colors: usize = args[0].parse()?;
    let snapshots: usize = args[1].parse()?;
    if colors == 0 || snapshots == 0 {
        return Err("counts must be positive".into());
    }
    let mut manager = StyleManager::new();
    manager.load_builtin_default_styles(0);
    let uid = manager.create_style_set("user", 0);
    for index in 0..colors {
        manager.set_color(
            format!("color_{index}"),
            SolidColor::from_rgb_u8(64, 152, 183),
            0,
            uid,
        );
    }
    manager.set_str_data("sel", "focus", "name C1", 0, uid);
    manager.set_style_value(0, "user", "Custom.sphr", "0.4");
    manager.fire_pending_events();
    let start = Instant::now();
    let first = black_box(manager.clone());
    let first_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut retained = Vec::with_capacity(snapshots + 1);
    retained.push(first);
    let start = Instant::now();
    for index in 0..snapshots {
        manager.set_str_data("sel", "focus", format!("resid {}", index + 1), 0, uid);
        retained.push(black_box(manager.clone()));
    }
    let snapshot_ms = start.elapsed().as_secs_f64() * 1000. / snapshots as f64;
    if retained[0].get_str_data("sel", "focus", 0) != "name C1" {
        return Err("editing changed an earlier snapshot".into());
    }
    let mut digest = 0xcbf29ce484222325_u64;
    for (index, snapshot) in retained.iter().enumerate() {
        if index > 0 && snapshot.get_str_data("sel", "focus", 0) != format!("resid {index}") {
            return Err("snapshot selection changed".into());
        }
        for byte in snapshot.get_style_set(uid).unwrap().to_xml().bytes() {
            digest = (digest ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    println!(
        "{}",
        json!({"colors":colors,"snapshots":snapshots,"first_ms":first_ms,"snapshot_ms":snapshot_ms,"digest":format!("{digest:016x}")})
    );
    Ok(())
}
