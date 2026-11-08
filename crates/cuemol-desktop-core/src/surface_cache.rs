use cuemol_molstr::qsys::LoadedSurface;
use cuemol_surface::SurfaceMesh;
use std::sync::{Arc, Mutex, Weak};

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 16;

#[derive(Debug)]
enum Source {
    Obj(Weak<str>),
    Ply(Weak<[u8]>),
    Qdf(Weak<[u8]>),
}

impl Source {
    fn new(surface: &LoadedSurface) -> Option<Self> {
        match surface.srctype.as_str() {
            "obj" => Some(Self::Obj(Arc::downgrade(&surface.data))),
            "ply" => Some(Self::Ply(Arc::downgrade(&surface.raw_data))),
            "qdfsurf" => Some(Self::Qdf(Arc::downgrade(&surface.raw_data))),
            _ => None,
        }
    }

    fn matches(&self, surface: &LoadedSurface) -> bool {
        match (self, surface.srctype.as_str()) {
            (Self::Obj(source), "obj") => source.ptr_eq(&Arc::downgrade(&surface.data)),
            (Self::Ply(source), "ply") | (Self::Qdf(source), "qdfsurf") => {
                source.ptr_eq(&Arc::downgrade(&surface.raw_data))
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
struct Entry {
    source: Source,
    mesh: Arc<SurfaceMesh>,
    bytes: usize,
}

#[derive(Debug, Default)]
pub(super) struct SurfaceCache(Mutex<Vec<Entry>>);

impl Clone for SurfaceCache {
    fn clone(&self) -> Self {
        // Undo snapshots and render jobs do not retain the interactive cache.
        Self::default()
    }
}

impl SurfaceCache {
    pub fn retain(&self, surfaces: &[LoadedSurface]) {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|entry| surfaces.iter().any(|surface| entry.source.matches(surface)));
    }

    pub fn mesh(&self, surface: &LoadedSurface) -> Option<Arc<SurfaceMesh>> {
        let source = Source::new(surface)?;
        {
            let mut entries = self.0.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(index) = entries
                .iter()
                .position(|entry| entry.source.matches(surface))
            {
                let entry = entries.remove(index);
                let mesh = entry.mesh.clone();
                entries.push(entry);
                return Some(mesh);
            }
        }
        let mesh = match surface.srctype.as_str() {
            "obj" => cuemol_surface::read_obj_str(&surface.data),
            "ply" => cuemol_surface::read_ply_bytes(&surface.raw_data),
            "qdfsurf" => cuemol_surface::qdf::read_qdf_surface_bytes(&surface.raw_data)
                .map(|mut object| std::mem::take(object.mesh_mut())),
            _ => return None,
        }
        .ok()?;
        let bytes = mesh.allocated_bytes();
        let mesh = Arc::new(mesh);
        if bytes <= MAX_BYTES {
            let mut entries = self.0.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(entry) = entries.iter().find(|entry| entry.source.matches(surface)) {
                return Some(entry.mesh.clone());
            }
            let mut retained: usize = entries.iter().map(|entry| entry.bytes).sum();
            while entries.len() >= MAX_ENTRIES || retained + bytes > MAX_BYTES {
                retained -= entries.remove(0).bytes;
            }
            entries.push(Entry {
                source,
                mesh: mesh.clone(),
                bytes,
            });
        }
        Some(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        undo::{ChangeSet, Direction},
        DesktopAppState,
    };
    use cuemol_core::{Uid, Vector4};

    const OBJ: &str = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
    const PLY: &[u8] = b"ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n";

    #[test]
    fn text_sources_share_storage_without_rewriting_invalid_utf8() {
        let (text, bytes) = LoadedSurface::source_payload("obj", OBJ.as_bytes().to_vec());
        assert_eq!(text.as_ptr(), bytes.as_ptr());
        let mut malformed = OBJ.as_bytes().to_vec();
        malformed.extend_from_slice(b"# comment \xff\n");
        let (text, bytes) = LoadedSurface::source_payload("obj", malformed.clone());
        assert_eq!(bytes.as_ref(), malformed);
        assert!(text.contains('\u{fffd}'));
        assert_eq!(
            cuemol_surface::read_obj_str(&text).unwrap(),
            cuemol_surface::read_obj_str(OBJ).unwrap()
        );
    }

    #[test]
    fn cached_geometry_tracks_replacement_undo_styles_and_scene_roundtrips() {
        let mut app = DesktopAppState::new();
        app.open_bytes("mesh.obj", OBJ.as_bytes()).unwrap();
        let scene = app.active_scene().unwrap();
        let uid = app.scene_manager().scene(scene).unwrap().loaded_surfaces()[0].uid;
        let original = app.active_scene_surface_meshes();
        assert!(!app.surface_mesh_cache.0.lock().unwrap().is_empty());
        let before = app.clone();
        assert!(before.surface_mesh_cache.0.lock().unwrap().is_empty());
        let surface = app
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .loaded_surface_mut(uid)
            .unwrap();
        let changed = OBJ.replacen("v 0 0 0", "v 2 0 0", 1);
        surface.data = changed.clone().into();
        surface.raw_data = changed.into_bytes().into();
        let edit = ChangeSet::between(&before, &app);
        assert_eq!(
            app.active_scene_surface_meshes()[0].vertices[0].position.x,
            2.
        );
        edit.apply(&mut app, Direction::Undo);
        assert_eq!(app.active_scene_surface_meshes(), original);
        edit.apply(&mut app, Direction::Redo);
        let surface = app
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .loaded_surface_mut(uid)
            .unwrap();
        surface
            .qsc_node
            .as_mut()
            .unwrap()
            .find_child_mut("renderer")
            .unwrap()
            .set_str_attr("defaultcolor", "#ff0000");
        let styled = app.active_scene_surface_meshes();
        assert_eq!(styled[0].vertices[0].color, [1., 0., 0., 1.]);
        assert_eq!(styled[0].vertices[0].position.x, 2.);
        let bytes = app.active_scene_bytes().unwrap();
        let mut restored = DesktopAppState::new();
        restored.open_bytes("surface.qsc", &bytes).unwrap();
        assert_eq!(restored.active_scene_surface_meshes(), styled);
        app.active_scene = None;
        assert!(app.active_render_snapshot("").is_err());
        assert!(app.surface_mesh_cache.0.lock().unwrap().is_empty());
    }

    #[test]
    fn binary_sources_avoid_text_shadows_and_mutation_invalidates_cached_ply() {
        let mut object = cuemol_surface::MolSurfObj::new();
        *object.mesh_mut() = cuemol_surface::read_obj_str(OBJ).unwrap();
        let qdf = cuemol_surface::qdf::write_qdf_surface_bytes(&object);
        let directory = tempfile::tempdir().unwrap();
        for (name, source) in [("surface.qdf", qdf.as_slice()), ("surface.ply", PLY)] {
            let path = directory.path().join(name);
            std::fs::write(&path, source).unwrap();
            let mut app = DesktopAppState::new();
            app.open_path(&path).unwrap();
            let scene = app.active_scene().unwrap();
            let surface = &app.scene_manager().scene(scene).unwrap().loaded_surfaces()[0];
            assert!(surface.data.is_empty());
            assert_eq!(surface.raw_data.as_ref(), source);
            let original = app.active_scene_surface_meshes();
            let bytes = app.active_scene_bytes().unwrap();
            let mut restored = DesktopAppState::new();
            restored.open_bytes("binary.qsc", &bytes).unwrap();
            assert_eq!(restored.active_scene_surface_meshes(), original);
            let scene = restored.active_scene().unwrap();
            let surface = &mut restored
                .scene_manager_mut()
                .scene_mut(scene)
                .unwrap()
                .loaded_surfaces_mut()[0];
            assert!(surface.data.is_empty());
            assert_eq!(surface.raw_data.as_ref(), source);
            if name.ends_with(".ply") {
                let position = source
                    .windows(7)
                    .position(|bytes| bytes == b"\n0 0 0\n")
                    .unwrap()
                    + 1;
                Arc::make_mut(&mut surface.raw_data)[position] = b'2';
                assert_eq!(
                    restored.active_scene_surface_meshes()[0].vertices[0]
                        .position
                        .x,
                    2.
                );
            }
        }
    }

    #[test]
    fn cache_bounds_retained_buffers_and_releases_inactive_sources() {
        let mut object = cuemol_surface::MolSurfObj::new();
        for index in 0..60_000 {
            object
                .mesh_mut()
                .push_vertex(cuemol_surface::SurfaceVertex::new(
                    Vector4::new3(index as f64, 0., 0.),
                    Vector4::new3(0., 0., 1.),
                ));
        }
        object.mesh_mut().push_triangle([0, 1, 2]);
        let bytes = cuemol_surface::qdf::write_qdf_surface_bytes(&object);
        let surfaces: Vec<_> = (0..10)
            .map(|index| LoadedSurface {
                uid: Uid::new(index + 1),
                name: "mesh".into(),
                object_type: "MolSurfObj".into(),
                srctype: "qdfsurf".into(),
                data: "".into(),
                raw_data: bytes.clone().into(),
                qsc_node: None,
            })
            .collect();
        let cache = SurfaceCache::default();
        for surface in &surfaces {
            assert_eq!(cache.mesh(surface).unwrap().vertex_count(), 60_000);
            assert_eq!(Arc::strong_count(&surface.raw_data), 1);
            let entries = cache.0.lock().unwrap();
            assert!(entries.iter().map(|entry| entry.bytes).sum::<usize>() <= MAX_BYTES);
            assert!(entries.len() <= MAX_ENTRIES);
        }
        assert!(cache.0.lock().unwrap().len() < surfaces.len());
        cache.retain(&[]);
        assert!(cache.0.lock().unwrap().is_empty());
    }
}
