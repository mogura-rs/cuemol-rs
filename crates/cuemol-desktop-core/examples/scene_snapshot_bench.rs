use cuemol_desktop_core::{
    undo::{ChangeSet, Direction},
    DesktopAppState,
};
use cuemol_molstr::MolScene;
use std::{collections::hash_map::DefaultHasher, hash::Hasher, time::Instant};

fn payload_hash(scene: &MolScene) -> u64 {
    let mut hash = DefaultHasher::new();
    for surface in scene.loaded_surfaces() {
        hash.write(surface.data.as_bytes());
        hash.write(&surface.raw_data);
    }
    for object in scene.loaded_lw_objects() {
        hash.write(&object.raw_data);
    }
    for object in scene.loaded_qsc_objects() {
        if let Some(bytes) = &object.raw_data {
            hash.write(bytes);
        }
    }
    hash.finish()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if !(2..=4).contains(&arguments.len())
        || arguments.get(3).is_some_and(|value| value != "lightweight")
    {
        return Err("usage: scene_snapshot_bench INPUT EDITS [OUTPUT_QDF [lightweight]]".into());
    }
    let edits: usize = arguments[1].parse()?;
    if edits == 0 {
        return Err("edits must be positive".into());
    }
    let bytes = std::fs::read(&arguments[0])?;
    if let Some(output) = arguments.get(2) {
        if std::path::Path::new(output) == std::path::Path::new(&arguments[0]) {
            return Err("output must differ from input".into());
        }
        let mut surface = cuemol_surface::MolSurfObj::new();
        *surface.mesh_mut() = cuemol_surface::read_obj_str(std::str::from_utf8(&bytes)?)?;
        let bytes = if arguments.len() == 4 {
            use cuemol_lwview::{LWDrawData, LWDrawElement, LWObject, LWVertexNormalColor};
            let vector =
                |value: cuemol_core::Vector4| [value.x as f32, value.y as f32, value.z as f32];
            let vertices = surface
                .mesh()
                .vertices()
                .iter()
                .map(|vertex| LWVertexNormalColor {
                    position: vector(vertex.position),
                    normal: vector(vertex.normal),
                    color: [255; 4],
                })
                .collect();
            let indices = surface
                .mesh()
                .triangles()
                .iter()
                .flatten()
                .map(|index| *index as i32)
                .collect();
            let mut object = LWObject::new();
            object.add_draw_element(LWDrawElement::new(
                1,
                cuemol_gfx::AbstDrawElem::DRAW_TRIANGLES as i8,
                1.,
                [255; 4],
                LWDrawData::VertexNormalColorIndices { vertices, indices },
            ));
            cuemol_lwview::write_qdf_lwobj_bytes(&object)
        } else {
            cuemol_surface::qdf::write_qdf_surface_bytes(&surface)
        };
        std::fs::write(output, bytes)?;
    }
    let mut app = DesktopAppState::new();
    let started = Instant::now();
    app.open_bytes(&arguments[0], &bytes)?;
    let load_ms = started.elapsed().as_secs_f64() * 1000.;
    let scene_uid = app.active_scene().ok_or("missing scene")?;
    let scene = app.scene_manager().scene(scene_uid).unwrap();
    let original_name = scene.name().to_string();
    let hash = payload_hash(scene);
    let mut changes = Vec::new();
    let started = Instant::now();
    for index in 0..edits {
        let before = app.clone();
        app.scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .set_name(format!("edit-{index}"));
        app.mark_scene_modified(scene_uid);
        changes.push(ChangeSet::between(&before, &app));
    }
    let edits_ms = started.elapsed().as_secs_f64() * 1000.;
    let started = Instant::now();
    for _ in 0..2 {
        for change in changes.iter().rev() {
            change.apply(&mut app, Direction::Undo);
            assert_eq!(
                payload_hash(app.scene_manager().scene(scene_uid).unwrap()),
                hash
            );
        }
        assert_eq!(
            app.scene_manager().scene(scene_uid).unwrap().name(),
            original_name
        );
        for change in &changes {
            change.apply(&mut app, Direction::Redo);
            assert_eq!(
                payload_hash(app.scene_manager().scene(scene_uid).unwrap()),
                hash
            );
        }
        assert_eq!(
            app.scene_manager().scene(scene_uid).unwrap().name(),
            format!("edit-{}", edits - 1)
        );
    }
    let undo_redo_ms = started.elapsed().as_secs_f64() * 1000.;
    println!("{{\"input_bytes\":{},\"edits\":{edits},\"load_ms\":{load_ms},\"edits_ms\":{edits_ms},\"undo_redo_with_hash_ms\":{undo_redo_ms},\"hash\":\"{hash:016x}\"}}", bytes.len());
    Ok(())
}
