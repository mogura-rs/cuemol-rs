//! Tauri desktop shell backed by the shared [`DesktopAppState`].

use std::path::PathBuf;
use std::sync::mpsc;

use cuemol_core::{Uid, Vector4};
use cuemol_qsys::Camera;
use cuemol_render::{tessellate_snapshot, RenderMesh, RenderSceneSnapshot};
use serde::Serialize;
use tauri::State;

use crate::offscreen::{camera_mvp, encode_rgba_png, SceneRenderer};
use crate::DesktopAppState;

const VIEW_CLEAR: [f64; 4] = [0.025, 0.035, 0.045, 1.0];
const MAX_VIEW_DIM: u32 = 1_600;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneDto {
    uid: u64,
    name: String,
    object_count: usize,
    map_count: usize,
    active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererDto {
    uid: u64,
    name: String,
    renderer_type: String,
    visible: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectDto {
    uid: u64,
    scene_uid: u64,
    name: String,
    atom_count: usize,
    residue_count: usize,
    bond_count: usize,
    renderers: Vec<RendererDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshot {
    scenes: Vec<SceneDto>,
    objects: Vec<ObjectDto>,
    active_scene_uid: Option<u64>,
    selection: String,
    perspective: bool,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewportDto {
    image: Option<String>,
    width: u32,
    height: u32,
    status: String,
}

struct AppModel {
    desktop: DesktopAppState,
    camera: Camera,
    renderer: Option<SceneRenderer>,
    mesh: Option<RenderMesh>,
    selection: String,
    status: String,
}

impl AppModel {
    fn new(open_paths: Vec<PathBuf>) -> Self {
        let mut desktop = DesktopAppState::new();
        desktop.ensure_active_scene();
        let mut loaded = 0usize;
        let mut failures = Vec::new();
        for path in open_paths {
            match desktop.open_path(&path) {
                Ok(_) => loaded += 1,
                Err(error) => failures.push(format!("{}: {error}", path.display())),
            }
        }
        let status = if failures.is_empty() {
            if loaded == 0 {
                "Ready".to_string()
            } else {
                format!("Loaded {loaded} file(s)")
            }
        } else {
            format!("Load failed: {}", failures.join("; "))
        };
        let mut model = Self {
            desktop,
            camera: Camera::new(),
            renderer: SceneRenderer::new(),
            mesh: None,
            selection: String::new(),
            status,
        };
        let _ = model.rebuild_mesh(true);
        model
    }

    fn snapshot(&self) -> AppSnapshot {
        let scenes = self
            .desktop
            .scene_summaries()
            .into_iter()
            .map(|scene| SceneDto {
                uid: scene.uid.get(),
                name: scene.name,
                object_count: scene.object_count,
                map_count: scene.map_count,
                active: scene.active,
            })
            .collect::<Vec<_>>();
        let mut objects = Vec::new();
        for scene in self.desktop.scene_summaries() {
            for object in self.desktop.object_summaries(scene.uid) {
                let renderers = self
                    .desktop
                    .renderer_summaries(object.uid)
                    .into_iter()
                    .map(|renderer| RendererDto {
                        uid: renderer.uid.get(),
                        name: renderer.name,
                        renderer_type: renderer.renderer_type,
                        visible: renderer.visible,
                    })
                    .collect();
                objects.push(ObjectDto {
                    uid: object.uid.get(),
                    scene_uid: scene.uid.get(),
                    name: object.name,
                    atom_count: object.atom_count,
                    residue_count: object.residue_count,
                    bond_count: object.bond_count,
                    renderers,
                });
            }
        }
        AppSnapshot {
            scenes,
            objects,
            active_scene_uid: self.desktop.active_scene().map(Uid::get),
            selection: self.selection.clone(),
            perspective: self.camera.is_perspective(),
            status: self.status.clone(),
        }
    }

    fn rebuild_mesh(&mut self, refit: bool) -> Result<(), String> {
        let snapshot = self
            .desktop
            .active_render_snapshot(&self.selection)
            .map_err(|error| error.to_string())?;
        if refit {
            self.camera = self
                .desktop
                .active_scene_saved_camera()
                .unwrap_or_else(|| frame_camera(&snapshot));
        }
        self.mesh = Some(tessellate_snapshot(&snapshot));
        Ok(())
    }

    fn open_paths(&mut self, paths: Vec<String>) {
        let mut loaded = 0usize;
        let mut failures = Vec::new();
        for path in paths {
            match self.desktop.open_path(&path) {
                Ok(_) => loaded += 1,
                Err(error) => failures.push(format!("{path}: {error}")),
            }
        }
        self.selection.clear();
        self.status = if failures.is_empty() {
            format!("Loaded {loaded} file(s)")
        } else {
            format!("Loaded {loaded}; failed: {}", failures.join("; "))
        };
        if let Err(error) = self.rebuild_mesh(true) {
            self.status = format!("{}; render state failed: {error}", self.status);
        }
    }

    fn render(&mut self, width: u32, height: u32) -> ViewportDto {
        let width = width.clamp(64, MAX_VIEW_DIM);
        let height = height.clamp(64, MAX_VIEW_DIM);
        let image = self
            .renderer
            .as_ref()
            .zip(self.mesh.as_ref())
            .and_then(|(renderer, mesh)| {
                if mesh.indices.is_empty() {
                    return None;
                }
                let mvp = camera_mvp(&self.camera, f64::from(width) / f64::from(height));
                renderer
                    .render(std::slice::from_ref(mesh), mvp, width, height, VIEW_CLEAR)
                    .map(|rgba| {
                        let png = encode_rgba_png(&rgba, width, height);
                        format!("data:image/png;base64,{}", base64_encode(&png))
                    })
            });
        ViewportDto {
            image,
            width,
            height,
            status: self.status.clone(),
        }
    }

    fn update_camera(
        &mut self,
        action: &str,
        x: f64,
        y: f64,
        viewport_height: u32,
    ) -> Result<(), String> {
        match action {
            "rotate" => {
                let rotation = self
                    .camera
                    .rotation()
                    .rotate_y(x)
                    .and_then(|rotation| rotation.rotate_x(y))
                    .map_err(|error| error.to_string())?;
                self.camera.set_rotation(rotation);
            }
            "pan" => {
                self.camera
                    .set_center(pan_center(&self.camera, x, y, viewport_height.max(1)));
            }
            "zoom" => {
                self.camera
                    .set_zoom((self.camera.zoom() * x.clamp(0.2, 5.0)).clamp(0.5, 1.0e6));
            }
            "fit" => {
                let snapshot = self
                    .desktop
                    .active_render_snapshot(&self.selection)
                    .map_err(|error| error.to_string())?;
                self.camera = frame_camera(&snapshot);
                self.status = "View fitted".to_string();
            }
            "projection" => {
                self.camera.set_perspective(!self.camera.is_perspective());
                self.status = if self.camera.is_perspective() {
                    "Projection: perspective".to_string()
                } else {
                    "Projection: orthographic".to_string()
                };
            }
            _ => return Err(format!("unknown camera action: {action}")),
        }
        Ok(())
    }

    fn persist_camera(&mut self) -> Result<(), String> {
        let mut camera = self.camera.clone();
        camera.set_name("__current");
        self.desktop
            .set_active_scene_camera(camera)
            .map_err(|error| error.to_string())
    }
}

enum ModelRequest {
    GetSnapshot,
    OpenPaths(Vec<String>),
    NewScene(Option<String>),
    ActivateScene(u64),
    CloseActiveScene,
    SetSelection(String),
    SetRendererVisible {
        object_uid: u64,
        renderer_uid: u64,
        visible: bool,
    },
    AddRenderer {
        object_uid: u64,
        renderer_type: String,
    },
    Render {
        width: u32,
        height: u32,
    },
    UpdateCamera {
        action: String,
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    },
    SaveScene(String),
    ExportPng {
        path: String,
        width: u32,
        height: u32,
    },
}

enum ModelResponse {
    Snapshot(AppSnapshot),
    Viewport(ViewportDto),
}

struct ModelEnvelope {
    request: ModelRequest,
    response: mpsc::Sender<Result<ModelResponse, String>>,
}

struct ManagedState {
    sender: mpsc::Sender<ModelEnvelope>,
}

fn handle_model_request(
    model: &mut AppModel,
    request: ModelRequest,
) -> Result<ModelResponse, String> {
    const RENDERER_TYPES: &[&str] = &[
        "simple",
        "ballstick",
        "cpk",
        "ribbon",
        "cartoon",
        "trace",
        "tube",
        "spline",
    ];
    match request {
        ModelRequest::GetSnapshot => Ok(ModelResponse::Snapshot(model.snapshot())),
        ModelRequest::OpenPaths(paths) => {
            model.open_paths(paths);
            Ok(ModelResponse::Snapshot(model.snapshot()))
        }
        ModelRequest::NewScene(name) => {
            let name = name
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Untitled".to_string());
            model.desktop.new_scene(name, true);
            model.selection.clear();
            model.status = "New scene".to_string();
            model.rebuild_mesh(true)?;
            Ok(ModelResponse::Snapshot(model.snapshot()))
        }
        ModelRequest::ActivateScene(scene_uid) => {
            if !model.desktop.set_active_scene(Some(Uid::new(scene_uid))) {
                return Err(format!("scene {scene_uid} was not found"));
            }
            model.selection.clear();
            model.status = format!("Activated scene {scene_uid}");
            model.rebuild_mesh(true)?;
            Ok(ModelResponse::Snapshot(model.snapshot()))
        }
        ModelRequest::CloseActiveScene => {
            model
                .desktop
                .close_active_scene()
                .map_err(|error| error.to_string())?;
            model.desktop.ensure_active_scene();
            model.selection.clear();
            model.status = "Scene closed".to_string();
            model.rebuild_mesh(true)?;
            Ok(ModelResponse::Snapshot(model.snapshot()))
        }
        ModelRequest::SetSelection(selection) => {
            let previous = std::mem::replace(&mut model.selection, selection.trim().to_string());
            if let Err(error) = model.rebuild_mesh(false) {
                model.selection = previous;
                model.rebuild_mesh(false)?;
                return Err(error);
            }
            model.status = if model.selection.is_empty() {
                "Selection cleared".to_string()
            } else {
                format!("Selection: {}", model.selection)
            };
            Ok(ModelResponse::Snapshot(model.snapshot()))
        }
        ModelRequest::SetRendererVisible {
            object_uid,
            renderer_uid,
            visible,
        } => {
            model
                .desktop
                .set_renderer_visible(Uid::new(object_uid), Uid::new(renderer_uid), visible)
                .map_err(|error| error.to_string())?;
            model.status = if visible {
                "Renderer shown".to_string()
            } else {
                "Renderer hidden".to_string()
            };
            model.rebuild_mesh(false)?;
            Ok(ModelResponse::Snapshot(model.snapshot()))
        }
        ModelRequest::AddRenderer {
            object_uid,
            renderer_type,
        } => {
            if !RENDERER_TYPES.contains(&renderer_type.as_str()) {
                return Err(format!("unsupported renderer type: {renderer_type}"));
            }
            let count = model.desktop.renderer_summaries(Uid::new(object_uid)).len();
            model
                .desktop
                .new_renderer_in_active_scene(
                    Uid::new(object_uid),
                    renderer_type.clone(),
                    format!("{renderer_type} {}", count + 1),
                )
                .map_err(|error| error.to_string())?;
            model.status = format!("Added {renderer_type} renderer");
            model.rebuild_mesh(false)?;
            Ok(ModelResponse::Snapshot(model.snapshot()))
        }
        ModelRequest::Render { width, height } => {
            Ok(ModelResponse::Viewport(model.render(width, height)))
        }
        ModelRequest::UpdateCamera {
            action,
            x,
            y,
            width,
            height,
        } => {
            model.update_camera(&action, x, y, height)?;
            Ok(ModelResponse::Viewport(model.render(width, height)))
        }
        ModelRequest::SaveScene(path) => {
            model.persist_camera()?;
            model
                .desktop
                .save_active_scene_qsc(&path)
                .map_err(|error| error.to_string())?;
            model.status = format!("Saved scene: {path}");
            Ok(ModelResponse::Snapshot(model.snapshot()))
        }
        ModelRequest::ExportPng {
            path,
            width,
            height,
        } => {
            model.persist_camera()?;
            model
                .desktop
                .export_active_scene_png(
                    &path,
                    width.clamp(64, MAX_VIEW_DIM),
                    height.clamp(64, MAX_VIEW_DIM),
                    &model.selection,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!("Exported image: {path}");
            Ok(ModelResponse::Snapshot(model.snapshot()))
        }
    }
}

fn spawn_model(initial_paths: Vec<PathBuf>) -> ManagedState {
    let (sender, receiver) = mpsc::channel::<ModelEnvelope>();
    std::thread::Builder::new()
        .name("cuemol-tauri-model".to_string())
        .spawn(move || {
            let mut model = AppModel::new(initial_paths);
            while let Ok(envelope) = receiver.recv() {
                let result = handle_model_request(&mut model, envelope.request);
                let _ = envelope.response.send(result);
            }
        })
        .expect("spawn CueMol Tauri model thread");
    ManagedState { sender }
}

fn request_model(
    state: &State<'_, ManagedState>,
    request: ModelRequest,
) -> Result<ModelResponse, String> {
    let (sender, receiver) = mpsc::channel();
    state
        .sender
        .send(ModelEnvelope {
            request,
            response: sender,
        })
        .map_err(|_| "application model thread stopped".to_string())?;
    receiver
        .recv()
        .map_err(|_| "application model did not respond".to_string())?
}

fn snapshot_response(response: ModelResponse) -> Result<AppSnapshot, String> {
    match response {
        ModelResponse::Snapshot(snapshot) => Ok(snapshot),
        ModelResponse::Viewport(_) => Err("unexpected viewport response".to_string()),
    }
}

fn viewport_response(response: ModelResponse) -> Result<ViewportDto, String> {
    match response {
        ModelResponse::Viewport(viewport) => Ok(viewport),
        ModelResponse::Snapshot(_) => Err("unexpected snapshot response".to_string()),
    }
}

#[tauri::command]
fn get_snapshot(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::GetSnapshot)?)
}

#[tauri::command]
fn open_paths(paths: Vec<String>, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::OpenPaths(paths))?)
}

#[tauri::command]
fn new_scene(name: Option<String>, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::NewScene(name))?)
}

