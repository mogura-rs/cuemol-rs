// AnimKind/AnimObject + the `anim_spin_direction` timeline stepper are shared via core
// so egui/gpui/dioxus drive the animation timeline identically.
use crate::{anim_step, AnimKind, AnimObject, DesktopAppState, GuiLoadedPath, GuiSequenceSummary};
use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_gfx::Rgba8;
use cuemol_render::{MeshVertex, RenderAtom, RenderBond, RenderSceneSnapshot};
use egui_wgpu::ScreenDescriptor;
use glam::{Mat4, Quat, Vec3, Vec4 as GlamVec4};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const RENDER_PRESETS: &[(&str, u32, u32)] = &[
    ("HD 720", 1280, 720),
    ("Full HD", 1920, 1080),
    ("Square 2K", 2048, 2048),
    ("Print 4K", 3840, 2160),
];
const RENDERER_TYPES: &[(&str, &str)] = &[
    ("simple", "Simple"),
    ("cpk", "CPK"),
    ("ballstick", "Ball & Stick"),
    ("trace", "Trace"),
    ("ribbon", "Ribbon"),
    ("cartoon", "Cartoon"),
    ("tube", "Tube"),
    ("spline", "Spline"),
    ("nucl", "Nucleic Acid"),
    ("anisou", "Anisotropic U"),
    ("atomintr", "Atom Interaction"),
    ("molsurf", "Molecular Surface"),
    ("directsurf", "Direct Surface"),
    ("contour", "Map Mesh"),
    ("isosurf", "Map Surface"),
    ("gpu_mapmesh", "GPU Map Mesh"),
    ("gpu_mapvol", "GPU Map Volume"),
    ("*symm", "Symmetry"),
    ("*unitcell", "Unit Cell"),
];
const LEGACY_MENU_SURFACES: &[&str] = &[
    "About CueMol3-tritium",
    "Preferences...",
    "New Window",
    "New Tab",
    "Open File...",
    "Get PDB...",
    "Clear Menu",
    "Save File As...",
    "Save current view...",
    "Close Tab",
    "Open Scene...",
    "Reload Scene",
    "Save Scene",
    "Save Scene As...",
    "Undo",
    "Redo",
    "Clear undo data",
    "Merge molecule...",
    "Delete mol atoms...",
    "Change chain ID...",
    "Change residue number...",
    "Options",
    "POV-Ray rendering...",
    "Animation rendering...",
    "Export scene...",
    "White",
    "Black",
    "Use color proofing",
    "Properties...",
    "Perspective",
    "Orthographic",
    "Cross",
    "Axis",
    "None",
    "View property...",
    "Molecular superposition...",
    "Mol bond editor...",
    "Interaction...",
    "Reassign secondary str...",
    "Mol morphing animation...",
    "Mol surface generation...",
    "Mol surface cutter...",
    "APBS elepot calculation...",
    "Execute script...",
    "Performance measure",
    "Show/Hide Topbar",
    "Clear log contents",
    "Restore default panel location",
    "About plugins...",
    "About config...",
    "Addon manager...",
    "Console",
    "Check for updates",
];

pub fn run() -> Result<()> {
    install_panic_hook();
    spawn_process_exit_timer_from_env();
    let event_loop = EventLoop::new().map_err(gui_error)?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut runner = NativeRunner::new();
    let run_result = event_loop.run_app(&mut runner).map_err(gui_error);
    let init_error = runner.init_error.take();
    std::mem::forget(runner);
    run_result?;
    init_error.map_or(Ok(()), Err)
}

struct NativeRunner {
    window: Option<Arc<Window>>,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    gpu: Option<GpuState>,
    app: GuiApplication,
    init_error: Option<CueMolError>,
    started_at: Instant,
    exit_after: Option<Duration>,
}

impl NativeRunner {
    fn new() -> Self {
        Self {
            window: None,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            gpu: None,
            app: GuiApplication::new(),
            init_error: None,
            started_at: Instant::now(),
            exit_after: exit_after_from_env(),
        }
    }

    fn should_exit_now(&self) -> bool {
        self.exit_after
            .is_some_and(|duration| self.started_at.elapsed() >= duration)
    }
}

impl ApplicationHandler for NativeRunner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.guard_gui_callback(event_loop, "resumed", Self::resumed_impl);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let _ = self.guard_gui_callback(event_loop, "window_event", |this, event_loop| {
            this.window_event_impl(event_loop, window_id, event);
        });
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let _ = self.guard_gui_callback(event_loop, "about_to_wait", |this, event_loop| {
            this.about_to_wait_impl(event_loop);
        });
    }
}

impl NativeRunner {
    fn guard_gui_callback(
        &mut self,
        event_loop: &ActiveEventLoop,
        context: &'static str,
        callback: impl FnOnce(&mut Self, &ActiveEventLoop),
    ) -> bool {
        match catch_unwind(AssertUnwindSafe(|| callback(self, event_loop))) {
            Ok(()) => false,
            Err(payload) => {
                let message = panic_payload_message(payload.as_ref());
                eprintln!("cuemol-desktop GUI panic in {context}: {message}");
                self.init_error = Some(CueMolError::invalid_argument(format!(
                    "GUI panic in {context}: {message}"
                )));
                event_loop.exit();
                true
            }
        }
    }

    fn resumed_impl(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("CueMol2-rs")
            .with_inner_size(PhysicalSize::new(1280, 820));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.init_error = Some(gui_error(error));
                event_loop.exit();
                return;
            }
        };
        let egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let gpu = match pollster::block_on(GpuState::new(window.clone())) {
            Ok(gpu) => gpu,
            Err(error) => {
                self.init_error = Some(error);
                event_loop.exit();
                return;
            }
        };
        self.egui_state = Some(egui_state);
        self.gpu = Some(gpu);
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event_impl(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        if window_id != window.id() {
            return;
        }
        if matches!(event, WindowEvent::RedrawRequested) && self.should_exit_now() {
            event_loop.exit();
            return;
        }
        let Some(egui_state) = self.egui_state.as_mut() else {
            return;
        };
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
        let response = egui_state.on_window_event(window.as_ref(), &event);
        let mut needs_redraw = response.repaint;
        if self.app.handle_window_event(&event, &mut gpu.camera) {
            needs_redraw = true;
        }
        match event {
            WindowEvent::CloseRequested => std::process::exit(0),
            WindowEvent::Resized(size) => {
                gpu.resize(size);
                needs_redraw = true;
            }
            WindowEvent::RedrawRequested => {
                let raw_input = egui_state.take_egui_input(window.as_ref());
                let full_output = self.egui_ctx.run(raw_input, |ctx| {
                    self.app.ui(ctx, &mut gpu.camera);
                });
                egui_state.handle_platform_output(window.as_ref(), full_output.platform_output);
                let paint_jobs = self
                    .egui_ctx
                    .tessellate(full_output.shapes, full_output.pixels_per_point);
                match gpu.render(
                    &self.app.snapshot,
                    self.app.background.clear_color(),
                    &paint_jobs,
                    &full_output.textures_delta,
                    full_output.pixels_per_point,
                    self.app.viewport_rect,
                ) {
                    Ok(()) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        gpu.resize(gpu.size)
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(wgpu::SurfaceError::Timeout) => {}
                    Err(_) => {}
                }
                if self.egui_ctx.has_requested_repaint() {
                    needs_redraw = true;
                }
            }
            _ => {}
        }
        if needs_redraw {
            window.request_redraw();
        }
    }

    fn about_to_wait_impl(&mut self, event_loop: &ActiveEventLoop) {
        if self.should_exit_now() {
            event_loop.exit();
        }
    }
}

fn exit_after_from_env() -> Option<Duration> {
    let millis = std::env::var("CUEMOL_DESKTOP_EXIT_AFTER_MS")
        .ok()?
        .parse::<u64>()
        .ok()?;
    Some(Duration::from_millis(millis))
}

fn spawn_process_exit_timer_from_env() {
    let Some(duration) = exit_after_from_env() else {
        return;
    };
    std::thread::spawn(move || {
        std::thread::sleep(duration);
        std::process::exit(0);
    });
}

fn install_panic_hook() {
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            eprintln!("cuemol-desktop panic: {info}");
            eprintln!("{}", std::backtrace::Backtrace::force_capture());
            default_hook(info);
        }));
    });
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

#[derive(Debug)]
struct GuiApplication {
    state: DesktopAppState,
    open_path: String,
    save_path: String,
    object_save_path: String,
    /// AMBER mdcrd trajectory path for the MolAnim animation type.
    trajectory_path: String,
    view_save_path: String,
    export_path: String,
    log_save_path: String,
    pdb_id: String,
    script_path: String,
    console_input: String,
    selection_text: String,
    selected_object: Option<cuemol_core::Uid>,
    selected_renderer: Option<cuemol_core::Uid>,
    /// Molecule chosen in the MolStruct panel's molecule selector (CueMol2
    /// `molstruct-panel.xul`); independent of the Workspace tree selection.
    molstruct_object: Option<cuemol_core::Uid>,
    /// MolStruct tree expand-all / collapse-all state. Folded into each node's
    /// persistent id so toggling recreates every chain/residue node with this as the
    /// default-open state (egui only honours `default_open` on first creation).
    molstruct_expand_all: bool,
    /// Active tab of the Selection side panel (Command vs Editor).
    selection_tab: SelectionTab,
    /// Whether the crystallographic unit-cell box is drawn (Symmetry panel "Unit cell").
    show_unit_cell: bool,
    /// Whether the home-cell symmetry-mate copies are drawn (Symmetry panel "Symm mol").
    show_symm_mates: bool,
    /// Density-map "chickenwire" contour display + its sigma contour level
    /// (CueMol2 Density map panel level slider).
    show_density_contour: bool,
    density_level: f64,
    /// Filled density isosurface (the solid-surface density mode alongside the
    /// chickenwire contour) + a cached mesh (marching cubes is expensive).
    show_density_solid: bool,
    density_solid_mesh: Option<cuemol_render::RenderMesh>,
    /// Molecular-surface (SES) display: the toggle + a cached mesh (the SES is
    /// expensive, so it is computed once on enable, not every rebuild).
    show_surface: bool,
    surface_mesh: Option<cuemol_render::RenderMesh>,
    /// Live in-viewport surface clipping: display the surface as the plane cross-section.
    clip_surface: bool,
    /// Clip-plane offset along +Z from the bbox centre (Å).
    clip_offset: f64,
    /// Colour the surface by the loaded electrostatic potential (SCAPOT) instead of by
    /// nearest atom, and the APBS/OpenDX `.dx` path to load the potential from.
    potential_surface: bool,
    potential_path: String,
    /// Path of an external surface mesh (Wavefront OBJ / Stanford PLY) to import
    /// into the surface-display slot (CueMol2 `ObjReader`/`PLYFileReader`).
    mesh_import_path: String,
    /// Atom/residue labels overlaid on the selected atoms in the 3D view.
    show_labels: bool,
    /// Hydrogen-bond dashed lines (heavy-atom N···O heuristic).
    show_hbonds: bool,
    /// Light vs dark UI theme (CueMol2's native XUL look is light; dark is the default).
    light_theme: bool,
    /// Force every left-dock accordion section open (an "expand all panels" aid, also
    /// used to snapshot the bodies inside the REAL narrow dock for layout review).
    force_open_all_panels: bool,
    /// Rainbow coloring hue endpoints (degrees) edited in the Coloring panel; the
    /// classic N→C blue→red gradient by default (start 240° = blue, end 0° = red).
    coloring_start_hue: f32,
    coloring_end_hue: f32,
    snapshot: RenderSceneSnapshot,
    background: SceneBackground,
    center_mark: CenterMark,
    color_proofing: bool,
    bottom_tab: BottomTab,
    render_preset: String,
    render_backend: String,
    ribbon_tab: RibbonTab,
    interaction_mode: InteractionMode,
    measure_mode: MeasureMode,
    show_toolbar: bool,
    show_workspace: bool,
    show_inspector: bool,
    show_bottom_panel: bool,
    show_about: bool,
    show_options: bool,
    show_scene_props: bool,
    show_view_props: bool,
    show_chain_dialog: bool,
    show_residue_dialog: bool,
    show_renderer_dialog: bool,
    new_renderer_target: Option<cuemol_core::Uid>,
    new_renderer_type: String,
    new_renderer_name: String,
    chain_edit_value: String,
    residue_start_value: String,
    tool_dialog: Option<String>,
    show_get_pdb: bool,
    show_script_dialog: bool,
    show_console: bool,
    viewport_rect: Option<egui::Rect>,
    pixels_per_point: f32,
    shift_down: bool,
    viewport_menu: Option<ViewportMenu>,
    perf_measure: bool,
    animation_frames: usize,
    animation_playing: bool,
    /// Animation timeline objects (CueMol2 `anim-panel` list) + the elapsed playback
    /// time in seconds while playing.
    animation_objects: Vec<AnimObject>,
    animation_elapsed: f32,
    superpose_reference: Option<cuemol_core::Uid>,
    superpose_moving: Option<cuemol_core::Uid>,
    superpose_reference_selection: String,
    superpose_moving_selection: String,
    bond_aid1: String,
    bond_aid2: String,
    contact_r_min: f64,
    contact_r_max: f64,
    contact_hbond: bool,
    secondary_ignore_breaks: bool,
    secondary_angle: f64,
    surface_path: String,
    surface_density: f64,
    surface_probe_radius: f64,
    cut_surface_path: String,
    cut_normal: [f64; 3],
    cut_position: [f64; 3],
    apbs_pqr_path: String,
    apbs_input_path: String,
    apbs_dx_path: String,
    morph_target: Option<cuemol_core::Uid>,
    morph_source: Option<cuemol_core::Uid>,
    morph_frame_name: String,
    morph_frame_value: f64,
    config_path: String,
    config_key: String,
    config_value: String,
    plugin_dir: String,
    addon_dir: String,
    last_loaded_path: Option<PathBuf>,
    recent_paths: Vec<PathBuf>,
    undo_stack: Vec<GuiUndoSnapshot>,
    redo_stack: Vec<GuiUndoSnapshot>,
    status: String,
    log_lines: Vec<String>,
    selection_history: Vec<String>,
}

#[derive(Debug, Clone)]
struct GuiUndoSnapshot {
    state: DesktopAppState,
    selected_object: Option<cuemol_core::Uid>,
    selected_renderer: Option<cuemol_core::Uid>,
    selection_text: String,
    last_loaded_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneBackground {
    White,
    Black,
}

impl SceneBackground {
    fn clear_color(self) -> wgpu::Color {
        match self {
            Self::White => wgpu::Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            Self::Black => wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CenterMark {
    Cross,
    Axis,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Output,
    Sequence,
    Animation,
    Render,
}

/// Selection-panel tab (CueMol2 `selection-panel.xul`): the free-text Command tab
/// vs the click-to-build Editor tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionTab {
    Command,
    Editor,
}

/// Object-level coloring scheme chosen in the Coloring panel (CueMol2
/// `coloring-panel.xul` type dropdown). The subset wired into the snapshot's
/// object-coloring cascade (Element/CPK default, Rainbow, B-factor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColoringMode {
    Element,
    Rainbow,
    Bfac,
}

/// Ribbon toolbar tabs mirroring CueMol2's `topbar/cuemol2-ribbon.xul`
/// (`tool-ribbon.js`): Home (navigate), Measure, Animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RibbonTab {
    Home,
    Measure,
    Animation,
}

/// Home-tab interaction mode radio group (CueMol2 `navi-toolribbon.js`):
/// Rotate (default) vs rectangle select.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractionMode {
    Rotate,
    RectSelect,
}

/// Measure-tab radio group (CueMol2 `measure-toolribbon.js`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeasureMode {
    Distance,
    Angle,
    Torsion,
}

/// Pending viewport right-click context menu (CueMol2 `navi-ctxtmenu-atom`).
#[derive(Debug, Clone, Copy)]
struct ViewportMenu {
    pos: egui::Pos2,
    hit: Option<PickHit>,
}

/// Left-dock accordion sections mirroring CueMol2's `cuemol2-panels.js`
/// registration order in the `sidepanelholder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeftPanel {
    Workspace,
    MolStruct,
    Selection,
    Coloring,
    Symmetry,
    DensityMap,
    View,
    Animation,
}

impl LeftPanel {
    const ALL: [LeftPanel; 8] = [
        LeftPanel::Workspace,
        LeftPanel::MolStruct,
        LeftPanel::Selection,
        LeftPanel::Coloring,
        LeftPanel::Symmetry,
        LeftPanel::DensityMap,
        LeftPanel::View,
        LeftPanel::Animation,
    ];

    fn title(self) -> &'static str {
        match self {
            LeftPanel::Workspace => "Workspace",
            LeftPanel::MolStruct => "MolStruct",
            LeftPanel::Selection => "Selection",
            LeftPanel::Coloring => "Coloring",
            LeftPanel::Symmetry => "Symmetry",
            LeftPanel::DensityMap => "Density map",
            LeftPanel::View => "View",
            LeftPanel::Animation => "Animation",
        }
    }
}

impl GuiApplication {
    fn new() -> Self {
        let mut state = DesktopAppState::new();
        state.new_scene("Untitled", true);
        let mut app = Self {
            state,
            open_path: "../cuemol2/tests/test_data/1CRN.pdb".to_string(),
            save_path: "scene.qsc".to_string(),
            object_save_path: "object.pdb".to_string(),
            trajectory_path: String::new(),
            view_save_path: "current_view.cam".to_string(),
            export_path: "scene.pov".to_string(),
            log_save_path: "cuemol.log".to_string(),
            pdb_id: "1CRN".to_string(),
            script_path: "script.js".to_string(),
            console_input: String::new(),
            selection_text: String::new(),
            selected_object: None,
            molstruct_object: None,
            molstruct_expand_all: false,
            selection_tab: SelectionTab::Command,
            show_unit_cell: false,
            show_symm_mates: false,
            show_density_contour: false,
            density_level: 1.5,
            show_density_solid: false,
            density_solid_mesh: None,
            show_surface: false,
            surface_mesh: None,
            clip_surface: false,
            clip_offset: 0.0,
            potential_surface: false,
            potential_path: String::new(),
            mesh_import_path: String::new(),
            show_labels: false,
            show_hbonds: false,
            light_theme: false,
            force_open_all_panels: false,
            coloring_start_hue: 240.0,
            coloring_end_hue: 0.0,
            selected_renderer: None,
            snapshot: RenderSceneSnapshot::empty(),
            background: SceneBackground::Black,
            center_mark: CenterMark::Cross,
            color_proofing: false,
            bottom_tab: BottomTab::Output,
            render_preset: RENDER_PRESETS[1].0.to_string(),
            render_backend: "POV-Ray".to_string(),
            ribbon_tab: RibbonTab::Home,
            interaction_mode: InteractionMode::Rotate,
            measure_mode: MeasureMode::Distance,
            show_toolbar: true,
            show_workspace: true,
            // CueMol2 opens properties on demand (no docked inspector); start closed.
            show_inspector: false,
            show_bottom_panel: true,
            show_about: false,
            show_options: false,
            show_scene_props: false,
            show_view_props: false,
            show_chain_dialog: false,
            show_residue_dialog: false,
            show_renderer_dialog: false,
            new_renderer_target: None,
            new_renderer_type: "simple".to_string(),
            new_renderer_name: "simple".to_string(),
            chain_edit_value: "A".to_string(),
            residue_start_value: "1".to_string(),
            tool_dialog: None,
            show_get_pdb: false,
            show_script_dialog: false,
            show_console: false,
            viewport_rect: None,
            pixels_per_point: 1.0,
            shift_down: false,
            viewport_menu: None,
            perf_measure: false,
            animation_frames: 0,
            animation_playing: false,
            animation_objects: Vec::new(),
            animation_elapsed: 0.0,
            superpose_reference: None,
            superpose_moving: None,
            superpose_reference_selection: "name CA".to_string(),
            superpose_moving_selection: "name CA".to_string(),
            bond_aid1: String::new(),
            bond_aid2: String::new(),
            contact_r_min: 1.0,
            contact_r_max: 3.5,
            contact_hbond: false,
            secondary_ignore_breaks: false,
            secondary_angle: 60.0,
            surface_path: "surface.obj".to_string(),
            surface_density: 1.0,
            surface_probe_radius: 1.4,
            cut_surface_path: "cut_surface.obj".to_string(),
            cut_normal: [0.0, 0.0, 1.0],
            cut_position: [0.0, 0.0, 0.0],
            apbs_pqr_path: "apbs.pqr".to_string(),
            apbs_input_path: "apbs.in".to_string(),
            apbs_dx_path: "apbs.dx".to_string(),
            morph_target: None,
            morph_source: None,
            morph_frame_name: "frame".to_string(),
            morph_frame_value: 0.0,
            config_path: "cuemol-desktop.conf".to_string(),
            config_key: "gui:background".to_string(),
            config_value: "black".to_string(),
            plugin_dir: default_user_dir("plugins"),
            addon_dir: default_user_dir("addons"),
            last_loaded_path: None,
            recent_paths: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            status: "Ready".to_string(),
            log_lines: Vec::new(),
            selection_history: Vec::new(),
        };
        app.rebuild_snapshot();
        app
    }

    fn ui(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        self.pixels_per_point = ctx.pixels_per_point();
        // UI theme (View menu): CueMol2's native XUL look is light; dark is the default.
        ctx.set_visuals(if self.light_theme {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        });
        // Animation playback: drive the camera from the timeline objects (or a simple
        // spin when the list is empty). Keep requesting repaints so it animates.
        if self.animation_playing {
            let dt = ctx.input(|input| input.stable_dt).clamp(0.0, 0.1);
            self.advance_animation(dt, camera);
            ctx.request_repaint();
        }
        // CueMol2 window order (uxp_gui `cuemol2.xul`):
        // menu bar (top) -> ribbon (top) -> [ left dock | tabbed view | bottom dock ]
        // -> status bar (very bottom). There is no right dock; properties open modally.
        self.menu_bar(ctx, camera);
        if self.show_toolbar {
            self.ribbon_bar(ctx, camera);
        }
        // Register the status bar before the bottom dock so it sits at the very bottom.
        self.status_bar(ctx);
        if self.show_bottom_panel {
            self.bottom_panel(ctx);
        }
        if self.show_workspace {
            self.left_panel(ctx, camera);
        }
        // Scene-tab strip registered after the side/bottom docks, so it is confined to
        // the central content column (above the 3D view), like CueMol2's `tabmolview`.
        self.scene_tab_strip(ctx, camera);
        self.central_view(ctx, camera);
        if self.show_inspector {
            self.properties_window(ctx, camera);
        }
        self.dialogs(ctx, camera);
    }

