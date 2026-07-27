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
    coloring_type: Option<String>,
    renderers: Vec<RendererDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructAtomDto {
    atom_index: usize,
    serial: i32,
    name: String,
    element: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructResidueDto {
    res_name: String,
    res_id: i32,
    ins_code: Option<char>,
    atoms: Vec<StructAtomDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructChainDto {
    chain_id: String,
    residues: Vec<StructResidueDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructObjectDto {
    object_uid: u64,
    object_name: String,
    chains: Vec<StructChainDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SequenceResidueDto {
    letter: String,
    res_id: i32,
    res_name: String,
    selection: String,
    selected: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SequenceChainDto {
    object_uid: u64,
    object_name: String,
    chain_id: String,
    residues: Vec<SequenceResidueDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CrystalDto {
    object_uid: u64,
    object_name: String,
    a: f64,
    b: f64,
    c: f64,
    alpha: f64,
    beta: f64,
    gamma: f64,
    space_group_name: String,
    space_group_number: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DensityMapDto {
    uid: u64,
    name: String,
    format: String,
    counts: [usize; 3],
    min_density: f64,
    max_density: f64,
    mean_density: f64,
    rmsd_density: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshot {
    scenes: Vec<SceneDto>,
    objects: Vec<ObjectDto>,
    structure: Vec<StructObjectDto>,
    sequence: Vec<SequenceChainDto>,
    crystals: Vec<CrystalDto>,
    density_maps: Vec<DensityMapDto>,
    active_scene_uid: Option<u64>,
    selection: String,
    measure_result: Option<String>,
    perspective: bool,
    background: String,
    show_unit_cell: bool,
    show_symmetry_mates: bool,
    show_density_contour: bool,
    color_proofing: bool,
    density_level: f64,
    can_undo: bool,
    can_redo: bool,
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
    background: String,
    show_unit_cell: bool,
    show_symmetry_mates: bool,
    show_density_contour: bool,
    color_proofing: bool,
    density_level: f64,
    undo_stack: Vec<AppModelUndo>,
    redo_stack: Vec<AppModelUndo>,
    status: String,
}

#[derive(Clone)]
struct AppModelUndo {
    desktop: DesktopAppState,
    camera: Camera,
    selection: String,
    background: String,
    show_unit_cell: bool,
    show_symmetry_mates: bool,
    show_density_contour: bool,
    color_proofing: bool,
    density_level: f64,
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
            background: "black".to_string(),
            show_unit_cell: false,
            show_symmetry_mates: false,
            show_density_contour: false,
            color_proofing: false,
            density_level: 1.5,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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
                    coloring_type: self.desktop.object_coloring_type(object.uid),
                    renderers,
                });
            }
        }
        let structure = self
            .desktop
            .active_structure_tree()
            .into_iter()
            .map(|object| StructObjectDto {
                object_uid: object.object_uid.get(),
                object_name: object.object_name,
                chains: object
                    .chains
                    .into_iter()
                    .map(|chain| StructChainDto {
                        chain_id: chain.chain_id,
                        residues: chain
                            .residues
                            .into_iter()
                            .map(|residue| StructResidueDto {
                                res_name: residue.res_name,
                                res_id: residue.res_id,
                                ins_code: residue.ins_code,
                                atoms: residue
                                    .atoms
                                    .into_iter()
                                    .map(|atom| StructAtomDto {
                                        atom_index: atom.atom_index,
                                        serial: atom.serial,
                                        name: atom.name,
                                        element: atom.element,
                                    })
                                    .collect(),
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect();
        let sequence = self
            .desktop
            .active_sequence_view(&self.selection)
            .into_iter()
            .map(|chain| SequenceChainDto {
                object_uid: chain.object_uid.get(),
                object_name: chain.object_name,
                chain_id: chain.chain_id,
                residues: chain
                    .residues
                    .into_iter()
                    .map(|residue| SequenceResidueDto {
                        letter: residue.letter,
                        res_id: residue.res_id,
                        res_name: residue.res_name,
                        selection: residue.selection,
                        selected: residue.selected,
                    })
                    .collect(),
            })
            .collect();
        let crystals = self
            .desktop
            .active_crystal_summaries()
            .into_iter()
            .map(|crystal| CrystalDto {
                object_uid: crystal.object_uid.get(),
                object_name: crystal.object_name,
                a: crystal.a,
                b: crystal.b,
                c: crystal.c,
                alpha: crystal.alpha,
                beta: crystal.beta,
                gamma: crystal.gamma,
                space_group_name: crystal.space_group_name,
                space_group_number: crystal.space_group_number,
            })
            .collect();
        let density_maps = self
            .desktop
            .active_density_map_summaries()
            .into_iter()
            .map(|map| DensityMapDto {
                uid: map.uid.get(),
                name: map.name,
                format: map.format,
                counts: map.counts,
                min_density: map.min_density,
                max_density: map.max_density,
                mean_density: map.mean_density,
                rmsd_density: map.rmsd_density,
            })
            .collect();
        AppSnapshot {
            scenes,
            objects,
            structure,
            sequence,
            crystals,
            density_maps,
            active_scene_uid: self.desktop.active_scene().map(Uid::get),
            selection: self.selection.clone(),
            measure_result: self.desktop.active_measure(&self.selection),
            perspective: self.camera.is_perspective(),
            background: self.background.clone(),
            show_unit_cell: self.show_unit_cell,
            show_symmetry_mates: self.show_symmetry_mates,
            show_density_contour: self.show_density_contour,
            color_proofing: self.color_proofing,
            density_level: self.density_level,
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            status: self.status.clone(),
        }
    }

    fn rebuild_mesh(&mut self, refit: bool) -> Result<(), String> {
        let mut snapshot = self
            .desktop
            .active_render_snapshot(&self.selection)
            .map_err(|error| error.to_string())?;
        self.desktop.append_crystal_geometry(
            &mut snapshot,
            self.show_unit_cell,
            self.show_symmetry_mates,
        );
        self.desktop.append_density_geometry(
            &mut snapshot,
            self.density_level,
            self.show_density_contour,
        );
        if self.color_proofing && self.desktop.apply_color_proof(&mut snapshot).is_err() {
            cuemol_render::apply_generic_color_proof(&mut snapshot);
        }
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
                    .render(
                        std::slice::from_ref(mesh),
                        mvp,
                        width,
                        height,
                        if self.background == "white" {
                            [1.0, 1.0, 1.0, 1.0]
                        } else {
                            [0.0, 0.0, 0.0, 1.0]
                        },
                    )
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

    fn capture_undo(&self) -> AppModelUndo {
        AppModelUndo {
            desktop: self.desktop.clone(),
            camera: self.camera.clone(),
            selection: self.selection.clone(),
            background: self.background.clone(),
            show_unit_cell: self.show_unit_cell,
            show_symmetry_mates: self.show_symmetry_mates,
            show_density_contour: self.show_density_contour,
            color_proofing: self.color_proofing,
            density_level: self.density_level,
        }
    }

    fn push_undo(&mut self, snapshot: AppModelUndo) {
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn restore_undo(&mut self, snapshot: AppModelUndo) -> Result<(), String> {
        self.desktop = snapshot.desktop;
        self.camera = snapshot.camera;
        self.selection = snapshot.selection;
        self.background = snapshot.background;
        self.show_unit_cell = snapshot.show_unit_cell;
        self.show_symmetry_mates = snapshot.show_symmetry_mates;
        self.show_density_contour = snapshot.show_density_contour;
        self.color_proofing = snapshot.color_proofing;
        self.density_level = snapshot.density_level;
        self.rebuild_mesh(false)
    }

    fn undo(&mut self) -> Result<(), String> {
        let Some(snapshot) = self.undo_stack.pop() else {
            self.status = "Nothing to undo".to_string();
            return Ok(());
        };
        let redo = self.capture_undo();
        self.restore_undo(snapshot)?;
        self.redo_stack.push(redo);
        self.status = "Undo".to_string();
        Ok(())
    }

    fn redo(&mut self) -> Result<(), String> {
        let Some(snapshot) = self.redo_stack.pop() else {
            self.status = "Nothing to redo".to_string();
            return Ok(());
        };
        let undo = self.capture_undo();
        self.restore_undo(snapshot)?;
        self.undo_stack.push(undo);
        self.status = "Redo".to_string();
        Ok(())
    }

    fn clear_undo(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.status = "Undo data cleared".to_string();
    }
}

enum ModelRequest {
    GetSnapshot,
    Undo,
    Redo,
    ClearUndo,
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
    SetObjectColoring {
        object_uid: u64,
        coloring: String,
    },
    DeleteAtoms {
        object_uid: u64,
        selection: String,
    },
    ChangeChain {
        object_uid: u64,
        selection: String,
        chain_name: String,
    },
    RenumberResidues {
        object_uid: u64,
        selection: String,
        start: i32,
    },
    SetDisplayOption {
        option: String,
        value: bool,
    },
    SetDensityLevel(f64),
    SetBackground(String),
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
    Snapshot(Box<AppSnapshot>),
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
        ModelRequest::GetSnapshot => Ok(ModelResponse::Snapshot(Box::new(model.snapshot()))),
        ModelRequest::Undo => {
            model.undo()?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::Redo => {
            model.redo()?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ClearUndo => {
            model.clear_undo();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::OpenPaths(paths) => {
            let undo = model.capture_undo();
            model.open_paths(paths);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::NewScene(name) => {
            let undo = model.capture_undo();
            let name = name
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Untitled".to_string());
            model.desktop.new_scene(name, true);
            model.selection.clear();
            model.status = "New scene".to_string();
            model.rebuild_mesh(true)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ActivateScene(scene_uid) => {
            if !model.desktop.set_active_scene(Some(Uid::new(scene_uid))) {
                return Err(format!("scene {scene_uid} was not found"));
            }
            model.selection.clear();
            model.status = format!("Activated scene {scene_uid}");
            model.rebuild_mesh(true)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CloseActiveScene => {
            let undo = model.capture_undo();
            model
                .desktop
                .close_active_scene()
                .map_err(|error| error.to_string())?;
            model.desktop.ensure_active_scene();
            model.selection.clear();
            model.status = "Scene closed".to_string();
            model.rebuild_mesh(true)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
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
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetRendererVisible {
            object_uid,
            renderer_uid,
            visible,
        } => {
            let undo = model.capture_undo();
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
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AddRenderer {
            object_uid,
            renderer_type,
        } => {
            if !RENDERER_TYPES.contains(&renderer_type.as_str()) {
                return Err(format!("unsupported renderer type: {renderer_type}"));
            }
            let undo = model.capture_undo();
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
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetObjectColoring {
            object_uid,
            coloring,
        } => {
            let undo = model.capture_undo();
            let (coloring_type, attrs) = match coloring.as_str() {
                "element" => (None, std::collections::BTreeMap::new()),
                "rainbow" => {
                    let mut attrs = std::collections::BTreeMap::new();
                    attrs.insert("mode".to_string(), "mol".to_string());
                    attrs.insert("start_hue".to_string(), "240".to_string());
                    attrs.insert("end_hue".to_string(), "0".to_string());
                    attrs.insert("sat".to_string(), "1".to_string());
                    attrs.insert("bri".to_string(), "1".to_string());
                    (Some("RainbowColoring".to_string()), attrs)
                }
                "bfac" => (
                    Some("BfacColoring".to_string()),
                    std::collections::BTreeMap::new(),
                ),
                _ => return Err(format!("unsupported coloring type: {coloring}")),
            };
            model
                .desktop
                .set_object_coloring(Uid::new(object_uid), coloring_type, attrs)
                .map_err(|error| error.to_string())?;
            model.status = format!("Applied {coloring} coloring");
            model.rebuild_mesh(false)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::DeleteAtoms {
            object_uid,
            selection,
        } => {
            let selection = selection.trim();
            if selection.is_empty() {
                return Err("an atom selection is required".to_string());
            }
            let undo = model.capture_undo();
            let count = model
                .desktop
                .delete_atoms_in_object(Uid::new(object_uid), selection)
                .map_err(|error| error.to_string())?;
            if count > 0 {
                model.selection.clear();
                model.rebuild_mesh(true)?;
                model.push_undo(undo);
            }
            model.status = format!("Deleted {count} atom(s)");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ChangeChain {
            object_uid,
            selection,
            chain_name,
        } => {
            let selection = selection.trim();
            if selection.is_empty() {
                return Err("an atom selection is required".to_string());
            }
            let undo = model.capture_undo();
            let count = model
                .desktop
                .change_chain_in_object(Uid::new(object_uid), selection, &chain_name)
                .map_err(|error| error.to_string())?;
            if count > 0 {
                model.rebuild_mesh(false)?;
                model.push_undo(undo);
            }
            model.status = format!("Changed chain ID for {count} atom(s)");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RenumberResidues {
            object_uid,
            selection,
            start,
        } => {
            let selection = selection.trim();
            if selection.is_empty() {
                return Err("an atom selection is required".to_string());
            }
            let undo = model.capture_undo();
            let count = model
                .desktop
                .renumber_residues_in_object(Uid::new(object_uid), selection, start)
                .map_err(|error| error.to_string())?;
            if count > 0 {
                model.rebuild_mesh(false)?;
                model.push_undo(undo);
            }
            model.status = format!("Renumbered residues for {count} atom(s)");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetDisplayOption { option, value } => {
            let undo = model.capture_undo();
            match option.as_str() {
                "unit-cell" => model.show_unit_cell = value,
                "symmetry-mates" => model.show_symmetry_mates = value,
                "density-contour" => model.show_density_contour = value,
                "color-proof" => model.color_proofing = value,
                _ => return Err(format!("unsupported display option: {option}")),
            }
            model.status = format!("{option}: {}", if value { "shown" } else { "hidden" });
            model.rebuild_mesh(false)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetDensityLevel(level) => {
            if !level.is_finite() {
                return Err("density level must be finite".to_string());
            }
            let undo = model.capture_undo();
            model.density_level = level.clamp(0.1, 10.0);
            model.status = format!("Density level: {:.2} σ", model.density_level);
            model.rebuild_mesh(false)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetBackground(background) => {
            if !matches!(background.as_str(), "black" | "white") {
                return Err(format!("unsupported background: {background}"));
            }
            let undo = model.capture_undo();
            model.background = background;
            model.status = format!("Background: {}", model.background);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
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
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
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
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
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
        ModelResponse::Snapshot(snapshot) => Ok(*snapshot),
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
fn undo(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::Undo)?)
}

#[tauri::command]
fn redo(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::Redo)?)
}

#[tauri::command]
fn clear_undo(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::ClearUndo)?)
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
fn set_object_coloring(
    object_uid: u64,
    coloring: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetObjectColoring {
            object_uid,
            coloring,
        },
    )?)
}

#[tauri::command]
fn delete_atoms(
    object_uid: u64,
    selection: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::DeleteAtoms {
            object_uid,
            selection,
        },
    )?)
}

#[tauri::command]
fn change_chain(
    object_uid: u64,
    selection: String,
    chain_name: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ChangeChain {
            object_uid,
            selection,
            chain_name,
        },
    )?)
}

#[tauri::command]
fn renumber_residues(
    object_uid: u64,
    selection: String,
    start: i32,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RenumberResidues {
            object_uid,
            selection,
            start,
        },
    )?)
}

#[tauri::command]
fn set_display_option(
    option: String,
    value: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetDisplayOption { option, value },
    )?)
}

#[tauri::command]
fn set_density_level(level: f64, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::SetDensityLevel(level))?)
}

#[tauri::command]
fn set_background(
    background: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetBackground(background),
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
            undo,
            redo,
            clear_undo,
            open_paths,
            new_scene,
            activate_scene,
            close_active_scene,
            set_selection,
            set_renderer_visible,
            add_renderer,
            set_object_coloring,
            delete_atoms,
            change_chain,
            renumber_residues,
            set_display_option,
            set_density_level,
            set_background,
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

    fn model_without_gpu() -> AppModel {
        let mut desktop = DesktopAppState::new();
        desktop.ensure_active_scene();
        AppModel {
            desktop,
            camera: Camera::new(),
            renderer: None,
            mesh: None,
            selection: String::new(),
            background: "black".to_string(),
            show_unit_cell: false,
            show_symmetry_mates: false,
            show_density_contour: false,
            color_proofing: false,
            density_level: 1.5,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            status: "Ready".to_string(),
        }
    }

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

    #[test]
    fn panel_display_options_update_the_snapshot() {
        let mut model = model_without_gpu();
        handle_model_request(
            &mut model,
            ModelRequest::SetDisplayOption {
                option: "unit-cell".to_string(),
                value: true,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SetDisplayOption {
                option: "color-proof".to_string(),
                value: true,
            },
        )
        .unwrap();
        handle_model_request(&mut model, ModelRequest::SetDensityLevel(2.4)).unwrap();
        let response =
            handle_model_request(&mut model, ModelRequest::SetBackground("white".to_string()))
                .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert!(snapshot.show_unit_cell);
        assert!(snapshot.color_proofing);
        assert_eq!(snapshot.density_level, 2.4);
        assert_eq!(snapshot.background, "white");
    }

    #[test]
    fn model_undo_and_redo_restore_gui_mutations() {
        let mut model = model_without_gpu();
        handle_model_request(&mut model, ModelRequest::SetBackground("white".to_string())).unwrap();
        assert!(model.snapshot().can_undo);
        handle_model_request(&mut model, ModelRequest::Undo).unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.background, "black");
        assert!(snapshot.can_redo);
        handle_model_request(&mut model, ModelRequest::Redo).unwrap();
        assert_eq!(model.snapshot().background, "white");
    }

    #[test]
    fn shared_core_populates_structure_sequence_and_coloring_panels() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.selection = "chain A and resid 1".to_string();
        model.rebuild_mesh(true).unwrap();
        let object_uid = model.desktop.active_object_summaries()[0].uid;
        let response = handle_model_request(
            &mut model,
            ModelRequest::SetObjectColoring {
                object_uid: object_uid.get(),
                coloring: "rainbow".to_string(),
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(snapshot.structure[0].object_name, "1CRN");
        assert_eq!(snapshot.structure[0].chains[0].chain_id, "A");
        assert_eq!(snapshot.sequence[0].residues[0].letter, "T");
        assert!(snapshot.sequence[0].residues[0].selected);
        assert_eq!(
            snapshot.objects[0].coloring_type.as_deref(),
            Some("RainbowColoring")
        );
    }

    #[test]
    fn edit_menu_atom_deletion_is_undoable() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();
        let object_uid = model.desktop.active_object_summaries()[0].uid;
        handle_model_request(
            &mut model,
            ModelRequest::DeleteAtoms {
                object_uid: object_uid.get(),
                selection: "aid 1".to_string(),
            },
        )
        .unwrap();
        assert_eq!(model.snapshot().objects[0].atom_count, 326);
        handle_model_request(&mut model, ModelRequest::Undo).unwrap();
        assert_eq!(model.snapshot().objects[0].atom_count, 327);
    }
}
