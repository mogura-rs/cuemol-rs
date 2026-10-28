use cuemol_core::{Matrix4, Quat, Uid, Vector4};
use cuemol_molstr::{parse_selector, read_pdb_str, MolCoord};
use cuemol_qsys::View;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn molecule(points: &[Vector4]) -> MolCoord {
    let template = read_pdb_str(
        "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n",
    )
    .unwrap()
    .atoms()[0]
        .clone();
    MolCoord::with_atoms(
        points
            .iter()
            .enumerate()
            .map(|(i, point)| {
                let mut atom = template.clone();
                atom.serial = i as i32 + 1;
                atom.res_id = atom.serial;
                atom.position = *point;
                atom
            })
            .collect(),
    )
}

#[test]
fn fitting_honors_transforms_minimum_extent_and_empty_selections() {
    let molecule = molecule(&[Vector4::new3(10., 20., 30.)]);
    let mut view = View::new(Uid::new(1), Uid::new(2));
    view.set_view_size(100, 200);
    let transform = Matrix4::make_trans_mat(Vector4::new3(4., 5., 6.));
    assert!(molecule.fit_view_with_xform(&mut view, None, transform));
    assert_eq!(view.view_center(), Vector4::new3(14., 25., 36.));
    assert_eq!((view.zoom(), view.slab_depth()), (8., 4.));
    let before = view.camera();
    let empty = parse_selector("none").unwrap();
    assert!(!molecule.orient_view(&mut view, Some(&empty)));
    assert_eq!(view.camera(), before);
}

#[test]
fn fitting_and_orientation_match_cpp_including_degenerate_clouds() {
    let Some(oracle) = std::env::var_os("CUEMOL_CAMERA_FIT_ORACLE") else {
        eprintln!("skipping camera oracle: run scripts/check-reference-camera-fit.py");
        return;
    };
    let config = std::env::var_os("CUEMOL_CAMERA_FIT_CONFIG").unwrap();
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.cache/camera-fit-reference/output.txt");
    let mut clouds = vec![
        vec![],
        vec![Vector4::new3(10., 20., 30.)],
        vec![Vector4::new3(2., 3., 4.); 4],
        vec![Vector4::new3(-4., -4., 0.), Vector4::new3(4., 4., 0.)],
    ];
    for seed in 0..64 {
        let rotation = Matrix4::make_rot_mat(
            Quat::from_axis_angle(Vector4::new3(1., 2., 3.), seed as f64 * 0.137).unwrap(),
        );
        clouds.push(
            (0..32)
                .map(|i| {
                    let t = i as f64 * 0.537 + seed as f64;
                    rotation.transform_point(Vector4::new3(
                        12. * t.cos(),
                        4. * t.sin(),
                        (t * 1.739).cos(),
                    )) + Vector4::new3(7., -3., 2.)
                })
                .collect(),
        );
    }
    // MolAtom stores positions as float32 in the C++ implementation.
    for point in clouds.iter_mut().flatten() {
        *point = Vector4::new3(
            point.x as f32 as f64,
            point.y as f32 as f64,
            point.z as f32 as f64,
        );
    }
    let mut input = String::new();
    let mut expected = Vec::new();
    for orient in [false, true] {
        for (i, points) in clouds.iter().enumerate() {
            let (width, height) = if i % 2 == 0 { (200, 100) } else { (100, 200) };
            input.push_str(&format!(
                "{} {width} {height} {}\n",
                u8::from(orient),
                points.len()
            ));
            for p in points {
                input.push_str(&format!("{} {} {}\n", p.x, p.y, p.z));
            }
            let mut view = View::new(Uid::new(1), Uid::new(2));
            view.set_view_size(width, height);
            view.set_view_center(Vector4::new3(1., 2., 3.));
            view.rotate_view(17., -31., 23.);
            let molecule = molecule(points);
            if orient {
                molecule.orient_view(&mut view, None);
            } else {
                molecule.fit_view2(&mut view, None);
            }
            expected.push(view.camera());
        }
    }
    let mut child = Command::new(oracle)
        .arg(config)
        .arg(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
    let rows = fs::read_to_string(output).unwrap();
    let rows = rows.lines().collect::<Vec<_>>();
    assert_eq!(rows.len(), expected.len());
    for (i, (line, camera)) in rows.iter().zip(expected).enumerate() {
        let v = line
            .split_whitespace()
            .map(|v| v.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        let c = camera.center();
        for (actual, reference) in [c.x, c.y, c.z, camera.zoom(), camera.slab_depth()]
            .into_iter()
            .zip([v[0], v[1], v[2], v[7], v[8]])
        {
            assert!(
                (actual - reference).abs() < 1e-7,
                "case {i}: {actual} != {reference}"
            );
        }
        let q = camera.rotation();
        let dot = q.x * v[3] + q.y * v[4] + q.z * v[5] + q.w * v[6];
        assert!(
            (dot.abs() - 1.).abs() < 1e-8,
            "case {i}: rotation dot={dot}, cpp={v:?}, rust={q:?}"
        );
    }
    eprintln!("compared {} C++ camera cases", rows.len());
}
