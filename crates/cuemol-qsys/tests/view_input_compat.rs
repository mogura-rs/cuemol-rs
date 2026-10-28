use cuemol_core::Vector4;
use cuemol_qsys::view_input::{ViewInputController, LEFT, RIGHT};
use cuemol_qsys::{Camera, StyleManager, ViewInputConfig};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn camera_input_matches_cpp_for_presets_modifiers_and_momentum() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let oracle = std::env::var_os("CUEMOL2_INPUT_ORACLE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join(".cache/input-reference/oracle"));
    if !oracle.is_file() {
        eprintln!("C++ input oracle is unavailable; skipping reference comparison");
        return;
    }
    let config_path = std::env::var_os("CUEMOL2_INPUT_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join(".cache/backbone-reference/sysconfig.xml"));
    assert!(
        config_path.is_file(),
        "C++ configuration missing: set CUEMOL2_INPUT_CONFIG"
    );
    let mut commands = String::new();
    for style in [
        "DefaultViewInConf",
        "CootViewInConf",
        "PymolViewInConf",
        "IphoneViewInConf",
        "TrackpadViewInConf",
    ] {
        for (w, h, x, y, z, radius) in [
            (800, 600, 0, 0, 0, 0.8),
            (1200, 350, 25, -57, 93, 1.4),
            (401, 907, 190, 70, -40, 0.3),
        ] {
            for modifiers in [
                LEFT,
                RIGHT,
                16,
                LEFT | RIGHT,
                LEFT | 1,
                LEFT | 2,
                LEFT | 4,
                RIGHT | 1,
                RIGHT | 2,
            ] {
                commands += &format!("reset {style} {w} {h} {x} {y} {z} {radius}\n");
                for (px, py, dx, dy) in [
                    (200, 140, 11, 7),
                    (243, 99, 43, -41),
                    (344, 207, 101, 108),
                    (12, 640, -332, 433),
                ] {
                    commands += &format!("drag {modifiers} {px} {py} {dx} {dy}\n");
                }
                commands += "start\n";
                commands +=
                    &format!("drag {modifiers} 200 200 2 20\ndrag {modifiers} 231 202 31 2\n");
                for keys in [0, 1, 2, 3, 4] {
                    commands += &format!("wheel {keys} -13 40\nwheel {keys} 0 -120\ngesture {keys} 6 32\ngesture {keys} 7 -64\n");
                }
            }
        }
    }
    for (x, y) in [("conf_rotx", "conf_roty"), ("conf_trax", "conf_tray")] {
        commands += "reset DefaultViewInConf 800 600 27 81 -33 0.8\n";
        commands += &format!("binding {x} ALT|LBUTTON|MOUSE_Y\nbinding {y} ALT|LBUTTON|MOUSE_X\ndrag 12 405 321 31 -17\ndrag 12 441 299 36 -22\n");
    }
    for kind in [1, 2] {
        for (vx, vy) in [
            (600.0, 800.0),
            (1800.0, -2400.0),
            (0.0, 20000.0),
            (300.0, 0.0),
        ] {
            for rho in [0.0, 0.5, 1.0] {
                commands += "reset DefaultViewInConf 800 600 27 81 -33 0.8\n";
                commands += &format!(
                    "drag {} 405 321 31 -17\nmomentum {kind} 405 321 {vx} {vy} {rho}\n",
                    if kind == 1 { LEFT } else { RIGHT }
                );
            }
        }
    }
    let path = std::env::temp_dir().join(format!("cuemol-input-{}.txt", std::process::id()));
    let mut child = Command::new(oracle)
        .arg(config_path)
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("run C++ oracle");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(commands.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success(), "C++ input oracle failed");
    let expected = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    let expected: Vec<Vec<f64>> = expected
        .lines()
        .map(|line| {
            line.split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect()
        })
        .collect();
    assert_eq!(expected.len(), commands.lines().count());
    let mut styles = StyleManager::new();
    styles.load_builtin_default_styles(0);
    let mut camera = Camera::new();
    let mut config = ViewInputConfig::new();
    let mut input = ViewInputController::default();
    let mut size = [800.0, 600.0];
    for (index, line) in commands.lines().enumerate() {
        let fields: Vec<_> = line.split_whitespace().collect();
        let n = |i: usize| fields[i].parse::<f64>().unwrap();
        match fields[0] {
            "reset" => {
                config = ViewInputConfig::new();
                config.set_style(fields[1]);
                config.set_tbrad(n(7));
                size = [n(2), n(3)];
                camera = Camera::new();
                camera.set_center(Vector4::new3(1.0, 2.0, 3.0));
                camera.set_rotation(
                    camera
                        .rotation()
                        .rotate_x(n(4))
                        .unwrap()
                        .rotate_y(n(5))
                        .unwrap()
                        .rotate_z(n(6))
                        .unwrap(),
                );
                input = ViewInputController::default();
                input.rotate_momentum = true;
                input.translate_momentum = true;
            }
            "start" => {
                input.cancel();
            }
            "binding" => {
                let id = match fields[1] {
                    "conf_rotx" => 1,
                    "conf_roty" => 2,
                    "conf_trax" => 5,
                    "conf_tray" => 6,
                    _ => panic!("unknown binding"),
                };
                config.set_binding(id, fields[2]);
            }
            "drag" => {
                input.drag(
                    &mut camera,
                    &config.resolved(&styles),
                    size,
                    [n(2), n(3)],
                    [n(4), n(5)],
                    n(1) as i32,
                );
            }
            "wheel" => {
                input.wheel(
                    &mut camera,
                    &config.resolved(&styles),
                    size[1],
                    [n(2), n(3)],
                    n(1) as i32,
                );
            }
            "gesture" => {
                let scale = if n(2) == 6.0 { 3.2 } else { -16.0 };
                input.gesture(
                    &mut camera,
                    &config.resolved(&styles),
                    size[1],
                    n(2) as i32,
                    n(3) / scale,
                    n(1) as i32,
                );
            }
            "momentum" => {
                input.start_momentum(
                    &camera,
                    &config.resolved(&styles),
                    size,
                    0,
                    [n(2), n(3)],
                    [n(4), n(5)],
                );
                let duration = if n(1) == 1.0 {
                    n(4).hypot(n(5)).min(2000.0)
                } else {
                    500.0
                };
                input.tick(&mut camera, size[1], (duration * n(6)) as i64);
            }
            _ => panic!("unknown command"),
        }
        let c = camera.center();
        let q = camera.rotation();
        let actual = [
            c.x,
            c.y,
            c.z,
            q.x,
            q.y,
            q.z,
            q.w,
            camera.zoom(),
            camera.cam_dist(),
            camera.slab_depth(),
        ];
        assert_eq!(expected[index].len(), actual.len());
        for (field, (&actual, &expected)) in actual.iter().zip(&expected[index]).enumerate() {
            assert!(
                (actual - expected).abs() < 2.0e-9,
                "line {}: {line}, field {field}: Rust {actual}, C++ {expected}",
                index + 1
            );
        }
    }
    eprintln!("Compared {} camera states against C++", expected.len());
}