    fn central_view(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                self.viewport_rect = Some(rect);
                self.draw_center_mark(ui, rect);
                self.draw_atom_labels(ui, camera, rect);
                ui.allocate_ui(rect.size(), |ui| {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} atoms / {} bonds",
                                self.snapshot.atom_count(),
                                self.snapshot.bond_count()
                            ))
                            .color(egui::Color32::from_rgba_unmultiplied(220, 226, 235, 180)),
                        );
                    });
                });
            });
        self.viewport_context_menu(ctx, camera);
    }

    /// Right-click atom context menu over the 3D view (CueMol2 `navi-ctxtmenu-atom`):
    /// Center at / Select atom·residue·chain / Around {3,5,7,10 Å} / Clear.
    fn viewport_context_menu(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        let Some(menu) = self.viewport_menu else {
            return;
        };
        // Resolve the picked atom up front so the menu closure stays self-borrow-free.
        let atom_info = menu.hit.and_then(|hit| {
            let object_uid = cuemol_core::Uid::new(hit.object_uid);
            let atom = self.state.atom_summary(object_uid, hit.atom_index).ok()?;
            let center = self
                .snapshot
                .atoms
                .iter()
                .find(|a| {
                    a.object_uid == Some(hit.object_uid) && a.atom_index == Some(hit.atom_index)
                })
                .map(|a| vec3_from_vector(a.center));
            Some((atom, center))
        });

        enum Choice {
            Center(Vec3),
            Select(String),
            Around(String, f64),
            Clear,
        }
        let mut chosen: Option<Choice> = None;
        let mut close = false;

        egui::Area::new(egui::Id::new("viewport_ctx_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(menu.pos)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_max_width(240.0);
                    if let Some((atom, center)) = &atom_info {
                        let chain = if atom.chain_id.is_empty() {
                            "_"
                        } else {
                            atom.chain_id.as_str()
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}{}:{}  (chain {chain})",
                                atom.serial, atom.residue_name, atom.residue_id, atom.name
                            ))
                            .strong(),
                        );
                        ui.separator();
                        let atom_sel = format!("aid {}", atom.serial);
                        let resid_sel = if atom.chain_id.is_empty() {
                            format!("resid {}", atom.residue_id)
                        } else {
                            format!("chain {} and resid {}", atom.chain_id, atom.residue_id)
                        };
                        let chain_sel = format!("chain {}", atom.chain_id);
                        if let Some(center) = center {
                            if ui.button("Center at this atom").clicked() {
                                chosen = Some(Choice::Center(*center));
                                close = true;
                            }
                        }
                        if ui.button("Select this atom").clicked() {
                            chosen = Some(Choice::Select(atom_sel.clone()));
                            close = true;
                        }
                        if ui.button("Select residue").clicked() {
                            chosen = Some(Choice::Select(resid_sel));
                            close = true;
                        }
                        if ui.button("Select chain").clicked() {
                            chosen = Some(Choice::Select(chain_sel));
                            close = true;
                        }
                        ui.menu_button("Around", |ui| {
                            for radius in [3.0_f64, 5.0, 7.0, 10.0] {
                                if ui.button(format!("{radius:.0} Å")).clicked() {
                                    chosen = Some(Choice::Around(atom_sel.clone(), radius));
                                    close = true;
                                    ui.close();
                                }
                            }
                        });
                        ui.separator();
                    } else {
                        ui.label("(no atom under cursor)");
                        ui.separator();
                    }
                    if ui.button("Clear selection").clicked() {
                        chosen = Some(Choice::Clear);
                        close = true;
                    }
                });
            });

        if let Some(choice) = chosen {
            match choice {
                Choice::Center(center) => {
                    camera.target = center;
                    self.set_status("Centered at atom");
                }
                Choice::Select(sel) => {
                    self.selection_text = sel;
                    self.push_selection_history();
                    self.rebuild_snapshot();
                }
                Choice::Around(base, radius) => {
                    self.selection_text = format!("({base}) around {radius}");
                    self.push_selection_history();
                    self.rebuild_snapshot();
                }
                Choice::Clear => {
                    self.selection_text.clear();
                    self.rebuild_snapshot();
                }
            }
        }
        if close || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.viewport_menu = None;
        }
    }

    /// Horizontal scene-tab strip above the 3D view (CueMol2 `tabmolview` multi-tab).
    fn scene_tab_strip(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::TopBottomPanel::top("scene_tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let scenes = self.state.scene_summaries();
                let active = self.state.active_scene();
                for scene in &scenes {
                    let selected = Some(scene.uid) == active;
                    if ui.selectable_label(selected, &scene.name).clicked() && !selected {
                        self.state.set_active_scene(Some(scene.uid));
                        self.selected_object = None;
                        self.selected_renderer = None;
                        self.rebuild_snapshot();
                        camera.fit_snapshot(&self.snapshot);
                    }
                }
                if ui.button("+").on_hover_text("New scene tab").clicked() {
                    self.new_scene(camera);
                }
            });
        });
    }

    /// One-line status bar pinned to the very bottom (CueMol2 `<statusbar>`).
    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "{} atoms / {} bonds",
                        self.snapshot.atom_count(),
                        self.snapshot.bond_count()
                    ));
                });
            });
        });
    }

    fn menu_bar(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Window").clicked() {
                        self.new_scene(camera);
                        self.set_status("New window mapped to a new scene in this native runner");
                        ui.close();
                    }
                    if ui.button("New Tab").clicked() {
                        self.new_scene(camera);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Open File...").clicked() {
                        self.open_current_path(camera);
                        ui.close();
                    }
                    if ui.button("Get PDB...").clicked() {
                        self.show_get_pdb = true;
                        ui.close();
                    }
                    ui.menu_button("Open Recent", |ui| {
                        let recent_paths = self.recent_paths.clone();
                        for path in recent_paths {
                            let label = path.to_string_lossy();
                            if ui.button(label.as_ref()).clicked() {
                                self.open_path = label.to_string();
                                self.open_current_path(camera);
                                ui.close();
                            }
                        }
                        if !self.recent_paths.is_empty() {
                            ui.separator();
                        }
                        if ui.button("Clear Menu").clicked() {
                            self.clear_recent_paths();
                            ui.close();
                        }
                    });
                    ui.separator();
                    if ui.button("Save File As...").clicked() {
                        self.save_selected_object();
                        ui.close();
                    }
                    if ui.button("Save current view...").clicked() {
                        self.save_current_view(camera);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Close Tab").clicked() {
                        self.close_current_scene(camera);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Open Scene...").clicked() {
                        self.open_current_path(camera);
                        ui.close();
                    }
                    if ui.button("Reload Scene").clicked() {
                        self.reload_current_path(camera);
                        ui.close();
                    }
                    if ui.button("Save Scene").clicked() {
                        self.save_current_scene(camera);
                        ui.close();
                    }
                    if ui.button("Save Scene As...").clicked() {
                        self.save_current_scene(camera);
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        self.undo(camera);
                        ui.close();
                    }
                    if ui.button("Redo").clicked() {
                        self.redo(camera);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Clear undo data").clicked() {
                        self.clear_undo_data();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Merge molecule...").clicked() {
                        self.merge_current_path(camera);
                        ui.close();
                    }
                    if ui.button("Delete mol atoms...").clicked() {
                        self.delete_selected_atoms();
                        ui.close();
                    }
                    if ui.button("Change chain ID...").clicked() {
                        self.show_chain_dialog = true;
                        ui.close();
                    }
                    if ui.button("Change residue number...").clicked() {
                        self.show_residue_dialog = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Options").clicked() {
                        self.show_options = true;
                        ui.close();
                    }
                });
                ui.menu_button("Rendering", |ui| {
                    if ui.button("POV-Ray rendering...").clicked() {
                        self.bottom_tab = BottomTab::Render;
                        self.render_backend = "POV-Ray".to_string();
                        ui.close();
                    }
                    if ui.button("Animation rendering...").clicked() {
                        self.bottom_tab = BottomTab::Animation;
                        ui.close();
                    }
                    if ui.button("Export scene...").clicked() {
                        self.export_current_scene();
                        ui.close();
                    }
                });
                ui.menu_button("Scene", |ui| {
                    ui.menu_button("Background", |ui| {
                        if ui
                            .radio_value(&mut self.background, SceneBackground::White, "White")
                            .clicked()
                        {
                            self.set_status("Background set to white");
                            ui.close();
                        }
                        if ui
                            .radio_value(&mut self.background, SceneBackground::Black, "Black")
                            .clicked()
                        {
                            self.set_status("Background set to black");
                            ui.close();
                        }
                    });
                    ui.separator();
                    if ui
                        .checkbox(&mut self.color_proofing, "Use color proofing")
                        .clicked()
                    {
                        self.rebuild_snapshot();
                        self.set_status(format!("Color proofing: {}", self.color_proofing));
                    }
                    if ui.button("Properties...").clicked() {
                        self.show_scene_props = true;
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.menu_button("Theme", |ui| {
                        if ui
                            .radio_value(&mut self.light_theme, false, "Dark")
                            .clicked()
                        {
                            self.set_status("Dark theme");
                        }
                        if ui
                            .radio_value(&mut self.light_theme, true, "Light")
                            .clicked()
                        {
                            self.set_status("Light theme");
                        }
                    });
                    ui.separator();
                    if ui
                        .radio_value(&mut camera.perspective, true, "Perspective")
                        .clicked()
                    {
                        self.set_status("Perspective projection");
                    }
                    if ui
                        .radio_value(&mut camera.perspective, false, "Orthographic")
                        .clicked()
                    {
                        self.set_status("Orthographic projection");
                    }
                    ui.separator();
                    ui.menu_button("Center mark", |ui| {
                        ui.radio_value(&mut self.center_mark, CenterMark::Cross, "Cross");
                        ui.radio_value(&mut self.center_mark, CenterMark::Axis, "Axis");
                        ui.radio_value(&mut self.center_mark, CenterMark::None, "None");
                    });
                    ui.separator();
                    if ui.button("View property...").clicked() {
                        self.show_view_props = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Fit").clicked() {
                        camera.fit_snapshot(&self.snapshot);
                        ui.close();
                    }
                });
                ui.menu_button("Tools", |ui| {
                    for label in [
                        "Molecular superposition...",
                        "Mol bond editor...",
                        "Interaction...",
                        "Reassign secondary str...",
                        "Mol morphing animation...",
                        "Mol surface generation...",
                        "Mol surface cutter...",
                        "APBS elepot calculation...",
                        "Execute script...",
                        "Performance measure",
                    ] {
                        if ui.button(label).clicked() {
                            match label {
                                "Execute script..." => self.show_script_dialog = true,
                                "Performance measure" => {
                                    self.perf_measure = !self.perf_measure;
                                    self.set_status(format!(
                                        "Performance measure: {}",
                                        self.perf_measure
                                    ));
                                }
                                _ => {
                                    self.tool_dialog =
                                        Some(label.trim_end_matches("...").to_string());
                                }
                            }
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Window", |ui| {
                    if ui.button("Show/Hide Topbar").clicked() {
                        self.show_toolbar = !self.show_toolbar;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Clear log contents").clicked() {
                        self.clear_log();
                        ui.close();
                    }
                    if ui.button("Restore default panel location").clicked() {
                        self.restore_panels();
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About CueMol3-tritium").clicked() {
                        self.show_about = true;
                        ui.close();
                    }
                    ui.separator();
                    for label in [
                        "About plugins...",
                        "About config...",
                        "Addon manager...",
                        "Console",
                        "Check for updates",
                    ] {
                        if ui.button(label).clicked() {
                            match label {
                                "Console" => {
                                    self.bottom_tab = BottomTab::Output;
                                    self.show_console = true;
                                }
                                "Check for updates" => {
                                    self.set_status("Update check is not available offline");
                                }
                                _ => {
                                    self.tool_dialog =
                                        Some(label.trim_end_matches("...").to_string());
                                }
                            }
                            ui.close();
                        }
                    }
                });
            });
        });
    }

    /// Ribbon toolbar (CueMol2 `topbar/cuemol2-ribbon.xul`): a tabbed toolbar with
    /// Home / Measure / Animation tabs sitting directly below the menu bar.
    fn ribbon_bar(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::TopBottomPanel::top("ribbon").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.ribbon_tab, RibbonTab::Home, "Home");
                ui.selectable_value(&mut self.ribbon_tab, RibbonTab::Measure, "Measure");
                ui.selectable_value(&mut self.ribbon_tab, RibbonTab::Animation, "Animation");
            });
            ui.separator();
            match self.ribbon_tab {
                RibbonTab::Home => self.ribbon_home(ui, camera),
                RibbonTab::Measure => self.ribbon_measure(ui),
                RibbonTab::Animation => self.ribbon_animation(ui),
            }
        });
    }

    fn ribbon_home(&mut self, ui: &mut egui::Ui, camera: &mut CameraController) {
        ui.horizontal(|ui| {
            if ui.button("New Tab").on_hover_text("New scene").clicked() {
                self.new_scene(camera);
            }
            if ui.button("Open").on_hover_text("Open file").clicked() {
                self.open_current_path(camera);
            }
            if ui.button("Save As").on_hover_text("Save file as").clicked() {
                self.save_selected_object();
            }
            ui.separator();
            if ui.button("Open Scene").clicked() {
                self.open_current_path(camera);
            }
            if ui.button("Reload").on_hover_text("Reload scene").clicked() {
                self.reload_current_path(camera);
            }
            if ui.button("Save Scene").clicked() {
                self.save_current_scene(camera);
            }
            ui.separator();
            if ui.button("Get PDB").clicked() {
                self.show_get_pdb = true;
            }
            ui.separator();
            if ui.button("Undo").clicked() {
                self.undo(camera);
            }
            if ui.button("Redo").clicked() {
                self.redo(camera);
            }
            ui.separator();
            // Interaction-mode radio (CueMol2 Home tab: Rotate / Rect select).
            ui.selectable_value(
                &mut self.interaction_mode,
                InteractionMode::Rotate,
                "Rotate",
            );
            ui.selectable_value(
                &mut self.interaction_mode,
                InteractionMode::RectSelect,
                "Rect select",
            );
            ui.separator();
            if ui.button("Fit").clicked() {
                camera.fit_snapshot(&self.snapshot);
            }
        });
    }

    fn ribbon_measure(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Measure:");
            ui.selectable_value(&mut self.measure_mode, MeasureMode::Distance, "Distance");
            ui.selectable_value(&mut self.measure_mode, MeasureMode::Angle, "Angle");
            ui.selectable_value(&mut self.measure_mode, MeasureMode::Torsion, "Torsion");
            ui.separator();
            // Measure the current selection: 2 atoms → distance, 3 → angle, 4 → torsion
            // (CueMol2 AtomIntr distance/angle/torsion, computed from the picked atoms).
            match self.measure_selection() {
                Some(result) => ui.strong(result),
                None => ui.weak("select 2 / 3 / 4 atoms to measure distance / angle / torsion"),
            };
        });
    }

    /// Distance / angle / torsion of the currently selected atoms (in atom-index order),
    /// the CueMol2 AtomIntr measurement computed over a 2 / 3 / 4-atom selection.
    fn measure_selection(&self) -> Option<String> {
        let points: Vec<Vec3> = self
            .snapshot
            .atoms
            .iter()
            .filter(|atom| atom.selected)
            .map(|atom| vec3_from_vector(atom.center))
            .collect();
        match points.as_slice() {
            [a, b] => Some(format!("Distance: {:.3} Å", (*b - *a).length())),
            [a, b, c] => {
                let u = (*a - *b).normalize_or_zero();
                let v = (*c - *b).normalize_or_zero();
                let angle = u.dot(v).clamp(-1.0, 1.0).acos().to_degrees();
                Some(format!("Angle: {angle:.2}°"))
            }
            [a, b, c, d] => {
                let b1 = *b - *a;
                let b2 = *c - *b;
                let b3 = *d - *c;
                let n1 = b1.cross(b2);
                let n2 = b2.cross(b3);
                let m = n1.cross(b2.normalize_or_zero());
                let torsion = m.dot(n2).atan2(n1.dot(n2)).to_degrees();
                Some(format!("Torsion: {torsion:.2}°"))
            }
            _ => None,
        }
    }

    fn ribbon_animation(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let play_label = if self.animation_playing {
                "⏸ Pause"
            } else {
                "▶ Play"
            };
            if ui.button(play_label).clicked() {
                self.animation_playing = !self.animation_playing;
            }
            if ui.button("⏹ Stop").clicked() {
                self.animation_playing = false;
            }
            ui.separator();
            ui.label(format!("frames: {}", self.animation_frames));
        });
    }

    /// Left dock: a vertical accordion of CueMol2's eight side panels
    /// (`cuemol2-panels.js` / `sidepanelholder`). Each is an independently
    /// collapsible `CollapsingHeader`.
    fn left_panel(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::SidePanel::left("left_dock")
            .resizable(true)
            .default_width(240.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for panel in LeftPanel::ALL {
                        egui::CollapsingHeader::new(panel.title())
                            .default_open(matches!(panel, LeftPanel::Workspace))
                            .open(self.force_open_all_panels.then_some(true))
                            .show(ui, |ui| {
                                self.left_panel_body(ui, camera, panel);
                            });
                    }
                });
            });
    }

    fn left_panel_body(
        &mut self,
        ui: &mut egui::Ui,
        camera: &mut CameraController,
        panel: LeftPanel,
    ) {
        match panel {
            LeftPanel::Workspace => self.workspace_section(ui, camera),
            LeftPanel::MolStruct => self.molstruct_section(ui, camera),
            LeftPanel::Selection => self.selection_section(ui),
            LeftPanel::Coloring => self.coloring_section(ui),
            LeftPanel::Symmetry => self.symmetry_section(ui),
            LeftPanel::DensityMap => self.densitymap_section(ui),
            LeftPanel::View => self.view_section(ui, camera),
            LeftPanel::Animation => self.animation_section(ui),
        }
    }

    /// Workspace section — the scene/object/renderer tree (CueMol2 `objectTree`).
    fn workspace_section(&mut self, ui: &mut egui::Ui, camera: &mut CameraController) {
        for scene in self.state.scene_summaries() {
            let scene_label = format!("{}  [{} objects]", scene.name, scene.object_count);
            let selected_scene = Some(scene.uid) == self.state.active_scene();
            if ui.selectable_label(selected_scene, scene_label).clicked() {
                self.state.set_active_scene(Some(scene.uid));
                self.selected_object = None;
                self.selected_renderer = None;
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
            }
            ui.indent(format!("scene-{}", scene.uid.get()), |ui| {
                for object in self.state.object_summaries(scene.uid) {
                    let selected = self.selected_object == Some(object.uid);
                    let label = format!(
                        "{}  [{} atoms, {} bonds]",
                        object.name, object.atom_count, object.bond_count
                    );
                    if ui.selectable_label(selected, label).clicked() {
                        self.selected_object = Some(object.uid);
                        self.selected_renderer = None;
                    }
                    ui.indent(format!("object-{}", object.uid.get()), |ui| {
                        for renderer in self.state.renderer_summaries(object.uid) {
                            let selected = self.selected_renderer == Some(renderer.uid);
                            let mut visible = renderer.visible;
                            ui.horizontal(|ui| {
                                if ui
                                    .checkbox(&mut visible, "")
                                    .on_hover_text("Visible")
                                    .changed()
                                {
                                    self.set_renderer_visibility(object.uid, renderer.uid, visible);
                                }
                                let label =
                                    format!("{} ({})", renderer.name, renderer.renderer_type);
                                if ui.selectable_label(selected, label).clicked() {
                                    self.selected_object = Some(object.uid);
                                    self.selected_renderer = Some(renderer.uid);
                                }
                            });
                        }
                    });
                }
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Zoom").on_hover_text("Zoom at scene").clicked() {
                camera.fit_snapshot(&self.snapshot);
            }
            if ui.button("Add").on_hover_text("Add renderer").clicked() {
                let target = self.selected_object.or_else(|| {
                    self.state
                        .active_object_summaries()
                        .first()
                        .map(|object| object.uid)
                });
                if let Some(object_uid) = target {
                    let count = self.state.renderer_summaries(object_uid).len();
                    self.open_renderer_dialog(object_uid, count + 1);
                }
            }
            if ui.button("Delete").clicked() {
                if let (Some(object_uid), Some(renderer_uid)) =
                    (self.selected_object, self.selected_renderer)
                {
                    self.delete_renderer(object_uid, renderer_uid);
                }
            }
            if ui.button("Property").clicked() {
                self.show_inspector = true;
            }
        });
    }

    /// MolStruct section — the molecule's chain → residue → atom hierarchy
    /// (CueMol2 `molstruct-panel.xul` `molStructTree`): a molecule selector menulist,
    /// a Select/Center/Zoom/Property toolbar, and a collapsible tree where each row is
    /// click-to-select (the triangle toggles, the label selects, like the XUL tree).
    fn molstruct_section(&mut self, ui: &mut egui::Ui, camera: &mut CameraController) {
        let tree = self.state.active_structure_tree();
        if tree.is_empty() {
            ui.weak("No molecule loaded.");
            return;
        }

        // Molecule selector menulist — default to the first molecule, kept valid if the
        // previously-chosen object was removed.
        let mut selected = self
            .molstruct_object
            .filter(|uid| tree.iter().any(|object| object.object_uid == *uid))
            .or_else(|| tree.first().map(|object| object.object_uid));
        let selected_name = tree
            .iter()
            .find(|object| Some(object.object_uid) == selected)
            .map(|object| object.object_name.clone())
            .unwrap_or_default();
        egui::ComboBox::from_id_salt("molstruct-molecule")
            .selected_text(selected_name)
            .show_ui(ui, |ui| {
                for object in &tree {
                    ui.selectable_value(
                        &mut selected,
                        Some(object.object_uid),
                        &object.object_name,
                    );
                }
            });
        self.molstruct_object = selected;
        let Some(object) = tree
            .iter()
            .find(|object| Some(object.object_uid) == selected)
        else {
            return;
        };

        // Toolbar (CueMol2 molStruct toolbar: Select atoms / Center at / Zoom at /
        // Properties). "Select atoms" selects the whole molecule; finer selection comes
        // from clicking a chain / residue / atom row below.
        let mut apply: Option<String> = None;
        let mut center = false;
        let mut zoom = false;
        ui.horizontal(|ui| {
            if ui
                .button("Select")
                .on_hover_text("Select all atoms")
                .clicked()
            {
                apply = Some("*".to_string());
            }
            if ui
                .button("Center")
                .on_hover_text("Center view on selection")
                .clicked()
            {
                center = true;
            }
            if ui
                .button("Zoom")
                .on_hover_text("Zoom to fit molecule")
                .clicked()
            {
                zoom = true;
            }
            if ui.button("Property").clicked() {
                self.show_inspector = true;
            }
            let toggle = if self.molstruct_expand_all {
                "Collapse"
            } else {
                "Expand"
            };
            if ui
                .button(toggle)
                .on_hover_text("Expand / collapse all")
                .clicked()
            {
                self.molstruct_expand_all = !self.molstruct_expand_all;
            }
            let surface_label = if self.show_surface {
                "Surface ✓"
            } else {
                "Surface"
            };
            if ui
                .button(surface_label)
                .on_hover_text("Show / hide the molecular surface (SES)")
                .clicked()
            {
                self.toggle_surface();
            }
            let clip_label = if self.clip_surface {
                "Clip ✓"
            } else {
                "Clip"
            };
            if ui
                .button(clip_label)
                .on_hover_text("Live in-viewport surface clipping at the cut plane")
                .clicked()
            {
                self.toggle_clip_surface();
            }
            if self.clip_surface {
                let mut offset = self.clip_offset;
                if ui
                    .add(egui::Slider::new(&mut offset, -20.0..=20.0).text("Clip Z"))
                    .changed()
                {
                    self.set_clip_offset(offset);
                }
            }
            // Electrostatic-potential surface (SCAPOT): load an APBS/OpenDX .dx map, then
            // colour the surface by it (red −, white 0, blue +) instead of by atom.
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.potential_path)
                        .hint_text("APBS .dx path")
                        .desired_width(120.0),
                );
                if ui
                    .button("Load .dx")
                    .on_hover_text("Load an electrostatic potential map")
                    .clicked()
                {
                    self.load_potential();
                }
                let potential_label = if self.potential_surface {
                    "Potential ✓"
                } else {
                    "Potential"
                };
                if ui
                    .add_enabled(
                        self.state.has_surface_potential(),
                        egui::Button::new(potential_label),
                    )
                    .on_hover_text("Colour the surface by electrostatic potential (SCAPOT)")
                    .clicked()
                {
                    self.toggle_potential_surface();
                }
                if self.state.has_surface_potential() {
                    // SCAPOT colour ± scale (potential units); 0 = robust auto (2·rmsd).
                    let mut scale = self.state.potential_scale().unwrap_or(0.0);
                    if ui
                        .add(egui::DragValue::new(&mut scale).speed(0.5).prefix("±"))
                        .on_hover_text("Potential colour ± scale (0 = auto, robust to spikes)")
                        .changed()
                    {
                        self.state
                            .set_potential_scale((scale > 0.0).then_some(scale));
                        if self.potential_surface {
                            self.rebuild_surface_mesh();
                            self.rebuild_snapshot();
                        }
                    }
                }
            });
            // Import an external surface mesh (Wavefront OBJ / Stanford PLY) into the
            // surface slot, shown via the same toggle as the computed SES.
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.mesh_import_path)
                        .hint_text("mesh .obj/.ply path")
                        .desired_width(120.0),
                );
                if ui
                    .button("Import mesh")
                    .on_hover_text("Load an OBJ/PLY surface mesh and show it as the surface")
                    .clicked()
                {
                    self.import_mesh_surface(camera);
                }
            });
            let labels_label = if self.show_labels {
                "Labels ✓"
            } else {
                "Labels"
            };
            if ui
                .button(labels_label)
                .on_hover_text("Label the selected atoms in the 3D view")
                .clicked()
            {
                self.show_labels = !self.show_labels;
            }
            let hbond_label = if self.show_hbonds {
                "H-bonds ✓"
            } else {
                "H-bonds"
            };
            if ui
                .button(hbond_label)
                .on_hover_text("Show hydrogen bonds as dashed lines (N···O heuristic)")
                .clicked()
            {
                self.show_hbonds = !self.show_hbonds;
                self.rebuild_snapshot();
            }
        });

        egui::ScrollArea::vertical()
            .max_height(360.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for chain in &object.chains {
                    let chain_disp = if chain.chain_id.is_empty() {
                        "_"
                    } else {
                        chain.chain_id.as_str()
                    };
                    let chain_id = ui.make_persistent_id((
                        "molstruct-chain",
                        object.object_uid.get(),
                        chain_disp,
                        self.molstruct_expand_all,
                    ));
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        chain_id,
                        self.molstruct_expand_all,
                    )
                    .show_header(ui, |ui| {
                        let label = format!("Chain {chain_disp}  ({} res)", chain.residues.len());
                        if ui.selectable_label(false, label).clicked() {
                            apply = Some(format!("chain {}", chain.chain_id));
                        }
                    })
                    .body(|ui| {
                        for residue in &chain.residues {
                            let ins = residue.ins_code.map(String::from).unwrap_or_default();
                            let res_label =
                                format!("{} {}{}", residue.res_name.trim(), residue.res_id, ins);
                            let res_id = ui.make_persistent_id((
                                "molstruct-res",
                                object.object_uid.get(),
                                chain_disp,
                                residue.res_id,
                                residue.ins_code,
                                self.molstruct_expand_all,
                            ));
                            let res_sel = if chain.chain_id.is_empty() {
                                format!("resid {}", residue.res_id)
                            } else {
                                format!("chain {} and resid {}", chain.chain_id, residue.res_id)
                            };
                            egui::collapsing_header::CollapsingState::load_with_default_open(
                                ui.ctx(),
                                res_id,
                                self.molstruct_expand_all,
                            )
                            .show_header(ui, |ui| {
                                if ui.selectable_label(false, &res_label).clicked() {
                                    apply = Some(res_sel.clone());
                                }
                            })
                            .body(|ui| {
                                for atom in &residue.atoms {
                                    let atom_label = format!("{}  ({})", atom.name, atom.element);
                                    if ui.selectable_label(false, atom_label).clicked() {
                                        apply = Some(format!("aid {}", atom.serial));
                                    }
                                }
                            });
                        }
                    });
                }
            });

        if let Some(sel) = apply {
            self.selection_text = sel;
            self.push_selection_history();
            self.rebuild_snapshot();
        }
        if center {
            if let Some(target) = self.selection_centroid() {
                camera.target = target;
                self.set_status("Centered on selection");
            }
        }
        if zoom {
            camera.fit_snapshot(&self.snapshot);
            self.set_status("Zoomed to molecule");
        }
    }

    /// Centroid of the currently selected atoms in the live snapshot (used by the
    /// MolStruct "Center" toolbar button). Falls back to the whole-scene bounds
    /// centre when nothing is selected.
    fn selection_centroid(&self) -> Option<Vec3> {
        let mut sum = Vec3::ZERO;
        let mut count = 0u32;
        for atom in self.snapshot.atoms.iter().filter(|atom| atom.selected) {
            sum += vec3_from_vector(atom.center);
            count += 1;
        }
        if count > 0 {
            return Some(sum / count as f32);
        }
        self.snapshot
            .bounds
            .map(|(min, max)| (vec3_from_vector(min) + vec3_from_vector(max)) * 0.5)
    }

    /// Selection section (CueMol2 `selection-panel.xul`): a molecule selector and two
    /// tabs — a free-text Command tab and a click-to-build Editor tab — over the shared
    /// selection grammar.
    fn selection_section(&mut self, ui: &mut egui::Ui) {
        // Molecule selector menulist (parity with the XUL panel; the chosen molecule
        // scopes the Editor tab's per-chain quick-selects).
        let sequences = self.state.active_sequence_summaries();
        if !sequences.is_empty() {
            let mut selected = self
                .molstruct_object
                .filter(|uid| sequences.iter().any(|seq| seq.object_uid == *uid))
                .or_else(|| sequences.first().map(|seq| seq.object_uid));
            let name = sequences
                .iter()
                .find(|seq| Some(seq.object_uid) == selected)
                .map(|seq| seq.object_name.clone())
                .unwrap_or_default();
            egui::ComboBox::from_id_salt("selection-molecule")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    let mut seen = std::collections::BTreeSet::new();
                    for seq in &sequences {
                        if seen.insert(seq.object_uid) {
                            ui.selectable_value(
                                &mut selected,
                                Some(seq.object_uid),
                                &seq.object_name,
                            );
                        }
                    }
                });
            self.molstruct_object = selected;
        }

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.selection_tab, SelectionTab::Command, "Command");
            ui.selectable_value(&mut self.selection_tab, SelectionTab::Editor, "Editor");
        });
        ui.separator();

        match self.selection_tab {
            SelectionTab::Command => self.selection_command_tab(ui),
            SelectionTab::Editor => self.selection_editor_tab(ui, &sequences),
        }

        let selected_count = self
            .snapshot
            .atoms
            .iter()
            .filter(|atom| atom.selected)
            .count();
        ui.separator();
        ui.label(format!("Selected: {selected_count} atoms"));
    }

    /// Command tab: the free-text selection expression + history (CueMol2 Command tab).
    fn selection_command_tab(&mut self, ui: &mut egui::Ui) {
        ui.label("Selection command");
        let response = ui.add_sized(
            [ui.available_width(), 56.0],
            egui::TextEdit::multiline(&mut self.selection_text)
                .hint_text("name CA")
                .font(egui::TextStyle::Monospace),
        );
        if response.changed() {
            self.rebuild_snapshot();
        }
        ui.horizontal(|ui| {
            if ui.button("Select").clicked() {
                self.push_selection_history();
                self.rebuild_snapshot();
            }
            if ui.button("Clear").clicked() {
                self.selection_text.clear();
                self.rebuild_snapshot();
            }
        });
        if !self.selection_history.is_empty() {
            let mut history_pick: Option<String> = None;
            ui.menu_button("History", |ui| {
                for past in self.selection_history.iter().rev() {
                    if ui.selectable_label(false, past).clicked() {
                        history_pick = Some(past.clone());
                        ui.close();
                    }
                }
            });
            if let Some(sel) = history_pick {
                self.selection_text = sel;
                self.push_selection_history();
                self.rebuild_snapshot();
            }
        }
    }

    /// Editor tab: click-to-build selection terms over the shared grammar — the egui
    /// analogue of the XUL Editor tab's Hierarchical / Terminal / Around–Expand adds.
    fn selection_editor_tab(&mut self, ui: &mut egui::Ui, sequences: &[GuiSequenceSummary]) {
        // `set` replaces the whole expression; `wrap` rewrites the current one.
        let mut set: Option<String> = None;
        let mut wrap: Option<String> = None;

        ui.label("Whole molecule");
        ui.horizontal_wrapped(|ui| {
            if ui.button("All").clicked() {
                set = Some("*".to_string());
            }
            if ui.button("None").clicked() {
                set = Some("none".to_string());
            }
            if ui.button("Backbone").clicked() {
                set = Some("bymainch *".to_string());
            }
            if ui.button("Sidechain").clicked() {
                set = Some("bysidech *".to_string());
            }
            if ui.button("Hydrogen").clicked() {
                set = Some("elem H".to_string());
            }
        });

        // Per-chain Hierarchical adds for the selected molecule.
        let active = self
            .molstruct_object
            .or_else(|| sequences.first().map(|seq| seq.object_uid));
        let chains: Vec<String> = sequences
            .iter()
            .filter(|seq| Some(seq.object_uid) == active)
            .map(|seq| seq.chain_id.clone())
            .collect();
        if !chains.is_empty() {
            ui.label("Chain");
            ui.horizontal_wrapped(|ui| {
                for chain in &chains {
                    let label = if chain.is_empty() {
                        "_"
                    } else {
                        chain.as_str()
                    };
                    if ui.button(label).clicked() {
                        set = Some(format!("chain {chain}"));
                    }
                }
            });
        }

        ui.label("Modify current");
        ui.horizontal_wrapped(|ui| {
            if ui.button("By residue").clicked() {
                wrap = Some("byres".to_string());
            }
            if ui.button("Around 5Å").clicked() {
                wrap = Some("around".to_string());
            }
            if ui.button("Invert").clicked() {
                wrap = Some("invert".to_string());
            }
        });

        if let Some(expr) = set {
            self.selection_text = expr;
            self.push_selection_history();
            self.rebuild_snapshot();
        } else if let Some(kind) = wrap {
            let current = self.selection_text.trim();
            if !current.is_empty() {
                self.selection_text = match kind.as_str() {
                    "byres" => format!("byres ({current})"),
                    "around" => format!("({current}) around 5"),
                    "invert" => format!("!({current})"),
                    _ => current.to_string(),
                };
                self.push_selection_history();
                self.rebuild_snapshot();
            }
        }
    }

    /// Push the current selection text onto the most-recently-used history list.
    fn push_selection_history(&mut self) {
        let sel = self.selection_text.trim().to_string();
        if sel.is_empty() {
            return;
        }
        self.selection_history.retain(|entry| entry != &sel);
        self.selection_history.push(sel);
        if self.selection_history.len() > 20 {
            self.selection_history.remove(0);
        }
    }

    /// Coloring section (deepened in M2 with per-type decks).
    fn coloring_section(&mut self, ui: &mut egui::Ui) {
        let objects = self.state.active_object_summaries();
        if objects.is_empty() {
            ui.weak("No molecule loaded.");
            return;
        }

        // Molecule selector (the Coloring panel applies object-level coloring).
        let mut selected = self
            .molstruct_object
            .filter(|uid| objects.iter().any(|object| object.uid == *uid))
            .or_else(|| objects.first().map(|object| object.uid));
        let name = objects
            .iter()
            .find(|object| Some(object.uid) == selected)
            .map(|object| object.name.clone())
            .unwrap_or_default();
        egui::ComboBox::from_id_salt("coloring-molecule")
            .selected_text(name)
            .show_ui(ui, |ui| {
                for object in &objects {
                    ui.selectable_value(&mut selected, Some(object.uid), &object.name);
                }
            });
        self.molstruct_object = selected;
        let Some(object_uid) = selected else {
            return;
        };

        // Reflect the molecule's live object-coloring scheme in the dropdown.
        let mut mode = match self.state.object_coloring_type(object_uid).as_deref() {
            Some("RainbowColoring") => ColoringMode::Rainbow,
            Some("BfacColoring") => ColoringMode::Bfac,
            _ => ColoringMode::Element,
        };
        let before = mode;
        egui::ComboBox::from_id_salt("coloring-type")
            .selected_text(match mode {
                ColoringMode::Element => "Element (CPK)",
                ColoringMode::Rainbow => "Rainbow",
                ColoringMode::Bfac => "B-factor",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut mode, ColoringMode::Element, "Element (CPK)");
                ui.selectable_value(&mut mode, ColoringMode::Rainbow, "Rainbow");
                ui.selectable_value(&mut mode, ColoringMode::Bfac, "B-factor");
            });

        // Rainbow exposes its hue endpoints (the XUL Rainbow deck's Start H / End H).
        let mut params_changed = false;
        if mode == ColoringMode::Rainbow {
            ui.horizontal(|ui| {
                ui.label("Start H");
                params_changed |= ui
                    .add(egui::Slider::new(&mut self.coloring_start_hue, 0.0..=360.0).suffix("°"))
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("End H");
                params_changed |= ui
                    .add(egui::Slider::new(&mut self.coloring_end_hue, 0.0..=360.0).suffix("°"))
                    .changed();
            });
        } else if mode == ColoringMode::Bfac {
            ui.weak("Blue (low) → red (high), auto-scaled to the molecule.");
        } else {
            ui.weak("Per-element CPK colours (carbon = molecule colour).");
        }

        if mode != before || params_changed {
            self.apply_object_coloring(object_uid, mode);
        }
    }

    /// Push the chosen object-coloring scheme into the model and re-render. The
    /// snapshot's object-coloring cascade (`molecule_render_snapshot`) recolours each
    /// carbon atom (and the ribbon backbone) by the scheme — the C++ `$molcol` path.
    fn apply_object_coloring(&mut self, object_uid: cuemol_core::Uid, mode: ColoringMode) {
        let (coloring_type, attrs) = match mode {
            ColoringMode::Element => (None, std::collections::BTreeMap::new()),
            ColoringMode::Rainbow => {
                let mut attrs = std::collections::BTreeMap::new();
                attrs.insert("mode".to_string(), "mol".to_string());
                attrs.insert(
                    "start_hue".to_string(),
                    format!("{:.0}", self.coloring_start_hue),
                );
                attrs.insert(
                    "end_hue".to_string(),
                    format!("{:.0}", self.coloring_end_hue),
                );
                attrs.insert("sat".to_string(), "1".to_string());
                attrs.insert("bri".to_string(), "1".to_string());
                (Some("RainbowColoring".to_string()), attrs)
            }
            ColoringMode::Bfac => (
                Some("BfacColoring".to_string()),
                std::collections::BTreeMap::new(),
            ),
        };
        if let Err(error) = self
            .state
            .set_object_coloring(object_uid, coloring_type, attrs)
        {
            self.set_status(format!("Coloring failed: {error}"));
            return;
        }
        self.rebuild_snapshot();
        self.set_status("Applied coloring");
    }

    /// Symmetry section — crystal cell / space group table (CueMol2 `symmetry-panel.xul`).
    fn symmetry_section(&mut self, ui: &mut egui::Ui) {
        let crystals = self.state.active_crystal_summaries();
        if crystals.is_empty() {
            ui.weak("No crystal cell (non-crystallographic).");
            return;
        }
        for cell in crystals {
            ui.label(egui::RichText::new(&cell.object_name).strong());
            egui::Grid::new(("symm-grid", cell.object_uid.get()))
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Space group");
                    let sg = cell.space_group_name.trim();
                    ui.label(if sg.is_empty() { "(none)" } else { sg });
                    ui.end_row();
                    ui.label("a, b, c (Å)");
                    ui.label(format!("{:.3}, {:.3}, {:.3}", cell.a, cell.b, cell.c));
                    ui.end_row();
                    ui.label("α, β, γ (°)");
                    ui.label(format!(
                        "{:.2}, {:.2}, {:.2}",
                        cell.alpha, cell.beta, cell.gamma
                    ));
                    ui.end_row();
                });
        }
        ui.separator();
        // CueMol2 Symmetry panel "Unit cell" / "Symm mol" toggles: draw the cell box
        // and the home-cell symmetry-mate copies.
        let mut changed = ui.checkbox(&mut self.show_unit_cell, "Unit cell").changed();
        changed |= ui
            .checkbox(&mut self.show_symm_mates, "Symm mol")
            .on_hover_text("Show symmetry-related molecules in the unit cell")
            .changed();
        if changed {
            self.rebuild_snapshot();
        }
    }

    /// Density-map section (deepened in M2 with contour sliders).
    fn densitymap_section(&mut self, ui: &mut egui::Ui) {
        let maps = self
            .state
            .active_scene()
            .map(|scene| self.state.density_map_summaries(scene))
            .unwrap_or_default();
        if maps.is_empty() {
            ui.weak("No density map loaded.");
            return;
        }
        for map in maps {
            ui.label(egui::RichText::new(&map.name).strong());
            ui.label(format!(
                "{} grid {}×{}×{}",
                map.format, map.counts[0], map.counts[1], map.counts[2]
            ));
            ui.label(format!(
                "ρ {:.3} … {:.3}  (mean {:.3}, rmsd {:.3})",
                map.min_density, map.max_density, map.mean_density, map.rmsd_density
            ));
        }
        ui.separator();
        // CueMol2 Density map panel: chickenwire contour + filled-surface modes + σ level.
        let mut changed = ui
            .checkbox(&mut self.show_density_contour, "Show contour")
            .changed();
        let solid_label = if self.show_density_solid {
            "Solid surface ✓"
        } else {
            "Solid surface"
        };
        if ui.button(solid_label).clicked() {
            // Toggling solid (re)builds + caches the marching-cubes mesh.
            self.toggle_density_solid();
        }
        ui.horizontal(|ui| {
            ui.label("Level (σ)");
            if ui
                .add(egui::Slider::new(&mut self.density_level, 0.5..=5.0).step_by(0.1))
                .changed()
            {
                changed = true;
                // A level change invalidates the cached solid mesh; rebuild it if shown.
                if self.show_density_solid {
                    self.density_solid_mesh =
                        self.state.build_density_surface_mesh(self.density_level);
                }
            }
        });
        if changed {
            self.rebuild_snapshot();
        }
    }

    /// View section — fake-dial style camera controls (CueMol2 `fakedial-panel.xul`).
    /// Rotation is relative-delta (ADR-0025); translation / zoom / slab are absolute.
    fn view_section(&mut self, ui: &mut egui::Ui, camera: &mut CameraController) {
        ui.horizontal(|ui| {
            ui.radio_value(&mut camera.perspective, false, "Ortho");
            ui.radio_value(&mut camera.perspective, true, "Persp");
        });
        ui.separator();
        ui.label("Rotation");
        for (axis, label) in [(Vec3::X, "Rot X"), (Vec3::Y, "Rot Y"), (Vec3::Z, "Rot Z")] {
            ui.horizontal(|ui| {
                ui.label(label);
                if ui.small_button("−15°").clicked() {
                    camera.orientation =
                        Quat::from_axis_angle(axis, -15.0_f32.to_radians()) * camera.orientation;
                }
                if ui.small_button("+15°").clicked() {
                    camera.orientation =
                        Quat::from_axis_angle(axis, 15.0_f32.to_radians()) * camera.orientation;
                }
            });
        }
        ui.separator();
        ui.label("Translation (Å)");
        ui.add(
            egui::DragValue::new(&mut camera.target.x)
                .prefix("X ")
                .speed(0.2),
        );
        ui.add(
            egui::DragValue::new(&mut camera.target.y)
                .prefix("Y ")
                .speed(0.2),
        );
        ui.add(
            egui::DragValue::new(&mut camera.target.z)
                .prefix("Z ")
                .speed(0.2),
        );
        ui.separator();
        ui.label("Zoom / Slab");
        ui.add(
            egui::DragValue::new(&mut camera.ortho_scale)
                .prefix("Zoom ")
                .speed(1.0),
        );
        camera.ortho_scale = camera.ortho_scale.max(1.0);
        ui.add(
            egui::DragValue::new(&mut camera.distance)
                .prefix("Dist ")
                .speed(1.0),
        );
        if ui.button("Fit").clicked() {
            camera.fit_snapshot(&self.snapshot);
        }
    }

    /// Advance the animation timeline by `dt` seconds, driving the camera from the
    /// active object via the shared-core `anim_spin_direction` (CueMol2 anim playback;
    /// empty list → a simple continuous spin).
    fn advance_animation(&mut self, dt: f32, camera: &mut CameraController) {
        let frame = anim_step(
            &self.animation_objects,
            self.animation_elapsed as f64,
            dt as f64,
        );
        self.animation_elapsed = frame.elapsed as f32;
        if frame.rotate_dir != 0.0 {
            camera.rotate_by(frame.rotate_dir as f32 * 120.0 * dt, 0.0);
        }
        if frame.slide_dir != 0.0 {
            // zoom_by: +step zooms in, so SlideIn (slide_dir -1) → +step.
            camera.zoom_by(-frame.slide_dir as f32 * 20.0 * dt);
        }
        if let Some(target) = frame.set_visible {
            self.apply_anim_visibility(target);
        }
        if let Some(fraction) = frame.frame_fraction {
            if let Some(object_uid) = self
                .state
                .active_object_summaries()
                .first()
                .map(|object| object.uid)
            {
                if self.state.object_has_trajectory(object_uid) {
                    self.state.apply_trajectory_fraction(object_uid, fraction);
                    self.rebuild_snapshot();
                }
            }
        }
    }

    /// Set every renderer's visibility for a Show/Hide animation object, re-tessellating
    /// only if something actually changed (so it fires once on entry, not every frame).
    fn apply_anim_visibility(&mut self, target: bool) {
        let mut changed = false;
        let objects: Vec<_> = self
            .state
            .active_object_summaries()
            .iter()
            .map(|object| object.uid)
            .collect();
        for object_uid in objects {
            for renderer in self.state.renderer_summaries(object_uid) {
                if renderer.visible != target {
                    let _ = self
                        .state
                        .set_renderer_visible(object_uid, renderer.uid, target);
                    changed = true;
                }
            }
        }
        if changed {
            self.rebuild_snapshot();
        }
    }

    /// Animation panel — the timeline object list (CueMol2 `anim-panel`): Add (Spin /
    /// Rock / Pause) / Delete / Move up·down + Play / Stop.
    fn animation_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.menu_button("Add", |ui| {
                for kind in [
                    AnimKind::Spin,
                    AnimKind::CameraMotion,
                    AnimKind::Rock,
                    AnimKind::Pause,
                    AnimKind::SlideIn,
                    AnimKind::SlideOut,
                    AnimKind::ShowAnim,
                    AnimKind::HideAnim,
                    AnimKind::MolAnim,
                ] {
                    if ui.button(kind.label()).clicked() {
                        self.animation_objects.push(AnimObject {
                            kind,
                            duration: 3.0,
                        });
                        ui.close();
                    }
                }
            });
            let play = if self.animation_playing {
                "⏸ Pause"
            } else {
                "▶ Play"
            };
            if ui.button(play).clicked() {
                self.animation_playing = !self.animation_playing;
            }
            if ui.button("⏹ Stop").clicked() {
                self.animation_playing = false;
                self.animation_elapsed = 0.0;
            }
        });
        // MolAnim source: load an AMBER mdcrd trajectory onto the active object.
        ui.horizontal(|ui| {
            ui.label("Traj:");
            ui.add(
                egui::TextEdit::singleline(&mut self.trajectory_path)
                    .hint_text("trajectory.mdcrd")
                    .desired_width(120.0),
            );
            if ui.button("Load").clicked() {
                let path = self.trajectory_path.trim().to_string();
                if let Some(object_uid) = self
                    .state
                    .active_object_summaries()
                    .first()
                    .map(|object| object.uid)
                {
                    match self
                        .state
                        .load_amber_trajectory(object_uid, std::path::Path::new(&path))
                    {
                        Ok(frames) => self.set_status(format!("Trajectory: {frames} frames")),
                        Err(error) => self.set_status(format!("Trajectory load failed: {error}")),
                    }
                }
            }
        });
        ui.separator();
        if self.animation_objects.is_empty() {
            ui.weak("No animation objects — Add a Spin / Rock / Pause (or Play for a spin).");
            return;
        }
        let mut delete: Option<usize> = None;
        let mut move_up: Option<usize> = None;
        for (index, object) in self.animation_objects.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}. {}", index + 1, object.kind.label()));
                ui.add(
                    egui::DragValue::new(&mut object.duration)
                        .speed(0.1)
                        .range(0.1..=60.0)
                        .suffix(" s"),
                );
                if index > 0 && ui.small_button("↑").clicked() {
                    move_up = Some(index);
                }
                if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                    delete = Some(index);
                }
            });
        }
        if let Some(index) = move_up {
            self.animation_objects.swap(index - 1, index);
        }
        if let Some(index) = delete {
            self.animation_objects.remove(index);
            self.animation_elapsed = 0.0;
        }
    }

    /// Properties — CueMol2 opens object/renderer properties as a modal dialog
    /// (no right dock). Interim floating window; promoted to per-renderer modals in M3.
    fn properties_window(&mut self, ctx: &egui::Context, camera: &CameraController) {
        let mut open = self.show_inspector;
        egui::Window::new("Properties")
            .open(&mut open)
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();
                ui.label("Selection");
                let response = ui.add_sized(
                    [ui.available_width(), 72.0],
                    egui::TextEdit::multiline(&mut self.selection_text)
                        .hint_text("name CA")
                        .font(egui::TextStyle::Monospace),
                );
                if response.changed() {
                    self.rebuild_snapshot();
                }
                let selected_count = self
                    .snapshot
                    .atoms
                    .iter()
                    .filter(|atom| atom.selected)
                    .count();
                ui.label(format!("Selected atoms: {selected_count}"));
                if ui.button("Clear Selection").clicked() {
                    self.selection_text.clear();
                    self.rebuild_snapshot();
                }

                ui.separator();
                ui.label("Scene");
                if let Some(scene) = self.state.active_scene_summary() {
                    ui.monospace(format!("uid: {}", scene.uid.get()));
                    ui.label(format!("objects: {}", scene.object_count));
                    ui.label(format!("cameras: {}", scene.camera_count));
                }
                if let Some(object_uid) = self.selected_object {
                    ui.separator();
                    ui.label("Object");
                    if let Some(object) = self
                        .state
                        .active_object_summaries()
                        .into_iter()
                        .find(|object| object.uid == object_uid)
                    {
                        ui.monospace(format!("uid: {}", object.uid.get()));
                        ui.label(format!("name: {}", object.name));
                        ui.label(format!("atoms: {}", object.atom_count));
                        ui.label(format!("residues: {}", object.residue_count));
                        ui.label(format!("bonds: {}", object.bond_count));
                        if ui.button("Add Renderer").clicked() {
                            self.open_renderer_dialog(object.uid, object.renderer_count + 1);
                        }
                    }
                }
                if let Some(renderer_uid) = self.selected_renderer {
                    ui.separator();
                    ui.label("Renderer");
                    for object in self.state.active_object_summaries() {
                        if let Some(renderer) = self
                            .state
                            .renderer_summaries(object.uid)
                            .into_iter()
                            .find(|renderer| renderer.uid == renderer_uid)
                        {
                            ui.monospace(format!("uid: {}", renderer.uid.get()));
                            ui.label(format!("name: {}", renderer.name));
                            ui.label(format!("type: {}", renderer.renderer_type));
                            if let Some(group) = renderer.group.as_deref() {
                                ui.label(format!("group: {group}"));
                            }
                            if let Some(style) = renderer.style.as_deref() {
                                ui.label(format!("style: {style}"));
                            }
                            let mut visible = renderer.visible;
                            if ui.checkbox(&mut visible, "Visible").changed() {
                                self.set_renderer_visibility(object.uid, renderer.uid, visible);
                            }
                            // Editable geometry overrides (CueMol2 renderer property
                            // dialog): bond cylinder + atom sphere radius. The inline
                            // attr wins over the style, so the 3D view updates live.
                            let mut bondw = self
                                .state
                                .renderer_property(object.uid, renderer.uid, "bondw")
                                .and_then(|value| value.parse::<f32>().ok())
                                .unwrap_or(0.18);
                            ui.horizontal(|ui| {
                                ui.label("Bond radius");
                                if ui
                                    .add(egui::DragValue::new(&mut bondw).speed(0.01))
                                    .changed()
                                {
                                    bondw = bondw.clamp(0.01, 2.0);
                                    let _ = self.state.set_renderer_property(
                                        object.uid,
                                        renderer.uid,
                                        "bondw",
                                        format!("{bondw:.3}"),
                                    );
                                    self.rebuild_snapshot();
                                }
                            });
                            let mut sphr = self
                                .state
                                .renderer_property(object.uid, renderer.uid, "sphr")
                                .and_then(|value| value.parse::<f32>().ok())
                                .unwrap_or(0.30);
                            ui.horizontal(|ui| {
                                ui.label("Sphere radius");
                                if ui
                                    .add(egui::DragValue::new(&mut sphr).speed(0.01))
                                    .changed()
                                {
                                    sphr = sphr.clamp(0.05, 3.0);
                                    let _ = self.state.set_renderer_property(
                                        object.uid,
                                        renderer.uid,
                                        "sphr",
                                        format!("{sphr:.3}"),
                                    );
                                    self.rebuild_snapshot();
                                }
                            });
                            if ui.button("Delete Renderer").clicked() {
                                self.delete_renderer(object.uid, renderer.uid);
                            }
                            break;
                        }
                    }
                }

                ui.separator();
                ui.label("Scene Save");
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut self.save_path),
                );
                if ui.button("Save QSC").clicked() {
                    self.save_current_scene(camera);
                }
                ui.separator();
                ui.label("Object Save");
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut self.object_save_path),
                );
                if ui.button("Save Object").clicked() {
                    self.save_selected_object();
                }
                ui.separator();
                ui.label("View / Export");
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut self.view_save_path),
                );
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut self.export_path),
                );
                if ui.button("Export POV").clicked() {
                    self.export_current_scene();
                }
            });
        self.show_inspector = open;
    }

    fn bottom_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .default_height(150.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Output, "Output");
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Sequence, "Sequence");
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Animation, "Animation");
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Render, "Render");
                });
                ui.separator();
                match self.bottom_tab {
                    BottomTab::Output => self.output_tab(ui),
                    BottomTab::Sequence => self.sequence_tab(ui),
                    BottomTab::Animation => self.animation_tab(ui),
                    BottomTab::Render => self.render_tab(ui),
                }
            });
    }

    fn output_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Clear").clicked() {
                self.clear_log();
            }
            if ui.button("Save As").clicked() {
                self.save_log();
            }
        });
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log_lines {
                    ui.monospace(line);
                }
            });
    }

    /// Interactive per-residue sequence viewer (CueMol2 `seqpanel`): each residue is a
    /// clickable 1-letter cell that selects it; cells whose atoms are in the current
    /// selection are highlighted, so the sequence and the 3D selection stay in sync.
    /// Built from the shared-core `active_sequence_view`.
    fn sequence_tab(&mut self, ui: &mut egui::Ui) {
        let chains = self.state.active_sequence_view(&self.selection_text);
        if chains.is_empty() {
            ui.weak("No molecule loaded.");
            return;
        }
        let mut clicked: Option<String> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for chain in &chains {
                let chain_disp = if chain.chain_id.is_empty() {
                    "_"
                } else {
                    chain.chain_id.as_str()
                };
                ui.monospace(format!(
                    "{} : chain {} ({} res)",
                    chain.object_name,
                    chain_disp,
                    chain.residues.len()
                ));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for residue in &chain.residues {
                        let response = ui
                            .selectable_label(
                                residue.selected,
                                egui::RichText::new(&residue.letter).monospace(),
                            )
                            .on_hover_text(format!(
                                "{} {}",
                                residue.res_name.trim(),
                                residue.res_id
                            ));
                        if response.clicked() {
                            clicked = Some(residue.selection.clone());
                        }
                    }
                });
                ui.separator();
            }
        });

        if let Some(sel) = clicked {
            self.selection_text = sel;
            self.push_selection_history();
            self.rebuild_snapshot();
        }
    }

    fn animation_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("Frames: {}", self.animation_frames));
            if ui.button("Add Camera").clicked() {
                self.animation_frames += 1;
                self.set_status(format!("Animation camera {} added", self.animation_frames));
            }
            let label = if self.animation_playing {
                "Stop"
            } else {
                "Play"
            };
            if ui.button(label).clicked() {
                self.animation_playing = !self.animation_playing;
                self.set_status(format!("Animation playback: {}", self.animation_playing));
            }
        });
    }

    fn render_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Backend")
                .selected_text(&self.render_backend)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.render_backend, "POV-Ray".to_string(), "POV-Ray");
                    ui.selectable_value(
                        &mut self.render_backend,
                        "Internal".to_string(),
                        "Internal",
                    );
                });
            egui::ComboBox::from_label("Size")
                .selected_text(&self.render_preset)
                .show_ui(ui, |ui| {
                    for (label, width, height) in RENDER_PRESETS {
                        ui.selectable_value(
                            &mut self.render_preset,
                            (*label).to_string(),
                            format!("{label} ({width}x{height})"),
                        );
                    }
                });
            if ui.button("Settings").clicked() {
                self.show_options = true;
            }
            if ui.button("Render").clicked() {
                self.export_current_scene();
            }
            if ui.button("Cancel").clicked() {
                self.set_status("No render job is running");
            }
        });
    }

    fn open_current_path(&mut self, camera: &mut CameraController) {
        let open_path = self.open_path.trim().to_string();
        if open_path.is_empty() {
            self.set_status("Open path is empty");
            return;
        }
        let path = if is_network_url(&open_path) {
            match download_url_to_temp(&open_path) {
                Ok(path) => path,
                Err(error) => {
                    self.set_error(error);
                    return;
                }
            }
        } else {
            PathBuf::from(&open_path)
        };
        if is_view_file(&path) {
            self.load_view_path(&path, camera);
            return;
        }
        let undo = self.capture_undo();
        match self.state.open_path(&path) {
            Ok(GuiLoadedPath::Scene { scene_uid }) => {
                self.push_undo_snapshot(undo);
                self.selected_object = None;
                self.selected_renderer = None;
                self.last_loaded_path = Some(path);
                self.add_recent_path(self.last_loaded_path.clone().unwrap());
                // Report unsupported object classes or source formats so a partial
                // scene load is not silent.
                let skipped = self.state.active_scene_skipped_objects();
                if skipped.is_empty() {
                    self.set_status(format!("Opened scene {}", scene_uid.get()));
                } else {
                    self.set_status(format!(
                        "Opened scene {} ({} object(s) skipped: {})",
                        scene_uid.get(),
                        skipped.len(),
                        skipped
                            .iter()
                            .map(|(name, ty)| format!("{name} [{ty}]"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                // Display any surface objects the `.qsc` carried (OBJ/PLY-text MolSurf
                // objects), merged into the surface slot and shown via the surface toggle.
                let surfaces = self.state.active_scene_surface_meshes();
                if !surfaces.is_empty() {
                    let mut merged = cuemol_render::RenderMesh::default();
                    for mesh in surfaces {
                        let base = merged.vertices.len() as u32;
                        merged.vertices.extend(mesh.vertices);
                        merged
                            .indices
                            .extend(mesh.indices.iter().map(|index| base + index));
                    }
                    self.surface_mesh = Some(merged);
                    self.show_surface = true;
                }
                self.rebuild_snapshot();
                // Restore the scene's saved view (a `.qsc` `<camera>`); only a
                // camera-less scene falls back to auto-framing the bounding box.
                match self.state.active_scene_saved_camera() {
                    Some(saved) => *camera = CameraController::from_qsys_camera(&saved),
                    None => camera.fit_snapshot(&self.snapshot),
                }
            }
            Ok(GuiLoadedPath::Object {
                scene_uid,
                object_uid,
            }) => {
                self.push_undo_snapshot(undo);
                self.selected_object = Some(object_uid);
                self.selected_renderer = None;
                self.last_loaded_path = Some(path);
                self.add_recent_path(self.last_loaded_path.clone().unwrap());
                self.set_status(format!(
                    "Opened object {} in scene {}",
                    object_uid.get(),
                    scene_uid.get()
                ));
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
            }
            Ok(GuiLoadedPath::Map { scene_uid, map_uid }) => {
                self.push_undo_snapshot(undo);
                self.selected_object = None;
                self.selected_renderer = None;
                self.last_loaded_path = Some(path);
                self.add_recent_path(self.last_loaded_path.clone().unwrap());
                self.set_status(format!(
                    "Opened density map {} in scene {}",
                    map_uid.get(),
                    scene_uid.get()
                ));
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
            }
            Err(error) => self.set_error(error),
        }
    }

    fn reload_current_path(&mut self, camera: &mut CameraController) {
        let Some(path) = self.last_loaded_path.clone() else {
            self.set_status("No loaded path to reload");
            return;
        };
        self.open_path = path.to_string_lossy().to_string();
        self.open_current_path(camera);
    }

    fn new_scene(&mut self, camera: &mut CameraController) {
        let undo = self.capture_undo();
        self.state.new_scene("Untitled", true);
        self.push_undo_snapshot(undo);
        self.selected_object = None;
        self.selected_renderer = None;
        self.last_loaded_path = None;
        self.set_status("New scene created");
        self.rebuild_snapshot();
        camera.fit_snapshot(&self.snapshot);
    }

    fn close_current_scene(&mut self, camera: &mut CameraController) {
        let undo = self.capture_undo();
        match self.state.close_active_scene() {
            Ok(Some(_)) => {
                self.push_undo_snapshot(undo);
                self.selected_object = None;
                self.selected_renderer = None;
                self.last_loaded_path = None;
                self.set_status("Current tab closed");
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
            }
            Ok(None) => {
                self.push_undo_snapshot(undo);
                self.state.new_scene("Untitled", true);
                self.selected_object = None;
                self.selected_renderer = None;
                self.last_loaded_path = None;
                self.set_status("Last tab closed; new scene created");
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
            }
            Err(error) => self.set_error(error),
        }
    }

    fn save_current_scene(&mut self, camera: &CameraController) {
        if let Err(error) = self
            .state
            .set_active_scene_camera(camera.to_qsys_camera("__current"))
        {
            self.set_error(error);
            return;
        }
        match self.state.save_active_scene_qsc(self.save_path.trim()) {
            Ok(()) => self.set_status(format!("Saved {}", self.save_path.trim())),
            Err(error) => self.set_error(error),
        }
    }

    fn save_selected_object(&mut self) {
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        match self
            .state
            .save_object_as(object_uid, self.object_save_path.trim())
        {
            Ok(()) => self.set_status(format!("Saved object {}", self.object_save_path.trim())),
            Err(error) => self.set_error(error),
        }
    }

    fn save_current_view(&mut self, camera: &CameraController) {
        match fs::write(self.view_save_path.trim(), camera.to_view_data()) {
            Ok(()) => self.set_status(format!("Saved view {}", self.view_save_path.trim())),
            Err(error) => self.set_status(format!("Error: {error}")),
        }
    }

    fn load_view_path(&mut self, path: &Path, camera: &mut CameraController) {
        match fs::read_to_string(path)
            .map_err(CueMolError::from)
            .and_then(|data| CameraController::from_view_data(&data))
        {
            Ok(loaded) => {
                *camera = loaded;
                self.last_loaded_path = Some(path.to_path_buf());
                self.add_recent_path(path.to_path_buf());
                self.set_status(format!("Loaded view {}", path.to_string_lossy()));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn export_current_scene(&mut self) {
        let path = self.export_path.trim().to_string();
        // Dispatch on the file extension: `.png` renders an image via the offscreen
        // wgpu path (the same one the live viewport uses), anything else stays POV-Ray.
        // Previously every export produced POV-Ray regardless of the typed name.
        if crate::DesktopAppState::path_has_extension(&path, "png") {
            let (width, height) = self
                .viewport_rect
                .map(|rect| {
                    (
                        (rect.width().round() as u32).max(1),
                        (rect.height().round() as u32).max(1),
                    )
                })
                .unwrap_or((1280, 960));
            match self
                .state
                .export_active_scene_png(&path, width, height, &self.selection_text)
            {
                Ok(()) => self.set_status(format!("Exported {width}x{height} image to {path}")),
                Err(error) => self.set_error(error),
            }
        } else {
            match self
                .state
                .export_active_scene_pov(&path, &self.selection_text)
            {
                Ok(count) => self.set_status(format!("Exported {count} atoms to {path}")),
                Err(error) => self.set_error(error),
            }
        }
    }

    fn save_log(&mut self) {
        match fs::write(self.log_save_path.trim(), self.log_lines.join("\n")) {
            Ok(()) => self.set_status(format!("Saved log {}", self.log_save_path.trim())),
            Err(error) => self.set_status(format!("Error: {error}")),
        }
    }

    fn merge_current_path(&mut self, camera: &mut CameraController) {
        let path = PathBuf::from(self.open_path.trim());
        let undo = self.capture_undo();
        match self.state.open_path(&path) {
            Ok(GuiLoadedPath::Object { object_uid, .. }) => {
                self.push_undo_snapshot(undo);
                self.selected_object = Some(object_uid);
                self.selected_renderer = None;
                self.last_loaded_path = Some(path.clone());
                self.add_recent_path(path);
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
                self.set_status(format!("Merged object {}", object_uid.get()));
            }
            Ok(GuiLoadedPath::Scene { .. }) | Ok(GuiLoadedPath::Map { .. }) => {
                self.set_status("Merge molecule expects an object file");
            }
            Err(error) => self.set_error(error),
        }
    }

    fn download_pdb(&mut self, camera: &mut CameraController) {
        let pdb_id = self.pdb_id.trim().to_ascii_uppercase();
        if pdb_id.len() != 4 || !pdb_id.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            self.set_status("PDB ID must be four alphanumeric characters");
            return;
        }
        let url = format!("https://files.rcsb.org/download/{pdb_id}.pdb");
        let path = std::env::temp_dir().join(format!("cuemol-{pdb_id}.pdb"));
        match Command::new("curl")
            .arg("-fsSL")
            .arg("-o")
            .arg(&path)
            .arg(&url)
            .status()
        {
            Ok(status) if status.success() => {
                self.open_path = path.to_string_lossy().to_string();
                self.open_current_path(camera);
            }
            Ok(status) => self.set_status(format!("curl failed with status {status}")),
            Err(error) => self.set_status(format!("Error: {error}")),
        }
    }

    fn load_script_into_log(&mut self) {
        let script = match fs::read_to_string(self.script_path.trim()) {
            Ok(script) => script,
            Err(error) => {
                self.set_status(format!("Error: {error}"));
                return;
            }
        };
        // Run the JS with the embedded boa engine (cuemol.print/select/color).
        match self.state.run_script_js(&script) {
            Ok((log, selection)) => {
                for line in log.lines() {
                    self.log_lines.push(format!("  {line}"));
                }
                if let Some(expr) = selection {
                    self.selection_text = expr;
                    self.rebuild_snapshot();
                }
                self.set_status("Script executed");
            }
            Err(error) => self.set_status(format!("Script error: {error}")),
        }
    }

    fn apply_superpose(&mut self) {
        let Some(reference) = self.superpose_reference else {
            self.set_status("No reference object selected");
            return;
        };
        let Some(moving) = self.superpose_moving else {
            self.set_status("No moving object selected");
            return;
        };
        if reference == moving {
            self.set_status("Reference and moving objects must differ");
            return;
        }
        let undo = self.capture_undo();
        match self.state.superpose_objects(
            reference,
            &self.superpose_reference_selection,
            moving,
            &self.superpose_moving_selection,
        ) {
            Ok(matrix) => {
                self.push_undo_snapshot(undo);
                self.selected_object = Some(moving);
                self.rebuild_snapshot();
                self.log_lines.push(format!("Superpose matrix: {matrix}"));
                self.set_status("Molecular superposition applied");
            }
            Err(error) => self.set_error(error),
        }
    }

    fn edit_bond(&mut self, make: bool) {
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        let aid1 = match self.bond_aid1.trim().parse::<i32>() {
            Ok(value) => value,
            Err(_) => {
                self.set_status("Invalid first atom serial");
                return;
            }
        };
        let aid2 = match self.bond_aid2.trim().parse::<i32>() {
            Ok(value) => value,
            Err(_) => {
                self.set_status("Invalid second atom serial");
                return;
            }
        };
        let undo = self.capture_undo();
        let result = if make {
            self.state.make_bond_in_object(object_uid, aid1, aid2)
        } else {
            self.state.remove_bond_in_object(object_uid, aid1, aid2)
        };
        match result {
            Ok(true) => {
                self.push_undo_snapshot(undo);
                self.rebuild_snapshot();
                if make {
                    self.set_status("Bond created");
                } else {
                    self.set_status("Bond removed");
                }
            }
            Ok(false) => self.set_status("Bond was unchanged"),
            Err(error) => self.set_error(error),
        }
    }

    fn calculate_contacts(&mut self) {
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        match self.state.calc_atom_contacts_in_object(
            object_uid,
            &self.selection_text,
            self.contact_r_min,
            self.contact_r_max,
            self.contact_hbond,
            200,
        ) {
            Ok(json) => {
                self.log_lines.push(format!("Contacts: {json}"));
                self.set_status("Atom contacts calculated");
            }
            Err(error) => self.set_error(error),
        }
    }

    fn assign_secondary_structure(&mut self) {
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        let undo = self.capture_undo();
        match self.state.assign_secondary_structure_in_object(
            object_uid,
            self.secondary_ignore_breaks,
            self.secondary_angle,
        ) {
            Ok(count) => {
                self.push_undo_snapshot(undo);
                self.rebuild_snapshot();
                self.set_status(format!("Secondary structure assigned for {count} atoms"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn generate_surface_obj(&mut self) {
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        match self.state.write_surface_obj(
            object_uid,
            &self.selection_text,
            self.surface_density,
            self.surface_probe_radius,
            self.surface_path.trim(),
        ) {
            Ok(summary) => self.set_status(format!(
                "Surface OBJ written: {} vertices / {} triangles",
                summary.vertex_count, summary.triangle_count
            )),
            Err(error) => self.set_error(error),
        }
    }

    fn generate_cut_surface_obj(&mut self) {
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        match self.state.write_cut_surface_obj(
            object_uid,
            &self.selection_text,
            self.surface_density,
            self.surface_probe_radius,
            Vector4::new3(self.cut_normal[0], self.cut_normal[1], self.cut_normal[2]),
            Vector4::new3(
                self.cut_position[0],
                self.cut_position[1],
                self.cut_position[2],
            ),
            self.cut_surface_path.trim(),
        ) {
            Ok(summary) => self.set_status(format!(
                "Cut surface OBJ written: {} vertices / {} triangles",
                summary.vertex_count, summary.triangle_count
            )),
            Err(error) => self.set_error(error),
        }
    }

    fn write_apbs_files(&mut self) {
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        match self.state.write_apbs_files(
            object_uid,
            &self.selection_text,
            self.apbs_pqr_path.trim(),
            self.apbs_input_path.trim(),
            self.apbs_dx_path.trim(),
        ) {
            Ok(summary) => self.set_status(format!(
                "APBS files written for {} atoms: {}, {}",
                summary.atom_count, summary.pqr_path, summary.input_path
            )),
            Err(error) => self.set_error(error),
        }
    }

    fn save_config_file(&mut self) {
        match self.state.save_config_file(self.config_path.trim()) {
            Ok(()) => self.set_status(format!("Saved config {}", self.config_path.trim())),
            Err(error) => self.set_error(error),
        }
    }

    fn load_config_file(&mut self) {
        match self.state.load_config_file(self.config_path.trim()) {
            Ok(count) => self.set_status(format!("Loaded {count} config entries")),
            Err(error) => self.set_error(error),
        }
    }

    fn append_morph_frame(&mut self) {
        let Some(target) = self.morph_target else {
            self.set_status("No morph target selected");
            return;
        };
        let Some(source) = self.morph_source else {
            self.set_status("No morph source selected");
            return;
        };
        match self.state.append_morph_frame_from_object(
            target,
            source,
            self.morph_frame_name.trim(),
        ) {
            Ok(summary) => self.set_status(format!(
                "Morph frame appended: {} frames / {} atoms",
                summary.frame_count, summary.atom_count
            )),
            Err(error) => self.set_error(error),
        }
    }

    fn apply_morph_frame(&mut self) {
        let Some(target) = self.morph_target else {
            self.set_status("No morph target selected");
            return;
        };
        let undo = self.capture_undo();
        match self.state.apply_morph_frame(target, self.morph_frame_value) {
            Ok(summary) => {
                self.push_undo_snapshot(undo);
                self.selected_object = Some(target);
                self.rebuild_snapshot();
                self.set_status(format!(
                    "Morph frame applied: {} frames / {} atoms",
                    summary.frame_count, summary.atom_count
                ));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn rebuild_snapshot(&mut self) {
        match self.state.active_render_snapshot(&self.selection_text) {
            Ok(snapshot) => self.snapshot = snapshot,
            Err(error) => self.set_error(error),
        }
        // Crystal packing (Symmetry "Unit cell" / "Symm mol") via the shared core path,
        // so egui/gpui/dioxus render it identically.
        self.state.append_crystal_geometry(
            &mut self.snapshot,
            self.show_unit_cell,
            self.show_symm_mates,
        );
        // Density-map chickenwire contour (Density panel), shared too.
        self.state.append_density_geometry(
            &mut self.snapshot,
            self.density_level,
            self.show_density_contour,
        );
        // Hydrogen-bond dashed lines.
        self.state
            .append_hbond_geometry(&mut self.snapshot, self.show_hbonds);
        // Cached filled density isosurface, when shown.
        if self.show_density_solid {
            if let Some(mesh) = self.density_solid_mesh.clone() {
                self.grow_bounds_to_mesh(&mesh);
                self.snapshot.meshes.push(mesh);
            }
        }
        // Cached molecular surface (SES) mesh, when shown.
        if self.show_surface {
            if let Some(mesh) = self.surface_mesh.clone() {
                self.grow_bounds_to_mesh(&mesh);
                self.snapshot.meshes.push(mesh);
            }
        }
        if self.color_proofing {
            if self.state.apply_color_proof(&mut self.snapshot).is_err() {
                cuemol_render::apply_generic_color_proof(&mut self.snapshot);
            }
        }
    }

    /// Grow the live snapshot bounds to enclose every vertex of a mesh (so Fit frames
    /// an appended surface / isosurface).
    fn grow_bounds_to_mesh(&mut self, mesh: &cuemol_render::RenderMesh) {
        for vertex in &mesh.vertices {
            let p = vertex.position;
            self.snapshot.bounds = Some(match self.snapshot.bounds {
                Some((min, max)) => (
                    Vector4::new3(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z)),
                    Vector4::new3(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z)),
                ),
                None => (p, p),
            });
        }
    }

    /// Toggle the filled density isosurface (the solid density-surface mode). On enable,
    /// compute the marching-cubes mesh once at the current σ level and cache it.
    fn toggle_density_solid(&mut self) {
        self.show_density_solid = !self.show_density_solid;
        if self.show_density_solid && self.density_solid_mesh.is_none() {
            self.density_solid_mesh = self.state.build_density_surface_mesh(self.density_level);
            if self.density_solid_mesh.is_none() {
                self.show_density_solid = false;
                self.set_status("No density map for a solid surface");
            }
        }
        self.rebuild_snapshot();
    }

    /// Toggle the molecular-surface (SES) display. On enable, compute the SES mesh once
    /// (it is expensive) for the active molecule at the current selection and cache it;
    /// the cached mesh is added to every subsequent snapshot until toggled off.
    fn toggle_surface(&mut self) {
        self.show_surface = !self.show_surface;
        if self.show_surface && self.surface_mesh.is_none() {
            self.rebuild_surface_mesh();
        }
        self.rebuild_snapshot();
    }

    /// Load an APBS/OpenDX `.dx` electrostatic-potential map from `potential_path`.
    fn load_potential(&mut self) {
        let path = self.potential_path.trim().to_string();
        if path.is_empty() {
            self.set_status("Enter an APBS .dx path first");
            return;
        }
        match self.state.load_surface_potential(&path) {
            Ok((lo, hi)) => self.set_status(format!("Potential loaded (range {lo:.2}..{hi:.2})")),
            Err(error) => self.set_status(format!("Potential load failed: {error}")),
        }
    }

    /// Import an external OBJ/PLY surface mesh from `mesh_import_path` into the
    /// surface slot and show it via the existing surface toggle (CueMol2
    /// `ObjReader`/`PLYFileReader`). The mesh replaces any computed SES; potential
    /// colouring is turned off since an imported mesh carries no molecule to sample.
    fn import_mesh_surface(&mut self, camera: &mut CameraController) {
        let path = self.mesh_import_path.trim().to_string();
        if path.is_empty() {
            self.set_status("Enter an OBJ/PLY mesh path first");
            return;
        }
        match self.state.load_mesh_surface_file(&path) {
            Ok(mesh) => {
                let (verts, tris) = (mesh.vertices.len(), mesh.indices.len() / 3);
                self.surface_mesh = Some(mesh);
                self.show_surface = true;
                self.potential_surface = false;
                self.set_status(format!("Imported mesh: {verts} vertices, {tris} triangles"));
                self.rebuild_snapshot();
                // Frame the imported mesh (it may sit far from the molecule), matching
                // gpui's refit-on-import so all three shells behave the same.
                camera.fit_snapshot(&self.snapshot);
            }
            Err(error) => self.set_status(format!("Mesh import failed: {error}")),
        }
    }

    /// Toggle colouring the surface by electrostatic potential (SCAPOT) vs by nearest
    /// atom. Rebuilds the cached surface mesh so the new colouring takes effect.
    fn toggle_potential_surface(&mut self) {
        self.potential_surface = !self.potential_surface;
        if !self.show_surface {
            self.show_surface = true;
        }
        self.rebuild_surface_mesh();
        self.rebuild_snapshot();
    }

    /// (Re)build the cached surface mesh — clipped by the plane when `clip_surface` is
    /// set (live cross-section), else the full surface.
    fn rebuild_surface_mesh(&mut self) {
        let Some(object_uid) = self
            .molstruct_object
            .or_else(|| self.state.active_object_summaries().first().map(|o| o.uid))
        else {
            return;
        };
        let result = if self.potential_surface && self.state.has_surface_potential() {
            // SCAPOT: colour the surface by the loaded electrostatic potential.
            self.state.build_surface_potential_mesh(
                object_uid,
                &self.selection_text,
                self.surface_density,
                self.surface_probe_radius,
            )
        } else if self.clip_surface {
            let normal = cuemol_core::Vector4::new3(0.0, 0.0, 1.0);
            let position = self
                .state
                .active_render_snapshot(&self.selection_text)
                .ok()
                .and_then(|snapshot| snapshot.bounds)
                .map(|(min, max)| {
                    cuemol_core::Vector4::new3(
                        (min.x + max.x) / 2.0,
                        (min.y + max.y) / 2.0,
                        (min.z + max.z) / 2.0 + self.clip_offset,
                    )
                })
                .unwrap_or_else(|| cuemol_core::Vector4::new3(0.0, 0.0, self.clip_offset));
            self.state.build_cut_surface_mesh(
                object_uid,
                &self.selection_text,
                self.surface_density,
                self.surface_probe_radius,
                normal,
                position,
            )
        } else {
            self.state.build_surface_mesh(
                object_uid,
                &self.selection_text,
                self.surface_density,
                self.surface_probe_radius,
            )
        };
        match result {
            Ok(mesh) => {
                self.surface_mesh = Some(mesh);
                self.set_status("Surface generated");
            }
            Err(error) => {
                self.show_surface = false;
                self.set_status(format!("Surface failed: {error}"));
            }
        }
    }

    /// Toggle live in-viewport surface clipping (CueMol2 surface cutter, live mode).
    fn toggle_clip_surface(&mut self) {
        self.clip_surface = !self.clip_surface;
        if self.show_surface {
            self.rebuild_surface_mesh();
            self.rebuild_snapshot();
        }
    }

    /// Set the clip-plane offset and re-clip live if the surface is shown.
    fn set_clip_offset(&mut self, value: f64) {
        self.clip_offset = value;
        if self.clip_surface && self.show_surface {
            self.rebuild_surface_mesh();
            self.rebuild_snapshot();
        }
    }

    /// Overlay residue/atom labels on the selected atoms (CueMol2 atom labels). Each
    /// selected atom's 3D position is projected to the viewport and its
    /// `RES id : ATOM` drawn as 2D text. Capped so a huge selection doesn't clutter.
    fn draw_atom_labels(&self, ui: &egui::Ui, camera: &CameraController, rect: egui::Rect) {
        if !self.show_labels {
            return;
        }
        const MAX_LABELS: usize = 80;
        let selected: Vec<&RenderAtom> = self
            .snapshot
            .atoms
            .iter()
            .filter(|atom| atom.selected)
            .take(MAX_LABELS)
            .collect();
        if selected.is_empty() {
            return;
        }
        let painter = ui.painter();
        let color = match self.background {
            SceneBackground::White => egui::Color32::from_rgb(20, 24, 30),
            SceneBackground::Black => egui::Color32::from_rgb(235, 240, 248),
        };
        for atom in selected {
            let (Some(object_uid), Some(atom_index)) = (atom.object_uid, atom.atom_index) else {
                continue;
            };
            let Some((screen, _depth)) =
                camera.project_to_viewport(vec3_from_vector(atom.center), rect)
            else {
                continue;
            };
            let label = match self
                .state
                .atom_summary(cuemol_core::Uid::new(object_uid), atom_index)
            {
                Ok(info) => format!(
                    "{}{}:{}",
                    info.residue_name.trim(),
                    info.residue_id,
                    info.name
                ),
                Err(_) => continue,
            };
            painter.text(
                screen,
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(11.0),
                color,
            );
        }
    }

    fn draw_center_mark(&self, ui: &egui::Ui, rect: egui::Rect) {
        if self.center_mark == CenterMark::None {
            return;
        }
        let center = rect.center();
        let stroke = egui::Stroke::new(
            1.0,
            match self.background {
                SceneBackground::White => egui::Color32::from_rgb(25, 30, 36),
                SceneBackground::Black => egui::Color32::from_rgb(220, 226, 235),
            },
        );
        let painter = ui.painter();
        match self.center_mark {
            CenterMark::Cross => {
                painter.line_segment(
                    [
                        egui::pos2(center.x - 10.0, center.y),
                        egui::pos2(center.x + 10.0, center.y),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(center.x, center.y - 10.0),
                        egui::pos2(center.x, center.y + 10.0),
                    ],
                    stroke,
                );
            }
            CenterMark::Axis => {
                painter.line_segment(
                    [
                        egui::pos2(center.x - 14.0, center.y),
                        egui::pos2(center.x + 14.0, center.y),
                    ],
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 68, 68)),
                );
                painter.line_segment(
                    [
                        egui::pos2(center.x, center.y - 14.0),
                        egui::pos2(center.x, center.y + 14.0),
                    ],
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 180, 96)),
                );
            }
            CenterMark::None => {}
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        self.about_dialog(ctx);
        self.options_dialog(ctx);
        self.scene_props_dialog(ctx);
        self.view_props_dialog(ctx, camera);
        self.chain_dialog(ctx);
        self.residue_dialog(ctx);
        self.renderer_dialog(ctx);
        self.get_pdb_dialog(ctx, camera);
        self.script_dialog(ctx);
        self.console_dialog(ctx);
        self.tool_dialog(ctx);
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = self.show_about;
        let mut close = false;
        egui::Window::new("About CueMol2-rs")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.heading("CueMol2-rs");
                ui.label("Native Rust GUI");
                ui.label(format!("Menu surfaces: {}", LEGACY_MENU_SURFACES.len()));
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        self.show_about = open && !close;
    }

    fn options_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_options {
            return;
        }
        let mut open = self.show_options;
        egui::Window::new("Options")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.checkbox(&mut self.show_toolbar, "Topbar");
                ui.checkbox(&mut self.show_workspace, "Workspace panel");
                ui.checkbox(&mut self.show_inspector, "Inspector panel");
                ui.checkbox(&mut self.show_bottom_panel, "Bottom panel");
                if ui
                    .checkbox(&mut self.color_proofing, "Color proofing")
                    .changed()
                {
                    self.rebuild_snapshot();
                }
            });
        self.show_options = open;
    }

    fn scene_props_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_scene_props {
            return;
        }
        let mut open = self.show_scene_props;
        egui::Window::new("Scene Properties")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                if let Some(scene) = self.state.active_scene_summary() {
                    ui.label(format!("name: {}", scene.name));
                    ui.monospace(format!("uid: {}", scene.uid.get()));
                    ui.label(format!("objects: {}", scene.object_count));
                    ui.label(format!("cameras: {}", scene.camera_count));
                }
                ui.separator();
                ui.radio_value(&mut self.background, SceneBackground::White, "White");
                ui.radio_value(&mut self.background, SceneBackground::Black, "Black");
            });
        self.show_scene_props = open;
    }

    fn view_props_dialog(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        if !self.show_view_props {
            return;
        }
        let mut open = self.show_view_props;
        egui::Window::new("View Property")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.radio_value(&mut camera.perspective, true, "Perspective");
                ui.radio_value(&mut camera.perspective, false, "Orthographic");
                ui.separator();
                ui.radio_value(&mut self.center_mark, CenterMark::Cross, "Cross");
                ui.radio_value(&mut self.center_mark, CenterMark::Axis, "Axis");
                ui.radio_value(&mut self.center_mark, CenterMark::None, "None");
            });
        self.show_view_props = open;
    }

    fn chain_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_chain_dialog {
            return;
        }
        let mut open = self.show_chain_dialog;
        let mut apply = false;
        egui::Window::new("Change Chain ID")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut self.chain_edit_value),
                );
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_chain_dialog = false;
                    }
                });
            });
        self.show_chain_dialog = open && self.show_chain_dialog;
        if apply {
            self.change_chain_from_dialog();
        }
    }

    fn residue_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_residue_dialog {
            return;
        }
        let mut open = self.show_residue_dialog;
        let mut apply = false;
        egui::Window::new("Change Residue Number")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut self.residue_start_value),
                );
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_residue_dialog = false;
                    }
                });
            });
        self.show_residue_dialog = open && self.show_residue_dialog;
        if apply {
            self.renumber_residues_from_dialog();
        }
    }

    fn renderer_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_renderer_dialog {
            return;
        }
        let mut open = self.show_renderer_dialog;
        let mut create = false;
        egui::Window::new("New Renderer")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                if let Some(uid) = self.new_renderer_target {
                    ui.monospace(format!("object uid: {}", uid.get()));
                }
                egui::ComboBox::from_label("Type")
                    .selected_text(renderer_type_label(&self.new_renderer_type))
                    .show_ui(ui, |ui| {
                        for (renderer_type, label) in RENDERER_TYPES {
                            ui.selectable_value(
                                &mut self.new_renderer_type,
                                (*renderer_type).to_string(),
                                *label,
                            );
                        }
                    });
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut self.new_renderer_name),
                );
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        create = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_renderer_dialog = false;
                    }
                });
            });
        self.show_renderer_dialog = open && self.show_renderer_dialog;
        if create {
            self.create_renderer_from_dialog();
        }
    }

    fn tool_dialog(&mut self, ctx: &egui::Context) {
        let Some(title) = self.tool_dialog.clone() else {
            return;
        };
        let mut open = true;
        let mut close = false;
        egui::Window::new(title.as_str())
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                match title.as_str() {
                    "Molecular superposition" => self.superpose_tool_ui(ui),
                    "Mol bond editor" => self.bond_editor_tool_ui(ui),
                    "Interaction" => self.interaction_tool_ui(ui),
                    "Reassign secondary str" => self.secondary_structure_tool_ui(ui),
                    "Mol morphing animation" => self.morph_tool_ui(ui),
                    "Mol surface generation" => self.surface_tool_ui(ui),
                    "Mol surface cutter" => self.surface_cutter_tool_ui(ui),
                    "APBS elepot calculation" => self.apbs_tool_ui(ui),
                    "About plugins" => self.plugin_tool_ui(ui),
                    "About config" => self.config_tool_ui(ui),
                    "Addon manager" => self.addon_tool_ui(ui),
                    _ => {
                        ui.label(tool_status_text(&title));
                    }
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        if !open || close {
            self.tool_dialog = None;
        }
    }

    fn superpose_tool_ui(&mut self, ui: &mut egui::Ui) {
        let objects = self.state.active_object_summaries();
        if objects.len() < 2 {
            ui.label("At least two objects are required.");
            return;
        }
        if self
            .superpose_reference
            .is_none_or(|uid| !objects.iter().any(|object| object.uid == uid))
        {
            self.superpose_reference = Some(objects[0].uid);
        }
        if self
            .superpose_moving
            .is_none_or(|uid| !objects.iter().any(|object| object.uid == uid))
        {
            self.superpose_moving = Some(objects[1].uid);
        }
        egui::ComboBox::from_label("Reference")
            .selected_text(object_combo_label(&objects, self.superpose_reference))
            .show_ui(ui, |ui| {
                for object in &objects {
                    ui.selectable_value(
                        &mut self.superpose_reference,
                        Some(object.uid),
                        object.name.as_str(),
                    );
                }
            });
        egui::ComboBox::from_label("Moving")
            .selected_text(object_combo_label(&objects, self.superpose_moving))
            .show_ui(ui, |ui| {
                for object in &objects {
                    ui.selectable_value(
                        &mut self.superpose_moving,
                        Some(object.uid),
                        object.name.as_str(),
                    );
                }
            });
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.superpose_reference_selection)
                .hint_text("reference selection"),
        );
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.superpose_moving_selection)
                .hint_text("moving selection"),
        );
        if ui.button("Superpose").clicked() {
            self.apply_superpose();
        }
    }

    fn bond_editor_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Atom serials");
            ui.add_sized(
                [80.0, 22.0],
                egui::TextEdit::singleline(&mut self.bond_aid1),
            );
            ui.add_sized(
                [80.0, 22.0],
                egui::TextEdit::singleline(&mut self.bond_aid2),
            );
        });
        ui.horizontal(|ui| {
            if ui.button("Make Bond").clicked() {
                self.edit_bond(true);
            }
            if ui.button("Remove Bond").clicked() {
                self.edit_bond(false);
            }
        });
    }

    fn interaction_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Distance");
            ui.add(egui::DragValue::new(&mut self.contact_r_min).speed(0.1));
            ui.add(egui::DragValue::new(&mut self.contact_r_max).speed(0.1));
            ui.checkbox(&mut self.contact_hbond, "H-bond");
        });
        if ui.button("Calculate Contacts").clicked() {
            self.calculate_contacts();
        }
    }

    fn secondary_structure_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.secondary_ignore_breaks, "Ignore chain breaks");
        ui.horizontal(|ui| {
            ui.label("Helix angle");
            ui.add(egui::DragValue::new(&mut self.secondary_angle).speed(1.0));
        });
        if ui.button("Assign").clicked() {
            self.assign_secondary_structure();
        }
    }

    fn surface_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.surface_path),
        );
        ui.horizontal(|ui| {
            ui.label("Density");
            ui.add(egui::DragValue::new(&mut self.surface_density).speed(0.1));
            ui.label("Probe");
            ui.add(egui::DragValue::new(&mut self.surface_probe_radius).speed(0.1));
        });
        if ui.button("Generate OBJ").clicked() {
            self.generate_surface_obj();
        }
    }

    fn surface_cutter_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.cut_surface_path),
        );
        ui.label("Normal");
        ui.horizontal(|ui| {
            for value in &mut self.cut_normal {
                ui.add(egui::DragValue::new(value).speed(0.1));
            }
        });
        ui.label("Position");
        ui.horizontal(|ui| {
            for value in &mut self.cut_position {
                ui.add(egui::DragValue::new(value).speed(0.1));
            }
        });
        if ui.button("Cut Surface OBJ").clicked() {
            self.generate_cut_surface_obj();
        }
    }

    fn apbs_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("PQR");
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.apbs_pqr_path),
        );
        ui.label("Input");
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.apbs_input_path),
        );
        ui.label("DX output");
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.apbs_dx_path),
        );
        if ui.button("Write APBS Files").clicked() {
            self.write_apbs_files();
        }
    }

    fn morph_tool_ui(&mut self, ui: &mut egui::Ui) {
        let objects = self.state.active_object_summaries();
        if objects.is_empty() {
            ui.label("No molecule object loaded.");
            return;
        }
        if self
            .morph_target
            .is_none_or(|uid| !objects.iter().any(|object| object.uid == uid))
        {
            self.morph_target = self.selected_object.or(Some(objects[0].uid));
        }
        if self
            .morph_source
            .is_none_or(|uid| !objects.iter().any(|object| object.uid == uid))
        {
            self.morph_source = Some(objects[0].uid);
        }
        egui::ComboBox::from_label("Target")
            .selected_text(object_combo_label(&objects, self.morph_target))
            .show_ui(ui, |ui| {
                for object in &objects {
                    ui.selectable_value(
                        &mut self.morph_target,
                        Some(object.uid),
                        object.name.as_str(),
                    );
                }
            });
        egui::ComboBox::from_label("Source frame")
            .selected_text(object_combo_label(&objects, self.morph_source))
            .show_ui(ui, |ui| {
                for object in &objects {
                    ui.selectable_value(
                        &mut self.morph_source,
                        Some(object.uid),
                        object.name.as_str(),
                    );
                }
            });
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.morph_frame_name),
        );
        if ui.button("Append Frame").clicked() {
            self.append_morph_frame();
        }
        ui.horizontal(|ui| {
            ui.label("Frame");
            ui.add(egui::DragValue::new(&mut self.morph_frame_value).speed(0.05));
            if ui.button("Apply").clicked() {
                self.apply_morph_frame();
            }
        });
    }

    fn config_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Config file");
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.config_path),
        );
        ui.horizontal(|ui| {
            if ui.button("Load").clicked() {
                self.load_config_file();
            }
            if ui.button("Save").clicked() {
                self.save_config_file();
            }
        });
        ui.separator();
        ui.label("Entry");
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.config_key),
        );
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.config_value),
        );
        ui.horizontal(|ui| {
            if ui.button("Get").clicked() {
                self.config_value = self.state.config_value(self.config_key.trim());
                self.set_status("Config value loaded");
            }
            if ui.button("Set").clicked() {
                self.state
                    .put_config_value(self.config_key.trim(), self.config_value.clone());
                self.set_status("Config value stored");
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(180.0)
            .show(ui, |ui| {
                for entry in self.state.config_entries() {
                    ui.monospace(format!("{} = {}", entry.key, entry.value));
                }
            });
    }

    fn plugin_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Plugin directory");
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.plugin_dir),
        );
        self.directory_listing_ui(ui, self.plugin_dir.trim().to_string());
    }

    fn addon_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Addon directory");
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(&mut self.addon_dir),
        );
        self.directory_listing_ui(ui, self.addon_dir.trim().to_string());
    }

    fn directory_listing_ui(&mut self, ui: &mut egui::Ui, directory: String) {
        if ui.button("Refresh").clicked() {
            self.set_status(format!("Scanned {directory}"));
        }
        ui.separator();
        match list_directory_entries(&directory) {
            Ok(entries) if entries.is_empty() => {
                ui.label("No entries found.");
            }
            Ok(entries) => {
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for entry in entries {
                            ui.monospace(entry);
                        }
                    });
            }
            Err(error) => {
                ui.label(format!("Error: {error}"));
            }
        }
    }

    fn get_pdb_dialog(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        if !self.show_get_pdb {
            return;
        }
        let mut open = self.show_get_pdb;
        egui::Window::new("Get PDB")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut self.pdb_id),
                );
                ui.label(format!(
                    "https://files.rcsb.org/download/{}.pdb",
                    self.pdb_id.trim().to_ascii_uppercase()
                ));
                if ui.button("Use as Open Path").clicked() {
                    self.open_path = format!(
                        "https://files.rcsb.org/download/{}.pdb",
                        self.pdb_id.trim().to_ascii_uppercase()
                    );
                    self.set_status("URL copied to Open path");
                }
                if ui.button("Download and Open").clicked() {
                    self.download_pdb(camera);
                }
            });
        self.show_get_pdb = open;
    }

    fn script_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_script_dialog {
            return;
        }
        let mut open = self.show_script_dialog;
        egui::Window::new("Execute Script")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut self.script_path),
                );
                if ui.button("Load Script Into Scene").clicked() {
                    self.load_script_into_log();
                }
            });
        self.show_script_dialog = open;
    }

    fn console_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_console {
            return;
        }
        let mut open = self.show_console;
        egui::Window::new("Console")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.add_sized(
                    [ui.available_width(), 72.0],
                    egui::TextEdit::multiline(&mut self.console_input)
                        .font(egui::TextStyle::Monospace),
                );
                if ui.button("Append To Log").clicked() {
                    let input = self.console_input.trim().to_string();
                    if !input.is_empty() {
                        self.set_status(format!("Console: {input}"));
                        self.console_input.clear();
                    }
                }
            });
        self.show_console = open;
    }

    fn clear_log(&mut self) {
        self.log_lines.clear();
        self.set_status("Log cleared");
    }

    fn capture_undo(&self) -> GuiUndoSnapshot {
        GuiUndoSnapshot {
            state: self.state.clone(),
            selected_object: self.selected_object,
            selected_renderer: self.selected_renderer,
            selection_text: self.selection_text.clone(),
            last_loaded_path: self.last_loaded_path.clone(),
        }
    }

    fn push_undo_snapshot(&mut self, snapshot: GuiUndoSnapshot) {
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn restore_undo_snapshot(&mut self, snapshot: GuiUndoSnapshot, camera: &mut CameraController) {
        self.state = snapshot.state;
        self.selected_object = snapshot.selected_object;
        self.selected_renderer = snapshot.selected_renderer;
        self.selection_text = snapshot.selection_text;
        self.last_loaded_path = snapshot.last_loaded_path;
        self.rebuild_snapshot();
        camera.fit_snapshot(&self.snapshot);
    }

    fn undo(&mut self, camera: &mut CameraController) {
        let Some(snapshot) = self.undo_stack.pop() else {
            self.set_status("Nothing to undo");
            return;
        };
        let redo = self.capture_undo();
        self.restore_undo_snapshot(snapshot, camera);
        self.redo_stack.push(redo);
        self.set_status("Undo");
    }

    fn redo(&mut self, camera: &mut CameraController) {
        let Some(snapshot) = self.redo_stack.pop() else {
            self.set_status("Nothing to redo");
            return;
        };
        let undo = self.capture_undo();
        self.restore_undo_snapshot(snapshot, camera);
        self.undo_stack.push(undo);
        self.set_status("Redo");
    }

    fn clear_undo_data(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.set_status("Undo data cleared");
    }

    fn add_recent_path(&mut self, path: PathBuf) {
        self.recent_paths.retain(|existing| existing != &path);
        self.recent_paths.insert(0, path);
        self.recent_paths.truncate(10);
    }

    fn clear_recent_paths(&mut self) {
        self.recent_paths.clear();
        self.set_status("Recent files cleared");
    }

    fn restore_panels(&mut self) {
        self.show_toolbar = true;
        self.show_workspace = true;
        self.show_inspector = true;
        self.show_bottom_panel = true;
        self.bottom_tab = BottomTab::Output;
        self.set_status("Panel layout restored");
    }

    fn open_renderer_dialog(&mut self, object_uid: cuemol_core::Uid, next_index: usize) {
        self.new_renderer_target = Some(object_uid);
        if self.new_renderer_type.trim().is_empty() {
            self.new_renderer_type = "simple".to_string();
        }
        self.new_renderer_name = format!("{}{}", self.new_renderer_type.trim(), next_index);
        self.show_renderer_dialog = true;
    }

    fn create_renderer_from_dialog(&mut self) {
        let Some(object_uid) = self.new_renderer_target else {
            self.set_status("No target object selected");
            return;
        };
        let renderer_type = self.new_renderer_type.trim().to_string();
        let renderer_name = self.new_renderer_name.trim().to_string();
        if renderer_type.is_empty() || renderer_name.is_empty() {
            self.set_status("Renderer type and name are required");
            return;
        }
        let undo = self.capture_undo();
        match self
            .state
            .new_renderer_in_active_scene(object_uid, renderer_type, renderer_name)
        {
            Ok(result) => {
                self.push_undo_snapshot(undo);
                self.selected_object = Some(object_uid);
                self.selected_renderer = Some(result.result_renderer);
                self.show_renderer_dialog = false;
                self.rebuild_snapshot();
                self.set_status("Renderer added");
            }
            Err(error) => self.set_error(error),
        }
    }

    fn set_renderer_visibility(
        &mut self,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        visible: bool,
    ) {
        let undo = self.capture_undo();
        match self
            .state
            .set_renderer_visible(object_uid, renderer_uid, visible)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.rebuild_snapshot();
                self.set_status(format!("Renderer visibility: {visible}"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn delete_renderer(&mut self, object_uid: cuemol_core::Uid, renderer_uid: cuemol_core::Uid) {
        let undo = self.capture_undo();
        match self
            .state
            .destroy_renderer_in_object(object_uid, renderer_uid)
        {
            Ok(true) => {
                self.push_undo_snapshot(undo);
                if self.selected_renderer == Some(renderer_uid) {
                    self.selected_renderer = None;
                }
                self.rebuild_snapshot();
                self.set_status("Renderer deleted");
            }
            Ok(false) => self.set_status("Renderer was already absent"),
            Err(error) => self.set_error(error),
        }
    }

    fn target_object_uid(&self) -> Option<cuemol_core::Uid> {
        self.selected_object.or_else(|| {
            self.state
                .active_object_summaries()
                .first()
                .map(|object| object.uid)
        })
    }

    fn require_selection_for_edit(&mut self) -> bool {
        if self.selection_text.trim().is_empty() {
            self.set_status("Selection is required");
            return false;
        }
        true
    }

    fn delete_selected_atoms(&mut self) {
        if !self.require_selection_for_edit() {
            return;
        }
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        let undo = self.capture_undo();
        match self
            .state
            .delete_atoms_in_object(object_uid, &self.selection_text)
        {
            Ok(count) => {
                self.push_undo_snapshot(undo);
                self.rebuild_snapshot();
                self.set_status(format!("Deleted {count} atoms"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn change_chain_from_dialog(&mut self) {
        if !self.require_selection_for_edit() {
            return;
        }
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        let undo = self.capture_undo();
        match self.state.change_chain_in_object(
            object_uid,
            &self.selection_text,
            &self.chain_edit_value,
        ) {
            Ok(count) => {
                self.push_undo_snapshot(undo);
                self.show_chain_dialog = false;
                self.rebuild_snapshot();
                self.set_status(format!("Changed chain ID on {count} atoms"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn renumber_residues_from_dialog(&mut self) {
        if !self.require_selection_for_edit() {
            return;
        }
        let Some(object_uid) = self.target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        let start = match self.residue_start_value.trim().parse::<i32>() {
            Ok(value) => value,
            Err(_) => {
                self.set_status("Invalid residue number");
                return;
            }
        };
        let undo = self.capture_undo();
        match self
            .state
            .renumber_residues_in_object(object_uid, &self.selection_text, start)
        {
            Ok(count) => {
                self.push_undo_snapshot(undo);
                self.show_residue_dialog = false;
                self.rebuild_snapshot();
                self.set_status(format!("Renumbered residues for {count} atoms"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn handle_window_event(&mut self, event: &WindowEvent, camera: &mut CameraController) -> bool {
        match event {
            WindowEvent::ModifiersChanged(modifiers) => {
                self.shift_down = modifiers.state().shift_key();
                false
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed if self.pointer_in_viewport(camera.last_cursor) => {
                    // Shift+left = zoom drag (CueMol2 default view-input preset).
                    camera.drag_mode = if *button == MouseButton::Left && self.shift_down {
                        Some(CameraDragMode::Zoom)
                    } else {
                        CameraDragMode::from_button(*button)
                    };
                    camera.last_drag_cursor = camera.last_cursor;
                    camera.drag_total = 0.0;
                    self.viewport_menu = None;
                    camera.drag_mode.is_some()
                }
                ElementState::Released => {
                    let was_dragging = camera.drag_mode.is_some();
                    let in_viewport = self.pointer_in_viewport(camera.last_cursor);
                    let is_click = camera.drag_total < 4.0 && in_viewport;
                    let should_pick = *button == MouseButton::Left
                        && camera.drag_mode == Some(CameraDragMode::Rotate)
                        && is_click;
                    if should_pick {
                        if let Some(position) = camera.last_cursor {
                            self.pick_atom_at(camera, position);
                        }
                    }
                    // Right-click (no drag) opens the atom context menu
                    // (CueMol2 `navi-ctxtmenu-atom`).
                    let should_menu = *button == MouseButton::Right
                        && camera.drag_mode == Some(CameraDragMode::Pan)
                        && is_click;
                    if should_menu {
                        if let Some(position) = camera.last_cursor {
                            let hit = self.pick_hit(camera, position);
                            self.viewport_menu = Some(ViewportMenu { pos: position, hit });
                        }
                    }
                    camera.drag_mode = None;
                    camera.last_drag_cursor = None;
                    camera.drag_total = 0.0;
                    was_dragging
                }
                _ => false,
            },
            WindowEvent::CursorLeft { .. } => {
                camera.last_cursor = None;
                camera.last_drag_cursor = None;
                camera.drag_mode = None;
                camera.drag_total = 0.0;
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = self.cursor_position_points(position.x, position.y);
                let mut changed = false;
                if let Some(mode) = camera.drag_mode {
                    if let Some(last) = camera.last_drag_cursor {
                        let dx = pos.x - last.x;
                        let dy = pos.y - last.y;
                        match mode {
                            CameraDragMode::Rotate => match self.viewport_rect {
                                Some(rect) => {
                                    let width = rect.width().max(1.0);
                                    let height = rect.height().max(1.0);
                                    camera.trackball_move(
                                        ((pos.x - rect.left()) / width) as f64,
                                        ((pos.y - rect.top()) / height) as f64,
                                        ((last.x - rect.left()) / width) as f64,
                                        ((last.y - rect.top()) / height) as f64,
                                    );
                                }
                                None => camera.rotate_by(dx, dy),
                            },
                            CameraDragMode::Pan => camera.pan_by(dx, dy),
                            // Drag up to zoom in, down to zoom out.
                            CameraDragMode::Zoom => camera.zoom_by(-dy * 0.1),
                        }
                        camera.drag_total += dx.abs() + dy.abs();
                        changed = dx != 0.0 || dy != 0.0;
                    }
                    camera.last_drag_cursor = Some(pos);
                }
                camera.last_cursor = Some(pos);
                changed
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !self.pointer_in_viewport(camera.last_cursor) {
                    return false;
                }
                let step = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 48.0,
                };
                camera.zoom_by(step);
                step != 0.0
            }
            _ => false,
        }
    }

    fn cursor_position_points(&self, x: f64, y: f64) -> egui::Pos2 {
        let scale = self.pixels_per_point.max(0.1) as f64;
        egui::pos2((x / scale) as f32, (y / scale) as f32)
    }

    fn pointer_in_viewport(&self, position: Option<egui::Pos2>) -> bool {
        match (self.viewport_rect, position) {
            (Some(rect), Some(position)) => rect.contains(position),
            (None, _) => true,
            _ => false,
        }
    }

    fn pick_atom_at(&mut self, camera: &CameraController, position: egui::Pos2) -> bool {
        let Some(hit) = self.pick_hit(camera, position) else {
            return false;
        };
        let object_uid = cuemol_core::Uid::new(hit.object_uid);
        match self.state.atom_summary(object_uid, hit.atom_index) {
            Ok(atom) => {
                self.selected_object = Some(atom.object_uid);
                self.selected_renderer = None;
                self.selection_text = atom.selection_text;
                self.rebuild_snapshot();
                self.set_status(format!(
                    "Selected atom {} {}{}:{}",
                    atom.serial, atom.residue_name, atom.residue_id, atom.name
                ));
                true
            }
            Err(error) => {
                self.set_error(error);
                false
            }
        }
    }

    fn pick_hit(&self, camera: &CameraController, position: egui::Pos2) -> Option<PickHit> {
        let viewport = self.viewport_rect?;
        let mut best: Option<(f32, PickHit)> = None;
        for atom in &self.snapshot.atoms {
            let object_uid = atom.object_uid?;
            let atom_index = atom.atom_index?;
            let center = vec3_from_vector(atom.center);
            let (screen, depth) = camera.project_to_viewport(center, viewport)?;
            let radius = camera
                .projected_radius(atom.radius, depth, viewport)
                .max(7.0);
            let distance = screen.distance(position);
            if distance > radius {
                continue;
            }
            let score = distance + depth * 0.001;
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score < *best_score)
            {
                best = Some((
                    score,
                    PickHit {
                        object_uid,
                        atom_index,
                    },
                ));
            }
        }
        best.map(|(_, hit)| hit)
    }

    fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.log_lines.push(self.status.clone());
        if self.log_lines.len() > 200 {
            self.log_lines.remove(0);
        }
    }

    fn set_error(&mut self, error: CueMolError) {
        self.set_status(format!("Error: {error}"));
    }
}

#[derive(Debug, Clone)]
struct CameraController {
    target: Vec3,
    /// Accumulated trackball orientation (object-to-camera rotation).
    ///
    /// Replaces the previous yaw/pitch Euler angles, which suffered from gimbal
    /// behavior and a clamped pitch. Mirrors the original CueMol2 `View::m_rotQuat`
    /// accumulation in `qsys/View.cpp`.
    orientation: Quat,
    distance: f32,
    ortho_scale: f32,
    perspective: bool,
    drag_mode: Option<CameraDragMode>,
    last_cursor: Option<egui::Pos2>,
    last_drag_cursor: Option<egui::Pos2>,
    drag_total: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PickHit {
    object_uid: u64,
    atom_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CameraDragMode {
    Rotate,
    Pan,
    /// Shift+left drag = zoom (CueMol2 default view-input preset).
    Zoom,
}

impl CameraDragMode {
    fn from_button(button: MouseButton) -> Option<Self> {
        match button {
            MouseButton::Left => Some(Self::Rotate),
            MouseButton::Right | MouseButton::Middle => Some(Self::Pan),
            _ => None,
        }
    }
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            orientation: Quat::IDENTITY,
            distance: 120.0,
            ortho_scale: 40.0,
            perspective: true,
            drag_mode: None,
            last_cursor: None,
            last_drag_cursor: None,
            drag_total: 0.0,
        }
    }
}

impl CameraController {
    fn fit_snapshot(&mut self, snapshot: &RenderSceneSnapshot) {
        let Some((min, max)) = snapshot.bounds else {
            return;
        };
        let min = vec3_from_vector(min);
        let max = vec3_from_vector(max);
        let center = (min + max) * 0.5;
        let radius = (max - min).length().max(1.0) * 0.5;
        self.target = center;
        self.distance = (radius * 3.2).max(8.0);
        self.ortho_scale = (radius * 1.25).max(4.0);
    }

    fn rotate_by(&mut self, dx: f32, dy: f32) {
        // Virtual trackball: build an incremental rotation about the camera-local
        // axes (horizontal drag -> up axis, vertical drag -> right axis) and
        // accumulate it into `orientation`. Because the axis is expressed in the
        // current camera frame, the gesture stays consistent at any orientation,
        // with no gimbal lock and no clamped pitch.
        let axis_local = Vec3::new(dy, dx, 0.0);
        let len = axis_local.length();
        if len < 1.0e-6 {
            return;
        }
        let angle = len * 0.01;
        let axis_world = self.orientation * (axis_local / len);
        let delta = Quat::from_axis_angle(axis_world, angle);
        self.orientation = (delta * self.orientation).normalize();
    }

    /// Trackball rotation from the previous to the current viewport-normalized
    /// cursor position (both components in `[0, 1]`).
    ///
    /// Ports `View::trackBallMove` / `getTrackRotQuat`: project both endpoints
    /// onto the virtual sphere, take the rotation mapping one to the other, and
    /// post-multiply it onto the accumulated orientation — matching the original
    /// `m_curcam.m_rotQuat.mulSelf(dqrot)`. Unlike a pure incremental rotation,
    /// gestures near the viewport edge naturally introduce roll about the view
    /// axis, reproducing the original "grab and twist" feel.
    fn trackball_move(&mut self, cur_x: f64, cur_y: f64, prev_x: f64, prev_y: f64) {
        const TB_RAD: f64 = 0.8;
        let x1 = 2.0 * prev_x - 1.0;
        let y1 = 1.0 - 2.0 * prev_y;
        let x2 = 2.0 * cur_x - 1.0;
        let y2 = 1.0 - 2.0 * cur_y;
        let p1 = project_trackball(x1, y1, TB_RAD);
        let p2 = project_trackball(x2, y2, TB_RAD);
        let axis = p2.cross(p1);
        let axis_len = axis.length();
        if !axis_len.is_finite() || axis_len < 1.0e-7 {
            return;
        }
        let axis = axis / axis_len;
        let chord = (p1 - p2).length() as f64;
        let t = (chord / (2.0 * TB_RAD)).clamp(-1.0, 1.0);
        let phi = 2.0 * t.asin();
        if !phi.is_finite() || phi.abs() < 1.0e-5 {
            return;
        }
        let dqrot = Quat::from_axis_angle(axis, phi as f32);
        self.orientation = (self.orientation * dqrot).normalize();
    }

    fn pan_by(&mut self, dx: f32, dy: f32) {
        let rotation = self.rotation();
        let right = rotation * Vec3::X;
        let up = rotation * Vec3::Y;
        let scale = if self.perspective {
            self.distance * 0.0018
        } else {
            self.ortho_scale * 0.003
        };
        self.target -= right * dx * scale;
        self.target += up * dy * scale;
    }

    fn zoom_by(&mut self, step: f32) {
        let factor = (1.0 - step * 0.08).clamp(0.05, 20.0);
        self.distance = (self.distance * factor).clamp(1.0, 10000.0);
        self.ortho_scale = (self.ortho_scale * factor).clamp(0.2, 10000.0);
    }

    fn rotation(&self) -> Quat {
        self.orientation
    }

    fn eye(&self) -> Vec3 {
        self.target + self.rotation() * Vec3::new(0.0, 0.0, self.distance)
    }

    fn view_projection(&self, aspect: f32) -> Mat4 {
        let rotation = self.rotation();
        let up = rotation * Vec3::Y;
        let view = Mat4::look_at_rh(self.eye(), self.target, up);
        let proj = if self.perspective {
            Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 20000.0)
        } else {
            Mat4::orthographic_rh(
                -self.ortho_scale * aspect,
                self.ortho_scale * aspect,
                -self.ortho_scale,
                self.ortho_scale,
                -20000.0,
                20000.0,
            )
        };
        proj * view
    }

    fn project_to_viewport(&self, point: Vec3, viewport: egui::Rect) -> Option<(egui::Pos2, f32)> {
        let aspect = (viewport.width() / viewport.height().max(1.0)).max(0.01);
        let clip = self.view_projection(aspect) * GlamVec4::new(point.x, point.y, point.z, 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.x.abs() > 1.2 || ndc.y.abs() > 1.2 {
            return None;
        }
        let x = viewport.left() + (ndc.x + 1.0) * 0.5 * viewport.width();
        let y = viewport.top() + (1.0 - ndc.y) * 0.5 * viewport.height();
        Some((egui::pos2(x, y), (self.eye() - point).length()))
    }

    fn projected_radius(&self, radius: f32, depth: f32, viewport: egui::Rect) -> f32 {
        if self.perspective {
            let focal = 0.5 * viewport.height() / (45.0_f32.to_radians() * 0.5).tan();
            (radius * focal / depth.max(0.1)).clamp(2.0, 48.0)
        } else {
            (radius * viewport.height() / (self.ortho_scale * 2.0).max(0.1)).clamp(2.0, 48.0)
        }
    }

    fn uniform(&self, size: PhysicalSize<u32>) -> CameraUniform {
        let width = size.width.max(1) as f32;
        let height = size.height.max(1) as f32;
        let aspect = width / height;
        let rotation = self.rotation();
        let right = rotation * Vec3::X;
        let up = rotation * Vec3::Y;
        let view_proj = self.view_projection(aspect);
        CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            right: [right.x, right.y, right.z, 0.0],
            up: [up.x, up.y, up.z, 0.0],
            light: [0.25, 0.45, 0.85, 0.0],
        }
    }

    fn to_view_data(&self) -> String {
        format!(
            "cuemol2-rs-view\t2\n\
target\t{:.6}\t{:.6}\t{:.6}\n\
rotation\t{:.6}\t{:.6}\t{:.6}\t{:.6}\n\
distance\t{:.6}\n\
ortho_scale\t{:.6}\n\
perspective\t{}\n",
            self.target.x,
            self.target.y,
            self.target.z,
            self.orientation.x,
            self.orientation.y,
            self.orientation.z,
            self.orientation.w,
            self.distance,
            self.ortho_scale,
            self.perspective
        )
    }

    fn to_qsys_camera(&self, name: &str) -> cuemol_qsys::Camera {
        let rotation = self.rotation();
        let mut camera = cuemol_qsys::Camera::new();
        camera.set_name(name);
        camera.set_center(Vector4::new3(
            self.target.x as f64,
            self.target.y as f64,
            self.target.z as f64,
        ));
        camera.set_cam_dist(self.distance as f64);
        camera.set_slab_depth((self.distance * 2.0).max(1.0) as f64);
        camera.set_zoom((1000.0 / self.ortho_scale.max(0.1)) as f64);
        camera.set_perspective(self.perspective);
        camera.set_rotation(cuemol_core::Quat::new(
            rotation.x as f64,
            rotation.y as f64,
            rotation.z as f64,
            rotation.w as f64,
        ));
        camera
    }

    /// Build a controller from a saved `cuemol_qsys::Camera` (a `.qsc` view) —
    /// the exact inverse of [`to_qsys_camera`], so a save → load round-trips. The
    /// drag/transient fields reset to defaults. (`ortho_scale` inverts the
    /// `zoom = 1000 / ortho_scale` mapping; the orientation quaternion maps
    /// component-wise — `q` and `-q` denote the same rotation.)
    fn from_qsys_camera(camera: &cuemol_qsys::Camera) -> Self {
        let q = camera.rotation();
        let zoom = camera.zoom();
        let defaults = Self::default();
        let ortho_scale = if zoom.abs() > 1.0e-9 {
            (1000.0 / zoom) as f32
        } else {
            defaults.ortho_scale
        };
        let center = camera.center();
        Self {
            target: Vec3::new(center.x as f32, center.y as f32, center.z as f32),
            orientation: Quat::from_xyzw(q.x as f32, q.y as f32, q.z as f32, q.w as f32)
                .normalize(),
            distance: camera.cam_dist() as f32,
            ortho_scale,
            perspective: camera.is_perspective(),
            ..defaults
        }
    }

    fn from_view_data(data: &str) -> Result<Self> {
        let mut camera = Self::default();
        let mut saw_header = false;
        // Legacy (v1) files stored Euler yaw/pitch; collect and convert on close.
        let mut legacy_yaw: Option<f32> = None;
        let mut legacy_pitch: Option<f32> = None;
        for line in data.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parts = trimmed.split_whitespace().collect::<Vec<_>>();
            match parts.as_slice() {
                ["cuemol2-rs-view", "1" | "2"] => saw_header = true,
                ["target", x, y, z] => {
                    camera.target = Vec3::new(parse_f32(x)?, parse_f32(y)?, parse_f32(z)?);
                }
                ["rotation", x, y, z, w] => {
                    let quat =
                        Quat::from_xyzw(parse_f32(x)?, parse_f32(y)?, parse_f32(z)?, parse_f32(w)?);
                    camera.orientation = normalized_or_identity(quat);
                }
                ["yaw", value] => legacy_yaw = Some(parse_f32(value)?),
                ["pitch", value] => legacy_pitch = Some(parse_f32(value)?.clamp(-1.45, 1.45)),
                ["distance", value] => camera.distance = parse_f32(value)?.clamp(1.0, 10000.0),
                ["ortho_scale", value] => {
                    camera.ortho_scale = parse_f32(value)?.clamp(0.2, 10000.0);
                }
                ["perspective", value] => {
                    camera.perspective = match *value {
                        "true" => true,
                        "false" => false,
                        _ => return Err(CueMolError::parse("invalid perspective value")),
                    };
                }
                _ => return Err(CueMolError::parse(format!("invalid view line: {trimmed}"))),
            }
        }
        if !saw_header {
            return Err(CueMolError::parse("missing view header"));
        }
        if legacy_yaw.is_some() || legacy_pitch.is_some() {
            let yaw = legacy_yaw.unwrap_or(0.0);
            let pitch = legacy_pitch.unwrap_or(0.0);
            camera.orientation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
        }
        Ok(camera)
    }
}

fn tool_status_text(_title: &str) -> &'static str {
    "No additional action is available."
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    depth_view: wgpu::TextureView,
    atom_pipeline: wgpu::RenderPipeline,
    bond_pipeline: wgpu::RenderPipeline,
    camera: CameraController,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    atom_quad_buffer: wgpu::Buffer,
    atom_instance_buffer: wgpu::Buffer,
    atom_instance_count: u32,
    bond_vertex_buffer: wgpu::Buffer,
    bond_vertex_count: u32,
    mesh_pipeline: wgpu::RenderPipeline,
    mesh_vertex_buffer: wgpu::Buffer,
    mesh_index_buffer: wgpu::Buffer,
    mesh_index_count: u32,
    egui_renderer: egui_wgpu::Renderer,
}

impl GpuState {
    async fn new(window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window).map_err(gui_error)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(gui_error)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("cuemol-desktop-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .map_err(gui_error)?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);
        let present_mode = caps
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .unwrap_or(caps.present_modes[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: Vec::new(),
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth_view = create_depth_view(&device, config.width, config.height);
        let camera = CameraController::default();
        let camera_uniform = camera.uniform(size);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera-uniform-buffer"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-bind-group-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("molecule-shader"),
            source: wgpu::ShaderSource::Wgsl(MOLECULE_SHADER.into()),
        });
        let atom_pipeline = create_atom_pipeline(&device, &shader, &camera_layout, config.format);
        let bond_pipeline = create_bond_pipeline(&device, &shader, &camera_layout, config.format);
        let atom_quad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("atom-quad-buffer"),
            contents: bytemuck::cast_slice(&ATOM_QUAD),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let atom_instance_buffer = empty_buffer(&device, "atom-instance-buffer");
        let bond_vertex_buffer = empty_buffer(&device, "bond-vertex-buffer");
        let mesh_pipeline = create_mesh_pipeline(&device, &shader, &camera_layout, config.format);
        let mesh_vertex_buffer = empty_buffer(&device, "mesh-vertex-buffer");
        let mesh_index_buffer = empty_index_buffer(&device, "mesh-index-buffer");
        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            depth_view,
            atom_pipeline,
            bond_pipeline,
            camera,
            camera_buffer,
            camera_bind_group,
            atom_quad_buffer,
            atom_instance_buffer,
            atom_instance_count: 0,
            bond_vertex_buffer,
            bond_vertex_count: 0,
            mesh_pipeline,
            mesh_vertex_buffer,
            mesh_index_buffer,
            mesh_index_count: 0,
            egui_renderer,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_view(&self.device, size.width, size.height);
    }

    fn render(
        &mut self,
        snapshot: &RenderSceneSnapshot,
        background: wgpu::Color,
        paint_jobs: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
        pixels_per_point: f32,
        viewport: Option<egui::Rect>,
    ) -> std::result::Result<(), wgpu::SurfaceError> {
        self.prepare_snapshot(snapshot);
        // Confine the 3D scene to the central viewport region (everything but the
        // docked panels), in physical pixels, so the molecule is centred in the
        // visible area and its on-screen position matches the picking projection
        // (which also uses `viewport_rect`). Without this the scene is centred on
        // the whole window and appears shifted once the left dock takes space.
        let (vx, vy, vw, vh) = match viewport {
            Some(rect) => {
                let x = (rect.left() * pixels_per_point).max(0.0);
                let y = (rect.top() * pixels_per_point).max(0.0);
                let w = (rect.width() * pixels_per_point)
                    .min(self.config.width as f32 - x)
                    .max(1.0);
                let h = (rect.height() * pixels_per_point)
                    .min(self.config.height as f32 - y)
                    .max(1.0);
                (x, y, w, h)
            }
            None => (
                0.0,
                0.0,
                self.config.width as f32,
                self.config.height as f32,
            ),
        };
        let camera_uniform = self.camera.uniform(PhysicalSize::new(vw as u32, vh as u32));
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));
        for (id, image_delta) in &textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cuemol-render-encoder"),
            });
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };
        let mut command_buffers = self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            paint_jobs,
            &screen_descriptor,
        );
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("molecule-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(background),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_viewport(vx, vy, vw, vh, 0.0, 1.0);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            if self.mesh_index_count > 0 {
                render_pass.set_pipeline(&self.mesh_pipeline);
                render_pass.set_vertex_buffer(0, self.mesh_vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(self.mesh_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..self.mesh_index_count, 0, 0..1);
            }
            if self.bond_vertex_count > 0 {
                render_pass.set_pipeline(&self.bond_pipeline);
                render_pass.set_vertex_buffer(0, self.bond_vertex_buffer.slice(..));
                render_pass.draw(0..self.bond_vertex_count, 0..1);
            }
            if self.atom_instance_count > 0 {
                render_pass.set_pipeline(&self.atom_pipeline);
                render_pass.set_vertex_buffer(0, self.atom_quad_buffer.slice(..));
                render_pass.set_vertex_buffer(1, self.atom_instance_buffer.slice(..));
                render_pass.draw(0..6, 0..self.atom_instance_count);
            }
        }
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.egui_renderer.render(
                &mut render_pass.forget_lifetime(),
                paint_jobs,
                &screen_descriptor,
            );
        }
        for id in &textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        command_buffers.push(encoder.finish());
        self.queue.submit(command_buffers);
        frame.present();
        Ok(())
    }

    fn prepare_snapshot(&mut self, snapshot: &RenderSceneSnapshot) {
        let atoms = snapshot.atoms.iter().map(atom_instance).collect::<Vec<_>>();
        self.atom_instance_count = atoms.len() as u32;
        self.atom_instance_buffer = if atoms.is_empty() {
            empty_buffer(&self.device, "atom-instance-buffer")
        } else {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("atom-instance-buffer"),
                    contents: bytemuck::cast_slice(&atoms),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        };

        let bonds = snapshot
            .bonds
            .iter()
            .flat_map(bond_vertices)
            .collect::<Vec<_>>();
        self.bond_vertex_count = bonds.len() as u32;
        self.bond_vertex_buffer = if bonds.is_empty() {
            empty_buffer(&self.device, "bond-vertex-buffer")
        } else {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("bond-vertex-buffer"),
                    contents: bytemuck::cast_slice(&bonds),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        };

        // Ribbon/cartoon triangle meshes: concatenate all chain meshes into one
        // vertex + index buffer, offsetting each mesh's indices by its base.
        let mut mesh_vertices: Vec<MeshGpuVertex> = Vec::new();
        let mut mesh_indices: Vec<u32> = Vec::new();
        for mesh in &snapshot.meshes {
            let base = mesh_vertices.len() as u32;
            mesh_vertices.extend(mesh.vertices.iter().map(mesh_gpu_vertex));
            mesh_indices.extend(mesh.indices.iter().map(|index| base + index));
        }
        self.mesh_index_count = mesh_indices.len() as u32;
        self.mesh_vertex_buffer = if mesh_vertices.is_empty() {
            empty_buffer(&self.device, "mesh-vertex-buffer")
        } else {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("mesh-vertex-buffer"),
                    contents: bytemuck::cast_slice(&mesh_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        };
        self.mesh_index_buffer = if mesh_indices.is_empty() {
            empty_index_buffer(&self.device, "mesh-index-buffer")
        } else {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("mesh-index-buffer"),
                    contents: bytemuck::cast_slice(&mesh_indices),
                    usage: wgpu::BufferUsages::INDEX,
                })
        };
    }
}

fn create_atom_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    camera_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("atom-pipeline-layout"),
        bind_group_layouts: &[camera_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("atom-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("atom_vs"),
            buffers: &[atom_quad_layout(), atom_instance_layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("atom_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn create_bond_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    camera_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("bond-pipeline-layout"),
        bind_group_layouts: &[camera_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bond-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("bond_vs"),
            buffers: &[bond_vertex_layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("bond_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn create_mesh_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    camera_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mesh-pipeline-layout"),
        bind_group_layouts: &[camera_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mesh-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("mesh_vs"),
            buffers: &[mesh_vertex_layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("mesh_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn atom_quad_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATOM_QUAD_ATTRS,
    }
}

fn atom_instance_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<AtomInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATOM_INSTANCE_ATTRS,
    }
}

fn bond_vertex_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<BondGpuVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &BOND_VERTEX_ATTRS,
    }
}

fn mesh_vertex_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<MeshGpuVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &MESH_VERTEX_ATTRS,
    }
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth-texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn empty_buffer(device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: &[0, 0, 0, 0],
        usage: wgpu::BufferUsages::VERTEX,
    })
}

fn empty_index_buffer(device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: &[0, 0, 0, 0],
        usage: wgpu::BufferUsages::INDEX,
    })
}

fn atom_instance(atom: &RenderAtom) -> AtomInstance {
    let color = atom_color(atom);
    AtomInstance {
        center_radius: [
            atom.center.x as f32,
            atom.center.y as f32,
            atom.center.z as f32,
            atom.radius.max(0.08),
        ],
        color,
        selected: [if atom.selected { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
    }
}

fn bond_vertices(bond: &RenderBond) -> [BondGpuVertex; 2] {
    let color = bond_color(bond);
    [
        BondGpuVertex {
            position: [bond.from.x as f32, bond.from.y as f32, bond.from.z as f32],
            color,
        },
        BondGpuVertex {
            position: [bond.to.x as f32, bond.to.y as f32, bond.to.z as f32],
            color,
        },
    ]
}

fn mesh_gpu_vertex(vertex: &MeshVertex) -> MeshGpuVertex {
    MeshGpuVertex {
        position: [
            vertex.position.x as f32,
            vertex.position.y as f32,
            vertex.position.z as f32,
        ],
        normal: [
            vertex.normal.x as f32,
            vertex.normal.y as f32,
            vertex.normal.z as f32,
        ],
        color: vertex.color,
    }
}

fn atom_color(atom: &RenderAtom) -> [f32; 4] {
    let mut color = rgba_to_float(atom.color.as_ref().and_then(cuemol_gfx::ColorSpec::rgba8));
    if atom.selected {
        color[0] = (color[0] * 0.4 + 0.0 * 0.6).min(1.0);
        color[1] = (color[1] * 0.4 + 0.95 * 0.6).min(1.0);
        color[2] = (color[2] * 0.4 + 1.0 * 0.6).min(1.0);
    }
    color
}

fn bond_color(bond: &RenderBond) -> [f32; 4] {
    if bond.selected {
        [0.0, 0.90, 1.0, 1.0]
    } else {
        rgba_to_float(bond.color.as_ref().and_then(cuemol_gfx::ColorSpec::rgba8))
    }
}

fn rgba_to_float(rgba: Option<Rgba8>) -> [f32; 4] {
    let rgba = rgba.unwrap_or(Rgba8 {
        r: 196,
        g: 201,
        b: 211,
        a: 255,
    });
    [
        f32::from(rgba.r) / 255.0,
        f32::from(rgba.g) / 255.0,
        f32::from(rgba.b) / 255.0,
        f32::from(rgba.a) / 255.0,
    ]
}

fn vec3_from_vector(value: Vector4) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

fn renderer_type_label(renderer_type: &str) -> String {
    RENDERER_TYPES
        .iter()
        .find_map(|(value, label)| (*value == renderer_type).then_some((*label).to_string()))
        .unwrap_or_else(|| renderer_type.to_string())
}

fn object_combo_label(
    objects: &[crate::GuiObjectSummary],
    selected: Option<cuemol_core::Uid>,
) -> String {
    selected
        .and_then(|uid| objects.iter().find(|object| object.uid == uid))
        .map(|object| object.name.clone())
        .unwrap_or_else(|| "Select object".to_string())
}

fn gui_error(error: impl std::fmt::Display) -> CueMolError {
    CueMolError::invalid_argument(error.to_string())
}

fn parse_f32(value: &str) -> Result<f32> {
    value
        .parse::<f32>()
        .map_err(|_| CueMolError::parse(format!("invalid float: {value}")))
}

/// Normalize a quaternion, falling back to identity for zero/degenerate input.
///
/// Mirrors the guard in the original `View::setRotQuat` (qsys/View.cpp).
fn normalized_or_identity(quat: Quat) -> Quat {
    let len_sq = quat.length_squared();
    if !len_sq.is_finite() || len_sq < 1.0e-12 {
        Quat::IDENTITY
    } else {
        quat.normalize()
    }
}

/// Bell virtual-trackball projection: map a 2D point onto a sphere of radius
/// `tkrad` near the center, or onto a hyperbolic sheet farther out. Mirrors
/// `View::projSphere` in the original qsys/View.cpp.
fn project_trackball(x: f64, y: f64, tkrad: f64) -> Vec3 {
    let d = (x * x + y * y).sqrt();
    let z = if d < tkrad * std::f64::consts::FRAC_1_SQRT_2 {
        (tkrad * tkrad - d * d).sqrt()
    } else {
        (tkrad * tkrad / 2.0) / d
    };
    Vec3::new(x as f32, y as f32, z as f32)
}

fn is_network_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn is_view_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "cam" | "view"))
}

fn download_url_to_temp(url: &str) -> Result<PathBuf> {
    let path = url_temp_path(url);
    let status = Command::new("curl")
        .arg("-fsSL")
        .arg("-o")
        .arg(&path)
        .arg(url)
        .status()?;
    if status.success() {
        Ok(path)
    } else {
        Err(CueMolError::invalid_argument(format!(
            "curl failed with status {status}"
        )))
    }
}

fn url_temp_path(url: &str) -> PathBuf {
    let basename = url
        .split('?')
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("download.pdb");
    std::env::temp_dir().join(format!("cuemol-url-{}", sanitize_filename(basename)))
}

fn sanitize_filename(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "download.pdb".to_string()
    } else {
        sanitized
    }
}

