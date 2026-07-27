//! Tauri desktop shell backed by the shared [`DesktopAppState`].

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;

use cuemol_core::{Quat, Uid, Vector4};
use cuemol_qsys::Camera;
use cuemol_render::{tessellate_snapshot, RenderMesh, RenderSceneSnapshot};
use serde::Serialize;
use tauri::State;

use crate::offscreen::{camera_mvp, encode_rgba_png, SceneRenderer};
use crate::{
    anim_step, atoms_in_screen_rect, pick_atom_screen, AnimKind, AnimObject, DesktopAppState,
};

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
struct AnimationObjectDto {
    index: usize,
    name: String,
    start: f64,
    end: f64,
    duration: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MorphFrameDto {
    index: usize,
    name: String,
    source: String,
    source_type: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MorphObjectDto {
    object_uid: u64,
    frame: f64,
    frames: Vec<MorphFrameDto>,
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
    animation_objects: Vec<AnimationObjectDto>,
    animation_elapsed: f64,
    animation_duration: f64,
    animation_playing: bool,
    animation_loop: bool,
    morphs: Vec<MorphObjectDto>,
    can_undo: bool,
    can_redo: bool,
    log_lines: Vec<String>,
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
    animation_objects: Vec<AnimObject>,
    animation_elapsed: f64,
    animation_playing: bool,
    animation_loop: bool,
    animation_start_camera: Option<Camera>,
    undo_stack: Vec<AppModelUndo>,
    redo_stack: Vec<AppModelUndo>,
    log_lines: Vec<String>,
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
    animation_objects: Vec<AnimObject>,
    animation_elapsed: f64,
    animation_loop: bool,
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
            animation_objects: Vec::new(),
            animation_elapsed: 0.0,
            animation_playing: false,
            animation_loop: false,
            animation_start_camera: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            log_lines: Vec::new(),
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
        let mut animation_start = 0.0;
        let animation_objects = self
            .animation_objects
            .iter()
            .enumerate()
            .map(|(index, object)| {
                let duration = object.duration.max(0.01);
                let start = animation_start;
                animation_start += duration;
                AnimationObjectDto {
                    index,
                    name: object.kind.label().to_string(),
                    start,
                    end: animation_start,
                    duration,
                }
            })
            .collect();
        let morphs = objects
            .iter()
            .filter_map(|object| {
                let object_uid = Uid::new(object.uid);
                let frames = self.desktop.morph_frame_summaries(object_uid);
                (!frames.is_empty()).then(|| MorphObjectDto {
                    object_uid: object.uid,
                    frame: self.desktop.morph_frame_value(object_uid).unwrap_or(0.0),
                    frames: frames
                        .into_iter()
                        .map(|frame| MorphFrameDto {
                            index: frame.index,
                            name: frame.name,
                            source: frame.source,
                            source_type: frame.source_type,
                        })
                        .collect(),
                })
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
            animation_objects,
            animation_elapsed: self.animation_elapsed,
            animation_duration: animation_start,
            animation_playing: self.animation_playing,
            animation_loop: self.animation_loop,
            morphs,
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            log_lines: self.log_lines.clone(),
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

    fn render_png_bytes(&self, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let width = width.clamp(64, MAX_VIEW_DIM);
        let height = height.clamp(64, MAX_VIEW_DIM);
        let renderer = self
            .renderer
            .as_ref()
            .ok_or_else(|| "the GPU renderer is unavailable".to_string())?;
        let mesh = self
            .mesh
            .as_ref()
            .filter(|mesh| !mesh.indices.is_empty())
            .ok_or_else(|| "the active scene has no renderable geometry".to_string())?;
        let mvp = camera_mvp(&self.camera, f64::from(width) / f64::from(height));
        let rgba = renderer
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
            .ok_or_else(|| "rendering the animation frame failed".to_string())?;
        Ok(encode_rgba_png(&rgba, width, height))
    }

    fn render_animation_frames(
        &mut self,
        output_dir: &str,
        base_name: &str,
        width: u32,
        height: u32,
        frame_rate: f64,
    ) -> Result<usize, String> {
        if self.animation_objects.is_empty() {
            return Err("no animation objects are defined".to_string());
        }
        if !frame_rate.is_finite() || !(1.0..=120.0).contains(&frame_rate) {
            return Err("frame rate must be between 1 and 120 fps".to_string());
        }
        let base_name = base_name.trim();
        if base_name.is_empty()
            || base_name.contains('/')
            || base_name.contains('\\')
            || base_name == "."
            || base_name == ".."
        {
            return Err("output base name must be a simple file name".to_string());
        }
        let total = self.animation_duration();
        let frame_count = ((total * frame_rate).ceil() as usize + 1).min(10_000);
        if frame_count == 10_000 && total * frame_rate >= 10_000.0 {
            return Err("animation rendering is limited to 9999 frames".to_string());
        }
        let output_dir = Path::new(output_dir);
        fs::create_dir_all(output_dir).map_err(|error| error.to_string())?;

        let original_desktop = self.desktop.clone();
        let original_camera = self.camera.clone();
        let original_elapsed = self.animation_elapsed;
        let original_playing = self.animation_playing;
        let original_start_camera = self.animation_start_camera.clone();

        self.animation_elapsed = 0.0;
        self.animation_playing = true;
        self.animation_start_camera = Some(self.camera.clone());
        let render_result = (|| {
            for index in 0..frame_count {
                let target = (index as f64 / frame_rate).min(total);
                while self.animation_elapsed + 1.0e-9 < target {
                    self.animation_playing = true;
                    self.tick_animation((target - self.animation_elapsed).min(0.25))?;
                }
                let png = self.render_png_bytes(width, height)?;
                let path = output_dir.join(format!("{base_name}_{index:05}.png"));
                fs::write(&path, png).map_err(|error| error.to_string())?;
            }
            Ok(frame_count)
        })();

        self.desktop = original_desktop;
        self.camera = original_camera;
        self.animation_elapsed = original_elapsed;
        self.animation_playing = original_playing;
        self.animation_start_camera = original_start_camera;
        self.rebuild_mesh(false)?;
        render_result
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

    fn pick_atom(&mut self, x: f64, y: f64, width: u32, height: u32) -> Result<(), String> {
        let snapshot = self
            .desktop
            .active_render_snapshot(&self.selection)
            .map_err(|error| error.to_string())?;
        let mvp = camera_mvp(
            &self.camera,
            f64::from(width.max(1)) / f64::from(height.max(1)),
        );
        let Some((object_uid, atom_index)) = pick_atom_screen(
            &snapshot.atoms,
            mvp,
            (x, y),
            (f64::from(width), f64::from(height)),
            10.0,
        ) else {
            self.status = "No atom at cursor".to_string();
            return Ok(());
        };
        let atom = self
            .desktop
            .atom_summary(Uid::new(object_uid), atom_index)
            .map_err(|error| error.to_string())?;
        self.selection = atom.selection_text;
        self.status = format!(
            "Selected atom {} {}{}:{}",
            atom.serial, atom.residue_name, atom.residue_id, atom.name
        );
        self.rebuild_mesh(false)
    }

    fn select_screen_rect(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let snapshot = self
            .desktop
            .active_render_snapshot(&self.selection)
            .map_err(|error| error.to_string())?;
        let mvp = camera_mvp(
            &self.camera,
            f64::from(width.max(1)) / f64::from(height.max(1)),
        );
        let hits = atoms_in_screen_rect(
            &snapshot.atoms,
            mvp,
            (x1, y1),
            (x2, y2),
            (f64::from(width), f64::from(height)),
        );
        let serials = hits
            .into_iter()
            .filter_map(|(object_uid, atom_index)| {
                self.desktop
                    .atom_summary(Uid::new(object_uid), atom_index)
                    .ok()
                    .map(|atom| atom.serial)
            })
            .collect::<std::collections::BTreeSet<_>>();
        self.selection = if serials.is_empty() {
            String::new()
        } else {
            format!(
                "aid {}",
                serials
                    .iter()
                    .map(i32::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        self.status = format!("Rectangle selected {} atom(s)", serials.len());
        self.rebuild_mesh(false)
    }

    fn animation_duration(&self) -> f64 {
        self.animation_objects
            .iter()
            .map(|object| object.duration.max(0.01))
            .sum()
    }

    fn apply_animation_visibility(&mut self, visible: bool) -> Result<bool, String> {
        let mut changed = false;
        for object in self.desktop.active_object_summaries() {
            for renderer in self.desktop.renderer_summaries(object.uid) {
                if renderer.visible != visible {
                    self.desktop
                        .set_renderer_visible(object.uid, renderer.uid, visible)
                        .map_err(|error| error.to_string())?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }

    fn tick_animation(&mut self, dt: f64) -> Result<(), String> {
        if !self.animation_playing {
            return Ok(());
        }
        let total = self.animation_duration();
        if total <= 0.0 {
            self.animation_playing = false;
            self.status = "No animation objects".to_string();
            return Ok(());
        }
        let reaches_end = !self.animation_loop && self.animation_elapsed + dt >= total;
        let step = if reaches_end {
            (total - self.animation_elapsed - 1.0e-6).max(0.0)
        } else {
            dt.clamp(0.0, 0.25)
        };
        let frame = anim_step(&self.animation_objects, self.animation_elapsed, step);
        self.animation_elapsed = if reaches_end { total } else { frame.elapsed };
        if frame.rotate_dir != 0.0 {
            let rotation = self
                .camera
                .rotation()
                .rotate_y(frame.rotate_dir * step * 45.0)
                .map_err(|error| error.to_string())?;
            self.camera.set_rotation(rotation);
        }
        if frame.slide_dir != 0.0 {
            let factor = (1.0 + frame.slide_dir * step * 0.3).clamp(0.2, 5.0);
            self.camera
                .set_zoom((self.camera.zoom() * factor).clamp(0.5, 1.0e6));
        }
        let mut rebuild = false;
        if let Some(visible) = frame.set_visible {
            rebuild |= self.apply_animation_visibility(visible)?;
        }
        if let Some(fraction) = frame.frame_fraction {
            if let Some(object_uid) = self
                .desktop
                .active_object_summaries()
                .first()
                .map(|object| object.uid)
            {
                if self.desktop.object_has_trajectory(object_uid) {
                    self.desktop.apply_trajectory_fraction(object_uid, fraction);
                    rebuild = true;
                }
            }
        }
        if rebuild {
            self.rebuild_mesh(false)?;
        }
        if reaches_end {
            self.animation_playing = false;
            self.status = "Animation finished".to_string();
        }
        Ok(())
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
            animation_objects: self.animation_objects.clone(),
            animation_elapsed: self.animation_elapsed,
            animation_loop: self.animation_loop,
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
        self.animation_objects = snapshot.animation_objects;
        self.animation_elapsed = snapshot.animation_elapsed;
        self.animation_loop = snapshot.animation_loop;
        self.animation_playing = false;
        self.animation_start_camera = None;
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
    OpenPdb(String),
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
    MergeMolecules {
        source_object_uid: u64,
        selection: String,
        target_object_uid: u64,
        copy: bool,
    },
    SuperposeObjects {
        algorithm: String,
        reference_object_uid: u64,
        reference_selection: String,
        moving_object_uid: u64,
        moving_selection: String,
        auto_recenter: bool,
        use_property: bool,
    },
    EditBond {
        object_uid: u64,
        aid1: i32,
        aid2: i32,
        make: bool,
    },
    CalculateContacts {
        object_uid: u64,
        selection: String,
        r_min: f64,
        r_max: f64,
        hbond: bool,
        max_count: usize,
    },
    AssignSecondaryStructure {
        object_uid: u64,
        ignore_bulge: bool,
        angle: f64,
    },
    WriteSurface {
        object_uid: u64,
        selection: String,
        density: f64,
        probe_radius: f64,
        path: String,
    },
    WriteCutSurface {
        object_uid: u64,
        selection: String,
        density: f64,
        probe_radius: f64,
        normal: [f64; 3],
        position: [f64; 3],
        path: String,
    },
    WriteApbsFiles {
        object_uid: u64,
        selection: String,
        pqr_path: String,
        input_path: String,
        dx_path: String,
    },
    AppendMorphFrame {
        target_object_uid: u64,
        source_object_uid: u64,
        frame_name: String,
    },
    RemoveMorphFrame {
        object_uid: u64,
        index: usize,
    },
    ApplyMorphFrame {
        object_uid: u64,
        frame: f64,
    },
    SetDisplayOption {
        option: String,
        value: bool,
    },
    SetDensityLevel(f64),
    SetBackground(String),
    PickAtom {
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    },
    SelectScreenRect {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        width: u32,
        height: u32,
    },
    AddAnimationObject(String),
    DeleteAnimationObject(usize),
    MoveAnimationObject {
        index: usize,
        direction: i32,
    },
    SetAnimationDuration {
        index: usize,
        duration: f64,
    },
    AnimationControl(String),
    SetAnimationLoop(bool),
    SeekAnimation(f64),
    TickAnimation(f64),
    ClearLog,
    ExecuteScript(String),
    ExportPov {
        path: String,
    },
    RenderAnimation {
        output_dir: String,
        base_name: String,
        width: u32,
        height: u32,
        frame_rate: f64,
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
    SaveObject {
        object_uid: u64,
        path: String,
    },
    SaveView(String),
    LoadView(String),
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

fn animation_kind(value: &str) -> Option<AnimKind> {
    match value {
        "simple-spin" => Some(AnimKind::Spin),
        "camera-motion" => Some(AnimKind::CameraMotion),
        "rock" => Some(AnimKind::Rock),
        "show" => Some(AnimKind::ShowAnim),
        "hide" => Some(AnimKind::HideAnim),
        "slide-in" => Some(AnimKind::SlideIn),
        "slide-out" => Some(AnimKind::SlideOut),
        "mol-anim" => Some(AnimKind::MolAnim),
        "noop" => Some(AnimKind::Pause),
        _ => None,
    }
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
        ModelRequest::OpenPdb(pdb_id) => {
            let path = download_pdb_to_temp(&pdb_id)?;
            let undo = model.capture_undo();
            let result = model
                .desktop
                .open_path(&path)
                .map_err(|error| error.to_string());
            let _ = fs::remove_file(&path);
            result?;
            model.selection.clear();
            model.status = format!("Opened PDB {}", pdb_id.trim().to_ascii_uppercase());
            model.rebuild_mesh(true)?;
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
        ModelRequest::MergeMolecules {
            source_object_uid,
            selection,
            target_object_uid,
            copy,
        } => {
            let undo = model.capture_undo();
            let count = model
                .desktop
                .merge_molecules(
                    Uid::new(source_object_uid),
                    &selection,
                    Uid::new(target_object_uid),
                    copy,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "{} {count} atom(s) into the target molecule",
                if copy { "Copied" } else { "Moved" }
            );
            model.rebuild_mesh(true)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SuperposeObjects {
            algorithm,
            reference_object_uid,
            reference_selection,
            moving_object_uid,
            moving_selection,
            auto_recenter,
            use_property,
        } => {
            if reference_object_uid == moving_object_uid {
                return Err("reference and moving objects must differ".to_string());
            }
            let undo = model.capture_undo();
            model
                .desktop
                .superpose_objects_with_algorithm(
                    Uid::new(reference_object_uid),
                    &reference_selection,
                    Uid::new(moving_object_uid),
                    &moving_selection,
                    &algorithm,
                    use_property,
                )
                .map_err(|error| error.to_string())?;
            model.status = "Molecular superposition applied".to_string();
            model.rebuild_mesh(auto_recenter)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::EditBond {
            object_uid,
            aid1,
            aid2,
            make,
        } => {
            if aid1 == aid2 {
                return Err("two different atom serials are required".to_string());
            }
            let undo = model.capture_undo();
            let changed = if make {
                model
                    .desktop
                    .make_bond_in_object(Uid::new(object_uid), aid1, aid2)
            } else {
                model
                    .desktop
                    .remove_bond_in_object(Uid::new(object_uid), aid1, aid2)
            }
            .map_err(|error| error.to_string())?;
            if changed {
                model.rebuild_mesh(false)?;
                model.push_undo(undo);
            }
            model.status = if changed {
                if make {
                    "Bond created"
                } else {
                    "Bond removed"
                }
            } else {
                "Bond was unchanged"
            }
            .to_string();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::CalculateContacts {
            object_uid,
            selection,
            r_min,
            r_max,
            hbond,
            max_count,
        } => {
            if !r_min.is_finite() || !r_max.is_finite() || r_min < 0.0 || r_max < r_min {
                return Err("contact distance range is invalid".to_string());
            }
            let contacts = model
                .desktop
                .calc_atom_contacts_in_object(
                    Uid::new(object_uid),
                    &selection,
                    r_min,
                    r_max,
                    hbond,
                    max_count.clamp(1, 10_000),
                )
                .map_err(|error| error.to_string())?;
            model.status = format!("Contacts: {contacts}");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AssignSecondaryStructure {
            object_uid,
            ignore_bulge,
            angle,
        } => {
            if !angle.is_finite() || !(0.0..=180.0).contains(&angle) {
                return Err("helix angle must be between 0 and 180 degrees".to_string());
            }
            let undo = model.capture_undo();
            let count = model
                .desktop
                .assign_secondary_structure_in_object(Uid::new(object_uid), ignore_bulge, angle)
                .map_err(|error| error.to_string())?;
            model.status = format!("Secondary structure assigned for {count} atom(s)");
            model.rebuild_mesh(false)?;
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::WriteSurface {
            object_uid,
            selection,
            density,
            probe_radius,
            path,
        } => {
            if !density.is_finite() || density < 0.1 {
                return Err("surface density must be at least 0.1".to_string());
            }
            if !probe_radius.is_finite() || probe_radius < 0.1 {
                return Err("probe radius must be at least 0.1 Å".to_string());
            }
            if path.trim().is_empty() {
                return Err("an output OBJ path is required".to_string());
            }
            let summary = model
                .desktop
                .write_surface_obj(
                    Uid::new(object_uid),
                    &selection,
                    density,
                    probe_radius,
                    &path,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "Surface OBJ written: {} vertices / {} triangles",
                summary.vertex_count, summary.triangle_count
            );
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::WriteCutSurface {
            object_uid,
            selection,
            density,
            probe_radius,
            normal,
            position,
            path,
        } => {
            if !density.is_finite() || density < 0.1 {
                return Err("surface density must be at least 0.1".to_string());
            }
            if !probe_radius.is_finite() || probe_radius < 0.1 {
                return Err("probe radius must be at least 0.1 Å".to_string());
            }
            if normal
                .iter()
                .chain(position.iter())
                .any(|value| !value.is_finite())
            {
                return Err("surface cut plane must be finite".to_string());
            }
            if normal.iter().map(|value| value * value).sum::<f64>() < 1.0e-12 {
                return Err("surface cut normal must be non-zero".to_string());
            }
            if path.trim().is_empty() {
                return Err("an output OBJ path is required".to_string());
            }
            let summary = model
                .desktop
                .write_cut_surface_obj(
                    Uid::new(object_uid),
                    &selection,
                    density,
                    probe_radius,
                    Vector4::new3(normal[0], normal[1], normal[2]),
                    Vector4::new3(position[0], position[1], position[2]),
                    &path,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "Cut surface OBJ written: {} vertices / {} triangles",
                summary.vertex_count, summary.triangle_count
            );
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::WriteApbsFiles {
            object_uid,
            selection,
            pqr_path,
            input_path,
            dx_path,
        } => {
            if [pqr_path.as_str(), input_path.as_str(), dx_path.as_str()]
                .iter()
                .any(|path| path.trim().is_empty())
            {
                return Err("PQR, input, and DX paths are required".to_string());
            }
            let summary = model
                .desktop
                .write_apbs_files(
                    Uid::new(object_uid),
                    &selection,
                    &pqr_path,
                    &input_path,
                    &dx_path,
                )
                .map_err(|error| error.to_string())?;
            model.status = format!("APBS files written for {} atom(s)", summary.atom_count);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AppendMorphFrame {
            target_object_uid,
            source_object_uid,
            frame_name,
        } => {
            if frame_name.trim().is_empty() {
                return Err("a morph frame name is required".to_string());
            }
            let undo = model.capture_undo();
            let summary = model
                .desktop
                .append_morph_frame_from_object(
                    Uid::new(target_object_uid),
                    Uid::new(source_object_uid),
                    frame_name.trim(),
                )
                .map_err(|error| error.to_string())?;
            model.status = format!("Morph frame appended: {} frame(s)", summary.frame_count);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RemoveMorphFrame { object_uid, index } => {
            let undo = model.capture_undo();
            let summary = model
                .desktop
                .remove_morph_frame(Uid::new(object_uid), index)
                .map_err(|error| error.to_string())?;
            model.status = format!("Morph frame removed: {} frame(s)", summary.frame_count);
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ApplyMorphFrame { object_uid, frame } => {
            if !frame.is_finite() {
                return Err("morph frame must be finite".to_string());
            }
            let undo = model.capture_undo();
            let summary = model
                .desktop
                .apply_morph_frame(Uid::new(object_uid), frame)
                .map_err(|error| error.to_string())?;
            model.status = format!(
                "Morph frame {:.2} applied to {} atom(s)",
                frame, summary.atom_count
            );
            model.rebuild_mesh(false)?;
            model.push_undo(undo);
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
        ModelRequest::PickAtom {
            x,
            y,
            width,
            height,
        } => {
            model.pick_atom(x, y, width, height)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SelectScreenRect {
            x1,
            y1,
            x2,
            y2,
            width,
            height,
        } => {
            model.select_screen_rect(x1, y1, x2, y2, width, height)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AddAnimationObject(kind) => {
            let kind =
                animation_kind(&kind).ok_or_else(|| format!("unsupported animation: {kind}"))?;
            let undo = model.capture_undo();
            model.animation_objects.push(AnimObject {
                kind,
                duration: 3.0,
            });
            model.animation_elapsed = 0.0;
            model.animation_playing = false;
            model.animation_start_camera = None;
            model.status = format!("Added {} animation", kind.label());
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::DeleteAnimationObject(index) => {
            if index >= model.animation_objects.len() {
                return Err(format!("animation object {index} was not found"));
            }
            let undo = model.capture_undo();
            model.animation_objects.remove(index);
            model.animation_elapsed = 0.0;
            model.animation_playing = false;
            model.animation_start_camera = None;
            model.status = "Animation object deleted".to_string();
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::MoveAnimationObject { index, direction } => {
            if index >= model.animation_objects.len() {
                return Err(format!("animation object {index} was not found"));
            }
            let target = if direction < 0 {
                index.checked_sub(1)
            } else {
                index
                    .checked_add(1)
                    .filter(|target| *target < model.animation_objects.len())
            }
            .ok_or_else(|| "animation object cannot move farther".to_string())?;
            let undo = model.capture_undo();
            model.animation_objects.swap(index, target);
            model.animation_elapsed = 0.0;
            model.status = "Animation object moved".to_string();
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetAnimationDuration { index, duration } => {
            if !duration.is_finite() {
                return Err("animation duration must be finite".to_string());
            }
            if index >= model.animation_objects.len() {
                return Err(format!("animation object {index} was not found"));
            }
            let undo = model.capture_undo();
            model.animation_objects[index].duration = duration.clamp(0.1, 3600.0);
            model.animation_elapsed = model.animation_elapsed.min(model.animation_duration());
            model.status = "Animation duration changed".to_string();
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::AnimationControl(action) => {
            match action.as_str() {
                "play" => {
                    if model.animation_objects.is_empty() {
                        model.status = "No animation objects".to_string();
                    } else {
                        if model.animation_elapsed >= model.animation_duration() {
                            model.animation_elapsed = 0.0;
                        }
                        if model.animation_elapsed == 0.0 {
                            model.animation_start_camera = Some(model.camera.clone());
                        }
                        model.animation_playing = true;
                        model.status = "Animation playing".to_string();
                    }
                }
                "pause" => {
                    model.animation_playing = false;
                    model.status = "Animation paused".to_string();
                }
                "stop" => {
                    model.animation_playing = false;
                    model.animation_elapsed = 0.0;
                    if let Some(camera) = model.animation_start_camera.take() {
                        model.camera = camera;
                    }
                    model.status = "Animation stopped".to_string();
                }
                _ => return Err(format!("unsupported animation control: {action}")),
            }
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SetAnimationLoop(value) => {
            let undo = model.capture_undo();
            model.animation_loop = value;
            model.status = format!("Animation loop: {value}");
            model.push_undo(undo);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SeekAnimation(elapsed) => {
            if !elapsed.is_finite() {
                return Err("animation time must be finite".to_string());
            }
            model.animation_elapsed = elapsed.clamp(0.0, model.animation_duration());
            model.status = format!("Animation time: {:.2} s", model.animation_elapsed);
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::TickAnimation(dt) => {
            model.tick_animation(dt)?;
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ClearLog => {
            model.log_lines.clear();
            model.status = "Output log cleared".to_string();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ExecuteScript(script) => {
            if script.trim().is_empty() {
                return Err("a script is required".to_string());
            }
            let undo = model.capture_undo();
            let (log, selection) = model
                .desktop
                .run_script_js(&script)
                .map_err(|error| error.to_string())?;
            model
                .log_lines
                .extend(log.lines().map(|line| line.to_string()));
            if let Some(selection) = selection {
                model.selection = selection;
            }
            model.rebuild_mesh(false)?;
            model.push_undo(undo);
            model.status = "Script executed".to_string();
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::ExportPov { path } => {
            if path.trim().is_empty() {
                return Err("a POV-Ray output path is required".to_string());
            }
            model.persist_camera()?;
            let count = model
                .desktop
                .export_active_scene_pov(&path, &model.selection)
                .map_err(|error| error.to_string())?;
            model.status = format!("POV-Ray scene written: {count} atom(s)");
            model.log_lines.push(format!("POV-Ray scene: {path}"));
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::RenderAnimation {
            output_dir,
            base_name,
            width,
            height,
            frame_rate,
        } => {
            let count = model.render_animation_frames(
                &output_dir,
                &base_name,
                width,
                height,
                frame_rate,
            )?;
            model.status = format!("Animation rendered: {count} frame(s)");
            model
                .log_lines
                .push(format!("Animation frames: {output_dir}/{base_name}_*.png"));
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
        ModelRequest::SaveObject { object_uid, path } => {
            if path.trim().is_empty() {
                return Err("an object output path is required".to_string());
            }
            model
                .desktop
                .save_object_as(Uid::new(object_uid), &path)
                .map_err(|error| error.to_string())?;
            model.status = format!("Saved object: {path}");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::SaveView(path) => {
            if path.trim().is_empty() {
                return Err("a view output path is required".to_string());
            }
            fs::write(&path, camera_to_view_data(&model.camera))
                .map_err(|error| error.to_string())?;
            model.status = format!("Saved current view: {path}");
            Ok(ModelResponse::Snapshot(Box::new(model.snapshot())))
        }
        ModelRequest::LoadView(path) => {
            let data = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            model.camera = camera_from_view_data(&data)?;
            model.status = format!("Loaded view: {path}");
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
fn open_pdb(pdb_id: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::OpenPdb(pdb_id))?)
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
fn merge_molecules(
    source_object_uid: u64,
    selection: String,
    target_object_uid: u64,
    copy: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::MergeMolecules {
            source_object_uid,
            selection,
            target_object_uid,
            copy,
        },
    )?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn superpose_objects(
    algorithm: String,
    reference_object_uid: u64,
    reference_selection: String,
    moving_object_uid: u64,
    moving_selection: String,
    auto_recenter: bool,
    use_property: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SuperposeObjects {
            algorithm,
            reference_object_uid,
            reference_selection,
            moving_object_uid,
            moving_selection,
            auto_recenter,
            use_property,
        },
    )?)
}

#[tauri::command]
fn edit_bond(
    object_uid: u64,
    aid1: i32,
    aid2: i32,
    make: bool,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::EditBond {
            object_uid,
            aid1,
            aid2,
            make,
        },
    )?)
}

#[tauri::command]
fn calculate_contacts(
    object_uid: u64,
    selection: String,
    r_min: f64,
    r_max: f64,
    hbond: bool,
    max_count: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::CalculateContacts {
            object_uid,
            selection,
            r_min,
            r_max,
            hbond,
            max_count,
        },
    )?)
}

#[tauri::command]
fn assign_secondary_structure(
    object_uid: u64,
    ignore_bulge: bool,
    angle: f64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AssignSecondaryStructure {
            object_uid,
            ignore_bulge,
            angle,
        },
    )?)
}

#[tauri::command]
fn write_surface(
    object_uid: u64,
    selection: String,
    density: f64,
    probe_radius: f64,
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::WriteSurface {
            object_uid,
            selection,
            density,
            probe_radius,
            path,
        },
    )?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn write_cut_surface(
    object_uid: u64,
    selection: String,
    density: f64,
    probe_radius: f64,
    normal: [f64; 3],
    position: [f64; 3],
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::WriteCutSurface {
            object_uid,
            selection,
            density,
            probe_radius,
            normal,
            position,
            path,
        },
    )?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn write_apbs_files(
    object_uid: u64,
    selection: String,
    pqr_path: String,
    input_path: String,
    dx_path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::WriteApbsFiles {
            object_uid,
            selection,
            pqr_path,
            input_path,
            dx_path,
        },
    )?)
}

#[tauri::command]
fn append_morph_frame(
    target_object_uid: u64,
    source_object_uid: u64,
    frame_name: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AppendMorphFrame {
            target_object_uid,
            source_object_uid,
            frame_name,
        },
    )?)
}

#[tauri::command]
fn remove_morph_frame(
    object_uid: u64,
    index: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RemoveMorphFrame { object_uid, index },
    )?)
}

#[tauri::command]
fn apply_morph_frame(
    object_uid: u64,
    frame: f64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::ApplyMorphFrame { object_uid, frame },
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
fn pick_atom(
    x: f64,
    y: f64,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::PickAtom {
            x,
            y,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn select_screen_rect(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    width: u32,
    height: u32,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SelectScreenRect {
            x1,
            y1,
            x2,
            y2,
            width,
            height,
        },
    )?)
}

#[tauri::command]
fn add_animation_object(
    kind: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AddAnimationObject(kind),
    )?)
}

#[tauri::command]
fn delete_animation_object(
    index: usize,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::DeleteAnimationObject(index),
    )?)
}

#[tauri::command]
fn move_animation_object(
    index: usize,
    direction: i32,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::MoveAnimationObject { index, direction },
    )?)
}

#[tauri::command]
fn set_animation_duration(
    index: usize,
    duration: f64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetAnimationDuration { index, duration },
    )?)
}

#[tauri::command]
fn animation_control(
    action: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::AnimationControl(action),
    )?)
}

#[tauri::command]
fn set_animation_loop(value: bool, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SetAnimationLoop(value),
    )?)
}

#[tauri::command]
fn seek_animation(elapsed: f64, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::SeekAnimation(elapsed))?)
}

#[tauri::command]
fn tick_animation(dt: f64, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::TickAnimation(dt))?)
}

#[tauri::command]
fn clear_log(state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::ClearLog)?)
}

#[tauri::command]
fn execute_script(script: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::ExecuteScript(script))?)
}

#[tauri::command]
fn export_pov(path: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::ExportPov { path })?)
}

#[tauri::command]
fn render_animation(
    output_dir: String,
    base_name: String,
    width: u32,
    height: u32,
    frame_rate: f64,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::RenderAnimation {
            output_dir,
            base_name,
            width,
            height,
            frame_rate,
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
fn save_object(
    object_uid: u64,
    path: String,
    state: State<'_, ManagedState>,
) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(
        &state,
        ModelRequest::SaveObject { object_uid, path },
    )?)
}

#[tauri::command]
fn save_view(path: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::SaveView(path))?)
}

#[tauri::command]
fn load_view(path: String, state: State<'_, ManagedState>) -> Result<AppSnapshot, String> {
    snapshot_response(request_model(&state, ModelRequest::LoadView(path))?)
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

fn download_pdb_to_temp(pdb_id: &str) -> Result<PathBuf, String> {
    let pdb_id = pdb_id.trim().to_ascii_uppercase();
    if !(4..=16).contains(&pdb_id.len())
        || !pdb_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err("PDB ID must contain 4-16 letters, digits, or underscores".to_string());
    }
    let path = std::env::temp_dir().join(format!("cuemol-pdb-{pdb_id}.cif"));
    let url = format!("https://files.rcsb.org/download/{pdb_id}.cif");
    let output = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location"])
        .arg("--max-time")
        .arg("30")
        .arg("--output")
        .arg(&path)
        .arg(&url)
        .output()
        .map_err(|error| format!("failed to start curl: {error}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(&path);
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("RCSB download failed with {}", output.status)
        } else {
            format!("RCSB download failed: {detail}")
        });
    }
    Ok(path)
}

fn camera_to_view_data(camera: &Camera) -> String {
    let center = camera.center();
    let rotation = camera.rotation();
    let ortho_scale = 1000.0 / camera.zoom().max(0.1);
    format!(
        "cuemol2-rs-view\t2\n\
target\t{:.6}\t{:.6}\t{:.6}\n\
rotation\t{:.6}\t{:.6}\t{:.6}\t{:.6}\n\
distance\t{:.6}\n\
ortho_scale\t{:.6}\n\
perspective\t{}\n",
        center.x,
        center.y,
        center.z,
        rotation.x,
        rotation.y,
        rotation.z,
        rotation.w,
        camera.cam_dist(),
        ortho_scale,
        camera.is_perspective()
    )
}

fn camera_from_view_data(data: &str) -> Result<Camera, String> {
    let mut camera = Camera::new();
    let mut saw_header = false;
    for line in data.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.as_slice() {
            [] => {}
            ["cuemol2-rs-view", "1" | "2"] => saw_header = true,
            ["target", x, y, z] => {
                camera.set_center(Vector4::new3(
                    parse_view_number(x)?,
                    parse_view_number(y)?,
                    parse_view_number(z)?,
                ));
            }
            ["rotation", x, y, z, w] => {
                let rotation = Quat::new(
                    parse_view_number(x)?,
                    parse_view_number(y)?,
                    parse_view_number(z)?,
                    parse_view_number(w)?,
                )
                .normalize()
                .map_err(|error| error.to_string())?;
                camera.set_rotation(rotation);
            }
            ["distance", value] => camera.set_cam_dist(parse_view_number(value)?),
            ["ortho_scale", value] => {
                let scale = parse_view_number(value)?;
                if scale <= 0.0 {
                    return Err("orthographic scale must be positive".to_string());
                }
                camera.set_zoom(1000.0 / scale);
            }
            ["perspective", "true"] => camera.set_perspective(true),
            ["perspective", "false"] => camera.set_perspective(false),
            _ => return Err(format!("invalid view line: {line}")),
        }
    }
    if !saw_header {
        return Err("missing view header".to_string());
    }
    Ok(camera)
}

fn parse_view_number(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| format!("invalid view number: {value}"))
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
            open_pdb,
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
            merge_molecules,
            superpose_objects,
            edit_bond,
            calculate_contacts,
            assign_secondary_structure,
            write_surface,
            write_cut_surface,
            write_apbs_files,
            append_morph_frame,
            remove_morph_frame,
            apply_morph_frame,
            set_display_option,
            set_density_level,
            set_background,
            pick_atom,
            select_screen_rect,
            add_animation_object,
            delete_animation_object,
            move_animation_object,
            set_animation_duration,
            animation_control,
            set_animation_loop,
            seek_animation,
            tick_animation,
            clear_log,
            execute_script,
            export_pov,
            render_animation,
            render_viewport,
            update_camera,
            save_scene,
            save_object,
            save_view,
            load_view,
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
            animation_objects: Vec::new(),
            animation_elapsed: 0.0,
            animation_playing: false,
            animation_loop: false,
            animation_start_camera: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            log_lines: Vec::new(),
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
    fn current_view_text_roundtrips_the_qsys_camera() {
        let mut camera = Camera::new();
        camera.set_center(Vector4::new3(1.0, 2.0, 3.0));
        camera.set_cam_dist(80.0);
        camera.set_zoom(25.0);
        camera.set_perspective(true);
        camera.set_rotation(Quat::new(0.0, 0.0, 0.0, 1.0));
        let restored = camera_from_view_data(&camera_to_view_data(&camera)).unwrap();
        assert_eq!(restored.center(), camera.center());
        assert_eq!(restored.cam_dist(), camera.cam_dist());
        assert_eq!(restored.zoom(), camera.zoom());
        assert_eq!(restored.rotation(), camera.rotation());
        assert!(restored.is_perspective());
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

    #[test]
    fn viewport_click_selects_the_projected_atom() {
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
        model.selection = "aid 1".to_string();
        let mvp = camera_mvp(&model.camera, 800.0 / 600.0);
        let labels = model
            .desktop
            .active_atom_labels(&model.selection, mvp, 800.0, 600.0, 1);
        assert_eq!(labels.len(), 1);
        model.selection.clear();
        model.rebuild_mesh(false).unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::PickAtom {
                x: f64::from(labels[0].x),
                y: f64::from(labels[0].y),
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        assert_eq!(model.snapshot().selection, "aid 1");
    }

    #[test]
    fn animation_ribbon_controls_the_shared_timeline() {
        let mut model = model_without_gpu();
        let before = model.camera.rotation();
        handle_model_request(
            &mut model,
            ModelRequest::AddAnimationObject("simple-spin".to_string()),
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::AnimationControl("play".to_string()),
        )
        .unwrap();
        handle_model_request(&mut model, ModelRequest::TickAnimation(0.1)).unwrap();
        let snapshot = model.snapshot();
        assert_eq!(snapshot.animation_objects.len(), 1);
        assert_eq!(snapshot.animation_duration, 3.0);
        assert!(snapshot.animation_elapsed > 0.0);
        assert!(snapshot.animation_playing);
        assert_ne!(model.camera.rotation(), before);
        handle_model_request(
            &mut model,
            ModelRequest::AnimationControl("stop".to_string()),
        )
        .unwrap();
        assert_eq!(model.snapshot().animation_elapsed, 0.0);
        assert_eq!(model.camera.rotation(), before);
    }

    #[test]
    fn analysis_dialog_requests_call_the_shared_tools() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();
        let objects = model.desktop.active_object_summaries();
        let first = objects[0].uid.get();
        let second = objects[1].uid.get();

        handle_model_request(
            &mut model,
            ModelRequest::SuperposeObjects {
                algorithm: "lsq".to_string(),
                reference_object_uid: first,
                reference_selection: "name CA".to_string(),
                moving_object_uid: second,
                moving_selection: "name CA".to_string(),
                auto_recenter: true,
                use_property: true,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::EditBond {
                object_uid: first,
                aid1: 1,
                aid2: 327,
                make: true,
            },
        )
        .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::CalculateContacts {
                object_uid: first,
                selection: "name CA".to_string(),
                r_min: 1.0,
                r_max: 8.0,
                hbond: false,
                max_count: 4,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert!(snapshot.status.starts_with("Contacts: ["));
        let response = handle_model_request(
            &mut model,
            ModelRequest::AssignSecondaryStructure {
                object_uid: first,
                ignore_bulge: false,
                angle: 0.0,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(
            snapshot.status,
            "Secondary structure assigned for 327 atom(s)"
        );
        assert!(snapshot.can_undo);
    }

    #[test]
    fn surface_apbs_and_morph_dialog_requests_write_and_update_data() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let output_dir =
            std::env::temp_dir().join(format!("cuemol-tauri-tools-{}", std::process::id()));
        std::fs::create_dir_all(&output_dir).unwrap();
        let surface_path = output_dir.join("surface.obj");
        let cut_path = output_dir.join("cut.obj");
        let pqr_path = output_dir.join("apbs.pqr");
        let input_path = output_dir.join("apbs.in");
        let dx_path = output_dir.join("apbs.dx");

        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();
        let objects = model.desktop.active_object_summaries();
        let first = objects[0].uid.get();
        let second = objects[1].uid.get();

        handle_model_request(
            &mut model,
            ModelRequest::WriteSurface {
                object_uid: first,
                selection: "aid 1".to_string(),
                density: 0.5,
                probe_radius: 1.4,
                path: surface_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(surface_path.exists());
        handle_model_request(
            &mut model,
            ModelRequest::WriteCutSurface {
                object_uid: first,
                selection: "aid 1".to_string(),
                density: 0.5,
                probe_radius: 1.4,
                normal: [0.0, 0.0, 1.0],
                position: [0.0, 0.0, 0.0],
                path: cut_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(cut_path.exists());
        handle_model_request(
            &mut model,
            ModelRequest::WriteApbsFiles {
                object_uid: first,
                selection: "aid 1".to_string(),
                pqr_path: pqr_path.to_string_lossy().to_string(),
                input_path: input_path.to_string_lossy().to_string(),
                dx_path: dx_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(pqr_path.exists() && input_path.exists());

        for (source, name) in [(first, "start"), (second, "end")] {
            handle_model_request(
                &mut model,
                ModelRequest::AppendMorphFrame {
                    target_object_uid: first,
                    source_object_uid: source,
                    frame_name: name.to_string(),
                },
            )
            .unwrap();
        }
        handle_model_request(
            &mut model,
            ModelRequest::ApplyMorphFrame {
                object_uid: first,
                frame: 0.5,
            },
        )
        .unwrap();
        let response = handle_model_request(
            &mut model,
            ModelRequest::RemoveMorphFrame {
                object_uid: first,
                index: 1,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(snapshot.morphs.len(), 1);
        assert_eq!(snapshot.morphs[0].frames.len(), 1);

        for path in [surface_path, cut_path, pqr_path, input_path] {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir(output_dir);
    }

    #[test]
    fn rendering_and_script_dialog_requests_produce_outputs() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let pov_path =
            std::env::temp_dir().join(format!("cuemol-tauri-pov-{}.pov", std::process::id()));
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();

        let response = handle_model_request(
            &mut model,
            ModelRequest::ExecuteScript("print('tauri-script');".to_string()),
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(snapshot.log_lines, ["tauri-script"]);
        handle_model_request(
            &mut model,
            ModelRequest::ExportPov {
                path: pov_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(std::fs::read_to_string(&pov_path)
            .unwrap()
            .contains("CueMol POV-Ray scene"));
        let response = handle_model_request(&mut model, ModelRequest::ClearLog).unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert!(snapshot.log_lines.is_empty());
        let _ = std::fs::remove_file(pov_path);
    }

    #[test]
    fn file_dialog_requests_merge_save_and_restore_views() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let output_dir =
            std::env::temp_dir().join(format!("cuemol-tauri-files-{}", std::process::id()));
        std::fs::create_dir_all(&output_dir).unwrap();
        let object_path = output_dir.join("merged.pdb");
        let view_path = output_dir.join("current.view");
        let mut model = model_without_gpu();
        model.desktop.open_path(&pdb).unwrap();
        model.desktop.open_path(&pdb).unwrap();
        model.rebuild_mesh(true).unwrap();
        let objects = model.desktop.active_object_summaries();
        let source = objects[0].uid.get();
        let target = objects[1].uid.get();
        handle_model_request(
            &mut model,
            ModelRequest::ChangeChain {
                object_uid: source,
                selection: "name CA".to_string(),
                chain_name: "B".to_string(),
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::MergeMolecules {
                source_object_uid: source,
                selection: "chain B".to_string(),
                target_object_uid: target,
                copy: false,
            },
        )
        .unwrap();
        handle_model_request(
            &mut model,
            ModelRequest::SaveObject {
                object_uid: target,
                path: object_path.to_string_lossy().to_string(),
            },
        )
        .unwrap();
        assert!(std::fs::read_to_string(&object_path)
            .unwrap()
            .contains("ATOM"));

        model.camera.set_center(Vector4::new3(4.0, 5.0, 6.0));
        handle_model_request(
            &mut model,
            ModelRequest::SaveView(view_path.to_string_lossy().to_string()),
        )
        .unwrap();
        model.camera.set_center(Vector4::ZERO);
        handle_model_request(
            &mut model,
            ModelRequest::LoadView(view_path.to_string_lossy().to_string()),
        )
        .unwrap();
        assert_eq!(model.camera.center(), Vector4::new3(4.0, 5.0, 6.0));

        for path in [object_path, view_path] {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir(output_dir);
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn animation_rendering_writes_numbered_png_frames() {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = root.join("tests/test_data/1CRN.pdb");
        if !pdb.exists() {
            return;
        }
        let output_dir =
            std::env::temp_dir().join(format!("cuemol-tauri-animation-{}", std::process::id()));
        let mut model = AppModel::new(vec![pdb]);
        assert!(model.renderer.is_some());
        model.animation_objects.push(AnimObject {
            kind: AnimKind::Spin,
            duration: 0.1,
        });
        let response = handle_model_request(
            &mut model,
            ModelRequest::RenderAnimation {
                output_dir: output_dir.to_string_lossy().to_string(),
                base_name: "frame".to_string(),
                width: 64,
                height: 64,
                frame_rate: 2.0,
            },
        )
        .unwrap();
        let ModelResponse::Snapshot(snapshot) = response else {
            panic!("expected a snapshot");
        };
        assert_eq!(snapshot.status, "Animation rendered: 2 frame(s)");
        let first = std::fs::read(output_dir.join("frame_00000.png")).unwrap();
        assert_eq!(&first[..8], b"\x89PNG\r\n\x1a\n");
        let _ = std::fs::remove_file(output_dir.join("frame_00000.png"));
        let _ = std::fs::remove_file(output_dir.join("frame_00001.png"));
        let _ = std::fs::remove_dir(output_dir);
    }
}
