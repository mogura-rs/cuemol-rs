use cuemol_desktop_core::DesktopAppState;
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() != 2 {
        return Err("usage: render_geometry SCENE OUTPUT_VERTICES".into());
    }
    let mut state = DesktopAppState::new();
    state.open_scene_file(PathBuf::from(&arguments[0]), None)?;
    let camera = state
        .active_scene_saved_camera()
        .ok_or("saved camera missing")?;
    let snapshot = state.active_render_snapshot("")?;
    let objects: Vec<_> = state.scene_manager().scene(state.active_scene().ok_or("scene missing")?).unwrap()
        .objects().iter().map(|object| {
            let molecule = object.mol();
            let residues: Vec<_> = molecule.residue_summaries().into_iter().map(|residue| {
                serde_json::json!({"chain":residue.key.chain_id,"index":residue.key.script_index(),
                    "secondary":molecule.get_residue_prop_str(&residue.key.chain_id,&residue.key.script_index(),"secondary").ok().flatten(),
                    "secondary2":molecule.get_residue_prop_str(&residue.key.chain_id,&residue.key.script_index(),"secondary2").ok().flatten()})
            }).collect();
            serde_json::json!({"name":object.name(),"atoms":molecule.atom_count(),"bonds":molecule.bond_count(),"residues":residues})
        }).collect();
    println!("{}", serde_json::json!({"objects":objects}));
    let mut output = BufWriter::new(File::create(&arguments[1])?);
    for (mesh_id, mesh) in snapshot.meshes.iter().enumerate() {
        writeln!(
            output,
            "mesh {mesh_id} {} {}",
            mesh.vertices.len(),
            mesh.indices.len() / 3
        )?;
        for vertex in &mesh.vertices {
            let point = camera
                .rotation()
                .to_matrix()
                .mul_vec(vertex.position - camera.center());
            let normal = camera.rotation().to_matrix().mul_vec(vertex.normal);
            writeln!(
                output,
                "{:.9} {:.9} {:.9} {:.9} {:.9} {:.9}",
                point.x, point.y, point.z, normal.x, normal.y, normal.z
            )?;
        }
        for face in mesh.indices.chunks_exact(3) {
            writeln!(output, "f {} {} {}", face[0], face[1], face[2])?;
        }
    }
    output.flush()?;
    Ok(())
}