fn default_user_dir(name: &str) -> String {
    std::env::var("HOME")
        .map(|home| format!("{home}/.cuemol2/{name}"))
        .unwrap_or_else(|_| name.to_string())
}

fn list_directory_entries(directory: &str) -> std::io::Result<Vec<String>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let suffix = if file_type.is_dir() { "/" } else { "" };
        entries.push(format!("{}{}", entry.file_name().to_string_lossy(), suffix));
    }
    entries.sort();
    Ok(entries)
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    right: [f32; 4],
    up: [f32; 4],
    light: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadVertex {
    corner: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct AtomInstance {
    center_radius: [f32; 4],
    color: [f32; 4],
    selected: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BondGpuVertex {
    position: [f32; 3],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshGpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
}

const ATOM_QUAD_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];
const ATOM_INSTANCE_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
const BOND_VERTEX_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
const MESH_VERTEX_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];

const ATOM_QUAD: [QuadVertex; 6] = [
    QuadVertex {
        corner: [-1.0, -1.0],
    },
    QuadVertex {
        corner: [1.0, -1.0],
    },
    QuadVertex { corner: [1.0, 1.0] },
    QuadVertex {
        corner: [-1.0, -1.0],
    },
    QuadVertex { corner: [1.0, 1.0] },
    QuadVertex {
        corner: [-1.0, 1.0],
    },
];

