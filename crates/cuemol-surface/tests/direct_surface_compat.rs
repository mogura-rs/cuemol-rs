use cuemol_surface::DirectSurfRenderer;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn equidistant_at_mesh_precision(
    position: cuemol_core::Vector4,
    first: cuemol_core::Vector4,
    second: cuemol_core::Vector4,
) -> bool {
    let p = [position.x, position.y, position.z];
    let first = [first.x, first.y, first.z].map(|x| x as f32 as f64);
    let second = [second.x, second.y, second.z].map(|x| x as f32 as f64);
    let squared = |center: [f64; 3]| (0..3).map(|i| (p[i] - center[i]).powi(2)).sum::<f64>();
    let a = squared(first);
    let b = squared(second);
    let quantization = (0..3)
        .map(|i| {
            let value = (p[i] as f32).abs();
            let ulp = f64::from(f32::from_bits(value.to_bits() + 1) - value);
            ulp * (first[i] - second[i]).abs()
        })
        .sum::<f64>();
    (a - b).abs() <= quantization + 64.0 * f64::EPSILON * a.max(b).max(1.0)
}

#[test]
fn direct_surface_geometry_normals_topology_and_owners_match_cpp() {
    let executable = std::env::var("CUEMOL_SURFACE_ORACLE").ok();
    let fixtures = std::env::var_os("CUEMOL_SURFACE_FIXTURE_DIR").map(PathBuf::from);
    if executable.is_none() && fixtures.is_none() {
        eprintln!("Skipping C++ surface oracle: set CUEMOL_SURFACE_ORACLE and CUEMOL_SURFACE_CONFIG, or CUEMOL_SURFACE_FIXTURE_DIR");
        return;
    }
    let config = executable
        .as_ref()
        .map(|_| std::env::var("CUEMOL_SURFACE_CONFIG").expect("C++ oracle configuration"));
    if executable.is_some() {
        if let Some(directory) = &fixtures {
            std::fs::create_dir_all(directory).unwrap();
        }
    }
    let reference = std::env::var_os("CUEMOL2_REF_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"),
        PathBuf::from,
    );
    let pdb = reference.join("tests/test_data/1CRN.pdb");
    let molecule = cuemol_molstr::read_pdb_file(&pdb).unwrap();
    let atom_indices = molecule
        .atoms()
        .iter()
        .enumerate()
        .map(|(index, atom)| (atom.serial as u32, index as u32))
        .collect::<std::collections::BTreeMap<_, _>>();
    let output = std::env::temp_dir().join(format!("cuemol-surface-{}.txt", std::process::id()));
    let mut failures = Vec::new();
    for (algorithm, name) in [(0, "edtsurf"), (1, "distfield"), (2, "meshms")] {
        for (surface_type, kind) in [(0, "vdw"), (1, "sas"), (2, "ses")] {
            for detail in [1, 6, 16] {
                let case = format!("{name}/{kind}/detail={detail}");
                let output = fixtures.as_ref().map_or_else(
                    || output.clone(),
                    |directory| directory.join(format!("{name}-{kind}-{detail}.txt")),
                );
                if let Some(executable) = &executable {
                    let status = Command::new(executable)
                        .arg(config.as_ref().unwrap())
                        .arg(&pdb)
                        .arg(&output)
                        .args([
                            "sel=resid 1:3".to_string(),
                            format!("surfalgor={name}"),
                            format!("surftype={kind}"),
                            format!("detail={detail}"),
                        ])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .unwrap();
                    assert!(status.success(), "C++ surface oracle failed: {case}");
                }
                let mut renderer = DirectSurfRenderer::new();
                renderer.set_surface_algorithm(algorithm);
                renderer.set_surface_type(surface_type);
                renderer.set_detail(detail);
                renderer.set_selection(cuemol_molstr::parse_selector("resid 1:3").unwrap());
                let mesh = renderer.render_mesh(&molecule);
                let text = std::fs::read_to_string(&output).unwrap();
                let mut values = text.split_whitespace().map(|v| v.parse::<f64>().unwrap());
                let counts = (
                    values.next().unwrap() as usize,
                    values.next().unwrap() as usize,
                );
                if (mesh.vertex_count(), mesh.triangle_count()) != counts {
                    failures.push(format!(
                        "{case}: counts {:?} != {counts:?}",
                        (mesh.vertex_count(), mesh.triangle_count())
                    ));
                    continue;
                }
                assert!(!mesh.is_empty(), "{case}");
                let mut max_position: f64 = 0.0;
                let mut max_normal: f64 = 0.0;
                let mut owner_errors = Vec::new();
                let mut tied_owners = 0;
                let mut canonical_owners = Vec::new();
                let mut face_errors = 0;
                for (index, vertex) in mesh.vertices().iter().enumerate() {
                    for actual in [vertex.position.x, vertex.position.y, vertex.position.z] {
                        max_position = max_position.max((actual - values.next().unwrap()).abs());
                    }
                    for actual in [vertex.normal.x, vertex.normal.y, vertex.normal.z] {
                        max_normal = max_normal.max((actual - values.next().unwrap()).abs());
                    }
                    let owner = atom_indices.get(&vertex.info).copied().unwrap_or(u32::MAX);
                    canonical_owners.push(owner);
                    let expected_owner = values.next().unwrap() as u32;
                    if owner != expected_owner {
                        let tied = algorithm == 2
                            && surface_type == 2
                            && molecule
                                .atoms()
                                .get(owner as usize)
                                .zip(molecule.atoms().get(expected_owner as usize))
                                .is_some_and(|(a, b)| {
                                    equidistant_at_mesh_precision(
                                        vertex.position,
                                        a.position,
                                        b.position,
                                    )
                                });
                        if tied {
                            tied_owners += 1;
                        } else {
                            owner_errors.push((index, owner, expected_owner));
                        }
                    }
                }
                for triangle in mesh.triangles() {
                    for &index in triangle {
                        face_errors += usize::from(index != values.next().unwrap() as u32);
                    }
                }
                assert!(values.next().is_none(), "{case}");
                if fixtures.is_some() {
                    let path = output.with_extension("owners");
                    if executable.is_some() {
                        std::fs::write(
                            path,
                            canonical_owners
                                .iter()
                                .map(u32::to_string)
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )
                        .unwrap();
                    } else {
                        let expected = std::fs::read_to_string(path)
                            .unwrap()
                            .split_whitespace()
                            .map(|value| value.parse::<u32>().unwrap())
                            .collect::<Vec<_>>();
                        assert_eq!(
                            canonical_owners, expected,
                            "cross-platform surface owners: {case}"
                        );
                    }
                }
                let report = format!("{case}: vertices={}, position={max_position}, normal={max_normal}, owners={owner_errors:?}, ties={tied_owners}, faces={face_errors}", mesh.vertex_count());
                eprintln!("{report}");
                if max_position > 2.0e-6
                    || max_normal > 2.0e-6
                    || !owner_errors.is_empty()
                    || face_errors != 0
                {
                    failures.push(report);
                }
            }
        }
    }
    if fixtures.is_none() {
        std::fs::remove_file(output).unwrap();
    }
    assert!(
        failures.is_empty(),
        "C++ surface differences:\n{}",
        failures.join("\n")
    );
}
