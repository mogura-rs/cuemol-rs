use cuemol_molstr::{read_pdb_file, MolSceneManager};
use cuemol_molvis::{
    apply_renderer_styles, apply_tube_renderer_styles, default_style_manager, ribbon_mesh_snapshot,
    MoleculeRenderOptions,
};
use std::io::{BufWriter, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 4 {
        return Err("Expected INPUT.pdb RENDERER STYLE OUTPUT.txt [PROPERTY=VALUE ...]".into());
    }
    let mol = read_pdb_file(&args[0])?;
    let mut manager = MolSceneManager::new();
    let scene = manager.new_scene("comparison");
    let object = manager
        .scene_mut(scene)
        .unwrap()
        .add_molecule("molecule", mol);
    let object = manager
        .scene_mut(scene)
        .unwrap()
        .object_mut(object)
        .unwrap();
    let renderer = object.add_renderer(&args[1], "backbone");
    let renderer = object.renderer_mut(renderer).unwrap();
    renderer.set_qsc_attr("style", &args[2]);
    for setting in &args[4..] {
        let (key, value) = setting.split_once('=').ok_or("Expected PROPERTY=VALUE")?;
        if let Some((child, key)) = key.split_once('.') {
            renderer.set_qsc_child_attr(child, key, value);
        } else {
            renderer.set_qsc_attr(key, value);
        }
    }
    let mut options = MoleculeRenderOptions::default();
    let styles = default_style_manager();
    apply_renderer_styles(&mut options, renderer, &styles, scene.get());
    if args[1] == "tube" {
        apply_tube_renderer_styles(&mut options, renderer, &styles, scene.get());
    }
    let selection = renderer.resolved_attr("sel", &styles, scene.get());
    if let Some(selection) = selection {
        let selector = cuemol_molstr::parse_selector(&selection).unwrap();
        options.draw_indices = Some(object.mol().select_indices(&selector));
    }
    let snapshot = ribbon_mesh_snapshot(object.mol(), &options);
    let mut out = BufWriter::new(std::fs::File::create(&args[3])?);
    for mesh in snapshot.meshes {
        for vertex in mesh.vertices {
            let p = vertex.position;
            let n = vertex.normal;
            writeln!(out, "{} {} {} {} {} {}", p.x, p.y, p.z, n.x, n.y, n.z)?;
        }
    }
    Ok(())
}