const MOLECULE_SHADER: &str = r#"
struct Camera {
    view_proj: mat4x4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    light: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct AtomVsIn {
    @location(0) corner: vec2<f32>,
    @location(1) center_radius: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) selected: vec4<f32>,
};

struct AtomVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) selected: f32,
};

@vertex
fn atom_vs(input: AtomVsIn) -> AtomVsOut {
    let center = input.center_radius.xyz;
    let radius = input.center_radius.w;
    let world = center
        + camera.right.xyz * input.corner.x * radius
        + camera.up.xyz * input.corner.y * radius;
    var output: AtomVsOut;
    output.position = camera.view_proj * vec4<f32>(world, 1.0);
    output.local = input.corner;
    output.color = input.color;
    output.selected = input.selected.x;
    return output;
}

@fragment
fn atom_fs(input: AtomVsOut) -> @location(0) vec4<f32> {
    let d = dot(input.local, input.local);
    if (d > 1.0) {
        discard;
    }
    let z = sqrt(max(0.0, 1.0 - d));
    let normal = normalize(vec3<f32>(input.local.x, input.local.y, z));
    let shade = 0.36 + 0.64 * max(dot(normal, normalize(camera.light.xyz)), 0.0);
    var color = vec4<f32>(input.color.rgb * shade, input.color.a);
    if (input.selected > 0.5 && d > 0.62) {
        color = vec4<f32>(0.0, 0.95, 1.0, 1.0);
    }
    return color;
}

