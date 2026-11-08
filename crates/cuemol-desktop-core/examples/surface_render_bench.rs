use cuemol_desktop_core::DesktopAppState;
use cuemol_render::RenderMesh;
use std::{collections::hash_map::DefaultHasher, hash::Hasher, time::Instant};

fn hash(meshes: &[RenderMesh]) -> u64 {
    let mut hash = DefaultHasher::new();
    for mesh in meshes {
        hash.write_usize(mesh.vertices.len());
        for vertex in &mesh.vertices {
            for value in [
                vertex.position.x,
                vertex.position.y,
                vertex.position.z,
                vertex.normal.x,
                vertex.normal.y,
                vertex.normal.z,
            ] {
                hash.write_u64(value.to_bits());
            }
            for value in vertex.color {
                hash.write_u32(value.to_bits());
            }
        }
        hash.write_usize(mesh.indices.len());
        for index in &mesh.indices {
            hash.write_u32(*index);
        }
    }
    hash.finish()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: surface_render_bench INPUT ITERATIONS".into());
    }
    let iterations = args[1].parse::<usize>()?;
    if iterations == 0 {
        return Err("iterations must be positive".into());
    }
    let mut app = DesktopAppState::new();
    let start = Instant::now();
    app.open_path(&args[0])?;
    let load_ms = start.elapsed().as_secs_f64() * 1000.;
    let scene = app
        .scene_manager()
        .scene(app.active_scene().ok_or("missing scene")?)
        .unwrap();
    let text_bytes: usize = scene
        .loaded_surfaces()
        .iter()
        .map(|surface| surface.data.len())
        .sum();
    let raw_bytes: usize = scene
        .loaded_surfaces()
        .iter()
        .map(|surface| surface.raw_data.len())
        .sum();
    let start = Instant::now();
    let meshes = app.active_scene_surface_meshes();
    let first_ms = start.elapsed().as_secs_f64() * 1000.;
    if meshes.is_empty() {
        return Err("input must contain a visible surface".into());
    }
    let vertices: usize = meshes.iter().map(|mesh| mesh.vertices.len()).sum();
    let triangles: usize = meshes.iter().map(|mesh| mesh.indices.len() / 3).sum();
    let identity = hash(&meshes);
    drop(meshes);
    let mut repeated_ms = 0.;
    for _ in 0..iterations {
        let start = Instant::now();
        let meshes = app.active_scene_surface_meshes();
        repeated_ms += start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(hash(&meshes), identity);
    }
    println!(
        "{}",
        serde_json::json!({
            "iterations":iterations,"vertices":vertices,"triangles":triangles,
            "text_bytes":text_bytes,"raw_bytes":raw_bytes,"load_ms":load_ms,
            "first_ms":first_ms,"repeated_ms":repeated_ms,"ms_per_snapshot":repeated_ms / iterations as f64,
            "hash":format!("{identity:016x}")
        })
    );
    Ok(())
}
