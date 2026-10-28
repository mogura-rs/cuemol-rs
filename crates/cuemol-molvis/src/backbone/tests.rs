use super::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn oracle(mode: &str, input: &str) -> Option<Vec<f64>> {
    let Ok(executable) = std::env::var("CUEMOL_BACKBONE_ORACLE") else {
        eprintln!(
            "Skipping C++ backbone oracle: set CUEMOL_BACKBONE_ORACLE and CUEMOL_BACKBONE_CONFIG"
        );
        return None;
    };
    let config = std::env::var("CUEMOL_BACKBONE_CONFIG").expect("C++ oracle configuration");
    let output =
        std::env::temp_dir().join(format!("cuemol-backbone-{}-{mode}.txt", std::process::id()));
    let mut child = Command::new(executable)
        .args([config.as_str(), mode])
        .arg(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
    let text = std::fs::read_to_string(&output).unwrap();
    std::fs::remove_file(output).unwrap();
    Some(
        text.split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect(),
    )
}

#[test]
fn cross_sections_match_cpp_vertices_and_normals() {
    let mut input = String::new();
    let mut expected = Vec::new();
    for (kind, shape) in [
        (0, SectionShape::Elliptical),
        (1, SectionShape::RoundSquare),
        (2, SectionShape::Rectangular),
        (3, SectionShape::Fancy),
    ] {
        for (width, tuber, sharp, detail) in [
            (0.18, 6.5, 0.4, 16),
            (0.35, 1.0, 0.4, 16),
            (0.2, 6.0, 0.3, 7),
            (0.4, 2.5, 0.0, 24),
            (0.2, 8.0, 0.8, 32),
        ] {
            input += &format!("{kind} {width} {tuber} {sharp} {detail}\n");
            expected.push(Profile::new(width * tuber, width, shape, sharp, detail));
        }
    }
    let Some(values) = oracle("profile", &input) else {
        return;
    };
    let mut values = values.into_iter();
    for profile in expected {
        assert_eq!(
            values.next().unwrap() as usize,
            profile.vertices.len(),
            "{:?}",
            profile.shape
        );
        for vertex in profile.vertices.iter() {
            for value in vertex.xy.into_iter().chain(vertex.normal) {
                let reference = values.next().unwrap();
                assert!(
                    (value - reference).abs() < 1.0e-12,
                    "{:?}: {value} != {reference}",
                    profile.shape
                );
            }
            let face = values.next().unwrap() as i32;
            let actual = match vertex.face {
                Face::Unknown => 0,
                Face::Front => 1,
                Face::Back => 2,
                Face::Side1 => 3,
                Face::Side2 => 4,
            };
            assert_eq!(actual, face);
        }
    }
    assert!(values.next().is_none());
}

#[test]
fn smoothing_spline_matches_cpp_weighted_and_small_fits() {
    let mut input = String::new();
    let mut expected = Vec::new();
    for n in [2, 3, 4, 9, 17, 100] {
        for rho in [-10.0, -1.0, 0.0, 3.0, 5.0] {
            for weighted in [false, true] {
                let points = (0..n)
                    .map(|i| {
                        let t = i as f64;
                        Vector4::new3(0.7 * t + t.sin(), (t * 0.6).cos(), t * t * 0.02)
                    })
                    .collect::<Vec<_>>();
                let weights = (0..n)
                    .map(|i| if i % 3 == 0 { 10.0 } else { 1.0 })
                    .collect::<Vec<_>>();
                input += &format!("{n} {rho} -1 {}\n", usize::from(weighted));
                for (p, w) in points.iter().zip(&weights) {
                    input += &format!("{} {} {} {w}\n", p.x, p.y, p.z);
                }
                let spline = smoothing::SmoothSpline::new(
                    &points,
                    weighted.then_some(weights.as_slice()),
                    rho,
                    -1.0,
                    4,
                )
                .unwrap();
                for i in -4..=n * 4 {
                    let p = spline.interpolate(-1.0 + i as f64 * 0.25);
                    for value in [p.position, p.tangent, p.curvature] {
                        expected.extend([value.x, value.y, value.z]);
                    }
                }
            }
        }
    }
    let Some(values) = oracle("smooth", &input) else {
        return;
    };
    assert_eq!(expected.len(), values.len());
    let mut max_error: f64 = 0.0;
    for (i, (actual, reference)) in expected.into_iter().zip(values).enumerate() {
        let error = (actual - reference).abs();
        max_error = max_error.max(error);
        assert!(
            error < 2.0e-7,
            "sample {i}: {actual} != {reference}, error={error}"
        );
    }
    eprintln!("C++ spline maximum absolute error: {max_error:e}");
}

#[test]
fn smoothing_preserves_straight_lines_and_tangent() {
    let points = (0..1000)
        .map(|i| Vector4::new3(i as f64, 2.0 * i as f64, -0.5 * i as f64))
        .collect::<Vec<_>>();
    let spline = smoothing::SmoothSpline::new(&points, None, 3.0, 0.0, 4).unwrap();
    for t in [-0.5, 0.0, 10.25, 500.0, 999.0, 999.5] {
        let p = spline.interpolate(t);
        assert!((p.position - Vector4::new3(t, 2.0 * t, -0.5 * t)).length3d() < 1.0e-10);
        assert!((p.tangent - Vector4::new3(1.0, 2.0, -0.5)).length3d() < 1.0e-10);
        assert!(p.curvature.length3d() < 1.0e-10);
    }
}

fn reference_mesh(
    mode: &str,
    pdb: &std::path::Path,
    kind: &str,
    style: &str,
    settings: &[&str],
) -> Vec<f64> {
    let executable = std::env::var("CUEMOL_BACKBONE_ORACLE").unwrap();
    let config = std::env::var("CUEMOL_BACKBONE_CONFIG").unwrap();
    let output = std::env::temp_dir().join(format!(
        "cuemol-backbone-{}-geometry.txt",
        std::process::id()
    ));
    let status = Command::new(executable)
        .arg(config)
        .arg(mode)
        .arg(&output)
        .arg(pdb)
        .args([kind, style])
        .args(settings)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "C++ {mode} {kind} {style} {settings:?}");
    let text = std::fs::read_to_string(&output).unwrap();
    std::fs::remove_file(output).unwrap();
    text.split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect()
}