struct BondVsIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct BondVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn bond_vs(input: BondVsIn) -> BondVsOut {
    var output: BondVsOut;
    output.position = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn bond_fs(input: BondVsOut) -> @location(0) vec4<f32> {
    return input.color;
}

struct MeshVsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct MeshVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn mesh_vs(input: MeshVsIn) -> MeshVsOut {
    var output: MeshVsOut;
    output.position = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    return output;
}

@fragment
fn mesh_fs(input: MeshVsOut) -> @location(0) vec4<f32> {
    let n = normalize(input.normal);
    let l = normalize(camera.light.xyz);
    // Two-sided lighting so tube ends and ribbon backs are not left black.
    let diff = abs(dot(n, l));
    let shade = 0.30 + 0.70 * diff;
    return vec4<f32>(input.color.rgb * shade, input.color.a);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_gui_tracks_legacy_menu_surfaces() {
        assert!(LEGACY_MENU_SURFACES.len() >= 50);
        assert!(LEGACY_MENU_SURFACES.contains(&"Open File..."));
        assert!(LEGACY_MENU_SURFACES.contains(&"Mol surface generation..."));
        assert!(LEGACY_MENU_SURFACES.contains(&"Restore default panel location"));
        assert!(RENDERER_TYPES
            .iter()
            .any(|(value, _)| *value == "ballstick"));
        assert_eq!(renderer_type_label("cartoon"), "Cartoon");
    }

    #[test]
    fn camera_controller_roundtrips_view_data_and_mutates_by_mouse_actions() {
        let mut camera = CameraController {
            target: Vec3::new(1.0, 2.0, 3.0),
            ..Default::default()
        };
        camera.rotate_by(20.0, -10.0);
        camera.pan_by(5.0, -3.0);
        camera.zoom_by(2.0);

        let data = camera.to_view_data();
        let parsed = CameraController::from_view_data(&data).unwrap();
        assert!((parsed.target.x - camera.target.x).abs() < 1.0e-5);
        assert!((parsed.target.y - camera.target.y).abs() < 1.0e-5);
        assert!((parsed.target.z - camera.target.z).abs() < 1.0e-5);
        assert!(parsed.orientation.abs_diff_eq(camera.orientation, 1.0e-5));
        assert!(parsed.distance < 120.0);
        let qsys_camera = parsed.to_qsys_camera("__current");
        assert_eq!(qsys_camera.name(), "__current");
        assert_eq!(qsys_camera.center().x, parsed.target.x as f64);
        assert_eq!(qsys_camera.is_perspective(), parsed.perspective);
        assert_eq!(
            CameraDragMode::from_button(MouseButton::Left),
            Some(CameraDragMode::Rotate)
        );
        assert_eq!(
            CameraDragMode::from_button(MouseButton::Right),
            Some(CameraDragMode::Pan)
        );
    }

    #[test]
    fn camera_round_trips_through_a_qsys_camera() {
        // from_qsys_camera must invert to_qsys_camera so that a `.qsc` save then
        // reload restores the exact same view. Headless guarantee for the Phase-4
        // camera restore; the on-screen orientation/zoom is user-verified.
        let mut cc = CameraController {
            target: Vec3::new(7.2782, 10.1857, 6.8747),
            orientation: Quat::from_axis_angle(Vec3::new(0.3, 0.5, 0.8).normalize(), 1.1)
                .normalize(),
            distance: 84.0,
            ortho_scale: 22.5,
            perspective: false,
            ..Default::default()
        };
        // Sanity: a non-identity orientation, so the quaternion path is exercised.
        assert!(!cc.orientation.abs_diff_eq(Quat::IDENTITY, 1.0e-3));

        let qsys = cc.to_qsys_camera("__current");
        let back = CameraController::from_qsys_camera(&qsys);
        assert!((back.target - cc.target).length() < 1.0e-3, "target");
        assert!((back.distance - cc.distance).abs() < 1.0e-2, "distance");
        assert!(
            (back.ortho_scale - cc.ortho_scale).abs() < 1.0e-2,
            "ortho_scale"
        );
        assert_eq!(back.perspective, cc.perspective, "perspective");
        // q and -q are the same rotation; accept either sign.
        let diff = (back.orientation - cc.orientation)
            .length()
            .min((back.orientation + cc.orientation).length());
        assert!(diff < 1.0e-3, "orientation mismatch: {diff}");

        // The perspective flag round-trips both ways.
        cc.perspective = true;
        assert!(CameraController::from_qsys_camera(&cc.to_qsys_camera("x")).perspective);
    }

    #[test]
    fn trackball_move_accumulates_normalized_rotation() {
        let mut camera = CameraController::default();
        // Horizontal drag across the center of the viewport.
        camera.trackball_move(0.6, 0.5, 0.4, 0.5);
        assert!(camera.orientation.is_normalized());
        assert!(!camera.orientation.abs_diff_eq(Quat::IDENTITY, 1.0e-4));

        // A zero-length drag must leave the orientation untouched.
        let before = camera.orientation;
        camera.trackball_move(0.5, 0.5, 0.5, 0.5);
        assert!(camera.orientation.abs_diff_eq(before, 1.0e-6));
    }

    #[test]
    fn project_trackball_switches_between_sphere_and_hyperbola() {
        // The viewport center maps to the top of the sphere (z == radius).
        let center = project_trackball(0.0, 0.0, 0.8);
        assert!((center.z - 0.8).abs() < 1.0e-5);
        // A far point falls onto the hyperbolic sheet (0 < z < radius).
        let far = project_trackball(2.0, 0.0, 0.8);
        assert!(far.z > 0.0 && far.z < 0.8);
    }

    #[test]
    fn native_gui_recognizes_url_and_view_paths() {
        assert!(is_network_url("https://files.rcsb.org/download/1CRN.pdb"));
        assert!(is_view_file(Path::new("current.cam")));
        assert_eq!(sanitize_filename("a/b?c.pdb"), "a_b_c.pdb");
        assert!(
            url_temp_path("https://example.test/foo/1CRN.pdb?download=1")
                .ends_with("cuemol-url-1CRN.pdb")
        );
    }

    #[test]
    fn native_gui_projects_atoms_for_picking() {
        let camera = CameraController::default();
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let (screen, depth) = camera
            .project_to_viewport(Vec3::ZERO, viewport)
            .expect("origin should project");
        assert!((screen.x - 400.0).abs() < 1.0);
        assert!((screen.y - 300.0).abs() < 1.0);
        assert!((depth - camera.distance).abs() < 1.0e-5);
        assert!(camera.projected_radius(0.45, depth, viewport) > 2.0);

        let mut app = GuiApplication::new();
        app.viewport_rect = Some(viewport);
        app.snapshot.atoms.push(RenderAtom {
            center: Vector4::ZERO,
            radius: 0.45,
            color: None,
            object_uid: Some(7),
            atom_index: Some(3),
            selected: false,
            detail: None,
        });
        assert_eq!(
            app.pick_hit(&camera, egui::pos2(400.0, 300.0)),
            Some(PickHit {
                object_uid: 7,
                atom_index: 3,
            })
        );
    }

    fn reference_pdb_path() -> Option<std::path::PathBuf> {
        let root = option_env!("CUEMOL2_REF_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2")
            });
        let pdb = root.join("tests/test_data/1CRN.pdb");
        pdb.exists().then_some(pdb)
    }

    /// Exercise the whole shell + every left-panel section headlessly so a runtime
    /// panic / borrow problem in the new CueMol2 layout is caught without a window.
    #[test]
    fn cuemol2_shell_and_left_panels_render_headless() {
        let ctx = egui::Context::default();
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();

        // Whole window (menu / ribbon / left dock / scene tabs / bottom dock /
        // status bar / properties window) on the empty default scene.
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.ui(ctx, &mut camera);
        });

        // With a real molecule (when the reference tree is present), drive every
        // section body directly so the non-empty branches run too.
        if let Some(pdb) = reference_pdb_path() {
            app.open_path = pdb.to_string_lossy().to_string();
            app.open_current_path(&mut camera);
            assert!(app.snapshot.atom_count() > 0);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.workspace_section(ui, &mut camera);
                    app.molstruct_section(ui, &mut camera);
                    app.selection_section(ui);
                    app.coloring_section(ui);
                    app.symmetry_section(ui);
                    app.densitymap_section(ui);
                    app.view_section(ui, &mut camera);
                    app.animation_section(ui);
                });
            });
            // MolStruct chain-select wrote a selection; Symmetry found 1CRN's cell.
            assert!(!app.state.active_crystal_summaries().is_empty());

            // Exercise the viewport right-click context menu render path against a
            // real picked atom (catches a panic in navi-ctxtmenu-atom without a window).
            app.viewport_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            ));
            let first = app
                .snapshot
                .atoms
                .iter()
                .find_map(|atom| Some((atom.object_uid?, atom.atom_index?)))
                .expect("loaded molecule has a pickable atom");
            app.viewport_menu = Some(ViewportMenu {
                pos: egui::pos2(100.0, 100.0),
                hit: Some(PickHit {
                    object_uid: first.0,
                    atom_index: first.1,
                }),
            });
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                app.ui(ctx, &mut camera);
            });
            assert!(app.viewport_menu.is_some(), "menu stays open until chosen");
        }
    }

    #[test]
    fn rainbow_object_coloring_spreads_atom_colors_into_a_gradient() {
        // Probe that the Coloring panel actually recolours atoms: applying Rainbow must
        // spread the carbon atoms across a hue gradient (one distinct colour per
        // residue), where Element mode gives a single carbon colour. Verifies the
        // object-coloring cascade reaches the ball-and-stick atom path, not just ribbon.
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().to_string();
        app.open_current_path(&mut camera);
        let object_uid = app
            .state
            .active_object_summaries()
            .first()
            .map(|object| object.uid)
            .expect("1CRN loads as one object");

        let distinct_colors = |app: &GuiApplication| -> usize {
            app.snapshot
                .atoms
                .iter()
                .filter_map(|atom| atom.color.as_ref().and_then(|color| color.rgba8()))
                .map(|color| format!("{color:?}"))
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        };

        app.apply_object_coloring(object_uid, ColoringMode::Element);
        let element_colors = distinct_colors(&app);
        app.apply_object_coloring(object_uid, ColoringMode::Rainbow);
        let rainbow_colors = distinct_colors(&app);
        assert!(
            rainbow_colors > element_colors + 10,
            "rainbow must spread carbon colours into a gradient: \
             element={element_colors} rainbow={rainbow_colors}"
        );
    }

    #[test]
    fn measure_selection_computes_distance_angle_torsion() {
        // Synthetic, controlled-geometry oracle (independent of any molecule): 2 selected
        // atoms → distance, 3 → angle, 4 → torsion.
        let mut app = GuiApplication::new();
        let atom = |x: f64, y: f64, z: f64| RenderAtom {
            center: Vector4::new3(x, y, z),
            radius: 0.3,
            color: None,
            object_uid: None,
            atom_index: None,
            selected: true,
            detail: None,
        };
        // 3-4-5 right triangle legs → distance 5.
        app.snapshot.atoms = vec![atom(0.0, 0.0, 0.0), atom(3.0, 4.0, 0.0)];
        assert!(
            app.measure_selection().unwrap().contains("5.000"),
            "distance: {:?}",
            app.measure_selection()
        );
        // Right angle at the central atom → 90°.
        app.snapshot.atoms = vec![
            atom(1.0, 0.0, 0.0),
            atom(0.0, 0.0, 0.0),
            atom(0.0, 1.0, 0.0),
        ];
        assert!(
            app.measure_selection().unwrap().contains("90.00"),
            "angle: {:?}",
            app.measure_selection()
        );
        // A ±90° dihedral.
        app.snapshot.atoms = vec![
            atom(1.0, 0.0, 0.0),
            atom(0.0, 0.0, 0.0),
            atom(0.0, 0.0, 1.0),
            atom(0.0, 1.0, 1.0),
        ];
        let torsion = app.measure_selection().unwrap();
        assert!(
            torsion.contains("Torsion") && torsion.contains("90.00"),
            "torsion: {torsion}"
        );
        // A single selected atom yields no measurement.
        app.snapshot.atoms = vec![atom(0.0, 0.0, 0.0)];
        assert!(app.measure_selection().is_none());
    }

    #[test]
    fn setting_a_renderer_bond_radius_changes_the_rendered_geometry() {
        // The renderer property dialog's "Bond radius" must reach the geometry: setting
        // the inline `bondw` attribute and re-rendering changes the snapshot's bond
        // cylinder radii (the inline attr wins over the style in apply_renderer_styles).
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().to_string();
        app.open_current_path(&mut camera);
        let object_uid = app
            .state
            .active_object_summaries()
            .first()
            .map(|object| object.uid)
            .expect("1CRN loads as one object");
        // Ensure a renderer exists to target (a plain PDB may render implicitly).
        let renderer_uid = app
            .state
            .renderer_summaries(object_uid)
            .first()
            .map(|renderer| renderer.uid)
            .unwrap_or_else(|| {
                app.state
                    .new_renderer_in_active_scene(object_uid, "ballstick", "test")
                    .expect("create renderer")
                    .result_renderer
            });

        app.state
            .set_renderer_property(object_uid, renderer_uid, "bondw", "0.5")
            .expect("set bondw");
        app.rebuild_snapshot();
        let radius = app
            .snapshot
            .bonds
            .first()
            .map(|bond| bond.radius)
            .expect("the rendered scene has bonds");
        assert!(
            (radius - 0.5).abs() < 0.05,
            "bond radius should reflect bondw=0.5, got {radius}"
        );
    }

    /// A synthetic isotropic-Gaussian density map (no deposited fixture needed — the
    /// only bundled `.dx` is all-zeros and contours to nothing). Col(X)-fastest values.
    fn synthetic_gaussian_map() -> cuemol_xtal::DensityMap {
        let n = 20usize;
        let center = (n as f64 - 1.0) / 2.0;
        let sigma2 = 9.0;
        let mut values = Vec::with_capacity(n * n * n);
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    let dx = i as f64 - center;
                    let dy = j as f64 - center;
                    let dz = k as f64 - center;
                    values.push((-(dx * dx + dy * dy + dz * dz) / (2.0 * sigma2)).exp());
                }
            }
        }
        let grid = cuemol_molstr::GridMap {
            counts: [n, n, n],
            origin: Vector4::new3(0.0, 0.0, 0.0),
            delta: [
                Vector4::new3(1.0, 0.0, 0.0),
                Vector4::new3(0.0, 1.0, 0.0),
                Vector4::new3(0.0, 0.0, 1.0),
            ],
            values,
        };
        cuemol_xtal::DensityMap::from_grid_map(grid)
    }

    #[test]
    fn hbonds_add_dashed_lines_for_1crn() {
        // 1CRN (helices + a β-sheet) has many backbone N···O hydrogen bonds, each drawn
        // as dashed segments; toggling H-bonds adds them and toggling off removes them.
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().to_string();
        app.open_current_path(&mut camera);
        let base = app.snapshot.bonds.len();

        app.show_hbonds = true;
        app.rebuild_snapshot();
        let added = app.snapshot.bonds.len() - base;
        assert!(
            added > 20,
            "1CRN's backbone H-bonds add dashed segments, got {added}"
        );

        app.show_hbonds = false;
        app.rebuild_snapshot();
        assert_eq!(app.snapshot.bonds.len(), base, "removed on toggle off");
    }

    #[test]
    fn execute_script_runs_real_js_in_egui() {
        // Execute-script RUNS JavaScript via boa (not load+preview): JS arithmetic +
        // cuemol.print logs the result, and cuemol.select applies a selection.
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().to_string();
        app.open_current_path(&mut camera);
        let script = std::env::temp_dir().join("cuemol_egui_script.js");
        std::fs::write(
            &script,
            "cuemol.print('val=' + (10/2)); cuemol.select('chain A');",
        )
        .unwrap();
        app.script_path = script.to_string_lossy().to_string();
        app.load_script_into_log();
        assert!(
            app.log_lines.iter().any(|line| line.contains("val=5")),
            "JS executed + printed, log: {:?}",
            app.log_lines
        );
        assert_eq!(app.selection_text, "chain A", "script applied a selection");
        let _ = std::fs::remove_file(&script);
    }

    #[test]
    fn live_surface_clipping_in_egui() {
        // Live in-viewport surface clipping: enabling clip rebuilds the displayed surface
        // as the plane cross-section, and moving the offset re-clips it live.
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().to_string();
        app.open_current_path(&mut camera);
        app.selection_text = "resid 1:5".to_string();
        let tris = |app: &GuiApplication| {
            app.surface_mesh
                .as_ref()
                .map(|m| m.indices.len())
                .unwrap_or(0)
        };
        app.toggle_surface();
        let full = tris(&app);
        assert!(full > 0, "full surface built");
        app.toggle_clip_surface();
        assert!(app.clip_surface, "clip enabled");
        let clipped = tris(&app);
        assert!(clipped <= full, "clipping doesn't add geometry");
        app.set_clip_offset(50.0);
        assert_ne!(tris(&app), clipped, "moving the plane re-clips live");
    }

    #[test]
    fn surface_display_adds_an_ses_mesh_to_the_snapshot() {
        // Molecular-surface DISPLAY (until now the desktop only EXPORTED the SES to OBJ):
        // build_surface_mesh yields a triangle mesh, and toggling Surface adds/removes it
        // from the snapshot. A small selection keeps the SES grid cheap for the test.
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().to_string();
        app.open_current_path(&mut camera);
        let object_uid = app
            .state
            .active_object_summaries()
            .first()
            .map(|object| object.uid)
            .expect("1CRN loads as one object");

        let mesh = app
            .state
            .build_surface_mesh(object_uid, "resid 1:3", 1.0, 1.4)
            .expect("SES builds");
        assert!(
            mesh.vertices.len() > 10 && mesh.indices.len() >= 30,
            "the SES is a real triangle mesh ({} verts, {} indices)",
            mesh.vertices.len(),
            mesh.indices.len()
        );

        app.molstruct_object = Some(object_uid);
        app.selection_text = "resid 1:3".to_string();
        let base_meshes = app.snapshot.meshes.len();
        app.toggle_surface();
        assert!(app.show_surface, "surface toggled on");
        assert!(
            app.snapshot.meshes.len() > base_meshes,
            "the SES mesh is added to the snapshot"
        );
        app.toggle_surface();
        assert!(!app.show_surface);
        assert_eq!(
            app.snapshot.meshes.len(),
            base_meshes,
            "the SES mesh is removed when toggled off"
        );
    }

    #[test]
    fn potential_surface_colours_the_ses_by_electrostatic_potential_in_egui() {
        // The egui Potential toggle routes the SES through build_surface_potential_mesh
        // (SCAPOT) and adds the potential-coloured mesh to the snapshot. Inject a ramp
        // potential (−x→+x through the residues' centroid) so the surface spans red→blue.
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().to_string();
        app.open_current_path(&mut camera);
        let object_uid = app
            .state
            .active_object_summaries()
            .first()
            .map(|object| object.uid)
            .expect("1CRN loads as one object");
        let centroid = {
            let atoms: Vec<cuemol_core::Vector4> = app
                .state
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .mol()
                .atoms()
                .iter()
                .filter(|a| (1..=3).contains(&a.res_id))
                .map(|a| a.position)
                .collect();
            let n = atoms.len() as f64;
            let sum = atoms
                .iter()
                .fold(cuemol_core::Vector4::ZERO, |acc, p| acc + *p);
            cuemol_core::Vector4::new3(sum.x / n, sum.y / n, sum.z / n)
        };
        let n = 80usize;
        let origin =
            cuemol_core::Vector4::new3(centroid.x - 40.0, centroid.y - 40.0, centroid.z - 40.0);
        let mut values = vec![0.0_f64; n * n * n];
        for sec in 0..n {
            for row in 0..n {
                for col in 0..n {
                    values[col + (row + sec * n) * n] = (origin.x + col as f64) - centroid.x;
                }
            }
        }
        app.state
            .set_surface_potential_map(cuemol_desktop_core::DensityMap::from_grid_map(
                cuemol_desktop_core::GridMap {
                    counts: [n, n, n],
                    origin,
                    delta: [
                        cuemol_core::Vector4::new3(1.0, 0.0, 0.0),
                        cuemol_core::Vector4::new3(0.0, 1.0, 0.0),
                        cuemol_core::Vector4::new3(0.0, 0.0, 1.0),
                    ],
                    values,
                },
            ));
        app.molstruct_object = Some(object_uid);
        app.selection_text = "resid 1:3".to_string();
        let base_meshes = app.snapshot.meshes.len();
        app.toggle_potential_surface();
        assert!(app.potential_surface && app.show_surface);
        assert!(
            app.snapshot.meshes.len() > base_meshes,
            "the potential SES mesh is added to the snapshot"
        );
        let mesh = app.surface_mesh.as_ref().expect("potential SES cached");
        let red = mesh.vertices.iter().any(|v| v.color[0] > v.color[2] + 0.02);
        let blue = mesh.vertices.iter().any(|v| v.color[2] > v.color[0] + 0.02);
        assert!(
            red && blue,
            "the SES must span the red→blue potential gradient"
        );
    }

    #[test]
    fn density_contour_appears_when_toggled() {
        // Wire the orphaned MapMeshRenderer contour into the GUI: toggling "Show contour"
        // on a (synthetic) density map adds chickenwire line segments to the snapshot.
        let mut app = GuiApplication::new();
        let scene = app
            .state
            .active_scene()
            .expect("a default scene exists after construction");
        app.state
            .add_density_map(scene, "gauss", synthetic_gaussian_map());
        let base_bonds = app.snapshot.bonds.len();

        app.show_density_contour = true;
        app.density_level = 1.0;
        app.rebuild_snapshot();
        let added = app.snapshot.bonds.len() - base_bonds;
        assert!(
            added > 50,
            "the contour adds chickenwire segments, got {added}"
        );

        app.show_density_contour = false;
        app.rebuild_snapshot();
        assert_eq!(
            app.snapshot.bonds.len(),
            base_bonds,
            "toggling the contour off removes it"
        );
    }

    #[test]
    fn density_solid_surface_adds_a_filled_isosurface_mesh() {
        // The filled density isosurface (2nd density mode) wires the orphaned
        // generate_surf_obj: build a marching-cubes mesh + toggle it into the snapshot.
        let mut app = GuiApplication::new();
        let scene = app.state.active_scene().expect("default scene");
        app.state
            .add_density_map(scene, "gauss", synthetic_gaussian_map());
        app.density_level = 1.0;

        let mesh = app
            .state
            .build_density_surface_mesh(1.0)
            .expect("a filled isosurface builds");
        assert!(
            mesh.indices.len() >= 30,
            "the filled isosurface is a real triangle mesh ({} indices)",
            mesh.indices.len()
        );

        let base = app.snapshot.meshes.len();
        app.toggle_density_solid();
        assert!(app.show_density_solid);
        assert!(
            app.snapshot.meshes.len() > base,
            "the solid isosurface mesh is added"
        );
        app.toggle_density_solid();
        assert_eq!(app.snapshot.meshes.len(), base, "removed on toggle off");
    }

    #[test]
    fn unit_cell_toggle_adds_the_twelve_cell_box_edges() {
        // 1CRN's CRYST1 (P 1 21 1) gives a crystallographic cell, so toggling the
        // Symmetry "Unit cell" must add exactly the 12 box edges as bonds, and toggling
        // it off must remove them.
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().to_string();
        app.open_current_path(&mut camera);
        let base_bonds = app.snapshot.bonds.len();

        app.show_unit_cell = true;
        app.rebuild_snapshot();
        assert_eq!(
            app.snapshot.bonds.len() - base_bonds,
            12,
            "the unit cell box has 12 edges"
        );

        app.show_unit_cell = false;
        app.rebuild_snapshot();
        assert_eq!(
            app.snapshot.bonds.len(),
            base_bonds,
            "toggling the unit cell off removes its edges"
        );
    }

    #[test]
    fn symm_mol_toggle_adds_symmetry_mate_copies() {
        // 1CRN is P 1 21 1 (one 2₁ screw beyond the identity), so "Symm mol" adds
        // exactly one whole-molecule copy; the atom count scales by (mates + 1).
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().to_string();
        app.open_current_path(&mut camera);
        let base_atoms = app.snapshot.atoms.len();
        let mate_count = app.state.active_symmetry_mate_transforms().len();
        assert!(
            mate_count >= 1,
            "P 1 21 1 has at least one non-identity symmetry mate"
        );

        app.show_symm_mates = true;
        app.rebuild_snapshot();
        assert_eq!(
            app.snapshot.atoms.len(),
            base_atoms * (mate_count + 1),
            "each mate copies the whole molecule"
        );

        app.show_symm_mates = false;
        app.rebuild_snapshot();
        assert_eq!(
            app.snapshot.atoms.len(),
            base_atoms,
            "toggling off removes copies"
        );
    }

    #[test]
    fn selection_editor_expressions_parse_as_valid_grammar() {
        // Every expression the Selection Editor tab can emit must be valid selection
        // grammar, so clicking a quick-select never produces an unparseable selection.
        for expr in [
            "*",
            "none",
            "bymainch *",
            "bysidech *",
            "elem H",
            "chain A",
            "byres (chain A)",
            "(chain A) around 5",
            "!(chain A)",
        ] {
            assert!(
                cuemol_molstr::parse_selector(expr).is_ok(),
                "editor expression must parse: {expr}"
            );
        }
    }

    #[test]
    fn animation_timeline_drives_the_camera() {
        // A Spin object rotates the camera while active; a Pause object holds it.
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.animation_objects.push(AnimObject {
            kind: AnimKind::Spin,
            duration: 3.0,
        });
        let before = camera.orientation;
        app.advance_animation(0.1, &mut camera);
        assert!(
            !camera.orientation.abs_diff_eq(before, 1.0e-5),
            "a Spin object rotates the camera"
        );

        app.animation_objects = vec![AnimObject {
            kind: AnimKind::Pause,
            duration: 3.0,
        }];
        app.animation_elapsed = 0.0;
        let still = camera.orientation;
        app.advance_animation(0.1, &mut camera);
        assert!(
            camera.orientation.abs_diff_eq(still, 1.0e-6),
            "a Pause object holds the camera"
        );
    }

    #[test]
    fn animation_play_spins_the_camera() {
        // Playing the animation continuously spins the camera (CueMol2 SimpleSpin);
        // stopping leaves it still.
        let ctx = egui::Context::default();
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();

        app.animation_playing = true;
        let before = camera.orientation;
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.ui(ctx, &mut camera));
        assert!(
            !camera.orientation.abs_diff_eq(before, 1.0e-4),
            "playing must spin the camera"
        );

        app.animation_playing = false;
        let stopped = camera.orientation;
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.ui(ctx, &mut camera));
        assert!(
            camera.orientation.abs_diff_eq(stopped, 1.0e-6),
            "stopped must leave the camera still"
        );
    }

    #[test]
    fn shift_left_drag_zooms_the_camera() {
        let mut camera = CameraController::default();
        let start = camera.ortho_scale;
        // Dragging up in zoom mode reduces ortho_scale (zoom in).
        camera.zoom_by(-(-20.0) * 0.1);
        assert!(
            camera.ortho_scale < start,
            "shift+left drag-up should zoom in"
        );
    }

    /// Opt-in visual self-verification: render the whole CueMol2 shell to a PNG via
    /// egui_kittest's offscreen wgpu (Metal) renderer, so the layout can be compared
    /// against the authoritative XUL inventory (`docs/migration/uxp-inventory/`)
    /// without an on-screen window. No-op unless `CUEMOL_SNAPSHOT_OUT` names an output
    /// path; self-skips (does not fail) when no GPU adapter is available — the same
    /// graceful-degradation contract as `offscreen.rs`. The 3D viewport renders blank
    /// here (egui_wgpu paint callbacks don't run under kittest's own renderer); the
    /// chrome (menu / ribbon / 8-panel left dock / scene tabs / status bar) is the
    /// subject of the snapshot.
    #[test]
    fn render_cuemol2_shell_to_png_for_visual_review() {
        let Ok(out_dir) = std::env::var("CUEMOL_SNAPSHOT_OUT") else {
            return;
        };
        let out_dir = std::path::PathBuf::from(out_dir);
        let _ = std::fs::create_dir_all(&out_dir);

        let load = |app: &mut GuiApplication, camera: &mut CameraController| {
            if let Some(pdb) = reference_pdb_path() {
                app.open_path = pdb.to_string_lossy().to_string();
                app.open_current_path(camera);
            }
        };

        // (1) Whole shell — menu / ribbon / left dock / scene tabs / bottom dock /
        // status bar. The accordion is mostly collapsed here (egui default state),
        // so this verifies the SKELETON only.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1366.0, 850.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("shell.png"));
        }

        // (1b) The REAL shell with every accordion section forced open, at the actual
        // ~240px dock width — verifies the panel bodies (toolbars, combos, hue sliders)
        // fit / wrap acceptably inside the narrow left dock the user actually opens, not
        // just in an isolated wide column.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            app.force_open_all_panels = true;
            if let Some(uid) = app.state.active_object_summaries().first().map(|o| o.uid) {
                app.apply_object_coloring(uid, ColoringMode::Rainbow);
            }
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1100.0, 980.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("dock.png"));
        }

        // (1b2) The shell in the LIGHT theme (View → Theme → Light).
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            app.light_theme = true;
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1366.0, 850.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("theme_light.png"));
        }

        // (1c) Atom labels overlaid on a small selection. The egui painter text DOES
        // render under kittest (unlike the wgpu 3D callback), so the projected
        // "RESid:ATOM" labels appear over the (blank) viewport — verifying the projection
        // + label text end to end.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            app.selection_text = "resid 1:2".to_string();
            app.rebuild_snapshot();
            app.show_labels = true;
            camera.fit_snapshot(&app.snapshot);
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1100.0, 850.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("labels.png"));
        }

        // (1d) The interactive per-residue sequence viewer (bottom Sequence tab) with a
        // selection highlighted — verifies the 1-letter cells render and the selected
        // residues are highlighted (egui widgets render under kittest).
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            app.bottom_tab = BottomTab::Sequence;
            app.selection_text = "resid 1:12".to_string();
            app.rebuild_snapshot();
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1100.0, 850.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("sequence.png"));
        }

        // (2) Every left-dock panel body expanded at once, so each panel's CONTROLS
        // can be diffed against the XUL inventory (`docs/migration/uxp-inventory/
        // panels.md`). Drives the section bodies directly — the same entry points the
        // headless smoke test exercises — inside one tall scrollable column.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            // Expand the MolStruct tree so the chain → residue nesting is visible.
            app.molstruct_expand_all = true;
            // Show the Selection Editor tab (the new build-by-click controls).
            app.selection_tab = SelectionTab::Editor;
            // Put the Coloring panel into Rainbow mode so its hue controls render.
            if let Some(uid) = app.state.active_object_summaries().first().map(|o| o.uid) {
                app.apply_object_coloring(uid, ColoringMode::Rainbow);
            }
            // A couple of animation objects so the Animation panel list renders.
            app.animation_objects = vec![
                AnimObject {
                    kind: AnimKind::Spin,
                    duration: 3.0,
                },
                AnimObject {
                    kind: AnimKind::Pause,
                    duration: 1.0,
                },
            ];
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(420.0, 1500.0))
                .build(|ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.heading("Workspace");
                            app.workspace_section(ui, &mut camera);
                            ui.separator();
                            ui.heading("MolStruct");
                            app.molstruct_section(ui, &mut camera);
                            ui.separator();
                            ui.heading("Selection");
                            app.selection_section(ui);
                            ui.separator();
                            ui.heading("Coloring");
                            app.coloring_section(ui);
                            ui.separator();
                            ui.heading("Symmetry");
                            app.symmetry_section(ui);
                            ui.separator();
                            ui.heading("Density map");
                            app.densitymap_section(ui);
                            ui.separator();
                            ui.heading("View");
                            app.view_section(ui, &mut camera);
                            ui.separator();
                            ui.heading("Animation");
                            app.animation_section(ui);
                        });
                    });
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("panels.png"));
        }

        // (3) A real 3D render of the rainbow-coloured molecule (offscreen wgpu), so the
        // object-coloring cascade is verified visually — not only by the colour-count
        // oracle. egui's own central viewport stays blank under kittest, so this uses
        // the headless mesh renderer directly.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            if let Some(uid) = app.state.active_object_summaries().first().map(|o| o.uid) {
                app.apply_object_coloring(uid, ColoringMode::Rainbow);
                camera.fit_snapshot(&app.snapshot);
                let (w, h) = (900u32, 700u32);
                let mvp = camera
                    .view_projection(w as f32 / h as f32)
                    .to_cols_array_2d();
                if let Some(png) = crate::offscreen::render_snapshot_png(
                    &app.snapshot,
                    mvp,
                    w,
                    h,
                    [0.02, 0.02, 0.05, 1.0],
                ) {
                    let path = out_dir.join("rainbow_3d.png");
                    std::fs::write(&path, png).expect("write rainbow_3d.png");
                    eprintln!("snapshot written to {}", path.display());
                }
            }
        }

        // (4) 3D render of the molecule wrapped in its crystallographic unit-cell box
        // (Symmetry "Unit cell"), verifying the cell-edge geometry visually.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            app.show_unit_cell = true;
            app.rebuild_snapshot();
            if app.snapshot.bounds.is_some() {
                camera.fit_snapshot(&app.snapshot);
                let (w, h) = (900u32, 700u32);
                let mvp = camera
                    .view_projection(w as f32 / h as f32)
                    .to_cols_array_2d();
                if let Some(png) = crate::offscreen::render_snapshot_png(
                    &app.snapshot,
                    mvp,
                    w,
                    h,
                    [0.02, 0.02, 0.05, 1.0],
                ) {
                    let path = out_dir.join("unit_cell_3d.png");
                    std::fs::write(&path, png).expect("write unit_cell_3d.png");
                    eprintln!("snapshot written to {}", path.display());
                }
            }
        }

        // (5) 3D render of the home-cell crystal packing: the molecule, its symmetry
        // mates ("Symm mol"), and the unit-cell box together.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            app.show_symm_mates = true;
            app.show_unit_cell = true;
            app.rebuild_snapshot();
            if app.snapshot.bounds.is_some() {
                camera.fit_snapshot(&app.snapshot);
                let (w, h) = (900u32, 700u32);
                let mvp = camera
                    .view_projection(w as f32 / h as f32)
                    .to_cols_array_2d();
                if let Some(png) = crate::offscreen::render_snapshot_png(
                    &app.snapshot,
                    mvp,
                    w,
                    h,
                    [0.02, 0.02, 0.05, 1.0],
                ) {
                    let path = out_dir.join("symm_pack_3d.png");
                    std::fs::write(&path, png).expect("write symm_pack_3d.png");
                    eprintln!("snapshot written to {}", path.display());
                }
            }
        }

        // (6) 3D render of the electron-density chickenwire contour over a synthetic
        // Gaussian map — the formerly-orphaned MapMeshRenderer wired into the GUI.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            if let Some(scene) = app.state.active_scene() {
                app.state
                    .add_density_map(scene, "gauss", synthetic_gaussian_map());
                app.show_density_contour = true;
                app.density_level = 1.0;
                app.rebuild_snapshot();
                if app.snapshot.bounds.is_some() {
                    camera.fit_snapshot(&app.snapshot);
                    let (w, h) = (900u32, 700u32);
                    let mvp = camera
                        .view_projection(w as f32 / h as f32)
                        .to_cols_array_2d();
                    if let Some(png) = crate::offscreen::render_snapshot_png(
                        &app.snapshot,
                        mvp,
                        w,
                        h,
                        [0.02, 0.02, 0.05, 1.0],
                    ) {
                        let path = out_dir.join("density_3d.png");
                        std::fs::write(&path, png).expect("write density_3d.png");
                        eprintln!("snapshot written to {}", path.display());
                    }
                }
            }
        }

        // (7) 3D render of the molecular surface (SES) — the formerly export-only
        // surface now drawn in the viewport. A moderate selection keeps the SES cheap.
        if let Some(pdb) = reference_pdb_path() {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            app.open_path = pdb.to_string_lossy().to_string();
            app.open_current_path(&mut camera);
            app.selection_text = "resid 1:25".to_string();
            app.toggle_surface();
            if !app.snapshot.meshes.is_empty() {
                camera.fit_snapshot(&app.snapshot);
                let (w, h) = (900u32, 700u32);
                let mvp = camera
                    .view_projection(w as f32 / h as f32)
                    .to_cols_array_2d();
                if let Some(png) = crate::offscreen::render_snapshot_png(
                    &app.snapshot,
                    mvp,
                    w,
                    h,
                    [0.02, 0.02, 0.05, 1.0],
                ) {
                    let path = out_dir.join("surface_3d.png");
                    std::fs::write(&path, png).expect("write surface_3d.png");
                    eprintln!("snapshot written to {}", path.display());
                }
            }
        }

        // (8) 3D render of the FILLED density isosurface (solid mode) over the same
        // synthetic Gaussian — the solid-surface counterpart of the chickenwire.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            if let Some(scene) = app.state.active_scene() {
                app.state
                    .add_density_map(scene, "gauss", synthetic_gaussian_map());
                app.density_level = 1.0;
                app.toggle_density_solid();
                if !app.snapshot.meshes.is_empty() {
                    camera.fit_snapshot(&app.snapshot);
                    let (w, h) = (900u32, 700u32);
                    let mvp = camera
                        .view_projection(w as f32 / h as f32)
                        .to_cols_array_2d();
                    if let Some(png) = crate::offscreen::render_snapshot_png(
                        &app.snapshot,
                        mvp,
                        w,
                        h,
                        [0.02, 0.02, 0.05, 1.0],
                    ) {
                        let path = out_dir.join("density_solid_3d.png");
                        std::fs::write(&path, png).expect("write density_solid_3d.png");
                        eprintln!("snapshot written to {}", path.display());
                    }
                }
            }
        }

        // (9) 3D render of 1CRN's hydrogen-bond network (dashed cyan lines) over the
        // ball-and-stick model.
        if let Some(pdb) = reference_pdb_path() {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            app.open_path = pdb.to_string_lossy().to_string();
            app.open_current_path(&mut camera);
            app.show_hbonds = true;
            app.rebuild_snapshot();
            camera.fit_snapshot(&app.snapshot);
            let (w, h) = (900u32, 700u32);
            let mvp = camera
                .view_projection(w as f32 / h as f32)
                .to_cols_array_2d();
            if let Some(png) = crate::offscreen::render_snapshot_png(
                &app.snapshot,
                mvp,
                w,
                h,
                [0.02, 0.02, 0.05, 1.0],
            ) {
                let path = out_dir.join("hbonds_3d.png");
                std::fs::write(&path, png).expect("write hbonds_3d.png");
                eprintln!("snapshot written to {}", path.display());
            }
        }

        // (imported mesh) 3D render of an external OBJ mesh brought in via
        // `import_mesh_surface` and shown through the standard surface toggle — the
        // molecule-independent display path. The mesh is a flat-blue octahedron, so a
        // recognisable solid should appear enclosing the molecule.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            let obj_path = out_dir.join("imported.obj");
            std::fs::write(
                &obj_path,
                "o oct\n\
                 v 8 0 0\nv -8 0 0\nv 0 8 0\nv 0 -8 0\nv 0 0 8\nv 0 0 -8\n\
                 f 1 3 5\nf 3 2 5\nf 2 4 5\nf 4 1 5\n\
                 f 3 1 6\nf 2 3 6\nf 4 2 6\nf 1 4 6\n",
            )
            .expect("write imported.obj");
            app.mesh_import_path = obj_path.to_string_lossy().to_string();
            app.import_mesh_surface(&mut camera);
            assert!(
                app.surface_mesh.is_some(),
                "import must populate the surface slot"
            );
            let (w, h) = (900u32, 700u32);
            let mvp = camera
                .view_projection(w as f32 / h as f32)
                .to_cols_array_2d();
            if let Some(png) = crate::offscreen::render_snapshot_png(
                &app.snapshot,
                mvp,
                w,
                h,
                [0.02, 0.02, 0.05, 1.0],
            ) {
                let path = out_dir.join("imported_mesh_3d.png");
                std::fs::write(&path, png).expect("write imported_mesh_3d.png");
                eprintln!("snapshot written to {}", path.display());
            }
        }

        // (qsc surface) A `.qsc` that CARRIES an OBJ-text MolSurf object now loads AND
        // displays the surface (the scene-object → surface-display slice), rendered
        // offscreen. Verifies the whole chain: loader captures the surface datachunk →
        // active_scene_surface_meshes converts it → the open handler shows it.
        {
            let mut builder = cuemol_molstr::MolSceneManager::new();
            let bscene = builder.new_scene("s");
            if let Some(pdb) = reference_pdb_path() {
                if let Ok(mol) = cuemol_molstr::read_pdb_file(&pdb) {
                    builder
                        .scene_mut(bscene)
                        .unwrap()
                        .add_molecule("crambin", mol);
                }
            }
            let mut qsc = builder.scene(bscene).unwrap().write_qsc_string();
            let surf = "\t<object type=\"MolSurf\" name=\"surf1\" src=\"datachunk:90000\" srctype=\"obj\">\n\t</object>\n</scene>";
            qsc = qsc.replace("</scene>", surf);
            qsc.push_str(
                "\n==========START datachunk:90000==========\nv 8 0 0\nv -8 0 0\nv 0 8 0\nv 0 -8 0\nv 0 0 8\nv 0 0 -8\nf 1 3 5\nf 3 2 5\nf 2 4 5\nf 4 1 5\nf 3 1 6\nf 2 3 6\nf 4 2 6\nf 1 4 6\n==========END datachunk:90000==========\n",
            );
            let qsc_path = out_dir.join("surface_scene.qsc");
            std::fs::write(&qsc_path, &qsc).expect("write surface_scene.qsc");

            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            app.open_path = qsc_path.to_string_lossy().to_string();
            app.open_current_path(&mut camera);
            assert!(
                app.surface_mesh.is_some(),
                "loading a .qsc with an OBJ surface must populate the surface slot"
            );
            let (w, h) = (900u32, 700u32);
            let mvp = camera
                .view_projection(w as f32 / h as f32)
                .to_cols_array_2d();
            if let Some(png) = crate::offscreen::render_snapshot_png(
                &app.snapshot,
                mvp,
                w,
                h,
                [0.02, 0.02, 0.05, 1.0],
            ) {
                let path = out_dir.join("qsc_surface_3d.png");
                std::fs::write(&path, png).expect("write qsc_surface_3d.png");
                eprintln!("snapshot written to {}", path.display());
            }
        }
    }

    fn save_harness(harness: &mut egui_kittest::Harness<'_>, path: &std::path::Path) {
        match harness.render() {
            Ok(image) => {
                image.save(path).expect("save snapshot PNG");
                eprintln!("snapshot written to {}", path.display());
            }
            Err(err) => eprintln!(
                "no GPU adapter for kittest, skipping {}: {err}",
                path.display()
            ),
        }
    }
}