#[tauri::command]
fn activate_scene(scene_uid: u64, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ActivateScene(scene_uid),
    )?)
}

#[tauri::command]
fn close_active_scene(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::CloseActiveScene)?)
}

#[tauri::command]
fn set_selection(selection: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetSelection(selection),
    )?)
}

#[tauri::command]
fn set_renderer_visible(
    object_uid: u64,
    renderer_uid: u64,
    visible: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetRendererVisible {
            object_uid,
            renderer_uid,
            visible,
        },
    )?)
}

#[tauri::command]
fn add_renderer(
    object_uid: u64,
    renderer_type: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AddRenderer {
            object_uid,
            renderer_type,
        },
    )?)
}

#[tauri::command]
fn render_viewport(
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<ViewportDto, String> {
    viewport_response(request_model(
        &state,
        ModelRequest::Render { width, height },
    )?)
}

#[tauri::command]
fn update_camera(
    action: String,
    x: f64,
    y: f64,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<ViewportDto, String> {
    viewport_response(request_model(
        &state,
        ModelRequest::UpdateCamera {
            action,
            x,
            y,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn save_scene(path: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::SaveScene(path))?)
}

#[tauri::command]
fn export_png(
    path: String,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ExportPng {
            path,
            width,
            height,
        },
    )?)
}

fn frame_camera(snapshot: &RenderSceneSnapshot) -> Camera {
    let mut camera = Camera::new();
    if let Some((min, max)) = snapshot.bounds {
        let size = ((max.x - min.x).powi(2) + (max.y - min.y).powi(2) + (max.z - min.z).powi(2))
            .sqrt()
            .max(1.0);
        camera.set_center(Vector4::new3(
            (min.x + max.x) / 2.0,
            (min.y + max.y) / 2.0,
            (min.z + max.z) / 2.0,
        ));
        camera.set_zoom(size * 1.2);
        camera.set_cam_dist(size * 3.0);
        camera.set_slab_depth(size * 2.5);
    }
    camera
}

fn pan_center(camera: &Camera, dx_pix: f64, dy_pix: f64, viewport_height: u32) -> Vector4 {
    let scale = camera.zoom() / f64::from(viewport_height.max(1));
    let rotation = camera.rotation().to_matrix();
    let center = camera.center();
    let camera_x = dx_pix * scale;
    let camera_y = -dy_pix * scale;
    Vector4::new3(
        center.x - camera_x * rotation.values[0][0] - camera_y * rotation.values[1][0],
        center.y - camera_x * rotation.values[0][1] - camera_y * rotation.values[1][1],
        center.z - camera_x * rotation.values[0][2] - camera_y * rotation.values[1][2],
    )
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let byte_0 = u32::from(chunk[0]);
        let byte_1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let byte_2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let value = (byte_0 << 16) | (byte_1 << 8) | byte_2;
        output.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

pub fn run() -> Result<(), tauri::Error> {
    let initial_paths = std::env::args_os().skip(1).map(PathBuf::from).collect();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(spawn_model(initial_paths))
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            open_paths,
            new_scene,
            activate_scene,
            close_active_scene,
            set_selection,
            set_renderer_visible,
            add_renderer,
            render_viewport,
            update_camera,
            save_scene,
            export_png,
        ])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_camera_fits_snapshot_bounds() {
        let mut snapshot = RenderSceneSnapshot::empty();
        snapshot.bounds = Some((
            Vector4::new3(-2.0, -1.0, -3.0),
            Vector4::new3(2.0, 3.0, 1.0),
        ));
        let camera = frame_camera(&snapshot);
        assert_eq!(camera.center(), Vector4::new3(0.0, 1.0, -1.0));
        assert!(camera.zoom() > 4.0);
    }

    #[test]
    fn base64_encoder_uses_standard_padding() {
        assert_eq!(base64_encode(b"CueMol"), "Q3VlTW9s");
        assert_eq!(base64_encode(b"rs"), "cnM=");
    }
}