fn cell(point: Vector4) -> [i64; 3] {
    [point.x, point.y, point.z].map(|v| (v / 1.0e-4).floor() as i64)
}

fn in_neighbors<'a, T>(
    map: &'a std::collections::HashMap<[i64; 3], Vec<T>>,
    p: Vector4,
) -> impl Iterator<Item = &'a T> {
    let [x, y, z] = cell(p);
    (-1..=1)
        .flat_map(move |dx| {
            (-1..=1).flat_map(move |dy| {
                (-1..=1).filter_map(move |dz| map.get(&[x + dx, y + dy, z + dz]))
            })
        })
        .flatten()
}

fn geometry_matches(
    actual: &[MeshVertex],
    expected: &[MeshVertex],
    label: &str,
    compare_rgb: bool,
) {
    let mut grid = std::collections::HashMap::<_, Vec<_>>::new();
    for vertex in expected {
        grid.entry(cell(vertex.position)).or_default().push(vertex);
    }
    for (i, vertex) in actual.iter().enumerate() {
        assert!(
            in_neighbors(&grid, vertex.position).any(|candidate| {
                (vertex.position - candidate.position).length3d() < 5.0e-6
                    && (!compare_rgb
                        || (0..3).all(|i| {
                            (vertex.color[i] - candidate.color[i]).abs() <= 1.0 / 255.0 + 1.0e-6
                        }))
                    && (vertex.color[3] - candidate.color[3]).abs() <= 1.0 / 255.0 + 1.0e-6
                    && normalize_or(vertex.normal, Vector4::ZERO)
                        .dot3d(normalize_or(candidate.normal, Vector4::ZERO))
                        > 1.0 - 1.0e-6
            }),
            "{label}: unmatched vertex {i}: {:?}, {:?}, {:?}; nearest colors: {:?}",
            vertex.position,
            vertex.normal,
            vertex.color,
            in_neighbors(&grid, vertex.position)
                .map(|c| c.color)
                .collect::<Vec<_>>()
        );
    }
}

fn nondegenerate(tri: &[Vector4; 3]) -> bool {
    (0..3).all(|i| (tri[i] - tri[(i + 1) % 3]).length3d() > 1.0e-5)
        && (tri[1] - tri[0]).cross(tri[2] - tri[0]).length3d() > 1.0e-5
}

fn faces_match(actual: &[[Vector4; 3]], expected: &[[Vector4; 3]], label: &str) {
    let center = |tri: &[Vector4; 3]| (tri[0] + tri[1] + tri[2]).scale(1.0 / 3.0);
    let mut grid = std::collections::HashMap::<_, Vec<_>>::new();
    for face in expected.iter().filter(|t| nondegenerate(t)) {
        grid.entry(cell(center(face))).or_default().push(face);
    }
    for (i, face) in actual.iter().enumerate().filter(|(_, t)| nondegenerate(t)) {
        assert!(
            in_neighbors(&grid, center(face)).any(|candidate| (0..3).any(|offset| (0..3)
                .all(|j| (face[j] - candidate[(j + offset) % 3]).length3d() < 5.0e-6))),
            "{label}: unmatched triangle {i}: {face:?}"
        );
    }
}

#[test]
fn backbone_geometry_and_triangle_winding_match_cpp() {
    if std::env::var_os("CUEMOL_BACKBONE_ORACLE").is_none() {
        eprintln!(
            "Skipping C++ geometry oracle: set CUEMOL_BACKBONE_ORACLE and CUEMOL_BACKBONE_CONFIG"
        );
        return;
    }
    let reference = std::env::var_os("CUEMOL2_REF_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2")
        });
    let pdb = reference.join("tests/test_data/1CRN.pdb");
    let cases: &[(&str, &str, &[&str])] = &[
        ("ribbon", "DefaultRibbon", &["defaultcolor=#234567"]),
        ("cartoon", "RoundCartoon", &["defaultcolor=#765432"]),
        ("tube", "", &["defaultcolor=#123456"]),
        (
            "ribbon",
            "DefaultRibbon,DefaultHSCPaint",
            &["object.defaultcolor=#13579b"],
        ),
        (
            "cartoon",
            "RoundCartoon,DefaultHSCPaint",
            &[
                "object.defaultcolor=#13579b",
                "defaultcolor=$molcol{mod_h:45;mod_s:-0.3;alpha:0.6}",
            ],
        ),
        (
            "ribbon",
            "RoundRibbon",
            &["defaultcolor=okabe_ito_1{mod_s:-0.6;mod_b:0.1}"],
        ),
        ("ribbon", "DefaultRibbon", &[]),
        ("ribbon", "DefaultRibbon,WoodyHSCPaint", &[]),
        ("ribbon", "RoundRibbon,WoodyHSCPaint", &["smoothcolor=true"]),
        (
            "ribbon",
            "RoundRibbon,WoodyHSCPaint",
            &["smoothcolor=false"],
        ),
        (
            "ribbon",
            "RoundRibbon,WoodyHSCPaint",
            &["sel=A.5:20.*", "segend_fade=true", "smoothcolor=true"],
        ),
        ("cartoon", "DefaultCartoon,WoodyHSCPaint", &[]),
        (
            "cartoon",
            "RoundCartoon,WoodyHSCPaint",
            &["smoothcolor=true"],
        ),
        ("tube", "WoodyHSCPaint", &["smoothcolor=true"]),
        ("ribbon", "RoundRibbon", &[]),
        ("ribbon", "Fancy1Ribbon", &[]),
        ("cartoon", "DefaultCartoon", &[]),
        ("cartoon", "RoundCartoon", &[]),
        ("cartoon", "DefaultCartoon", &["helix_width_mode=wavy"]),
        (
            "cartoon",
            "DefaultCartoon",
            &["helix_width_mode=const", "helix_width=3"],
        ),
        ("cartoon", "DefaultCartoon", &["helix_ribbon=true"]),
        ("cartoon", "RoundCartoon", &["helix_ribbon=true"]),
        (
            "cartoon",
            "RoundCartoon",
            &["start_captype=sphere", "end_captype=sphere"],
        ),
        (
            "ribbon",
            "RoundRibbon",
            &["helixhead.type=flat", "helixtail.type=flat"],
        ),
        (
            "ribbon",
            "Fancy1Ribbon",
            &["sheethead.basw=0.3", "sheethead.arrow=2.5"],
        ),
        ("ribbon", "DefaultRibbon", &["sel=A.9:15.*"]),
        ("ribbon", "RoundRibbon", &["sel=A.7:17.*"]),
        ("ribbon", "DefaultRibbon", &["sel=A.7:17.*"]),
        (
            "ribbon",
            "DefaultRibbon",
            &["sel=A.7:17.*", "segend_fade=true"],
        ),
        (
            "ribbon",
            "RoundRibbon",
            &["sel=A.9:15.*", "segend_fade=true", "smoothcolor=false"],
        ),
        ("ribbon", "RoundRibbon", &["sel=A.12.*"]),
        ("ribbon", "Fancy1Ribbon", &["sel=A.1:5.* or A.25:35.*"]),
        (
            "ribbon",
            "RoundRibbon",
            &["sel=A.9:15.*", "segend_fade=true"],
        ),
        ("ribbon", "DefaultRibbon", &["segend_fade=true"]),
        (
            "ribbon",
            "RoundRibbon",
            &["sel=A.9:15.N", "segend_fade=true"],
        ),
        ("cartoon", "DefaultCartoon", &["sel=A.9:15.*"]),
        ("cartoon", "RoundCartoon", &["sel=A.7:17.*"]),
        ("cartoon", "RoundCartoon", &["sel=A.1:5.* or A.25:35.*"]),
        ("cartoon", "RoundCartoon", &["segend_fade=true"]),
        ("tube", "", &["sel=A.9:15.*"]),
        (
            "tube",
            "",
            &["sel=A.1:5.* or A.25:35.*", "putty_mode=linear1"],
        ),
        ("tube", "", &["sel=A.9:15.*", "segend_fade=true"]),
        ("tube", "", &["segend_fade=true"]),
        ("tube", "", &[]),
        ("tube", "", &["putty_mode=linear1"]),
        ("tube", "", &["putty_mode=scale1", "smooth=0.5"]),
        (
            "tube",
            "",
            &[
                "section.type=roundsquare",
                "section.tuber=2.5",
                "section.sharp=0.7",
            ],
        ),
        (
            "cartoon",
            "DefaultCartoon",
            &[
                "anchor_sel=A.12:15.*",
                "anchor_weight=15",
                "helix_smooth=1",
                "sheet_smooth=0.5",
            ],
        ),
    ];
    for &(kind, style, settings) in cases {
        let molecule = cuemol_molstr::read_pdb_file(&pdb).unwrap();
        let mut manager = cuemol_molstr::MolSceneManager::new();
        let scene = manager.new_scene("oracle");
        let object = manager
            .scene_mut(scene)
            .unwrap()
            .add_molecule("protein", molecule);
        let object = manager
            .scene_mut(scene)
            .unwrap()
            .object_mut(object)
            .unwrap();
        for setting in settings {
            if let Some(value) = setting.strip_prefix("object.defaultcolor=") {
                object
                    .mol_mut()
                    .set_default_color(default_style_manager().compile_color(value, 0));
            }
        }
        let renderer = object.add_renderer(kind, "backbone");
        let renderer = object.renderer_mut(renderer).unwrap();
        renderer.set_qsc_attr("style", style);
        for setting in settings {
            let (key, value) = setting.split_once('=').unwrap();
            if key.starts_with("object.") {
                continue;
            }
            if let Some((child, key)) = key.split_once('.') {
                renderer.set_qsc_child_attr(child, key, value);
            } else {
                renderer.set_qsc_attr(key, value);
            }
        }
        let mut options = MoleculeRenderOptions::default();
        let styles = default_style_manager();
        apply_renderer_styles(&mut options, renderer, &styles, scene.get());
        if kind == "tube" {
            apply_tube_renderer_styles(&mut options, renderer, &styles, scene.get());
        }
        let selection = renderer.resolved_attr("sel", &styles, scene.get());
        let renderer_uid = renderer.uid();
        apply_molecular_coloring(
            &mut options,
            object,
            object.renderer(renderer_uid).unwrap(),
            &styles,
            scene.get(),
        );
        if let Some(selection) = selection {
            let selector = cuemol_molstr::parse_selector(&selection).unwrap();
            options.draw_indices = Some(object.mol().select_indices(&selector));
        }
        let snapshot = ribbon_mesh_snapshot(object.mol(), &options);
        let actual = snapshot
            .meshes
            .iter()
            .flat_map(|mesh| {
                mesh.indices
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .map(|i| mesh.vertices[i as usize].clone())
            })
            .collect::<Vec<_>>();
        let expected = reference_mesh("color", &pdb, kind, style, settings)
            .chunks_exact(10)
            .map(|v| MeshVertex {
                position: Vector4::new3(v[0], v[1], v[2]),
                normal: Vector4::new3(v[3], v[4], v[5]),
                color: std::array::from_fn(|i| v[6 + i] as f32 / 255.0),
            })
            .collect::<Vec<_>>();
        let label = format!("{kind} {style} {settings:?}");
        geometry_matches(&actual, &expected, &format!("Rust to C++: {label}"), true);
        geometry_matches(&expected, &actual, &format!("C++ to Rust: {label}"), true);
        let actual_faces = snapshot
            .meshes
            .iter()
            .flat_map(|mesh| {
                mesh.indices
                    .chunks_exact(3)
                    .map(|t| std::array::from_fn(|j| mesh.vertices[t[j] as usize].position))
            })
            .collect::<Vec<_>>();
        let expected_faces = reference_mesh("faces", &pdb, kind, style, settings)
            .chunks_exact(9)
            .map(|v| std::array::from_fn(|j| Vector4::new3(v[j * 3], v[j * 3 + 1], v[j * 3 + 2])))
            .collect::<Vec<_>>();
        faces_match(
            &actual_faces,
            &expected_faces,
            &format!("Rust to C++: {label}"),
        );
        faces_match(
            &expected_faces,
            &actual_faces,
            &format!("C++ to Rust: {label}"),
        );
        eprintln!(
            "Matched {label}: {} reference vertices and {} triangles",
            expected.len(),
            expected_faces.len()
        );
    }
}
