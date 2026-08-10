// AnimKind/AnimObject + the `anim_spin_direction` timeline stepper are shared via core
// so both desktop front ends drive the animation timeline identically.
use crate::{
    anim_step, AnimKind, AnimObject, DesktopAppState, GuiAtomLabel, GuiLoadedPath,
    GuiSequenceSummary, GuiStructObject, WorkspaceCameraClipboard, WorkspaceObjectClipboard,
    WorkspaceRendererClipboard, WorkspaceStyleClipboard,
};
use ab_glyph::{point, Font, FontRef, ScaleFont};
use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_gfx::Rgba8;
use cuemol_qsys::{StereoMode, View};
use cuemol_render::{
    tessellate_snapshot, MeshVertex, PovSceneExporter, RenderAtom, RenderBond, RenderSceneSnapshot,
    UmbreonDenoiseMode, UmbreonSceneExporter,
};
use egui_wgpu::ScreenDescriptor;
use glam::{Mat4, Quat, Vec3, Vec4 as GlamVec4};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const RENDER_PRESETS: &[(&str, u32, u32)] = &[
    ("HD 720", 1280, 720),
    ("Full HD", 1920, 1080),
    ("Square 2K", 2048, 2048),
    ("Print 4K", 3840, 2160),
];
const ANIMATION_MOVIE_FORMATS: &[(&str, &str)] = &[
    ("mov_h264", "QuickTime/H.264"),
    ("mov_h265", "QuickTime/H.265"),
    ("mov_raw", "QuickTime/Raw"),
    ("mp4_h264", "MP4/H.264"),
    ("mp4_h265", "MP4/H.265"),
    ("wmv2", "WMV"),
    ("gifanim", "GIF animation"),
];
const POV_RADIOSITY_MODES: &[(i32, &str)] = &[
    (-1, "Disable"),
    (0, "Default"),
    (1, "Debug"),
    (2, "Fast"),
    (3, "Normal"),
    (4, "2Bounce"),
    (5, "Final"),
    (6, "OutdoorLQ"),
    (7, "OutdoorHQ"),
    (8, "OutdoorLight"),
    (9, "IndoorLQ"),
    (10, "IndoorHQ"),
];
const MOUSE_PRESETS: &[(&str, &str)] = &[
    ("DefaultViewInConf", "Default"),
    ("CootViewInConf", "Coot"),
    ("PymolViewInConf", "Pymol"),
    ("IphoneViewInConf", "iPhone"),
    ("TrackpadViewInConf", "Trackpad"),
];
const RENDERER_TYPES: &[&str] = crate::MOLECULE_RENDERER_TYPES;
const WORKSPACE_PAINT_MONOCHROME: &[(&str, &str)] = &[
    ("White", "#FFF"),
    ("75% Gray", "rgb(0.75,0.75,0.75)"),
    ("50% Gray", "rgb(0.5,0.5,0.5)"),
    ("25% Gray", "rgb(0.25,0.25,0.25)"),
    ("Black", "#000"),
];
const WORKSPACE_PAINT_HUES: &[(&str, i32)] = &[
    ("Red", 0),
    ("Orange", 30),
    ("Yellow", 60),
    ("Green", 120),
    ("Cyan", 180),
    ("Blue", 240),
    ("Purple", 300),
];
const DENSITY_RENDERER_TYPES: &[&str] = &["contour", "isosurf", "gpu_mapvol"];
const SURFACE_RENDERER_TYPES: &[&str] = &["molsurf"];
const LEGACY_MENU_SURFACES: &[&str] = &[
    "About CueMol2",
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
    "Hardware stereo",
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
    let mut runner = NativeRunner::new(event_loop.create_proxy());
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
    gpu_init_receiver: Option<mpsc::Receiver<Result<GpuState>>>,
    event_proxy: EventLoopProxy<()>,
    app: GuiApplication,
    init_error: Option<CueMolError>,
    started_at: Instant,
    exit_after: Option<Duration>,
}

impl NativeRunner {
    fn new(event_proxy: EventLoopProxy<()>) -> Self {
        Self {
            window: None,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            gpu: None,
            gpu_init_receiver: None,
            event_proxy,
            app: GuiApplication::new_persistent(),
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

    fn user_event(&mut self, event_loop: &ActiveEventLoop, (): ()) {
        let _ = self.guard_gui_callback(event_loop, "user_event", |this, event_loop| {
            this.poll_gpu_init(event_loop);
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
            .with_title("CueMol2")
            .with_inner_size(PhysicalSize::new(1024, 768));
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
        self.egui_state = Some(egui_state);
        self.window = Some(window.clone());

        let gpu_bootstrap = match GpuBootstrap::new(window.clone()) {
            Ok(bootstrap) => bootstrap,
            Err(error) => {
                self.init_error = Some(error);
                event_loop.exit();
                return;
            }
        };
        let (sender, receiver) = mpsc::channel();
        let proxy = self.event_proxy.clone();
        match std::thread::Builder::new()
            .name("cuemol-gpu-init".to_string())
            .spawn(move || {
                let result = pollster::block_on(GpuState::new(gpu_bootstrap));
                let _ = sender.send(result);
                let _ = proxy.send_event(());
            }) {
            Ok(_) => self.gpu_init_receiver = Some(receiver),
            Err(error) => {
                self.init_error = Some(CueMolError::Io(format!(
                    "failed to start GPU initialization: {error}"
                )));
                event_loop.exit();
            }
        }
    }

    fn poll_gpu_init(&mut self, event_loop: &ActiveEventLoop) {
        let result = self
            .gpu_init_receiver
            .as_ref()
            .map(mpsc::Receiver::try_recv);
        match result {
            Some(Ok(Ok(mut gpu))) => {
                self.gpu_init_receiver = None;
                if let Some(window) = &self.window {
                    gpu.resize(window.inner_size());
                    window.request_redraw();
                }
                self.gpu = Some(gpu);
            }
            Some(Ok(Err(error))) => {
                self.gpu_init_receiver = None;
                self.init_error = Some(error);
                event_loop.exit();
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.gpu_init_receiver = None;
                self.init_error = Some(CueMolError::Io(
                    "GPU initialization thread stopped".to_string(),
                ));
                event_loop.exit();
            }
            Some(Err(mpsc::TryRecvError::Empty)) | None => {}
        }
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
        if matches!(event, WindowEvent::CloseRequested) {
            std::process::exit(0);
        }
        if matches!(event, WindowEvent::RedrawRequested) && self.should_exit_now() {
            event_loop.exit();
            return;
        }
        let Some(egui_state) = self.egui_state.as_mut() else {
            return;
        };
        let response = egui_state.on_window_event(window.as_ref(), &event);
        let mut needs_redraw = response.repaint;
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
        if self.app.handle_window_event(&event, &mut gpu.camera) {
            needs_redraw = true;
        }
        match event {
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
                    GpuSceneFrame {
                        snapshot: &self.app.snapshot,
                        revision: self.app.snapshot_revision,
                        background: self.app.background.clear_color(),
                        viewport: self.app.viewport_rect,
                    },
                    &paint_jobs,
                    &full_output.textures_delta,
                    full_output.pixels_per_point,
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
        self.poll_gpu_init(event_loop);
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

#[derive(Debug, Clone, PartialEq)]
struct NativeCrystalDraft {
    object_uid: cuemol_core::Uid,
    lattice: String,
    space_group_number: i32,
    restrict_by_symmetry: bool,
    a: f64,
    b: f64,
    c: f64,
    alpha: f64,
    beta: f64,
    gamma: f64,
}

const CRYSTAL_LATTICES: [(&str, &str); 7] = [
    ("TRICLINIC", "Triclinic"),
    ("MONOCLINIC", "Monoclinic"),
    ("ORTHORHOMBIC", "Orthorhombic"),
    ("TRIGONAL", "Trigonal"),
    ("TETRAGONAL", "Tetragonal"),
    ("HEXAGONAL", "Hexagonal"),
    ("CUBIC", "Cubic"),
];

const SURFACE_CUT_MODES: [(&str, &str); 4] = [
    ("full", "Complete"),
    ("separate", "Separately"),
    ("sect", "Section only"),
    ("body", "No section"),
];

fn lattice_label(lattice: &str) -> &str {
    CRYSTAL_LATTICES
        .iter()
        .find(|entry| entry.0 == lattice)
        .map_or(lattice, |entry| entry.1)
}

fn surface_cut_mode_label(mode: &str) -> &str {
    SURFACE_CUT_MODES
        .iter()
        .find(|entry| entry.0 == mode)
        .map_or(mode, |entry| entry.1)
}

fn crystal_field_locks(
    draft: &NativeCrystalDraft,
    space_group_name: &str,
) -> (bool, bool, bool, bool, bool) {
    if !draft.restrict_by_symmetry {
        return (false, false, false, false, false);
    }
    match draft.lattice.as_str() {
        "MONOCLINIC" => (false, false, true, false, true),
        "ORTHORHOMBIC" => (false, false, true, true, true),
        "TETRAGONAL" => (true, false, true, true, true),
        "CUBIC" => (true, true, true, true, true),
        "TRIGONAL" if space_group_name.starts_with('R') => (true, true, false, false, false),
        "TRIGONAL" | "HEXAGONAL" => (true, false, true, true, true),
        _ => (false, false, false, false, false),
    }
}

fn restrict_crystal_draft(draft: &mut NativeCrystalDraft, space_group_name: &str) {
    if !draft.restrict_by_symmetry {
        return;
    }
    match draft.lattice.as_str() {
        "MONOCLINIC" => {
            draft.alpha = 90.0;
            draft.gamma = 90.0;
        }
        "ORTHORHOMBIC" => {
            draft.alpha = 90.0;
            draft.beta = 90.0;
            draft.gamma = 90.0;
        }
        "TETRAGONAL" => {
            draft.b = draft.a;
            draft.alpha = 90.0;
            draft.beta = 90.0;
            draft.gamma = 90.0;
        }
        "CUBIC" => {
            draft.b = draft.a;
            draft.c = draft.a;
            draft.alpha = 90.0;
            draft.beta = 90.0;
            draft.gamma = 90.0;
        }
        "TRIGONAL" if space_group_name.starts_with('R') => {
            draft.b = draft.a;
            draft.c = draft.a;
        }
        "TRIGONAL" | "HEXAGONAL" => {
            draft.b = draft.a;
            draft.alpha = 90.0;
            draft.beta = 90.0;
            draft.gamma = 120.0;
        }
        _ => {}
    }
}

type PdbDownloadMessage = std::result::Result<
    (
        String,
        Vec<crate::PdbDownloadedFile>,
        Option<crate::GuiRendererSetup>,
    ),
    String,
>;
type PdbProbeMessage = (
    crate::PdbFetchOptions,
    std::result::Result<Vec<crate::GuiPdbFileKind>, String>,
);

#[derive(Debug, Clone)]
enum NativeRendererSetupAction {
    PdbDownload,
    OpenPath(PathBuf),
}

#[derive(Debug, Clone)]
struct NativeRendererSetupDraft {
    action: NativeRendererSetupAction,
    reader_options: Option<crate::GuiMoleculeReaderOptions>,
    object_name: String,
    renderer_type: String,
    renderer_name: String,
    renderer_name_default: bool,
    selection_enabled: bool,
    selection: String,
    recenter: bool,
    options: Vec<crate::GuiRendererSetupOption>,
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
    pdb_history: Vec<String>,
    pdb_history_loaded: bool,
    pdb_fetch_coordinates: bool,
    pdb_coordinate_source: String,
    pdb_fetch_two_fofc: bool,
    pdb_fetch_fofc: bool,
    pdb_map_source: String,
    pdb_probe_options: Option<crate::PdbFetchOptions>,
    pdb_probe_deadline: Option<Instant>,
    pdb_probe_receiver: Option<mpsc::Receiver<PdbProbeMessage>>,
    pdb_probe_result: Option<std::result::Result<Vec<crate::GuiPdbFileKind>, String>>,
    renderer_setup_draft: Option<NativeRendererSetupDraft>,
    pdb_download_receiver: Option<mpsc::Receiver<PdbDownloadMessage>>,
    url_download_receiver: Option<mpsc::Receiver<std::result::Result<PathBuf, String>>>,
    script_path: String,
    console_input: String,
    selection_text: String,
    selected_object: Option<cuemol_core::Uid>,
    selected_renderer: Option<cuemol_core::Uid>,
    workspace_aux_selection: Option<WorkspaceAuxSelection>,
    editing_saved_camera: Option<(cuemol_core::Uid, String)>,
    camera_visibility_draft: Option<NativeCameraVisibilityDraft>,
    pending_camera_visibility_clear: Option<(cuemol_core::Uid, String)>,
    style_properties_draft: Option<NativeStyleProperties>,
    renderer_style_create_draft: Option<NativeRendererStyleCreate>,
    workspace_camera_clipboard: Option<WorkspaceCameraClipboard>,
    workspace_object_clipboard: Option<WorkspaceObjectClipboard>,
    workspace_renderer_clipboard: Option<WorkspaceRendererClipboard>,
    workspace_style_clipboard: Option<WorkspaceStyleClipboard>,
    pending_style_paste_scene: Option<cuemol_core::Uid>,
    /// Molecule chosen in the MolStruct panel's molecule selector (CueMol2
    /// `molstruct-panel.xul`); independent of the Workspace tree selection.
    molstruct_object: Option<cuemol_core::Uid>,
    /// MolStruct tree expand-all / collapse-all state. Folded into each node's
    /// persistent id so toggling recreates every chain/residue node with this as the
    /// default-open state (egui only honours `default_open` on first creation).
    molstruct_expand_all: bool,
    /// Immutable molecule-panel data is reused across egui redraws. Camera motion and
    /// animation frames do not alter hierarchy labels, so rebuilding them every frame
    /// only creates avoidable allocations for large molecules.
    molstruct_tree_cache: Option<(MoleculePanelCacheKey, Arc<Vec<GuiStructObject>>)>,
    sequence_summary_cache: Option<(MoleculePanelCacheKey, Arc<Vec<GuiSequenceSummary>>)>,
    /// Active tab of the Selection side panel (Command vs Editor).
    selection_tab: SelectionTab,
    /// Whether the crystallographic unit-cell box is drawn (Symmetry panel "Unit cell").
    show_unit_cell: bool,
    /// Whether the home-cell symmetry-mate copies are drawn (Symmetry panel "Symm mol").
    show_symm_mates: bool,
    crystal_draft: Option<NativeCrystalDraft>,
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
    /// Light vs dark UI theme (CueMol2's native XUL look is light by default).
    light_theme: bool,
    /// Force every left-dock accordion section open (an "expand all panels" aid, also
    /// used to snapshot the bodies inside the REAL narrow dock for layout review).
    force_open_all_panels: bool,
    /// Rainbow coloring hue endpoints (degrees) edited in the Coloring panel; the
    /// classic N→C blue→red gradient by default (start 240° = blue, end 0° = red).
    coloring_start_hue: f32,
    coloring_end_hue: f32,
    snapshot: RenderSceneSnapshot,
    /// Monotonic generation used to avoid rebuilding immutable GPU scene buffers
    /// during camera-only and egui-only redraws.
    snapshot_revision: u64,
    background: SceneBackground,
    color_proofing: bool,
    bottom_tab: BottomTab,
    render_preset: String,
    render_backend: String,
    umbreon_denoise: UmbreonDenoiseMode,
    ribbon_tab: RibbonTab,
    interaction_mode: InteractionMode,
    measure_mode: MeasureMode,
    show_toolbar: bool,
    show_workspace: bool,
    left_panel_visible: [bool; LeftPanel::ALL.len()],
    reset_left_panel_layout: bool,
    show_inspector: bool,
    object_properties_draft: Option<NativeObjectProperties>,
    renderer_properties_draft: Option<NativeRendererProperties>,
    show_bottom_panel: bool,
    show_about: bool,
    show_options: bool,
    options_pane: OptionsPane,
    options_draft: Option<NativePreferences>,
    show_scene_props: bool,
    scene_properties_draft: Option<NativeSceneProperties>,
    scene_property_selected: usize,
    show_view_props: bool,
    view_properties_draft: Option<NativeViewProperties>,
    view_property_selected: usize,
    show_chain_dialog: bool,
    show_residue_dialog: bool,
    show_renderer_dialog: bool,
    new_renderer_owner: Option<NativePropertyObject>,
    new_renderer_group: Option<String>,
    new_renderer_type: String,
    new_renderer_name: String,
    chain_edit_value: String,
    residue_start_value: String,
    tool_dialog: Option<String>,
    show_get_pdb: bool,
    show_console: bool,
    viewport_rect: Option<egui::Rect>,
    pixels_per_point: f32,
    shift_down: bool,
    ctrl_down: bool,
    rect_select_start: Option<egui::Pos2>,
    rect_select_current: Option<egui::Pos2>,
    viewport_menu: Option<ViewportMenu>,
    perf_measure: bool,
    animation_playing: bool,
    animation_loop: bool,
    /// Animation timeline objects (CueMol2 `anim-panel` list) + the elapsed playback
    /// time in seconds while playing.
    animation_objects: Vec<AnimObject>,
    animation_elapsed: f32,
    animation_render_settings: AnimationRenderSettings,
    animation_render_job: Option<AnimationRenderJob>,
    animation_render_log: Vec<String>,
    animation_render_total_frames: u32,
    animation_preview_frame: u32,
    animation_preview_texture: Option<AnimationPreviewTexture>,
    superpose_reference: Option<cuemol_core::Uid>,
    superpose_moving: Option<cuemol_core::Uid>,
    superpose_algorithm: String,
    superpose_reference_selection: String,
    superpose_moving_selection: String,
    superpose_auto_recenter: bool,
    superpose_use_property: bool,
    bond_object: Option<cuemol_core::Uid>,
    bond_picker_target: Option<usize>,
    selected_nonstandard_bonds: std::collections::BTreeSet<(i32, i32)>,
    bond_aid1: String,
    bond_aid2: String,
    contact_object: Option<cuemol_core::Uid>,
    contact_selection1: String,
    contact_use_object2: bool,
    contact_object2: Option<cuemol_core::Uid>,
    contact_use_selection2: bool,
    contact_selection2: String,
    contact_r_min: f64,
    contact_r_max: f64,
    contact_hbond: bool,
    contact_max_count: usize,
    contact_renderer_name: String,
    secondary_ignore_breaks: bool,
    secondary_gap_fill: bool,
    secondary_angle: f64,
    surface_path: String,
    surface_density: f64,
    surface_probe_radius: f64,
    cut_surface_target: Option<cuemol_core::Uid>,
    cut_surface_mode: String,
    cut_surface_density: f64,
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
    modified_scenes: std::collections::BTreeSet<cuemol_core::Uid>,
    status: String,
    log_lines: Vec<String>,
    selection_history: Vec<String>,
}

#[derive(Debug, Clone)]
struct GuiUndoSnapshot {
    state: DesktopAppState,
    selected_object: Option<cuemol_core::Uid>,
    selected_renderer: Option<cuemol_core::Uid>,
    workspace_aux_selection: Option<WorkspaceAuxSelection>,
    selection_text: String,
    last_loaded_path: Option<PathBuf>,
    modified_scenes: std::collections::BTreeSet<cuemol_core::Uid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MoleculePanelCacheKey {
    scene_uid: Option<cuemol_core::Uid>,
    objects: Vec<(cuemol_core::Uid, String, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimationRenderTab {
    Main,
    Render,
    Movie,
    Preview,
}

#[derive(Debug, Clone)]
struct AnimationRenderSettings {
    tab: AnimationRenderTab,
    output_dir: String,
    base_name: String,
    width: u32,
    height: u32,
    perspective: bool,
    fps: u32,
    concurrency: u32,
    duplicate_last_frame: bool,
    backend: String,
    povray_executable: String,
    povray_include_dir: String,
    pov_clip_plane: bool,
    pov_post_blend: bool,
    pov_shadow: bool,
    pov_edge_lines: bool,
    pov_labels: bool,
    pov_radiosity: i32,
    make_movie: bool,
    ffmpeg_executable: String,
    movie_format: String,
    bitrate_kbps: u32,
}

impl Default for AnimationRenderSettings {
    fn default() -> Self {
        Self {
            tab: AnimationRenderTab::Main,
            output_dir: ".".to_string(),
            base_name: "output".to_string(),
            width: 640,
            height: 480,
            perspective: true,
            fps: 30,
            concurrency: default_animation_render_concurrency(),
            duplicate_last_frame: false,
            backend: "POV-Ray".to_string(),
            povray_executable: "povray".to_string(),
            povray_include_dir: String::new(),
            pov_clip_plane: true,
            pov_post_blend: true,
            pov_shadow: false,
            pov_edge_lines: true,
            pov_labels: false,
            pov_radiosity: -1,
            make_movie: false,
            ffmpeg_executable: "ffmpeg".to_string(),
            movie_format: "mp4_h264".to_string(),
            bitrate_kbps: 10_240,
        }
    }
}

#[derive(Debug)]
enum AnimationTaskOutput {
    Frame {
        index: u32,
        path: PathBuf,
        log: String,
    },
    Movie {
        path: PathBuf,
        log: String,
    },
}

#[derive(Debug)]
struct AnimationRenderJob {
    settings: AnimationRenderSettings,
    restore: AnimationRenderRestore,
    total_frames: u32,
    next_frame: u32,
    completed_frames: u32,
    frame_paths: Vec<PathBuf>,
    frame_receivers: Vec<mpsc::Receiver<std::result::Result<AnimationTaskOutput, String>>>,
    movie_receiver: Option<mpsc::Receiver<std::result::Result<AnimationTaskOutput, String>>>,
    progress: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
    encoding_started: bool,
}

#[derive(Debug)]
struct AnimationRenderRestore {
    renderer_visibility: Vec<(cuemol_core::Uid, cuemol_core::Uid, bool)>,
    trajectory_frames: Vec<(cuemol_core::Uid, usize)>,
    camera: CameraController,
    elapsed: f32,
}

struct AnimationFrameTask {
    settings: AnimationRenderSettings,
    index: u32,
    total_frames: u32,
    snapshot: RenderSceneSnapshot,
    camera: cuemol_qsys::Camera,
    labels: Vec<GuiAtomLabel>,
    background: [f32; 4],
    progress: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
    denoise: UmbreonDenoiseMode,
}

struct AnimationMovieTask {
    settings: AnimationRenderSettings,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU32>,
}

struct AnimationPreviewTexture {
    path: PathBuf,
    texture: egui::TextureHandle,
    size: egui::Vec2,
}

impl std::fmt::Debug for AnimationPreviewTexture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AnimationPreviewTexture")
            .field("path", &self.path)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneBackground {
    White,
    Black,
    Custom(Rgba8),
}

impl SceneBackground {
    fn parse(value: &str) -> std::result::Result<Self, String> {
        let rgb = |r, g, b| Self::Custom(Rgba8 { r, g, b, a: 255 });
        let named = match value.trim().to_ascii_lowercase().as_str() {
            "white" => Ok(Self::White),
            "black" => Ok(Self::Black),
            "red" => Ok(rgb(255, 0, 0)),
            "green" => Ok(rgb(0, 128, 0)),
            "lime" => Ok(rgb(0, 255, 0)),
            "blue" => Ok(rgb(0, 0, 255)),
            "yellow" => Ok(rgb(255, 255, 0)),
            "cyan" | "aqua" => Ok(rgb(0, 255, 255)),
            "magenta" | "fuchsia" => Ok(rgb(255, 0, 255)),
            "gray" | "grey" => Ok(rgb(128, 128, 128)),
            "orange" => Ok(rgb(255, 165, 0)),
            _ => {
                return cuemol_gfx::parse_color(value.trim())
                    .map_err(|error| error.to_string())?
                    .rgba8()
                    .map(Self::Custom)
                    .ok_or_else(|| format!("unresolved background color: {value}"))
            }
        };
        named
    }

    fn clear_color(self) -> wgpu::Color {
        let rgba = self.rgba_f32();
        wgpu::Color {
            r: f64::from(rgba[0]),
            g: f64::from(rgba[1]),
            b: f64::from(rgba[2]),
            a: f64::from(rgba[3]),
        }
    }

    fn rgba_f32(self) -> [f32; 4] {
        match self {
            Self::White => [1.0, 1.0, 1.0, 1.0],
            Self::Black => [0.0, 0.0, 0.0, 1.0],
            Self::Custom(rgba) => [
                f32::from(rgba.r) / 255.0,
                f32::from(rgba.g) / 255.0,
                f32::from(rgba.b) / 255.0,
                f32::from(rgba.a) / 255.0,
            ],
        }
    }

    fn foreground(self, dark: egui::Color32, light: egui::Color32) -> egui::Color32 {
        let [red, green, blue, _] = self.rgba_f32();
        let luminance = red * 0.2126 + green * 0.7152 + blue * 0.0722;
        if luminance >= 0.5 {
            dark
        } else {
            light
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

/// Ribbon toolbar tabs mirroring CueMol2's base ribbon plus `anim-ribbon.xul`.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeFileDialog {
    OpenObject,
    OpenScene,
    OpenCamera,
    OpenStyle,
    SaveObject,
    SaveScene,
    SaveView,
    SaveCamera,
    SaveStyle,
    ExportScene,
    OpenScript,
    SaveLog,
    MergeObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainShortcutCommand {
    OpenScene,
    NewTab,
    NewWindow,
    OpenFile,
    ReloadScene,
    SaveScene,
    Undo,
    Redo,
    Config,
    Performance,
}

impl MainShortcutCommand {
    const ALL: [Self; 10] = [
        Self::OpenScene,
        Self::NewTab,
        Self::NewWindow,
        Self::OpenFile,
        Self::ReloadScene,
        Self::SaveScene,
        Self::Undo,
        Self::Redo,
        Self::Config,
        Self::Performance,
    ];

    fn shortcut(self) -> egui::KeyboardShortcut {
        let (mut modifiers, key) = match self {
            Self::OpenScene => (egui::Modifiers::COMMAND, egui::Key::O),
            Self::NewTab => (egui::Modifiers::COMMAND, egui::Key::T),
            Self::NewWindow => (egui::Modifiers::COMMAND, egui::Key::N),
            Self::OpenFile => (egui::Modifiers::COMMAND, egui::Key::O),
            Self::ReloadScene => (egui::Modifiers::COMMAND, egui::Key::R),
            Self::SaveScene => (egui::Modifiers::COMMAND, egui::Key::S),
            Self::Undo => (egui::Modifiers::COMMAND, egui::Key::Z),
            Self::Redo => (egui::Modifiers::COMMAND, egui::Key::Y),
            Self::Config => (egui::Modifiers::COMMAND, egui::Key::K),
            Self::Performance => (egui::Modifiers::NONE, egui::Key::F2),
        };
        if self == Self::OpenScene {
            modifiers.shift = true;
        }
        egui::KeyboardShortcut::new(modifiers, key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeFileFilter {
    label: &'static str,
    extensions: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeFileDialogSpec {
    title: &'static str,
    filters: &'static [NativeFileFilter],
    default_extension: Option<&'static str>,
    save: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptionsPane {
    Misc,
    Key,
    Mouse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeShortcut {
    id: String,
    key: String,
    ctrl: bool,
    alt: bool,
    shift: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct NativePreferences {
    atom_label_font: String,
    atom_label_size: f64,
    atom_label_color: String,
    atom_label_italic: bool,
    atom_label_bold: bool,
    ui_language: String,
    use_hidpi: bool,
    mouse_preset: String,
    trackball_radius: f64,
    hit_precision: f64,
    momentum_scroll: bool,
    multitouch_pad: bool,
    emulate_right_button: bool,
    shortcuts: Vec<NativeShortcut>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum NativePropertyObject {
    Molecule(cuemol_core::Uid),
    DensityMap(cuemol_core::Uid),
    Surface(cuemol_core::Uid),
}

impl NativePropertyObject {
    fn uid(self) -> cuemol_core::Uid {
        match self {
            Self::Molecule(uid) | Self::DensityMap(uid) | Self::Surface(uid) => uid,
        }
    }

    fn workspace_kind(self) -> crate::GuiWorkspaceObjectKind {
        match self {
            Self::Molecule(_) => crate::GuiWorkspaceObjectKind::Molecule,
            Self::DensityMap(_) => crate::GuiWorkspaceObjectKind::DensityMap,
            Self::Surface(_) => crate::GuiWorkspaceObjectKind::Surface,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceAuxSelection {
    CameraRoot {
        scene_uid: cuemol_core::Uid,
    },
    Camera {
        scene_uid: cuemol_core::Uid,
        camera_index: usize,
    },
    StyleRoot {
        scene_uid: cuemol_core::Uid,
    },
    Style {
        scene_uid: cuemol_core::Uid,
        style_index: usize,
    },
    DensityMap {
        scene_uid: cuemol_core::Uid,
        map_uid: cuemol_core::Uid,
        renderer_uid: Option<cuemol_core::Uid>,
    },
    Surface {
        scene_uid: cuemol_core::Uid,
        surface_uid: cuemol_core::Uid,
        renderer_uid: Option<cuemol_core::Uid>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeStyleProperties {
    scene_uid: cuemol_core::Uid,
    local_index: Option<usize>,
    name: String,
    read_only: bool,
    xml: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeCameraVisibilityDraft {
    scene_uid: cuemol_core::Uid,
    camera_name: String,
    entries: Vec<crate::GuiCameraVisibilityEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeRendererStyleCreate {
    scene_uid: cuemol_core::Uid,
    object_uid: cuemol_core::Uid,
    renderer_uid: cuemol_core::Uid,
    renderer_name: String,
    renderer_type: String,
    style_set_index: Option<usize>,
    name_prefix: String,
    overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeRendererTreeNode {
    renderer: crate::GuiRendererSummary,
    children: Vec<crate::GuiRendererSummary>,
}

#[derive(Debug, Clone, PartialEq)]
enum NativeWorkspaceObjectSummary {
    Molecule(crate::GuiObjectSummary),
    DensityMap(crate::GuiDensityMapSummary),
    Surface(crate::GuiLoadedSurfaceSummary),
}

impl NativeWorkspaceObjectSummary {
    fn sort_key(&self) -> (i32, crate::GuiWorkspaceObjectKind, cuemol_core::Uid) {
        match self {
            Self::Molecule(summary) => (
                summary.ui_order,
                crate::GuiWorkspaceObjectKind::Molecule,
                summary.uid,
            ),
            Self::DensityMap(summary) => (
                summary.ui_order,
                crate::GuiWorkspaceObjectKind::DensityMap,
                summary.uid,
            ),
            Self::Surface(summary) => (
                summary.ui_order,
                crate::GuiWorkspaceObjectKind::Surface,
                summary.uid,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeWorkspaceObjectDragPayload {
    scene_uid: cuemol_core::Uid,
    owner: NativePropertyObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeRendererDragPayload {
    object_uid: cuemol_core::Uid,
    renderer_uid: cuemol_core::Uid,
    is_group: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeObjectProperties {
    target: NativePropertyObject,
    name: String,
    visible: bool,
    locked: bool,
    linked: String,
}

#[derive(Debug, Clone, PartialEq)]
struct NativeRendererProperties {
    owner: NativePropertyObject,
    renderer_uid: cuemol_core::Uid,
    renderer_type: String,
    selected_tab: usize,
    name: String,
    selection: String,
    visible: bool,
    locked: bool,
    style: String,
    material: String,
    alpha: f64,
    edge_type: String,
    edge_width: f64,
    edge_color: String,
    line_width: f64,
    detail: i32,
    bond_width: f64,
    sphere_radius: f64,
    show_ring: bool,
    ring_thickness: f64,
    ring_color: String,
    specific: std::collections::BTreeMap<String, String>,
}

impl NativeRendererProperties {
    fn from_state(
        state: &DesktopAppState,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) -> Option<Self> {
        let object = state.scene_manager().object(object_uid)?;
        let renderer = object.renderer(renderer_uid)?;
        let renderer_type = renderer.renderer_type().to_string();
        let numeric = |name: &str, fallback: f64| {
            renderer
                .qsc_attr(name)
                .and_then(|value| value.parse().ok())
                .unwrap_or(fallback)
        };
        let selection = if matches!(renderer_type.as_str(), "*namelabel" | "name_label") {
            String::new()
        } else if matches!(renderer_type.as_str(), "*selection" | "selection") {
            let selection = object.mol().selection_text();
            if selection.is_empty() {
                "!*".to_string()
            } else {
                selection.to_string()
            }
        } else {
            renderer.qsc_attr("sel").unwrap_or("*").to_string()
        };
        let styles = state
            .scene_manager()
            .scene_uid_for_object(object_uid)
            .and_then(|scene_uid| state.scene_manager().scene(scene_uid))
            .map(cuemol_molvis::scene_style_manager)
            .unwrap_or_else(cuemol_molvis::default_style_manager);
        let mut specific = renderer.qsc_attrs().clone();
        for (key, fallback) in native_renderer_specific_defaults(&renderer_type) {
            let resolved = if let Some((child, attribute)) = key.split_once('.') {
                if attribute == "type" {
                    renderer.resolved_child_type(child, &styles, cuemol_molvis::DEFAULT_STYLE_SCOPE)
                } else {
                    renderer.resolved_child_attr(
                        child,
                        attribute,
                        &styles,
                        cuemol_molvis::DEFAULT_STYLE_SCOPE,
                    )
                }
            } else {
                renderer.resolved_attr(key, &styles, cuemol_molvis::DEFAULT_STYLE_SCOPE)
            };
            specific.insert(
                (*key).to_string(),
                resolved.unwrap_or_else(|| (*fallback).to_string()),
            );
        }
        Some(Self {
            owner: NativePropertyObject::Molecule(object_uid),
            renderer_uid,
            renderer_type,
            selected_tab: 0,
            name: renderer.name().to_string(),
            selection,
            visible: renderer.is_visible(),
            locked: renderer.qsc_attr("locked").is_some_and(value_is_true),
            style: renderer.style().unwrap_or_default().to_string(),
            material: renderer.qsc_attr("material").unwrap_or("").to_string(),
            alpha: numeric("alpha", 1.0),
            edge_type: renderer.qsc_attr("egtype").unwrap_or("none").to_string(),
            edge_width: numeric("eglinew", 0.01),
            edge_color: renderer
                .qsc_attr("egcolor")
                .unwrap_or("#000000")
                .to_string(),
            line_width: numeric(
                if renderer.renderer_type() == "spline" {
                    "line_width"
                } else {
                    "width"
                },
                1.2,
            ),
            detail: numeric("detail", 3.0) as i32,
            bond_width: numeric("bondw", 0.2),
            sphere_radius: numeric("sphr", 0.3),
            show_ring: renderer.qsc_attr("ring").is_some_and(value_is_true),
            ring_thickness: numeric("thickness", 0.2),
            ring_color: renderer
                .qsc_attr("ringcolor")
                .unwrap_or("#ffffbf")
                .to_string(),
            specific,
        })
    }

    fn from_density_state(
        state: &DesktopAppState,
        map_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) -> Option<Self> {
        let renderer = state
            .density_map_summary(map_uid)?
            .renderers
            .into_iter()
            .find(|renderer| renderer.uid == renderer_uid)?;
        let style = renderer.specific.get("style").cloned().unwrap_or_default();
        Some(Self::from_workspace_summary(
            NativePropertyObject::DensityMap(map_uid),
            renderer.uid,
            renderer.renderer_type,
            renderer.name,
            String::new(),
            renderer.visible,
            renderer.locked,
            style,
            renderer.material,
            renderer.alpha,
            renderer.edge_type,
            renderer.edge_width,
            renderer.edge_color,
            renderer.specific,
        ))
    }

    fn from_surface_state(
        state: &DesktopAppState,
        surface_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) -> Option<Self> {
        let renderer = state
            .surface_summary(surface_uid)?
            .renderers
            .into_iter()
            .find(|renderer| renderer.uid == renderer_uid)?;
        let style = renderer.specific.get("style").cloned().unwrap_or_default();
        Some(Self::from_workspace_summary(
            NativePropertyObject::Surface(surface_uid),
            renderer.uid,
            renderer.renderer_type,
            renderer.name,
            renderer.selection,
            renderer.visible,
            renderer.locked,
            style,
            renderer.material,
            renderer.alpha,
            renderer.edge_type,
            renderer.edge_width,
            renderer.edge_color,
            renderer.specific,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn from_workspace_summary(
        owner: NativePropertyObject,
        renderer_uid: cuemol_core::Uid,
        renderer_type: String,
        name: String,
        selection: String,
        visible: bool,
        locked: bool,
        style: String,
        material: String,
        alpha: f64,
        edge_type: String,
        edge_width: f64,
        edge_color: String,
        mut specific: std::collections::BTreeMap<String, String>,
    ) -> Self {
        strip_native_renderer_common_specific(&mut specific);
        for (key, fallback) in native_renderer_specific_defaults(&renderer_type) {
            specific
                .entry((*key).to_string())
                .or_insert_with(|| (*fallback).to_string());
        }
        Self {
            owner,
            renderer_uid,
            renderer_type,
            selected_tab: 0,
            name,
            selection,
            visible,
            locked,
            style,
            material,
            alpha,
            edge_type,
            edge_width,
            edge_color,
            line_width: 1.2,
            detail: 3,
            bond_width: 0.2,
            sphere_radius: 0.3,
            show_ring: false,
            ring_thickness: 0.2,
            ring_color: "#ffffbf".to_string(),
            specific,
        }
    }

    fn is_density_renderer(&self) -> bool {
        matches!(self.owner, NativePropertyObject::DensityMap(_))
    }

    fn is_renderer_group(&self) -> bool {
        self.renderer_type == "*group"
    }

    fn title(&self) -> &'static str {
        match self.renderer_type.as_str() {
            "*group" => "Renderer group properties",
            "simple" => "Simple Rend Properties",
            "trace" => "Trace renderer properties",
            "spline" => "Spline renderer properties",
            "*namelabel" | "name_label" => "Name label renderer properties",
            "*selection" | "selection" => "Selection renderer properties",
            "atomintr" => "Interaction renderer properties",
            "anisou" => "Anisotropic U renderer properties",
            "ballstick" => "Ballstick renderer properties",
            "cpk" => "CPK Rend Properties",
            "disorder" => "Disorder renderer properties",
            "tube" => "Tube Rend Properties",
            "nucl" => "Nucl renderer properties",
            "ribbon" => "Ribbon representation properties",
            "cartoon" => "Cartoon representation properties",
            "contour" | "gpu_mapmesh" => "Contour renderer properties",
            "isosurf" => "Isosurf renderer properties",
            "gpu_mapvol" => "Volume renderer properties",
            "surface" | "molsurf" | "dsurface" | "directsurf" | "dsurf2" => {
                "MolSurf Rend Properties"
            }
            _ => "Renderer properties",
        }
    }

    fn tabs(&self) -> &'static [&'static str] {
        match self.renderer_type.as_str() {
            "*group" => &["Common"],
            "simple" => &["Simple"],
            "trace" => &["Trace"],
            "spline" => &["Common", "Spline"],
            "*namelabel" | "name_label" => &["Label"],
            "*selection" | "selection" => &["Selection"],
            "atomintr" => &["Common", "Interaction"],
            "anisou" => &["Common", "Ball & Stick", "Anisotropic U"],
            "ballstick" => &["Common", "Ball & Stick"],
            "cpk" => &["Common", "Atom radii"],
            "disorder" => &["Common", "Disorder"],
            "tube" => &["Common", "Tube"],
            "nucl" => &["Common", "Nucleic acid", "Tube"],
            "ribbon" => &["Common", "Helix", "Sheet", "Coil"],
            "cartoon" => &["Common", "Cartoon", "Helix", "Sheet", "Coil"],
            "contour" | "gpu_mapmesh" | "isosurf" | "gpu_mapvol" => &["Common", "Map"],
            "surface" | "molsurf" | "dsurface" | "directsurf" | "dsurf2" => &["Common", "MolSurf"],
            _ => &["Common"],
        }
    }

    fn specific_value(&self, key: &str, fallback: &str) -> String {
        self.specific
            .get(key)
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }
}

impl NativeObjectProperties {
    fn from_state(state: &DesktopAppState, object_uid: cuemol_core::Uid) -> Option<Self> {
        let object = state.scene_manager().object(object_uid)?;
        let source = object.source();
        let linked = if source.is_empty() || source.starts_with("datachunk:") {
            "(embeded)".to_string()
        } else if object.source_type().is_empty() {
            source.to_string()
        } else {
            format!("{source}  ({})", object.source_type())
        };
        Some(Self {
            target: NativePropertyObject::Molecule(object_uid),
            name: object.name().to_string(),
            visible: object.is_visible(),
            locked: object.is_ui_locked(),
            linked,
        })
    }

    fn from_density_state(state: &DesktopAppState, map_uid: cuemol_core::Uid) -> Option<Self> {
        let map = state.density_map_summary(map_uid)?;
        Some(Self {
            target: NativePropertyObject::DensityMap(map_uid),
            name: map.name,
            visible: map.visible,
            locked: map.locked,
            linked: workspace_linked_text(&map.source, &map.format),
        })
    }

    fn from_surface_state(state: &DesktopAppState, surface_uid: cuemol_core::Uid) -> Option<Self> {
        let surface = state.surface_summary(surface_uid)?;
        Some(Self {
            target: NativePropertyObject::Surface(surface_uid),
            name: surface.name,
            visible: surface.visible,
            locked: surface.locked,
            linked: workspace_linked_text(&surface.source, &surface.format),
        })
    }
}

fn workspace_linked_text(source: &str, source_type: &str) -> String {
    if source.is_empty() || source.starts_with("datachunk:") {
        "(embeded)".to_string()
    } else if source_type.is_empty() {
        source.to_string()
    } else {
        format!("{source}  ({source_type})")
    }
}

fn workspace_lock_toggle(ui: &mut egui::Ui, locked: bool, enabled: bool) -> Option<bool> {
    let label = if locked { "🔒" } else { "🔓" };
    ui.add_enabled(enabled, egui::Button::new(label).frame(false))
        .on_hover_text(if locked { "Unlock" } else { "Lock" })
        .clicked()
        .then_some(!locked)
}

fn ctx_pointer_vertical_fraction(ctx: &egui::Context, rect: egui::Rect) -> f32 {
    let y = ctx
        .input(|input| input.pointer.latest_pos())
        .map_or(rect.center().y, |position| position.y);
    if rect.height() <= f32::EPSILON {
        0.5
    } else {
        ((y - rect.top()) / rect.height()).clamp(0.0, 1.0)
    }
}

fn strip_native_renderer_common_specific(
    specific: &mut std::collections::BTreeMap<String, String>,
) {
    for key in [
        "name", "sel", "visible", "locked", "style", "material", "alpha", "egtype", "eglinew",
        "egcolor",
    ] {
        specific.remove(key);
    }
}

fn native_renderer_tree_nodes(
    renderers: Vec<crate::GuiRendererSummary>,
) -> Vec<NativeRendererTreeNode> {
    let mut children = std::collections::BTreeMap::<String, Vec<_>>::new();
    for renderer in renderers.iter().filter(|renderer| renderer.group.is_some()) {
        children
            .entry(renderer.group.clone().unwrap_or_default())
            .or_default()
            .push(renderer.clone());
    }
    renderers
        .into_iter()
        .filter(|renderer| renderer.group.is_none())
        .map(|renderer| {
            let grouped = if renderer.renderer_type == "*group" {
                children.remove(&renderer.name).unwrap_or_default()
            } else {
                Vec::new()
            };
            NativeRendererTreeNode {
                renderer,
                children: grouped,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
struct NativeSceneProperties {
    uid: u64,
    modified: bool,
    name: String,
    source: String,
    source_type: String,
    base_path: String,
    background: String,
    ao_enabled: bool,
    ao_radius: f64,
    ao_intensity: f64,
    ao_slices: i32,
    ao_steps: i32,
    ao_half_res: bool,
    aa_method: String,
    aa_jitter_level: i32,
    onload: String,
    object_uids: String,
    active_object_id: u64,
    active_renderer_id: u64,
    view_uids: String,
    active_view_id: u64,
    use_color_proof: bool,
    icc_filename: String,
    icc_intent: String,
}

impl NativeSceneProperties {
    fn from_state(state: &DesktopAppState) -> Option<Self> {
        let uid = state.active_scene()?;
        let scene = state.scene_manager().scene(uid)?;
        let object_uids = scene
            .objects()
            .iter()
            .map(|object| object.uid().get().to_string())
            .collect::<Vec<_>>()
            .join(",");
        let view_uids = state
            .command_manager()
            .views()
            .iter()
            .filter(|view| view.scene_uid() == uid)
            .map(|view| view.uid().get().to_string())
            .collect::<Vec<_>>()
            .join(",");
        Some(Self {
            uid: uid.get(),
            modified: false,
            name: scene.name().to_string(),
            source: scene.source().to_string(),
            source_type: scene.source_type().to_string(),
            base_path: scene.base_path().to_string(),
            background: scene
                .scene_attribute("bgcolor")
                .unwrap_or("black")
                .to_string(),
            ao_enabled: scene_attr_bool(scene.scene_attribute("aoEnabled"), false),
            ao_radius: scene_attr_f64(scene.scene_attribute("aoRadius"), 4.0),
            ao_intensity: scene_attr_f64(scene.scene_attribute("aoIntensity"), 2.2),
            ao_slices: scene_attr_i32(scene.scene_attribute("aoSlices"), 9),
            ao_steps: scene_attr_i32(scene.scene_attribute("aoSteps"), 3),
            ao_half_res: scene_attr_bool(scene.scene_attribute("aoHalfRes"), false),
            aa_method: scene
                .scene_attribute("aa_method")
                .unwrap_or("fxaa")
                .to_string(),
            aa_jitter_level: scene_attr_i32(scene.scene_attribute("aaJitterLevel"), 0),
            onload: scene.scene_attribute("onload").unwrap_or("").to_string(),
            object_uids,
            active_object_id: scene.active_object_uid().map_or(0, cuemol_core::Uid::get),
            active_renderer_id: scene.active_renderer_uid().map_or(0, cuemol_core::Uid::get),
            view_uids,
            active_view_id: scene.active_view_uid().map_or(0, cuemol_core::Uid::get),
            use_color_proof: scene_attr_bool(scene.scene_attribute("use_colproof"), false),
            icc_filename: scene
                .scene_attribute("icc_filename")
                .unwrap_or("")
                .to_string(),
            icc_intent: scene
                .scene_attribute("icc_intent")
                .unwrap_or("perceptual")
                .to_string(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScenePropertyKey {
    Uid,
    Modified,
    Name,
    Source,
    SourceType,
    BasePath,
    Background,
    AoEnabled,
    AoRadius,
    AoIntensity,
    AoSlices,
    AoSteps,
    AoHalfRes,
    AaMethod,
    AaJitterLevel,
    Onload,
    ObjectUids,
    ActiveObjectId,
    ActiveRendererId,
    ViewUids,
    ActiveViewId,
    UseColorProof,
    IccFilename,
    IccIntent,
}

#[derive(Debug, Clone, Copy)]
struct ScenePropertyDefinition {
    key: ScenePropertyKey,
    name: &'static str,
    type_name: &'static str,
    readonly: bool,
    has_default: bool,
}

const SCENE_PROPERTY_DEFINITIONS: &[ScenePropertyDefinition] = &[
    scene_property(ScenePropertyKey::Uid, "uid", "integer", true, false),
    scene_property(
        ScenePropertyKey::Modified,
        "modified",
        "boolean",
        true,
        false,
    ),
    scene_property(ScenePropertyKey::Name, "name", "string", true, false),
    scene_property(ScenePropertyKey::Source, "src", "string", true, false),
    scene_property(
        ScenePropertyKey::SourceType,
        "srctype",
        "string",
        true,
        false,
    ),
    scene_property(
        ScenePropertyKey::BasePath,
        "basePath",
        "string",
        true,
        false,
    ),
    scene_property(
        ScenePropertyKey::Background,
        "bgcolor",
        "color",
        false,
        true,
    ),
    scene_property(
        ScenePropertyKey::AoEnabled,
        "aoEnabled",
        "boolean",
        false,
        true,
    ),
    scene_property(ScenePropertyKey::AoRadius, "aoRadius", "real", false, true),
    scene_property(
        ScenePropertyKey::AoIntensity,
        "aoIntensity",
        "real",
        false,
        true,
    ),
    scene_property(
        ScenePropertyKey::AoSlices,
        "aoSlices",
        "integer",
        false,
        true,
    ),
    scene_property(ScenePropertyKey::AoSteps, "aoSteps", "integer", false, true),
    scene_property(
        ScenePropertyKey::AoHalfRes,
        "aoHalfRes",
        "boolean",
        false,
        true,
    ),
    scene_property(ScenePropertyKey::AaMethod, "aa_method", "enum", false, true),
    scene_property(
        ScenePropertyKey::AaJitterLevel,
        "aaJitterLevel",
        "integer",
        false,
        true,
    ),
    scene_property(ScenePropertyKey::Onload, "onload", "string", false, true),
    scene_property(
        ScenePropertyKey::ObjectUids,
        "obj_uids",
        "string",
        true,
        false,
    ),
    scene_property(
        ScenePropertyKey::ActiveObjectId,
        "activeObjID",
        "integer",
        true,
        false,
    ),
    scene_property(
        ScenePropertyKey::ActiveRendererId,
        "activeRendID",
        "integer",
        true,
        false,
    ),
    scene_property(
        ScenePropertyKey::ViewUids,
        "view_uids",
        "string",
        true,
        false,
    ),
    scene_property(
        ScenePropertyKey::ActiveViewId,
        "activeViewID",
        "integer",
        true,
        false,
    ),
    scene_property(
        ScenePropertyKey::UseColorProof,
        "use_colproof",
        "boolean",
        false,
        true,
    ),
    scene_property(
        ScenePropertyKey::IccFilename,
        "icc_filename",
        "string",
        false,
        true,
    ),
    scene_property(
        ScenePropertyKey::IccIntent,
        "icc_intent",
        "enum",
        false,
        true,
    ),
];

const fn scene_property(
    key: ScenePropertyKey,
    name: &'static str,
    type_name: &'static str,
    readonly: bool,
    has_default: bool,
) -> ScenePropertyDefinition {
    ScenePropertyDefinition {
        key,
        name,
        type_name,
        readonly,
        has_default,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct NativeViewProperties {
    name: String,
    uid: u64,
    active: bool,
    zoom: f64,
    distance: f64,
    slab: f64,
    center: [f64; 3],
    rotation: [f64; 4],
    perspective: bool,
    width: u32,
    height: u32,
    trans_mms: bool,
    rot_mms: bool,
    stereo_mode: StereoMode,
    has_hw_stereo: bool,
    stereo_dist: f64,
    swap_stereo_eyes: bool,
    center_mark: CenterMark,
}

impl NativeViewProperties {
    fn from_camera(camera: &CameraController, viewport: Option<egui::Rect>) -> Self {
        let size = viewport.map_or(egui::Vec2::ZERO, |rect| rect.size());
        Self {
            name: camera.name.clone(),
            uid: 1,
            active: true,
            zoom: 1000.0 / f64::from(camera.ortho_scale.max(0.1)),
            distance: f64::from(camera.distance),
            slab: f64::from(camera.slab_depth),
            center: camera.target.to_array().map(f64::from),
            rotation: camera.orientation.to_array().map(f64::from),
            perspective: camera.perspective,
            width: size.x.max(0.0).round() as u32,
            height: size.y.max(0.0).round() as u32,
            trans_mms: camera.trans_mms,
            rot_mms: camera.rot_mms,
            stereo_mode: camera.stereo_mode,
            has_hw_stereo: false,
            stereo_dist: f64::from(camera.stereo_distance),
            swap_stereo_eyes: camera.swap_stereo_eyes,
            center_mark: camera.center_mark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewPropertyKey {
    Name,
    Uid,
    Active,
    Zoom,
    Distance,
    Slab,
    Center,
    CenterX,
    CenterY,
    CenterZ,
    Rotation,
    RotationX,
    RotationY,
    RotationZ,
    RotationW,
    Perspective,
    Width,
    Height,
    TransMms,
    RotMms,
    StereoMode,
    HasHwStereo,
    StereoDist,
    SwapStereoEyes,
    UpVec,
    UpVecX,
    UpVecY,
    UpVecZ,
    RightVec,
    RightVecX,
    RightVecY,
    RightVecZ,
    ForwardVec,
    ForwardVecX,
    ForwardVecY,
    ForwardVecZ,
    CenterMark,
}

#[derive(Debug, Clone, Copy)]
struct ViewPropertyDefinition {
    key: ViewPropertyKey,
    name: &'static str,
    type_name: &'static str,
    readonly: bool,
    has_default: bool,
}

const VIEW_PROPERTY_DEFINITIONS: &[ViewPropertyDefinition] = &[
    view_property(ViewPropertyKey::Name, "name", "string", false, true),
    view_property(ViewPropertyKey::Uid, "uid", "integer", true, false),
    view_property(ViewPropertyKey::Active, "active", "boolean", true, false),
    view_property(ViewPropertyKey::Zoom, "zoom", "real", false, true),
    view_property(ViewPropertyKey::Distance, "distance", "real", false, true),
    view_property(ViewPropertyKey::Slab, "slab", "real", false, true),
    view_property(ViewPropertyKey::Center, "center", "Vector", true, false),
    view_property(ViewPropertyKey::CenterX, "  center.x", "real", false, true),
    view_property(ViewPropertyKey::CenterY, "  center.y", "real", false, true),
    view_property(ViewPropertyKey::CenterZ, "  center.z", "real", false, true),
    view_property(ViewPropertyKey::Rotation, "rotation", "Quat", true, false),
    view_property(
        ViewPropertyKey::RotationX,
        "  rotation.x",
        "real",
        false,
        true,
    ),
    view_property(
        ViewPropertyKey::RotationY,
        "  rotation.y",
        "real",
        false,
        true,
    ),
    view_property(
        ViewPropertyKey::RotationZ,
        "  rotation.z",
        "real",
        false,
        true,
    ),
    view_property(
        ViewPropertyKey::RotationW,
        "  rotation.w",
        "real",
        false,
        true,
    ),
    view_property(
        ViewPropertyKey::Perspective,
        "perspective",
        "boolean",
        false,
        true,
    ),
    view_property(ViewPropertyKey::Width, "width", "integer", true, false),
    view_property(ViewPropertyKey::Height, "height", "integer", true, false),
    view_property(
        ViewPropertyKey::TransMms,
        "trans_mms",
        "boolean",
        false,
        true,
    ),
    view_property(ViewPropertyKey::RotMms, "rot_mms", "boolean", false, true),
    view_property(
        ViewPropertyKey::StereoMode,
        "stereoMode",
        "enum",
        false,
        true,
    ),
    view_property(
        ViewPropertyKey::HasHwStereo,
        "hasHWStereo",
        "boolean",
        true,
        false,
    ),
    view_property(
        ViewPropertyKey::StereoDist,
        "stereoDist",
        "real",
        false,
        true,
    ),
    view_property(
        ViewPropertyKey::SwapStereoEyes,
        "swapStereoEyes",
        "boolean",
        false,
        true,
    ),
    view_property(ViewPropertyKey::UpVec, "up_vec", "Vector", true, false),
    view_property(ViewPropertyKey::UpVecX, "  up_vec.x", "real", true, false),
    view_property(ViewPropertyKey::UpVecY, "  up_vec.y", "real", true, false),
    view_property(ViewPropertyKey::UpVecZ, "  up_vec.z", "real", true, false),
    view_property(
        ViewPropertyKey::RightVec,
        "right_vec",
        "Vector",
        true,
        false,
    ),
    view_property(
        ViewPropertyKey::RightVecX,
        "  right_vec.x",
        "real",
        true,
        false,
    ),
    view_property(
        ViewPropertyKey::RightVecY,
        "  right_vec.y",
        "real",
        true,
        false,
    ),
    view_property(
        ViewPropertyKey::RightVecZ,
        "  right_vec.z",
        "real",
        true,
        false,
    ),
    view_property(
        ViewPropertyKey::ForwardVec,
        "forward_vec",
        "Vector",
        true,
        false,
    ),
    view_property(
        ViewPropertyKey::ForwardVecX,
        "  forward_vec.x",
        "real",
        true,
        false,
    ),
    view_property(
        ViewPropertyKey::ForwardVecY,
        "  forward_vec.y",
        "real",
        true,
        false,
    ),
    view_property(
        ViewPropertyKey::ForwardVecZ,
        "  forward_vec.z",
        "real",
        true,
        false,
    ),
    view_property(
        ViewPropertyKey::CenterMark,
        "centerMark",
        "enum",
        false,
        true,
    ),
];

const fn view_property(
    key: ViewPropertyKey,
    name: &'static str,
    type_name: &'static str,
    readonly: bool,
    has_default: bool,
) -> ViewPropertyDefinition {
    ViewPropertyDefinition {
        key,
        name,
        type_name,
        readonly,
        has_default,
    }
}

impl NativePreferences {
    fn from_state(state: &DesktopAppState) -> Self {
        let input = state.view_input_config();
        Self {
            atom_label_font: nonempty_or(
                state.style_value(0, "", "DefaultLabel.font_name"),
                "san-serif",
            ),
            atom_label_size: state
                .style_value(0, "", "DefaultLabel.font_size")
                .parse()
                .unwrap_or(12.0),
            atom_label_color: nonempty_or(state.style_value(0, "", "DefaultLabel.color"), "Yellow"),
            atom_label_italic: matches!(
                state.style_value(0, "", "DefaultLabel.font_style").as_str(),
                "italic" | "oblique"
            ),
            atom_label_bold: state.style_value(0, "", "DefaultLabel.font_weight") == "bold",
            ui_language: nonempty_or(state.config_value("general.useragent.locale"), "en-US"),
            use_hidpi: config_bool(state, "cuemol2.ui.view.use_hidpi", false),
            mouse_preset: nonempty_or(
                state.config_value("cuemol2.ui.viewinconf"),
                "DefaultViewInConf",
            ),
            trackball_radius: input.tbrad(),
            hit_precision: input.hitprec(),
            momentum_scroll: config_bool(state, "cuemol2.ui.mouse-momentum-scroll", false),
            multitouch_pad: config_bool(state, "cuemol2.ui.mouse-multitouch-pad", false),
            emulate_right_button: config_bool(state, "cuemol2.ui.mouse-emulate-rbutton", false),
            shortcuts: ["view-trans-plusx", "view-trans-minusx"]
                .into_iter()
                .map(|id| NativeShortcut {
                    id: id.to_string(),
                    key: state.config_value(&format!("shortcut:{id}:key")),
                    ctrl: config_bool(state, &format!("shortcut:{id}:ctrl"), true),
                    alt: config_bool(state, &format!("shortcut:{id}:alt"), false),
                    shift: config_bool(state, &format!("shortcut:{id}:shift"), false),
                })
                .collect(),
        }
    }
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

    fn default_open(self) -> bool {
        matches!(
            self,
            LeftPanel::Workspace
                | LeftPanel::MolStruct
                | LeftPanel::Selection
                | LeftPanel::Coloring
                | LeftPanel::DensityMap
        )
    }

    fn index(self) -> usize {
        self as usize
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
            pdb_id: String::new(),
            pdb_history: Vec::new(),
            pdb_history_loaded: false,
            pdb_fetch_coordinates: true,
            pdb_coordinate_source: "RCSB_CIF".to_string(),
            pdb_fetch_two_fofc: false,
            pdb_fetch_fofc: false,
            pdb_map_source: "RCSB_CIF".to_string(),
            pdb_probe_options: None,
            pdb_probe_deadline: None,
            pdb_probe_receiver: None,
            pdb_probe_result: None,
            renderer_setup_draft: None,
            pdb_download_receiver: None,
            url_download_receiver: None,
            script_path: "script.js".to_string(),
            console_input: String::new(),
            selection_text: String::new(),
            selected_object: None,
            workspace_aux_selection: None,
            editing_saved_camera: None,
            camera_visibility_draft: None,
            pending_camera_visibility_clear: None,
            style_properties_draft: None,
            renderer_style_create_draft: None,
            workspace_camera_clipboard: None,
            workspace_object_clipboard: None,
            workspace_renderer_clipboard: None,
            workspace_style_clipboard: None,
            pending_style_paste_scene: None,
            molstruct_object: None,
            molstruct_expand_all: false,
            molstruct_tree_cache: None,
            sequence_summary_cache: None,
            selection_tab: SelectionTab::Command,
            show_unit_cell: false,
            show_symm_mates: false,
            crystal_draft: None,
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
            light_theme: true,
            force_open_all_panels: false,
            coloring_start_hue: 240.0,
            coloring_end_hue: 0.0,
            selected_renderer: None,
            snapshot: RenderSceneSnapshot::empty(),
            snapshot_revision: 0,
            background: SceneBackground::Black,
            color_proofing: false,
            bottom_tab: BottomTab::Output,
            render_preset: RENDER_PRESETS[1].0.to_string(),
            render_backend: "POV-Ray".to_string(),
            umbreon_denoise: UmbreonDenoiseMode::default(),
            ribbon_tab: RibbonTab::Home,
            interaction_mode: InteractionMode::Rotate,
            measure_mode: MeasureMode::Distance,
            show_toolbar: true,
            show_workspace: true,
            left_panel_visible: [true; LeftPanel::ALL.len()],
            reset_left_panel_layout: false,
            // CueMol2 opens properties on demand (no docked inspector); start closed.
            show_inspector: false,
            object_properties_draft: None,
            renderer_properties_draft: None,
            show_bottom_panel: true,
            show_about: false,
            show_options: false,
            options_pane: OptionsPane::Misc,
            options_draft: None,
            show_scene_props: false,
            scene_properties_draft: None,
            scene_property_selected: 0,
            show_view_props: false,
            view_properties_draft: None,
            view_property_selected: 0,
            show_chain_dialog: false,
            show_residue_dialog: false,
            show_renderer_dialog: false,
            new_renderer_owner: None,
            new_renderer_group: None,
            new_renderer_type: "simple".to_string(),
            new_renderer_name: "simple".to_string(),
            chain_edit_value: "A".to_string(),
            residue_start_value: "1".to_string(),
            tool_dialog: None,
            show_get_pdb: false,
            show_console: false,
            viewport_rect: None,
            pixels_per_point: 1.0,
            shift_down: false,
            ctrl_down: false,
            rect_select_start: None,
            rect_select_current: None,
            viewport_menu: None,
            perf_measure: false,
            animation_playing: false,
            animation_loop: false,
            animation_objects: Vec::new(),
            animation_elapsed: 0.0,
            animation_render_settings: AnimationRenderSettings::default(),
            animation_render_job: None,
            animation_render_log: Vec::new(),
            animation_render_total_frames: 0,
            animation_preview_frame: 0,
            animation_preview_texture: None,
            superpose_reference: None,
            superpose_moving: None,
            superpose_algorithm: "lsq".to_string(),
            superpose_reference_selection: "name CA".to_string(),
            superpose_moving_selection: "name CA".to_string(),
            superpose_auto_recenter: true,
            superpose_use_property: false,
            bond_object: None,
            bond_picker_target: None,
            selected_nonstandard_bonds: std::collections::BTreeSet::new(),
            bond_aid1: String::new(),
            bond_aid2: String::new(),
            contact_object: None,
            contact_selection1: String::new(),
            contact_use_object2: false,
            contact_object2: None,
            contact_use_selection2: false,
            contact_selection2: String::new(),
            contact_r_min: 1.5,
            contact_r_max: 3.5,
            contact_hbond: true,
            contact_max_count: 100,
            contact_renderer_name: "measure".to_string(),
            secondary_ignore_breaks: false,
            secondary_gap_fill: false,
            secondary_angle: 120.0,
            surface_path: "surface.obj".to_string(),
            surface_density: 1.0,
            surface_probe_radius: 1.4,
            cut_surface_target: None,
            cut_surface_mode: "full".to_string(),
            cut_surface_density: 1.0,
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
            modified_scenes: std::collections::BTreeSet::new(),
            status: "Ready".to_string(),
            log_lines: Vec::new(),
            selection_history: Vec::new(),
        };
        app.rebuild_snapshot();
        app
    }

    fn new_persistent() -> Self {
        let mut app = Self::new();
        let Some(path) = default_config_path() else {
            return app;
        };
        app.config_path = path.to_string_lossy().into_owned();
        if path.is_file() {
            match app.state.load_config_file(&path) {
                Ok(_) => {
                    app.apply_stored_preferences();
                    app.status = format!("Loaded config {}", path.display());
                }
                Err(error) => app.status = format!("Config load failed: {error}"),
            }
        }
        app
    }

    fn ui(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        self.poll_pdb_download(camera);
        self.poll_url_download(camera);
        self.poll_animation_render(camera);
        self.pixels_per_point = ctx.pixels_per_point();
        // UI theme (View menu): CueMol2's native XUL look is light by default.
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
        if self.show_workspace && self.left_panel_visible.iter().any(|visible| *visible) {
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
        self.keyboard_shortcuts(ctx, camera);
        if self.pdb_download_receiver.is_some()
            || self.url_download_receiver.is_some()
            || self.animation_render_job.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn central_view(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                self.viewport_rect = Some(rect);
                self.draw_screen_lines(ui, camera, rect);
                self.draw_center_mark(ui, rect, camera);
                self.draw_name_labels(ui, camera, rect);
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
                self.draw_rect_selection(ui);
            });
        self.viewport_context_menu(ctx, camera);
    }

    fn draw_rect_selection(&self, ui: &egui::Ui) {
        let (Some(start), Some(current)) = (self.rect_select_start, self.rect_select_current)
        else {
            return;
        };
        let rect = egui::Rect::from_two_pos(start, current);
        let painter = ui.painter();
        painter.rect_filled(
            rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(64, 153, 255, 28),
        );
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(64, 153, 255)),
            egui::StrokeKind::Inside,
        );
    }

    /// Right-click atom context menu over the 3D view (CueMol2 `navi-ctxtmenu-atom`).
    fn viewport_context_menu(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        let Some(menu) = self.viewport_menu else {
            return;
        };
        // Resolve the picked atom up front so the menu closure stays self-borrow-free.
        let atom_info = menu.hit.and_then(|hit| {
            let object_uid = cuemol_core::Uid::new(hit.object_uid);
            let atom = self.state.atom_summary(object_uid, hit.atom_index).ok()?;
            let object_name = self
                .state
                .active_object_summaries()
                .into_iter()
                .find(|object| object.uid == object_uid)
                .map(|object| object.name)
                .unwrap_or_default();
            let renderer_label = self
                .state
                .renderer_summaries(object_uid)
                .into_iter()
                .find(|renderer| renderer.visible && !renderer.renderer_type.starts_with('*'))
                .map(|renderer| format!("{} ({})", renderer.name, renderer.renderer_type));
            let center = self
                .snapshot
                .atoms
                .iter()
                .find(|a| {
                    a.object_uid == Some(hit.object_uid) && a.atom_index == Some(hit.atom_index)
                })
                .map(|a| vec3_from_vector(a.center));
            Some((atom, center, object_name, renderer_label))
        });

        enum Choice {
            Center(Vec3),
            Select {
                object_uid: cuemol_core::Uid,
                expression: String,
                status: &'static str,
            },
        }
        let mut chosen: Option<Choice> = None;
        let mut close = false;

        egui::Area::new(egui::Id::new("viewport_ctx_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(menu.pos)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_max_width(240.0);
                    if let Some((atom, center, object_name, renderer_label)) = &atom_info {
                        let chain = if atom.chain_id.is_empty() {
                            "_"
                        } else {
                            atom.chain_id.as_str()
                        };
                        let object_uid = atom.object_uid;
                        let stored_selection = self
                            .state
                            .molecule_selection_text(object_uid)
                            .unwrap_or_default();
                        let current_selection = if stored_selection.trim().is_empty()
                            && self.selected_object == Some(object_uid)
                        {
                            self.selection_text.trim()
                        } else {
                            stored_selection.trim()
                        }
                        .to_string();
                        ui.label(
                            egui::RichText::new(format!(
                                "{object_name}: {}{}:{}  (chain {chain})",
                                atom.residue_name, atom.residue_id, atom.name
                            ))
                            .strong(),
                        );
                        if let Some(renderer_label) = renderer_label {
                            ui.label(renderer_label);
                        }
                        ui.separator();
                        let atom_sel = format!("aid {}", atom.serial);
                        let resid_sel = if atom.chain_id.is_empty() {
                            format!("resid {}", atom.residue_id)
                        } else {
                            format!("chain {} and resid {}", atom.chain_id, atom.residue_id)
                        };
                        let chain_sel = if atom.chain_id.is_empty() {
                            "''.*.*".to_string()
                        } else {
                            format!("chain {}", atom.chain_id)
                        };
                        if let Some(center) = center {
                            if ui.button("Center at this atom").clicked() {
                                chosen = Some(Choice::Center(*center));
                                close = true;
                            }
                        }
                        if ui.button("Select this atom").clicked() {
                            chosen = Some(Choice::Select {
                                object_uid,
                                expression: atom_sel.clone(),
                                status: "Selected atom",
                            });
                            close = true;
                        }
                        if ui.button("Select this residue").clicked() {
                            chosen = Some(Choice::Select {
                                object_uid,
                                expression: resid_sel.clone(),
                                status: "Selected residue",
                            });
                            close = true;
                        }
                        if ui.button("Select this chain").clicked() {
                            chosen = Some(Choice::Select {
                                object_uid,
                                expression: chain_sel.clone(),
                                status: "Selected chain",
                            });
                            close = true;
                        }
                        if ui.button("Select this molecule").clicked() {
                            chosen = Some(Choice::Select {
                                object_uid,
                                expression: "*".to_string(),
                                status: "Selected molecule",
                            });
                            close = true;
                        }
                        ui.menu_button("Add Select", |ui| {
                            for (label, expression) in [
                                ("This atom", atom_sel.as_str()),
                                ("This residue", resid_sel.as_str()),
                                ("This chain", chain_sel.as_str()),
                            ] {
                                if ui.button(label).clicked() {
                                    chosen = Some(Choice::Select {
                                        object_uid,
                                        expression: add_selection_expression(
                                            &current_selection,
                                            expression,
                                        ),
                                        status: "Added to selection",
                                    });
                                    close = true;
                                    ui.close();
                                }
                            }
                        });
                        ui.separator();
                        if ui.button("Unselect").clicked() {
                            chosen = Some(Choice::Select {
                                object_uid,
                                expression: String::new(),
                                status: "Cleared molecule selection",
                            });
                            close = true;
                        }
                        if ui.button("Invert Sel").clicked() {
                            chosen = Some(Choice::Select {
                                object_uid,
                                expression: invert_selection_expression(&current_selection),
                                status: "Inverted molecule selection",
                            });
                            close = true;
                        }
                        if ui.button("Toggle Sidechain").clicked() {
                            if !current_selection.is_empty() {
                                chosen = Some(Choice::Select {
                                    object_uid,
                                    expression: toggle_sidechain_expression(&current_selection),
                                    status: "Toggled sidechain selection",
                                });
                            }
                            close = true;
                        }
                        for (label, by_residue) in [("Around Byresid", true), ("Around", false)] {
                            ui.menu_button(label, |ui| {
                                for radius in [3.0_f64, 5.0, 7.0, 10.0] {
                                    if ui.button(format!("{radius:.0} Å")).clicked() {
                                        if !current_selection.is_empty() {
                                            chosen = Some(Choice::Select {
                                                object_uid,
                                                expression: around_selection_expression(
                                                    &current_selection,
                                                    radius,
                                                    by_residue,
                                                ),
                                                status: "Expanded molecule selection",
                                            });
                                        }
                                        close = true;
                                        ui.close();
                                    }
                                }
                            });
                        }
                        ui.separator();
                    } else {
                        ui.label("(no atom under cursor)");
                        ui.separator();
                    }
                });
            });

        if let Some(choice) = chosen {
            match choice {
                Choice::Center(center) => {
                    camera.target = center;
                    self.set_status("Centered at atom");
                }
                Choice::Select {
                    object_uid,
                    expression,
                    status,
                } => {
                    self.apply_molecule_selection(object_uid, expression, status);
                }
            }
        }
        if close || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.viewport_menu = None;
        }
    }

    fn apply_molecule_selection(
        &mut self,
        object_uid: cuemol_core::Uid,
        expression: String,
        status: &str,
    ) -> bool {
        let undo = self.capture_undo();
        match self
            .state
            .set_molecule_selection_text(object_uid, &expression)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.selected_object = Some(object_uid);
                self.molstruct_object = Some(object_uid);
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
                self.selection_text = expression;
                self.push_selection_history();
                self.rebuild_snapshot();
                self.set_status(status);
                true
            }
            Err(error) => {
                self.set_error(error);
                false
            }
        }
    }

    fn selection_target_object(&self) -> Option<cuemol_core::Uid> {
        let objects = self.state.active_object_summaries();
        self.molstruct_object
            .filter(|uid| objects.iter().any(|object| object.uid == *uid))
            .or_else(|| {
                self.selected_object
                    .filter(|uid| objects.iter().any(|object| object.uid == *uid))
            })
            .or_else(|| objects.first().map(|object| object.uid))
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
                        self.workspace_aux_selection = None;
                        self.sync_active_scene_properties();
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
                        self.new_window(camera);
                        ui.close();
                    }
                    if ui.button("New Tab").clicked() {
                        self.new_scene(camera);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Open File...").clicked() {
                        self.choose_open_file(camera);
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
                                self.request_open_current_path(camera);
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
                        self.choose_save_selected_object();
                        ui.close();
                    }
                    if ui.button("Save current view...").clicked() {
                        self.choose_save_current_view(camera);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Close Tab").clicked() {
                        self.close_current_scene(camera);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Open Scene...").clicked() {
                        self.choose_open_scene(camera);
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
                        self.choose_save_scene(camera);
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
                        self.choose_merge_molecule(camera);
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
                        self.open_options_dialog();
                        ui.close();
                    }
                });
                ui.menu_button("Rendering", |ui| {
                    if ui.button("POV-Ray rendering...").clicked() {
                        self.render_backend = "POV-Ray".to_string();
                        self.tool_dialog = Some("POV-Ray rendering".to_string());
                        ui.close();
                    }
                    if ui.button("Animation rendering...").clicked() {
                        self.tool_dialog = Some("Animation rendering".to_string());
                        ui.close();
                    }
                    if ui.button("Export scene...").clicked() {
                        self.choose_export_scene();
                        ui.close();
                    }
                });
                ui.menu_button("Scene", |ui| {
                    ui.menu_button("Background", |ui| {
                        if ui
                            .radio_value(&mut self.background, SceneBackground::White, "White")
                            .clicked()
                        {
                            self.persist_scene_background("white");
                            ui.close();
                        }
                        if ui
                            .radio_value(&mut self.background, SceneBackground::Black, "Black")
                            .clicked()
                        {
                            self.persist_scene_background("black");
                            ui.close();
                        }
                    });
                    ui.separator();
                    if ui
                        .checkbox(&mut self.color_proofing, "Use color proofing")
                        .clicked()
                    {
                        self.persist_scene_color_proofing();
                    }
                    if ui.button("Properties...").clicked() {
                        self.open_scene_properties();
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
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
                        ui.radio_value(&mut camera.center_mark, CenterMark::Cross, "Cross");
                        ui.radio_value(&mut camera.center_mark, CenterMark::Axis, "Axis");
                        ui.radio_value(&mut camera.center_mark, CenterMark::None, "None");
                    });
                    ui.separator();
                    ui.add_enabled(false, egui::Button::new("Hardware stereo"))
                        .on_hover_text("No hardware stereo buffer is available");
                    if ui.button("View property...").clicked() {
                        self.open_view_properties(camera);
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
                                "Execute script..." => self.choose_execute_script(),
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
                    ui.menu_button("Panels", |ui| {
                        for panel in LeftPanel::ALL {
                            let visible = &mut self.left_panel_visible[panel.index()];
                            if ui.checkbox(visible, panel.title()).clicked() && *visible {
                                self.show_workspace = true;
                            }
                        }
                    });
                    ui.menu_button("Windows", |ui| {
                        let _ = ui.radio(true, "CueMol2");
                    });
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About CueMol2").clicked() {
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

    /// Ribbon toolbar (CueMol2 base ribbon + `anim-ribbon.xul`): Home, Measure,
    /// and Animation tabs sitting directly below the menu bar.
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
                self.choose_open_file(camera);
            }
            if ui.button("Save As").on_hover_text("Save file as").clicked() {
                self.choose_save_selected_object();
            }
            ui.separator();
            if ui.button("Open Scene").clicked() {
                self.choose_open_scene(camera);
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
        if let Some(measurement) = self.state.active_measure(&self.selection_text) {
            return Some(measurement);
        }
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
        let total = self
            .animation_objects
            .iter()
            .map(|object| object.duration.max(0.01))
            .sum::<f64>() as f32;
        ui.horizontal(|ui| {
            let play_label = if self.animation_playing {
                "⏸ Pause"
            } else {
                "▶ Play"
            };
            if ui
                .add_enabled(total > 0.0, egui::Button::new(play_label))
                .clicked()
            {
                self.animation_playing = !self.animation_playing;
            }
            if ui
                .add_enabled(
                    self.animation_playing || self.animation_elapsed > 0.0,
                    egui::Button::new("⏹ Stop"),
                )
                .clicked()
            {
                self.animation_playing = false;
                self.animation_elapsed = 0.0;
            }
            ui.checkbox(&mut self.animation_loop, "Loop");
            if total > 0.0 {
                ui.add(
                    egui::Slider::new(&mut self.animation_elapsed, 0.0..=total).show_value(false),
                );
            } else {
                ui.add_enabled(
                    false,
                    egui::Slider::new(&mut self.animation_elapsed, 0.0..=1.0).show_value(false),
                );
            }
            let current = self.animation_elapsed.max(0.0);
            ui.monospace(format!(
                "{:02}:{:02} / {:02}:{:02}",
                (current / 60.0).floor() as u32,
                (current % 60.0).floor() as u32,
                (total / 60.0).floor() as u32,
                (total % 60.0).floor() as u32
            ));
        });
    }

    /// Left dock: a vertical accordion of CueMol2's eight side panels
    /// (`cuemol2-panels.js` / `sidepanelholder`). Each is an independently
    /// collapsible `CollapsingHeader`.
    fn left_panel(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::SidePanel::left("left_dock")
            .resizable(true)
            .default_width(250.0)
            .width_range(220.0..=360.0)
            .show(ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    let reset_layout = self.reset_left_panel_layout;
                    for panel in LeftPanel::ALL {
                        if !self.left_panel_visible[panel.index()] {
                            continue;
                        }
                        let id = ui.make_persistent_id(("left-panel", panel.title()));
                        let mut state =
                            egui::collapsing_header::CollapsingState::load_with_default_open(
                                ui.ctx(),
                                id,
                                panel.default_open(),
                            );
                        if self.force_open_all_panels {
                            state.set_open(true);
                        } else if reset_layout {
                            state.set_open(panel.default_open());
                        }
                        state
                            .show_header(ui, |ui| ui.label(panel.title()))
                            .body(|ui| {
                                self.left_panel_body(ui, camera, panel);
                            });
                    }
                    self.reset_left_panel_layout = false;
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
            let objects = self.state.object_summaries(scene.uid);
            let density_maps = self.state.density_map_summaries(scene.uid);
            let surfaces = self.state.surface_summaries(scene.uid);
            let cameras = self.state.camera_summaries(scene.uid);
            let styles = self.state.style_summaries(scene.uid);
            let object_count = objects.len() + density_maps.len() + surfaces.len();
            let mut workspace_objects = objects
                .into_iter()
                .map(NativeWorkspaceObjectSummary::Molecule)
                .chain(
                    density_maps
                        .into_iter()
                        .map(NativeWorkspaceObjectSummary::DensityMap),
                )
                .chain(
                    surfaces
                        .into_iter()
                        .map(NativeWorkspaceObjectSummary::Surface),
                )
                .collect::<Vec<_>>();
            workspace_objects.sort_by_key(NativeWorkspaceObjectSummary::sort_key);
            let scene_label = format!("{}  [{} objects]", scene.name, object_count);
            let selected_scene = Some(scene.uid) == self.state.active_scene();
            let scene_response = ui.selectable_label(selected_scene, scene_label);
            if scene_response.clicked() {
                self.activate_workspace_scene(scene.uid, camera);
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
            }
            let scene_background = self
                .state
                .scene_manager()
                .scene(scene.uid)
                .and_then(|value| value.scene_attribute("bgcolor"))
                .and_then(|value| SceneBackground::parse(value).ok())
                .unwrap_or(SceneBackground::Black);
            let scene_color_proofing = self
                .state
                .scene_manager()
                .scene(scene.uid)
                .is_some_and(|value| scene_attr_bool(value.scene_attribute("use_colproof"), false));
            scene_response.context_menu(|ui| {
                ui.menu_button("Background color", |ui| {
                    for (label, background, value) in [
                        ("White", SceneBackground::White, "white"),
                        ("Black", SceneBackground::Black, "black"),
                    ] {
                        if ui.radio(scene_background == background, label).clicked() {
                            self.activate_workspace_scene(scene.uid, camera);
                            if self.background != background {
                                self.persist_scene_background(value);
                            }
                            ui.close();
                        }
                    }
                });
                let mut color_proofing = scene_color_proofing;
                if ui
                    .checkbox(&mut color_proofing, "Use color proofing")
                    .changed()
                {
                    self.activate_workspace_scene(scene.uid, camera);
                    self.color_proofing = color_proofing;
                    self.persist_scene_color_proofing();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.workspace_object_clipboard.is_some(),
                        egui::Button::new("Paste Object"),
                    )
                    .clicked()
                {
                    self.activate_workspace_scene(scene.uid, camera);
                    self.paste_workspace_object_clipboard(scene.uid);
                    ui.close();
                }
                if ui.button("Properties...").clicked() {
                    self.activate_workspace_scene(scene.uid, camera);
                    self.open_scene_properties();
                    ui.close();
                }
            });
            ui.indent(format!("scene-{}", scene.uid.get()), |ui| {
                for workspace_object in workspace_objects {
                    match workspace_object {
                        NativeWorkspaceObjectSummary::Molecule(object) => {
                            let selected = self.workspace_aux_selection.is_none()
                                && self.selected_object == Some(object.uid);
                            let label = format!(
                                "{}  [{} atoms, {} bonds]",
                                object.name, object.atom_count, object.bond_count
                            );
                            let mut visible = object.visible;
                            let owner = NativePropertyObject::Molecule(object.uid);
                            let drag_source = ui.dnd_drag_source(
                                egui::Id::new(("workspace-object-drag", owner)),
                                NativeWorkspaceObjectDragPayload {
                                    scene_uid: scene.uid,
                                    owner,
                                },
                                |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .checkbox(&mut visible, "")
                                            .on_hover_text("Visible")
                                            .changed()
                                        {
                                            self.set_workspace_object_visibility(
                                                NativePropertyObject::Molecule(object.uid),
                                                visible,
                                            );
                                        }
                                        if let Some(locked) =
                                            workspace_lock_toggle(ui, object.locked, true)
                                        {
                                            self.set_workspace_object_locked(
                                                NativePropertyObject::Molecule(object.uid),
                                                locked,
                                            );
                                        }
                                        if ui.selectable_label(selected, label).clicked() {
                                            self.activate_workspace_scene(scene.uid, camera);
                                            self.selected_object = Some(object.uid);
                                            self.selected_renderer = None;
                                            self.workspace_aux_selection = None;
                                        }
                                    })
                                },
                            );
                            self.workspace_object_drop_target(
                                ui,
                                scene.uid,
                                owner,
                                &drag_source.response,
                            );
                            drag_source.response.context_menu(|ui| {
                                self.workspace_molecule_object_context_menu(
                                    ui, camera, scene.uid, object.uid,
                                );
                            });
                            ui.indent(format!("object-{}", object.uid.get()), |ui| {
                                let nodes = native_renderer_tree_nodes(
                                    self.state.renderer_summaries(object.uid),
                                );
                                for node in nodes {
                                    let is_group = node.renderer.renderer_type == "*group";
                                    self.workspace_molecule_renderer_row(
                                        ui,
                                        camera,
                                        scene.uid,
                                        object.uid,
                                        &node.renderer,
                                        is_group,
                                    );
                                    if is_group {
                                        ui.indent(
                                            format!("renderer-group-{}", node.renderer.uid.get()),
                                            |ui| {
                                                for renderer in &node.children {
                                                    self.workspace_molecule_renderer_row(
                                                        ui, camera, scene.uid, object.uid,
                                                        renderer, false,
                                                    );
                                                }
                                            },
                                        );
                                    }
                                }
                            });
                        }
                        NativeWorkspaceObjectSummary::DensityMap(map) => {
                            let object_selection = WorkspaceAuxSelection::DensityMap {
                                scene_uid: scene.uid,
                                map_uid: map.uid,
                                renderer_uid: None,
                            };
                            let selected = self.selected_object.is_none()
                                && self.workspace_aux_selection == Some(object_selection);
                            let mut visible = map.visible;
                            let owner = NativePropertyObject::DensityMap(map.uid);
                            let drag_source = ui.dnd_drag_source(
                                egui::Id::new(("workspace-object-drag", owner)),
                                NativeWorkspaceObjectDragPayload {
                                    scene_uid: scene.uid,
                                    owner,
                                },
                                |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .checkbox(&mut visible, "")
                                            .on_hover_text("Visible")
                                            .changed()
                                        {
                                            self.set_density_map_visibility(map.uid, visible);
                                        }
                                        if let Some(locked) =
                                            workspace_lock_toggle(ui, map.locked, true)
                                        {
                                            self.set_workspace_object_locked(
                                                NativePropertyObject::DensityMap(map.uid),
                                                locked,
                                            );
                                        }
                                        let label = format!("{} ({})", map.name, map.class_name);
                                        if ui.selectable_label(selected, label).clicked() {
                                            self.activate_workspace_scene(scene.uid, camera);
                                            self.selected_object = None;
                                            self.selected_renderer = None;
                                            self.workspace_aux_selection = Some(object_selection);
                                        }
                                    })
                                },
                            );
                            self.workspace_object_drop_target(
                                ui,
                                scene.uid,
                                owner,
                                &drag_source.response,
                            );
                            drag_source.response.context_menu(|ui| {
                                self.workspace_aux_object_context_menu(
                                    ui,
                                    camera,
                                    scene.uid,
                                    object_selection,
                                    owner,
                                );
                            });
                            ui.indent(format!("density-map-{}", map.uid.get()), |ui| {
                                for renderer in map.renderers {
                                    let selection = WorkspaceAuxSelection::DensityMap {
                                        scene_uid: scene.uid,
                                        map_uid: map.uid,
                                        renderer_uid: Some(renderer.uid),
                                    };
                                    let selected = self.selected_object.is_none()
                                        && self.workspace_aux_selection == Some(selection);
                                    let mut visible = renderer.visible;
                                    let response = ui
                                        .horizontal(|ui| {
                                            if ui
                                                .checkbox(&mut visible, "")
                                                .on_hover_text("Visible")
                                                .changed()
                                            {
                                                self.set_density_renderer_visibility(
                                                    map.uid,
                                                    renderer.uid,
                                                    visible,
                                                );
                                            }
                                            if let Some(locked) =
                                                workspace_lock_toggle(ui, renderer.locked, true)
                                            {
                                                self.set_workspace_renderer_locked(
                                                    NativePropertyObject::DensityMap(map.uid),
                                                    renderer.uid,
                                                    locked,
                                                );
                                            }
                                            let label = format!(
                                                "{} ({})",
                                                renderer.name, renderer.renderer_type
                                            );
                                            if ui.selectable_label(selected, label).clicked() {
                                                self.activate_workspace_scene(scene.uid, camera);
                                                self.selected_object = None;
                                                self.selected_renderer = None;
                                                self.workspace_aux_selection = Some(selection);
                                            }
                                        })
                                        .response;
                                    response.context_menu(|ui| {
                                        self.workspace_aux_renderer_context_menu(ui, selection);
                                    });
                                }
                            });
                        }
                        NativeWorkspaceObjectSummary::Surface(surface) => {
                            let object_selection = WorkspaceAuxSelection::Surface {
                                scene_uid: scene.uid,
                                surface_uid: surface.uid,
                                renderer_uid: None,
                            };
                            let selected = self.selected_object.is_none()
                                && self.workspace_aux_selection == Some(object_selection);
                            let mut visible = surface.visible;
                            let owner = NativePropertyObject::Surface(surface.uid);
                            let drag_source = ui.dnd_drag_source(
                                egui::Id::new(("workspace-object-drag", owner)),
                                NativeWorkspaceObjectDragPayload {
                                    scene_uid: scene.uid,
                                    owner,
                                },
                                |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .checkbox(&mut visible, "")
                                            .on_hover_text("Visible")
                                            .changed()
                                        {
                                            self.set_surface_visibility(surface.uid, visible);
                                        }
                                        if let Some(locked) =
                                            workspace_lock_toggle(ui, surface.locked, true)
                                        {
                                            self.set_workspace_object_locked(
                                                NativePropertyObject::Surface(surface.uid),
                                                locked,
                                            );
                                        }
                                        let label =
                                            format!("{} ({})", surface.name, surface.class_name);
                                        if ui.selectable_label(selected, label).clicked() {
                                            self.activate_workspace_scene(scene.uid, camera);
                                            self.selected_object = None;
                                            self.selected_renderer = None;
                                            self.workspace_aux_selection = Some(object_selection);
                                        }
                                    })
                                },
                            );
                            self.workspace_object_drop_target(
                                ui,
                                scene.uid,
                                owner,
                                &drag_source.response,
                            );
                            drag_source.response.context_menu(|ui| {
                                self.workspace_aux_object_context_menu(
                                    ui,
                                    camera,
                                    scene.uid,
                                    object_selection,
                                    owner,
                                );
                            });
                            ui.indent(format!("surface-{}", surface.uid.get()), |ui| {
                                for renderer in surface.renderers {
                                    let selection = WorkspaceAuxSelection::Surface {
                                        scene_uid: scene.uid,
                                        surface_uid: surface.uid,
                                        renderer_uid: Some(renderer.uid),
                                    };
                                    let selected = self.selected_object.is_none()
                                        && self.workspace_aux_selection == Some(selection);
                                    let mut visible = renderer.visible;
                                    let response = ui
                                        .horizontal(|ui| {
                                            if ui
                                                .checkbox(&mut visible, "")
                                                .on_hover_text("Visible")
                                                .changed()
                                            {
                                                self.set_surface_renderer_visibility(
                                                    surface.uid,
                                                    renderer.uid,
                                                    visible,
                                                );
                                            }
                                            if let Some(locked) =
                                                workspace_lock_toggle(ui, renderer.locked, true)
                                            {
                                                self.set_workspace_renderer_locked(
                                                    NativePropertyObject::Surface(surface.uid),
                                                    renderer.uid,
                                                    locked,
                                                );
                                            }
                                            let label = format!(
                                                "{} ({})",
                                                renderer.name, renderer.renderer_type
                                            );
                                            if ui.selectable_label(selected, label).clicked() {
                                                self.activate_workspace_scene(scene.uid, camera);
                                                self.selected_object = None;
                                                self.selected_renderer = None;
                                                self.workspace_aux_selection = Some(selection);
                                            }
                                        })
                                        .response;
                                    response.context_menu(|ui| {
                                        self.workspace_aux_renderer_context_menu(ui, selection);
                                    });
                                }
                            });
                        }
                    }
                }
                let root_selection = WorkspaceAuxSelection::CameraRoot {
                    scene_uid: scene.uid,
                };
                let root_selected = self.selected_object.is_none()
                    && self.workspace_aux_selection == Some(root_selection);
                let root_response = ui.selectable_label(root_selected, "Camera");
                if root_response.clicked() {
                    self.activate_workspace_scene(scene.uid, camera);
                    self.selected_object = None;
                    self.selected_renderer = None;
                    self.workspace_aux_selection = Some(root_selection);
                }
                root_response.context_menu(|ui| {
                    if ui.button("New Camera...").clicked() {
                        self.open_new_camera_properties(scene.uid, camera);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.workspace_camera_clipboard.is_some(),
                            egui::Button::new("Paste"),
                        )
                        .clicked()
                    {
                        self.paste_workspace_camera_clipboard(scene.uid);
                        ui.close();
                    }
                    ui.separator();
                    ui.menu_button("Camera file", |ui| {
                        if ui.button("Load ...").clicked() {
                            self.choose_load_workspace_camera_file(scene.uid, camera);
                            ui.close();
                        }
                        ui.add_enabled(false, egui::Button::new("Reload"));
                        ui.add_enabled(false, egui::Button::new("Save"));
                        ui.add_enabled(false, egui::Button::new("Save as..."));
                    });
                });
                ui.indent(format!("camera-root-{}", scene.uid.get()), |ui| {
                    for (camera_index, summary) in cameras.iter().enumerate() {
                        let selection = WorkspaceAuxSelection::Camera {
                            scene_uid: scene.uid,
                            camera_index,
                        };
                        let selected = self.selected_object.is_none()
                            && self.workspace_aux_selection == Some(selection);
                        let mut label = summary.name.clone();
                        if !summary.source.is_empty() {
                            label.push_str("  ↗");
                        }
                        if summary.visibility_count > 0 {
                            label.push_str("  ◉");
                        }
                        let response = ui.selectable_label(selected, label);
                        if response.clicked() {
                            self.activate_workspace_scene(scene.uid, camera);
                            self.selected_object = None;
                            self.selected_renderer = None;
                            self.workspace_aux_selection = Some(selection);
                        }
                        if response.double_clicked() {
                            self.load_workspace_camera(selection, camera);
                        }
                        response.context_menu(|ui| {
                            if ui.button("New Camera...").clicked() {
                                self.open_new_camera_properties(scene.uid, camera);
                                ui.close();
                            }
                            if ui.button("Delete").clicked() {
                                self.delete_aux_selection(selection);
                                ui.close();
                            }
                            if ui.button("Copy").clicked() {
                                self.copy_workspace_camera_to_clipboard(scene.uid, &summary.name);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    self.workspace_camera_clipboard.is_some(),
                                    egui::Button::new("Paste"),
                                )
                                .clicked()
                            {
                                self.paste_workspace_camera_clipboard(scene.uid);
                                ui.close();
                            }
                            ui.separator();
                            ui.menu_button("Camera file", |ui| {
                                if ui.button("Load ...").clicked() {
                                    self.choose_load_workspace_camera_file(scene.uid, camera);
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        !summary.source.is_empty(),
                                        egui::Button::new("Reload"),
                                    )
                                    .clicked()
                                {
                                    self.reload_workspace_camera_file(selection);
                                    ui.close();
                                }
                                if ui.button("Save").clicked() {
                                    self.choose_save_workspace_camera_file(selection, false);
                                    ui.close();
                                }
                                if ui.button("Save as...").clicked() {
                                    self.choose_save_workspace_camera_file(selection, true);
                                    ui.close();
                                }
                            });
                            ui.separator();
                            if ui.button("Save from view").clicked() {
                                self.save_workspace_camera_from_view(selection, camera);
                                ui.close();
                            }
                            if ui.button("Apply to view").clicked() {
                                self.load_workspace_camera_view(selection, camera);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Save from scene (with vis flags)").clicked() {
                                self.save_workspace_camera_from_scene(selection, camera);
                                ui.close();
                            }
                            if ui.button("Apply to scene (with vis flags)").clicked() {
                                self.load_workspace_camera(selection, camera);
                                ui.close();
                            }
                            if ui.button("Edit vis flags...").clicked() {
                                self.open_camera_visibility_editor(selection);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    summary.visibility_count > 0,
                                    egui::Button::new("Clear vis flags"),
                                )
                                .clicked()
                            {
                                self.pending_camera_visibility_clear =
                                    Some((summary.scene_uid, summary.name.clone()));
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Rename...").clicked() {
                                self.open_camera_properties(selection);
                                ui.close();
                            }
                            if ui.button("Properties...").clicked() {
                                self.open_camera_properties(selection);
                                ui.close();
                            }
                        });
                    }
                });
                let root_selection = WorkspaceAuxSelection::StyleRoot {
                    scene_uid: scene.uid,
                };
                let root_selected = self.selected_object.is_none()
                    && self.workspace_aux_selection == Some(root_selection);
                let root_response = ui.selectable_label(root_selected, "Styles");
                if root_response.clicked() {
                    self.activate_workspace_scene(scene.uid, camera);
                    self.selected_object = None;
                    self.selected_renderer = None;
                    self.workspace_aux_selection = Some(root_selection);
                }
                root_response.context_menu(|ui| {
                    if ui.button("New Style...").clicked() {
                        self.open_new_style_properties(scene.uid);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.workspace_style_clipboard.is_some(),
                            egui::Button::new("Paste"),
                        )
                        .clicked()
                    {
                        self.request_paste_workspace_style(scene.uid);
                        ui.close();
                    }
                    ui.separator();
                    ui.menu_button("Style file", |ui| {
                        if ui.button("Load ...").clicked() {
                            self.choose_load_workspace_style_file(scene.uid, camera);
                            ui.close();
                        }
                        ui.add_enabled(false, egui::Button::new("Reload"));
                        ui.add_enabled(false, egui::Button::new("Save"));
                        ui.add_enabled(false, egui::Button::new("Save As..."));
                    });
                });
                ui.indent(format!("style-root-{}", scene.uid.get()), |ui| {
                    for (style_index, summary) in styles.iter().enumerate() {
                        let selection = WorkspaceAuxSelection::Style {
                            scene_uid: scene.uid,
                            style_index,
                        };
                        let selected = self.selected_object.is_none()
                            && self.workspace_aux_selection == Some(selection);
                        let mut label = summary.name.clone();
                        if summary.read_only || summary.scene_uid.get() == 0 {
                            label.push_str("  🔒");
                        }
                        if !summary.source.is_empty() {
                            label.push_str("  ↗");
                        }
                        let response = ui.selectable_label(selected, label);
                        if response.clicked() {
                            self.activate_workspace_scene(scene.uid, camera);
                            self.selected_object = None;
                            self.selected_renderer = None;
                            self.workspace_aux_selection = Some(selection);
                        }
                        if response.double_clicked() && summary.local_index.is_some() {
                            self.open_style_properties(selection);
                        }
                        response.context_menu(|ui| {
                            if ui.button("New Style...").clicked() {
                                self.open_new_style_properties(scene.uid);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    summary.local_index.is_some(),
                                    egui::Button::new("Copy"),
                                )
                                .clicked()
                            {
                                self.copy_workspace_style_to_clipboard(
                                    scene.uid,
                                    summary.local_index.expect("local style was enabled"),
                                );
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    self.workspace_style_clipboard.is_some(),
                                    egui::Button::new("Paste"),
                                )
                                .clicked()
                            {
                                self.request_paste_workspace_style(scene.uid);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    summary.local_index.is_some(),
                                    egui::Button::new("Delete"),
                                )
                                .clicked()
                            {
                                self.delete_aux_selection(selection);
                                ui.close();
                            }
                            ui.separator();
                            ui.menu_button("Style file", |ui| {
                                if ui.button("Load ...").clicked() {
                                    self.choose_load_workspace_style_file(scene.uid, camera);
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        summary.local_index.is_some() && !summary.source.is_empty(),
                                        egui::Button::new("Reload"),
                                    )
                                    .clicked()
                                {
                                    self.set_status("Not implemented!!");
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        summary.local_index.is_some(),
                                        egui::Button::new("Save"),
                                    )
                                    .clicked()
                                {
                                    self.choose_save_workspace_style_file(selection, false);
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        summary.local_index.is_some(),
                                        egui::Button::new("Save As..."),
                                    )
                                    .clicked()
                                {
                                    self.choose_save_workspace_style_file(selection, true);
                                    ui.close();
                                }
                            });
                            ui.separator();
                            let mut read_only = summary.read_only;
                            if ui
                                .add_enabled(
                                    summary.local_index.is_some()
                                        && (summary.read_only || !summary.modified),
                                    egui::Checkbox::new(&mut read_only, "Read-only"),
                                )
                                .changed()
                            {
                                self.set_workspace_style_read_only(selection, read_only);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    summary.local_index.is_some(),
                                    egui::Button::new("Rename..."),
                                )
                                .clicked()
                            {
                                self.open_style_properties(selection);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    summary.local_index.is_some(),
                                    egui::Button::new("Edit..."),
                                )
                                .clicked()
                            {
                                self.open_style_properties(selection);
                                ui.close();
                            }
                        });
                    }
                });
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            let zoom_enabled = (self.selected_object.is_some()
                && !self.selected_renderer_is_group())
                || matches!(
                    self.workspace_aux_selection,
                    Some(WorkspaceAuxSelection::DensityMap { .. })
                );
            if ui
                .add_enabled(zoom_enabled, egui::Button::new("Zoom"))
                .on_hover_text("Zoom at selected item")
                .clicked()
            {
                self.zoom_workspace_selection(camera);
            }
            let adding_camera = matches!(
                self.workspace_aux_selection,
                Some(
                    WorkspaceAuxSelection::CameraRoot { .. } | WorkspaceAuxSelection::Camera { .. }
                )
            );
            let adding_style = matches!(
                self.workspace_aux_selection,
                Some(WorkspaceAuxSelection::StyleRoot { .. } | WorkspaceAuxSelection::Style { .. })
            );
            if ui
                .button("Add")
                .on_hover_text(if adding_camera {
                    "Save current view as camera"
                } else if adding_style {
                    "Create scene style"
                } else {
                    "Add renderer"
                })
                .clicked()
            {
                if let Some(object_uid) = self.selected_object {
                    let group = self.selected_renderer_group_name();
                    self.open_renderer_dialog_in_group(object_uid, group);
                } else if let Some(selection) = self.workspace_aux_selection {
                    match selection {
                        WorkspaceAuxSelection::CameraRoot { scene_uid }
                        | WorkspaceAuxSelection::Camera { scene_uid, .. } => {
                            self.open_new_camera_properties(scene_uid, camera);
                        }
                        WorkspaceAuxSelection::StyleRoot { scene_uid }
                        | WorkspaceAuxSelection::Style { scene_uid, .. } => {
                            self.open_new_style_properties(scene_uid);
                        }
                        _ => self.open_aux_renderer_dialog(selection),
                    }
                } else if let Some(object_uid) = self
                    .state
                    .active_object_summaries()
                    .first()
                    .map(|object| object.uid)
                {
                    self.open_renderer_dialog(object_uid);
                }
            }
            let delete_enabled = self.selected_object.is_some()
                || self
                    .workspace_aux_selection
                    .is_some_and(|selection| match selection {
                        WorkspaceAuxSelection::Camera { .. } => true,
                        WorkspaceAuxSelection::Style { .. } => self
                            .style_summary_for_selection(selection)
                            .is_some_and(|style| style.local_index.is_some()),
                        WorkspaceAuxSelection::DensityMap { .. }
                        | WorkspaceAuxSelection::Surface { .. } => true,
                        WorkspaceAuxSelection::CameraRoot { .. }
                        | WorkspaceAuxSelection::StyleRoot { .. } => false,
                    });
            if ui
                .add_enabled(delete_enabled, egui::Button::new("Delete"))
                .clicked()
            {
                if let Some(object_uid) = self.selected_object {
                    if let Some(renderer_uid) = self.selected_renderer {
                        self.delete_renderer(object_uid, renderer_uid);
                    } else {
                        self.delete_workspace_object(NativePropertyObject::Molecule(object_uid));
                    }
                } else if let Some(selection) = self.workspace_aux_selection {
                    self.delete_aux_selection(selection);
                }
            }
            let property_enabled = !matches!(
                self.workspace_aux_selection,
                Some(
                    WorkspaceAuxSelection::CameraRoot { .. }
                        | WorkspaceAuxSelection::StyleRoot { .. }
                )
            ) && self.workspace_aux_selection.is_none_or(|selection| {
                match selection {
                    WorkspaceAuxSelection::Style { .. } => self
                        .style_summary_for_selection(selection)
                        .is_some_and(|style| style.local_index.is_some()),
                    _ => true,
                }
            });
            if ui
                .add_enabled(property_enabled, egui::Button::new("Property"))
                .clicked()
            {
                if let (Some(object_uid), Some(renderer_uid)) =
                    (self.selected_object, self.selected_renderer)
                {
                    self.open_renderer_properties(object_uid, renderer_uid);
                } else if let Some(object_uid) = self.selected_object {
                    self.open_object_properties(object_uid);
                } else if let Some(selection) = self.workspace_aux_selection {
                    match selection {
                        WorkspaceAuxSelection::Camera { .. } => {
                            self.open_camera_properties(selection);
                        }
                        WorkspaceAuxSelection::CameraRoot { .. } => {}
                        WorkspaceAuxSelection::Style { .. } => {
                            self.open_style_properties(selection);
                        }
                        WorkspaceAuxSelection::StyleRoot { .. } => {}
                        _ => self.open_aux_properties(selection),
                    }
                } else {
                    self.open_scene_properties();
                }
            }
        });
    }

    fn workspace_object_drop_target(
        &mut self,
        ui: &mut egui::Ui,
        scene_uid: cuemol_core::Uid,
        target: NativePropertyObject,
        response: &egui::Response,
    ) {
        let drop_rect = response.rect;
        if let Some(source) = response.dnd_hover_payload::<NativeWorkspaceObjectDragPayload>() {
            if source.scene_uid == scene_uid && source.owner != target {
                let y = if ctx_pointer_vertical_fraction(ui.ctx(), drop_rect) < 0.5 {
                    drop_rect.top()
                } else {
                    drop_rect.bottom()
                };
                ui.painter().line_segment(
                    [
                        egui::pos2(drop_rect.left(), y),
                        egui::pos2(drop_rect.right(), y),
                    ],
                    egui::Stroke::new(2.0, ui.visuals().selection.bg_fill),
                );
            }
        }
        let Some(source) = response.dnd_release_payload::<NativeWorkspaceObjectDragPayload>()
        else {
            return;
        };
        if source.scene_uid != scene_uid || source.owner == target {
            return;
        }
        let after = ctx_pointer_vertical_fraction(ui.ctx(), drop_rect) >= 0.5;
        self.move_workspace_object(scene_uid, source.owner, target, after);
    }

    fn move_workspace_object(
        &mut self,
        scene_uid: cuemol_core::Uid,
        source: NativePropertyObject,
        target: NativePropertyObject,
        after: bool,
    ) {
        let undo = self.capture_undo();
        match self.state.move_workspace_object(
            scene_uid,
            source.workspace_kind(),
            source.uid(),
            target.workspace_kind(),
            target.uid(),
            after,
        ) {
            Ok(true) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.set_status("Move object");
            }
            Ok(false) => {}
            Err(error) => self.set_status(format!("Move object error: {error}")),
        }
    }

    fn workspace_molecule_object_context_menu(
        &mut self,
        ui: &mut egui::Ui,
        camera: &mut CameraController,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
    ) {
        let current = self
            .state
            .molecule_selection_text(object_uid)
            .unwrap_or_default()
            .to_string();
        ui.menu_button("Selection", |ui| {
            for (label, expression) in [
                ("Select All", "*"),
                ("Unselect", ""),
                ("Protein", "protein"),
                ("Nucleic acid", "nucleic"),
                ("Water", "water"),
                ("Ligand", "!protein & !nucleic & !water"),
                ("Sugar", "sugar"),
                ("Hydrogen", "hydrogen"),
            ] {
                if ui.button(label).clicked() {
                    self.apply_molecule_selection(
                        object_uid,
                        expression.to_string(),
                        "Changed molecule selection",
                    );
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Invert").clicked() {
                self.apply_molecule_selection(
                    object_uid,
                    invert_selection_expression(&current),
                    "Inverted molecule selection",
                );
                ui.close();
            }
            if ui
                .add_enabled(!current.is_empty(), egui::Button::new("Toggle Sidechain"))
                .clicked()
            {
                self.apply_molecule_selection(
                    object_uid,
                    toggle_sidechain_expression(&current),
                    "Toggled sidechain selection",
                );
                ui.close();
            }
            for (label, by_residue) in [("Around Byresid", true), ("Around", false)] {
                ui.menu_button(label, |ui| {
                    for radius in [3.0_f64, 5.0, 7.0, 10.0] {
                        if ui
                            .add_enabled(
                                !current.is_empty(),
                                egui::Button::new(format!("{radius:.0} Å")),
                            )
                            .clicked()
                        {
                            self.apply_molecule_selection(
                                object_uid,
                                around_selection_expression(&current, radius, by_residue),
                                "Expanded molecule selection",
                            );
                            ui.close();
                        }
                    }
                });
            }
        });
        ui.separator();
        if ui.button("Copy Object").clicked() {
            self.copy_workspace_object_to_clipboard(object_uid);
            ui.close();
        }
        if ui
            .add_enabled(
                self.workspace_renderer_clipboard.is_some(),
                egui::Button::new("Paste Renderer"),
            )
            .clicked()
        {
            self.paste_workspace_renderer_clipboard(scene_uid, object_uid, None);
            ui.close();
        }
        ui.separator();
        if ui.button("New Group...").clicked() {
            self.activate_workspace_scene(scene_uid, camera);
            self.open_renderer_group_dialog(object_uid);
            ui.close();
        }
        if ui.button("New Renderer...").clicked() {
            self.activate_workspace_scene(scene_uid, camera);
            self.open_renderer_dialog(object_uid);
            ui.close();
        }
        if ui.button("Save As ...").clicked() {
            self.activate_workspace_scene(scene_uid, camera);
            self.selected_object = Some(object_uid);
            self.selected_renderer = None;
            self.workspace_aux_selection = None;
            self.choose_save_selected_object();
            ui.close();
        }
        if ui.button("Delete Object").clicked() {
            self.delete_workspace_object(NativePropertyObject::Molecule(object_uid));
            ui.close();
        }
        ui.separator();
        if ui.button("Properties...").clicked() {
            self.open_object_properties(object_uid);
            ui.close();
        }
    }

    fn workspace_aux_object_context_menu(
        &mut self,
        ui: &mut egui::Ui,
        camera: &mut CameraController,
        scene_uid: cuemol_core::Uid,
        selection: WorkspaceAuxSelection,
        owner: NativePropertyObject,
    ) {
        if ui.button("New Renderer...").clicked() {
            self.activate_workspace_scene(scene_uid, camera);
            self.open_aux_renderer_dialog(selection);
            ui.close();
        }
        let can_regenerate = match selection {
            WorkspaceAuxSelection::Surface { surface_uid, .. } => self
                .state
                .surface_summary(surface_uid)
                .is_some_and(|surface| !surface.original_molecule.is_empty()),
            _ => false,
        };
        if can_regenerate && ui.button("Regenerate surface...").clicked() {
            self.activate_workspace_scene(scene_uid, camera);
            self.regenerate_workspace_surface(selection);
            ui.close();
        }
        if ui.button("Delete Object").clicked() {
            self.delete_workspace_object(owner);
            ui.close();
        }
        ui.separator();
        if ui.button("Properties...").clicked() {
            self.open_aux_properties(selection);
            ui.close();
        }
    }

    fn workspace_aux_renderer_context_menu(
        &mut self,
        ui: &mut egui::Ui,
        selection: WorkspaceAuxSelection,
    ) {
        let can_generate_surface = match selection {
            WorkspaceAuxSelection::DensityMap {
                map_uid,
                renderer_uid: Some(renderer_uid),
                ..
            } => self.state.density_map_summary(map_uid).is_some_and(|map| {
                map.renderers.into_iter().any(|renderer| {
                    renderer.uid == renderer_uid && renderer.renderer_type == "isosurf"
                })
            }),
            _ => false,
        };
        if can_generate_surface && ui.button("Generate surface obj").clicked() {
            self.generate_workspace_surface(selection);
            ui.close();
        }
        if can_generate_surface {
            ui.separator();
        }
        if ui.button("Delete").clicked() {
            self.delete_aux_selection(selection);
            ui.close();
        }
        ui.separator();
        if ui.button("Properties...").clicked() {
            self.open_aux_properties(selection);
            ui.close();
        }
    }

    fn workspace_molecule_renderer_row(
        &mut self,
        ui: &mut egui::Ui,
        camera: &mut CameraController,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer: &crate::GuiRendererSummary,
        is_group: bool,
    ) {
        let selected = self.workspace_aux_selection.is_none()
            && self.selected_object == Some(object_uid)
            && self.selected_renderer == Some(renderer.uid);
        let payload = NativeRendererDragPayload {
            object_uid,
            renderer_uid: renderer.uid,
            is_group,
        };
        let drag_source = ui.dnd_drag_source(
            egui::Id::new(("workspace-renderer-drag", renderer.uid.get())),
            payload,
            |ui| {
                let mut visible = renderer.visible;
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(&mut visible, "")
                        .on_hover_text(if is_group {
                            "Toggle all renderers in this group"
                        } else {
                            "Visible"
                        })
                        .changed()
                    {
                        if is_group {
                            self.set_renderer_group_visibility(object_uid, renderer.uid, visible);
                        } else {
                            self.set_renderer_visibility(object_uid, renderer.uid, visible);
                        }
                    }
                    if let Some(locked) = workspace_lock_toggle(ui, renderer.locked, !is_group) {
                        self.set_workspace_renderer_locked(
                            NativePropertyObject::Molecule(object_uid),
                            renderer.uid,
                            locked,
                        );
                    }
                    let label = if is_group {
                        format!("{} (group)", renderer.name)
                    } else {
                        format!("{} ({})", renderer.name, renderer.renderer_type)
                    };
                    let response = ui.selectable_label(selected, label);
                    if response.clicked() {
                        self.activate_workspace_scene(scene_uid, camera);
                        self.selected_object = Some(object_uid);
                        self.selected_renderer = Some(renderer.uid);
                        self.workspace_aux_selection = None;
                    }
                    response.context_menu(|ui| {
                        if is_group {
                            if ui.button("Change Name...").clicked() {
                                self.open_renderer_properties(object_uid, renderer.uid);
                                ui.close();
                            }
                            if ui.button("Copy").clicked() {
                                self.copy_workspace_renderer_to_clipboard(object_uid, renderer.uid);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    self.workspace_renderer_clipboard.is_some(),
                                    egui::Button::new("Paste Renderer"),
                                )
                                .clicked()
                            {
                                self.paste_workspace_renderer_clipboard(
                                    scene_uid,
                                    object_uid,
                                    Some(renderer.uid),
                                );
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("New Renderer...").clicked() {
                                self.open_renderer_dialog_in_group(
                                    object_uid,
                                    Some(renderer.name.clone()),
                                );
                                ui.close();
                            }
                            if ui.button("Delete").clicked() {
                                self.delete_renderer(object_uid, renderer.uid);
                                ui.close();
                            }
                        } else {
                            if !matches!(
                                renderer.renderer_type.as_str(),
                                "*selection" | "selection" | "*namelabel" | "name_label"
                            ) {
                                self.workspace_renderer_selection_menu(
                                    ui,
                                    scene_uid,
                                    object_uid,
                                    renderer.uid,
                                );
                                ui.separator();
                            }
                            if renderer.renderer_type == "*selection"
                                || (!renderer.renderer_type.starts_with('*')
                                    && !matches!(
                                        renderer.renderer_type.as_str(),
                                        "atomintr" | "disorder"
                                    ))
                            {
                                ui.menu_button("Change type", |ui| {
                                    for renderer_type in
                                        RENDERER_TYPES.iter().copied().filter(|renderer_type| {
                                            *renderer_type != renderer.renderer_type
                                                && !matches!(
                                                    *renderer_type,
                                                    "atomintr" | "disorder"
                                                )
                                        })
                                    {
                                        if ui.button(renderer_type).clicked() {
                                            self.change_workspace_renderer_type(
                                                scene_uid,
                                                object_uid,
                                                renderer.uid,
                                                renderer_type,
                                            );
                                            ui.close();
                                        }
                                    }
                                });
                                ui.separator();
                            }
                            if renderer.renderer_type != "*selection" {
                                self.workspace_renderer_style_menus(
                                    ui, scene_uid, object_uid, renderer,
                                );
                            }
                            if ui.button("Create style...").clicked() {
                                self.open_renderer_style_create(
                                    scene_uid,
                                    object_uid,
                                    renderer.uid,
                                );
                                ui.close();
                            }
                            ui.separator();
                            if renderer.renderer_type == "atomintr" {
                                if ui.button("Edit interaction list...").clicked() {
                                    self.open_renderer_properties(object_uid, renderer.uid);
                                    ui.close();
                                }
                                ui.separator();
                            }
                            if ui.button("Copy").clicked() {
                                self.copy_workspace_renderer_to_clipboard(object_uid, renderer.uid);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("New Renderer...").clicked() {
                                self.open_renderer_dialog(object_uid);
                                ui.close();
                            }
                            if ui.button("Delete").clicked() {
                                self.delete_renderer(object_uid, renderer.uid);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Properties...").clicked() {
                                self.open_renderer_properties(object_uid, renderer.uid);
                                ui.close();
                            }
                        }
                    });
                });
            },
        );
        let drop_rect = drag_source.response.rect;
        if let Some(source) = drag_source
            .response
            .dnd_hover_payload::<NativeRendererDragPayload>()
        {
            if source.object_uid == object_uid
                && source.renderer_uid != renderer.uid
                && (!source.is_group || renderer.group.is_none())
            {
                let fraction = ctx_pointer_vertical_fraction(ui.ctx(), drop_rect);
                let inside = is_group && !source.is_group && (0.25..=0.75).contains(&fraction);
                let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                if inside {
                    ui.painter()
                        .rect_stroke(drop_rect, 0.0, stroke, egui::StrokeKind::Inside);
                } else {
                    let y = if fraction < 0.5 {
                        drop_rect.top()
                    } else {
                        drop_rect.bottom()
                    };
                    ui.painter().line_segment(
                        [
                            egui::pos2(drop_rect.left(), y),
                            egui::pos2(drop_rect.right(), y),
                        ],
                        stroke,
                    );
                }
            }
        }
        let Some(source) = drag_source
            .response
            .dnd_release_payload::<NativeRendererDragPayload>()
        else {
            return;
        };
        if source.object_uid != object_uid || source.renderer_uid == renderer.uid {
            return;
        }
        if source.is_group && renderer.group.is_some() {
            self.set_status("Renderer groups cannot be moved into another group");
            return;
        }
        let fraction = ctx_pointer_vertical_fraction(ui.ctx(), drop_rect);
        let position = if is_group && !source.is_group && (0.25..=0.75).contains(&fraction) {
            "inside"
        } else if fraction < 0.5 {
            "before"
        } else {
            "after"
        };
        self.move_workspace_renderer(
            scene_uid,
            object_uid,
            source.renderer_uid,
            renderer.uid,
            position,
        );
    }

    fn workspace_renderer_style_menus(
        &mut self,
        ui: &mut egui::Ui,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer: &crate::GuiRendererSummary,
    ) {
        let options = self
            .state
            .renderer_style_options(scene_uid, &renderer.renderer_type);
        let renderer_type = renderer
            .renderer_type
            .trim_start_matches('*')
            .to_ascii_lowercase();
        let suffix = match renderer_type.as_str() {
            "name_label" | "namelabel" => "label",
            "gpu_mapmesh" => "contour",
            other => other,
        };
        let shape = options
            .iter()
            .filter(|style| style.name.to_ascii_lowercase().ends_with(suffix))
            .collect::<Vec<_>>();
        let edge = options
            .iter()
            .filter(|style| style.name.to_ascii_lowercase().starts_with("egline"))
            .collect::<Vec<_>>();
        let paint = options
            .iter()
            .filter(|style| style.name.to_ascii_lowercase().ends_with("paint"))
            .collect::<Vec<_>>();
        let supports_coloring = !matches!(
            renderer.renderer_type.as_str(),
            "*selection" | "selection" | "*namelabel" | "name_label" | "atomintr"
        );
        ui.add_enabled_ui(supports_coloring, |ui| {
            ui.menu_button("Coloring", |ui| {
                for (label, style_name) in [
                    ("CPK molcol", "DefaultCPKColoring"),
                    ("CPK dark gray", "DarkCPKColoring"),
                    ("CPK light gray", "LightCPKColoring"),
                ] {
                    if options
                        .iter()
                        .any(|style| style.name.eq_ignore_ascii_case(style_name))
                        && ui.button(label).clicked()
                    {
                        self.apply_workspace_renderer_style(
                            scene_uid,
                            object_uid,
                            renderer.uid,
                            style_name,
                            "coloring",
                        );
                        ui.close();
                    }
                }
                for style in &paint {
                    let label = if style.description.is_empty() {
                        style.name.as_str()
                    } else {
                        style.description.as_str()
                    };
                    if ui.button(label).clicked() {
                        self.apply_workspace_renderer_style(
                            scene_uid,
                            object_uid,
                            renderer.uid,
                            &style.name,
                            "coloring",
                        );
                        ui.close();
                    }
                }
                ui.separator();
                for (label, preset) in [("B-factor", "bfactor"), ("Rainbow", "rainbow")] {
                    if ui.button(label).clicked() {
                        self.set_workspace_renderer_coloring_preset(
                            scene_uid,
                            object_uid,
                            renderer.uid,
                            preset,
                        );
                        ui.close();
                    }
                }
            });
        });
        let paint_enabled = self
            .state
            .renderer_coloring_type(object_uid, renderer.uid)
            .is_some_and(|coloring| coloring == "PaintColoring");
        ui.add_enabled_ui(paint_enabled, |ui| {
            ui.menu_button("Paint", |ui| {
                ui.menu_button("Monochrome", |ui| {
                    for &(label, color) in WORKSPACE_PAINT_MONOCHROME {
                        if ui.button(label).clicked() {
                            self.insert_workspace_renderer_paint(
                                scene_uid,
                                object_uid,
                                renderer.uid,
                                color,
                            );
                            ui.close();
                        }
                    }
                });
                for &(hue_name, hue) in WORKSPACE_PAINT_HUES {
                    ui.menu_button(hue_name, |ui| {
                        let variants = [
                            (hue_name.to_string(), format!("hsb({hue}, 1.0, 1.0)")),
                            (
                                format!("{hue_name}, sat 25%"),
                                format!("hsb({hue}, 0.25, 1.0)"),
                            ),
                            (
                                format!("{hue_name}, sat 50%"),
                                format!("hsb({hue}, 0.5, 1.0)"),
                            ),
                            (
                                format!("{hue_name}, sat 75%"),
                                format!("hsb({hue}, 0.75, 1.0)"),
                            ),
                            (
                                format!("{hue_name}, bri 75%"),
                                format!("hsb({hue}, 1.0, 0.75)"),
                            ),
                            (
                                format!("{hue_name}, bri 50%"),
                                format!("hsb({hue}, 1.0, 0.50)"),
                            ),
                            (
                                format!("{hue_name}, bri 25%"),
                                format!("hsb({hue}, 1.0, 0.25)"),
                            ),
                        ];
                        for (label, color) in variants {
                            if ui.button(label).clicked() {
                                self.insert_workspace_renderer_paint(
                                    scene_uid,
                                    object_uid,
                                    renderer.uid,
                                    &color,
                                );
                                ui.close();
                            }
                        }
                    });
                }
            });
        });
        ui.menu_button("Style", |ui| {
            for style in &shape {
                let label = if style.description.is_empty() {
                    style.name.as_str()
                } else {
                    style.description.as_str()
                };
                if ui.button(label).clicked() {
                    self.apply_workspace_renderer_style(
                        scene_uid,
                        object_uid,
                        renderer.uid,
                        &style.name,
                        "shape",
                    );
                    ui.close();
                }
            }
            if !shape.is_empty() && !edge.is_empty() {
                ui.separator();
            }
            for style in &edge {
                let label = if style.description.is_empty() {
                    style.name.as_str()
                } else {
                    style.description.as_str()
                };
                if ui.button(label).clicked() {
                    self.apply_workspace_renderer_style(
                        scene_uid,
                        object_uid,
                        renderer.uid,
                        &style.name,
                        "edge",
                    );
                    ui.close();
                }
            }
        });
        if ui.button("Edit style...").clicked() {
            self.open_renderer_properties(object_uid, renderer.uid);
            ui.close();
        }
    }

    fn apply_workspace_renderer_style(
        &mut self,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        style_name: &str,
        component: &str,
    ) {
        let undo = self.capture_undo();
        match self.state.apply_renderer_style_option(
            object_uid,
            renderer_uid,
            style_name,
            component,
        ) {
            Ok(_) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.rebuild_snapshot();
                self.set_status(if component == "coloring" {
                    "Change coloring style"
                } else {
                    "Change style"
                });
            }
            Err(error) => self.set_status(format!("Renderer style error: {error}")),
        }
    }

    fn set_workspace_renderer_coloring_preset(
        &mut self,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        preset: &str,
    ) {
        let undo = self.capture_undo();
        match self
            .state
            .set_workspace_renderer_coloring_preset(object_uid, renderer_uid, preset)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.rebuild_snapshot();
                self.set_status("Change coloring");
            }
            Err(error) => self.set_status(format!("Renderer coloring error: {error}")),
        }
    }

    fn insert_workspace_renderer_paint(
        &mut self,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        color: &str,
    ) {
        let undo = self.capture_undo();
        match self
            .state
            .insert_renderer_paint_entry(object_uid, renderer_uid, color)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.rebuild_snapshot();
                self.set_status("Insert paint entry");
            }
            Err(error) => self.set_status(format!("Paint error: {error}")),
        }
    }

    fn copy_workspace_renderer_to_clipboard(
        &mut self,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) {
        match self.state.copy_workspace_renderer(object_uid, renderer_uid) {
            Ok(clipboard) => {
                let count = clipboard.renderer_count();
                self.workspace_renderer_clipboard = Some(clipboard);
                self.set_status(format!("Copied {count} renderer(s)"));
            }
            Err(error) => self.set_status(format!("Copy renderer error: {error}")),
        }
    }

    fn copy_workspace_object_to_clipboard(&mut self, object_uid: cuemol_core::Uid) {
        match self.state.copy_workspace_object(object_uid) {
            Ok(clipboard) => {
                let name = clipboard.name().to_string();
                self.workspace_object_clipboard = Some(clipboard);
                self.set_status(format!("Copied object {name}"));
            }
            Err(error) => self.set_status(format!("Copy object error: {error}")),
        }
    }

    fn paste_workspace_object_clipboard(&mut self, scene_uid: cuemol_core::Uid) {
        let Some(clipboard) = self.workspace_object_clipboard.clone() else {
            self.set_status("No copied object");
            return;
        };
        let undo = self.capture_undo();
        match self.state.paste_workspace_object(scene_uid, &clipboard) {
            Ok(object_uid) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.selected_object = Some(object_uid);
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
                self.rebuild_snapshot();
                self.set_status("Paste object");
            }
            Err(error) => self.set_status(format!("Paste object error: {error}")),
        }
    }

    fn paste_workspace_renderer_clipboard(
        &mut self,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        destination_group_uid: Option<cuemol_core::Uid>,
    ) {
        let Some(clipboard) = self.workspace_renderer_clipboard.clone() else {
            self.set_status("No copied renderer");
            return;
        };
        let undo = self.capture_undo();
        match self
            .state
            .paste_workspace_renderers(object_uid, destination_group_uid, &clipboard)
        {
            Ok(result) => {
                let count = result.renderer_uids.len();
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.selected_object = Some(object_uid);
                self.selected_renderer = result
                    .group_uid
                    .or_else(|| result.renderer_uids.first().copied());
                self.workspace_aux_selection = None;
                self.rebuild_snapshot();
                self.set_status(format!("Pasted {count} renderer(s)"));
            }
            Err(error) => self.set_status(format!("Paste renderer error: {error}")),
        }
    }

    fn change_workspace_renderer_type(
        &mut self,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        renderer_type: &str,
    ) {
        let undo = self.capture_undo();
        match self
            .state
            .change_renderer_type_in_object(object_uid, renderer_uid, renderer_type)
        {
            Ok(new_uid) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.selected_object = Some(object_uid);
                self.selected_renderer = Some(new_uid);
                self.workspace_aux_selection = None;
                self.rebuild_snapshot();
                self.set_status(format!("Change renderer type to {renderer_type}"));
            }
            Err(error) => self.set_status(format!("Renderer type error: {error}")),
        }
    }

    fn workspace_renderer_selection_menu(
        &mut self,
        ui: &mut egui::Ui,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) {
        let current = self
            .state
            .molecule_selection_text(object_uid)
            .unwrap_or_default()
            .to_string();
        ui.menu_button("Change selection", |ui| {
            if ui
                .add_enabled(!current.is_empty(), egui::Button::new("Current"))
                .clicked()
            {
                self.set_workspace_renderer_selection(
                    scene_uid,
                    object_uid,
                    renderer_uid,
                    &current,
                );
                ui.close();
            }
            for (label, selection) in [
                ("All", "*"),
                ("Protein", "protein"),
                ("Nucleic acid", "nucleic"),
                ("Water", "water"),
                ("Ligand", "!protein & !nucleic & !water"),
                ("Sugar", "sugar"),
            ] {
                if ui.button(label).clicked() {
                    self.set_workspace_renderer_selection(
                        scene_uid,
                        object_uid,
                        renderer_uid,
                        selection,
                    );
                    ui.close();
                }
            }
        });
    }

    fn set_workspace_renderer_selection(
        &mut self,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        selection: &str,
    ) {
        let undo = self.capture_undo();
        match self
            .state
            .set_renderer_selection_text(object_uid, renderer_uid, selection)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.rebuild_snapshot();
                self.set_status("Change renderer selection");
            }
            Err(error) => self.set_status(format!("Renderer selection error: {error}")),
        }
    }

    fn move_workspace_renderer(
        &mut self,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        target_uid: cuemol_core::Uid,
        position: &str,
    ) {
        let undo = self.capture_undo();
        match self
            .state
            .move_renderer_in_object(object_uid, renderer_uid, target_uid, position)
        {
            Ok(true) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.set_status("Move renderer");
            }
            Ok(false) => {}
            Err(error) => self.set_status(format!("Move renderer error: {error}")),
        }
    }

    fn activate_workspace_scene(
        &mut self,
        scene_uid: cuemol_core::Uid,
        camera: &mut CameraController,
    ) {
        if self.state.active_scene() == Some(scene_uid) {
            return;
        }
        self.state.set_active_scene(Some(scene_uid));
        self.sync_active_scene_properties();
        self.rebuild_snapshot();
        camera.fit_snapshot(&self.snapshot);
    }

    fn selected_renderer_is_group(&self) -> bool {
        let (Some(object_uid), Some(renderer_uid)) = (self.selected_object, self.selected_renderer)
        else {
            return false;
        };
        self.state
            .renderer_summaries(object_uid)
            .into_iter()
            .any(|renderer| renderer.uid == renderer_uid && renderer.renderer_type == "*group")
    }

    fn selected_renderer_group_name(&self) -> Option<String> {
        let (object_uid, renderer_uid) = (self.selected_object?, self.selected_renderer?);
        self.state
            .renderer_summaries(object_uid)
            .into_iter()
            .find(|renderer| renderer.uid == renderer_uid && renderer.renderer_type == "*group")
            .map(|renderer| renderer.name)
    }

    fn zoom_workspace_selection(&mut self, camera: &mut CameraController) {
        match self.fit_workspace_selection(camera) {
            Ok(true) => self.set_status("View fitted"),
            Ok(false) => self.set_status("Selected item cannot be fitted"),
            Err(error) => self.set_status(format!("Zoom error: {error}")),
        }
    }

    fn fit_workspace_selection(
        &self,
        camera: &mut CameraController,
    ) -> std::result::Result<bool, String> {
        let scene_uid = self
            .state
            .active_scene()
            .ok_or_else(|| "no active scene".to_string())?;
        let mut view = View::new(cuemol_core::Uid::new(1), scene_uid);
        view.set_camera(&camera.to_qsys_camera("Workspace fit"));
        let viewport = self
            .viewport_rect
            .map(|rect| rect.size())
            .unwrap_or_else(|| egui::vec2(800.0, 600.0));
        view.set_view_size(
            viewport.x.max(1.0).round() as u32,
            viewport.y.max(1.0).round() as u32,
        );
        let fitted = if self.selected_renderer_is_group() {
            false
        } else if let Some(object_uid) = self.selected_object {
            self.state
                .fit_molecule_view(object_uid, self.selected_renderer, &mut view)
                .map_err(|error| error.to_string())?
        } else {
            match self.workspace_aux_selection {
                Some(WorkspaceAuxSelection::DensityMap { map_uid, .. }) => {
                    self.state
                        .fit_density_map_view(map_uid, &mut view)
                        .map_err(|error| error.to_string())?;
                    true
                }
                Some(
                    WorkspaceAuxSelection::CameraRoot { .. }
                    | WorkspaceAuxSelection::Camera { .. }
                    | WorkspaceAuxSelection::StyleRoot { .. }
                    | WorkspaceAuxSelection::Style { .. }
                    | WorkspaceAuxSelection::Surface { .. },
                )
                | None => false,
            }
        };
        if fitted {
            *camera = CameraController::from_qsys_camera(&view.camera());
        }
        Ok(fitted)
    }

    fn molecule_panel_cache_key(&self) -> MoleculePanelCacheKey {
        let scene_uid = self.state.active_scene();
        let objects = scene_uid
            .and_then(|uid| self.state.scene_manager().scene(uid))
            .map(|scene| {
                scene
                    .objects()
                    .iter()
                    .map(|object| {
                        (
                            object.uid(),
                            object.name().to_string(),
                            object.mol().atom_count(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        MoleculePanelCacheKey { scene_uid, objects }
    }

    fn cached_active_structure_tree(&mut self) -> Arc<Vec<GuiStructObject>> {
        let key = self.molecule_panel_cache_key();
        let current = self
            .molstruct_tree_cache
            .as_ref()
            .is_some_and(|(cached_key, _)| cached_key == &key);
        if !current {
            self.molstruct_tree_cache = Some((key, Arc::new(self.state.active_structure_tree())));
        }
        Arc::clone(
            &self
                .molstruct_tree_cache
                .as_ref()
                .expect("molecule structure cache was initialized")
                .1,
        )
    }

    fn cached_active_sequence_summaries(&mut self) -> Arc<Vec<GuiSequenceSummary>> {
        let key = self.molecule_panel_cache_key();
        let current = self
            .sequence_summary_cache
            .as_ref()
            .is_some_and(|(cached_key, _)| cached_key == &key);
        if !current {
            self.sequence_summary_cache =
                Some((key, Arc::new(self.state.active_sequence_summaries())));
        }
        Arc::clone(
            &self
                .sequence_summary_cache
                .as_ref()
                .expect("molecule sequence cache was initialized")
                .1,
        )
    }

    fn invalidate_molecule_panel_caches(&mut self) {
        self.molstruct_tree_cache = None;
        self.sequence_summary_cache = None;
    }

    /// MolStruct section — the molecule's chain → residue → atom hierarchy
    /// (CueMol2 `molstruct-panel.xul` `molStructTree`): a molecule selector menulist,
    /// a Select/Center/Zoom/Property toolbar, and a collapsible tree where each row is
    /// click-to-select (the triangle toggles, the label selects, like the XUL tree).
    fn molstruct_section(&mut self, ui: &mut egui::Ui, camera: &mut CameraController) {
        let tree = self.cached_active_structure_tree();
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
        let before = selected;
        let selected_name = tree
            .iter()
            .find(|object| Some(object.object_uid) == selected)
            .map(|object| object.object_name.clone())
            .unwrap_or_default();
        egui::ComboBox::from_id_salt("molstruct-molecule")
            .selected_text(selected_name)
            .show_ui(ui, |ui| {
                for object in tree.iter() {
                    ui.selectable_value(
                        &mut selected,
                        Some(object.object_uid),
                        &object.object_name,
                    );
                }
            });
        self.molstruct_object = selected;
        if selected != before {
            if let Some(object_uid) = selected {
                self.selection_text = self
                    .state
                    .molecule_selection_text(object_uid)
                    .unwrap_or_default()
                    .to_string();
            }
        }
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
                self.selected_object = Some(object.object_uid);
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
                self.open_object_properties(object.object_uid);
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
            self.apply_molecule_selection(object.object_uid, sel, "Selected molecule atoms");
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
        for atom in self
            .state
            .active_pick_atoms(&self.selection_text)
            .unwrap_or_default()
            .iter()
            .filter(|atom| atom.selected)
        {
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
        let sequences = self.cached_active_sequence_summaries();
        if !sequences.is_empty() {
            let mut selected = self
                .molstruct_object
                .filter(|uid| sequences.iter().any(|seq| seq.object_uid == *uid))
                .or_else(|| sequences.first().map(|seq| seq.object_uid));
            let before = selected;
            let name = sequences
                .iter()
                .find(|seq| Some(seq.object_uid) == selected)
                .map(|seq| seq.object_name.clone())
                .unwrap_or_default();
            egui::ComboBox::from_id_salt("selection-molecule")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    let mut seen = std::collections::BTreeSet::new();
                    for seq in sequences.iter() {
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
            if selected != before {
                if let Some(object_uid) = selected {
                    self.selection_text = self
                        .state
                        .molecule_selection_text(object_uid)
                        .unwrap_or_default()
                        .to_string();
                }
            }
        }

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.selection_tab, SelectionTab::Command, "Command");
            ui.selectable_value(&mut self.selection_tab, SelectionTab::Editor, "Editor");
        });
        ui.separator();

        match self.selection_tab {
            SelectionTab::Command => self.selection_command_tab(ui),
            SelectionTab::Editor => self.selection_editor_tab(ui, sequences.as_slice()),
        }

        let selected_count = self
            .state
            .active_pick_atoms(&self.selection_text)
            .unwrap_or_default()
            .into_iter()
            .filter(|atom| atom.selected)
            .count();
        ui.separator();
        ui.label(format!("Selected: {selected_count} atoms"));
    }

    /// Command tab: the free-text selection expression + history (CueMol2 Command tab).
    fn selection_command_tab(&mut self, ui: &mut egui::Ui) {
        ui.label("Selection command");
        ui.add_sized(
            [ui.available_width(), 56.0],
            egui::TextEdit::multiline(&mut self.selection_text)
                .hint_text("name CA")
                .font(egui::TextStyle::Monospace),
        );
        ui.horizontal(|ui| {
            if ui.button("Select").clicked() {
                if let Some(object_uid) = self.selection_target_object() {
                    self.apply_molecule_selection(
                        object_uid,
                        self.selection_text.clone(),
                        "Applied molecule selection",
                    );
                } else {
                    self.set_status("No molecule selected");
                }
            }
            if ui.button("Clear").clicked() {
                if let Some(object_uid) = self.selection_target_object() {
                    self.apply_molecule_selection(
                        object_uid,
                        String::new(),
                        "Cleared molecule selection",
                    );
                } else {
                    self.selection_text.clear();
                    self.rebuild_snapshot();
                }
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
                if let Some(object_uid) = self.selection_target_object() {
                    self.apply_molecule_selection(object_uid, sel, "Applied selection history");
                }
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
            if let Some(object_uid) = self.selection_target_object() {
                self.apply_molecule_selection(object_uid, expr, "Applied molecule selection");
            }
        } else if let Some(kind) = wrap {
            let current = self.selection_text.trim();
            if !current.is_empty() {
                let expression = match kind.as_str() {
                    "byres" => format!("byres ({current})"),
                    "around" => format!("({current}) around 5"),
                    "invert" => format!("!({current})"),
                    _ => current.to_string(),
                };
                if let Some(object_uid) = self.selection_target_object() {
                    self.apply_molecule_selection(
                        object_uid,
                        expression,
                        "Modified molecule selection",
                    );
                }
            }
        }
    }

    /// Push the current selection text onto the most-recently-used history list.
    fn push_selection_history(&mut self) {
        let sel = self.selection_text.trim().to_string();
        self.remember_selection_history(&sel);
    }

    fn remember_selection_history(&mut self, selection: &str) {
        let selection = selection.trim();
        if selection.is_empty() {
            return;
        }
        self.selection_history.retain(|entry| entry != selection);
        self.selection_history.push(selection.to_string());
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

    fn open_crystal_editor(&mut self, object_uid: cuemol_core::Uid) {
        let draft = self
            .state
            .crystal_summary(object_uid)
            .map(|crystal| NativeCrystalDraft {
                object_uid,
                lattice: crystal.lattice,
                space_group_number: crystal.space_group_number.unwrap_or(1),
                restrict_by_symmetry: false,
                a: crystal.a,
                b: crystal.b,
                c: crystal.c,
                alpha: crystal.alpha,
                beta: crystal.beta,
                gamma: crystal.gamma,
            })
            .unwrap_or(NativeCrystalDraft {
                object_uid,
                lattice: "TRICLINIC".to_string(),
                space_group_number: 1,
                restrict_by_symmetry: false,
                a: 100.0,
                b: 100.0,
                c: 100.0,
                alpha: 90.0,
                beta: 90.0,
                gamma: 90.0,
            });
        self.crystal_draft = Some(draft);
        self.tool_dialog = Some("Change symmetry".to_string());
    }

    fn crystal_tool_ui(&mut self, ui: &mut egui::Ui) {
        let Some(mut draft) = self.crystal_draft.clone() else {
            ui.label("Select a molecule first.");
            return;
        };
        let object_name = self
            .state
            .active_object_summaries()
            .into_iter()
            .find(|object| object.uid == draft.object_uid)
            .map_or_else(
                || format!("object {}", draft.object_uid.get()),
                |object| object.name,
            );
        ui.label(egui::RichText::new(object_name).strong());
        ui.separator();

        egui::ComboBox::from_label("Crystal system")
            .selected_text(lattice_label(&draft.lattice))
            .show_ui(ui, |ui| {
                for lattice in CRYSTAL_LATTICES {
                    ui.selectable_value(&mut draft.lattice, lattice.0.to_string(), lattice.1);
                }
            });
        let space_groups = self.state.space_group_summaries(&draft.lattice);
        if !space_groups
            .iter()
            .any(|group| group.id == draft.space_group_number)
        {
            draft.space_group_number = space_groups.first().map_or(1, |group| group.id);
        }
        let selected_space_group = space_groups
            .iter()
            .find(|group| group.id == draft.space_group_number)
            .map_or("(unknown)", |group| group.canonical_name.as_str());
        egui::ComboBox::from_label("Space Group")
            .selected_text(selected_space_group)
            .show_ui(ui, |ui| {
                for group in &space_groups {
                    ui.selectable_value(
                        &mut draft.space_group_number,
                        group.id,
                        &group.canonical_name,
                    );
                }
            });
        ui.label(format!("Space Group Number: {}", draft.space_group_number));
        ui.checkbox(&mut draft.restrict_by_symmetry, "Restrict by symmetry");

        let selected_space_group = space_groups
            .iter()
            .find(|group| group.id == draft.space_group_number)
            .map_or("", |group| group.canonical_name.as_str());
        restrict_crystal_draft(&mut draft, selected_space_group);
        let (lock_b, lock_c, lock_alpha, lock_beta, lock_gamma) =
            crystal_field_locks(&draft, selected_space_group);
        egui::Grid::new("native-crystal-cell-grid")
            .num_columns(4)
            .show(ui, |ui| {
                ui.label("a=");
                ui.add(egui::DragValue::new(&mut draft.a).speed(0.1));
                ui.label("α=");
                ui.add_enabled(
                    !lock_alpha,
                    egui::DragValue::new(&mut draft.alpha).speed(0.1),
                );
                ui.end_row();
                ui.label("b=");
                ui.add_enabled(!lock_b, egui::DragValue::new(&mut draft.b).speed(0.1));
                ui.label("β=");
                ui.add_enabled(!lock_beta, egui::DragValue::new(&mut draft.beta).speed(0.1));
                ui.end_row();
                ui.label("c=");
                ui.add_enabled(!lock_c, egui::DragValue::new(&mut draft.c).speed(0.1));
                ui.label("γ=");
                ui.add_enabled(
                    !lock_gamma,
                    egui::DragValue::new(&mut draft.gamma).speed(0.1),
                );
                ui.end_row();
            });
        restrict_crystal_draft(&mut draft, selected_space_group);

        let mut applied = false;
        if ui.button("OK").clicked() {
            let undo = self.capture_undo();
            match self.state.change_crystal_info(
                draft.object_uid,
                draft.a,
                draft.b,
                draft.c,
                draft.alpha,
                draft.beta,
                draft.gamma,
                draft.space_group_number,
            ) {
                Ok(()) => {
                    self.push_undo_snapshot(undo);
                    self.rebuild_snapshot();
                    self.set_status("Changed symmetry information");
                    self.tool_dialog = None;
                    applied = true;
                }
                Err(error) => self.set_error(error),
            }
        }
        self.crystal_draft = (!applied).then_some(draft);
    }

    /// Symmetry section — crystal cell / space group table (CueMol2 `symmetry-panel.xul`).
    fn symmetry_section(&mut self, ui: &mut egui::Ui) {
        let crystals = self.state.active_crystal_summaries();
        if crystals.is_empty() {
            ui.weak("No crystal cell (non-crystallographic).");
        }
        for cell in &crystals {
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
        let objects = self.state.active_object_summaries();
        let target = self
            .selected_object
            .filter(|uid| objects.iter().any(|object| object.uid == *uid))
            .or_else(|| objects.first().map(|object| object.uid));
        if ui
            .add_enabled(target.is_some(), egui::Button::new("Change symmetry…"))
            .clicked()
        {
            self.open_crystal_editor(target.expect("enabled symmetry target"));
        }
        ui.separator();
        // CueMol2 Symmetry panel "Unit cell" / "Symm mol" toggles: draw the cell box
        // and the home-cell symmetry-mate copies.
        let mut changed = false;
        ui.add_enabled_ui(!crystals.is_empty(), |ui| {
            changed |= ui.checkbox(&mut self.show_unit_cell, "Unit cell").changed();
            changed |= ui
                .checkbox(&mut self.show_symm_mates, "Symm mol")
                .on_hover_text("Show symmetry-related molecules in the unit cell")
                .changed();
        });
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
        let total = self
            .animation_objects
            .iter()
            .map(|object| object.duration.max(0.01))
            .sum::<f64>() as f32;
        if total > 0.0 && !self.animation_loop && self.animation_elapsed + dt >= total {
            self.animation_elapsed = total;
            self.animation_playing = false;
            return;
        }
        let frame = anim_step(
            &self.animation_objects,
            self.animation_elapsed as f64,
            dt as f64,
        );
        self.apply_animation_frame(frame, dt, camera);
    }

    fn apply_animation_frame(
        &mut self,
        frame: crate::AnimFrame,
        dt: f32,
        camera: &mut CameraController,
    ) {
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

    fn start_animation_render(&mut self, camera: &CameraController) {
        if self.animation_render_job.is_some() {
            return;
        }
        if self.animation_objects.is_empty() {
            self.set_status("No animation in active scene");
            return;
        }
        let mut settings = self.animation_render_settings.clone();
        settings.width = settings.width.clamp(100, 16_384);
        settings.height = settings.height.clamp(100, 16_384);
        settings.fps = settings.fps.clamp(1, 240);
        settings.concurrency = settings.concurrency.clamp(1, 32);
        settings.base_name = sanitize_animation_base_name(&settings.base_name);
        if settings.backend == "POV-Ray" && settings.povray_executable.trim().is_empty() {
            self.set_status("POV-Ray executable is required");
            return;
        }
        if settings.backend == "POV-Ray"
            && !settings.povray_include_dir.trim().is_empty()
            && !Path::new(settings.povray_include_dir.trim()).is_dir()
        {
            self.set_status("POV-Ray include directory does not exist");
            return;
        }
        if settings.make_movie && settings.ffmpeg_executable.trim().is_empty() {
            self.set_status("FFmpeg executable is required");
            return;
        }
        let output_dir = PathBuf::from(settings.output_dir.trim());
        if let Err(error) = fs::create_dir_all(&output_dir) {
            self.set_status(format!("Error: {error}"));
            return;
        }
        let total_frames = animation_render_frame_count(
            &self.animation_objects,
            settings.fps,
            settings.duplicate_last_frame,
        );
        if total_frames == 0 {
            self.set_status("Animation has no renderable frames");
            return;
        }

        self.animation_render_settings = settings.clone();
        self.animation_render_log.clear();
        self.animation_render_total_frames = total_frames;
        self.animation_preview_frame = 0;
        self.animation_preview_texture = None;
        self.animation_render_log.push(format!(
            "Starting {} frames at {}x{} / {} fps",
            total_frames, settings.width, settings.height, settings.fps
        ));
        let restore = self.capture_animation_render_restore(camera);
        let worker_capacity = settings.concurrency as usize;
        self.animation_elapsed = 0.0;
        self.animation_playing = false;
        self.animation_render_job = Some(AnimationRenderJob {
            settings,
            restore,
            total_frames,
            next_frame: 0,
            completed_frames: 0,
            frame_paths: Vec::with_capacity(total_frames as usize),
            frame_receivers: Vec::with_capacity(worker_capacity),
            movie_receiver: None,
            progress: Arc::new(AtomicU32::new(0)),
            cancel: Arc::new(AtomicBool::new(false)),
            encoding_started: false,
        });
        self.set_status("Animation rendering started");
    }

    fn capture_animation_render_restore(
        &self,
        camera: &CameraController,
    ) -> AnimationRenderRestore {
        let renderer_visibility = self
            .state
            .active_object_summaries()
            .into_iter()
            .flat_map(|object| {
                self.state
                    .renderer_summaries(object.uid)
                    .into_iter()
                    .map(move |renderer| (object.uid, renderer.uid, renderer.visible))
            })
            .collect();
        let trajectory_frames = self
            .state
            .active_trajectory_summaries()
            .into_iter()
            .map(|trajectory| (trajectory.object_uid, trajectory.current_frame))
            .collect();
        AnimationRenderRestore {
            renderer_visibility,
            trajectory_frames,
            camera: camera.clone(),
            elapsed: self.animation_elapsed,
        }
    }

    fn poll_animation_render(&mut self, camera: &mut CameraController) {
        let Some(mut job) = self.animation_render_job.take() else {
            return;
        };

        let mut pending_receivers = Vec::with_capacity(job.frame_receivers.len());
        let mut frame_failure = None;
        for receiver in std::mem::take(&mut job.frame_receivers) {
            match receiver.try_recv() {
                Ok(Ok(AnimationTaskOutput::Frame { index, path, log })) => {
                    job.completed_frames += 1;
                    job.frame_paths.push(path.clone());
                    self.animation_render_log
                        .push(format!("Frame {index:04}: {}", path.to_string_lossy()));
                    if !log.trim().is_empty() {
                        self.animation_render_log.push(log);
                    }
                }
                Ok(Ok(AnimationTaskOutput::Movie { .. })) => {
                    frame_failure.get_or_insert_with(|| {
                        "Animation frame worker returned a movie".to_string()
                    });
                }
                Ok(Err(error)) => {
                    if !job.cancel.load(Ordering::Relaxed) {
                        frame_failure
                            .get_or_insert_with(|| format!("Animation rendering failed: {error}"));
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    pending_receivers.push(receiver);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    frame_failure
                        .get_or_insert_with(|| "Animation render worker disconnected".to_string());
                }
            }
        }
        job.frame_receivers = pending_receivers;

        if let Some(message) = frame_failure {
            job.cancel.store(true, Ordering::Relaxed);
            self.animation_render_log.push(message.clone());
            self.finish_animation_render(job, camera, &message);
            return;
        }

        if let Some(receiver) = job.movie_receiver.take() {
            match receiver.try_recv() {
                Ok(Ok(AnimationTaskOutput::Movie { path, log })) => {
                    if !log.trim().is_empty() {
                        self.animation_render_log.push(log);
                    }
                    self.animation_render_log
                        .push(format!("Movie: {}", path.to_string_lossy()));
                    self.finish_animation_render(job, camera, "Animation movie rendered");
                    return;
                }
                Ok(Ok(AnimationTaskOutput::Frame { .. })) => {
                    let message = "Animation movie worker returned a frame";
                    self.animation_render_log.push(message.to_string());
                    self.finish_animation_render(job, camera, message);
                    return;
                }
                Ok(Err(error)) => {
                    let message = if job.cancel.load(Ordering::Relaxed) {
                        "Animation rendering cancelled".to_string()
                    } else {
                        format!("Animation rendering failed: {error}")
                    };
                    self.animation_render_log.push(message.clone());
                    self.finish_animation_render(job, camera, &message);
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    job.movie_receiver = Some(receiver);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let message = "Animation movie worker disconnected";
                    self.animation_render_log.push(message.to_string());
                    self.finish_animation_render(job, camera, message);
                    return;
                }
            }
        }

        if job.cancel.load(Ordering::Relaxed) {
            if job.frame_receivers.is_empty() && job.movie_receiver.is_none() {
                self.finish_animation_render(job, camera, "Animation rendering cancelled");
            } else {
                self.animation_render_job = Some(job);
            }
            return;
        }

        let concurrency = job.settings.concurrency as usize;
        while job.next_frame < job.total_frames && job.frame_receivers.len() < concurrency {
            self.schedule_animation_frame(&mut job, camera);
        }
        if !job.frame_receivers.is_empty() {
            self.animation_render_job = Some(job);
            return;
        }

        if job.settings.make_movie && !job.encoding_started {
            match spawn_animation_movie_task(&job) {
                Ok(receiver) => {
                    job.encoding_started = true;
                    job.movie_receiver = Some(receiver);
                    job.progress.store(990_000, Ordering::Relaxed);
                    self.animation_render_log
                        .push("Encoding movie with FFmpeg".to_string());
                    self.animation_render_job = Some(job);
                }
                Err(error) => {
                    self.animation_render_log.push(error.clone());
                    self.finish_animation_render(job, camera, &error);
                }
            }
            return;
        }

        self.finish_animation_render(job, camera, "Animation frames rendered");
    }

    fn schedule_animation_frame(
        &mut self,
        job: &mut AnimationRenderJob,
        camera: &mut CameraController,
    ) {
        let index = job.next_frame;
        let dt = if index == 0 {
            0.0
        } else {
            1.0 / job.settings.fps as f32
        };
        let frame = anim_step(
            &self.animation_objects,
            self.animation_elapsed as f64,
            f64::from(dt),
        );
        self.apply_animation_frame(frame, dt, camera);
        let mut frame_camera = camera.clone();
        frame_camera.perspective = job.settings.perspective;
        let qsys_camera = frame_camera.to_qsys_camera("__animation_render");
        let labels = animation_frame_labels(&self.state, &job.settings, &qsys_camera);
        let snapshot = self.snapshot.clone();
        let background = self.background.rgba_f32();
        let receiver = spawn_animation_frame_task(AnimationFrameTask {
            settings: job.settings.clone(),
            index,
            total_frames: job.total_frames,
            snapshot,
            camera: qsys_camera,
            labels,
            background,
            progress: Arc::clone(&job.progress),
            cancel: Arc::clone(&job.cancel),
            denoise: self.umbreon_denoise,
        });
        job.next_frame += 1;
        job.frame_receivers.push(receiver);
    }

    fn finish_animation_render(
        &mut self,
        job: AnimationRenderJob,
        camera: &mut CameraController,
        status: &str,
    ) {
        for (object_uid, renderer_uid, visible) in job.restore.renderer_visibility {
            let _ = self
                .state
                .set_renderer_visible(object_uid, renderer_uid, visible);
        }
        for (object_uid, frame) in job.restore.trajectory_frames {
            let _ = self.state.apply_trajectory_frame(object_uid, frame);
        }
        *camera = job.restore.camera;
        self.animation_elapsed = job.restore.elapsed;
        self.animation_playing = false;
        self.rebuild_snapshot();
        self.set_status(status);
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
                            self.open_renderer_dialog(object.uid);
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
                            if matches!(
                                renderer.renderer_type.as_str(),
                                "*namelabel" | "name_label"
                            ) {
                                let mut max_labels = self
                                    .state
                                    .renderer_property(object.uid, renderer.uid, "maxlabel")
                                    .and_then(|value| value.parse::<i32>().ok())
                                    .unwrap_or(50);
                                ui.horizontal(|ui| {
                                    ui.label("Maximum labels");
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut max_labels)
                                                .speed(1)
                                                .range(0..=1_000_000),
                                        )
                                        .changed()
                                    {
                                        let _ = self.state.set_renderer_property(
                                            object.uid,
                                            renderer.uid,
                                            "maxlabel",
                                            max_labels.to_string(),
                                        );
                                    }
                                });
                                for (property, label, default, speed, range) in [
                                    ("dispx", "X displacement", 0.0_f32, 0.5, -100.0..=100.0),
                                    ("dispy", "Y displacement", 0.0, 0.5, -100.0..=100.0),
                                    ("font_size", "Font size", 12.0, 0.5, 1.0..=512.0),
                                ] {
                                    let mut value = self
                                        .state
                                        .renderer_property(object.uid, renderer.uid, property)
                                        .and_then(|value| value.parse::<f32>().ok())
                                        .unwrap_or(default);
                                    ui.horizontal(|ui| {
                                        ui.label(label);
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut value)
                                                    .speed(speed)
                                                    .range(range),
                                            )
                                            .changed()
                                        {
                                            let _ = self.state.set_renderer_property(
                                                object.uid,
                                                renderer.uid,
                                                property,
                                                format!("{value:.3}"),
                                            );
                                            self.rebuild_snapshot();
                                        }
                                    });
                                }
                                for (property, label, default) in [
                                    ("color", "Color", "#ffff00"),
                                    ("font_name", "Font family", "sans-serif"),
                                    ("font_style", "Font style", "normal"),
                                    ("font_weight", "Font weight", "normal"),
                                ] {
                                    let mut value = self
                                        .state
                                        .renderer_property(object.uid, renderer.uid, property)
                                        .unwrap_or_else(|| default.to_string());
                                    ui.horizontal(|ui| {
                                        ui.label(label);
                                        if ui.text_edit_singleline(&mut value).changed() {
                                            let _ = self.state.set_renderer_property(
                                                object.uid,
                                                renderer.uid,
                                                property,
                                                value,
                                            );
                                            self.rebuild_snapshot();
                                        }
                                    });
                                }
                            } else if matches!(
                                renderer.renderer_type.as_str(),
                                "*selection" | "selection"
                            ) {
                                let mut mode = self
                                    .state
                                    .renderer_property(object.uid, renderer.uid, "mode")
                                    .unwrap_or_else(|| "stick".to_string());
                                let previous_mode = mode.clone();
                                egui::ComboBox::from_id_salt(("selection-mode", renderer.uid))
                                    .selected_text(&mode)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut mode,
                                            "stick".to_string(),
                                            "Stick",
                                        );
                                        ui.selectable_value(
                                            &mut mode,
                                            "point".to_string(),
                                            "Point",
                                        );
                                    });
                                if mode != previous_mode {
                                    let _ = self.state.set_renderer_property(
                                        object.uid,
                                        renderer.uid,
                                        "mode",
                                        mode,
                                    );
                                    self.rebuild_snapshot();
                                }
                                for (property, label, default) in [
                                    ("linew", "Line width", 3.0_f32),
                                    ("dispx", "X displacement", 3.0),
                                    ("dispy", "Y displacement", 3.0),
                                ] {
                                    let mut value = self
                                        .state
                                        .renderer_property(object.uid, renderer.uid, property)
                                        .and_then(|value| value.parse::<f32>().ok())
                                        .unwrap_or(default);
                                    ui.horizontal(|ui| {
                                        ui.label(label);
                                        if ui
                                            .add(egui::DragValue::new(&mut value).speed(0.2))
                                            .changed()
                                        {
                                            let _ = self.state.set_renderer_property(
                                                object.uid,
                                                renderer.uid,
                                                property,
                                                format!("{value:.3}"),
                                            );
                                            self.rebuild_snapshot();
                                        }
                                    });
                                }
                                let mut color = self
                                    .state
                                    .renderer_property(object.uid, renderer.uid, "color")
                                    .unwrap_or_else(|| "rgba(0,255,255,0.5)".to_string());
                                ui.horizontal(|ui| {
                                    ui.label("Color");
                                    if ui.text_edit_singleline(&mut color).changed() {
                                        let _ = self.state.set_renderer_property(
                                            object.uid,
                                            renderer.uid,
                                            "color",
                                            color,
                                        );
                                        self.rebuild_snapshot();
                                    }
                                });
                            } else if renderer.renderer_type == "disorder" {
                                ui.label("Disorder");
                                let mut target = self
                                    .state
                                    .renderer_property(object.uid, renderer.uid, "target")
                                    .unwrap_or_default();
                                let previous_target = target.clone();
                                egui::ComboBox::from_id_salt(("disorder-target", renderer.uid))
                                    .selected_text(if target.is_empty() {
                                        "(none)"
                                    } else {
                                        &target
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut target, String::new(), "(none)");
                                        for candidate in self
                                            .state
                                            .renderer_summaries(object.uid)
                                            .into_iter()
                                            .filter(|candidate| {
                                                matches!(
                                                    candidate.renderer_type.as_str(),
                                                    "tube" | "ribbon" | "cartoon" | "nucl"
                                                )
                                            })
                                        {
                                            ui.selectable_value(
                                                &mut target,
                                                candidate.name.clone(),
                                                candidate.name,
                                            );
                                        }
                                    });
                                if target != previous_target {
                                    let result = if target.is_empty() {
                                        self.state.reset_renderer_property(
                                            object.uid,
                                            renderer.uid,
                                            "target",
                                        )
                                    } else {
                                        self.state.set_renderer_property(
                                            object.uid,
                                            renderer.uid,
                                            "target",
                                            target,
                                        )
                                    };
                                    if result.is_ok() {
                                        self.rebuild_snapshot();
                                    }
                                }
                                let mut detail = self
                                    .state
                                    .renderer_property(object.uid, renderer.uid, "detail")
                                    .and_then(|value| value.parse::<i32>().ok())
                                    .unwrap_or(5);
                                ui.horizontal(|ui| {
                                    ui.label("Detail");
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut detail)
                                                .speed(1)
                                                .range(2..=20),
                                        )
                                        .changed()
                                    {
                                        let result = if detail == 5 {
                                            self.state.reset_renderer_property(
                                                object.uid,
                                                renderer.uid,
                                                "detail",
                                            )
                                        } else {
                                            self.state.set_renderer_property(
                                                object.uid,
                                                renderer.uid,
                                                "detail",
                                                detail.to_string(),
                                            )
                                        };
                                        if result.is_ok() {
                                            self.rebuild_snapshot();
                                        }
                                    }
                                });
                                for (property, label, default, minimum, maximum) in [
                                    ("width", "Dot size", 0.3_f32, 0.0, 3.0),
                                    ("dotsep", "Dot separation", 1.0, 0.0, 3.0),
                                    ("loopsize", "Loop size", 2.0, 0.0, 10.0),
                                    ("loopsize2", "Loop size 2", -1.0, -1.0, 10.0),
                                ] {
                                    let mut value = self
                                        .state
                                        .renderer_property(object.uid, renderer.uid, property)
                                        .and_then(|value| value.parse::<f32>().ok())
                                        .unwrap_or(default);
                                    ui.horizontal(|ui| {
                                        ui.label(label);
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut value)
                                                    .speed(0.1)
                                                    .range(minimum..=maximum),
                                            )
                                            .changed()
                                        {
                                            let result = if (value - default).abs() < f32::EPSILON {
                                                self.state.reset_renderer_property(
                                                    object.uid,
                                                    renderer.uid,
                                                    property,
                                                )
                                            } else {
                                                self.state.set_renderer_property(
                                                    object.uid,
                                                    renderer.uid,
                                                    property,
                                                    format!("{value:.3}"),
                                                )
                                            };
                                            if result.is_ok() {
                                                self.rebuild_snapshot();
                                            }
                                        }
                                    });
                                }
                                let mut color = self
                                    .state
                                    .renderer_property(object.uid, renderer.uid, "defaultcolor")
                                    .unwrap_or_else(|| "#ffffff".to_string());
                                ui.horizontal(|ui| {
                                    ui.label("Color");
                                    if ui.text_edit_singleline(&mut color).changed()
                                        && self
                                            .state
                                            .set_renderer_property(
                                                object.uid,
                                                renderer.uid,
                                                "defaultcolor",
                                                color,
                                            )
                                            .is_ok()
                                    {
                                        self.rebuild_snapshot();
                                    }
                                });
                            } else if renderer.renderer_type == "cpk" {
                                ui.label("Atom radii");
                                for (property, label, default) in [
                                    ("vdwr_C", "Carbon", 1.7_f32),
                                    ("vdwr_N", "Nitrogen", 1.55),
                                    ("vdwr_O", "Oxygen", 1.52),
                                    ("vdwr_S", "Sulfur", 1.8),
                                    ("vdwr_P", "Phosphorus", 1.8),
                                    ("vdwr_H", "Hydrogen", 1.2),
                                    ("vdwr_X", "Others", 1.7),
                                ] {
                                    let mut value = self
                                        .state
                                        .renderer_property(object.uid, renderer.uid, property)
                                        .and_then(|value| value.parse::<f32>().ok())
                                        .unwrap_or(default);
                                    ui.horizontal(|ui| {
                                        ui.label(label);
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut value)
                                                    .speed(0.01)
                                                    .range(0.0..=3.0),
                                            )
                                            .changed()
                                        {
                                            let result = if (value - default).abs() < f32::EPSILON {
                                                self.state.reset_renderer_property(
                                                    object.uid,
                                                    renderer.uid,
                                                    property,
                                                )
                                            } else {
                                                self.state.set_renderer_property(
                                                    object.uid,
                                                    renderer.uid,
                                                    property,
                                                    format!("{value:.3}"),
                                                )
                                            };
                                            if result.is_ok() {
                                                self.rebuild_snapshot();
                                            }
                                        }
                                    });
                                }
                                let mut detail = self
                                    .state
                                    .renderer_property(object.uid, renderer.uid, "detail")
                                    .and_then(|value| value.parse::<i32>().ok())
                                    .unwrap_or(10);
                                ui.horizontal(|ui| {
                                    ui.label("Detail");
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut detail)
                                                .speed(1)
                                                .range(2..=20),
                                        )
                                        .changed()
                                    {
                                        let result = if detail == 10 {
                                            self.state.reset_renderer_property(
                                                object.uid,
                                                renderer.uid,
                                                "detail",
                                            )
                                        } else {
                                            self.state.set_renderer_property(
                                                object.uid,
                                                renderer.uid,
                                                "detail",
                                                detail.to_string(),
                                            )
                                        };
                                        if result.is_ok() {
                                            self.rebuild_snapshot();
                                        }
                                    }
                                });
                                let mut render_mode = self
                                    .state
                                    .renderer_property(object.uid, renderer.uid, "glrender_mode")
                                    .unwrap_or_else(|| "default".to_string());
                                let previous_mode = render_mode.clone();
                                egui::ComboBox::from_id_salt(("cpk-render-mode", renderer.uid))
                                    .selected_text(&render_mode)
                                    .show_ui(ui, |ui| {
                                        for (value, caption) in [
                                            ("default", "Default"),
                                            ("shader", "Shader"),
                                            ("vbo", "VBO"),
                                            ("glu", "GLU"),
                                        ] {
                                            ui.selectable_value(
                                                &mut render_mode,
                                                value.to_string(),
                                                caption,
                                            );
                                        }
                                    });
                                if render_mode != previous_mode {
                                    let result = if render_mode == "default" {
                                        self.state.reset_renderer_property(
                                            object.uid,
                                            renderer.uid,
                                            "glrender_mode",
                                        )
                                    } else {
                                        self.state.set_renderer_property(
                                            object.uid,
                                            renderer.uid,
                                            "glrender_mode",
                                            render_mode,
                                        )
                                    };
                                    if result.is_ok() {
                                        self.rebuild_snapshot();
                                    }
                                }
                            } else {
                                // Editable geometry overrides (CueMol2 renderer
                                // property dialog). The inline attr wins over style.
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
                            }
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
                    let path = PathBuf::from(self.save_path.trim());
                    self.save_current_scene_to_path(camera, &path);
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
                });
                ui.separator();
                match self.bottom_tab {
                    BottomTab::Output => self.output_tab(ui),
                    BottomTab::Sequence => self.sequence_tab(ui),
                }
            });
    }

    fn output_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Clear").clicked() {
                self.clear_log();
            }
            if ui.button("Save As").clicked() {
                self.choose_save_log();
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
        let mut clicked: Option<(cuemol_core::Uid, String)> = None;
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
                            clicked = Some((chain.object_uid, residue.selection.clone()));
                        }
                    }
                });
                ui.separator();
            }
        });

        if let Some((object_uid, selection)) = clicked {
            self.apply_molecule_selection(object_uid, selection, "Selected sequence residue");
        }
    }

    fn animation_render_tool_ui(&mut self, ui: &mut egui::Ui, camera: &CameraController) {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.animation_render_settings.tab,
                AnimationRenderTab::Main,
                "Main options",
            );
            ui.selectable_value(
                &mut self.animation_render_settings.tab,
                AnimationRenderTab::Render,
                "Render options",
            );
            ui.selectable_value(
                &mut self.animation_render_settings.tab,
                AnimationRenderTab::Movie,
                "Movie options",
            );
            ui.selectable_value(
                &mut self.animation_render_settings.tab,
                AnimationRenderTab::Preview,
                "Preview",
            );
        });
        ui.separator();

        let running = self.animation_render_job.is_some();
        match self.animation_render_settings.tab {
            AnimationRenderTab::Main => {
                ui.add_enabled_ui(!running, |ui| {
                    self.animation_render_main_options_ui(ui);
                });
                self.animation_render_progress_ui(ui);
            }
            AnimationRenderTab::Render => {
                ui.add_enabled_ui(!running, |ui| {
                    self.animation_render_backend_options_ui(ui);
                });
            }
            AnimationRenderTab::Movie => {
                ui.add_enabled_ui(!running, |ui| {
                    self.animation_render_movie_options_ui(ui);
                });
            }
            AnimationRenderTab::Preview => self.animation_render_preview_ui(ui),
        }

        ui.separator();
        let mut start = false;
        let mut stop = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!running, egui::Button::new("Start"))
                .clicked()
            {
                start = true;
            }
            if ui.add_enabled(running, egui::Button::new("Stop")).clicked() {
                stop = true;
            }
        });
        if start {
            self.start_animation_render(camera);
        }
        if stop {
            if let Some(job) = &self.animation_render_job {
                job.cancel.store(true, Ordering::Relaxed);
                self.set_status("Cancelling animation render");
            }
        }
    }

    fn animation_render_main_options_ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("animation-render-main-grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Output dir");
                ui.text_edit_singleline(&mut self.animation_render_settings.output_dir);
                ui.end_row();
                ui.label("Output base name");
                ui.text_edit_singleline(&mut self.animation_render_settings.base_name);
                ui.end_row();
                ui.label("Width");
                ui.add(
                    egui::DragValue::new(&mut self.animation_render_settings.width)
                        .range(100..=16_384)
                        .suffix(" px"),
                );
                ui.end_row();
                ui.label("Height");
                ui.add(
                    egui::DragValue::new(&mut self.animation_render_settings.height)
                        .range(100..=16_384)
                        .suffix(" px"),
                );
                ui.end_row();
            });
        ui.horizontal(|ui| {
            ui.label("Preset size");
            for (label, width, height) in [
                ("VGA", 640, 480),
                ("SVGA", 800, 600),
                ("XGA", 1024, 768),
                ("HD720", 1280, 720),
                ("HD1080", 1920, 1080),
            ] {
                if ui.small_button(label).clicked() {
                    self.animation_render_settings.width = width;
                    self.animation_render_settings.height = height;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Projection");
            ui.radio_value(
                &mut self.animation_render_settings.perspective,
                true,
                "Perspective",
            );
            ui.radio_value(
                &mut self.animation_render_settings.perspective,
                false,
                "Orthographic",
            );
            ui.label("NCPU");
            ui.add(
                egui::DragValue::new(&mut self.animation_render_settings.concurrency).range(1..=32),
            );
        });
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut self.animation_render_settings.duplicate_last_frame,
                "Loop",
            );
            ui.label("Frame rate");
            ui.add(
                egui::DragValue::new(&mut self.animation_render_settings.fps)
                    .range(1..=240)
                    .suffix(" fps"),
            );
            egui::ComboBox::from_label("Backend")
                .selected_text(&self.animation_render_settings.backend)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.animation_render_settings.backend,
                        "POV-Ray".to_string(),
                        "POV-Ray",
                    );
                    ui.selectable_value(
                        &mut self.animation_render_settings.backend,
                        "Umbreon".to_string(),
                        "Umbreon",
                    );
                });
        });
    }

    fn animation_render_backend_options_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(format!(
            "Backend: {}",
            self.animation_render_settings.backend
        ));
        if self.animation_render_settings.backend == "POV-Ray" {
            egui::Grid::new("animation-render-pov-grid")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Povray exe file");
                    ui.text_edit_singleline(&mut self.animation_render_settings.povray_executable);
                    ui.end_row();
                    ui.label("Povray inc dir");
                    ui.text_edit_singleline(&mut self.animation_render_settings.povray_include_dir);
                    ui.end_row();
                });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.animation_render_settings.pov_clip_plane,
                    "Clipping plane",
                );
                ui.checkbox(
                    &mut self.animation_render_settings.pov_post_blend,
                    "Post-render alpha blending",
                );
                ui.checkbox(&mut self.animation_render_settings.pov_shadow, "Shadow");
                ui.checkbox(
                    &mut self.animation_render_settings.pov_edge_lines,
                    "Edge lines",
                );
                ui.checkbox(&mut self.animation_render_settings.pov_labels, "Labels");
            });
            let radiosity_label = POV_RADIOSITY_MODES
                .iter()
                .find(|(value, _)| *value == self.animation_render_settings.pov_radiosity)
                .map_or("Disable", |(_, label)| *label);
            ui.horizontal(|ui| {
                ui.label("Radiosity:");
                egui::ComboBox::from_id_salt("animation-render-radiosity")
                    .selected_text(radiosity_label)
                    .show_ui(ui, |ui| {
                        for (value, label) in POV_RADIOSITY_MODES {
                            ui.selectable_value(
                                &mut self.animation_render_settings.pov_radiosity,
                                *value,
                                *label,
                            );
                        }
                    });
            });
            ui.weak("POV-Ray frames use quality 11, PNG alpha, and 3-level antialiasing.");
        } else {
            egui::ComboBox::from_label("Denoise")
                .selected_text(self.umbreon_denoise.label())
                .show_ui(ui, |ui| {
                    for mode in UmbreonDenoiseMode::ALL {
                        ui.selectable_value(&mut self.umbreon_denoise, mode, mode.label());
                    }
                });
            ui.label("Umbreon frames are rendered in-process with cooperative cancellation.");
        }
    }

    fn animation_render_movie_options_ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.animation_render_settings.make_movie, "Make movie");
        ui.add_enabled_ui(self.animation_render_settings.make_movie, |ui| {
            ui.label("FFmpeg executable");
            ui.text_edit_singleline(&mut self.animation_render_settings.ffmpeg_executable);
            egui::ComboBox::from_label("Output format")
                .selected_text(animation_movie_format_label(
                    &self.animation_render_settings.movie_format,
                ))
                .show_ui(ui, |ui| {
                    for (value, label) in ANIMATION_MOVIE_FORMATS {
                        ui.selectable_value(
                            &mut self.animation_render_settings.movie_format,
                            (*value).to_string(),
                            *label,
                        );
                    }
                });
            ui.horizontal(|ui| {
                ui.label("Bitrate");
                ui.add(
                    egui::DragValue::new(&mut self.animation_render_settings.bitrate_kbps)
                        .range(1..=200_000)
                        .suffix(" kbps"),
                );
            });
        });
    }

    fn animation_render_progress_ui(&self, ui: &mut egui::Ui) {
        let (progress, text) = self.animation_render_job.as_ref().map_or_else(
            || (0.0, "Idle".to_string()),
            |job| {
                let progress = job.progress.load(Ordering::Relaxed) as f32 / 1_000_000.0_f32;
                let phase = if job.encoding_started {
                    "Encoding movie".to_string()
                } else {
                    format!(
                        "Rendering frame {}/{}",
                        job.completed_frames.min(job.total_frames),
                        job.total_frames
                    )
                };
                (progress, phase)
            },
        );
        ui.add(egui::ProgressBar::new(progress.clamp(0.0, 1.0)).text(text));
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for line in &self.animation_render_log {
                    ui.monospace(line);
                }
            });
    }

    fn animation_render_preview_ui(&mut self, ui: &mut egui::Ui) {
        if self.animation_render_total_frames == 0 {
            ui.weak("No rendered frames are available.");
            return;
        }

        let max_frame = self.animation_render_total_frames.saturating_sub(1);
        self.animation_preview_frame = self.animation_preview_frame.min(max_frame);
        ui.horizontal(|ui| {
            ui.label("Frame:");
            if ui
                .add(
                    egui::Slider::new(&mut self.animation_preview_frame, 0..=max_frame)
                        .show_value(false),
                )
                .changed()
            {
                self.animation_preview_texture = None;
            }
            ui.monospace(format!(
                "{} / {}",
                self.animation_preview_frame,
                self.animation_render_total_frames.saturating_sub(1)
            ));
        });

        let path = animation_frame_path(
            &self.animation_render_settings,
            self.animation_preview_frame,
            "png",
        );
        if let Err(error) = self.load_animation_preview_texture(ui.ctx(), &path) {
            if path.is_file() {
                ui.colored_label(egui::Color32::DARK_RED, error);
            } else {
                ui.weak(format!("Frame is not available yet: {}", path.display()));
            }
            return;
        }

        let Some(preview) = &self.animation_preview_texture else {
            return;
        };
        let max_size = egui::vec2(ui.available_width(), 420.0);
        let scale = (max_size.x / preview.size.x)
            .min(max_size.y / preview.size.y)
            .min(1.0);
        let display_size = preview.size * scale.max(0.01);
        ui.vertical_centered(|ui| {
            ui.add(egui::Image::new((preview.texture.id(), display_size)));
            ui.monospace(preview.path.display().to_string());
        });
    }

    fn load_animation_preview_texture(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
    ) -> std::result::Result<(), String> {
        if self
            .animation_preview_texture
            .as_ref()
            .is_some_and(|preview| preview.path == path)
        {
            return Ok(());
        }
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        let frame = image::load_from_memory(&bytes)
            .map_err(|error| format!("Failed to decode animation frame: {error}"))?
            .to_rgba8();
        let size = [frame.width() as usize, frame.height() as usize];
        let image = egui::ColorImage::from_rgba_unmultiplied(size, frame.as_raw());
        let texture = ctx.load_texture(
            format!("animation-preview:{}", path.display()),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.animation_preview_texture = Some(AnimationPreviewTexture {
            path: path.to_path_buf(),
            texture,
            size: egui::vec2(size[0] as f32, size[1] as f32),
        });
        Ok(())
    }

    fn render_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Backend")
                .selected_text(&self.render_backend)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.render_backend, "POV-Ray".to_string(), "POV-Ray");
                    ui.selectable_value(&mut self.render_backend, "Umbreon".to_string(), "Umbreon");
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
                self.open_options_dialog();
            }
            if ui.button("Render").clicked() {
                self.export_current_scene();
            }
            if ui.button("Cancel").clicked() {
                self.set_status("No render job is running");
            }
        });
        ui.horizontal(|ui| {
            ui.label("Output");
            ui.text_edit_singleline(&mut self.export_path);
            if self.render_backend == "Umbreon" {
                egui::ComboBox::from_label("Denoise")
                    .selected_text(self.umbreon_denoise.label())
                    .show_ui(ui, |ui| {
                        for mode in UmbreonDenoiseMode::ALL {
                            ui.selectable_value(&mut self.umbreon_denoise, mode, mode.label());
                        }
                    });
            }
        });
    }

    fn choose_open_file(&mut self, camera: &mut CameraController) {
        if let Some(path) = native_file_dialog(NativeFileDialog::OpenObject, &self.open_path) {
            self.open_path = path.to_string_lossy().into_owned();
            self.request_open_current_path(camera);
        }
    }

    fn choose_open_scene(&mut self, camera: &mut CameraController) {
        if let Some(path) = native_file_dialog(NativeFileDialog::OpenScene, &self.open_path) {
            self.open_path = path.to_string_lossy().into_owned();
            self.open_current_path(camera);
        }
    }

    fn choose_save_selected_object(&mut self) {
        if self.target_object_uid().is_none() {
            self.set_status("No object selected");
            return;
        }
        if let Some(path) = native_file_dialog(NativeFileDialog::SaveObject, &self.object_save_path)
        {
            self.object_save_path = path.to_string_lossy().into_owned();
            self.save_selected_object();
        }
    }

    fn choose_save_scene(&mut self, camera: &CameraController) {
        if let Some(path) = native_file_dialog(NativeFileDialog::SaveScene, &self.save_path) {
            self.save_path = path.to_string_lossy().into_owned();
            self.save_current_scene_to_path(camera, &path);
        }
    }

    fn choose_save_current_view(&mut self, camera: &CameraController) {
        if let Some(path) = native_file_dialog(NativeFileDialog::SaveView, &self.view_save_path) {
            self.view_save_path = path.to_string_lossy().into_owned();
            self.save_current_view(camera);
        }
    }

    fn choose_export_scene(&mut self) {
        if let Some(path) = native_file_dialog(NativeFileDialog::ExportScene, &self.export_path) {
            self.export_path = path.to_string_lossy().into_owned();
            self.export_current_scene();
        }
    }

    fn choose_merge_molecule(&mut self, camera: &mut CameraController) {
        if let Some(path) = native_file_dialog(NativeFileDialog::MergeObject, &self.open_path) {
            self.open_path = path.to_string_lossy().into_owned();
            self.merge_current_path(camera);
        }
    }

    fn choose_execute_script(&mut self) {
        if let Some(path) = native_file_dialog(NativeFileDialog::OpenScript, &self.script_path) {
            self.script_path = path.to_string_lossy().into_owned();
            self.load_script_into_log();
        }
    }

    fn choose_save_log(&mut self) {
        if let Some(path) = native_file_dialog(NativeFileDialog::SaveLog, &self.log_save_path) {
            self.log_save_path = path.to_string_lossy().into_owned();
            self.save_log();
        }
    }

    fn keyboard_shortcuts(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        if ctx.wants_keyboard_input() {
            return;
        }
        for (id, direction) in [("view-trans-plusx", 1.0_f32), ("view-trans-minusx", -1.0)] {
            let Some(shortcut) = self.configured_view_shortcut(id) else {
                continue;
            };
            if ctx.input_mut(|input| input.consume_shortcut(&shortcut)) {
                camera.pan_by(direction * 24.0, 0.0);
                self.set_status(if direction > 0.0 {
                    "Translated view +X"
                } else {
                    "Translated view -X"
                });
                return;
            }
        }
        let command = MainShortcutCommand::ALL
            .into_iter()
            .find(|command| ctx.input_mut(|input| input.consume_shortcut(&command.shortcut())));
        match command {
            Some(MainShortcutCommand::OpenScene) => self.choose_open_scene(camera),
            Some(MainShortcutCommand::NewTab) => self.new_scene(camera),
            Some(MainShortcutCommand::NewWindow) => self.new_window(camera),
            Some(MainShortcutCommand::OpenFile) => self.choose_open_file(camera),
            Some(MainShortcutCommand::ReloadScene) => self.reload_current_path(camera),
            Some(MainShortcutCommand::SaveScene) => self.save_current_scene(camera),
            Some(MainShortcutCommand::Undo) => self.undo(camera),
            Some(MainShortcutCommand::Redo) => self.redo(camera),
            Some(MainShortcutCommand::Config) => self.open_options_dialog(),
            Some(MainShortcutCommand::Performance) => self.perf_measure = !self.perf_measure,
            None => {}
        }
    }

    fn configured_view_shortcut(&self, id: &str) -> Option<egui::KeyboardShortcut> {
        let key = match self
            .state
            .config_value(&format!("shortcut:{id}:key"))
            .as_str()
        {
            "ArrowRight" => egui::Key::ArrowRight,
            "ArrowLeft" => egui::Key::ArrowLeft,
            "ArrowUp" => egui::Key::ArrowUp,
            "ArrowDown" => egui::Key::ArrowDown,
            _ => return None,
        };
        let mut modifiers = egui::Modifiers::NONE;
        modifiers.ctrl = config_bool(&self.state, &format!("shortcut:{id}:ctrl"), true);
        modifiers.alt = config_bool(&self.state, &format!("shortcut:{id}:alt"), false);
        modifiers.shift = config_bool(&self.state, &format!("shortcut:{id}:shift"), false);
        Some(egui::KeyboardShortcut::new(modifiers, key))
    }

    fn request_open_current_path(&mut self, camera: &mut CameraController) {
        let open_path = self.open_path.trim().to_string();
        if open_path.is_empty() {
            self.set_status("Open path is empty");
            return;
        }
        if is_network_url(&open_path) {
            self.open_current_path(camera);
            return;
        }
        let path = PathBuf::from(&open_path);
        if is_view_file(&path) {
            self.open_current_path(camera);
            return;
        }
        match self.state.path_supports_molecule_renderer_setup(&path) {
            Ok(true) => {
                let object_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("object")
                    .to_string();
                self.open_renderer_setup(&object_name, NativeRendererSetupAction::OpenPath(path));
            }
            Ok(false) => self.open_current_path(camera),
            Err(error) => {
                self.set_error(error);
            }
        }
    }

    fn open_current_path(&mut self, camera: &mut CameraController) {
        let open_path = self.open_path.trim().to_string();
        if open_path.is_empty() {
            self.set_status("Open path is empty");
            return;
        }
        let path = if is_network_url(&open_path) {
            self.start_url_download(open_path);
            return;
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
                self.modified_scenes.remove(&scene_uid);
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
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
                self.surface_mesh = None;
                self.show_surface = false;
                self.sync_active_scene_properties();
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
                self.modified_scenes.insert(scene_uid);
                self.selected_object = Some(object_uid);
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
                self.workspace_aux_selection = None;
                self.last_loaded_path = Some(path);
                self.add_recent_path(self.last_loaded_path.clone().unwrap());
                self.set_status(format!(
                    "Opened object {} in scene {}",
                    object_uid.get(),
                    scene_uid.get()
                ));
                self.sync_active_scene_properties();
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
            }
            Ok(GuiLoadedPath::Map { scene_uid, map_uid }) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = Some(WorkspaceAuxSelection::DensityMap {
                    scene_uid,
                    map_uid,
                    renderer_uid: None,
                });
                self.last_loaded_path = Some(path);
                self.add_recent_path(self.last_loaded_path.clone().unwrap());
                self.set_status(format!(
                    "Opened density map {} in scene {}",
                    map_uid.get(),
                    scene_uid.get()
                ));
                self.sync_active_scene_properties();
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
            }
            Err(error) => self.set_error(error),
        }
    }

    fn open_path_with_renderer_setup(
        &mut self,
        path: PathBuf,
        reader_options: Option<&crate::GuiMoleculeReaderOptions>,
        setup: crate::GuiRendererSetup,
        camera: &mut CameraController,
    ) {
        let recenter = setup.recenter;
        let undo = self.capture_undo();
        match self
            .state
            .open_path_with_reader_and_renderer_setup(&path, reader_options, &setup)
        {
            Ok(GuiLoadedPath::Object {
                scene_uid,
                object_uid,
            }) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.selected_object = Some(object_uid);
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
                self.last_loaded_path = Some(path.clone());
                self.add_recent_path(path.clone());
                self.sync_active_scene_properties();
                self.rebuild_snapshot();
                if recenter {
                    camera.fit_snapshot(&self.snapshot);
                }
                self.set_status(format!("File: [{}] is loaded.", path.display()));
            }
            Ok(_) => {
                self.restore_undo_snapshot(undo, camera);
                self.set_status("File did not create a molecule object");
            }
            Err(error) => {
                self.restore_undo_snapshot(undo, camera);
                self.set_error(error);
            }
        }
    }

    fn reload_current_path(&mut self, camera: &mut CameraController) {
        let Some(path) = self.state.active_scene_source_path().map(Path::to_path_buf) else {
            self.set_status("Active scene has no source path");
            return;
        };
        if !path.is_file() {
            self.set_status(format!("Scene source does not exist: {}", path.display()));
            return;
        }
        let undo = self.capture_undo();
        match self.state.reload_active_scene_qsc() {
            Ok(scene_uid) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.remove(&scene_uid);
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
                self.selection_text.clear();
                self.last_loaded_path = Some(path.clone());
                self.open_path = path.to_string_lossy().into_owned();
                self.surface_mesh = None;
                self.show_surface = false;
                self.sync_active_scene_properties();
                self.rebuild_snapshot();
                match self.state.active_scene_saved_camera() {
                    Some(saved) => *camera = CameraController::from_qsys_camera(&saved),
                    None => camera.fit_snapshot(&self.snapshot),
                }
                self.set_status(format!(
                    "Reloaded scene {} from {}",
                    scene_uid.get(),
                    path.display()
                ));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn new_scene(&mut self, camera: &mut CameraController) {
        let undo = self.capture_undo();
        self.state.new_scene("Untitled", true);
        self.push_undo_snapshot(undo);
        self.selected_object = None;
        self.selected_renderer = None;
        self.workspace_aux_selection = None;
        self.last_loaded_path = None;
        self.sync_active_scene_properties();
        self.set_status("New scene created");
        self.rebuild_snapshot();
        camera.fit_snapshot(&self.snapshot);
    }

    fn new_window(&mut self, camera: &mut CameraController) {
        self.new_scene(camera);
        self.set_status("New window mapped to a new scene in this native runner");
    }

    fn close_current_scene(&mut self, camera: &mut CameraController) {
        let modified_scenes = self.modified_scenes.clone();
        let undo = self.capture_undo();
        match self.state.close_active_scene() {
            Ok(Some(_)) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes = modified_scenes;
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
                self.last_loaded_path = None;
                self.sync_active_scene_properties();
                self.set_status("Current tab closed");
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
            }
            Ok(None) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes = modified_scenes;
                self.state.new_scene("Untitled", true);
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
                self.last_loaded_path = None;
                self.sync_active_scene_properties();
                self.set_status("Last tab closed; new scene created");
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
            }
            Err(error) => self.set_error(error),
        }
    }

    fn save_current_scene(&mut self, camera: &CameraController) {
        if let Some(path) = self
            .state
            .active_scene_source_path()
            .filter(|path| path.is_file())
            .map(Path::to_path_buf)
        {
            self.save_path = path.to_string_lossy().into_owned();
            self.save_current_scene_to_path(camera, &path);
        } else {
            self.choose_save_scene(camera);
        }
    }

    fn save_current_scene_to_path(&mut self, camera: &CameraController, path: &Path) {
        if let Err(error) = self
            .state
            .set_active_scene_camera(camera.to_qsys_camera("__current"))
        {
            self.set_error(error);
            return;
        }
        match self.state.save_active_scene_qsc(path) {
            Ok(()) => {
                self.save_path = path.to_string_lossy().into_owned();
                self.last_loaded_path = Some(path.to_path_buf());
                self.add_recent_path(path.to_path_buf());
                self.undo_stack.clear();
                self.redo_stack.clear();
                if let Some(scene_uid) = self.state.active_scene() {
                    self.modified_scenes.remove(&scene_uid);
                }
                self.set_status(format!("Saved {}", path.display()));
            }
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

    fn render_dimensions(&self) -> (u32, u32) {
        RENDER_PRESETS
            .iter()
            .find(|(label, _, _)| *label == self.render_preset)
            .map(|(_, width, height)| (*width, *height))
            .unwrap_or((1920, 1080))
    }

    fn export_current_scene(&mut self) {
        let path = self.export_path.trim().to_string();
        if self.render_backend == "Umbreon" {
            if !crate::DesktopAppState::path_has_extension(&path, "png") {
                self.set_status("Umbreon output path must use the .png extension");
                return;
            }
            let Some(scene_uid) = self.state.active_scene() else {
                self.set_status("No active scene");
                return;
            };
            let (width, height) = self.render_dimensions();
            let mut exporter = UmbreonSceneExporter::new();
            exporter.scene_mut().attach(scene_uid);
            exporter.handler_mut().set_path(&path);
            exporter.set_width(width.min(i32::MAX as u32) as i32);
            exporter.set_height(height.min(i32::MAX as u32) as i32);
            exporter.set_denoise_mode(self.umbreon_denoise);
            match self
                .state
                .write_umbreon_exporter(&mut exporter, &self.selection_text)
            {
                Ok(_) => {
                    self.set_status(format!("Rendered {width}x{height} Umbreon image to {path}"))
                }
                Err(error) => self.set_error(error),
            }
            return;
        }

        // Scene export remains extension-driven outside the Umbreon render
        // backend: PNG uses the live wgpu path and other paths use POV-Ray.
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

    fn pdb_fetch_options(&self) -> crate::PdbFetchOptions {
        crate::PdbFetchOptions {
            pdb_id: self.pdb_id.clone(),
            fetch_coordinates: self.pdb_fetch_coordinates,
            coordinate_source: self.pdb_coordinate_source.clone(),
            fetch_two_fofc: self.pdb_fetch_two_fofc,
            fetch_fofc: self.pdb_fetch_fofc,
            map_source: self.pdb_map_source.clone(),
        }
    }

    fn update_pdb_probe(&mut self, ctx: &egui::Context) {
        let options = self.pdb_fetch_options();
        let valid = crate::normalize_pdb_id(&options.pdb_id).is_ok();
        let any_selected =
            options.fetch_coordinates || options.fetch_two_fofc || options.fetch_fofc;
        if !valid || !any_selected {
            self.pdb_probe_options = None;
            self.pdb_probe_deadline = None;
            self.pdb_probe_receiver = None;
            self.pdb_probe_result = None;
            return;
        }
        if self.pdb_probe_options.as_ref() != Some(&options) {
            self.pdb_probe_options = Some(options.clone());
            self.pdb_probe_deadline = Some(Instant::now() + Duration::from_millis(180));
            self.pdb_probe_receiver = None;
            self.pdb_probe_result = None;
            ctx.request_repaint_after(Duration::from_millis(180));
        }

        if self
            .pdb_probe_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.pdb_probe_deadline = None;
            let (sender, receiver) = mpsc::channel();
            let probe_options = options.clone();
            let result_options = options.clone();
            let repaint = ctx.clone();
            match std::thread::Builder::new()
                .name("cuemol-pdb-probe".to_string())
                .spawn(move || {
                    let result = crate::probe_pdb_files(probe_options);
                    let _ = sender.send((result_options, result));
                    repaint.request_repaint();
                }) {
                Ok(_) => self.pdb_probe_receiver = Some(receiver),
                Err(error) => {
                    self.pdb_probe_result = Some(Err(format!(
                        "failed to start PDB availability check: {error}"
                    )));
                }
            }
        } else if let Some(deadline) = self.pdb_probe_deadline {
            ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }

        let result = self
            .pdb_probe_receiver
            .as_ref()
            .map(mpsc::Receiver::try_recv);
        match result {
            Some(Ok((result_options, result))) => {
                self.pdb_probe_receiver = None;
                if self.pdb_probe_options.as_ref() == Some(&result_options) {
                    self.pdb_probe_result = Some(result);
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.pdb_probe_receiver = None;
                self.pdb_probe_result = Some(Err(
                    "PDB availability check stopped unexpectedly".to_string()
                ));
            }
            Some(Err(mpsc::TryRecvError::Empty)) | None => {}
        }
    }

    fn start_pdb_download(
        &mut self,
        ctx: &egui::Context,
        renderer_setup: Option<crate::GuiRendererSetup>,
    ) {
        if self.pdb_download_receiver.is_some() {
            self.set_status("PDB download is already running");
            return;
        }
        let pdb_id = match crate::normalize_pdb_id(&self.pdb_id) {
            Ok(pdb_id) => pdb_id,
            Err(error) => {
                self.set_status(format!("Error: {error}"));
                return;
            }
        };
        let options = crate::PdbFetchOptions {
            pdb_id: pdb_id.clone(),
            fetch_coordinates: self.pdb_fetch_coordinates,
            coordinate_source: self.pdb_coordinate_source.clone(),
            fetch_two_fofc: self.pdb_fetch_two_fofc,
            fetch_fofc: self.pdb_fetch_fofc,
            map_source: self.pdb_map_source.clone(),
        };
        let (sender, receiver) = mpsc::channel();
        let repaint = ctx.clone();
        let result_pdb_id = pdb_id.clone();
        let spawn = std::thread::Builder::new()
            .name(format!("cuemol-pdb-{pdb_id}"))
            .spawn(move || {
                let result = crate::download_pdb_files(options)
                    .map(|files| (result_pdb_id, files, renderer_setup));
                let _ = sender.send(result);
                repaint.request_repaint();
            });
        match spawn {
            Ok(_) => {
                self.pdb_download_receiver = Some(receiver);
                self.set_status(format!("Downloading PDB {pdb_id}…"));
            }
            Err(error) => self.set_status(format!("Error: failed to start PDB download: {error}")),
        }
    }

    fn poll_pdb_download(&mut self, camera: &mut CameraController) {
        let result = self
            .pdb_download_receiver
            .as_ref()
            .map(mpsc::Receiver::try_recv);
        match result {
            Some(Ok(Ok((pdb_id, files, renderer_setup)))) => {
                self.pdb_download_receiver = None;
                let undo = self.capture_undo();
                let input = files
                    .iter()
                    .map(|file| (file.kind, file.path.clone()))
                    .collect::<Vec<_>>();
                let recenter = renderer_setup.as_ref().is_some_and(|setup| setup.recenter);
                match self
                    .state
                    .open_pdb_files(&pdb_id, &input, renderer_setup.as_ref())
                {
                    Ok(summary) => {
                        self.push_undo_snapshot(undo);
                        self.selected_object = summary.object_uid;
                        self.selected_renderer = None;
                        self.workspace_aux_selection = if summary.object_uid.is_none() {
                            summary.map_uids.last().and_then(|map_uid| {
                                self.state.active_scene().map(|scene_uid| {
                                    WorkspaceAuxSelection::DensityMap {
                                        scene_uid,
                                        map_uid: *map_uid,
                                        renderer_uid: None,
                                    }
                                })
                            })
                        } else {
                            None
                        };
                        self.last_loaded_path = files
                            .iter()
                            .find(|file| file.kind == crate::GuiPdbFileKind::Coordinates)
                            .map(|file| file.path.clone());
                        let normalized_id = pdb_id.to_ascii_lowercase();
                        self.pdb_history.retain(|entry| entry != &normalized_id);
                        self.pdb_history.insert(0, normalized_id);
                        self.pdb_history.truncate(crate::MAX_PDB_HISTORY);
                        if let Ok(history) = crate::remember_pdb_id(&pdb_id) {
                            self.pdb_history = history;
                        }
                        self.rebuild_snapshot();
                        if recenter {
                            camera.fit_snapshot(&self.snapshot);
                        }
                        self.set_status(format!("Opened PDB {pdb_id}: {} file(s)", files.len()));
                        self.show_get_pdb = false;
                    }
                    Err(error) => {
                        self.restore_undo_snapshot(undo, camera);
                        for file in files {
                            let _ = fs::remove_file(file.path);
                        }
                        self.set_error(error);
                    }
                }
            }
            Some(Ok(Err(error))) => {
                self.pdb_download_receiver = None;
                self.set_status(format!("Error: {error}"));
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.pdb_download_receiver = None;
                self.set_status("Error: PDB download task stopped");
            }
            Some(Err(mpsc::TryRecvError::Empty)) | None => {}
        }
    }

    fn start_url_download(&mut self, url: String) {
        if self.url_download_receiver.is_some() {
            self.set_status("URL download is already running");
            return;
        }
        let status_url = url.clone();
        let (sender, receiver) = mpsc::channel();
        let spawn = std::thread::Builder::new()
            .name("cuemol-url-download".to_string())
            .spawn(move || {
                let result = download_url_to_cache(&url).map_err(|error| error.to_string());
                let _ = sender.send(result);
            });
        match spawn {
            Ok(_) => {
                self.url_download_receiver = Some(receiver);
                self.set_status(format!("Downloading {status_url}…"));
            }
            Err(error) => self.set_status(format!("Error: failed to start URL download: {error}")),
        }
    }

    fn poll_url_download(&mut self, camera: &mut CameraController) {
        let result = self
            .url_download_receiver
            .as_ref()
            .map(mpsc::Receiver::try_recv);
        match result {
            Some(Ok(Ok(path))) => {
                self.url_download_receiver = None;
                self.open_path = path.to_string_lossy().to_string();
                self.request_open_current_path(camera);
            }
            Some(Ok(Err(error))) => {
                self.url_download_receiver = None;
                self.set_status(format!("Error: {error}"));
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.url_download_receiver = None;
                self.set_status("Error: URL download task stopped");
            }
            Some(Err(mpsc::TryRecvError::Empty)) | None => {}
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
                    if let Some(object_uid) = self.selection_target_object() {
                        self.apply_molecule_selection(object_uid, expr, "Script selection");
                    }
                }
                self.set_status("Script executed");
            }
            Err(error) => self.set_status(format!("Script error: {error}")),
        }
    }

    fn apply_superpose(&mut self, camera: &mut CameraController) {
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
        match self.state.superpose_objects_with_algorithm(
            reference,
            &self.superpose_reference_selection,
            moving,
            &self.superpose_moving_selection,
            &self.superpose_algorithm,
            self.superpose_use_property,
        ) {
            Ok(matrix) => {
                self.push_undo_snapshot(undo);
                self.selected_object = Some(moving);
                self.workspace_aux_selection = None;
                self.rebuild_snapshot();
                if self.superpose_auto_recenter {
                    camera.fit_snapshot(&self.snapshot);
                }
                self.log_lines.push(format!("Superpose matrix: {matrix}"));
                self.set_status("Molecular superposition applied");
            }
            Err(error) => self.set_error(error),
        }
    }

    fn bond_target_object_uid(&self) -> Option<cuemol_core::Uid> {
        self.bond_object
            .filter(|uid| self.state.scene_manager().object(*uid).is_some())
            .or_else(|| self.target_object_uid())
    }

    fn edit_bond(&mut self, make: bool) {
        let Some(object_uid) = self.bond_target_object_uid() else {
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
                    self.bond_aid1.clear();
                    self.bond_aid2.clear();
                    self.bond_picker_target = None;
                    self.set_status("Bond created");
                } else {
                    self.set_status("Bond removed");
                }
            }
            Ok(false) => self.set_status("Bond was unchanged"),
            Err(error) => self.set_error(error),
        }
    }

    fn remove_selected_nonstandard_bonds(&mut self) {
        let Some(object_uid) = self.bond_target_object_uid() else {
            self.set_status("No object selected");
            return;
        };
        if self.selected_nonstandard_bonds.is_empty() {
            self.set_status("Select at least one non-standard bond");
            return;
        }
        let undo = self.capture_undo();
        let pairs = self
            .selected_nonstandard_bonds
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let mut removed = 0;
        for (aid1, aid2) in pairs {
            match self.state.remove_bond_in_object(object_uid, aid1, aid2) {
                Ok(true) => removed += 1,
                Ok(false) => {}
                Err(error) => {
                    self.set_error(error);
                    return;
                }
            }
        }
        self.selected_nonstandard_bonds.clear();
        if removed > 0 {
            self.push_undo_snapshot(undo);
            self.rebuild_snapshot();
        }
        self.set_status(format!("Removed {removed} non-standard bond(s)"));
    }

    fn calculate_contacts(&mut self) {
        let Some(object_uid) = self
            .contact_object
            .filter(|uid| self.state.scene_manager().object(*uid).is_some())
            .or_else(|| self.target_object_uid())
        else {
            self.set_status("No object selected");
            return;
        };
        let object2_uid = self
            .contact_use_object2
            .then_some(self.contact_object2)
            .flatten();
        let selection2 = self
            .contact_use_selection2
            .then(|| self.contact_selection2.clone());
        let renderer_name = if self.contact_renderer_name.trim().is_empty() {
            "measure".to_string()
        } else {
            self.contact_renderer_name.trim().to_string()
        };
        self.contact_renderer_name.clone_from(&renderer_name);
        let undo = self.capture_undo();
        match self.state.define_atom_contacts(
            object_uid,
            &self.contact_selection1,
            object2_uid,
            selection2.as_deref(),
            self.contact_r_min,
            self.contact_r_max,
            self.contact_hbond,
            self.contact_max_count,
            &renderer_name,
        ) {
            Ok(summary) if summary.count > 0 => {
                self.push_undo_snapshot(undo);
                self.selected_object = Some(object_uid);
                self.selected_renderer = Some(summary.renderer_uid);
                self.rebuild_snapshot();
                self.set_status(format!(
                    "Defined {} interaction(s) in {}",
                    summary.count, renderer_name
                ));
            }
            Ok(_) => self.set_status("No interaction was found"),
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
            if self.secondary_gap_fill {
                self.secondary_angle
            } else {
                0.0
            },
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

    fn cut_surface_by_current_plane(&mut self, camera: &CameraController) {
        let Some(surface_uid) = self.cut_surface_target else {
            self.set_status("No surface selected");
            return;
        };
        let qsys_camera = camera.to_qsys_camera("__current");
        let (normal, position) = DesktopAppState::clipping_plane_from_camera(&qsys_camera);
        let undo = self.capture_undo();
        match self.state.cut_loaded_surface_by_plane(
            surface_uid,
            self.cut_surface_density,
            normal,
            position,
            &self.cut_surface_mode,
        ) {
            Ok(summary) => {
                self.push_undo_snapshot(undo);
                self.rebuild_snapshot();
                let selected_uid = summary.section_uid.unwrap_or(summary.target_uid);
                if let Some(scene_uid) = self
                    .state
                    .scene_manager()
                    .scene_uid_for_surface(selected_uid)
                {
                    self.selected_object = None;
                    self.selected_renderer = None;
                    self.workspace_aux_selection = Some(WorkspaceAuxSelection::Surface {
                        scene_uid,
                        surface_uid: selected_uid,
                        renderer_uid: None,
                    });
                }
                self.set_status(if summary.section_uid.is_some() {
                    format!(
                        "Surface cut: {} body / {} section triangles",
                        summary.target_triangle_count, summary.section_triangle_count
                    )
                } else {
                    format!("Surface cut: {} triangles", summary.target_triangle_count)
                });
            }
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
                self.workspace_aux_selection = None;
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
        // so both desktop front ends render it identically.
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
        if self.color_proofing && self.state.apply_color_proof(&mut self.snapshot).is_err() {
            cuemol_render::apply_generic_color_proof(&mut self.snapshot);
        }
        self.snapshot_revision = self.snapshot_revision.wrapping_add(1);
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
                // the shared refit-on-import behavior used by both shells.
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
        let pick_atoms = self
            .state
            .active_pick_atoms(&self.selection_text)
            .unwrap_or_default();
        let selected = pick_atoms
            .iter()
            .filter(|atom| atom.selected)
            .take(MAX_LABELS)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return;
        }
        let painter = ui.painter();
        let color = self.background.foreground(
            egui::Color32::from_rgb(20, 24, 30),
            egui::Color32::from_rgb(235, 240, 248),
        );
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

    /// Draw serialized `*namelabel` renderer entries independently of the
    /// temporary selection-label toggle.
    fn draw_name_labels(&self, ui: &egui::Ui, camera: &CameraController, rect: egui::Rect) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let mvp = camera
            .view_projection(rect.width() / rect.height().max(1.0))
            .to_cols_array_2d();
        let labels = self
            .state
            .active_name_labels(mvp, rect.width(), rect.height(), 1_000);
        let painter = ui.painter();
        for label in labels {
            let rgba = cuemol_gfx::parse_color(&label.color)
                .ok()
                .and_then(|color| color.rgba8())
                .unwrap_or(cuemol_gfx::Rgba8 {
                    r: 255,
                    g: 255,
                    b: 0,
                    a: 255,
                });
            let family = if label.font_name.to_ascii_lowercase().contains("mono") {
                egui::FontFamily::Monospace
            } else {
                egui::FontFamily::Proportional
            };
            painter.text(
                egui::pos2(rect.left() + label.x, rect.top() + label.y),
                egui::Align2::LEFT_BOTTOM,
                label.text,
                egui::FontId::new(label.font_size.max(1.0) as f32, family),
                egui::Color32::from_rgba_unmultiplied(rgba.r, rgba.g, rgba.b, rgba.a),
            );
        }
    }

    fn draw_screen_lines(&self, ui: &egui::Ui, camera: &CameraController, rect: egui::Rect) {
        let painter = ui.painter();
        for bond in self
            .snapshot
            .bonds
            .iter()
            .filter(|bond| bond.screen_width.is_some())
        {
            let Some((from, _)) = camera.project_to_viewport(vec3_from_vector(bond.from), rect)
            else {
                continue;
            };
            let Some((to, _)) = camera.project_to_viewport(vec3_from_vector(bond.to), rect) else {
                continue;
            };
            let offset = egui::vec2(bond.screen_offset[0], bond.screen_offset[1]);
            let from = from + offset;
            let to = to + offset;
            let rgba = bond
                .color
                .as_ref()
                .and_then(cuemol_gfx::ColorSpec::rgba8)
                .unwrap_or(Rgba8 {
                    r: 255,
                    g: 255,
                    b: 0,
                    a: 255,
                });
            let end_rgba = bond
                .end_color
                .as_ref()
                .and_then(cuemol_gfx::ColorSpec::rgba8)
                .unwrap_or(rgba);
            let width = bond.screen_width.unwrap_or(1.0).max(0.1);
            if bond.screen_point {
                painter.rect_filled(
                    egui::Rect::from_center_size(from, egui::vec2(width, width)),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(rgba.r, rgba.g, rgba.b, rgba.a),
                );
                continue;
            }
            let delta = to - from;
            let length = delta.length();
            if length <= f32::EPSILON {
                continue;
            }
            let direction = delta / length;
            let gradient = rgba != end_rgba;
            let paint_span = |start: f32, end: f32| {
                let steps = if gradient {
                    ((end - start) / 4.0).ceil().max(1.0) as usize
                } else {
                    1
                };
                for step in 0..steps {
                    let span_start = start + (end - start) * step as f32 / steps as f32;
                    let span_end = start + (end - start) * (step + 1) as f32 / steps as f32;
                    let t = ((span_start + span_end) * 0.5 / length).clamp(0.0, 1.0);
                    let lerp = |start: u8, end: u8| {
                        (f32::from(start) + (f32::from(end) - f32::from(start)) * t).round() as u8
                    };
                    let color = egui::Color32::from_rgba_unmultiplied(
                        lerp(rgba.r, end_rgba.r),
                        lerp(rgba.g, end_rgba.g),
                        lerp(rgba.b, end_rgba.b),
                        lerp(rgba.a, end_rgba.a),
                    );
                    painter.line_segment(
                        [from + direction * span_start, from + direction * span_end],
                        egui::Stroke::new(width, color),
                    );
                }
            };
            if !bond.screen_stipple {
                paint_span(0.0, length);
                continue;
            }
            let mut start = 0.0_f32;
            while start < length {
                let end = (start + 8.0).min(length);
                paint_span(start, end);
                start += 16.0;
            }
        }
    }

    fn draw_center_mark(&self, ui: &egui::Ui, rect: egui::Rect, camera: &CameraController) {
        if camera.center_mark == CenterMark::None {
            return;
        }
        let center = rect.center();
        let stroke = egui::Stroke::new(
            1.0,
            self.background.foreground(
                egui::Color32::from_rgb(25, 30, 36),
                egui::Color32::from_rgb(220, 226, 235),
            ),
        );
        let painter = ui.painter();
        match camera.center_mark {
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
        self.object_props_dialog(ctx);
        self.renderer_props_dialog(ctx);
        self.scene_props_dialog(ctx);
        self.view_props_dialog(ctx, camera);
        self.camera_visibility_dialog(ctx);
        self.camera_visibility_clear_dialog(ctx);
        self.style_props_dialog(ctx);
        self.renderer_style_create_dialog(ctx);
        self.style_paste_confirmation_dialog(ctx);
        self.chain_dialog(ctx);
        self.residue_dialog(ctx);
        self.renderer_dialog(ctx);
        self.get_pdb_dialog(ctx);
        self.renderer_setup_dialog(ctx, camera);
        self.console_dialog(ctx);
        self.tool_dialog(ctx, camera);
    }

    fn camera_visibility_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.camera_visibility_draft.take() else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Edit visibility flags")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .default_height(360.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        egui::Grid::new("camera_visibility_grid")
                            .num_columns(3)
                            .striped(true)
                            .spacing([12.0, 5.0])
                            .show(ui, |ui| {
                                ui.strong("Inc");
                                ui.strong("Obj/Rend");
                                ui.strong("Vis");
                                ui.end_row();
                                for entry in &mut draft.entries {
                                    let was_included = entry.included;
                                    ui.checkbox(&mut entry.included, "");
                                    if was_included && !entry.included {
                                        entry.visible = false;
                                    }
                                    ui.horizontal(|ui| {
                                        if !entry.is_object {
                                            ui.add_space(18.0);
                                        }
                                        ui.label(&entry.name);
                                        ui.weak(format!("({})", entry.type_name));
                                    });
                                    ui.add_enabled_ui(entry.included, |ui| {
                                        ui.checkbox(&mut entry.visible, "");
                                    });
                                    ui.end_row();
                                }
                            });
                    });
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        accept = true;
                    }
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if accept || apply {
            let undo = self.capture_undo();
            let settings = draft
                .entries
                .iter()
                .filter(|entry| entry.included)
                .map(|entry| (entry.target_uid, entry.visible, entry.is_object))
                .collect::<Vec<_>>();
            match self.state.set_scene_camera_visibility_settings(
                draft.scene_uid,
                &draft.camera_name,
                &settings,
            ) {
                Ok(count) => {
                    self.push_undo_snapshot(undo);
                    self.modified_scenes.insert(draft.scene_uid);
                    self.set_status(format!(
                        "Change visflags of {}: {count} item(s)",
                        draft.camera_name
                    ));
                    if !accept {
                        self.camera_visibility_draft = Some(draft);
                    }
                }
                Err(error) => {
                    self.set_status(format!("Camera visibility error: {error}"));
                    self.camera_visibility_draft = Some(draft);
                }
            }
        } else if !cancel && open {
            self.camera_visibility_draft = Some(draft);
        }
    }

    fn camera_visibility_clear_dialog(&mut self, ctx: &egui::Context) {
        let Some((scene_uid, name)) = self.pending_camera_visibility_clear.clone() else {
            return;
        };
        let mut open = true;
        let mut clear = false;
        let mut cancel = false;
        egui::Window::new("Clear visibility flags?")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Clear visibility flags in {name}?"));
                ui.horizontal(|ui| {
                    if ui.button("Clear").clicked() {
                        clear = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if clear {
            self.pending_camera_visibility_clear = None;
            self.clear_camera_visibility(scene_uid, &name);
        } else if cancel || !open {
            self.pending_camera_visibility_clear = None;
        }
    }

    fn style_paste_confirmation_dialog(&mut self, ctx: &egui::Context) {
        let Some(scene_uid) = self.pending_style_paste_scene else {
            return;
        };
        let name = self
            .workspace_style_clipboard
            .as_ref()
            .map(WorkspaceStyleClipboard::name)
            .unwrap_or_default();
        let mut open = true;
        let mut replace = false;
        let mut cancel = false;
        egui::Window::new("Replace Style?")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Style \"{name}\" already exists. Replace?"));
                ui.horizontal(|ui| {
                    if ui.button("Replace").clicked() {
                        replace = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if replace {
            self.pending_style_paste_scene = None;
            self.paste_workspace_style_clipboard(scene_uid, true);
        } else if cancel || !open {
            self.pending_style_paste_scene = None;
        }
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = self.show_about;
        let mut close = false;
        egui::Window::new("About CueMol2")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.heading("CueMol2");
                ui.label("Molecular visualization system — Rust desktop implementation.");
                ui.label(format!("Menu surfaces: {}", LEGACY_MENU_SURFACES.len()));
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        self.show_about = open && !close;
    }

    fn open_options_dialog(&mut self) {
        self.options_pane = OptionsPane::Misc;
        self.options_draft = Some(NativePreferences::from_state(&self.state));
        self.show_options = true;
    }

    fn options_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_options {
            return;
        }
        let mut draft = self
            .options_draft
            .take()
            .unwrap_or_else(|| NativePreferences::from_state(&self.state));
        let mut open = self.show_options;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Options")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .min_width(520.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.options_pane, OptionsPane::Misc, "Misc");
                    ui.selectable_value(&mut self.options_pane, OptionsPane::Key, "Key");
                    ui.selectable_value(&mut self.options_pane, OptionsPane::Mouse, "Mouse");
                });
                ui.separator();
                match self.options_pane {
                    OptionsPane::Misc => Self::options_misc_ui(ui, &mut draft),
                    OptionsPane::Key => Self::options_key_ui(ui, &mut draft),
                    OptionsPane::Mouse => Self::options_mouse_ui(ui, &mut draft),
                }
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        accept = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if accept {
            match self.apply_preferences(&draft) {
                Ok(()) => {
                    self.show_options = false;
                    self.options_draft = None;
                    self.set_status("Options updated");
                }
                Err(error) => {
                    self.show_options = true;
                    self.options_draft = Some(draft);
                    self.set_status(format!("Options error: {error}"));
                }
            }
        } else if cancel || !open {
            self.show_options = false;
            self.options_draft = None;
        } else {
            self.show_options = true;
            self.options_draft = Some(draft);
        }
    }

    fn options_misc_ui(ui: &mut egui::Ui, draft: &mut NativePreferences) {
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Atom label");
            egui::Grid::new("options_misc_atom_label")
                .num_columns(2)
                .spacing([12.0, 7.0])
                .show(ui, |ui| {
                    ui.label("Font:");
                    ui.text_edit_singleline(&mut draft.atom_label_font);
                    ui.end_row();
                    ui.label("Size:");
                    ui.add(
                        egui::DragValue::new(&mut draft.atom_label_size)
                            .range(4.0..=144.0)
                            .speed(1.0),
                    );
                    ui.end_row();
                    ui.label("Color:");
                    ui.text_edit_singleline(&mut draft.atom_label_color);
                    ui.end_row();
                    ui.label("");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut draft.atom_label_italic, "Italic");
                        ui.checkbox(&mut draft.atom_label_bold, "Bold");
                    });
                    ui.end_row();
                    ui.label("Sample:");
                    let mut sample = egui::RichText::new("ABC abc 123")
                        .size(draft.atom_label_size.clamp(4.0, 72.0) as f32);
                    if draft.atom_label_italic {
                        sample = sample.italics();
                    }
                    if draft.atom_label_bold {
                        sample = sample.strong();
                    }
                    if let Ok(color) = cuemol_gfx::parse_color(&draft.atom_label_color) {
                        if let Some(color) = color.rgba8() {
                            sample = sample.color(egui::Color32::from_rgba_unmultiplied(
                                color.r, color.g, color.b, color.a,
                            ));
                        }
                    }
                    egui::Frame::new()
                        .fill(egui::Color32::WHITE)
                        .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY))
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.label(sample);
                        });
                    ui.end_row();
                });
        });
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Misc settings");
            egui::ComboBox::from_label("UI language")
                .selected_text(match draft.ui_language.as_str() {
                    "ja" => "Japanese (ja)",
                    _ => "English (en-US)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut draft.ui_language,
                        "en-US".to_string(),
                        "English (en-US)",
                    );
                    ui.selectable_value(&mut draft.ui_language, "ja".to_string(), "Japanese (ja)");
                });
            ui.checkbox(&mut draft.use_hidpi, "Enable HiDPI (Retina) display");
        });
    }

    fn options_key_ui(ui: &mut egui::Ui, draft: &mut NativePreferences) {
        ui.label("Key config:");
        egui::Grid::new("options_key_bindings")
            .striped(true)
            .num_columns(5)
            .spacing([12.0, 7.0])
            .show(ui, |ui| {
                for heading in ["Name", "Key", "Ctrl", "Alt", "Shift"] {
                    ui.strong(heading);
                }
                ui.end_row();
                for shortcut in &mut draft.shortcuts {
                    let name = match shortcut.id.as_str() {
                        "view-trans-plusx" => "Translate view +X",
                        "view-trans-minusx" => "Translate view −X",
                        _ => shortcut.id.as_str(),
                    };
                    ui.label(name);
                    egui::ComboBox::from_id_salt(format!("options-key-{}", shortcut.id))
                        .selected_text(&shortcut.key)
                        .show_ui(ui, |ui| {
                            for key in ["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"] {
                                ui.selectable_value(&mut shortcut.key, key.to_string(), key);
                            }
                        });
                    ui.checkbox(&mut shortcut.ctrl, "");
                    ui.checkbox(&mut shortcut.alt, "");
                    ui.checkbox(&mut shortcut.shift, "");
                    ui.end_row();
                }
            });
    }

    fn options_mouse_ui(ui: &mut egui::Ui, draft: &mut NativePreferences) {
        egui::ComboBox::from_label("View operation preset")
            .selected_text(mouse_preset_label(&draft.mouse_preset))
            .show_ui(ui, |ui| {
                for (value, label) in MOUSE_PRESETS {
                    ui.selectable_value(&mut draft.mouse_preset, value.to_string(), *label);
                }
            });
        egui::Grid::new("options_mouse_values")
            .num_columns(2)
            .spacing([12.0, 7.0])
            .show(ui, |ui| {
                ui.label("XY-rot sensitivity:");
                ui.add(
                    egui::DragValue::new(&mut draft.trackball_radius)
                        .range(0.1..=10.0)
                        .speed(0.1),
                );
                ui.end_row();
                ui.label("Pick precision:");
                ui.add(
                    egui::DragValue::new(&mut draft.hit_precision)
                        .range(0.0..=100.0)
                        .speed(1.0),
                );
                ui.end_row();
            });
        ui.checkbox(&mut draft.momentum_scroll, "Momentum scroll");
        ui.checkbox(&mut draft.multitouch_pad, "Enable multi-touch trackpad");
        ui.checkbox(&mut draft.emulate_right_button, "Emulate mouse R button");
    }

    fn apply_preferences(
        &mut self,
        preferences: &NativePreferences,
    ) -> std::result::Result<(), String> {
        validate_native_preferences(preferences)?;
        self.state.set_style_value(
            0,
            "user",
            "DefaultLabel.font_name",
            preferences.atom_label_font.trim(),
        );
        self.state.set_style_value(
            0,
            "user",
            "DefaultLabel.font_size",
            preferences.atom_label_size.to_string(),
        );
        self.state.set_style_value(
            0,
            "user",
            "DefaultLabel.color",
            preferences.atom_label_color.trim(),
        );
        self.state.set_style_value(
            0,
            "user",
            "DefaultLabel.font_style",
            if preferences.atom_label_italic {
                "italic"
            } else {
                "normal"
            },
        );
        self.state.set_style_value(
            0,
            "user",
            "DefaultLabel.font_weight",
            if preferences.atom_label_bold {
                "bold"
            } else {
                "normal"
            },
        );
        {
            let input = self.state.view_input_config_mut();
            input.set_style(preferences.mouse_preset.clone());
            input.set_tbrad(preferences.trackball_radius);
            input.set_hitprec(preferences.hit_precision);
        }
        for (key, value) in [
            (
                "style:DefaultLabel.font_name",
                preferences.atom_label_font.trim().to_string(),
            ),
            (
                "style:DefaultLabel.font_size",
                preferences.atom_label_size.to_string(),
            ),
            (
                "style:DefaultLabel.color",
                preferences.atom_label_color.trim().to_string(),
            ),
            (
                "style:DefaultLabel.font_style",
                if preferences.atom_label_italic {
                    "italic".to_string()
                } else {
                    "normal".to_string()
                },
            ),
            (
                "style:DefaultLabel.font_weight",
                if preferences.atom_label_bold {
                    "bold".to_string()
                } else {
                    "normal".to_string()
                },
            ),
            ("general.useragent.locale", preferences.ui_language.clone()),
            (
                "cuemol2.ui.view.use_hidpi",
                preferences.use_hidpi.to_string(),
            ),
            ("cuemol2.ui.viewinconf", preferences.mouse_preset.clone()),
            ("view_input:tbrad", preferences.trackball_radius.to_string()),
            ("view_input:hitprec", preferences.hit_precision.to_string()),
            (
                "cuemol2.ui.mouse-momentum-scroll",
                preferences.momentum_scroll.to_string(),
            ),
            (
                "cuemol2.ui.mouse-multitouch-pad",
                preferences.multitouch_pad.to_string(),
            ),
            (
                "cuemol2.ui.mouse-emulate-rbutton",
                preferences.emulate_right_button.to_string(),
            ),
        ] {
            self.state.put_config_value(key, value);
        }
        for shortcut in &preferences.shortcuts {
            for (field, value) in [
                ("key", shortcut.key.clone()),
                ("ctrl", shortcut.ctrl.to_string()),
                ("alt", shortcut.alt.to_string()),
                ("shift", shortcut.shift.to_string()),
            ] {
                self.state
                    .put_config_value(&format!("shortcut:{}:{field}", shortcut.id), value);
            }
        }
        let path = PathBuf::from(self.config_path.trim());
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create config directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        self.state
            .save_config_file(&path)
            .map_err(|error| format!("failed to save config {}: {error}", path.display()))
    }

    fn apply_stored_preferences(&mut self) {
        for (style_key, config_key) in [
            ("DefaultLabel.font_name", "style:DefaultLabel.font_name"),
            ("DefaultLabel.font_size", "style:DefaultLabel.font_size"),
            ("DefaultLabel.color", "style:DefaultLabel.color"),
            ("DefaultLabel.font_style", "style:DefaultLabel.font_style"),
            ("DefaultLabel.font_weight", "style:DefaultLabel.font_weight"),
        ] {
            let value = self.state.config_value(config_key);
            if !value.is_empty() {
                self.state.set_style_value(0, "user", style_key, value);
            }
        }
        let style = nonempty_or(
            self.state.config_value("cuemol2.ui.viewinconf"),
            "DefaultViewInConf",
        );
        let tbrad = self
            .state
            .config_value("view_input:tbrad")
            .parse()
            .unwrap_or(0.8);
        let hitprec = self
            .state
            .config_value("view_input:hitprec")
            .parse()
            .unwrap_or(10.0);
        let input = self.state.view_input_config_mut();
        input.set_style(style);
        input.set_tbrad(tbrad);
        input.set_hitprec(hitprec);
    }

    fn open_object_properties(&mut self, object_uid: cuemol_core::Uid) {
        let Some(properties) = NativeObjectProperties::from_state(&self.state, object_uid) else {
            self.set_status(format!("Object {} was not found", object_uid.get()));
            return;
        };
        self.object_properties_draft = Some(properties);
    }

    fn open_density_map_properties(&mut self, map_uid: cuemol_core::Uid) {
        let Some(properties) = NativeObjectProperties::from_density_state(&self.state, map_uid)
        else {
            self.set_status(format!("Density map {} was not found", map_uid.get()));
            return;
        };
        self.object_properties_draft = Some(properties);
    }

    fn open_surface_properties(&mut self, surface_uid: cuemol_core::Uid) {
        let Some(properties) = NativeObjectProperties::from_surface_state(&self.state, surface_uid)
        else {
            self.set_status(format!("Surface {} was not found", surface_uid.get()));
            return;
        };
        self.object_properties_draft = Some(properties);
    }

    fn object_props_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.object_properties_draft.take() else {
            return;
        };
        let mut open = true;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Object properties")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .min_width(420.0)
            .show(ctx, |ui| {
                let _ = ui.selectable_label(true, "Common");
                ui.separator();
                egui::Frame::new()
                    .stroke(egui::Stroke::new(
                        1.0,
                        ui.visuals().widgets.noninteractive.bg_stroke.color,
                    ))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        egui::Grid::new("native_object_common_properties")
                            .num_columns(2)
                            .spacing([14.0, 8.0])
                            .show(ui, |ui| {
                                ui.label("Name:");
                                ui.add_sized(
                                    [ui.available_width().max(240.0), 22.0],
                                    egui::TextEdit::singleline(&mut draft.name),
                                );
                                ui.end_row();

                                ui.label("Selection:");
                                ui.monospace("(not available)");
                                ui.end_row();

                                ui.label("");
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut draft.visible, "Visible");
                                    ui.checkbox(&mut draft.locked, "Locked");
                                });
                                ui.end_row();

                                ui.label("Linked:");
                                ui.add_enabled(
                                    false,
                                    egui::TextEdit::singleline(&mut draft.linked),
                                );
                                ui.end_row();
                            });
                    });
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        accept = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if accept {
            match self.apply_object_properties(&draft) {
                Ok(()) => self.object_properties_draft = None,
                Err(error) => {
                    self.set_status(format!("Object properties error: {error}"));
                    self.object_properties_draft = Some(draft);
                }
            }
        } else if cancel || !open {
            self.object_properties_draft = None;
        } else {
            self.object_properties_draft = Some(draft);
        }
    }

    fn apply_object_properties(
        &mut self,
        draft: &NativeObjectProperties,
    ) -> std::result::Result<(), String> {
        let name = draft.name.trim();
        if name.is_empty() || name.contains(':') {
            return Err("object name must be non-empty and cannot contain ':'".to_string());
        }
        let scene_uid = match draft.target {
            NativePropertyObject::Molecule(object_uid) => self
                .state
                .scene_manager()
                .scene_uid_for_object(object_uid)
                .ok_or_else(|| format!("object {} was not found", object_uid.get()))?,
            NativePropertyObject::DensityMap(map_uid) => self
                .state
                .density_map_summary(map_uid)
                .map(|map| map.scene_uid)
                .ok_or_else(|| format!("density map {} was not found", map_uid.get()))?,
            NativePropertyObject::Surface(surface_uid) => self
                .state
                .surface_summary(surface_uid)
                .map(|surface| surface.scene_uid)
                .ok_or_else(|| format!("surface {} was not found", surface_uid.get()))?,
        };
        let undo = self.capture_undo();
        match draft.target {
            NativePropertyObject::Molecule(object_uid) => {
                self.state
                    .set_object_common_properties(object_uid, name, draft.visible, draft.locked)
                    .map_err(|error| error.to_string())?;
                self.selected_object = Some(object_uid);
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
            }
            NativePropertyObject::DensityMap(map_uid) => {
                self.state
                    .set_density_map_common_properties(map_uid, name, draft.visible, draft.locked)
                    .map_err(|error| error.to_string())?;
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = Some(WorkspaceAuxSelection::DensityMap {
                    scene_uid,
                    map_uid,
                    renderer_uid: None,
                });
            }
            NativePropertyObject::Surface(surface_uid) => {
                self.state
                    .set_surface_common_properties(surface_uid, name, draft.visible, draft.locked)
                    .map_err(|error| error.to_string())?;
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = Some(WorkspaceAuxSelection::Surface {
                    scene_uid,
                    surface_uid,
                    renderer_uid: None,
                });
            }
        }
        self.push_undo_snapshot(undo);
        self.modified_scenes.insert(scene_uid);
        self.rebuild_snapshot();
        self.set_status("Object properties updated");
        Ok(())
    }

    fn open_renderer_properties(
        &mut self,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) {
        let Some(properties) =
            NativeRendererProperties::from_state(&self.state, object_uid, renderer_uid)
        else {
            self.set_status(format!("Renderer {} was not found", renderer_uid.get()));
            return;
        };
        self.renderer_properties_draft = Some(properties);
    }

    fn open_density_renderer_properties(
        &mut self,
        map_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) {
        let Some(properties) =
            NativeRendererProperties::from_density_state(&self.state, map_uid, renderer_uid)
        else {
            self.set_status(format!(
                "Density renderer {} was not found",
                renderer_uid.get()
            ));
            return;
        };
        self.renderer_properties_draft = Some(properties);
    }

    fn open_surface_renderer_properties(
        &mut self,
        surface_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) {
        let Some(properties) =
            NativeRendererProperties::from_surface_state(&self.state, surface_uid, renderer_uid)
        else {
            self.set_status(format!(
                "Surface renderer {} was not found",
                renderer_uid.get()
            ));
            return;
        };
        self.renderer_properties_draft = Some(properties);
    }

    fn renderer_props_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.renderer_properties_draft.take() else {
            return;
        };
        let scene_uid = match draft.owner {
            NativePropertyObject::Molecule(object_uid) => {
                self.state.scene_manager().scene_uid_for_object(object_uid)
            }
            NativePropertyObject::DensityMap(map_uid) => self
                .state
                .density_map_summary(map_uid)
                .map(|map| map.scene_uid),
            NativePropertyObject::Surface(surface_uid) => self
                .state
                .surface_summary(surface_uid)
                .map(|surface| surface.scene_uid),
        };
        let material_names = scene_uid
            .map(|scene_uid| self.state.material_names(scene_uid.get()))
            .unwrap_or_default();
        let style_options = scene_uid
            .map(|scene_uid| {
                self.state
                    .renderer_style_options(scene_uid, &draft.renderer_type)
            })
            .unwrap_or_default();
        let mut open = true;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new(draft.title())
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .min_width(440.0)
            .default_height(500.0)
            .max_height(600.0)
            .show(ctx, |ui| {
                let tabs = draft.tabs();
                draft.selected_tab = draft.selected_tab.min(tabs.len().saturating_sub(1));
                ui.horizontal(|ui| {
                    for (index, tab) in tabs.iter().enumerate() {
                        ui.selectable_value(&mut draft.selected_tab, index, *tab);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(370.0)
                    .show(ui, |ui| {
                        let one_page = tabs.len() == 1;
                        if draft.selected_tab == 0 {
                            if one_page || draft.renderer_type == "ribbon" {
                                Self::renderer_specific_properties_ui(ui, &mut draft);
                                ui.add_space(8.0);
                            }
                            Self::renderer_common_properties_ui(
                                ui,
                                &mut draft,
                                &material_names,
                                &style_options,
                            );
                        } else {
                            Self::renderer_specific_properties_ui(ui, &mut draft);
                        }
                    });
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        accept = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if accept {
            match self.apply_renderer_properties(&draft) {
                Ok(()) => self.renderer_properties_draft = None,
                Err(error) => {
                    self.set_status(format!("Renderer properties error: {error}"));
                    self.renderer_properties_draft = Some(draft);
                }
            }
        } else if cancel || !open {
            self.renderer_properties_draft = None;
        } else {
            self.renderer_properties_draft = Some(draft);
        }
    }

    fn renderer_common_properties_ui(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        material_names: &[String],
        style_options: &[cuemol_qsys::StyleInfo],
    ) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.strong("Basic settings");
                ui.separator();
                egui::Grid::new("native_renderer_basic_properties")
                    .num_columns(2)
                    .spacing([14.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Name:");
                        ui.add_sized(
                            [ui.available_width().max(240.0), 22.0],
                            egui::TextEdit::singleline(&mut draft.name),
                        );
                        ui.end_row();

                        ui.label("Selection:");
                        let selection_readonly = draft.is_density_renderer()
                            || draft.is_renderer_group()
                            || matches!(
                                draft.renderer_type.as_str(),
                                "*selection" | "selection" | "*namelabel" | "name_label"
                            );
                        if selection_readonly {
                            let text = if draft.is_density_renderer()
                                || draft.is_renderer_group()
                                || matches!(
                                    draft.renderer_type.as_str(),
                                    "*namelabel" | "name_label"
                                ) {
                                "(not available)"
                            } else {
                                draft.selection.as_str()
                            };
                            ui.monospace(text);
                        } else {
                            ui.add_sized(
                                [ui.available_width().max(240.0), 22.0],
                                egui::TextEdit::singleline(&mut draft.selection)
                                    .font(egui::TextStyle::Monospace),
                            );
                        }
                        ui.end_row();

                        ui.label("");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut draft.visible, "Visible");
                            ui.checkbox(&mut draft.locked, "Locked");
                        });
                        ui.end_row();

                        ui.label("Style:");
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [ui.available_width().max(180.0) - 110.0, 22.0],
                                egui::TextEdit::singleline(&mut draft.style)
                                    .font(egui::TextStyle::Monospace),
                            );
                            egui::ComboBox::from_id_salt("native_renderer_style")
                                .selected_text("Add preset")
                                .show_ui(ui, |ui| {
                                    for style in style_options {
                                        let label = if style.description.is_empty() {
                                            style.name.as_str()
                                        } else {
                                            style.description.as_str()
                                        };
                                        if ui.button(label).on_hover_text(&style.name).clicked() {
                                            let mut names = draft
                                                .style
                                                .split(',')
                                                .map(str::trim)
                                                .filter(|name| !name.is_empty())
                                                .map(str::to_string)
                                                .collect::<Vec<_>>();
                                            if !names
                                                .iter()
                                                .any(|name| name.eq_ignore_ascii_case(&style.name))
                                            {
                                                names.push(style.name.clone());
                                                draft.style = names.join(",");
                                            }
                                            ui.close();
                                        }
                                    }
                                });
                        });
                        ui.end_row();

                        ui.label("Material:");
                        egui::ComboBox::from_id_salt("native_renderer_material")
                            .selected_text(if draft.material.is_empty() {
                                "(none)"
                            } else {
                                &draft.material
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut draft.material, String::new(), "(none)");
                                for name in material_names {
                                    ui.selectable_value(&mut draft.material, name.clone(), name);
                                }
                            });
                        ui.end_row();

                        ui.label("Opacity:");
                        ui.add(
                            egui::DragValue::new(&mut draft.alpha)
                                .speed(0.1)
                                .range(0.1..=1.0),
                        );
                        ui.end_row();
                    });
            });
        ui.add_space(8.0);
        egui::Frame::new()
            .stroke(egui::Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Edge lines:");
                    egui::ComboBox::from_id_salt("native_renderer_edge_type")
                        .selected_text(&draft.edge_type)
                        .show_ui(ui, |ui| {
                            for value in ["none", "edges", "silhouette"] {
                                ui.selectable_value(&mut draft.edge_type, value.to_string(), value);
                            }
                        });
                });
                ui.separator();
                ui.add_enabled_ui(draft.edge_type != "none", |ui| {
                    egui::Grid::new("native_renderer_edge_properties")
                        .num_columns(2)
                        .spacing([14.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Width:");
                            ui.add(
                                egui::DragValue::new(&mut draft.edge_width)
                                    .speed(0.01)
                                    .range(0.0..=1.0),
                            );
                            ui.end_row();
                            ui.label("Color:");
                            ui.text_edit_singleline(&mut draft.edge_color);
                            ui.end_row();
                        });
                });
            });
    }

    fn renderer_specific_properties_ui(ui: &mut egui::Ui, draft: &mut NativeRendererProperties) {
        let page = draft.tabs()[draft.selected_tab];
        match (draft.renderer_type.as_str(), page) {
            ("simple", "Simple") | ("trace", "Trace") => {
                Self::renderer_specific_group(ui, page, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Line width:");
                        ui.add(
                            egui::DragValue::new(&mut draft.line_width)
                                .speed(0.2)
                                .range(0.2..=10.0),
                        );
                    });
                });
            }
            ("spline", "Spline") => {
                Self::renderer_specific_group(ui, "Spline", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Line width:");
                        ui.add(
                            egui::DragValue::new(&mut draft.line_width)
                                .speed(0.2)
                                .range(0.2..=10.0),
                        );
                    });
                    Self::renderer_specific_i32(
                        ui,
                        draft,
                        "axialdetail",
                        "Axial detail:",
                        6,
                        1,
                        100,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "smooth",
                        "Smoothing:",
                        0.0,
                        -1000.0,
                        1000.0,
                        0.05,
                    );
                    Self::renderer_specific_bool(ui, draft, "smoothcolor", "Smooth colors", true);
                    Self::renderer_specific_text(ui, draft, "pivotatom", "Pivot atom:", "");
                });
            }
            ("ballstick" | "anisou", "Ball & Stick") => {
                Self::renderer_specific_group(ui, "Ball & Stick settings", |ui| {
                    Self::renderer_number_row(ui, "Detail:", &mut draft.detail, 2..=20);
                    Self::renderer_float_row(
                        ui,
                        "Bond width:",
                        &mut draft.bond_width,
                        0.0..=3.0,
                        0.01,
                    );
                    Self::renderer_float_row(
                        ui,
                        "Atom radius:",
                        &mut draft.sphere_radius,
                        0.0..=3.0,
                        0.01,
                    );
                });
                ui.add_space(8.0);
                Self::renderer_specific_group(ui, "Ring settings", |ui| {
                    ui.checkbox(&mut draft.show_ring, "Show ring");
                    ui.add_enabled_ui(draft.show_ring, |ui| {
                        Self::renderer_float_row(
                            ui,
                            "Thickness:",
                            &mut draft.ring_thickness,
                            0.0..=3.0,
                            0.01,
                        );
                        ui.horizontal(|ui| {
                            ui.label("Color:");
                            ui.text_edit_singleline(&mut draft.ring_color);
                        });
                    });
                });
            }
            ("anisou", "Anisotropic U") => {
                Self::renderer_specific_group(ui, "Thermal ellipsoid", |ui| {
                    Self::renderer_specific_bool(
                        ui,
                        draft,
                        "drawdisc",
                        "Draw principal discs",
                        true,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "discscale",
                        "Disc scale:",
                        1.1,
                        0.0,
                        10.0,
                        0.1,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "discthick",
                        "Disc thickness:",
                        0.1,
                        0.0,
                        10.0,
                        0.05,
                    );
                    Self::renderer_specific_i32(
                        ui,
                        draft,
                        "maxverts",
                        "Maximum vertices:",
                        1_000_000,
                        1,
                        100_000_000,
                    );
                });
            }
            ("*namelabel" | "name_label", "Label") => {
                Self::renderer_specific_group(ui, "Atom labels", |ui| {
                    Self::renderer_specific_i32(
                        ui,
                        draft,
                        "maxlabel",
                        "Maximum labels:",
                        50,
                        0,
                        1_000_000,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "dispx",
                        "X displacement:",
                        0.0,
                        -100.0,
                        100.0,
                        0.5,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "dispy",
                        "Y displacement:",
                        0.0,
                        -100.0,
                        100.0,
                        0.5,
                    );
                    Self::renderer_specific_text(ui, draft, "color", "Color:", "#ffff00");
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "font_size",
                        "Font size:",
                        12.0,
                        1.0,
                        512.0,
                        0.5,
                    );
                    Self::renderer_specific_text(
                        ui,
                        draft,
                        "font_name",
                        "Font family:",
                        "sans-serif",
                    );
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "font_style",
                        "Font style:",
                        "normal",
                        &[
                            ("normal", "Normal"),
                            ("italic", "Italic"),
                            ("oblique", "Oblique"),
                        ],
                    );
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "font_weight",
                        "Font weight:",
                        "normal",
                        &[("normal", "Normal"), ("bold", "Bold")],
                    );
                });
            }
            ("*selection" | "selection", "Selection") => {
                Self::renderer_specific_group(ui, "Selection overlay", |ui| {
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "mode",
                        "Mode:",
                        "stick",
                        &[("stick", "Stick"), ("point", "Point")],
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "linew",
                        "Line width:",
                        3.0,
                        0.0,
                        20.0,
                        0.2,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "dispx",
                        "X displacement:",
                        3.0,
                        -100.0,
                        100.0,
                        0.5,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "dispy",
                        "Y displacement:",
                        3.0,
                        -100.0,
                        100.0,
                        0.5,
                    );
                    Self::renderer_specific_text(
                        ui,
                        draft,
                        "color",
                        "Color:",
                        "rgba(0,255,255,0.5)",
                    );
                });
            }
            ("atomintr", "Interaction") => {
                Self::renderer_specific_group(ui, "Interaction", |ui| {
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "mode",
                        "Mode:",
                        "fancy",
                        &[("simple", "Simple line"), ("fancy", "3D tube")],
                    );
                    Self::renderer_specific_bool(ui, draft, "showlabel", "Show label", false);
                    Self::renderer_specific_f64(ui, draft, "width", "Width:", 0.1, 0.0, 5.0, 0.05);
                    Self::renderer_specific_text(ui, draft, "color", "Color:", "#ffff00");
                    Self::renderer_specific_i32(ui, draft, "detail", "Detail:", 5, 0, 20);
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "captype_start",
                        "Start cap:",
                        "sphere",
                        &[("flat", "Flat"), ("sphere", "Round"), ("arrow", "Arrow")],
                    );
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "captype_end",
                        "End cap:",
                        "sphere",
                        &[("flat", "Flat"), ("sphere", "Round"), ("arrow", "Arrow")],
                    );
                    for index in 0..6 {
                        let key = format!("stipple{index}");
                        let label = format!("Stipple {}:", index + 1);
                        let fallback = if index < 2 { 1.0 } else { -1.0 };
                        Self::renderer_specific_f64_owned(
                            ui, draft, key, &label, fallback, -1.0, 100.0, 0.05,
                        );
                    }
                });
            }
            ("cpk", "Atom radii") => {
                Self::renderer_specific_group(ui, "Atom radii", |ui| {
                    for (key, label, fallback) in [
                        ("vdwr_C", "Carbon:", 1.7),
                        ("vdwr_N", "Nitrogen:", 1.55),
                        ("vdwr_O", "Oxygen:", 1.52),
                        ("vdwr_S", "Sulfur:", 1.8),
                        ("vdwr_P", "Phosphorus:", 1.8),
                        ("vdwr_H", "Hydrogen:", 1.2),
                        ("vdwr_X", "Others:", 1.7),
                    ] {
                        Self::renderer_specific_f64(
                            ui, draft, key, label, fallback, 0.0, 3.0, 0.01,
                        );
                    }
                    Self::renderer_specific_i32(ui, draft, "detail", "Detail:", 10, 2, 20);
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "glrender_mode",
                        "Rendering mode:",
                        "default",
                        &[
                            ("default", "Default"),
                            ("shader", "Shader"),
                            ("vbo", "VBO"),
                            ("glu", "GLU"),
                        ],
                    );
                });
            }
            ("disorder", "Disorder") => {
                Self::renderer_specific_group(ui, "Disorder", |ui| {
                    Self::renderer_specific_text(ui, draft, "target", "Target:", "");
                    Self::renderer_specific_i32(ui, draft, "detail", "Detail:", 5, 2, 20);
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "width",
                        "Dot size:",
                        0.3,
                        0.0,
                        3.0,
                        0.1,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "dotsep",
                        "Dot separation:",
                        1.0,
                        0.0,
                        3.0,
                        0.1,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "loopsize",
                        "Loop size:",
                        2.0,
                        0.0,
                        10.0,
                        0.1,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "loopsize2",
                        "Loop size 2:",
                        -1.0,
                        -1.0,
                        10.0,
                        0.1,
                    );
                    Self::renderer_specific_text(ui, draft, "defaultcolor", "Color:", "#ffffff");
                });
            }
            ("tube" | "nucl", "Tube") => {
                Self::renderer_tube_properties_ui(ui, draft);
            }
            ("nucl", "Nucleic acid") => {
                Self::renderer_nucl_properties_ui(ui, draft);
            }
            ("ribbon", "Common") => {
                Self::renderer_backbone_properties_ui(ui, draft, 6, true, "sphere");
            }
            ("ribbon", "Helix") => {
                Self::renderer_section_properties_ui(ui, draft, "helix", "Helix section");
                ui.add_space(8.0);
                Self::renderer_specific_group(ui, "Helix spline", |ui| {
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "helix_smooth",
                        "Smoothing:",
                        0.0,
                        0.0,
                        5.0,
                        0.1,
                    );
                    Self::renderer_specific_bool(
                        ui,
                        draft,
                        "helix_usebackcol",
                        "Use back-face color",
                        false,
                    );
                    Self::renderer_specific_text(
                        ui,
                        draft,
                        "helix_backcol",
                        "Back-face color:",
                        "#ffffff",
                    );
                });
                ui.add_space(8.0);
                Self::renderer_junction_properties_ui(ui, draft, "helixhead", "Helix head");
                ui.add_space(8.0);
                Self::renderer_junction_properties_ui(ui, draft, "helixtail", "Helix tail");
            }
            ("ribbon", "Sheet") => {
                Self::renderer_section_properties_ui(ui, draft, "sheet", "Sheet section");
                ui.add_space(8.0);
                Self::renderer_specific_group(ui, "Sheet spline", |ui| {
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "sheet_smooth",
                        "Smoothing:",
                        0.5,
                        0.0,
                        5.0,
                        0.1,
                    );
                    Self::renderer_specific_bool(
                        ui,
                        draft,
                        "sheet_usesidecol",
                        "Use side-face color",
                        false,
                    );
                    Self::renderer_specific_text(
                        ui,
                        draft,
                        "sheet_sidecol",
                        "Side-face color:",
                        "#ffffff",
                    );
                });
                ui.add_space(8.0);
                Self::renderer_junction_properties_ui(ui, draft, "sheethead", "Sheet head");
                ui.add_space(8.0);
                Self::renderer_junction_properties_ui(ui, draft, "sheettail", "Sheet tail");
            }
            ("ribbon", "Coil") => {
                Self::renderer_section_properties_ui(ui, draft, "coil", "Coil section");
                ui.add_space(8.0);
                Self::renderer_specific_group(ui, "Coil spline", |ui| {
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "coil_smooth",
                        "Smoothing:",
                        0.0,
                        0.0,
                        5.0,
                        0.1,
                    );
                });
            }
            ("cartoon", "Cartoon") => {
                Self::renderer_backbone_properties_ui(ui, draft, 8, false, "flat");
                ui.add_space(8.0);
                Self::renderer_specific_group(ui, "Anchor selection", |ui| {
                    Self::renderer_specific_text(ui, draft, "anchor_sel", "Selection:", "");
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "anchor_weight",
                        "Weight:",
                        10.0,
                        0.0,
                        20.0,
                        0.5,
                    );
                });
            }
            ("cartoon", "Helix") => {
                Self::renderer_specific_group(ui, "Helix", |ui| {
                    Self::renderer_specific_bool(
                        ui,
                        draft,
                        "helix_ribbon",
                        "Draw helix as ribbon",
                        false,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "helix_smooth",
                        "Smoothing:",
                        3.0,
                        -5.0,
                        5.0,
                        0.1,
                    );
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "helix_width_mode",
                        "Width mode:",
                        "average",
                        &[
                            ("const", "Constant"),
                            ("average", "Average"),
                            ("wavy", "Wavy"),
                        ],
                    );
                    for (key, label, fallback, minimum, maximum) in [
                        ("helix_width", "Width:", 2.3, 0.01, 10.0),
                        ("helix_extend", "Extension:", 0.5, 0.0, 3.0),
                        ("helix_wsmooth", "Width smoothing:", 5.0, -5.0, 5.0),
                        ("helix_wplus", "Width adjustment:", 0.2, -2.0, 3.0),
                    ] {
                        Self::renderer_specific_f64(
                            ui, draft, key, label, fallback, minimum, maximum, 0.1,
                        );
                    }
                });
                ui.add_space(8.0);
                Self::renderer_section_properties_ui(ui, draft, "ribhelix", "Ribbon helix section");
                ui.add_space(8.0);
                Self::renderer_junction_properties_ui(
                    ui,
                    draft,
                    "ribhelix_head",
                    "Ribbon helix head",
                );
                ui.add_space(8.0);
                Self::renderer_junction_properties_ui(
                    ui,
                    draft,
                    "ribhelix_tail",
                    "Ribbon helix tail",
                );
            }
            ("cartoon", "Sheet") => {
                Self::renderer_section_properties_ui(ui, draft, "sheet", "Sheet section");
                ui.add_space(8.0);
                Self::renderer_specific_group(ui, "Sheet spline", |ui| {
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "sheet_smooth",
                        "Smoothing:",
                        2.0,
                        -5.0,
                        5.0,
                        0.1,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "sheet_wsmooth",
                        "Width smoothing:",
                        5.0,
                        -5.0,
                        5.0,
                        0.1,
                    );
                });
                ui.add_space(8.0);
                Self::renderer_junction_properties_ui(ui, draft, "sheethead", "Sheet head");
            }
            ("cartoon", "Coil") => {
                Self::renderer_section_properties_ui(ui, draft, "coil", "Coil section");
                ui.add_space(8.0);
                Self::renderer_specific_group(ui, "Coil spline", |ui| {
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "coil_smooth",
                        "Smoothing:",
                        -1.0,
                        -5.0,
                        5.0,
                        0.1,
                    );
                });
            }
            ("contour" | "gpu_mapmesh" | "isosurf" | "gpu_mapvol", "Map") => {
                Self::renderer_map_properties_ui(ui, draft);
            }
            ("surface" | "molsurf" | "dsurface" | "directsurf" | "dsurf2", "MolSurf") => {
                Self::renderer_surface_properties_ui(ui, draft);
            }
            _ => {
                ui.weak("Properties on this page are inherited from the current style.");
            }
        }
    }

    fn renderer_backbone_properties_ui(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        axial_detail: i32,
        smooth_color: bool,
        cap_type: &str,
    ) {
        Self::renderer_specific_group(ui, "Backbone spline", |ui| {
            Self::renderer_specific_i32(
                ui,
                draft,
                "axialdetail",
                "Axial detail:",
                axial_detail,
                2,
                20,
            );
            Self::renderer_specific_bool(ui, draft, "smoothcolor", "Smooth color", smooth_color);
            Self::renderer_specific_bool(ui, draft, "segend_fade", "Fade at segment ends", false);
            for (key, label) in [("start_captype", "Start cap:"), ("end_captype", "End cap:")] {
                Self::renderer_specific_choice(
                    ui,
                    draft,
                    key,
                    label,
                    cap_type,
                    &[("flat", "Flat"), ("sphere", "Sphere"), ("none", "None")],
                );
            }
        });
        ui.add_space(8.0);
        Self::renderer_specific_group(ui, "Pivot atom", |ui| {
            Self::renderer_specific_text(ui, draft, "pivotatom", "Atom name:", "");
        });
    }

    fn renderer_section_properties_ui(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        key: &str,
        title: &str,
    ) {
        Self::renderer_specific_group(ui, title, |ui| {
            let type_key = format!("{key}.type");
            Self::renderer_specific_choice(
                ui,
                draft,
                &type_key,
                "Type:",
                "elliptical",
                &[
                    ("elliptical", "Elliptical"),
                    ("roundsquare", "Rounded square"),
                    ("rectangle", "Rectangle"),
                    ("fancy1", "Fancy"),
                ],
            );
            let detail_key = format!("{key}.detail");
            Self::renderer_specific_i32(ui, draft, &detail_key, "Detail:", 16, 2, 20);
            let width_key = format!("{key}.width");
            Self::renderer_specific_f64(ui, draft, &width_key, "Width 1:", 0.35, 0.01, 10.0, 0.01);
            let width = draft
                .specific_value(&width_key, "0.35")
                .parse::<f64>()
                .unwrap_or(0.35)
                .max(f64::EPSILON);
            let ratio_key = format!("{key}.tuber");
            let ratio = draft
                .specific_value(&ratio_key, "1")
                .parse::<f64>()
                .unwrap_or(1.0);
            let mut width_2 = width * ratio;
            ui.horizontal(|ui| {
                ui.label("Width 2:");
                if ui
                    .add(
                        egui::DragValue::new(&mut width_2)
                            .speed(0.01)
                            .range(0.01..=10.0),
                    )
                    .changed()
                {
                    draft
                        .specific
                        .insert(ratio_key, (width_2 / width).to_string());
                }
            });
            let sharp_key = format!("{key}.sharp");
            Self::renderer_specific_f64(ui, draft, &sharp_key, "Sharpness:", 0.4, 0.0, 1.0, 0.05);
        });
    }

    fn renderer_junction_properties_ui(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        key: &str,
        title: &str,
    ) {
        Self::renderer_specific_group(ui, title, |ui| {
            let type_key = format!("{key}.type");
            Self::renderer_specific_choice(
                ui,
                draft,
                &type_key,
                "Type:",
                "smooth",
                &[("smooth", "Smooth"), ("flat", "Flat"), ("arrow", "Arrow")],
            );
            let arrow = draft.specific_value(&type_key, "smooth") == "arrow";
            ui.add_enabled_ui(arrow, |ui| {
                let gamma_key = format!("{key}.gamma");
                Self::renderer_specific_f64(ui, draft, &gamma_key, "Gamma:", 2.2, 0.1, 10.0, 0.1);
                for (suffix, label, fallback, minimum, maximum) in [
                    ("basw", "Base width (%):", 0.0, 0.0, 1.0),
                    ("arrow", "Arrow width (%):", 1.8, 1.0, 3.0),
                ] {
                    let property = format!("{key}.{suffix}");
                    Self::renderer_specific_percentage(
                        ui, draft, &property, label, fallback, minimum, maximum,
                    );
                }
            });
        });
    }

    fn renderer_specific_percentage(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        key: &str,
        label: &str,
        fallback: f64,
        minimum: f64,
        maximum: f64,
    ) {
        let mut value = draft
            .specific_value(key, &fallback.to_string())
            .parse::<f64>()
            .unwrap_or(fallback)
            * 100.0;
        ui.horizontal(|ui| {
            ui.label(label);
            if ui
                .add(
                    egui::DragValue::new(&mut value)
                        .speed(5.0)
                        .range(minimum * 100.0..=maximum * 100.0),
                )
                .changed()
            {
                draft
                    .specific
                    .insert(key.to_string(), (value / 100.0).to_string());
            }
        });
    }

    fn renderer_tube_properties_ui(ui: &mut egui::Ui, draft: &mut NativeRendererProperties) {
        let enabled = draft.renderer_type != "nucl"
            || draft
                .specific
                .get("show_tube")
                .map(|value| value_is_true(value))
                .unwrap_or(true);
        ui.add_enabled_ui(enabled, |ui| {
            Self::renderer_section_properties_ui(ui, draft, "section", "Section");
            ui.add_space(8.0);
            Self::renderer_specific_group(ui, "Spline", |ui| {
                Self::renderer_specific_i32(ui, draft, "axialdetail", "Axial detail:", 6, 2, 20);
                Self::renderer_specific_f64(ui, draft, "smooth", "Smoothing:", 0.0, 0.0, 0.5, 0.05);
                Self::renderer_specific_bool(ui, draft, "smoothcolor", "Smooth color", true);
                Self::renderer_specific_bool(
                    ui,
                    draft,
                    "segend_fade",
                    "Fade at segment ends",
                    false,
                );
                Self::renderer_specific_choice(
                    ui,
                    draft,
                    "start_captype",
                    "Cap type:",
                    "sphere",
                    &[("flat", "Flat"), ("sphere", "Sphere"), ("none", "None")],
                );
                let cap = draft.specific_value("start_captype", "sphere");
                draft.specific.insert("end_captype".to_string(), cap);
            });
            ui.add_space(8.0);
            Self::renderer_specific_group(ui, "Pivot atom", |ui| {
                Self::renderer_specific_text(ui, draft, "pivotatom", "Atom name:", "");
            });
            ui.add_space(8.0);
            Self::renderer_specific_group(ui, "Putty", |ui| {
                Self::renderer_specific_choice(
                    ui,
                    draft,
                    "putty_mode",
                    "Mode:",
                    "none",
                    &[
                        ("none", "None"),
                        ("linear1", "Linear"),
                        ("scale1", "Scaled"),
                    ],
                );
                let putty = draft.specific_value("putty_mode", "none") != "none";
                ui.add_enabled_ui(putty, |ui| {
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "putty_tgt",
                        "Target:",
                        "bfac",
                        &[("bfac", "B-factor"), ("occ", "Occupancy")],
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "putty_loscl",
                        "Low scale:",
                        3.0,
                        0.01,
                        10.0,
                        0.1,
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "putty_hiscl",
                        "High scale:",
                        3.0,
                        0.01,
                        10.0,
                        0.1,
                    );
                });
            });
        });
    }

    fn renderer_nucl_properties_ui(ui: &mut egui::Ui, draft: &mut NativeRendererProperties) {
        Self::renderer_specific_group(ui, "Nucleic acid", |ui| {
            Self::renderer_specific_bool(ui, draft, "show_tube", "Show tube", true);
            Self::renderer_specific_bool(ui, draft, "show_basepair", "Connect base pair", true);
        });
        ui.add_space(8.0);
        let show_base = draft
            .specific
            .get("show_basepair")
            .map(|value| value_is_true(value))
            .unwrap_or(true);
        ui.add_enabled_ui(show_base, |ui| {
            Self::renderer_specific_group(ui, "Base", |ui| {
                Self::renderer_specific_choice(
                    ui,
                    draft,
                    "base_type",
                    "Type:",
                    "basepair",
                    &[
                        ("basepair", "Base pair"),
                        ("simple1", "Simple"),
                        ("detail1", "Detailed 1"),
                        ("detail2", "Detailed 2"),
                    ],
                );
                Self::renderer_specific_i32(ui, draft, "base_detail", "Detail:", 3, 2, 20);
                Self::renderer_specific_f64(ui, draft, "base_size", "Size:", 0.5, 0.0, 10.0, 0.05);
                let base_size = draft
                    .specific_value("base_size", "0.5")
                    .parse::<f64>()
                    .unwrap_or(0.5)
                    .max(f64::EPSILON);
                let thickness = draft
                    .specific_value("base_thick", "0.5")
                    .parse::<f64>()
                    .unwrap_or(0.5);
                let mut percentage = thickness * 100.0 / base_size;
                ui.horizontal(|ui| {
                    ui.label("Thickness (%):");
                    if ui
                        .add(
                            egui::DragValue::new(&mut percentage)
                                .speed(5.0)
                                .range(0.0..=100.0),
                        )
                        .changed()
                    {
                        draft.specific.insert(
                            "base_thick".to_string(),
                            (percentage * base_size / 100.0).to_string(),
                        );
                    }
                });
            });
        });
    }

    fn renderer_map_properties_ui(ui: &mut egui::Ui, draft: &mut NativeRendererProperties) {
        Self::renderer_specific_group(ui, "Map", |ui| {
            let auto = draft
                .specific
                .get("autoupdate")
                .map(|value| value_is_true(value))
                .unwrap_or(true);
            let drag = draft
                .specific
                .get("dragupdate")
                .map(|value| value_is_true(value))
                .unwrap_or(false);
            let mut update_mode = if !auto {
                "none".to_string()
            } else if drag {
                "drag".to_string()
            } else {
                "auto".to_string()
            };
            ui.horizontal(|ui| {
                ui.label("Center update:");
                egui::ComboBox::from_id_salt("native_renderer_map_update")
                    .selected_text(&update_mode)
                    .show_ui(ui, |ui| {
                        for (value, label) in [
                            ("none", "None"),
                            ("auto", "Automatic"),
                            ("drag", "Automatic (drag)"),
                        ] {
                            ui.selectable_value(&mut update_mode, value.to_string(), label);
                        }
                    });
            });
            draft.specific.insert(
                "autoupdate".to_string(),
                (update_mode != "none").to_string(),
            );
            draft.specific.insert(
                "dragupdate".to_string(),
                (update_mode == "drag").to_string(),
            );
            match draft.renderer_type.as_str() {
                "isosurf" => {
                    Self::renderer_specific_choice(
                        ui,
                        draft,
                        "drawmode",
                        "Drawing Mode:",
                        "fill",
                        &[("fill", "fill"), ("line", "line"), ("point", "point")],
                    );
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "width",
                        "Line/Point size:",
                        1.2,
                        0.1,
                        10.0,
                        0.1,
                    );
                    Self::renderer_specific_i32(
                        ui,
                        draft,
                        "max_grids",
                        "Max grid size:",
                        100,
                        10,
                        2000,
                    );
                    Self::renderer_specific_bool(ui, draft, "cullface", "Back-face culling", false);
                }
                "contour" | "gpu_mapmesh" => {
                    Self::renderer_specific_f64(
                        ui,
                        draft,
                        "width",
                        "Line width:",
                        1.0,
                        0.1,
                        10.0,
                        0.1,
                    );
                    Self::renderer_specific_i32(ui, draft, "bufsize", "Buffer size:", 100, 50, 200);
                }
                "gpu_mapvol" => Self::renderer_specific_choice(
                    ui,
                    draft,
                    "xferfn_type",
                    "Transfer function:",
                    "rainbow1",
                    &[
                        ("rainbow1", "Rainbow"),
                        ("auto2", "Automatic"),
                        ("multigrad", "Multi-gradient"),
                    ],
                ),
                _ => {}
            }
            Self::renderer_specific_bool(ui, draft, "use_pbc", "Use periodic boundary", true);
        });
        ui.add_space(8.0);
        Self::renderer_specific_group(ui, "Display boundary", |ui| {
            Self::renderer_specific_text(ui, draft, "bndry_molname", "Target:", "");
            Self::renderer_specific_text(ui, draft, "bndry_sel", "Selection:", "all");
            Self::renderer_specific_f64(ui, draft, "bndry_rng", "Distance:", 5.0, 0.1, 100.0, 0.1);
        });
    }

    fn renderer_surface_properties_ui(ui: &mut egui::Ui, draft: &mut NativeRendererProperties) {
        let direct = matches!(
            draft.renderer_type.as_str(),
            "dsurface" | "directsurf" | "dsurf2"
        );
        Self::renderer_specific_group(ui, "Draw", |ui| {
            Self::renderer_specific_choice(
                ui,
                draft,
                "drawmode",
                "Drawing Mode:",
                "fill",
                &[("fill", "fill"), ("line", "line"), ("point", "point")],
            );
            Self::renderer_specific_f64(
                ui,
                draft,
                "width",
                "Line/Point size:",
                1.2,
                0.1,
                10.0,
                0.1,
            );
            ui.add_enabled_ui(direct, |ui| {
                Self::renderer_specific_choice(
                    ui,
                    draft,
                    "surftype",
                    "Surface type:",
                    "ses",
                    &[
                        ("vdw", "van der Waals"),
                        ("sas", "Solvent accessible"),
                        ("ses", "Solvent excluded"),
                    ],
                );
                Self::renderer_specific_i32(ui, draft, "detail", "Detail:", 6, 1, 21);
            });
        });
        ui.add_space(8.0);
        Self::renderer_specific_group(ui, "Show selected", |ui| {
            Self::renderer_specific_text(ui, draft, "target", "Selection mol:", "");
            Self::renderer_specific_text(ui, draft, "showsel", "Selection:", "*");
        });
        ui.add_space(8.0);
        let choices = if direct {
            &[("molecule", "By molecule"), ("potential", "By potential")][..]
        } else {
            &[
                ("solid", "Solid color"),
                ("molecule", "By molecule"),
                ("potential", "By potential"),
                ("multigrad", "Multi-gradient"),
            ][..]
        };
        Self::renderer_specific_choice(
            ui,
            draft,
            "colormode",
            "Coloring mode:",
            if direct { "molecule" } else { "solid" },
            choices,
        );
    }

    fn renderer_specific_group(
        ui: &mut egui::Ui,
        title: &str,
        add_contents: impl FnOnce(&mut egui::Ui),
    ) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.strong(title);
                ui.separator();
                add_contents(ui);
            });
    }

    fn renderer_number_row(
        ui: &mut egui::Ui,
        label: &str,
        value: &mut i32,
        range: std::ops::RangeInclusive<i32>,
    ) {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.add(egui::DragValue::new(value).speed(1).range(range));
        });
    }

    fn renderer_float_row(
        ui: &mut egui::Ui,
        label: &str,
        value: &mut f64,
        range: std::ops::RangeInclusive<f64>,
        speed: f64,
    ) {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.add(egui::DragValue::new(value).speed(speed).range(range));
        });
    }

    fn renderer_specific_text(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        key: &str,
        label: &str,
        fallback: &str,
    ) {
        let mut value = draft.specific_value(key, fallback);
        ui.horizontal(|ui| {
            ui.label(label);
            if ui.text_edit_singleline(&mut value).changed() {
                draft.specific.insert(key.to_string(), value);
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn renderer_specific_f64(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        key: &str,
        label: &str,
        fallback: f64,
        minimum: f64,
        maximum: f64,
        speed: f64,
    ) {
        Self::renderer_specific_f64_owned(
            ui,
            draft,
            key.to_string(),
            label,
            fallback,
            minimum,
            maximum,
            speed,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn renderer_specific_f64_owned(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        key: String,
        label: &str,
        fallback: f64,
        minimum: f64,
        maximum: f64,
        speed: f64,
    ) {
        let mut value = draft
            .specific_value(&key, &fallback.to_string())
            .parse::<f64>()
            .unwrap_or(fallback);
        ui.horizontal(|ui| {
            ui.label(label);
            if ui
                .add(
                    egui::DragValue::new(&mut value)
                        .speed(speed)
                        .range(minimum..=maximum),
                )
                .changed()
            {
                draft.specific.insert(key, value.to_string());
            }
        });
    }

    fn renderer_specific_i32(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        key: &str,
        label: &str,
        fallback: i32,
        minimum: i32,
        maximum: i32,
    ) {
        let mut value = draft
            .specific_value(key, &fallback.to_string())
            .parse::<i32>()
            .unwrap_or(fallback);
        ui.horizontal(|ui| {
            ui.label(label);
            if ui
                .add(
                    egui::DragValue::new(&mut value)
                        .speed(1)
                        .range(minimum..=maximum),
                )
                .changed()
            {
                draft.specific.insert(key.to_string(), value.to_string());
            }
        });
    }

    fn renderer_specific_bool(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        key: &str,
        label: &str,
        fallback: bool,
    ) {
        let mut value = draft
            .specific
            .get(key)
            .map(|value| value_is_true(value))
            .unwrap_or(fallback);
        if ui.checkbox(&mut value, label).changed() {
            draft.specific.insert(key.to_string(), value.to_string());
        }
    }

    fn renderer_specific_choice(
        ui: &mut egui::Ui,
        draft: &mut NativeRendererProperties,
        key: &str,
        label: &str,
        fallback: &str,
        choices: &[(&str, &str)],
    ) {
        let mut value = draft.specific_value(key, fallback);
        ui.horizontal(|ui| {
            ui.label(label);
            egui::ComboBox::from_id_salt(("native_renderer_specific", key))
                .selected_text(
                    choices
                        .iter()
                        .find_map(|(candidate, caption)| (*candidate == value).then_some(*caption))
                        .unwrap_or(&value),
                )
                .show_ui(ui, |ui| {
                    for (candidate, caption) in choices {
                        ui.selectable_value(&mut value, (*candidate).to_string(), *caption);
                    }
                });
        });
        draft.specific.insert(key.to_string(), value);
    }

    fn apply_renderer_properties(
        &mut self,
        draft: &NativeRendererProperties,
    ) -> std::result::Result<(), String> {
        match draft.owner {
            NativePropertyObject::Molecule(object_uid) => {
                self.apply_molecule_renderer_properties(draft, object_uid)
            }
            NativePropertyObject::DensityMap(map_uid) => {
                self.apply_density_renderer_properties(draft, map_uid)
            }
            NativePropertyObject::Surface(surface_uid) => {
                self.apply_surface_renderer_properties(draft, surface_uid)
            }
        }
    }

    fn apply_molecule_renderer_properties(
        &mut self,
        draft: &NativeRendererProperties,
        object_uid: cuemol_core::Uid,
    ) -> std::result::Result<(), String> {
        validate_native_renderer_properties(draft)?;
        let scene_uid = self
            .state
            .scene_manager()
            .scene_uid_for_object(object_uid)
            .ok_or_else(|| format!("object {} was not found", object_uid.get()))?;
        let renderer_type = self
            .state
            .scene_manager()
            .object(object_uid)
            .and_then(|object| object.renderer(draft.renderer_uid))
            .map(|renderer| renderer.renderer_type().to_string())
            .ok_or_else(|| format!("renderer {} was not found", draft.renderer_uid.get()))?;
        if renderer_type != draft.renderer_type {
            return Err("renderer type changed while the dialog was open".to_string());
        }
        let undo = self.capture_undo();
        if renderer_type == "*group" {
            self.state
                .set_renderer_group_common_properties(
                    object_uid,
                    draft.renderer_uid,
                    draft.name.trim(),
                    draft.visible,
                    draft.locked,
                    &draft.material,
                    draft.alpha,
                    &draft.edge_type,
                    draft.edge_width,
                    draft.edge_color.trim(),
                )
                .map_err(|error| error.to_string())?;
        } else {
            self.state
                .set_renderer_common_properties(
                    object_uid,
                    draft.renderer_uid,
                    draft.name.trim(),
                    &draft.selection,
                    draft.visible,
                    draft.locked,
                    &draft.material,
                    draft.alpha,
                    &draft.edge_type,
                    draft.edge_width,
                    draft.edge_color.trim(),
                )
                .map_err(|error| error.to_string())?;
        }
        self.state
            .set_renderer_styles(object_uid, draft.renderer_uid, draft.style.trim())
            .map_err(|error| error.to_string())?;

        match renderer_type.as_str() {
            "simple" | "trace" => self.set_renderer_draft_property(
                draft,
                "width",
                draft.line_width.to_string(),
                (draft.line_width - 1.2).abs() < f64::EPSILON,
            )?,
            "spline" => self.set_renderer_draft_property(
                draft,
                "line_width",
                draft.line_width.to_string(),
                (draft.line_width - 1.2).abs() < f64::EPSILON,
            )?,
            "ballstick" | "anisou" => {
                for (key, value, is_default) in [
                    ("detail", draft.detail.to_string(), draft.detail == 3),
                    (
                        "bondw",
                        draft.bond_width.to_string(),
                        (draft.bond_width - 0.2).abs() < f64::EPSILON,
                    ),
                    (
                        "sphr",
                        draft.sphere_radius.to_string(),
                        (draft.sphere_radius - 0.3).abs() < f64::EPSILON,
                    ),
                    ("ring", draft.show_ring.to_string(), !draft.show_ring),
                    (
                        "thickness",
                        draft.ring_thickness.to_string(),
                        (draft.ring_thickness - 0.2).abs() < f64::EPSILON,
                    ),
                    (
                        "ringcolor",
                        draft.ring_color.trim().to_string(),
                        draft.ring_color.trim().eq_ignore_ascii_case("#ffffbf"),
                    ),
                ] {
                    self.set_renderer_draft_property(draft, key, value, is_default)?;
                }
            }
            _ => {}
        }
        for (key, default) in native_renderer_specific_defaults(&renderer_type) {
            let value = draft.specific_value(key, default);
            self.set_renderer_draft_property(
                draft,
                key,
                value.clone(),
                renderer_value_is_default(&value, default),
            )?;
        }
        self.push_undo_snapshot(undo);
        self.modified_scenes.insert(scene_uid);
        self.selected_object = Some(object_uid);
        self.selected_renderer = Some(draft.renderer_uid);
        self.workspace_aux_selection = None;
        self.rebuild_snapshot();
        self.set_status("Renderer properties updated");
        Ok(())
    }

    fn apply_density_renderer_properties(
        &mut self,
        draft: &NativeRendererProperties,
        map_uid: cuemol_core::Uid,
    ) -> std::result::Result<(), String> {
        validate_native_renderer_properties(draft)?;
        let map = self
            .state
            .density_map_summary(map_uid)
            .ok_or_else(|| format!("density map {} was not found", map_uid.get()))?;
        let renderer_type = map
            .renderers
            .iter()
            .find(|renderer| renderer.uid == draft.renderer_uid)
            .map(|renderer| renderer.renderer_type.as_str())
            .ok_or_else(|| format!("renderer {} was not found", draft.renderer_uid.get()))?;
        if renderer_type != draft.renderer_type {
            return Err("renderer type changed while the dialog was open".to_string());
        }
        let scene_uid = map.scene_uid;
        let undo = self.capture_undo();
        self.state
            .set_density_renderer_properties(
                map_uid,
                draft.renderer_uid,
                draft.name.trim(),
                draft.visible,
                draft.locked,
                &draft.material,
                draft.alpha,
                &draft.edge_type,
                draft.edge_width,
                draft.edge_color.trim(),
                &draft.specific,
            )
            .map_err(|error| error.to_string())?;
        self.state
            .set_density_renderer_styles(map_uid, draft.renderer_uid, draft.style.trim())
            .map_err(|error| error.to_string())?;
        self.push_undo_snapshot(undo);
        self.modified_scenes.insert(scene_uid);
        self.selected_object = None;
        self.selected_renderer = None;
        self.workspace_aux_selection = Some(WorkspaceAuxSelection::DensityMap {
            scene_uid,
            map_uid,
            renderer_uid: Some(draft.renderer_uid),
        });
        self.rebuild_snapshot();
        self.set_status("Renderer properties updated");
        Ok(())
    }

    fn apply_surface_renderer_properties(
        &mut self,
        draft: &NativeRendererProperties,
        surface_uid: cuemol_core::Uid,
    ) -> std::result::Result<(), String> {
        validate_native_renderer_properties(draft)?;
        let surface = self
            .state
            .surface_summary(surface_uid)
            .ok_or_else(|| format!("surface {} was not found", surface_uid.get()))?;
        let renderer_type = surface
            .renderers
            .iter()
            .find(|renderer| renderer.uid == draft.renderer_uid)
            .map(|renderer| renderer.renderer_type.as_str())
            .ok_or_else(|| format!("renderer {} was not found", draft.renderer_uid.get()))?;
        if renderer_type != draft.renderer_type {
            return Err("renderer type changed while the dialog was open".to_string());
        }
        let scene_uid = surface.scene_uid;
        let undo = self.capture_undo();
        self.state
            .set_surface_renderer_properties(
                surface_uid,
                draft.renderer_uid,
                draft.name.trim(),
                &draft.selection,
                draft.visible,
                draft.locked,
                &draft.material,
                draft.alpha,
                &draft.edge_type,
                draft.edge_width,
                draft.edge_color.trim(),
                &draft.specific,
            )
            .map_err(|error| error.to_string())?;
        self.state
            .set_surface_renderer_styles(surface_uid, draft.renderer_uid, draft.style.trim())
            .map_err(|error| error.to_string())?;
        self.push_undo_snapshot(undo);
        self.modified_scenes.insert(scene_uid);
        self.selected_object = None;
        self.selected_renderer = None;
        self.workspace_aux_selection = Some(WorkspaceAuxSelection::Surface {
            scene_uid,
            surface_uid,
            renderer_uid: Some(draft.renderer_uid),
        });
        self.rebuild_snapshot();
        self.set_status("Renderer properties updated");
        Ok(())
    }

    fn set_renderer_draft_property(
        &mut self,
        draft: &NativeRendererProperties,
        key: &str,
        value: String,
        reset: bool,
    ) -> std::result::Result<(), String> {
        let NativePropertyObject::Molecule(object_uid) = draft.owner else {
            return Err("renderer property owner is not a molecule".to_string());
        };
        let result = if let Some((child, attribute)) = key.split_once('.') {
            if reset {
                self.state.reset_renderer_child_property(
                    object_uid,
                    draft.renderer_uid,
                    child,
                    attribute,
                )
            } else {
                self.state.set_renderer_child_property(
                    object_uid,
                    draft.renderer_uid,
                    child,
                    attribute,
                    value,
                )
            }
        } else if reset {
            self.state
                .reset_renderer_property(object_uid, draft.renderer_uid, key)
        } else {
            self.state
                .set_renderer_property(object_uid, draft.renderer_uid, key, value)
        };
        result.map_err(|error| error.to_string())
    }

    fn open_scene_properties(&mut self) {
        let Some(mut properties) = NativeSceneProperties::from_state(&self.state) else {
            self.set_status("No active scene");
            return;
        };
        properties.modified = self
            .state
            .active_scene()
            .is_some_and(|uid| self.modified_scenes.contains(&uid));
        self.scene_properties_draft = Some(properties);
        self.scene_property_selected = 0;
        self.show_scene_props = true;
    }

    fn scene_props_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_scene_props {
            return;
        }
        let Some(mut draft) = self
            .scene_properties_draft
            .take()
            .or_else(|| NativeSceneProperties::from_state(&self.state))
        else {
            self.show_scene_props = false;
            return;
        };
        let mut selected = self
            .scene_property_selected
            .min(SCENE_PROPERTY_DEFINITIONS.len().saturating_sub(1));
        let mut open = self.show_scene_props;
        let mut apply = false;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Object properties")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(640.0)
            .min_width(560.0)
            .show(ctx, |ui| {
                let _ = ui.selectable_label(true, "Generic");
                ui.separator();
                Self::scene_property_editor_ui(ui, &mut draft, &mut selected);
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        accept = true;
                    }
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        self.scene_property_selected = selected;
        if accept || apply {
            match self.apply_scene_properties(&draft) {
                Ok(()) => {
                    self.set_status("Scene properties updated");
                    if accept {
                        self.show_scene_props = false;
                        self.scene_properties_draft = None;
                    } else {
                        let mut refreshed =
                            NativeSceneProperties::from_state(&self.state).unwrap_or(draft);
                        refreshed.modified = true;
                        self.show_scene_props = true;
                        self.scene_properties_draft = Some(refreshed);
                    }
                }
                Err(error) => {
                    self.set_status(format!("Scene properties error: {error}"));
                    self.show_scene_props = true;
                    self.scene_properties_draft = Some(draft);
                }
            }
        } else if cancel || !open {
            self.show_scene_props = false;
            self.scene_properties_draft = None;
        } else {
            self.show_scene_props = true;
            self.scene_properties_draft = Some(draft);
        }
    }

    fn scene_property_editor_ui(
        ui: &mut egui::Ui,
        draft: &mut NativeSceneProperties,
        selected: &mut usize,
    ) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        egui::Grid::new("native_scene_property_table")
                            .striped(true)
                            .num_columns(4)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                for heading in ["Name", "R", "Type", "Value"] {
                                    ui.strong(heading);
                                }
                                ui.end_row();
                                for (index, definition) in
                                    SCENE_PROPERTY_DEFINITIONS.iter().enumerate()
                                {
                                    if ui
                                        .selectable_label(*selected == index, definition.name)
                                        .clicked()
                                    {
                                        *selected = index;
                                    }
                                    ui.label(if definition.readonly { "R" } else { "" });
                                    ui.label(definition.type_name);
                                    ui.monospace(scene_property_value(draft, definition.key));
                                    ui.end_row();
                                }
                            });
                    });
            });
        ui.add_space(8.0);
        let definition = SCENE_PROPERTY_DEFINITIONS[*selected];
        egui::Grid::new("native_scene_property_detail")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Name:");
                ui.monospace(definition.name);
                ui.end_row();
                ui.label("Type:");
                ui.monospace(definition.type_name);
                ui.end_row();
                ui.label("Value:");
                ui.horizontal(|ui| {
                    let mut is_default = scene_property_is_default(draft, definition.key);
                    if ui
                        .add_enabled(
                            definition.has_default && !definition.readonly,
                            egui::Checkbox::new(&mut is_default, "default"),
                        )
                        .changed()
                        && is_default
                    {
                        reset_scene_property(draft, definition.key);
                    }
                    Self::scene_property_control_ui(ui, draft, definition);
                });
                ui.end_row();
            });
    }

    fn scene_property_control_ui(
        ui: &mut egui::Ui,
        draft: &mut NativeSceneProperties,
        definition: ScenePropertyDefinition,
    ) {
        use ScenePropertyKey as Key;
        match definition.key {
            Key::Background => {
                ui.text_edit_singleline(&mut draft.background);
                let background = SceneBackground::parse(&draft.background)
                    .unwrap_or(SceneBackground::Black)
                    .rgba_f32();
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (background[0] * 255.0).round() as u8,
                    (background[1] * 255.0).round() as u8,
                    (background[2] * 255.0).round() as u8,
                    (background[3] * 255.0).round() as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    let [red, green, blue, _] = color.to_array();
                    draft.background = format!("#{red:02x}{green:02x}{blue:02x}");
                }
            }
            Key::AoEnabled => {
                ui.checkbox(&mut draft.ao_enabled, "");
            }
            Key::AoRadius => {
                ui.add(egui::DragValue::new(&mut draft.ao_radius).speed(0.1));
            }
            Key::AoIntensity => {
                ui.add(egui::DragValue::new(&mut draft.ao_intensity).speed(0.1));
            }
            Key::AoSlices => {
                ui.add(egui::DragValue::new(&mut draft.ao_slices).range(1..=64));
            }
            Key::AoSteps => {
                ui.add(egui::DragValue::new(&mut draft.ao_steps).range(1..=32));
            }
            Key::AoHalfRes => {
                ui.checkbox(&mut draft.ao_half_res, "");
            }
            Key::AaMethod => {
                egui::ComboBox::from_id_salt("native_scene_aa_method")
                    .selected_text(&draft.aa_method)
                    .show_ui(ui, |ui| {
                        for method in ["none", "fxaa", "smaa"] {
                            ui.selectable_value(&mut draft.aa_method, method.to_string(), method);
                        }
                    });
            }
            Key::AaJitterLevel => {
                ui.add(egui::DragValue::new(&mut draft.aa_jitter_level).range(0..=5));
            }
            Key::Onload => {
                ui.text_edit_singleline(&mut draft.onload);
            }
            Key::UseColorProof => {
                ui.checkbox(&mut draft.use_color_proof, "");
            }
            Key::IccFilename => {
                ui.text_edit_singleline(&mut draft.icc_filename);
            }
            Key::IccIntent => {
                egui::ComboBox::from_id_salt("native_scene_icc_intent")
                    .selected_text(icc_intent_label(&draft.icc_intent))
                    .show_ui(ui, |ui| {
                        for (value, label) in [
                            ("perceptual", "perceptual"),
                            ("relative_colorimetric", "relative colorimetric"),
                            ("saturation", "saturation"),
                            ("absolute_colorimetric", "absolute colorimetric"),
                        ] {
                            ui.selectable_value(&mut draft.icc_intent, value.to_string(), label);
                        }
                    });
            }
            _ => {
                ui.monospace(scene_property_value(draft, definition.key));
            }
        }
    }

    fn apply_scene_properties(
        &mut self,
        draft: &NativeSceneProperties,
    ) -> std::result::Result<(), String> {
        let scene_uid = cuemol_core::Uid::new(draft.uid);
        if self.state.scene_manager().scene(scene_uid).is_none() {
            return Err(format!("scene {} was not found", draft.uid));
        }
        let background = draft.background.trim();
        if background.is_empty() {
            return Err("scene background color is required".to_string());
        }
        let runtime_background = SceneBackground::parse(background)?;
        if !draft.ao_radius.is_finite() || draft.ao_radius <= 0.0 {
            return Err("AO radius must be a positive finite number".to_string());
        }
        if !draft.ao_intensity.is_finite() || draft.ao_intensity <= 0.0 {
            return Err("AO intensity must be a positive finite number".to_string());
        }
        if !(1..=64).contains(&draft.ao_slices) {
            return Err("AO slices must be between 1 and 64".to_string());
        }
        if !(1..=32).contains(&draft.ao_steps) {
            return Err("AO steps must be between 1 and 32".to_string());
        }
        if !matches!(draft.aa_method.as_str(), "none" | "fxaa" | "smaa") {
            return Err(format!("unsupported AA method: {}", draft.aa_method));
        }
        if !(0..=5).contains(&draft.aa_jitter_level) {
            return Err("AA jitter level must be between 0 and 5".to_string());
        }
        if !matches!(
            draft.icc_intent.as_str(),
            "perceptual" | "relative_colorimetric" | "saturation" | "absolute_colorimetric"
        ) {
            return Err(format!("unsupported ICC intent: {}", draft.icc_intent));
        }

        let undo = self.capture_undo();
        let scene = self
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .expect("validated scene must remain live");
        set_native_scene_attr_default(scene, "bgcolor", background, "black");
        set_native_scene_attr_default(scene, "aoEnabled", &draft.ao_enabled.to_string(), "false");
        set_native_scene_attr_default(scene, "aoRadius", &draft.ao_radius.to_string(), "4");
        set_native_scene_attr_default(scene, "aoIntensity", &draft.ao_intensity.to_string(), "2.2");
        set_native_scene_attr_default(scene, "aoSlices", &draft.ao_slices.to_string(), "9");
        set_native_scene_attr_default(scene, "aoSteps", &draft.ao_steps.to_string(), "3");
        set_native_scene_attr_default(scene, "aoHalfRes", &draft.ao_half_res.to_string(), "false");
        set_native_scene_attr_default(scene, "aa_method", &draft.aa_method, "fxaa");
        set_native_scene_attr_default(
            scene,
            "aaJitterLevel",
            &draft.aa_jitter_level.to_string(),
            "0",
        );
        set_native_scene_attr_default(scene, "onload", &draft.onload, "");
        set_native_scene_attr_default(
            scene,
            "use_colproof",
            &draft.use_color_proof.to_string(),
            "false",
        );
        set_native_scene_attr_default(scene, "icc_filename", &draft.icc_filename, "");
        set_native_scene_attr_default(scene, "icc_intent", &draft.icc_intent, "perceptual");
        self.push_undo_snapshot(undo);
        self.modified_scenes.insert(scene_uid);
        if self.state.active_scene() == Some(scene_uid) {
            self.background = runtime_background;
            self.color_proofing = draft.use_color_proof;
            self.rebuild_snapshot();
        }
        Ok(())
    }

    fn sync_active_scene_properties(&mut self) {
        self.invalidate_molecule_panel_caches();
        let Some(scene) = self
            .state
            .active_scene()
            .and_then(|uid| self.state.scene_manager().scene(uid))
        else {
            self.background = SceneBackground::Black;
            self.color_proofing = false;
            return;
        };
        self.background = scene
            .scene_attribute("bgcolor")
            .and_then(|value| SceneBackground::parse(value).ok())
            .unwrap_or(SceneBackground::Black);
        self.color_proofing = scene_attr_bool(scene.scene_attribute("use_colproof"), false);
    }

    fn persist_scene_background(&mut self, value: &str) {
        let Some(scene_uid) = self.state.active_scene() else {
            self.set_status("No active scene");
            return;
        };
        let Ok(background) = SceneBackground::parse(value) else {
            self.set_status(format!("Invalid background color: {value}"));
            return;
        };
        let undo = self.capture_undo();
        let Some(scene) = self.state.scene_manager_mut().scene_mut(scene_uid) else {
            self.set_status("No active scene");
            return;
        };
        set_native_scene_attr_default(scene, "bgcolor", value, "black");
        self.push_undo_snapshot(undo);
        self.modified_scenes.insert(scene_uid);
        self.background = background;
        self.set_status(format!("Background set to {value}"));
    }

    fn persist_scene_color_proofing(&mut self) {
        let Some(scene_uid) = self.state.active_scene() else {
            self.set_status("No active scene");
            return;
        };
        let undo = self.capture_undo();
        let Some(scene) = self.state.scene_manager_mut().scene_mut(scene_uid) else {
            self.set_status("No active scene");
            return;
        };
        set_native_scene_attr_default(
            scene,
            "use_colproof",
            &self.color_proofing.to_string(),
            "false",
        );
        self.push_undo_snapshot(undo);
        self.modified_scenes.insert(scene_uid);
        self.rebuild_snapshot();
        self.set_status(format!("Color proofing: {}", self.color_proofing));
    }

    fn open_view_properties(&mut self, camera: &CameraController) {
        self.editing_saved_camera = None;
        self.view_properties_draft = Some(NativeViewProperties::from_camera(
            camera,
            self.viewport_rect,
        ));
        self.view_property_selected = 0;
        self.show_view_props = true;
    }

    fn camera_summary_for_selection(
        &self,
        selection: WorkspaceAuxSelection,
    ) -> Option<crate::GuiCameraSummary> {
        let WorkspaceAuxSelection::Camera {
            scene_uid,
            camera_index,
        } = selection
        else {
            return None;
        };
        self.state
            .camera_summaries(scene_uid)
            .into_iter()
            .nth(camera_index)
    }

    fn open_new_camera_properties(
        &mut self,
        scene_uid: cuemol_core::Uid,
        camera: &CameraController,
    ) {
        let Ok(name) = self.state.suggest_camera_name(scene_uid) else {
            self.set_status("Cannot create camera without a scene");
            return;
        };
        let mut draft = NativeViewProperties::from_camera(camera, self.viewport_rect);
        draft.name = name;
        self.editing_saved_camera = Some((scene_uid, String::new()));
        self.view_properties_draft = Some(draft);
        self.view_property_selected = 0;
        self.show_view_props = true;
    }

    fn open_camera_properties(&mut self, selection: WorkspaceAuxSelection) {
        let Some(summary) = self.camera_summary_for_selection(selection) else {
            self.set_status("Camera not found");
            return;
        };
        let Some(saved) = self
            .state
            .scene_manager()
            .scene(summary.scene_uid)
            .and_then(|scene| scene.camera_by_name(&summary.name))
        else {
            self.set_status("Camera not found");
            return;
        };
        let saved = CameraController::from_qsys_camera(saved);
        self.editing_saved_camera = Some((summary.scene_uid, summary.name));
        self.view_properties_draft = Some(NativeViewProperties::from_camera(
            &saved,
            self.viewport_rect,
        ));
        self.view_property_selected = 0;
        self.show_view_props = true;
    }

    fn open_camera_visibility_editor(&mut self, selection: WorkspaceAuxSelection) {
        let Some(summary) = self.camera_summary_for_selection(selection) else {
            self.set_status("Camera not found");
            return;
        };
        match self
            .state
            .camera_visibility_entries(summary.scene_uid, &summary.name)
        {
            Ok(entries) => {
                self.camera_visibility_draft = Some(NativeCameraVisibilityDraft {
                    scene_uid: summary.scene_uid,
                    camera_name: summary.name,
                    entries,
                });
            }
            Err(error) => self.set_status(format!("Camera visibility error: {error}")),
        }
    }

    fn clear_camera_visibility(&mut self, scene_uid: cuemol_core::Uid, name: &str) {
        let undo = self.capture_undo();
        match self
            .state
            .clear_scene_camera_visibility_settings(scene_uid, name)
        {
            Ok(count) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.set_status(format!("Clear visibility flags in {name}: {count} item(s)"));
            }
            Err(error) => self.set_status(format!("Camera visibility error: {error}")),
        }
    }

    fn load_workspace_camera(
        &mut self,
        selection: WorkspaceAuxSelection,
        camera: &mut CameraController,
    ) {
        self.load_workspace_camera_impl(selection, camera, true);
    }

    fn load_workspace_camera_view(
        &mut self,
        selection: WorkspaceAuxSelection,
        camera: &mut CameraController,
    ) {
        self.load_workspace_camera_impl(selection, camera, false);
    }

    fn load_workspace_camera_impl(
        &mut self,
        selection: WorkspaceAuxSelection,
        camera: &mut CameraController,
        load_visibility: bool,
    ) {
        let Some(summary) = self.camera_summary_for_selection(selection) else {
            self.set_status("Camera not found");
            return;
        };
        let undo = (load_visibility && summary.visibility_count > 0).then(|| self.capture_undo());
        match self
            .state
            .load_scene_camera(summary.scene_uid, &summary.name, load_visibility)
        {
            Ok(saved) => {
                *camera = CameraController::from_qsys_camera(&saved);
                if let Some(undo) = undo {
                    self.push_undo_snapshot(undo);
                    self.modified_scenes.insert(summary.scene_uid);
                    self.rebuild_snapshot();
                }
                self.set_status(format!("Loaded camera {}", summary.name));
            }
            Err(error) => self.set_status(format!("Camera load error: {error}")),
        }
    }

    fn save_workspace_camera_from_scene(
        &mut self,
        selection: WorkspaceAuxSelection,
        camera: &CameraController,
    ) {
        self.save_workspace_camera_impl(selection, camera, true);
    }

    fn save_workspace_camera_from_view(
        &mut self,
        selection: WorkspaceAuxSelection,
        camera: &CameraController,
    ) {
        self.save_workspace_camera_impl(selection, camera, false);
    }

    fn save_workspace_camera_impl(
        &mut self,
        selection: WorkspaceAuxSelection,
        camera: &CameraController,
        save_visibility: bool,
    ) {
        let Some(summary) = self.camera_summary_for_selection(selection) else {
            self.set_status("Camera not found");
            return;
        };
        let undo = self.capture_undo();
        match self.state.save_scene_camera(
            summary.scene_uid,
            camera.to_qsys_camera(&summary.name),
            save_visibility,
        ) {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(summary.scene_uid);
                self.set_status(format!("Saved camera {}", summary.name));
            }
            Err(error) => self.set_status(format!("Camera save error: {error}")),
        }
    }

    fn choose_load_workspace_camera_file(
        &mut self,
        scene_uid: cuemol_core::Uid,
        camera: &mut CameraController,
    ) {
        if self.state.active_scene() != Some(scene_uid) {
            self.activate_workspace_scene(scene_uid, camera);
        }
        if let Some(path) = native_file_dialog(NativeFileDialog::OpenCamera, "") {
            self.load_workspace_camera_file(scene_uid, &path, camera);
        }
    }

    fn load_workspace_camera_file(
        &mut self,
        scene_uid: cuemol_core::Uid,
        path: &Path,
        camera: &mut CameraController,
    ) {
        let undo = self.capture_undo();
        match self.state.load_scene_camera_file(scene_uid, path) {
            Ok(name) => match self.state.load_scene_camera(scene_uid, &name, true) {
                Ok(saved) => {
                    *camera = CameraController::from_qsys_camera(&saved);
                    self.push_undo_snapshot(undo);
                    self.modified_scenes.insert(scene_uid);
                    self.rebuild_snapshot();
                    let camera_index = self
                        .state
                        .camera_summaries(scene_uid)
                        .iter()
                        .position(|summary| summary.name == name)
                        .expect("the loaded camera was just inserted");
                    self.selected_object = None;
                    self.selected_renderer = None;
                    self.workspace_aux_selection = Some(WorkspaceAuxSelection::Camera {
                        scene_uid,
                        camera_index,
                    });
                    self.set_status(format!("Load camera file {name}"));
                }
                Err(error) => self.set_status(format!("Camera load error: {error}")),
            },
            Err(error) => self.set_status(format!("Camera file error: {error}")),
        }
    }

    fn reload_workspace_camera_file(&mut self, selection: WorkspaceAuxSelection) {
        let Some(summary) = self.camera_summary_for_selection(selection) else {
            self.set_status("Camera not found");
            return;
        };
        let undo = self.capture_undo();
        match self
            .state
            .reload_scene_camera_file(summary.scene_uid, &summary.name)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(summary.scene_uid);
                self.set_status(format!("Reload camera file {}", summary.name));
            }
            Err(error) => self.set_status(format!("Camera file error: {error}")),
        }
    }

    fn choose_save_workspace_camera_file(
        &mut self,
        selection: WorkspaceAuxSelection,
        save_as: bool,
    ) {
        let Some(summary) = self.camera_summary_for_selection(selection) else {
            self.set_status("Camera not found");
            return;
        };
        let path = if !save_as && !summary.source.is_empty() {
            Some(PathBuf::from(&summary.source))
        } else {
            native_file_dialog(
                NativeFileDialog::SaveCamera,
                &format!("{}.cam", summary.name),
            )
        };
        let Some(path) = path else {
            return;
        };
        let undo = self.capture_undo();
        match self
            .state
            .save_scene_camera_file(summary.scene_uid, &summary.name, &path)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(summary.scene_uid);
                self.set_status(format!("Save camera file {}", summary.name));
            }
            Err(error) => self.set_status(format!("Camera file error: {error}")),
        }
    }

    fn copy_workspace_camera_to_clipboard(&mut self, scene_uid: cuemol_core::Uid, name: &str) {
        match self.state.copy_workspace_camera(scene_uid, name) {
            Ok(clipboard) => {
                let name = clipboard.name().to_string();
                self.workspace_camera_clipboard = Some(clipboard);
                self.set_status(format!("Copied camera {name}"));
            }
            Err(error) => self.set_status(format!("Copy camera error: {error}")),
        }
    }

    fn paste_workspace_camera_clipboard(&mut self, scene_uid: cuemol_core::Uid) {
        let Some(clipboard) = self.workspace_camera_clipboard.clone() else {
            self.set_status("No copied camera");
            return;
        };
        let undo = self.capture_undo();
        match self.state.paste_workspace_camera(scene_uid, &clipboard) {
            Ok(name) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                if self.state.active_scene() == Some(scene_uid) {
                    let camera_index = self
                        .state
                        .camera_summaries(scene_uid)
                        .iter()
                        .position(|camera| camera.name == name)
                        .expect("the pasted camera was just inserted");
                    self.selected_object = None;
                    self.selected_renderer = None;
                    self.workspace_aux_selection = Some(WorkspaceAuxSelection::Camera {
                        scene_uid,
                        camera_index,
                    });
                }
                self.set_status(format!("Pasted camera {name}"));
            }
            Err(error) => self.set_status(format!("Paste camera error: {error}")),
        }
    }

    fn style_summary_for_selection(
        &self,
        selection: WorkspaceAuxSelection,
    ) -> Option<crate::GuiStyleSummary> {
        let WorkspaceAuxSelection::Style {
            scene_uid,
            style_index,
        } = selection
        else {
            return None;
        };
        self.state
            .style_summaries(scene_uid)
            .into_iter()
            .nth(style_index)
    }

    fn copy_workspace_style_to_clipboard(
        &mut self,
        scene_uid: cuemol_core::Uid,
        local_index: usize,
    ) {
        match self.state.copy_workspace_style(scene_uid, local_index) {
            Ok(clipboard) => {
                let name = clipboard.name();
                self.workspace_style_clipboard = Some(clipboard);
                self.set_status(format!("Copied style {name}"));
            }
            Err(error) => self.set_status(format!("Copy style error: {error}")),
        }
    }

    fn request_paste_workspace_style(&mut self, scene_uid: cuemol_core::Uid) {
        let Some(clipboard) = self.workspace_style_clipboard.as_ref() else {
            self.set_status("No copied style");
            return;
        };
        if self
            .state
            .workspace_style_exists(scene_uid, &clipboard.name())
        {
            self.pending_style_paste_scene = Some(scene_uid);
        } else {
            self.paste_workspace_style_clipboard(scene_uid, false);
        }
    }

    fn choose_load_workspace_style_file(
        &mut self,
        scene_uid: cuemol_core::Uid,
        camera: &mut CameraController,
    ) {
        if self.state.active_scene() != Some(scene_uid) {
            self.activate_workspace_scene(scene_uid, camera);
        }
        if let Some(path) = native_file_dialog(NativeFileDialog::OpenStyle, "") {
            let undo = self.capture_undo();
            match self.state.load_scene_style_file(scene_uid, &path, true) {
                Ok(local_index) => {
                    self.push_undo_snapshot(undo);
                    self.modified_scenes.insert(scene_uid);
                    self.rebuild_snapshot();
                    let style_index = self
                        .state
                        .style_summaries(scene_uid)
                        .iter()
                        .position(|style| style.local_index == Some(local_index))
                        .expect("the loaded style was just inserted");
                    self.selected_object = None;
                    self.selected_renderer = None;
                    self.workspace_aux_selection = Some(WorkspaceAuxSelection::Style {
                        scene_uid,
                        style_index,
                    });
                    self.set_status(format!("Loaded style file {}", path.display()));
                }
                Err(error) => self.set_status(format!("Style file error: {error}")),
            }
        }
    }

    fn choose_save_workspace_style_file(
        &mut self,
        selection: WorkspaceAuxSelection,
        save_as: bool,
    ) {
        let Some(summary) = self.style_summary_for_selection(selection) else {
            self.set_status("Style not found");
            return;
        };
        let Some(local_index) = summary.local_index else {
            self.set_status("Global styles cannot be saved here");
            return;
        };
        let path = if !save_as && !summary.source.is_empty() {
            Some(PathBuf::from(&summary.source))
        } else {
            let base_name = if summary.name == "(anonymous)" {
                "style"
            } else {
                &summary.name
            };
            native_file_dialog(NativeFileDialog::SaveStyle, &format!("{base_name}.xml"))
        };
        let Some(path) = path else {
            return;
        };
        let undo = self.capture_undo();
        match self
            .state
            .save_scene_style_file(summary.scene_uid, local_index, &path)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(summary.scene_uid);
                self.set_status(format!("Saved style file {}", path.display()));
            }
            Err(error) => self.set_status(format!("Style file error: {error}")),
        }
    }

    fn paste_workspace_style_clipboard(&mut self, scene_uid: cuemol_core::Uid, replace: bool) {
        let Some(clipboard) = self.workspace_style_clipboard.clone() else {
            self.set_status("No copied style");
            return;
        };
        let undo = self.capture_undo();
        match self
            .state
            .paste_workspace_style(scene_uid, &clipboard, replace)
        {
            Ok(local_index) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                let style_index = self
                    .state
                    .style_summaries(scene_uid)
                    .iter()
                    .position(|style| style.local_index == Some(local_index))
                    .expect("the pasted style was just inserted");
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = Some(WorkspaceAuxSelection::Style {
                    scene_uid,
                    style_index,
                });
                self.rebuild_snapshot();
                self.set_status(format!("Pasted style {}", clipboard.name()));
            }
            Err(error) => self.set_status(format!("Paste style error: {error}")),
        }
    }

    fn open_new_style_properties(&mut self, scene_uid: cuemol_core::Uid) {
        let Ok(name) = self.state.suggest_style_name(scene_uid) else {
            self.set_status("Cannot create style without a scene");
            return;
        };
        self.style_properties_draft = Some(NativeStyleProperties {
            scene_uid,
            local_index: None,
            xml: format!("<styles id=\"{name}\">\n</styles>\n"),
            name,
            read_only: false,
        });
    }

    fn open_renderer_style_create(
        &mut self,
        scene_uid: cuemol_core::Uid,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) {
        let Some(renderer) = self
            .state
            .scene_manager()
            .scene(scene_uid)
            .and_then(|scene| scene.object(object_uid))
            .and_then(|object| object.renderer(renderer_uid))
        else {
            self.set_status("Renderer not found");
            return;
        };
        if renderer.renderer_type() == "*group" {
            self.set_status("Renderer groups cannot be saved as styles");
            return;
        }
        let style_set_index = self
            .state
            .style_summaries(scene_uid)
            .into_iter()
            .filter(|style| !style.read_only)
            .filter_map(|style| style.local_index)
            .next_back();
        self.renderer_style_create_draft = Some(NativeRendererStyleCreate {
            scene_uid,
            object_uid,
            renderer_uid,
            renderer_name: renderer.name().to_string(),
            renderer_type: renderer.renderer_type().to_string(),
            style_set_index,
            name_prefix: String::new(),
            overwrite: false,
        });
    }

    fn apply_renderer_style_create(
        &mut self,
        draft: &NativeRendererStyleCreate,
    ) -> std::result::Result<String, String> {
        let local_index = draft
            .style_set_index
            .ok_or_else(|| "no writable style set is available".to_string())?;
        let undo = self.capture_undo();
        let style_name = self
            .state
            .create_scene_renderer_style(
                draft.scene_uid,
                local_index,
                draft.object_uid,
                draft.renderer_uid,
                &draft.name_prefix,
                draft.overwrite,
            )
            .map_err(|error| error.to_string())?;
        self.push_undo_snapshot(undo);
        self.modified_scenes.insert(draft.scene_uid);
        self.rebuild_snapshot();
        Ok(style_name)
    }

    fn renderer_style_create_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.renderer_style_create_draft.take() else {
            return;
        };
        let style_sets = self
            .state
            .style_summaries(draft.scene_uid)
            .into_iter()
            .filter(|style| !style.read_only)
            .filter_map(|style| style.local_index.map(|index| (index, style.name)))
            .collect::<Vec<_>>();
        if draft
            .style_set_index
            .is_some_and(|index| !style_sets.iter().any(|(candidate, _)| *candidate == index))
        {
            draft.style_set_index = style_sets.last().map(|(index, _)| *index);
            draft.overwrite = false;
        }
        let style_name = format!("{}{}", draft.name_prefix.trim(), draft.renderer_type);
        let style_exists = draft.style_set_index.is_some_and(|local_index| {
            self.state
                .scene_manager()
                .scene(draft.scene_uid)
                .and_then(|scene| scene.loaded_styles().get(local_index))
                .is_some_and(|style| {
                    style.node.children().iter().any(|child| {
                        child.tag_name() == "style"
                            && child
                                .str_attr("id")
                                .as_str()
                                .eq_ignore_ascii_case(&style_name)
                    })
                })
        });
        let mut open = true;
        let mut create = false;
        let mut cancel = false;
        egui::Window::new("Create Renderer Style")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .default_height(190.0)
            .show(ctx, |ui| {
                egui::Grid::new("renderer_style_create_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Original rend:");
                        ui.label(format!("{} ({})", draft.renderer_name, draft.renderer_type));
                        ui.end_row();

                        ui.label("Target Style set:");
                        let selected_name = draft
                            .style_set_index
                            .and_then(|index| {
                                style_sets.iter().find(|(candidate, _)| *candidate == index)
                            })
                            .map(|(_, name)| name.as_str())
                            .unwrap_or("(none)");
                        let before = draft.style_set_index;
                        egui::ComboBox::from_id_salt("renderer_style_target_set")
                            .selected_text(selected_name)
                            .show_ui(ui, |ui| {
                                for (index, name) in &style_sets {
                                    ui.selectable_value(
                                        &mut draft.style_set_index,
                                        Some(*index),
                                        name,
                                    );
                                }
                            });
                        if draft.style_set_index != before {
                            draft.overwrite = false;
                        }
                        ui.end_row();

                        ui.label("Style name:");
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(&mut draft.name_prefix).changed() {
                                draft.overwrite = false;
                            }
                            ui.label(&draft.renderer_type);
                        });
                        ui.end_row();
                    });
                if style_sets.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(180, 70, 50),
                        "Create a writable scene Style set first.",
                    );
                } else if style_exists {
                    ui.checkbox(
                        &mut draft.overwrite,
                        format!("Overwrite existing style {style_name}"),
                    );
                }
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let create_enabled = draft.style_set_index.is_some()
                        && !draft.name_prefix.trim().is_empty()
                        && (!style_exists || draft.overwrite);
                    if ui
                        .add_enabled(create_enabled, egui::Button::new("Create"))
                        .clicked()
                    {
                        create = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if create {
            match self.apply_renderer_style_create(&draft) {
                Ok(style_name) => self.set_status(format!("Created style {style_name}")),
                Err(error) => {
                    self.set_status(format!("Style error: {error}"));
                    self.renderer_style_create_draft = Some(draft);
                }
            }
        } else if !cancel && open {
            self.renderer_style_create_draft = Some(draft);
        }
    }

    fn open_style_properties(&mut self, selection: WorkspaceAuxSelection) {
        let Some(summary) = self.style_summary_for_selection(selection) else {
            self.set_status("Style not found");
            return;
        };
        let Some(local_index) = summary.local_index else {
            self.set_status("Global styles are read-only");
            return;
        };
        let Some(style) = self
            .state
            .scene_manager()
            .scene(summary.scene_uid)
            .and_then(|scene| scene.loaded_styles().get(local_index))
        else {
            self.set_status("Style not found");
            return;
        };
        self.style_properties_draft = Some(NativeStyleProperties {
            scene_uid: summary.scene_uid,
            local_index: Some(local_index),
            name: summary.name,
            read_only: summary.read_only,
            xml: style.data.clone(),
        });
    }

    fn set_workspace_style_read_only(&mut self, selection: WorkspaceAuxSelection, read_only: bool) {
        let Some(summary) = self.style_summary_for_selection(selection) else {
            self.set_status("Style not found");
            return;
        };
        let Some(local_index) = summary.local_index else {
            self.set_status("Global styles are read-only");
            return;
        };
        if read_only && summary.modified {
            self.set_status("Modified styles cannot be made read-only");
            return;
        }
        let Some(xml) = self
            .state
            .scene_manager()
            .scene(summary.scene_uid)
            .and_then(|scene| scene.loaded_styles().get(local_index))
            .map(|style| style.data.clone())
        else {
            self.set_status("Style not found");
            return;
        };
        let undo = self.capture_undo();
        match self.state.update_scene_style(
            summary.scene_uid,
            local_index,
            &summary.name,
            read_only,
            &xml,
        ) {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(summary.scene_uid);
                self.rebuild_snapshot();
                self.set_status(format!("Style read-only: {read_only}"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn style_props_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.style_properties_draft.take() else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Style editor")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(640.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut draft.name);
                    ui.checkbox(&mut draft.read_only, "Read only");
                });
                ui.label("Style XML");
                ui.add_sized(
                    [ui.available_width(), 320.0],
                    egui::TextEdit::multiline(&mut draft.xml)
                        .font(egui::TextStyle::Monospace)
                        .code_editor(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        accept = true;
                    }
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if accept || apply {
            let undo = self.capture_undo();
            let result = (|| {
                let local_index = match draft.local_index {
                    Some(index) => index,
                    None => self
                        .state
                        .create_scene_style(draft.scene_uid, &draft.name)
                        .map_err(|error| error.to_string())?,
                };
                self.state
                    .update_scene_style(
                        draft.scene_uid,
                        local_index,
                        &draft.name,
                        draft.read_only,
                        &draft.xml,
                    )
                    .map_err(|error| error.to_string())?;
                Ok::<usize, String>(local_index)
            })();
            match result {
                Ok(local_index) => {
                    draft.local_index = Some(local_index);
                    self.push_undo_snapshot(undo);
                    self.modified_scenes.insert(draft.scene_uid);
                    let style_index = self
                        .state
                        .style_summaries(draft.scene_uid)
                        .iter()
                        .position(|style| style.local_index == Some(local_index))
                        .unwrap_or(0);
                    self.workspace_aux_selection = Some(WorkspaceAuxSelection::Style {
                        scene_uid: draft.scene_uid,
                        style_index,
                    });
                    self.rebuild_snapshot();
                    self.set_status(format!("Saved style {}", draft.name.trim()));
                    if !accept {
                        self.style_properties_draft = Some(draft);
                    }
                }
                Err(error) => {
                    self.state = undo.state;
                    self.set_status(format!("Style error: {error}"));
                    self.style_properties_draft = Some(draft);
                }
            }
        } else if !cancel && open {
            self.style_properties_draft = Some(draft);
        }
    }

    fn view_props_dialog(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        if !self.show_view_props {
            return;
        }
        let mut draft = self
            .view_properties_draft
            .take()
            .unwrap_or_else(|| NativeViewProperties::from_camera(camera, self.viewport_rect));
        let mut selected = self
            .view_property_selected
            .min(VIEW_PROPERTY_DEFINITIONS.len().saturating_sub(1));
        let mut open = self.show_view_props;
        let mut apply = false;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Object properties")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(640.0)
            .min_width(560.0)
            .show(ctx, |ui| {
                let _ = ui.selectable_label(true, "Generic");
                ui.separator();
                Self::view_property_editor_ui(ui, &mut draft, &mut selected);
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        accept = true;
                    }
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        self.view_property_selected = selected;
        if accept || apply {
            let update_result =
                if let Some((scene_uid, old_name)) = self.editing_saved_camera.clone() {
                    (|| {
                        let new_name = draft.name.trim().to_string();
                        let duplicate = new_name != old_name
                            && self
                                .state
                                .scene_manager()
                                .scene(scene_uid)
                                .is_some_and(|scene| scene.camera_by_name(&new_name).is_some());
                        if new_name.is_empty() {
                            return Err("camera name is required".to_string());
                        }
                        if duplicate {
                            return Err(format!("camera already exists: {new_name}"));
                        }
                        let mut target = if old_name.is_empty() {
                            camera.clone()
                        } else {
                            self.state
                                .scene_manager()
                                .scene(scene_uid)
                                .and_then(|scene| scene.camera_by_name(&old_name))
                                .map(CameraController::from_qsys_camera)
                                .ok_or_else(|| format!("camera not found: {old_name}"))?
                        };
                        Self::apply_view_properties(&mut target, &draft)?;
                        let mut saved = target.to_qsys_camera(&new_name);
                        if !old_name.is_empty() {
                            if let Some(original) = self
                                .state
                                .scene_manager()
                                .scene(scene_uid)
                                .and_then(|scene| scene.camera_by_name(&old_name))
                            {
                                saved.set_source(original.source());
                                saved.set_alt_source(original.alt_source());
                                for (uid, visible, is_object) in original.visibility_settings() {
                                    saved.vis_append(uid, visible, is_object);
                                }
                            }
                        }
                        let undo = self.capture_undo();
                        if !old_name.is_empty() && old_name != new_name {
                            self.state
                                .destroy_scene_camera(scene_uid, &old_name)
                                .map_err(|error| error.to_string())?;
                        }
                        self.state
                            .save_scene_camera(scene_uid, saved, false)
                            .map_err(|error| error.to_string())?;
                        self.push_undo_snapshot(undo);
                        self.modified_scenes.insert(scene_uid);
                        let camera_index = self
                            .state
                            .camera_summaries(scene_uid)
                            .iter()
                            .position(|summary| summary.name == new_name)
                            .unwrap_or(0);
                        self.workspace_aux_selection = Some(WorkspaceAuxSelection::Camera {
                            scene_uid,
                            camera_index,
                        });
                        self.editing_saved_camera = Some((scene_uid, new_name.clone()));
                        self.set_status(format!("Saved camera {new_name}"));
                        Ok(())
                    })()
                } else {
                    Self::apply_view_properties(camera, &draft)
                };
            match update_result {
                Ok(()) => {
                    if self.editing_saved_camera.is_none() {
                        self.set_status("View properties updated");
                    }
                    if accept {
                        self.show_view_props = false;
                        self.view_properties_draft = None;
                        self.editing_saved_camera = None;
                    } else {
                        self.show_view_props = true;
                        self.view_properties_draft = Some(draft);
                    }
                }
                Err(error) => {
                    self.set_status(format!("View properties error: {error}"));
                    self.show_view_props = true;
                    self.view_properties_draft = Some(draft);
                }
            }
        } else if cancel || !open {
            self.show_view_props = false;
            self.view_properties_draft = None;
            self.editing_saved_camera = None;
        } else {
            self.show_view_props = true;
            self.view_properties_draft = Some(draft);
        }
    }

    fn view_property_editor_ui(
        ui: &mut egui::Ui,
        draft: &mut NativeViewProperties,
        selected: &mut usize,
    ) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        egui::Grid::new("native_view_property_table")
                            .striped(true)
                            .num_columns(4)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                for heading in ["Name", "R", "Type", "Value"] {
                                    ui.strong(heading);
                                }
                                ui.end_row();
                                for (index, definition) in
                                    VIEW_PROPERTY_DEFINITIONS.iter().enumerate()
                                {
                                    if ui
                                        .selectable_label(*selected == index, definition.name)
                                        .clicked()
                                    {
                                        *selected = index;
                                    }
                                    ui.label(if definition.readonly { "R" } else { "" });
                                    ui.label(definition.type_name);
                                    ui.monospace(view_property_value(draft, definition.key));
                                    ui.end_row();
                                }
                            });
                    });
            });
        ui.add_space(8.0);
        let definition = VIEW_PROPERTY_DEFINITIONS[*selected];
        egui::Grid::new("native_view_property_detail")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Name:");
                ui.monospace(definition.name);
                ui.end_row();
                ui.label("Type:");
                ui.monospace(definition.type_name);
                ui.end_row();
                ui.label("Value:");
                ui.horizontal(|ui| {
                    let mut is_default = view_property_is_default(draft, definition.key);
                    if ui
                        .add_enabled(
                            definition.has_default && !definition.readonly,
                            egui::Checkbox::new(&mut is_default, "default"),
                        )
                        .changed()
                        && is_default
                    {
                        reset_view_property(draft, definition.key);
                    }
                    Self::view_property_control_ui(ui, draft, definition);
                });
                ui.end_row();
            });
    }

    fn view_property_control_ui(
        ui: &mut egui::Ui,
        draft: &mut NativeViewProperties,
        definition: ViewPropertyDefinition,
    ) {
        use ViewPropertyKey as Key;
        match definition.key {
            Key::Name => {
                ui.text_edit_singleline(&mut draft.name);
            }
            Key::Zoom => {
                ui.add(egui::DragValue::new(&mut draft.zoom).speed(1.0));
            }
            Key::Distance => {
                ui.add(egui::DragValue::new(&mut draft.distance).speed(1.0));
            }
            Key::Slab => {
                ui.add(egui::DragValue::new(&mut draft.slab).speed(1.0));
            }
            Key::CenterX | Key::CenterY | Key::CenterZ => {
                let index = match definition.key {
                    Key::CenterX => 0,
                    Key::CenterY => 1,
                    _ => 2,
                };
                ui.add(egui::DragValue::new(&mut draft.center[index]).speed(0.1));
            }
            Key::RotationX | Key::RotationY | Key::RotationZ | Key::RotationW => {
                let index = match definition.key {
                    Key::RotationX => 0,
                    Key::RotationY => 1,
                    Key::RotationZ => 2,
                    _ => 3,
                };
                ui.add(egui::DragValue::new(&mut draft.rotation[index]).speed(0.01));
            }
            Key::Perspective => {
                ui.checkbox(&mut draft.perspective, "");
            }
            Key::TransMms => {
                ui.checkbox(&mut draft.trans_mms, "");
            }
            Key::RotMms => {
                ui.checkbox(&mut draft.rot_mms, "");
            }
            Key::StereoMode => {
                egui::ComboBox::from_id_salt("native_view_stereo_mode")
                    .selected_text(stereo_mode_name(draft.stereo_mode))
                    .show_ui(ui, |ui| {
                        for (mode, name) in [
                            (StereoMode::None, "none"),
                            (StereoMode::Parallel, "para"),
                            (StereoMode::Cross, "cross"),
                            (StereoMode::Hardware, "hardware"),
                        ] {
                            ui.selectable_value(&mut draft.stereo_mode, mode, name);
                        }
                    });
            }
            Key::StereoDist => {
                ui.add(egui::DragValue::new(&mut draft.stereo_dist).speed(0.1));
            }
            Key::SwapStereoEyes => {
                ui.checkbox(&mut draft.swap_stereo_eyes, "");
            }
            Key::CenterMark => {
                egui::ComboBox::from_id_salt("native_view_center_mark")
                    .selected_text(center_mark_name(draft.center_mark))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.center_mark, CenterMark::None, "none");
                        ui.selectable_value(&mut draft.center_mark, CenterMark::Cross, "crosshair");
                        ui.selectable_value(&mut draft.center_mark, CenterMark::Axis, "axis");
                    });
            }
            _ => {
                ui.monospace(view_property_value(draft, definition.key));
            }
        }
    }

    fn apply_view_properties(
        camera: &mut CameraController,
        draft: &NativeViewProperties,
    ) -> std::result::Result<(), String> {
        if !draft.zoom.is_finite() || draft.zoom <= 0.0 {
            return Err("view zoom must be a positive finite number".to_string());
        }
        if !draft.distance.is_finite() || draft.distance <= 0.0 {
            return Err("view distance must be a positive finite number".to_string());
        }
        if !draft.slab.is_finite() || draft.slab <= 0.0 {
            return Err("view slab must be a positive finite number".to_string());
        }
        if !draft.stereo_dist.is_finite() || draft.stereo_dist < 0.0 {
            return Err("stereo distance must be a non-negative finite number".to_string());
        }
        if !draft.center.iter().all(|value| value.is_finite())
            || !draft.rotation.iter().all(|value| value.is_finite())
        {
            return Err("view vectors must contain finite numbers".to_string());
        }
        let rotation = Quat::from_xyzw(
            draft.rotation[0] as f32,
            draft.rotation[1] as f32,
            draft.rotation[2] as f32,
            draft.rotation[3] as f32,
        );
        if !rotation.is_finite() || rotation.length_squared() <= f32::EPSILON {
            return Err("view rotation must be a non-zero finite quaternion".to_string());
        }
        camera.name = draft.name.clone();
        camera.ortho_scale = (1000.0 / draft.zoom) as f32;
        camera.distance = draft.distance as f32;
        camera.slab_depth = draft.slab as f32;
        camera.target = Vec3::new(
            draft.center[0] as f32,
            draft.center[1] as f32,
            draft.center[2] as f32,
        );
        camera.orientation = rotation.normalize();
        camera.perspective = draft.perspective;
        camera.trans_mms = draft.trans_mms;
        camera.rot_mms = draft.rot_mms;
        camera.stereo_mode = draft.stereo_mode;
        camera.stereo_distance = draft.stereo_dist as f32;
        camera.swap_stereo_eyes = draft.swap_stereo_eyes;
        camera.center_mark = draft.center_mark;
        Ok(())
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
                if let Some(owner) = self.new_renderer_owner {
                    let (kind, uid) = match owner {
                        NativePropertyObject::Molecule(uid) => ("molecule", uid),
                        NativePropertyObject::DensityMap(uid) => ("density map", uid),
                        NativePropertyObject::Surface(uid) => ("surface", uid),
                    };
                    ui.monospace(format!("{kind} uid: {}", uid.get()));
                }
                if let Some(group) = self.new_renderer_group.as_deref() {
                    ui.label(format!("Group: {group}"));
                }
                let renderer_types = match self.new_renderer_owner {
                    Some(NativePropertyObject::DensityMap(_)) => DENSITY_RENDERER_TYPES,
                    Some(NativePropertyObject::Surface(_)) => SURFACE_RENDERER_TYPES,
                    Some(NativePropertyObject::Molecule(_)) | None => RENDERER_TYPES,
                };
                if self.new_renderer_type == "*group" {
                    ui.label("Type: Renderer group");
                } else {
                    egui::ComboBox::from_label("Type")
                        .selected_text(renderer_type_label(&self.new_renderer_type))
                        .show_ui(ui, |ui| {
                            for renderer_type in renderer_types {
                                ui.selectable_value(
                                    &mut self.new_renderer_type,
                                    (*renderer_type).to_string(),
                                    *renderer_type,
                                );
                            }
                        });
                }
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

    fn tool_dialog(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
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
                    "Molecular superposition" => self.superpose_tool_ui(ui, camera),
                    "Mol bond editor" => self.bond_editor_tool_ui(ui),
                    "Interaction" => self.interaction_tool_ui(ui),
                    "Change symmetry" => self.crystal_tool_ui(ui),
                    "Reassign secondary str" => self.secondary_structure_tool_ui(ui),
                    "Mol morphing animation" => self.morph_tool_ui(ui),
                    "Mol surface generation" => self.surface_tool_ui(ui),
                    "Mol surface cutter" => self.surface_cutter_tool_ui(ui, camera),
                    "APBS elepot calculation" => self.apbs_tool_ui(ui),
                    "About plugins" => self.plugin_tool_ui(ui),
                    "About config" => self.config_tool_ui(ui),
                    "Addon manager" => self.addon_tool_ui(ui),
                    "POV-Ray rendering" => self.render_tab(ui),
                    "Animation rendering" => self.animation_render_tool_ui(ui, camera),
                    _ => {
                        ui.label(tool_status_text(&title));
                    }
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        if !open || close {
            if title == "Mol bond editor" {
                self.bond_picker_target = None;
            }
            self.tool_dialog = None;
        }
    }

    fn superpose_tool_ui(&mut self, ui: &mut egui::Ui, camera: &mut CameraController) {
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
        egui::ComboBox::from_label("Algorithm")
            .selected_text(match self.superpose_algorithm.as_str() {
                "ssm" => "Secondary Structure Matching",
                _ => "Least Square Fitting",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.superpose_algorithm,
                    "lsq".to_string(),
                    "Least Square Fitting",
                );
                ui.selectable_value(
                    &mut self.superpose_algorithm,
                    "ssm".to_string(),
                    "Secondary Structure Matching",
                );
            });
        egui::ComboBox::from_label("Reference molecule")
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
        egui::ComboBox::from_label("Moving molecule")
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
        ui.checkbox(&mut self.superpose_auto_recenter, "Auto recenter");
        ui.checkbox(&mut self.superpose_use_property, "Use xformMat property");
        if ui.button("Superpose").clicked() {
            self.apply_superpose(camera);
        }
    }

    fn bond_editor_tool_ui(&mut self, ui: &mut egui::Ui) {
        let objects = self.state.active_object_summaries();
        if !objects
            .iter()
            .any(|object| Some(object.uid) == self.bond_object)
        {
            self.bond_object = self
                .target_object_uid()
                .filter(|uid| objects.iter().any(|object| object.uid == *uid))
                .or_else(|| objects.first().map(|object| object.uid));
            self.selected_nonstandard_bonds.clear();
        }
        let before = self.bond_object;
        egui::ComboBox::from_label("Mol name")
            .selected_text(object_combo_label(&objects, self.bond_object))
            .show_ui(ui, |ui| {
                for object in &objects {
                    ui.selectable_value(
                        &mut self.bond_object,
                        Some(object.uid),
                        object.name.as_str(),
                    );
                }
            });
        if self.bond_object != before {
            self.selected_nonstandard_bonds.clear();
        }

        let bonds = self
            .bond_object
            .and_then(|uid| self.state.nonstandard_bond_summaries(uid).ok())
            .unwrap_or_default();
        egui::Frame::new()
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .min_scrolled_height(96.0)
                    .max_height(220.0)
                    .show(ui, |ui| {
                        if bonds.is_empty() {
                            ui.weak("No non-standard bonds.");
                        }
                        for bond in &bonds {
                            let pair = (
                                bond.atom1.serial.min(bond.atom2.serial),
                                bond.atom1.serial.max(bond.atom2.serial),
                            );
                            let mut selected = self.selected_nonstandard_bonds.contains(&pair);
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut selected, "").changed() {
                                    if selected {
                                        self.selected_nonstandard_bonds.insert(pair);
                                    } else {
                                        self.selected_nonstandard_bonds.remove(&pair);
                                    }
                                }
                                ui.monospace(format_nonstandard_bond_atom(&bond.atom1));
                                ui.label("—");
                                ui.monospace(format_nonstandard_bond_atom(&bond.atom2));
                            });
                        }
                    });
            });
        if ui
            .add_enabled(
                !self.selected_nonstandard_bonds.is_empty(),
                egui::Button::new("Delete selected"),
            )
            .clicked()
        {
            self.remove_selected_nonstandard_bonds();
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Atom serials");
            let atom1 = ui.add_sized(
                [80.0, 22.0],
                egui::TextEdit::singleline(&mut self.bond_aid1),
            );
            if atom1.gained_focus() {
                self.bond_picker_target = Some(0);
            }
            let atom2 = ui.add_sized(
                [80.0, 22.0],
                egui::TextEdit::singleline(&mut self.bond_aid2),
            );
            if atom2.gained_focus() {
                self.bond_picker_target = Some(1);
            }
        });
        ui.weak("Focus a field, then click an atom in the viewport.");
        ui.horizontal(|ui| {
            if ui.button("Add").clicked() {
                self.edit_bond(true);
            }
        });
    }

    fn interaction_tool_ui(&mut self, ui: &mut egui::Ui) {
        let objects = self.state.active_object_summaries();
        if !objects
            .iter()
            .any(|object| Some(object.uid) == self.contact_object)
        {
            self.contact_object = self
                .target_object_uid()
                .filter(|uid| objects.iter().any(|object| object.uid == *uid))
                .or_else(|| objects.first().map(|object| object.uid));
        }
        if !objects
            .iter()
            .any(|object| Some(object.uid) == self.contact_object2)
        {
            self.contact_object2 = objects
                .iter()
                .find(|object| Some(object.uid) != self.contact_object)
                .or_else(|| objects.first())
                .map(|object| object.uid);
        }
        egui::ComboBox::from_label("Molecule")
            .selected_text(object_combo_label(&objects, self.contact_object))
            .show_ui(ui, |ui| {
                for object in &objects {
                    ui.selectable_value(
                        &mut self.contact_object,
                        Some(object.uid),
                        object.name.as_str(),
                    );
                }
            });
        ui.horizontal(|ui| {
            ui.label("Selection 1");
            ui.add_sized(
                [ui.available_width(), 22.0],
                egui::TextEdit::singleline(&mut self.contact_selection1),
            );
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.contact_use_object2, "Mol 2:");
            ui.add_enabled_ui(self.contact_use_object2, |ui| {
                egui::ComboBox::from_id_salt("native_contact_object2")
                    .selected_text(object_combo_label(&objects, self.contact_object2))
                    .show_ui(ui, |ui| {
                        for object in &objects {
                            ui.selectable_value(
                                &mut self.contact_object2,
                                Some(object.uid),
                                object.name.as_str(),
                            );
                        }
                    });
            });
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.contact_use_selection2, "Sel 2:");
            ui.add_enabled(
                self.contact_use_selection2,
                egui::TextEdit::singleline(&mut self.contact_selection2)
                    .desired_width(ui.available_width()),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Distance");
            ui.add(egui::DragValue::new(&mut self.contact_r_min).speed(0.1));
            ui.add(egui::DragValue::new(&mut self.contact_r_max).speed(0.1));
        });
        ui.horizontal(|ui| {
            ui.label("Max labels");
            ui.add(egui::DragValue::new(&mut self.contact_max_count).range(1..=10_000));
        });
        ui.checkbox(&mut self.contact_hbond, "Hydrogen bond (N, O) only");
        ui.horizontal(|ui| {
            ui.label("Renderer");
            ui.text_edit_singleline(&mut self.contact_renderer_name);
        });
        if ui.button("OK").clicked() {
            self.calculate_contacts();
        }
    }

    fn secondary_structure_tool_ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.secondary_ignore_breaks, "Ignore β bulge");
        ui.checkbox(&mut self.secondary_gap_fill, "Helix gap-fill angle (°)");
        ui.horizontal(|ui| {
            ui.label("Gap-fill angle");
            ui.add_enabled(
                self.secondary_gap_fill,
                egui::DragValue::new(&mut self.secondary_angle)
                    .speed(5.0)
                    .range(0.0..=180.0),
            );
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

    fn surface_cutter_tool_ui(&mut self, ui: &mut egui::Ui, camera: &CameraController) {
        ui.label("Cut molecular surface by the current clipping plane:");
        let surfaces = self.state.active_surface_summaries();
        if !surfaces
            .iter()
            .any(|surface| Some(surface.uid) == self.cut_surface_target)
        {
            self.cut_surface_target = match self.workspace_aux_selection {
                Some(WorkspaceAuxSelection::Surface { surface_uid, .. })
                    if surfaces.iter().any(|surface| surface.uid == surface_uid) =>
                {
                    Some(surface_uid)
                }
                _ => surfaces.first().map(|surface| surface.uid),
            };
        }
        if surfaces.is_empty() {
            ui.weak("No molecular surface is loaded.");
            return;
        }
        let selected_name = surfaces
            .iter()
            .find(|surface| Some(surface.uid) == self.cut_surface_target)
            .map_or("(none)", |surface| surface.name.as_str());
        egui::ComboBox::from_label("Target surface")
            .selected_text(selected_name)
            .show_ui(ui, |ui| {
                for surface in &surfaces {
                    ui.selectable_value(
                        &mut self.cut_surface_target,
                        Some(surface.uid),
                        &surface.name,
                    );
                }
            });
        egui::ComboBox::from_label("Cross section type")
            .selected_text(surface_cut_mode_label(&self.cut_surface_mode))
            .show_ui(ui, |ui| {
                for (mode, label) in SURFACE_CUT_MODES {
                    ui.selectable_value(&mut self.cut_surface_mode, mode.to_string(), label);
                }
            });
        ui.horizontal(|ui| {
            ui.label("Section mesh density (/Å)");
            ui.add(
                egui::DragValue::new(&mut self.cut_surface_density)
                    .speed(0.1)
                    .range(0.1..=100.0),
            );
        });
        if ui.button("OK").clicked() {
            self.cut_surface_by_current_plane(camera);
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

    fn get_pdb_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_get_pdb {
            return;
        }
        if !self.pdb_history_loaded {
            self.pdb_history = crate::load_pdb_history();
            if self.pdb_id.is_empty() {
                self.pdb_id = self.pdb_history.first().cloned().unwrap_or_default();
            }
            self.pdb_history_loaded = true;
        }
        let mut open = self.show_get_pdb;
        let mut proceed = false;
        egui::Window::new("Download PDB")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("Enter PDB ID:");
                let mut history_pick = None;
                ui.horizontal(|ui| {
                    let input_width = (ui.available_width() - 78.0).max(120.0);
                    ui.add_sized(
                        [input_width, 22.0],
                        egui::TextEdit::singleline(&mut self.pdb_id),
                    );
                    ui.add_enabled_ui(!self.pdb_history.is_empty(), |ui| {
                        ui.menu_button("History", |ui| {
                            for entry in &self.pdb_history {
                                if ui.button(entry).clicked() {
                                    history_pick = Some(entry.clone());
                                    ui.close();
                                }
                            }
                        });
                    });
                });
                if let Some(pdb_id) = history_pick {
                    self.pdb_id = pdb_id;
                }
                let valid_pdb_id = crate::normalize_pdb_id(&self.pdb_id).is_ok();
                ui.group(|ui| {
                    ui.strong("Coordinates");
                    ui.checkbox(&mut self.pdb_fetch_coordinates, "Fetch pdb file");
                    ui.add_enabled_ui(self.pdb_fetch_coordinates, |ui| {
                        egui::ComboBox::from_label("Coordinate source")
                            .selected_text(if self.pdb_coordinate_source == "RCSB_PDB" {
                                "RCSB (files.rcsb.org)/pdb.gz"
                            } else {
                                "RCSB (files.rcsb.org)/cif.gz"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.pdb_coordinate_source,
                                    "RCSB_CIF".to_string(),
                                    "RCSB (files.rcsb.org)/cif.gz",
                                );
                                ui.selectable_value(
                                    &mut self.pdb_coordinate_source,
                                    "RCSB_PDB".to_string(),
                                    "RCSB (files.rcsb.org)/pdb.gz",
                                );
                            });
                    });
                });
                ui.group(|ui| {
                    ui.strong("Density maps");
                    ui.checkbox(&mut self.pdb_fetch_two_fofc, "Fetch map file (2Fo-Fc)");
                    ui.checkbox(&mut self.pdb_fetch_fofc, "Fetch map file (Fo-Fc)");
                    ui.add_enabled_ui(self.pdb_fetch_two_fofc || self.pdb_fetch_fofc, |ui| {
                        egui::ComboBox::from_label("Map source")
                            .selected_text(if self.pdb_map_source == "EBI_MTZ" {
                                "EBI (www.ebi.ac.uk)"
                            } else {
                                "RCSB (files.rcsb.org)"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.pdb_map_source,
                                    "RCSB_CIF".to_string(),
                                    "RCSB (files.rcsb.org)",
                                );
                                ui.selectable_value(
                                    &mut self.pdb_map_source,
                                    "EBI_MTZ".to_string(),
                                    "EBI (www.ebi.ac.uk)",
                                );
                            });
                    });
                });
                let any_selected =
                    self.pdb_fetch_coordinates || self.pdb_fetch_two_fofc || self.pdb_fetch_fofc;
                self.update_pdb_probe(ctx);
                let availability_ok = self
                    .pdb_probe_result
                    .as_ref()
                    .is_some_and(|result| result.as_ref().is_ok_and(Vec::is_empty));
                let availability_message = if valid_pdb_id && !any_selected {
                    "Neither pdb nor map selected".to_string()
                } else if valid_pdb_id && any_selected {
                    match self.pdb_probe_result.as_ref() {
                        Some(Ok(unavailable)) if !unavailable.is_empty() => {
                            let entry_type =
                                if unavailable.contains(&crate::GuiPdbFileKind::Coordinates) {
                                    "PDB"
                                } else {
                                    "Map"
                                };
                            format!(
                                "{entry_type} Entry {}: not found.",
                                self.pdb_id.trim().to_ascii_lowercase()
                            )
                        }
                        Some(Err(error)) => format!("Availability check failed: {error}"),
                        Some(Ok(_)) => String::new(),
                        None => "Checking availability…".to_string(),
                    }
                } else {
                    String::new()
                };
                if !availability_message.is_empty() {
                    ui.label(availability_message);
                }
                if ui
                    .add_enabled(
                        valid_pdb_id
                            && any_selected
                            && availability_ok
                            && self.pdb_download_receiver.is_none(),
                        egui::Button::new("Download and Open"),
                    )
                    .clicked()
                {
                    proceed = true;
                }
            });
        self.show_get_pdb = open && !proceed;
        if proceed {
            if self.pdb_fetch_coordinates {
                self.open_pdb_renderer_setup();
            } else {
                self.start_pdb_download(ctx, None);
            }
        }
    }

    fn open_pdb_renderer_setup(&mut self) {
        let object_name = self.pdb_id.trim().to_ascii_lowercase();
        self.open_renderer_setup(&object_name, NativeRendererSetupAction::PdbDownload);
    }

    fn open_renderer_setup(&mut self, base_object_name: &str, action: NativeRendererSetupAction) {
        let reader_options = match &action {
            NativeRendererSetupAction::OpenPath(path) => {
                match self.state.molecule_reader_options_for_path(path) {
                    Ok(options) => options,
                    Err(error) => {
                        self.set_error(error);
                        return;
                    }
                }
            }
            NativeRendererSetupAction::PdbDownload => None,
        };
        let scene_uid = self.state.ensure_active_scene();
        let options = match self.state.molecule_renderer_setup_options(scene_uid) {
            Ok(options) if !options.is_empty() => options,
            Ok(_) => {
                self.set_status("No compatible molecular renderers are available");
                return;
            }
            Err(error) => {
                self.set_error(error);
                return;
            }
        };
        let preferences = crate::load_renderer_setup_preferences();
        let renderer_type = preferences
            .renderer_type
            .filter(|renderer_type| options.iter().any(|option| option.name == *renderer_type))
            .unwrap_or_else(|| options[0].name.clone());
        let object_name = self
            .state
            .suggest_object_name(scene_uid, base_object_name)
            .unwrap_or_else(|_| base_object_name.to_string());
        let renderer_name = self
            .state
            .suggest_renderer_name(scene_uid, &renderer_type)
            .unwrap_or_else(|_| format!("{renderer_type}1"));
        self.renderer_setup_draft = Some(NativeRendererSetupDraft {
            action,
            reader_options,
            object_name,
            renderer_type,
            renderer_name,
            renderer_name_default: true,
            selection_enabled: false,
            selection: String::new(),
            recenter: preferences.recenter,
            options,
        });
    }

    fn renderer_setup_dialog(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        let Some(mut draft) = self.renderer_setup_draft.take() else {
            return;
        };
        let mut open = true;
        let mut accept = false;
        let mut cancel = false;
        let selection_history = self.selection_history.clone();
        egui::Window::new("Setup renderer")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("pdb_renderer_setup_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Object:");
                        ui.add_sized(
                            [240.0, 22.0],
                            egui::TextEdit::singleline(&mut draft.object_name),
                        );
                        ui.end_row();

                        ui.label("Renderer type:");
                        let selected_label = draft
                            .options
                            .iter()
                            .find(|option| option.name == draft.renderer_type)
                            .map(|option| option.description.as_str())
                            .unwrap_or(draft.renderer_type.as_str());
                        let mut selected_type = None;
                        egui::ComboBox::from_id_salt("pdb_renderer_setup_type")
                            .selected_text(selected_label)
                            .show_ui(ui, |ui| {
                                let mut separator_added = false;
                                for option in &draft.options {
                                    if !option.preset && !separator_added {
                                        ui.separator();
                                        separator_added = true;
                                    }
                                    if ui
                                        .selectable_label(
                                            draft.renderer_type == option.name,
                                            &option.description,
                                        )
                                        .clicked()
                                    {
                                        selected_type = Some(option.name.clone());
                                    }
                                }
                            });
                        if let Some(renderer_type) = selected_type {
                            draft.renderer_type = renderer_type;
                            if draft.renderer_name_default {
                                let scene_uid = self.state.ensure_active_scene();
                                draft.renderer_name = self
                                    .state
                                    .suggest_renderer_name(scene_uid, &draft.renderer_type)
                                    .unwrap_or_else(|_| format!("{}1", draft.renderer_type));
                            }
                        }
                        ui.end_row();

                        ui.label("Renderer name:");
                        let name_response = ui.add_sized(
                            [240.0, 22.0],
                            egui::TextEdit::singleline(&mut draft.renderer_name),
                        );
                        if name_response.changed() && !draft.renderer_name.trim().is_empty() {
                            draft.renderer_name_default = false;
                        }
                        ui.end_row();

                        ui.checkbox(&mut draft.selection_enabled, "Selection:");
                        ui.add_enabled_ui(draft.selection_enabled, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [166.0, 22.0],
                                    egui::TextEdit::singleline(&mut draft.selection),
                                );
                                ui.add_enabled_ui(!selection_history.is_empty(), |ui| {
                                    ui.menu_button("History", |ui| {
                                        for entry in selection_history.iter().rev() {
                                            if ui.button(entry).clicked() {
                                                draft.selection = entry.clone();
                                                ui.close();
                                            }
                                        }
                                    });
                                });
                            });
                        });
                        ui.end_row();
                    });
                ui.checkbox(&mut draft.recenter, "Recenter view");
                if let Some(options) = draft.reader_options.as_mut() {
                    ui.separator();
                    ui.strong(match options.kind {
                        crate::GuiMoleculeReaderKind::Pdb => "PDB options",
                        crate::GuiMoleculeReaderKind::Mmcif => "mmCIF options",
                    });
                    ui.checkbox(
                        &mut options.ignore_multiple_models,
                        "Ignore multilpe models",
                    );
                    ui.checkbox(&mut options.ignore_anisou, "Ignore anisotropic U");
                    ui.checkbox(
                        &mut options.ignore_alt_conf,
                        "Ignore alternate conformation",
                    );
                    if options.kind == crate::GuiMoleculeReaderKind::Pdb {
                        ui.checkbox(&mut options.load_segid_as_chain, "Load SEGID as chain name");
                    }
                    ui.checkbox(
                        &mut options.calculate_secondary_structure,
                        "Calculate protein secondary structure",
                    );
                    ui.checkbox(
                        &mut options.auto_generate_topology,
                        "Auto-generate non-standard topology",
                    );
                }
                ui.separator();
                ui.horizontal(|ui| {
                    let valid = !draft.object_name.trim().is_empty()
                        && !draft.renderer_type.trim().is_empty()
                        && !draft.renderer_name.trim().is_empty()
                        && (!draft.selection_enabled || !draft.selection.trim().is_empty());
                    if ui.add_enabled(valid, egui::Button::new("OK")).clicked() {
                        accept = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if accept {
            let setup = crate::GuiRendererSetup {
                object_name: draft.object_name.trim().to_string(),
                renderer_type: draft.renderer_type.trim().to_string(),
                renderer_name: draft.renderer_name.trim().to_string(),
                selection: draft
                    .selection_enabled
                    .then(|| draft.selection.trim().to_string()),
                recenter: draft.recenter,
            };
            if let Some(selection) = setup.selection.as_deref() {
                self.remember_selection_history(selection);
            }
            let _ = crate::save_renderer_setup_preferences(&setup.renderer_type, setup.recenter);
            match draft.action {
                NativeRendererSetupAction::PdbDownload => {
                    self.start_pdb_download(ctx, Some(setup));
                }
                NativeRendererSetupAction::OpenPath(path) => {
                    self.open_path_with_renderer_setup(
                        path,
                        draft.reader_options.as_ref(),
                        setup,
                        camera,
                    );
                }
            }
        } else if open && !cancel {
            self.renderer_setup_draft = Some(draft);
        }
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
            workspace_aux_selection: self.workspace_aux_selection,
            selection_text: self.selection_text.clone(),
            last_loaded_path: self.last_loaded_path.clone(),
            modified_scenes: self.modified_scenes.clone(),
        }
    }

    fn push_undo_snapshot(&mut self, snapshot: GuiUndoSnapshot) {
        if let Some(scene_uid) = self
            .state
            .active_scene()
            .filter(|scene_uid| snapshot.state.scene_manager().scene(*scene_uid).is_some())
        {
            self.modified_scenes.insert(scene_uid);
        }
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
        self.workspace_aux_selection = snapshot.workspace_aux_selection;
        self.selection_text = snapshot.selection_text;
        self.last_loaded_path = snapshot.last_loaded_path;
        self.modified_scenes = snapshot.modified_scenes;
        self.sync_active_scene_properties();
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
        self.show_workspace = true;
        self.left_panel_visible.fill(true);
        self.reset_left_panel_layout = true;
        self.set_status("Panel layout restored");
    }

    fn open_renderer_dialog(&mut self, object_uid: cuemol_core::Uid) {
        self.open_renderer_dialog_in_group(object_uid, None);
    }

    fn open_renderer_group_dialog(&mut self, object_uid: cuemol_core::Uid) {
        let used = self
            .state
            .renderer_summaries(object_uid)
            .into_iter()
            .filter(|renderer| renderer.renderer_type == "*group")
            .map(|renderer| renderer.name)
            .collect::<std::collections::BTreeSet<_>>();
        let mut serial = 1;
        while used.contains(&format!("group{serial}")) {
            serial += 1;
        }
        self.new_renderer_owner = Some(NativePropertyObject::Molecule(object_uid));
        self.new_renderer_group = None;
        self.new_renderer_type = "*group".to_string();
        self.new_renderer_name = format!("group{serial}");
        self.show_renderer_dialog = true;
    }

    fn open_renderer_dialog_in_group(
        &mut self,
        object_uid: cuemol_core::Uid,
        group: Option<String>,
    ) {
        self.new_renderer_owner = Some(NativePropertyObject::Molecule(object_uid));
        self.new_renderer_group = group;
        if !RENDERER_TYPES.contains(&self.new_renderer_type.as_str()) {
            self.new_renderer_type = "simple".to_string();
        }
        self.new_renderer_name = self
            .state
            .suggest_renderer_name_in_active_scene(&self.new_renderer_type)
            .unwrap_or_else(|_| format!("{}1", self.new_renderer_type.trim()));
        self.show_renderer_dialog = true;
    }

    fn open_aux_renderer_dialog(&mut self, selection: WorkspaceAuxSelection) {
        match selection {
            WorkspaceAuxSelection::CameraRoot { .. }
            | WorkspaceAuxSelection::Camera { .. }
            | WorkspaceAuxSelection::StyleRoot { .. }
            | WorkspaceAuxSelection::Style { .. } => return,
            WorkspaceAuxSelection::DensityMap { map_uid, .. } => {
                self.new_renderer_owner = Some(NativePropertyObject::DensityMap(map_uid));
                self.new_renderer_group = None;
                if !DENSITY_RENDERER_TYPES.contains(&self.new_renderer_type.as_str()) {
                    self.new_renderer_type = "contour".to_string();
                }
                let serial = self
                    .state
                    .density_map_summary(map_uid)
                    .map(|map| {
                        map.renderers
                            .iter()
                            .filter(|renderer| renderer.renderer_type == self.new_renderer_type)
                            .count()
                            + 1
                    })
                    .unwrap_or(1);
                self.new_renderer_name = format!("{} {serial}", self.new_renderer_type);
            }
            WorkspaceAuxSelection::Surface { surface_uid, .. } => {
                self.new_renderer_owner = Some(NativePropertyObject::Surface(surface_uid));
                self.new_renderer_group = None;
                self.new_renderer_type = "molsurf".to_string();
                let serial = self
                    .state
                    .surface_summary(surface_uid)
                    .map(|surface| surface.renderers.len() + 1)
                    .unwrap_or(1);
                self.new_renderer_name = format!("molsurf {serial}");
            }
        }
        self.show_renderer_dialog = true;
    }

    fn create_renderer_from_dialog(&mut self) {
        let Some(owner) = self.new_renderer_owner else {
            self.set_status("No target object selected");
            return;
        };
        let renderer_type = self.new_renderer_type.trim().to_string();
        let renderer_name = self.new_renderer_name.trim().to_string();
        if renderer_type.is_empty() || renderer_name.is_empty() || renderer_name.contains(':') {
            self.set_status("Renderer type and name are required");
            return;
        }
        let undo = self.capture_undo();
        let renderer_group = self.new_renderer_group.clone();
        let created = match owner {
            NativePropertyObject::Molecule(object_uid) => (|| {
                let renderer_uid = if renderer_type == "*group" {
                    self.state
                        .create_renderer_group(object_uid, renderer_name)?
                } else {
                    self.state
                        .new_renderer_in_active_scene(object_uid, renderer_type, renderer_name)?
                        .result_renderer
                };
                if let Some(group) = renderer_group.as_deref() {
                    self.state
                        .set_renderer_property(object_uid, renderer_uid, "group", group)?;
                }
                Ok(renderer_uid)
            })(),
            NativePropertyObject::DensityMap(map_uid) => (|| {
                if !DENSITY_RENDERER_TYPES.contains(&renderer_type.as_str()) {
                    return Err(CueMolError::invalid_argument(format!(
                        "unsupported density renderer: {renderer_type}"
                    )));
                }
                let renderer_uid = self.state.add_density_renderer(map_uid, &renderer_type)?;
                let renderer = self
                    .state
                    .density_map_summary(map_uid)
                    .and_then(|map| {
                        map.renderers
                            .into_iter()
                            .find(|renderer| renderer.uid == renderer_uid)
                    })
                    .ok_or_else(|| {
                        CueMolError::NotFound(format!("renderer {}", renderer_uid.get()))
                    })?;
                let mut specific = renderer.specific;
                strip_native_renderer_common_specific(&mut specific);
                self.state.set_density_renderer_properties(
                    map_uid,
                    renderer_uid,
                    renderer_name,
                    renderer.visible,
                    renderer.locked,
                    renderer.material,
                    renderer.alpha,
                    renderer.edge_type,
                    renderer.edge_width,
                    renderer.edge_color,
                    &specific,
                )?;
                Ok(renderer_uid)
            })(),
            NativePropertyObject::Surface(surface_uid) => (|| {
                if renderer_type != "molsurf" {
                    return Err(CueMolError::invalid_argument(format!(
                        "unsupported surface renderer: {renderer_type}"
                    )));
                }
                let renderer_uid = self.state.add_surface_renderer(surface_uid)?;
                let renderer = self
                    .state
                    .surface_summary(surface_uid)
                    .and_then(|surface| {
                        surface
                            .renderers
                            .into_iter()
                            .find(|renderer| renderer.uid == renderer_uid)
                    })
                    .ok_or_else(|| {
                        CueMolError::NotFound(format!("renderer {}", renderer_uid.get()))
                    })?;
                let mut specific = renderer.specific;
                strip_native_renderer_common_specific(&mut specific);
                self.state.set_surface_renderer_properties(
                    surface_uid,
                    renderer_uid,
                    renderer_name,
                    &renderer.selection,
                    renderer.visible,
                    renderer.locked,
                    renderer.material,
                    renderer.alpha,
                    renderer.edge_type,
                    renderer.edge_width,
                    renderer.edge_color,
                    &specific,
                )?;
                Ok(renderer_uid)
            })(),
        };
        match created {
            Ok(renderer_uid) => {
                self.push_undo_snapshot(undo);
                match owner {
                    NativePropertyObject::Molecule(object_uid) => {
                        self.selected_object = Some(object_uid);
                        self.selected_renderer = Some(renderer_uid);
                        self.workspace_aux_selection = None;
                    }
                    NativePropertyObject::DensityMap(map_uid) => {
                        let scene_uid = self
                            .state
                            .density_map_summary(map_uid)
                            .map(|map| map.scene_uid)
                            .expect("created density renderer must retain its map");
                        self.selected_object = None;
                        self.selected_renderer = None;
                        self.workspace_aux_selection = Some(WorkspaceAuxSelection::DensityMap {
                            scene_uid,
                            map_uid,
                            renderer_uid: Some(renderer_uid),
                        });
                    }
                    NativePropertyObject::Surface(surface_uid) => {
                        let scene_uid = self
                            .state
                            .surface_summary(surface_uid)
                            .map(|surface| surface.scene_uid)
                            .expect("created surface renderer must retain its surface");
                        self.selected_object = None;
                        self.selected_renderer = None;
                        self.workspace_aux_selection = Some(WorkspaceAuxSelection::Surface {
                            scene_uid,
                            surface_uid,
                            renderer_uid: Some(renderer_uid),
                        });
                    }
                }
                self.show_renderer_dialog = false;
                self.rebuild_snapshot();
                self.set_status("Renderer added");
            }
            Err(error) => {
                self.state = undo.state;
                self.set_error(error);
            }
        }
    }

    fn set_renderer_visibility(
        &mut self,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        visible: bool,
    ) {
        let scene_uid = self.state.scene_manager().scene_uid_for_object(object_uid);
        let undo = self.capture_undo();
        match self
            .state
            .set_renderer_visible(object_uid, renderer_uid, visible)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                if let Some(scene_uid) = scene_uid {
                    self.modified_scenes.insert(scene_uid);
                }
                self.rebuild_snapshot();
                self.set_status(format!("Renderer visibility: {visible}"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn workspace_owner_scene_uid(&self, owner: NativePropertyObject) -> Option<cuemol_core::Uid> {
        match owner {
            NativePropertyObject::Molecule(object_uid) => {
                self.state.scene_manager().scene_uid_for_object(object_uid)
            }
            NativePropertyObject::DensityMap(map_uid) => self
                .state
                .density_map_summary(map_uid)
                .map(|map| map.scene_uid),
            NativePropertyObject::Surface(surface_uid) => self
                .state
                .surface_summary(surface_uid)
                .map(|surface| surface.scene_uid),
        }
    }

    fn finish_workspace_toggle(
        &mut self,
        undo: GuiUndoSnapshot,
        scene_uid: Option<cuemol_core::Uid>,
        result: Result<()>,
        status: String,
    ) {
        match result {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                if let Some(scene_uid) = scene_uid {
                    self.modified_scenes.insert(scene_uid);
                }
                self.rebuild_snapshot();
                self.set_status(status);
            }
            Err(error) => {
                self.state = undo.state;
                self.set_error(error);
            }
        }
    }

    fn set_workspace_object_visibility(&mut self, owner: NativePropertyObject, visible: bool) {
        let scene_uid = self.workspace_owner_scene_uid(owner);
        let undo = self.capture_undo();
        let result = match owner {
            NativePropertyObject::Molecule(object_uid) => {
                self.state.set_object_visible(object_uid, visible)
            }
            NativePropertyObject::DensityMap(map_uid) => {
                self.state.set_density_map_visible(map_uid, visible)
            }
            NativePropertyObject::Surface(surface_uid) => {
                self.state.set_surface_visible(surface_uid, visible)
            }
        };
        self.finish_workspace_toggle(
            undo,
            scene_uid,
            result,
            format!("Object visibility: {visible}"),
        );
    }

    fn set_workspace_object_locked(&mut self, owner: NativePropertyObject, locked: bool) {
        let scene_uid = self.workspace_owner_scene_uid(owner);
        let undo = self.capture_undo();
        let result = match owner {
            NativePropertyObject::Molecule(object_uid) => {
                self.state.set_object_locked(object_uid, locked)
            }
            NativePropertyObject::DensityMap(map_uid) => {
                self.state.set_density_map_locked(map_uid, locked)
            }
            NativePropertyObject::Surface(surface_uid) => {
                self.state.set_surface_locked(surface_uid, locked)
            }
        };
        self.finish_workspace_toggle(
            undo,
            scene_uid,
            result,
            if locked {
                "Object locked".to_string()
            } else {
                "Object unlocked".to_string()
            },
        );
    }

    fn set_workspace_renderer_locked(
        &mut self,
        owner: NativePropertyObject,
        renderer_uid: cuemol_core::Uid,
        locked: bool,
    ) {
        let scene_uid = self.workspace_owner_scene_uid(owner);
        let undo = self.capture_undo();
        let result =
            match owner {
                NativePropertyObject::Molecule(object_uid) => {
                    self.state
                        .set_renderer_locked(object_uid, renderer_uid, locked)
                }
                NativePropertyObject::DensityMap(map_uid) => self
                    .state
                    .set_density_renderer_locked(map_uid, renderer_uid, locked),
                NativePropertyObject::Surface(surface_uid) => self
                    .state
                    .set_surface_renderer_locked(surface_uid, renderer_uid, locked),
            };
        self.finish_workspace_toggle(
            undo,
            scene_uid,
            result,
            if locked {
                "Renderer locked".to_string()
            } else {
                "Renderer unlocked".to_string()
            },
        );
    }

    fn set_renderer_group_visibility(
        &mut self,
        object_uid: cuemol_core::Uid,
        group_uid: cuemol_core::Uid,
        visible: bool,
    ) {
        let scene_uid = self.state.scene_manager().scene_uid_for_object(object_uid);
        let undo = self.capture_undo();
        match self
            .state
            .set_renderer_group_visible(object_uid, group_uid, visible)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                if let Some(scene_uid) = scene_uid {
                    self.modified_scenes.insert(scene_uid);
                }
                self.rebuild_snapshot();
                self.set_status(format!("Renderer group visibility: {visible}"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn set_density_map_visibility(&mut self, map_uid: cuemol_core::Uid, visible: bool) {
        let Some(map) = self.state.density_map_summary(map_uid) else {
            self.set_status(format!("Density map {} was not found", map_uid.get()));
            return;
        };
        let undo = self.capture_undo();
        match self
            .state
            .set_density_map_common_properties(map_uid, map.name, visible, map.locked)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(map.scene_uid);
                self.rebuild_snapshot();
                self.set_status(format!("Density map visibility: {visible}"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn set_density_renderer_visibility(
        &mut self,
        map_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        visible: bool,
    ) {
        let scene_uid = self
            .state
            .density_map_summary(map_uid)
            .map(|map| map.scene_uid);
        let undo = self.capture_undo();
        match self
            .state
            .set_density_renderer_visible(map_uid, renderer_uid, visible)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                if let Some(scene_uid) = scene_uid {
                    self.modified_scenes.insert(scene_uid);
                }
                self.rebuild_snapshot();
                self.set_status(format!("Density renderer visibility: {visible}"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn set_surface_visibility(&mut self, surface_uid: cuemol_core::Uid, visible: bool) {
        let Some(surface) = self.state.surface_summary(surface_uid) else {
            self.set_status(format!("Surface {} was not found", surface_uid.get()));
            return;
        };
        let undo = self.capture_undo();
        match self.state.set_surface_common_properties(
            surface_uid,
            surface.name,
            visible,
            surface.locked,
        ) {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(surface.scene_uid);
                self.rebuild_snapshot();
                self.set_status(format!("Surface visibility: {visible}"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn set_surface_renderer_visibility(
        &mut self,
        surface_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        visible: bool,
    ) {
        let scene_uid = self
            .state
            .surface_summary(surface_uid)
            .map(|surface| surface.scene_uid);
        let undo = self.capture_undo();
        match self
            .state
            .set_surface_renderer_visible(surface_uid, renderer_uid, visible)
        {
            Ok(()) => {
                self.push_undo_snapshot(undo);
                if let Some(scene_uid) = scene_uid {
                    self.modified_scenes.insert(scene_uid);
                }
                self.rebuild_snapshot();
                self.set_status(format!("Surface renderer visibility: {visible}"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn open_aux_properties(&mut self, selection: WorkspaceAuxSelection) {
        match selection {
            WorkspaceAuxSelection::CameraRoot { .. }
            | WorkspaceAuxSelection::Camera { .. }
            | WorkspaceAuxSelection::StyleRoot { .. }
            | WorkspaceAuxSelection::Style { .. } => {}
            WorkspaceAuxSelection::DensityMap {
                map_uid,
                renderer_uid: Some(renderer_uid),
                ..
            } => self.open_density_renderer_properties(map_uid, renderer_uid),
            WorkspaceAuxSelection::DensityMap {
                map_uid,
                renderer_uid: None,
                ..
            } => self.open_density_map_properties(map_uid),
            WorkspaceAuxSelection::Surface {
                surface_uid,
                renderer_uid: Some(renderer_uid),
                ..
            } => self.open_surface_renderer_properties(surface_uid, renderer_uid),
            WorkspaceAuxSelection::Surface {
                surface_uid,
                renderer_uid: None,
                ..
            } => self.open_surface_properties(surface_uid),
        }
    }

    fn delete_aux_selection(&mut self, selection: WorkspaceAuxSelection) {
        match selection {
            WorkspaceAuxSelection::CameraRoot { .. } => {}
            WorkspaceAuxSelection::Camera { scene_uid, .. } => {
                let Some(summary) = self.camera_summary_for_selection(selection) else {
                    self.set_status("Camera not found");
                    return;
                };
                let undo = self.capture_undo();
                match self.state.destroy_scene_camera(scene_uid, &summary.name) {
                    Ok(true) => {
                        self.push_undo_snapshot(undo);
                        self.modified_scenes.insert(scene_uid);
                        self.workspace_aux_selection =
                            Some(WorkspaceAuxSelection::CameraRoot { scene_uid });
                        self.set_status(format!("Destroyed camera {}", summary.name));
                    }
                    Ok(false) => self.set_status("Camera was already absent"),
                    Err(error) => self.set_error(error),
                }
            }
            WorkspaceAuxSelection::StyleRoot { .. } => {}
            WorkspaceAuxSelection::Style { scene_uid, .. } => {
                let Some(summary) = self.style_summary_for_selection(selection) else {
                    self.set_status("Style not found");
                    return;
                };
                let Some(local_index) = summary.local_index else {
                    self.set_status("Global styles cannot be deleted");
                    return;
                };
                let undo = self.capture_undo();
                match self.state.destroy_scene_style(scene_uid, local_index) {
                    Ok(true) => {
                        self.push_undo_snapshot(undo);
                        self.modified_scenes.insert(scene_uid);
                        self.workspace_aux_selection =
                            Some(WorkspaceAuxSelection::StyleRoot { scene_uid });
                        self.rebuild_snapshot();
                        self.set_status(format!("Destroyed style {}", summary.name));
                    }
                    Ok(false) => self.set_status("Style was already absent"),
                    Err(error) => self.set_error(error),
                }
            }
            WorkspaceAuxSelection::DensityMap {
                map_uid,
                renderer_uid: None,
                ..
            } => self.delete_workspace_object(NativePropertyObject::DensityMap(map_uid)),
            WorkspaceAuxSelection::Surface {
                surface_uid,
                renderer_uid: None,
                ..
            } => self.delete_workspace_object(NativePropertyObject::Surface(surface_uid)),
            WorkspaceAuxSelection::DensityMap {
                scene_uid,
                map_uid,
                renderer_uid: Some(renderer_uid),
            } => {
                let undo = self.capture_undo();
                match self.state.destroy_density_renderer(map_uid, renderer_uid) {
                    Ok(true) => {
                        self.push_undo_snapshot(undo);
                        self.modified_scenes.insert(scene_uid);
                        self.workspace_aux_selection = Some(WorkspaceAuxSelection::DensityMap {
                            scene_uid,
                            map_uid,
                            renderer_uid: None,
                        });
                        self.rebuild_snapshot();
                        self.set_status("Renderer deleted");
                    }
                    Ok(false) => self.set_status("Renderer was already absent"),
                    Err(error) => self.set_error(error),
                }
            }
            WorkspaceAuxSelection::Surface {
                scene_uid,
                surface_uid,
                renderer_uid: Some(renderer_uid),
            } => {
                let undo = self.capture_undo();
                match self
                    .state
                    .destroy_surface_renderer(surface_uid, renderer_uid)
                {
                    Ok(true) => {
                        self.push_undo_snapshot(undo);
                        self.modified_scenes.insert(scene_uid);
                        self.workspace_aux_selection = Some(WorkspaceAuxSelection::Surface {
                            scene_uid,
                            surface_uid,
                            renderer_uid: None,
                        });
                        self.rebuild_snapshot();
                        self.set_status("Renderer deleted");
                    }
                    Ok(false) => self.set_status("Renderer was already absent"),
                    Err(error) => self.set_error(error),
                }
            }
        }
    }

    fn generate_workspace_surface(&mut self, selection: WorkspaceAuxSelection) {
        let WorkspaceAuxSelection::DensityMap {
            scene_uid,
            map_uid,
            renderer_uid: Some(renderer_uid),
        } = selection
        else {
            self.set_status("Select an isosurface renderer");
            return;
        };
        let undo = self.capture_undo();
        match self
            .state
            .generate_density_surface_object(map_uid, renderer_uid)
        {
            Ok(surface_uid) => {
                let name = self
                    .state
                    .surface_summary(surface_uid)
                    .map(|surface| surface.name)
                    .unwrap_or_else(|| surface_uid.get().to_string());
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = Some(WorkspaceAuxSelection::Surface {
                    scene_uid,
                    surface_uid,
                    renderer_uid: None,
                });
                self.rebuild_snapshot();
                self.set_status(format!("Generated surface object {name}"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn regenerate_workspace_surface(&mut self, selection: WorkspaceAuxSelection) {
        let WorkspaceAuxSelection::Surface {
            scene_uid,
            surface_uid,
            ..
        } = selection
        else {
            self.set_status("Select a molecular surface");
            return;
        };
        let undo = self.capture_undo();
        match self.state.regenerate_surface_object(surface_uid) {
            Ok(triangle_count) => {
                self.push_undo_snapshot(undo);
                self.modified_scenes.insert(scene_uid);
                self.rebuild_snapshot();
                self.set_status(format!("Regenerated surface: {triangle_count} triangle(s)"));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn delete_workspace_object(&mut self, target: NativePropertyObject) {
        let scene_uid = match target {
            NativePropertyObject::Molecule(object_uid) => {
                self.state.scene_manager().scene_uid_for_object(object_uid)
            }
            NativePropertyObject::DensityMap(map_uid) => self
                .state
                .density_map_summary(map_uid)
                .map(|map| map.scene_uid),
            NativePropertyObject::Surface(surface_uid) => self
                .state
                .surface_summary(surface_uid)
                .map(|surface| surface.scene_uid),
        };
        let undo = self.capture_undo();
        let result = match target {
            NativePropertyObject::Molecule(object_uid) => self.state.destroy_object(object_uid),
            NativePropertyObject::DensityMap(map_uid) => self.state.destroy_density_map(map_uid),
            NativePropertyObject::Surface(surface_uid) => self.state.destroy_surface(surface_uid),
        };
        match result {
            Ok(true) => {
                self.push_undo_snapshot(undo);
                if let Some(scene_uid) = scene_uid {
                    self.modified_scenes.insert(scene_uid);
                }
                self.selected_object = None;
                self.selected_renderer = None;
                self.workspace_aux_selection = None;
                self.rebuild_snapshot();
                self.set_status("Object deleted");
            }
            Ok(false) => self.set_status("Object was already absent"),
            Err(error) => self.set_error(error),
        }
    }

    fn delete_renderer(&mut self, object_uid: cuemol_core::Uid, renderer_uid: cuemol_core::Uid) {
        let is_group = self
            .state
            .renderer_summaries(object_uid)
            .into_iter()
            .any(|renderer| renderer.uid == renderer_uid && renderer.renderer_type == "*group");
        let scene_uid = self.state.scene_manager().scene_uid_for_object(object_uid);
        let undo = self.capture_undo();
        let result = if is_group {
            self.state
                .destroy_renderer_group_in_object(object_uid, renderer_uid)
        } else {
            self.state
                .destroy_renderer_in_object(object_uid, renderer_uid)
        };
        match result {
            Ok(true) => {
                self.push_undo_snapshot(undo);
                if let Some(scene_uid) = scene_uid {
                    self.modified_scenes.insert(scene_uid);
                }
                if self.selected_renderer == Some(renderer_uid) {
                    self.selected_renderer = None;
                }
                self.rebuild_snapshot();
                self.set_status(if is_group {
                    "Renderer group deleted"
                } else {
                    "Renderer deleted"
                });
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
                self.invalidate_molecule_panel_caches();
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
                self.invalidate_molecule_panel_caches();
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
                self.ctrl_down = modifiers.state().control_key();
                false
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed if self.pointer_in_viewport(camera.last_cursor) => {
                    if *button == MouseButton::Left
                        && self.interaction_mode == InteractionMode::RectSelect
                    {
                        self.rect_select_start = camera.last_cursor;
                        self.rect_select_current = camera.last_cursor;
                        camera.drag_mode = None;
                        camera.last_drag_cursor = None;
                        camera.drag_total = 0.0;
                        self.viewport_menu = None;
                        return self.rect_select_start.is_some();
                    }
                    camera.drag_mode = self.camera_drag_mode(*button);
                    camera.last_drag_cursor = camera.last_cursor;
                    camera.drag_total = 0.0;
                    self.viewport_menu = None;
                    camera.drag_mode.is_some()
                }
                ElementState::Released => {
                    if *button == MouseButton::Left {
                        if let Some(start) = self.rect_select_start.take() {
                            let current = self
                                .rect_select_current
                                .take()
                                .or(camera.last_cursor)
                                .unwrap_or(start);
                            camera.drag_mode = None;
                            camera.last_drag_cursor = None;
                            camera.drag_total = 0.0;
                            self.select_atoms_in_screen_rect(
                                camera,
                                egui::Rect::from_two_pos(start, current),
                            );
                            return true;
                        }
                    }
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
                    let should_menu = *button == MouseButton::Right && is_click;
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
                self.rect_select_start = None;
                self.rect_select_current = None;
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = self.cursor_position_points(position.x, position.y);
                if self.rect_select_start.is_some() {
                    self.rect_select_current = Some(pos);
                    camera.last_cursor = Some(pos);
                    return true;
                }
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
                                        self.state.view_input_config().tbrad(),
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
                let (x, y, step) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x * 24.0, *y * 24.0, *y),
                    MouseScrollDelta::PixelDelta(pos) => {
                        (pos.x as f32, pos.y as f32, pos.y as f32 / 48.0)
                    }
                };
                if self.state.view_input_config().style() == "TrackpadViewInConf"
                    && config_bool(&self.state, "cuemol2.ui.mouse-multitouch-pad", false)
                    && !self.ctrl_down
                {
                    camera.pan_by(x, y);
                    return x != 0.0 || y != 0.0;
                }
                camera.zoom_by(step);
                step != 0.0
            }
            _ => false,
        }
    }

    fn camera_drag_mode(&self, button: MouseButton) -> Option<CameraDragMode> {
        if button == MouseButton::Left
            && self.ctrl_down
            && config_bool(&self.state, "cuemol2.ui.mouse-emulate-rbutton", false)
        {
            return Some(CameraDragMode::Pan);
        }
        match self.state.view_input_config().style() {
            "CootViewInConf" => match (button, self.ctrl_down, self.shift_down) {
                (MouseButton::Left, true, _) => Some(CameraDragMode::Pan),
                (MouseButton::Left, false, true) | (MouseButton::Right, _, _) => {
                    Some(CameraDragMode::Zoom)
                }
                (MouseButton::Left, false, false) => Some(CameraDragMode::Rotate),
                _ => None,
            },
            "PymolViewInConf" => match button {
                MouseButton::Left => Some(CameraDragMode::Rotate),
                MouseButton::Middle => Some(CameraDragMode::Pan),
                MouseButton::Right => Some(CameraDragMode::Zoom),
                _ => None,
            },
            "IphoneViewInConf" => match (button, self.shift_down) {
                (MouseButton::Left, true) => Some(CameraDragMode::Zoom),
                (MouseButton::Left, false) => Some(CameraDragMode::Rotate),
                (MouseButton::Right, _) => Some(CameraDragMode::Pan),
                _ => None,
            },
            _ => {
                if button == MouseButton::Left && self.shift_down {
                    Some(CameraDragMode::Zoom)
                } else {
                    CameraDragMode::from_button(button)
                }
            }
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

    fn select_atoms_in_screen_rect(
        &mut self,
        camera: &CameraController,
        selection_rect: egui::Rect,
    ) -> usize {
        let Some(viewport) = self.viewport_rect else {
            return 0;
        };
        let selection_rect = selection_rect.intersect(viewport);
        if !selection_rect.is_positive() {
            return 0;
        }
        let mut selected_by_object = std::collections::BTreeMap::<u64, Vec<i32>>::new();
        for atom in self.state.active_pick_atoms("").unwrap_or_default() {
            let (Some(object_uid), Some(atom_index)) = (atom.object_uid, atom.atom_index) else {
                continue;
            };
            let Some((screen, _)) =
                camera.project_to_viewport(vec3_from_vector(atom.center), viewport)
            else {
                continue;
            };
            if !selection_rect.contains(screen) {
                continue;
            }
            let object_uid = cuemol_core::Uid::new(object_uid);
            if let Ok(summary) = self.state.atom_summary(object_uid, atom_index) {
                selected_by_object
                    .entry(object_uid.get())
                    .or_default()
                    .push(summary.serial);
            }
        }
        if selected_by_object.is_empty() {
            self.set_status("No atoms in rectangle");
            return 0;
        }

        let undo = self.capture_undo();
        let mut primary = None;
        let mut total = 0;
        for (object_uid, mut serials) in selected_by_object {
            serials.sort_unstable();
            serials.dedup();
            total += serials.len();
            let selection = atom_id_selection(&serials);
            let object_uid = cuemol_core::Uid::new(object_uid);
            if let Err(error) = self
                .state
                .set_molecule_selection_text(object_uid, &selection)
            {
                self.state = undo.state;
                self.set_error(error);
                return 0;
            }
            primary.get_or_insert((object_uid, selection));
        }
        self.push_undo_snapshot(undo);
        if let Some((object_uid, selection)) = primary {
            self.selected_object = Some(object_uid);
            self.molstruct_object = Some(object_uid);
            self.selected_renderer = None;
            self.workspace_aux_selection = None;
            self.selection_text = selection;
            self.push_selection_history();
        }
        self.rebuild_snapshot();
        self.set_status(format!("Selected {total} atoms in rectangle"));
        total
    }

    fn pick_atom_at(&mut self, camera: &CameraController, position: egui::Pos2) -> bool {
        let picker_target = self
            .bond_picker_target
            .filter(|_| self.tool_dialog.as_deref() == Some("Mol bond editor"));
        let hit = if picker_target.is_some() {
            self.pick_hit_with_object(camera, position, self.bond_target_object_uid())
        } else {
            self.pick_hit(camera, position)
        };
        let Some(hit) = hit else {
            return false;
        };
        let object_uid = cuemol_core::Uid::new(hit.object_uid);
        match self.state.atom_summary(object_uid, hit.atom_index) {
            Ok(atom) => {
                if let Some(target) = picker_target {
                    if Some(atom.object_uid) != self.bond_target_object_uid() {
                        return false;
                    }
                    if target == 0 {
                        self.bond_aid1 = atom.serial.to_string();
                    } else {
                        self.bond_aid2 = atom.serial.to_string();
                    }
                    self.set_status(format!("Picked atom {} for bond editor", atom.serial));
                    return true;
                }
                let status = format!(
                    "Selected atom {} {}{}:{}",
                    atom.serial, atom.residue_name, atom.residue_id, atom.name
                );
                if self.apply_molecule_selection(
                    atom.object_uid,
                    atom.selection_text,
                    "Selected atom",
                ) {
                    self.set_status(status);
                    true
                } else {
                    false
                }
            }
            Err(error) => {
                self.set_error(error);
                false
            }
        }
    }

    fn pick_hit(&self, camera: &CameraController, position: egui::Pos2) -> Option<PickHit> {
        self.pick_hit_with_object(camera, position, None)
    }

    fn pick_hit_with_object(
        &self,
        camera: &CameraController,
        position: egui::Pos2,
        object_filter: Option<cuemol_core::Uid>,
    ) -> Option<PickHit> {
        let viewport = self.viewport_rect?;
        let mut best: Option<(f32, PickHit)> = None;
        let selection = if object_filter.is_some() {
            ""
        } else {
            &self.selection_text
        };
        let atoms = self
            .state
            .active_pick_atoms(selection)
            .ok()
            .filter(|atoms| !atoms.is_empty())
            .unwrap_or_else(|| self.snapshot.atoms.clone());
        for atom in &atoms {
            let object_uid = atom.object_uid?;
            if object_filter.is_some_and(|filter| filter.get() != object_uid) {
                continue;
            }
            let atom_index = atom.atom_index?;
            let center = vec3_from_vector(atom.center);
            let (screen, depth) = camera.project_to_viewport(center, viewport)?;
            let radius = camera
                .projected_radius(atom.radius, depth, viewport)
                .max(self.state.view_input_config().hitprec() as f32);
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
    name: String,
    target: Vec3,
    /// Accumulated trackball orientation (object-to-camera rotation).
    ///
    /// Replaces the previous yaw/pitch Euler angles, which suffered from gimbal
    /// behavior and a clamped pitch. Mirrors the original CueMol2 `View::m_rotQuat`
    /// accumulation in `qsys/View.cpp`.
    orientation: Quat,
    distance: f32,
    ortho_scale: f32,
    slab_depth: f32,
    perspective: bool,
    trans_mms: bool,
    rot_mms: bool,
    stereo_mode: StereoMode,
    stereo_distance: f32,
    swap_stereo_eyes: bool,
    center_mark: CenterMark,
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
            name: "__current".to_string(),
            target: Vec3::ZERO,
            orientation: Quat::IDENTITY,
            distance: 120.0,
            ortho_scale: 40.0,
            slab_depth: 240.0,
            perspective: true,
            trans_mms: false,
            rot_mms: false,
            stereo_mode: StereoMode::None,
            stereo_distance: 1.0,
            swap_stereo_eyes: false,
            center_mark: CenterMark::Cross,
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
        self.slab_depth = (radius * 4.0).max(50.0);
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
    fn trackball_move(
        &mut self,
        cur_x: f64,
        cur_y: f64,
        prev_x: f64,
        prev_y: f64,
        trackball_radius: f64,
    ) {
        let trackball_radius = trackball_radius.clamp(0.1, 10.0);
        let x1 = 2.0 * prev_x - 1.0;
        let y1 = 1.0 - 2.0 * prev_y;
        let x2 = 2.0 * cur_x - 1.0;
        let y2 = 1.0 - 2.0 * cur_y;
        let p1 = project_trackball(x1, y1, trackball_radius);
        let p2 = project_trackball(x2, y2, trackball_radius);
        let axis = p2.cross(p1);
        let axis_len = axis.length();
        if !axis_len.is_finite() || axis_len < 1.0e-7 {
            return;
        }
        let axis = axis / axis_len;
        let chord = (p1 - p2).length() as f64;
        let t = (chord / (2.0 * trackball_radius)).clamp(-1.0, 1.0);
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
        let half_slab = self.slab_depth.max(0.1) * 0.5;
        let near = (self.distance - half_slab).max(0.01);
        let far = (self.distance + self.slab_depth.max(0.1)).max(near + 0.01);
        let proj = if self.perspective {
            Mat4::perspective_rh(45.0_f32.to_radians(), aspect, near, far)
        } else {
            Mat4::orthographic_rh(
                -self.ortho_scale * aspect,
                self.ortho_scale * aspect,
                -self.ortho_scale,
                self.ortho_scale,
                near,
                far,
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
            "cuemol2-rs-view\t3\n\
name\t{}\n\
target\t{:.6}\t{:.6}\t{:.6}\n\
rotation\t{:.6}\t{:.6}\t{:.6}\t{:.6}\n\
distance\t{:.6}\n\
ortho_scale\t{:.6}\n\
slab\t{:.6}\n\
perspective\t{}\n\
trans_mms\t{}\n\
rot_mms\t{}\n\
stereo_mode\t{}\n\
stereo_distance\t{:.6}\n\
swap_stereo_eyes\t{}\n\
center_mark\t{}\n",
            self.name,
            self.target.x,
            self.target.y,
            self.target.z,
            self.orientation.x,
            self.orientation.y,
            self.orientation.z,
            self.orientation.w,
            self.distance,
            self.ortho_scale,
            self.slab_depth,
            self.perspective,
            self.trans_mms,
            self.rot_mms,
            stereo_mode_name(self.stereo_mode),
            self.stereo_distance,
            self.swap_stereo_eyes,
            center_mark_name(self.center_mark),
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
        camera.set_slab_depth(self.slab_depth as f64);
        camera.set_zoom((1000.0 / self.ortho_scale.max(0.1)) as f64);
        camera.set_perspective(self.perspective);
        camera.set_stereo_mode(self.stereo_mode);
        camera.set_stereo_distance(self.stereo_distance as f64);
        camera.set_center_mark(qsys_center_mark(self.center_mark));
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
            name: nonempty_or(camera.name().to_string(), "__current"),
            target: Vec3::new(center.x as f32, center.y as f32, center.z as f32),
            orientation: Quat::from_xyzw(q.x as f32, q.y as f32, q.z as f32, q.w as f32)
                .normalize(),
            distance: camera.cam_dist() as f32,
            ortho_scale,
            slab_depth: camera.slab_depth().max(0.1) as f32,
            perspective: camera.is_perspective(),
            stereo_mode: camera.stereo_mode(),
            stereo_distance: camera.stereo_distance().max(0.0) as f32,
            center_mark: center_mark_from_qsys(camera.center_mark()),
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
            if let Some(name) = line.strip_prefix("name\t") {
                camera.name = name.to_string();
                continue;
            }
            let parts = trimmed.split_whitespace().collect::<Vec<_>>();
            match parts.as_slice() {
                ["cuemol2-rs-view", "1" | "2" | "3"] => saw_header = true,
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
                ["slab", value] => camera.slab_depth = parse_f32(value)?.clamp(0.1, 100_000.0),
                ["perspective", value] => {
                    camera.perspective = parse_view_bool(value)?;
                }
                ["trans_mms", value] => camera.trans_mms = parse_view_bool(value)?,
                ["rot_mms", value] => camera.rot_mms = parse_view_bool(value)?,
                ["stereo_mode", value] => camera.stereo_mode = parse_stereo_mode(value)?,
                ["stereo_distance", value] => camera.stereo_distance = parse_f32(value)?.max(0.0),
                ["swap_stereo_eyes", value] => camera.swap_stereo_eyes = parse_view_bool(value)?,
                ["center_mark", value] => camera.center_mark = parse_center_mark(value)?,
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

fn scene_attr_bool(value: Option<&str>, fallback: bool) -> bool {
    value.map_or(fallback, |value| matches!(value, "true" | "1"))
}

fn value_is_true(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

fn native_renderer_specific_defaults(
    renderer_type: &str,
) -> &'static [(&'static str, &'static str)] {
    match renderer_type {
        "spline" => &[
            ("axialdetail", "6"),
            ("smooth", "0"),
            ("smoothcolor", "true"),
            ("pivotatom", ""),
        ],
        "*namelabel" | "name_label" => &[
            ("maxlabel", "50"),
            ("dispx", "0"),
            ("dispy", "0"),
            ("color", "#ffff00"),
            ("font_size", "12"),
            ("font_name", "sans-serif"),
            ("font_style", "normal"),
            ("font_weight", "normal"),
        ],
        "*selection" | "selection" => &[
            ("mode", "stick"),
            ("linew", "3"),
            ("dispx", "3"),
            ("dispy", "3"),
            ("color", "rgba(0,255,255,0.5)"),
        ],
        "atomintr" => &[
            ("mode", "fancy"),
            ("showlabel", "false"),
            ("detail", "5"),
            ("stipple0", "1"),
            ("stipple1", "1"),
            ("stipple2", "-1"),
            ("stipple3", "-1"),
            ("stipple4", "-1"),
            ("stipple5", "-1"),
            ("width", "0.1"),
            ("color", "#ffff00"),
            ("captype_start", "sphere"),
            ("captype_end", "sphere"),
        ],
        "anisou" => &[
            ("drawdisc", "true"),
            ("discscale", "1.1"),
            ("discthick", "0.1"),
            ("maxverts", "1000000"),
        ],
        "cpk" => &[
            ("vdwr_H", "1.2"),
            ("vdwr_C", "1.7"),
            ("vdwr_N", "1.55"),
            ("vdwr_O", "1.52"),
            ("vdwr_S", "1.8"),
            ("vdwr_P", "1.8"),
            ("vdwr_X", "1.7"),
            ("detail", "10"),
            ("glrender_mode", "default"),
        ],
        "disorder" => &[
            ("target", ""),
            ("detail", "5"),
            ("width", "0.3"),
            ("dotsep", "1"),
            ("loopsize", "2"),
            ("loopsize2", "-1"),
            ("defaultcolor", "#ffffff"),
        ],
        "ribbon" => &[
            ("helix.type", "elliptical"),
            ("helix.width", "0.35"),
            ("helix.tuber", "1"),
            ("helix.sharp", "0.4"),
            ("helix.detail", "16"),
            ("sheet.type", "elliptical"),
            ("sheet.width", "0.35"),
            ("sheet.tuber", "1"),
            ("sheet.sharp", "0.4"),
            ("sheet.detail", "16"),
            ("coil.type", "elliptical"),
            ("coil.width", "0.35"),
            ("coil.tuber", "1"),
            ("coil.sharp", "0.4"),
            ("coil.detail", "16"),
            ("helixhead.type", "smooth"),
            ("helixhead.gamma", "2.2"),
            ("helixhead.basw", "0"),
            ("helixhead.arrow", "1.8"),
            ("helixtail.type", "smooth"),
            ("helixtail.gamma", "2.2"),
            ("helixtail.basw", "0"),
            ("helixtail.arrow", "1.8"),
            ("sheethead.type", "smooth"),
            ("sheethead.gamma", "2.2"),
            ("sheethead.basw", "0"),
            ("sheethead.arrow", "1.8"),
            ("sheettail.type", "smooth"),
            ("sheettail.gamma", "2.2"),
            ("sheettail.basw", "0"),
            ("sheettail.arrow", "1.8"),
            ("axialdetail", "6"),
            ("smoothcolor", "true"),
            ("pivotatom", ""),
            ("start_captype", "sphere"),
            ("end_captype", "sphere"),
            ("segend_fade", "false"),
            ("helix_smooth", "0"),
            ("sheet_smooth", "0.5"),
            ("coil_smooth", "0"),
            ("helix_usebackcol", "false"),
            ("helix_backcol", "#ffffff"),
            ("sheet_usesidecol", "false"),
            ("sheet_sidecol", "#ffffff"),
        ],
        "cartoon" => &[
            ("sheet.type", "elliptical"),
            ("sheet.width", "0.35"),
            ("sheet.tuber", "1"),
            ("sheet.sharp", "0.4"),
            ("sheet.detail", "16"),
            ("coil.type", "elliptical"),
            ("coil.width", "0.35"),
            ("coil.tuber", "1"),
            ("coil.sharp", "0.4"),
            ("coil.detail", "16"),
            ("sheethead.type", "smooth"),
            ("sheethead.gamma", "2.2"),
            ("sheethead.basw", "0"),
            ("sheethead.arrow", "1.8"),
            ("ribhelix.type", "elliptical"),
            ("ribhelix.width", "0.35"),
            ("ribhelix.tuber", "1"),
            ("ribhelix.sharp", "0.4"),
            ("ribhelix.detail", "16"),
            ("ribhelix_head.type", "smooth"),
            ("ribhelix_head.gamma", "2.2"),
            ("ribhelix_head.basw", "0"),
            ("ribhelix_head.arrow", "1.8"),
            ("ribhelix_tail.type", "smooth"),
            ("ribhelix_tail.gamma", "2.2"),
            ("ribhelix_tail.basw", "0"),
            ("ribhelix_tail.arrow", "1.8"),
            ("axialdetail", "8"),
            ("smoothcolor", "false"),
            ("pivotatom", ""),
            ("start_captype", "flat"),
            ("end_captype", "flat"),
            ("segend_fade", "false"),
            ("anchor_sel", ""),
            ("anchor_weight", "10"),
            ("helix_ribbon", "false"),
            ("helix_smooth", "3"),
            ("helix_width_mode", "average"),
            ("helix_width", "2.3"),
            ("helix_extend", "0.5"),
            ("helix_wsmooth", "5"),
            ("helix_wplus", "0.2"),
            ("sheet_smooth", "2"),
            ("sheet_wsmooth", "5"),
            ("coil_smooth", "-1"),
        ],
        "tube" => &[
            ("section.type", "elliptical"),
            ("section.width", "0.35"),
            ("section.tuber", "1"),
            ("section.sharp", "0.4"),
            ("section.detail", "16"),
            ("axialdetail", "6"),
            ("smooth", "0"),
            ("smoothcolor", "true"),
            ("pivotatom", ""),
            ("start_captype", "sphere"),
            ("end_captype", "sphere"),
            ("segend_fade", "false"),
            ("putty_mode", "none"),
            ("putty_tgt", "bfac"),
            ("putty_hiscl", "3"),
            ("putty_loscl", "3"),
        ],
        "nucl" => &[
            ("section.type", "elliptical"),
            ("section.width", "0.35"),
            ("section.tuber", "1"),
            ("section.sharp", "0.4"),
            ("section.detail", "16"),
            ("axialdetail", "6"),
            ("smooth", "0"),
            ("smoothcolor", "true"),
            ("pivotatom", ""),
            ("start_captype", "sphere"),
            ("end_captype", "sphere"),
            ("segend_fade", "false"),
            ("putty_mode", "none"),
            ("putty_tgt", "bfac"),
            ("putty_hiscl", "3"),
            ("putty_loscl", "3"),
            ("show_tube", "true"),
            ("show_basepair", "true"),
            ("base_size", "0.5"),
            ("base_detail", "3"),
            ("base_thick", "0.5"),
            ("base_type", "basepair"),
        ],
        "contour" | "gpu_mapmesh" => &[
            ("autoupdate", "true"),
            ("dragupdate", "false"),
            ("width", "1"),
            ("bufsize", "100"),
            ("use_pbc", "true"),
            ("bndry_molname", ""),
            ("bndry_sel", "all"),
            ("bndry_rng", "5"),
        ],
        "isosurf" => &[
            ("autoupdate", "true"),
            ("dragupdate", "false"),
            ("drawmode", "fill"),
            ("width", "1.2"),
            ("max_grids", "100"),
            ("cullface", "false"),
            ("use_pbc", "true"),
            ("bndry_molname", ""),
            ("bndry_sel", "all"),
            ("bndry_rng", "5"),
        ],
        "gpu_mapvol" => &[
            ("autoupdate", "true"),
            ("dragupdate", "false"),
            ("xferfn_type", "rainbow1"),
            ("use_pbc", "true"),
            ("bndry_molname", ""),
            ("bndry_sel", "all"),
            ("bndry_rng", "5"),
        ],
        "surface" | "molsurf" => &[
            ("drawmode", "fill"),
            ("width", "1.2"),
            ("target", ""),
            ("showsel", "*"),
            ("colormode", "solid"),
        ],
        "dsurface" | "directsurf" | "dsurf2" => &[
            ("drawmode", "fill"),
            ("width", "1.2"),
            ("surftype", "ses"),
            ("detail", "6"),
            ("target", ""),
            ("showsel", "*"),
            ("colormode", "molecule"),
        ],
        _ => &[],
    }
}

fn renderer_value_is_default(value: &str, default: &str) -> bool {
    let value = value.trim();
    let default = default.trim();
    match (value.parse::<f64>(), default.parse::<f64>()) {
        (Ok(value), Ok(default)) => (value - default).abs() < f64::EPSILON,
        _ => value.eq_ignore_ascii_case(default),
    }
}

fn validate_native_renderer_properties(
    draft: &NativeRendererProperties,
) -> std::result::Result<(), String> {
    let name = draft.name.trim();
    if (name.is_empty() && !matches!(draft.renderer_type.as_str(), "*selection" | "selection"))
        || name.contains(':')
    {
        return Err("renderer name must be non-empty and cannot contain ':'".to_string());
    }
    if !draft.alpha.is_finite() || draft.alpha <= 0.0 || draft.alpha > 1.0 {
        return Err("opacity must be between 0 and 1".to_string());
    }
    if !draft.edge_width.is_finite() || !(0.0..=1.0).contains(&draft.edge_width) {
        return Err("edge width must be between 0 and 1".to_string());
    }
    if !matches!(draft.edge_type.as_str(), "none" | "edges" | "silhouette") {
        return Err(format!("unsupported edge type: {}", draft.edge_type));
    }
    validate_renderer_color(&draft.edge_color, "edge color")?;
    if matches!(draft.renderer_type.as_str(), "simple" | "trace" | "spline")
        && (!draft.line_width.is_finite() || draft.line_width <= 0.0 || draft.line_width > 10.0)
    {
        return Err("line width must be greater than 0 and at most 10".to_string());
    }
    if matches!(draft.renderer_type.as_str(), "ballstick" | "anisou") {
        if !(2..=20).contains(&draft.detail) {
            return Err("detail must be between 2 and 20".to_string());
        }
        for (label, value) in [
            ("bond width", draft.bond_width),
            ("atom radius", draft.sphere_radius),
            ("ring thickness", draft.ring_thickness),
        ] {
            if !value.is_finite() || !(0.0..=10.0).contains(&value) {
                return Err(format!("{label} must be between 0 and 10"));
            }
        }
        validate_renderer_color(&draft.ring_color, "ring color")?;
    }

    match draft.renderer_type.as_str() {
        "spline" => {
            validate_renderer_integer(draft, "axialdetail", "6", "axial detail", 1, 100)?;
            validate_renderer_number(draft, "smooth", "0", "smoothing", -1000.0, 1000.0)?;
            if draft.specific_value("pivotatom", "").len() > 16 {
                return Err("pivot atom must be at most 16 characters".to_string());
            }
        }
        "*namelabel" | "name_label" => {
            validate_renderer_integer(draft, "maxlabel", "50", "maximum labels", 0, 1_000_000)?;
            validate_renderer_number(draft, "dispx", "0", "X displacement", -10_000.0, 10_000.0)?;
            validate_renderer_number(draft, "dispy", "0", "Y displacement", -10_000.0, 10_000.0)?;
            validate_renderer_number(draft, "font_size", "12", "font size", 1.0, 512.0)?;
            validate_renderer_color(&draft.specific_value("color", "#ffff00"), "label color")?;
            for (key, label) in [
                ("font_name", "font name"),
                ("font_style", "font style"),
                ("font_weight", "font weight"),
            ] {
                let value = draft.specific_value(key, "normal");
                if value.trim().is_empty() || value.len() > 128 {
                    return Err(format!("{label} must contain 1 to 128 characters"));
                }
            }
        }
        "*selection" | "selection" => {
            validate_renderer_choice(draft, "mode", "stick", &["stick", "point"])?;
            validate_renderer_number(draft, "linew", "3", "line width", 0.0, 100.0)?;
            validate_renderer_number(draft, "dispx", "3", "X displacement", -10_000.0, 10_000.0)?;
            validate_renderer_number(draft, "dispy", "3", "Y displacement", -10_000.0, 10_000.0)?;
            validate_renderer_color(
                &draft.specific_value("color", "rgba(0,255,255,0.5)"),
                "selection color",
            )?;
        }
        "atomintr" => {
            validate_renderer_choice(draft, "mode", "fancy", &["simple", "fancy"])?;
            validate_renderer_integer(draft, "detail", "5", "detail", 0, 20)?;
            validate_renderer_number(draft, "width", "0.1", "width", 0.0, 10.0)?;
            validate_renderer_color(
                &draft.specific_value("color", "#ffff00"),
                "interaction color",
            )?;
            for key in [
                "stipple0", "stipple1", "stipple2", "stipple3", "stipple4", "stipple5",
            ] {
                validate_renderer_number(draft, key, "-1", key, -1.0, 100.0)?;
            }
            for key in ["captype_start", "captype_end"] {
                validate_renderer_choice(draft, key, "sphere", &["flat", "sphere", "arrow"])?;
            }
        }
        "anisou" => {
            validate_renderer_number(draft, "discscale", "1.1", "disc scale", 0.0, 10.0)?;
            validate_renderer_number(draft, "discthick", "0.1", "disc thickness", 0.0, 10.0)?;
            validate_renderer_integer(
                draft,
                "maxverts",
                "1000000",
                "maximum vertices",
                1,
                100_000_000,
            )?;
        }
        "cpk" => {
            for (key, label, fallback) in [
                ("vdwr_H", "hydrogen radius", "1.2"),
                ("vdwr_C", "carbon radius", "1.7"),
                ("vdwr_N", "nitrogen radius", "1.55"),
                ("vdwr_O", "oxygen radius", "1.52"),
                ("vdwr_S", "sulfur radius", "1.8"),
                ("vdwr_P", "phosphorus radius", "1.8"),
                ("vdwr_X", "other radius", "1.7"),
            ] {
                validate_renderer_number(draft, key, fallback, label, 0.0, 3.0)?;
            }
            validate_renderer_integer(draft, "detail", "10", "detail", 2, 20)?;
            validate_renderer_choice(
                draft,
                "glrender_mode",
                "default",
                &["default", "shader", "vbo", "glu"],
            )?;
        }
        "disorder" => {
            validate_renderer_integer(draft, "detail", "5", "detail", 2, 20)?;
            for (key, label, fallback, minimum, maximum) in [
                ("width", "dot size", "0.3", 0.0, 3.0),
                ("dotsep", "dot separation", "1", 0.0, 3.0),
                ("loopsize", "loop size", "2", 0.0, 10.0),
                ("loopsize2", "loop size 2", "-1", -1.0, 10.0),
            ] {
                validate_renderer_number(draft, key, fallback, label, minimum, maximum)?;
            }
            validate_renderer_color(
                &draft.specific_value("defaultcolor", "#ffffff"),
                "disorder color",
            )?;
        }
        "ribbon" | "cartoon" => {
            let cartoon = draft.renderer_type == "cartoon";
            let sections = if cartoon {
                &["sheet", "coil", "ribhelix"][..]
            } else {
                &["helix", "sheet", "coil"][..]
            };
            for section in sections {
                validate_renderer_choice(
                    draft,
                    &format!("{section}.type"),
                    "elliptical",
                    &["elliptical", "roundsquare", "rectangle", "fancy1"],
                )?;
                validate_renderer_integer(
                    draft,
                    &format!("{section}.detail"),
                    "16",
                    &format!("{section} detail"),
                    2,
                    20,
                )?;
                for (suffix, label, fallback, minimum, maximum) in [
                    ("width", "width", "0.35", 0.001, 10.0),
                    ("tuber", "width ratio", "1", 0.1, 10.0),
                    ("sharp", "sharpness", "0.4", 0.0, 1.0),
                ] {
                    validate_renderer_number(
                        draft,
                        &format!("{section}.{suffix}"),
                        fallback,
                        &format!("{section} {label}"),
                        minimum,
                        maximum,
                    )?;
                }
            }
            let junctions = if cartoon {
                &["sheethead", "ribhelix_head", "ribhelix_tail"][..]
            } else {
                &["helixhead", "helixtail", "sheethead", "sheettail"][..]
            };
            for junction in junctions {
                validate_renderer_choice(
                    draft,
                    &format!("{junction}.type"),
                    "smooth",
                    &["smooth", "flat", "arrow"],
                )?;
                for (suffix, fallback, minimum, maximum) in [
                    ("gamma", "2.2", 0.1, 10.0),
                    ("basw", "0", 0.0, 1.0),
                    ("arrow", "1.8", 1.0, 3.0),
                ] {
                    validate_renderer_number(
                        draft,
                        &format!("{junction}.{suffix}"),
                        fallback,
                        &format!("{junction} {suffix}"),
                        minimum,
                        maximum,
                    )?;
                }
            }
            validate_renderer_integer(
                draft,
                "axialdetail",
                if cartoon { "8" } else { "6" },
                "axial detail",
                2,
                20,
            )?;
            for key in ["smoothcolor", "segend_fade"] {
                validate_renderer_bool(
                    draft,
                    key,
                    if key == "smoothcolor" {
                        !cartoon
                    } else {
                        false
                    },
                )?;
            }
            for key in ["start_captype", "end_captype"] {
                validate_renderer_choice(
                    draft,
                    key,
                    if cartoon { "flat" } else { "sphere" },
                    &["flat", "sphere", "none"],
                )?;
            }
            if cartoon {
                validate_renderer_bool(draft, "helix_ribbon", false)?;
                validate_renderer_choice(
                    draft,
                    "helix_width_mode",
                    "average",
                    &["const", "average", "wavy"],
                )?;
                for (key, fallback, minimum, maximum) in [
                    ("anchor_weight", "10", 0.0, 20.0),
                    ("helix_smooth", "3", -5.0, 5.0),
                    ("helix_width", "2.3", 0.001, 10.0),
                    ("helix_extend", "0.5", 0.0, 3.0),
                    ("helix_wsmooth", "5", -5.0, 5.0),
                    ("helix_wplus", "0.2", -2.0, 3.0),
                    ("sheet_smooth", "2", -5.0, 5.0),
                    ("sheet_wsmooth", "5", -5.0, 5.0),
                    ("coil_smooth", "-1", -5.0, 5.0),
                ] {
                    validate_renderer_number(draft, key, fallback, key, minimum, maximum)?;
                }
            } else {
                for (key, fallback) in [
                    ("helix_smooth", "0"),
                    ("sheet_smooth", "0.5"),
                    ("coil_smooth", "0"),
                ] {
                    validate_renderer_number(draft, key, fallback, key, 0.0, 5.0)?;
                }
                validate_renderer_bool(draft, "helix_usebackcol", false)?;
                validate_renderer_bool(draft, "sheet_usesidecol", false)?;
                validate_renderer_color(
                    &draft.specific_value("helix_backcol", "#ffffff"),
                    "helix back-face color",
                )?;
                validate_renderer_color(
                    &draft.specific_value("sheet_sidecol", "#ffffff"),
                    "sheet side-face color",
                )?;
            }
        }
        "tube" | "nucl" => {
            validate_renderer_choice(
                draft,
                "section.type",
                "elliptical",
                &["elliptical", "roundsquare", "rectangle", "fancy1"],
            )?;
            validate_renderer_integer(draft, "section.detail", "16", "section detail", 2, 20)?;
            validate_renderer_integer(draft, "axialdetail", "6", "axial detail", 2, 20)?;
            for (key, fallback, minimum, maximum) in [
                ("section.width", "0.35", f64::EPSILON, 10.0),
                ("section.tuber", "1", f64::EPSILON, 100.0),
                ("section.sharp", "0.4", 0.0, 1.0),
                ("smooth", "0", 0.0, 1.0),
                ("putty_hiscl", "3", 0.01, 10.0),
                ("putty_loscl", "3", 0.01, 10.0),
            ] {
                validate_renderer_number(draft, key, fallback, key, minimum, maximum)?;
            }
            for key in ["smoothcolor", "segend_fade"] {
                validate_renderer_bool(draft, key, key == "smoothcolor")?;
            }
            for key in ["start_captype", "end_captype"] {
                validate_renderer_choice(draft, key, "sphere", &["flat", "sphere", "none"])?;
            }
            validate_renderer_choice(draft, "putty_mode", "none", &["none", "linear1", "scale1"])?;
            validate_renderer_choice(draft, "putty_tgt", "bfac", &["bfac", "occ"])?;
            if draft.renderer_type == "nucl" {
                validate_renderer_bool(draft, "show_tube", true)?;
                validate_renderer_bool(draft, "show_basepair", true)?;
                validate_renderer_number(draft, "base_size", "0.5", "base size", 0.0, 10.0)?;
                validate_renderer_number(draft, "base_thick", "0.5", "base thickness", 0.0, 10.0)?;
                validate_renderer_integer(draft, "base_detail", "3", "base detail", 2, 20)?;
                validate_renderer_choice(
                    draft,
                    "base_type",
                    "basepair",
                    &["basepair", "simple1", "detail1", "detail2"],
                )?;
            }
        }
        "contour" | "gpu_mapmesh" | "isosurf" | "gpu_mapvol" => {
            for key in ["autoupdate", "dragupdate", "use_pbc"] {
                validate_renderer_bool(draft, key, key != "dragupdate")?;
            }
            validate_renderer_number(draft, "bndry_rng", "5", "boundary distance", 0.1, 100.0)?;
            match draft.renderer_type.as_str() {
                "contour" | "gpu_mapmesh" => {
                    validate_renderer_number(draft, "width", "1", "line width", 0.1, 10.0)?;
                    validate_renderer_integer(draft, "bufsize", "100", "buffer size", 50, 200)?;
                }
                "isosurf" => {
                    validate_renderer_choice(
                        draft,
                        "drawmode",
                        "fill",
                        &["fill", "line", "point"],
                    )?;
                    validate_renderer_number(draft, "width", "1.2", "line width", 0.1, 10.0)?;
                    validate_renderer_integer(
                        draft,
                        "max_grids",
                        "100",
                        "maximum grid size",
                        10,
                        2000,
                    )?;
                    validate_renderer_bool(draft, "cullface", false)?;
                }
                "gpu_mapvol" => validate_renderer_choice(
                    draft,
                    "xferfn_type",
                    "rainbow1",
                    &["rainbow1", "auto2", "multigrad"],
                )?,
                _ => {}
            }
        }
        "surface" | "molsurf" | "dsurface" | "directsurf" | "dsurf2" => {
            let direct = !matches!(draft.renderer_type.as_str(), "surface" | "molsurf");
            validate_renderer_choice(draft, "drawmode", "fill", &["fill", "line", "point"])?;
            validate_renderer_number(draft, "width", "1.2", "surface width", 0.1, 10.0)?;
            validate_renderer_choice(
                draft,
                "colormode",
                if direct { "molecule" } else { "solid" },
                if direct {
                    &["molecule", "potential"]
                } else {
                    &["solid", "molecule", "potential", "multigrad"]
                },
            )?;
            if direct {
                validate_renderer_choice(draft, "surftype", "ses", &["vdw", "sas", "ses"])?;
                validate_renderer_integer(draft, "detail", "6", "surface detail", 1, 21)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_renderer_number(
    draft: &NativeRendererProperties,
    key: &str,
    fallback: &str,
    label: &str,
    minimum: f64,
    maximum: f64,
) -> std::result::Result<(), String> {
    let raw = draft.specific_value(key, fallback);
    let value = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("{label} must be a number"))?;
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return Err(format!("{label} must be between {minimum} and {maximum}"));
    }
    Ok(())
}

fn validate_renderer_integer(
    draft: &NativeRendererProperties,
    key: &str,
    fallback: &str,
    label: &str,
    minimum: i32,
    maximum: i32,
) -> std::result::Result<(), String> {
    let raw = draft.specific_value(key, fallback);
    let value = raw
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("{label} must be an integer"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{label} must be between {minimum} and {maximum}"));
    }
    Ok(())
}

fn validate_renderer_choice(
    draft: &NativeRendererProperties,
    key: &str,
    fallback: &str,
    choices: &[&str],
) -> std::result::Result<(), String> {
    let value = draft.specific_value(key, fallback);
    if !choices.contains(&value.trim()) {
        return Err(format!("unsupported {key}: {value}"));
    }
    Ok(())
}

fn validate_renderer_bool(
    draft: &NativeRendererProperties,
    key: &str,
    fallback: bool,
) -> std::result::Result<(), String> {
    let value = draft.specific_value(key, if fallback { "true" } else { "false" });
    if !matches!(value.trim(), "true" | "false") {
        return Err(format!("{key} must be true or false"));
    }
    Ok(())
}

fn validate_renderer_color(value: &str, label: &str) -> std::result::Result<(), String> {
    cuemol_gfx::parse_color(value.trim())
        .map(|_| ())
        .map_err(|_| format!("invalid {label}: {value}"))
}

fn scene_attr_f64(value: Option<&str>, fallback: f64) -> f64 {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn scene_attr_i32(value: Option<&str>, fallback: i32) -> i32 {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn set_native_scene_attr_default(
    scene: &mut cuemol_molstr::MolScene,
    name: &str,
    value: &str,
    default_value: &str,
) {
    if value == default_value {
        scene.remove_scene_attribute(name);
    } else {
        scene.set_scene_attribute(name, value);
    }
}

fn icc_intent_label(value: &str) -> &str {
    match value {
        "relative_colorimetric" => "relative colorimetric",
        "absolute_colorimetric" => "absolute colorimetric",
        _ => value,
    }
}

fn scene_property_value(properties: &NativeSceneProperties, key: ScenePropertyKey) -> String {
    use ScenePropertyKey as Key;
    match key {
        Key::Uid => properties.uid.to_string(),
        Key::Modified => properties.modified.to_string(),
        Key::Name => properties.name.clone(),
        Key::Source => properties.source.clone(),
        Key::SourceType => properties.source_type.clone(),
        Key::BasePath => properties.base_path.clone(),
        Key::Background => properties.background.clone(),
        Key::AoEnabled => properties.ao_enabled.to_string(),
        Key::AoRadius => format!("{:.6}", properties.ao_radius),
        Key::AoIntensity => format!("{:.6}", properties.ao_intensity),
        Key::AoSlices => properties.ao_slices.to_string(),
        Key::AoSteps => properties.ao_steps.to_string(),
        Key::AoHalfRes => properties.ao_half_res.to_string(),
        Key::AaMethod => properties.aa_method.clone(),
        Key::AaJitterLevel => properties.aa_jitter_level.to_string(),
        Key::Onload => properties.onload.clone(),
        Key::ObjectUids => properties.object_uids.clone(),
        Key::ActiveObjectId => properties.active_object_id.to_string(),
        Key::ActiveRendererId => properties.active_renderer_id.to_string(),
        Key::ViewUids => properties.view_uids.clone(),
        Key::ActiveViewId => properties.active_view_id.to_string(),
        Key::UseColorProof => properties.use_color_proof.to_string(),
        Key::IccFilename => properties.icc_filename.clone(),
        Key::IccIntent => properties.icc_intent.clone(),
    }
}

fn scene_property_is_default(properties: &NativeSceneProperties, key: ScenePropertyKey) -> bool {
    use ScenePropertyKey as Key;
    match key {
        Key::Background => properties.background == "black",
        Key::AoEnabled => !properties.ao_enabled,
        Key::AoRadius => approx_property(properties.ao_radius, 4.0),
        Key::AoIntensity => approx_property(properties.ao_intensity, 2.2),
        Key::AoSlices => properties.ao_slices == 9,
        Key::AoSteps => properties.ao_steps == 3,
        Key::AoHalfRes => !properties.ao_half_res,
        Key::AaMethod => properties.aa_method == "fxaa",
        Key::AaJitterLevel => properties.aa_jitter_level == 0,
        Key::Onload => properties.onload.is_empty(),
        Key::UseColorProof => !properties.use_color_proof,
        Key::IccFilename => properties.icc_filename.is_empty(),
        Key::IccIntent => properties.icc_intent == "perceptual",
        _ => false,
    }
}

fn reset_scene_property(properties: &mut NativeSceneProperties, key: ScenePropertyKey) {
    use ScenePropertyKey as Key;
    match key {
        Key::Background => properties.background = "black".to_string(),
        Key::AoEnabled => properties.ao_enabled = false,
        Key::AoRadius => properties.ao_radius = 4.0,
        Key::AoIntensity => properties.ao_intensity = 2.2,
        Key::AoSlices => properties.ao_slices = 9,
        Key::AoSteps => properties.ao_steps = 3,
        Key::AoHalfRes => properties.ao_half_res = false,
        Key::AaMethod => properties.aa_method = "fxaa".to_string(),
        Key::AaJitterLevel => properties.aa_jitter_level = 0,
        Key::Onload => properties.onload.clear(),
        Key::UseColorProof => properties.use_color_proof = false,
        Key::IccFilename => properties.icc_filename.clear(),
        Key::IccIntent => properties.icc_intent = "perceptual".to_string(),
        _ => {}
    }
}

fn stereo_mode_name(mode: StereoMode) -> &'static str {
    match mode {
        StereoMode::None => "none",
        StereoMode::Parallel => "para",
        StereoMode::Cross => "cross",
        StereoMode::Hardware => "hardware",
    }
}

fn parse_stereo_mode(value: &str) -> Result<StereoMode> {
    match value {
        "none" => Ok(StereoMode::None),
        "para" | "parallel" => Ok(StereoMode::Parallel),
        "cross" => Ok(StereoMode::Cross),
        "hardware" | "hw" => Ok(StereoMode::Hardware),
        _ => Err(CueMolError::parse(format!("invalid stereo mode: {value}"))),
    }
}

fn center_mark_name(mark: CenterMark) -> &'static str {
    match mark {
        CenterMark::None => "none",
        CenterMark::Cross => "crosshair",
        CenterMark::Axis => "axis",
    }
}

fn parse_center_mark(value: &str) -> Result<CenterMark> {
    match value {
        "none" => Ok(CenterMark::None),
        "cross" | "crosshair" => Ok(CenterMark::Cross),
        "axis" => Ok(CenterMark::Axis),
        _ => Err(CueMolError::parse(format!("invalid center mark: {value}"))),
    }
}

fn qsys_center_mark(mark: CenterMark) -> cuemol_qsys::CenterMark {
    match mark {
        CenterMark::None => cuemol_qsys::CenterMark::None,
        CenterMark::Cross => cuemol_qsys::CenterMark::Cross,
        CenterMark::Axis => cuemol_qsys::CenterMark::Axis,
    }
}

fn center_mark_from_qsys(mark: cuemol_qsys::CenterMark) -> CenterMark {
    match mark {
        cuemol_qsys::CenterMark::None => CenterMark::None,
        cuemol_qsys::CenterMark::Cross => CenterMark::Cross,
        cuemol_qsys::CenterMark::Axis => CenterMark::Axis,
    }
}

fn parse_view_bool(value: &str) -> Result<bool> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(CueMolError::parse(format!("invalid boolean: {value}"))),
    }
}

fn view_property_vectors(properties: &NativeViewProperties) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let rotation = normalized_or_identity(Quat::from_xyzw(
        properties.rotation[0] as f32,
        properties.rotation[1] as f32,
        properties.rotation[2] as f32,
        properties.rotation[3] as f32,
    ));
    let right = (rotation * Vec3::X).to_array().map(f64::from);
    let up = (rotation * Vec3::Y).to_array().map(f64::from);
    let forward = (rotation * -Vec3::Z).to_array().map(f64::from);
    (up, right, forward)
}

fn view_property_value(properties: &NativeViewProperties, key: ViewPropertyKey) -> String {
    use ViewPropertyKey as Key;
    let (up, right, forward) = view_property_vectors(properties);
    match key {
        Key::Name => properties.name.clone(),
        Key::Uid => properties.uid.to_string(),
        Key::Active => properties.active.to_string(),
        Key::Zoom => format!("{:.6}", properties.zoom),
        Key::Distance => format!("{:.6}", properties.distance),
        Key::Slab => format!("{:.6}", properties.slab),
        Key::Center => format_vector(properties.center),
        Key::CenterX => format!("{:.6}", properties.center[0]),
        Key::CenterY => format!("{:.6}", properties.center[1]),
        Key::CenterZ => format!("{:.6}", properties.center[2]),
        Key::Rotation => format_quaternion(properties.rotation),
        Key::RotationX => format!("{:.6}", properties.rotation[0]),
        Key::RotationY => format!("{:.6}", properties.rotation[1]),
        Key::RotationZ => format!("{:.6}", properties.rotation[2]),
        Key::RotationW => format!("{:.6}", properties.rotation[3]),
        Key::Perspective => properties.perspective.to_string(),
        Key::Width => properties.width.to_string(),
        Key::Height => properties.height.to_string(),
        Key::TransMms => properties.trans_mms.to_string(),
        Key::RotMms => properties.rot_mms.to_string(),
        Key::StereoMode => stereo_mode_name(properties.stereo_mode).to_string(),
        Key::HasHwStereo => properties.has_hw_stereo.to_string(),
        Key::StereoDist => format!("{:.6}", properties.stereo_dist),
        Key::SwapStereoEyes => properties.swap_stereo_eyes.to_string(),
        Key::UpVec => format_vector(up),
        Key::UpVecX => format!("{:.6}", up[0]),
        Key::UpVecY => format!("{:.6}", up[1]),
        Key::UpVecZ => format!("{:.6}", up[2]),
        Key::RightVec => format_vector(right),
        Key::RightVecX => format!("{:.6}", right[0]),
        Key::RightVecY => format!("{:.6}", right[1]),
        Key::RightVecZ => format!("{:.6}", right[2]),
        Key::ForwardVec => format_vector(forward),
        Key::ForwardVecX => format!("{:.6}", forward[0]),
        Key::ForwardVecY => format!("{:.6}", forward[1]),
        Key::ForwardVecZ => format!("{:.6}", forward[2]),
        Key::CenterMark => center_mark_name(properties.center_mark).to_string(),
    }
}

fn format_vector(value: [f64; 3]) -> String {
    format!("({:.6}, {:.6}, {:.6})", value[0], value[1], value[2])
}

fn format_quaternion(value: [f64; 4]) -> String {
    format!(
        "({:.6}, {:.6}, {:.6}, {:.6})",
        value[0], value[1], value[2], value[3]
    )
}

fn view_property_is_default(properties: &NativeViewProperties, key: ViewPropertyKey) -> bool {
    use ViewPropertyKey as Key;
    match key {
        Key::Name => properties.name.is_empty(),
        Key::Zoom => approx_property(properties.zoom, 50.0),
        Key::Distance => approx_property(properties.distance, 200.0),
        Key::Slab => approx_property(properties.slab, 50.0),
        Key::CenterX => approx_property(properties.center[0], 0.0),
        Key::CenterY => approx_property(properties.center[1], 0.0),
        Key::CenterZ => approx_property(properties.center[2], 0.0),
        Key::RotationX => approx_property(properties.rotation[0], 0.0),
        Key::RotationY => approx_property(properties.rotation[1], 0.0),
        Key::RotationZ => approx_property(properties.rotation[2], 0.0),
        Key::RotationW => approx_property(properties.rotation[3], 1.0),
        Key::Perspective => !properties.perspective,
        Key::TransMms => !properties.trans_mms,
        Key::RotMms => !properties.rot_mms,
        Key::StereoMode => properties.stereo_mode == StereoMode::None,
        Key::StereoDist => approx_property(properties.stereo_dist, 1.0),
        Key::SwapStereoEyes => !properties.swap_stereo_eyes,
        Key::CenterMark => properties.center_mark == CenterMark::Cross,
        _ => false,
    }
}

fn reset_view_property(properties: &mut NativeViewProperties, key: ViewPropertyKey) {
    use ViewPropertyKey as Key;
    match key {
        Key::Name => properties.name.clear(),
        Key::Zoom => properties.zoom = 50.0,
        Key::Distance => properties.distance = 200.0,
        Key::Slab => properties.slab = 50.0,
        Key::CenterX => properties.center[0] = 0.0,
        Key::CenterY => properties.center[1] = 0.0,
        Key::CenterZ => properties.center[2] = 0.0,
        Key::RotationX => properties.rotation[0] = 0.0,
        Key::RotationY => properties.rotation[1] = 0.0,
        Key::RotationZ => properties.rotation[2] = 0.0,
        Key::RotationW => properties.rotation[3] = 1.0,
        Key::Perspective => properties.perspective = false,
        Key::TransMms => properties.trans_mms = false,
        Key::RotMms => properties.rot_mms = false,
        Key::StereoMode => properties.stereo_mode = StereoMode::None,
        Key::StereoDist => properties.stereo_dist = 1.0,
        Key::SwapStereoEyes => properties.swap_stereo_eyes = false,
        Key::CenterMark => properties.center_mark = CenterMark::Cross,
        _ => {}
    }
}

fn approx_property(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1.0e-9
}

fn tool_status_text(_title: &str) -> &'static str {
    "No additional action is available."
}

const OBJECT_OPEN_FILTERS: &[NativeFileFilter] = &[
    NativeFileFilter {
        label: "Molecule coordinates",
        extensions: &[
            "pdb", "ent", "cif", "mmcif", "sdf", "mol", "mol2", "gro", "psf", "prmtop", "parm7",
            "top", "pse", "qdf",
        ],
    },
    NativeFileFilter {
        label: "Density maps",
        extensions: &["mtz", "dx", "ccp4", "mrc", "cns", "brix", "omap", "map"],
    },
    NativeFileFilter {
        label: "Molecular surfaces",
        extensions: &["obj", "ply", "face", "vert"],
    },
];
const SCENE_OPEN_FILTERS: &[NativeFileFilter] = &[NativeFileFilter {
    label: "CueMol scene",
    extensions: &["qsc", "qsl"],
}];
const OBJECT_SAVE_FILTERS: &[NativeFileFilter] = &[
    NativeFileFilter {
        label: "Protein Data Bank",
        extensions: &["pdb", "ent"],
    },
    NativeFileFilter {
        label: "CueMol QDF molecule",
        extensions: &["qdf"],
    },
    NativeFileFilter {
        label: "MDL structure data",
        extensions: &["sdf", "mol"],
    },
];
const SCENE_SAVE_FILTERS: &[NativeFileFilter] = &[NativeFileFilter {
    label: "CueMol scene",
    extensions: &["qsc"],
}];
const VIEW_SAVE_FILTERS: &[NativeFileFilter] = &[NativeFileFilter {
    label: "Camera setting",
    extensions: &["cam"],
}];
const STYLE_FILE_FILTERS: &[NativeFileFilter] = &[NativeFileFilter {
    label: "Style file",
    extensions: &["xml"],
}];
const EXPORT_FILTERS: &[NativeFileFilter] = &[
    NativeFileFilter {
        label: "POV-Ray scene",
        extensions: &["pov"],
    },
    NativeFileFilter {
        label: "PNG image",
        extensions: &["png"],
    },
];
const SCRIPT_OPEN_FILTERS: &[NativeFileFilter] = &[NativeFileFilter {
    label: "Internal JavaScript",
    extensions: &["js"],
}];
const LOG_SAVE_FILTERS: &[NativeFileFilter] = &[NativeFileFilter {
    label: "Text log",
    extensions: &["log", "txt"],
}];

fn native_file_dialog_spec(dialog: NativeFileDialog) -> NativeFileDialogSpec {
    match dialog {
        NativeFileDialog::OpenObject => NativeFileDialogSpec {
            title: "Select a File",
            filters: OBJECT_OPEN_FILTERS,
            default_extension: None,
            save: false,
        },
        NativeFileDialog::OpenScene => NativeFileDialogSpec {
            title: "Open Scene",
            filters: SCENE_OPEN_FILTERS,
            default_extension: None,
            save: false,
        },
        NativeFileDialog::OpenCamera => NativeFileDialogSpec {
            title: "Load camera from file",
            filters: VIEW_SAVE_FILTERS,
            default_extension: None,
            save: false,
        },
        NativeFileDialog::OpenStyle => NativeFileDialogSpec {
            title: "Open style file",
            filters: STYLE_FILE_FILTERS,
            default_extension: None,
            save: false,
        },
        NativeFileDialog::SaveObject => NativeFileDialogSpec {
            title: "Save File As",
            filters: OBJECT_SAVE_FILTERS,
            default_extension: Some("pdb"),
            save: true,
        },
        NativeFileDialog::SaveScene => NativeFileDialogSpec {
            title: "Save Scene As",
            filters: SCENE_SAVE_FILTERS,
            default_extension: Some("qsc"),
            save: true,
        },
        NativeFileDialog::SaveView => NativeFileDialogSpec {
            title: "Save camera to file",
            filters: VIEW_SAVE_FILTERS,
            default_extension: Some("cam"),
            save: true,
        },
        NativeFileDialog::SaveCamera => NativeFileDialogSpec {
            title: "Save camera to file",
            filters: VIEW_SAVE_FILTERS,
            default_extension: Some("cam"),
            save: true,
        },
        NativeFileDialog::SaveStyle => NativeFileDialogSpec {
            title: "Save style to file",
            filters: STYLE_FILE_FILTERS,
            default_extension: Some("xml"),
            save: true,
        },
        NativeFileDialog::ExportScene => NativeFileDialogSpec {
            title: "Select a file to export",
            filters: EXPORT_FILTERS,
            default_extension: Some("pov"),
            save: true,
        },
        NativeFileDialog::OpenScript => NativeFileDialogSpec {
            title: "Select a File",
            filters: SCRIPT_OPEN_FILTERS,
            default_extension: None,
            save: false,
        },
        NativeFileDialog::SaveLog => NativeFileDialogSpec {
            title: "Save log contents",
            filters: LOG_SAVE_FILTERS,
            default_extension: Some("log"),
            save: true,
        },
        NativeFileDialog::MergeObject => NativeFileDialogSpec {
            title: "Select a molecule to merge",
            filters: OBJECT_OPEN_FILTERS,
            default_extension: None,
            save: false,
        },
    }
}

fn native_file_dialog(dialog: NativeFileDialog, initial: &str) -> Option<PathBuf> {
    let spec = native_file_dialog_spec(dialog);
    let mut picker = rfd::FileDialog::new().set_title(spec.title);
    let initial = initial.trim();
    if !initial.is_empty() && !is_network_url(initial) {
        let path = Path::new(initial);
        if path.is_dir() {
            picker = picker.set_directory(path);
        } else {
            if let Some(parent) = path.parent().filter(|parent| parent.is_dir()) {
                picker = picker.set_directory(parent);
            }
            if let Some(name) = path.file_name().filter(|name| !name.is_empty()) {
                picker = picker.set_file_name(name.to_string_lossy());
            }
        }
    }
    for filter in spec.filters {
        picker = picker.add_filter(filter.label, filter.extensions);
    }
    let path = if spec.save {
        picker.save_file()
    } else {
        picker.pick_file()
    }?;
    Some(apply_default_extension(path, spec))
}

fn apply_default_extension(mut path: PathBuf, spec: NativeFileDialogSpec) -> PathBuf {
    if spec.save && path.extension().is_none() {
        if let Some(extension) = spec.default_extension {
            path.set_extension(extension);
        }
    }
    path
}

fn atom_id_selection(serials: &[i32]) -> String {
    let mut serials = serials.to_vec();
    serials.sort_unstable();
    serials.dedup();
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < serials.len() {
        let start = serials[index];
        let mut end = start;
        while index + 1 < serials.len() && end.checked_add(1) == Some(serials[index + 1]) {
            index += 1;
            end = serials[index];
        }
        if start == end {
            ranges.push(start.to_string());
        } else {
            ranges.push(format!("{start}:{end}"));
        }
        index += 1;
    }
    format!("aid {}", ranges.join(","))
}

fn add_selection_expression(current: &str, addition: &str) -> String {
    let current = current.trim();
    if current.is_empty() {
        addition.trim().to_string()
    } else {
        format!("({current}) or ({})", addition.trim())
    }
}

fn invert_selection_expression(current: &str) -> String {
    let current = current.trim();
    if current.is_empty() {
        return "*".to_string();
    }
    if let Some(inner) = current
        .strip_prefix("!(")
        .and_then(|inner| inner.strip_suffix(')'))
    {
        inner.trim().to_string()
    } else {
        format!("!({current})")
    }
}

fn toggle_sidechain_expression(current: &str) -> String {
    let current = current.trim();
    current
        .strip_prefix("bysidech ")
        .map(str::trim)
        .map(str::to_string)
        .unwrap_or_else(|| format!("bysidech {current}"))
}

fn around_selection_expression(current: &str, radius: f64, by_residue: bool) -> String {
    let current = current.trim();
    let mut base = current;
    if let Some((candidate, distance)) = current.rsplit_once(" around ") {
        if distance.trim().parse::<f64>().is_ok() {
            base = candidate.trim();
            if let Some(without_byres) = base.strip_prefix("byres ") {
                base = without_byres.trim();
            }
            if let Some(inner) = base
                .strip_prefix('(')
                .and_then(|inner| inner.strip_suffix(')'))
            {
                base = inner.trim();
            }
        }
    }
    let base = if by_residue {
        format!("byres ({base})")
    } else {
        format!("({base})")
    };
    format!("{base} around {radius}")
}

fn default_animation_render_concurrency() -> u32 {
    std::thread::available_parallelism()
        .map(|count| count.get().min(8) as u32)
        .unwrap_or(2)
}

fn animation_render_frame_count(
    objects: &[AnimObject],
    fps: u32,
    duplicate_last_frame: bool,
) -> u32 {
    let duration = objects
        .iter()
        .map(|object| object.duration.max(0.01))
        .sum::<f64>();
    let intervals = (duration * f64::from(fps.max(1)))
        .round()
        .clamp(1.0, f64::from(u32::MAX - 1)) as u32;
    intervals + u32::from(duplicate_last_frame)
}

fn sanitize_animation_base_name(value: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "output".to_string()
    } else {
        sanitized
    }
}

fn animation_movie_format_label(format: &str) -> &str {
    ANIMATION_MOVIE_FORMATS
        .iter()
        .find(|(value, _)| *value == format)
        .map_or(format, |(_, label)| *label)
}

fn animation_movie_extension(format: &str) -> &'static str {
    match format {
        "mov_h264" | "mov_h265" | "mov_raw" => "mov",
        "wmv2" => "wmv",
        "gifanim" => "gif",
        _ => "mp4",
    }
}

fn animation_frame_path(
    settings: &AnimationRenderSettings,
    index: u32,
    extension: &str,
) -> PathBuf {
    PathBuf::from(settings.output_dir.trim())
        .join(format!("{}_{index:04}.{extension}", settings.base_name))
}

fn animation_frame_labels(
    state: &DesktopAppState,
    settings: &AnimationRenderSettings,
    camera: &cuemol_qsys::Camera,
) -> Vec<GuiAtomLabel> {
    if settings.backend != "POV-Ray"
        || !settings.pov_labels
        || settings.width == 0
        || settings.height == 0
    {
        return Vec::new();
    }
    const MAX_LABELS: usize = 10_000;
    let aspect = f64::from(settings.width) / f64::from(settings.height);
    let mvp = crate::offscreen::camera_mvp(camera, aspect);
    let mut labels = state.active_name_labels(
        mvp,
        settings.width as f32,
        settings.height as f32,
        MAX_LABELS,
    );
    if labels.len() < MAX_LABELS {
        labels.extend(state.active_atom_intr_labels(
            mvp,
            settings.width as f32,
            settings.height as f32,
            MAX_LABELS - labels.len(),
        ));
    }
    labels
}

fn overlay_animation_labels(
    path: &Path,
    labels: &[GuiAtomLabel],
) -> std::result::Result<(), String> {
    let mut frame = image::open(path)
        .map_err(|error| format!("failed to read label target {}: {error}", path.display()))?
        .to_rgba8();
    for label in labels {
        draw_animation_label(&mut frame, label);
    }
    frame
        .save(path)
        .map_err(|error| format!("failed to write labelled frame {}: {error}", path.display()))
}

fn draw_animation_label(frame: &mut image::RgbaImage, label: &GuiAtomLabel) {
    let monospace = label.font_name.to_ascii_lowercase().contains("mono");
    let font_data = if monospace {
        epaint_default_fonts::HACK_REGULAR
    } else {
        epaint_default_fonts::UBUNTU_LIGHT
    };
    let font = FontRef::try_from_slice(font_data).expect("embedded label font is valid");
    let scale = label.font_size.clamp(1.0, 512.0) as f32;
    let scaled = font.as_scaled(scale);
    let line_height = (scaled.height() + scaled.line_gap()).max(1.0);
    let color = cuemol_gfx::parse_color(&label.color)
        .ok()
        .and_then(|color| color.rgba8())
        .unwrap_or(Rgba8 {
            r: 255,
            g: 255,
            b: 0,
            a: 255,
        });
    let style = label.font_style.to_ascii_lowercase();
    let italic = style.contains("italic") || style.contains("oblique");
    let weight = label.font_weight.trim().to_ascii_lowercase();
    let bold = weight.contains("bold")
        || weight
            .parse::<u16>()
            .is_ok_and(|numeric_weight| numeric_weight >= 600);
    let mut caret_x = label.x;
    let mut baseline_y = label.y + scaled.descent();
    let mut previous = None;
    for character in label.text.chars() {
        if character == '\n' {
            caret_x = label.x;
            baseline_y += line_height;
            previous = None;
            continue;
        }
        let glyph_id = font.glyph_id(character);
        if let Some(previous) = previous {
            caret_x += scaled.kern(previous, glyph_id);
        }
        let glyph = glyph_id.with_scale_and_position(scale, point(caret_x, baseline_y));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|x, y, coverage| {
                let italic_offset = if italic {
                    ((bounds.height() - y as f32) * 0.18).round() as i32
                } else {
                    0
                };
                let pixel_x = bounds.min.x as i32 + x as i32 + italic_offset;
                let pixel_y = bounds.min.y as i32 + y as i32;
                blend_animation_label_pixel(frame, pixel_x, pixel_y, color, coverage);
                if bold {
                    blend_animation_label_pixel(frame, pixel_x + 1, pixel_y, color, coverage);
                }
            });
        }
        caret_x += scaled.h_advance(glyph_id);
        previous = Some(glyph_id);
    }
}

fn blend_animation_label_pixel(
    frame: &mut image::RgbaImage,
    x: i32,
    y: i32,
    color: Rgba8,
    coverage: f32,
) {
    if x < 0 || y < 0 || x >= frame.width() as i32 || y >= frame.height() as i32 {
        return;
    }
    let pixel = frame.get_pixel_mut(x as u32, y as u32);
    let source_alpha = coverage.clamp(0.0, 1.0) * f32::from(color.a) / 255.0;
    let target_alpha = f32::from(pixel[3]) / 255.0;
    let output_alpha = source_alpha + target_alpha * (1.0 - source_alpha);
    if output_alpha <= f32::EPSILON {
        return;
    }
    for (target, source) in pixel.0[..3].iter_mut().zip([color.r, color.g, color.b]) {
        let target_value = f32::from(*target) / 255.0;
        let source_value = f32::from(source) / 255.0;
        let output = (source_value * source_alpha
            + target_value * target_alpha * (1.0 - source_alpha))
            / output_alpha;
        *target = (output * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = (output_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn update_animation_render_progress(task: &AnimationFrameTask, frame_progress: f64) {
    let frames_weight = if task.settings.make_movie { 0.95 } else { 1.0 };
    let total = f64::from(task.total_frames.max(1));
    let completed = f64::from(task.index) + frame_progress.clamp(0.0, 1.0);
    let progress = ((completed / total) * frames_weight * 1_000_000.0)
        .round()
        .clamp(0.0, 1_000_000.0) as u32;
    task.progress.fetch_max(progress, Ordering::Relaxed);
}

fn spawn_animation_frame_task(
    task: AnimationFrameTask,
) -> mpsc::Receiver<std::result::Result<AnimationTaskOutput, String>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = render_animation_frame(&task);
        let _ = sender.send(result);
    });
    receiver
}

fn render_animation_frame(
    task: &AnimationFrameTask,
) -> std::result::Result<AnimationTaskOutput, String> {
    if task.cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }
    if task.settings.backend == "Umbreon" {
        render_umbreon_animation_frame(task)
    } else {
        render_pov_animation_frame(task)
    }
}

fn render_umbreon_animation_frame(
    task: &AnimationFrameTask,
) -> std::result::Result<AnimationTaskOutput, String> {
    let path = animation_frame_path(&task.settings, task.index, "png");
    let mut exporter = UmbreonSceneExporter::new();
    exporter
        .handler_mut()
        .set_path(path.to_string_lossy().into_owned());
    exporter.set_width(task.settings.width.min(i32::MAX as u32) as i32);
    exporter.set_height(task.settings.height.min(i32::MAX as u32) as i32);
    exporter.set_denoise_mode(task.denoise);
    exporter.set_render_input(task.snapshot.clone(), task.camera.clone(), task.background);
    exporter.begin_render().map_err(|error| error.to_string())?;
    while !exporter.is_render_done() {
        if task.cancel.load(Ordering::Relaxed) {
            exporter.cancel_render();
        }
        update_animation_render_progress(task, exporter.get_render_progress());
        std::thread::sleep(Duration::from_millis(20));
    }
    let frame = exporter.end_render().map_err(|error| error.to_string())?;
    if frame.is_none() || task.cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }
    update_animation_render_progress(task, 1.0);
    Ok(AnimationTaskOutput::Frame {
        index: task.index,
        path,
        log: "Umbreon frame complete".to_string(),
    })
}

fn render_pov_animation_frame(
    task: &AnimationFrameTask,
) -> std::result::Result<AnimationTaskOutput, String> {
    let pov_path = animation_frame_path(&task.settings, task.index, "pov");
    let png_path = animation_frame_path(&task.settings, task.index, "png");
    let mesh = tessellate_snapshot(&task.snapshot);
    let mut exporter = PovSceneExporter::new();
    exporter.set_use_clip_z(task.settings.pov_clip_plane);
    exporter.set_use_post_blend(task.settings.pov_post_blend);
    exporter.set_show_edge_lines(task.settings.pov_edge_lines);
    exporter
        .handler_mut()
        .set_path(pov_path.to_string_lossy().into_owned());
    exporter
        .write_scene_file_with_camera(&mesh, &task.camera)
        .map_err(|error| error.to_string())?;
    if task.cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }
    update_animation_render_progress(task, 0.25);

    let mut child = Command::new(task.settings.povray_executable.trim())
        .args(animation_povray_args(&task.settings, &pov_path, &png_path))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to start POV-Ray: {error}"))?;
    let status = loop {
        if task.cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("cancelled".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(25));
                update_animation_render_progress(task, 0.5);
            }
            Err(error) => return Err(format!("POV-Ray wait failed: {error}")),
        }
    };
    if !status.success() {
        return Err(format!("POV-Ray exited with {status}"));
    }
    if !png_path.is_file() {
        return Err(format!(
            "POV-Ray did not create {}",
            png_path.to_string_lossy()
        ));
    }
    if task.settings.pov_labels && !task.labels.is_empty() {
        overlay_animation_labels(&png_path, &task.labels)?;
    }
    update_animation_render_progress(task, 1.0);
    Ok(AnimationTaskOutput::Frame {
        index: task.index,
        path: png_path,
        log: format!("POV-Ray exited with {status}"),
    })
}

fn animation_povray_args(
    settings: &AnimationRenderSettings,
    pov_path: &Path,
    png_path: &Path,
) -> Vec<String> {
    let mut args = vec![
        format!("+I{}", pov_path.to_string_lossy()),
        format!("+O{}", png_path.to_string_lossy()),
        format!("+W{}", settings.width),
        format!("+H{}", settings.height),
        "+FN8".to_string(),
        "+UA".to_string(),
        "-D".to_string(),
        "+WT1".to_string(),
        "Quality=11".to_string(),
        "Antialias=On".to_string(),
        "Antialias_Depth=3".to_string(),
        "Antialias_Threshold=0.1".to_string(),
        "Jitter=Off".to_string(),
        "File_Gamma=1".to_string(),
        format!("Declare=_shadow={}", u8::from(settings.pov_shadow)),
    ];
    if !settings.povray_include_dir.trim().is_empty() {
        args.push(format!("+L{}", settings.povray_include_dir.trim()));
    }
    if settings.pov_radiosity >= 0 {
        args.push(format!("Declare=_radiosity={}", settings.pov_radiosity));
    }
    args
}

fn spawn_animation_movie_task(
    job: &AnimationRenderJob,
) -> std::result::Result<mpsc::Receiver<std::result::Result<AnimationTaskOutput, String>>, String> {
    if job.frame_paths.len() != job.total_frames as usize
        || job.frame_paths.iter().any(|path| {
            path.extension()
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("png"))
        })
    {
        return Err("Movie encoding requires a complete PNG frame sequence".to_string());
    }
    let task = AnimationMovieTask {
        settings: job.settings.clone(),
        cancel: Arc::clone(&job.cancel),
        progress: Arc::clone(&job.progress),
    };
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = encode_animation_movie(&task);
        let _ = sender.send(result);
    });
    Ok(receiver)
}

fn encode_animation_movie(
    task: &AnimationMovieTask,
) -> std::result::Result<AnimationTaskOutput, String> {
    let extension = animation_movie_extension(&task.settings.movie_format);
    let output_dir = PathBuf::from(task.settings.output_dir.trim());
    let output_file = format!("{}.{}", task.settings.base_name, extension);
    let output_path = output_dir.join(&output_file);
    let input_pattern = format!("{}_%04d.png", task.settings.base_name);
    let mut command = Command::new(task.settings.ffmpeg_executable.trim());
    command
        .current_dir(&output_dir)
        .args(["-y", "-framerate"])
        .arg(task.settings.fps.to_string())
        .args(["-start_number", "0", "-i"])
        .arg(input_pattern);
    match task.settings.movie_format.as_str() {
        "mov_h265" | "mp4_h265" => {
            command.args(["-c:v", "libx265", "-pix_fmt", "yuv420p"]);
        }
        "mov_raw" => {
            command.args(["-c:v", "rawvideo"]);
        }
        "wmv2" => {
            command.args(["-c:v", "wmv2"]);
        }
        "gifanim" => {
            command.args(["-loop", "0"]);
        }
        _ => {
            command.args(["-c:v", "libx264", "-pix_fmt", "yuv420p"]);
        }
    }
    if !matches!(task.settings.movie_format.as_str(), "mov_raw" | "gifanim") {
        command
            .arg("-b:v")
            .arg(format!("{}k", task.settings.bitrate_kbps.max(1)));
    }
    command
        .arg(&output_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start FFmpeg: {error}"))?;
    let status = loop {
        if task.cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("cancelled".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(format!("FFmpeg wait failed: {error}")),
        }
    };
    if !status.success() {
        return Err(format!("FFmpeg exited with {status}"));
    }
    if !output_path.is_file() {
        return Err(format!(
            "FFmpeg did not create {}",
            output_path.to_string_lossy()
        ));
    }
    task.progress.store(1_000_000, Ordering::Relaxed);
    Ok(AnimationTaskOutput::Movie {
        path: output_path,
        log: format!("FFmpeg exited with {status}"),
    })
}

struct GpuSceneFrame<'a> {
    snapshot: &'a RenderSceneSnapshot,
    revision: u64,
    background: wgpu::Color,
    viewport: Option<egui::Rect>,
}

struct ReusableGpuBuffer {
    buffer: wgpu::Buffer,
    capacity: wgpu::BufferAddress,
    label: &'static str,
    usage: wgpu::BufferUsages,
}

impl ReusableGpuBuffer {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        Self {
            buffer: create_scene_buffer(device, label, usage, MIN_SCENE_BUFFER_CAPACITY),
            capacity: MIN_SCENE_BUFFER_CAPACITY,
            label,
            usage,
        }
    }

    fn upload<T: bytemuck::Pod>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        values: &[T],
    ) {
        let bytes = bytemuck::cast_slice(values);
        if bytes.is_empty() {
            return;
        }
        let required = bytes.len() as wgpu::BufferAddress;
        if required > self.capacity {
            self.capacity = scene_buffer_capacity(required);
            self.buffer = create_scene_buffer(device, self.label, self.usage, self.capacity);
        }
        queue.write_buffer(&self.buffer, 0, bytes);
    }

    fn slice(&self) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(..)
    }
}

struct GpuBootstrap {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    size: PhysicalSize<u32>,
}

impl GpuBootstrap {
    fn new(window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window).map_err(gui_error)?;
        Ok(Self {
            instance,
            surface,
            size,
        })
    }
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
    atom_instance_buffer: ReusableGpuBuffer,
    atom_instance_count: u32,
    bond_instance_buffer: ReusableGpuBuffer,
    bond_instance_count: u32,
    mesh_pipeline: wgpu::RenderPipeline,
    mesh_vertex_buffer: ReusableGpuBuffer,
    mesh_index_buffer: ReusableGpuBuffer,
    mesh_index_count: u32,
    uploaded_snapshot_revision: Option<u64>,
    egui_renderer: egui_wgpu::Renderer,
}

impl GpuState {
    async fn new(bootstrap: GpuBootstrap) -> Result<Self> {
        let GpuBootstrap {
            instance,
            surface,
            size,
        } = bootstrap;
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
        let atom_instance_buffer =
            ReusableGpuBuffer::new(&device, "atom-instance-buffer", wgpu::BufferUsages::VERTEX);
        let bond_instance_buffer =
            ReusableGpuBuffer::new(&device, "bond-instance-buffer", wgpu::BufferUsages::VERTEX);
        let mesh_pipeline = create_mesh_pipeline(&device, &shader, &camera_layout, config.format);
        let mesh_vertex_buffer =
            ReusableGpuBuffer::new(&device, "mesh-vertex-buffer", wgpu::BufferUsages::VERTEX);
        let mesh_index_buffer =
            ReusableGpuBuffer::new(&device, "mesh-index-buffer", wgpu::BufferUsages::INDEX);
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
            bond_instance_buffer,
            bond_instance_count: 0,
            mesh_pipeline,
            mesh_vertex_buffer,
            mesh_index_buffer,
            mesh_index_count: 0,
            uploaded_snapshot_revision: None,
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
        scene: GpuSceneFrame<'_>,
        paint_jobs: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
        pixels_per_point: f32,
    ) -> std::result::Result<(), wgpu::SurfaceError> {
        if snapshot_upload_required(self.uploaded_snapshot_revision, scene.revision) {
            self.prepare_snapshot(scene.snapshot);
            self.uploaded_snapshot_revision = Some(scene.revision);
        }
        // Confine the 3D scene to the central viewport region (everything but the
        // docked panels), in physical pixels, so the molecule is centred in the
        // visible area and its on-screen position matches the picking projection
        // (which also uses `viewport_rect`). Without this the scene is centred on
        // the whole window and appears shifted once the left dock takes space.
        let (vx, vy, vw, vh) = match scene.viewport {
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
                        load: wgpu::LoadOp::Clear(scene.background),
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
                render_pass.set_vertex_buffer(0, self.mesh_vertex_buffer.slice());
                render_pass
                    .set_index_buffer(self.mesh_index_buffer.slice(), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..self.mesh_index_count, 0, 0..1);
            }
            if self.bond_instance_count > 0 {
                render_pass.set_pipeline(&self.bond_pipeline);
                render_pass.set_vertex_buffer(0, self.atom_quad_buffer.slice(..));
                render_pass.set_vertex_buffer(1, self.bond_instance_buffer.slice());
                render_pass.draw(0..6, 0..self.bond_instance_count);
            }
            if self.atom_instance_count > 0 {
                render_pass.set_pipeline(&self.atom_pipeline);
                render_pass.set_vertex_buffer(0, self.atom_quad_buffer.slice(..));
                render_pass.set_vertex_buffer(1, self.atom_instance_buffer.slice());
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
        self.atom_instance_buffer
            .upload(&self.device, &self.queue, &atoms);

        let bonds = snapshot
            .bonds
            .iter()
            .filter(|bond| bond.screen_width.is_none())
            .map(bond_instance)
            .collect::<Vec<_>>();
        self.bond_instance_count = bonds.len() as u32;
        self.bond_instance_buffer
            .upload(&self.device, &self.queue, &bonds);

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
        self.mesh_vertex_buffer
            .upload(&self.device, &self.queue, &mesh_vertices);
        self.mesh_index_buffer
            .upload(&self.device, &self.queue, &mesh_indices);
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
            buffers: &[atom_quad_layout(), bond_instance_layout()],
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

fn bond_instance_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<BondInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &BOND_INSTANCE_ATTRS,
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

const MIN_SCENE_BUFFER_CAPACITY: wgpu::BufferAddress = 256;

fn snapshot_upload_required(uploaded: Option<u64>, current: u64) -> bool {
    uploaded != Some(current)
}

fn scene_buffer_capacity(required: wgpu::BufferAddress) -> wgpu::BufferAddress {
    required
        .max(MIN_SCENE_BUFFER_CAPACITY)
        .checked_next_power_of_two()
        .unwrap_or(required)
}

fn create_scene_buffer(
    device: &wgpu::Device,
    label: &str,
    usage: wgpu::BufferUsages,
    capacity: wgpu::BufferAddress,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: capacity.max(MIN_SCENE_BUFFER_CAPACITY),
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn atom_instance(atom: &RenderAtom) -> AtomInstance {
    let color = atom_color(atom);
    let edge = edge_gpu_style(atom.edge);
    AtomInstance {
        center_radius: [
            atom.center.x as f32,
            atom.center.y as f32,
            atom.center.z as f32,
            atom.radius.max(0.08),
        ],
        color,
        selected: [if atom.selected { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        edge: [edge.0, edge.1, 0.0, 0.0],
        edge_color: edge.2,
    }
}

fn bond_instance(bond: &RenderBond) -> BondInstance {
    let color = bond_color(bond);
    let end_color = if bond.selected {
        color
    } else {
        bond.end_color
            .as_ref()
            .and_then(cuemol_gfx::ColorSpec::rgba8)
            .map_or(color, |rgba| rgba_to_float(Some(rgba)))
    };
    let edge = edge_gpu_style(bond.edge);
    BondInstance {
        from_radius: [
            bond.from.x as f32,
            bond.from.y as f32,
            bond.from.z as f32,
            bond.radius.max(0.0025),
        ],
        to_edge: [bond.to.x as f32, bond.to.y as f32, bond.to.z as f32, edge.0],
        color,
        end_color,
        edge_color: edge.2,
        edge_mode: [edge.1, 0.0, 0.0, 0.0],
    }
}

fn edge_gpu_style(edge: Option<cuemol_render::RenderEdgeStyle>) -> (f32, f32, [f32; 4]) {
    edge.filter(|edge| edge.width > 0.0)
        .map_or((0.0, 0.0, [0.0, 0.0, 0.0, 1.0]), |edge| {
            (
                edge.width,
                match edge.mode {
                    cuemol_render::RenderEdgeMode::Edges => 1.0,
                    cuemol_render::RenderEdgeMode::Silhouette => 2.0,
                },
                edge.color,
            )
        })
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
        cull_back_faces: vertex.normal.w as f32,
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
    renderer_type.to_string()
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

fn format_nonstandard_bond_atom(atom: &crate::GuiBondAtomSummary) -> String {
    let chain = if atom.chain_id.is_empty() {
        "_"
    } else {
        &atom.chain_id
    };
    let residue_id = atom.insertion_code.map_or_else(
        || atom.residue_id.to_string(),
        |code| format!("{}{code}", atom.residue_id),
    );
    format!(
        "{chain} {} {residue_id} {} (#{})",
        atom.residue_name, atom.atom_name, atom.serial
    )
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

fn download_url_to_cache(url: &str) -> Result<PathBuf> {
    let path = url_cache_path(url);
    download_to_cache(url, &path)
}

fn download_to_cache(url: &str, path: &Path) -> Result<PathBuf> {
    crate::download_to_shared_cache(url, path).map_err(CueMolError::invalid_argument)
}

fn url_cache_path(url: &str) -> PathBuf {
    let basename = url
        .split('?')
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("download.pdb");
    cached_download_path(&format!("cuemol-url-{}", sanitize_filename(basename)))
}

fn cached_download_path(filename: &str) -> PathBuf {
    let persistent = crate::download_cache_path(filename);
    let temporary = std::env::temp_dir().join(filename);
    if fs::metadata(&persistent).is_ok_and(|metadata| metadata.len() > 0) {
        return persistent;
    }
    if fs::metadata(&temporary).is_ok_and(|metadata| metadata.len() > 0) {
        return temporary;
    }
    if persistent
        .parent()
        .is_some_and(|parent| fs::create_dir_all(parent).is_ok())
    {
        persistent
    } else {
        temporary
    }
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
    edge: [f32; 4],
    edge_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BondInstance {
    from_radius: [f32; 4],
    to_edge: [f32; 4],
    color: [f32; 4],
    end_color: [f32; 4],
    edge_color: [f32; 4],
    edge_mode: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshGpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
    cull_back_faces: f32,
}

const ATOM_QUAD_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];
const ATOM_INSTANCE_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    1 => Float32x4,
    2 => Float32x4,
    3 => Float32x4,
    4 => Float32x4,
    5 => Float32x4
];
const BOND_INSTANCE_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    1 => Float32x4,
    2 => Float32x4,
    3 => Float32x4,
    4 => Float32x4,
    5 => Float32x4,
    6 => Float32x4
];
const MESH_VERTEX_ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32];

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
    @location(4) edge: vec4<f32>,
    @location(5) edge_color: vec4<f32>,
};

struct AtomVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) selected: f32,
    @location(3) edge_ratio: f32,
    @location(4) edge_mode: f32,
    @location(5) edge_color: vec4<f32>,
};

@vertex
fn atom_vs(input: AtomVsIn) -> AtomVsOut {
    let center = input.center_radius.xyz;
    let radius = input.center_radius.w;
    let outer_radius = radius + input.edge.x;
    let world = center
        + camera.right.xyz * input.corner.x * outer_radius
        + camera.up.xyz * input.corner.y * outer_radius;
    var output: AtomVsOut;
    output.position = camera.view_proj * vec4<f32>(world, 1.0);
    output.local = input.corner * (outer_radius / radius);
    output.color = input.color;
    output.selected = input.selected.x;
    output.edge_ratio = outer_radius / radius;
    output.edge_mode = input.edge.y;
    output.edge_color = input.edge_color;
    return output;
}

struct PrimitiveFsOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn atom_fs(input: AtomVsOut) -> PrimitiveFsOut {
    let d = dot(input.local, input.local);
    if (d > input.edge_ratio * input.edge_ratio) {
        discard;
    }
    var output: PrimitiveFsOut;
    output.depth = input.position.z;
    if (d > 1.0) {
        output.color = input.edge_color;
        if (input.edge_mode > 1.5) {
            output.depth = 0.99;
        }
        return output;
    }
    let z = sqrt(max(0.0, 1.0 - d));
    let normal = normalize(
        camera.right.xyz * input.local.x + camera.up.xyz * input.local.y
            + normalize(cross(camera.right.xyz, camera.up.xyz)) * z
    );
    let shade = 0.36 + 0.64 * max(dot(normal, normalize(camera.light.xyz)), 0.0);
    output.color = vec4<f32>(input.color.rgb * shade, input.color.a);
    if (input.selected > 0.5 && d > 0.62) {
        output.color = vec4<f32>(0.0, 0.95, 1.0, 1.0);
    }
    return output;
}

struct BondVsIn {
    @location(0) corner: vec2<f32>,
    @location(1) from_radius: vec4<f32>,
    @location(2) to_edge: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) end_color: vec4<f32>,
    @location(5) edge_color: vec4<f32>,
    @location(6) edge_mode: vec4<f32>,
};

struct BondVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_x: f32,
    @location(2) edge_ratio: f32,
    @location(3) edge_mode: f32,
    @location(4) edge_color: vec4<f32>,
    @location(5) perpendicular: vec3<f32>,
};

@vertex
fn bond_vs(input: BondVsIn) -> BondVsOut {
    let start = input.from_radius.xyz;
    let finish = input.to_edge.xyz;
    let radius = input.from_radius.w;
    let outer_radius = radius + input.to_edge.w;
    let axis = finish - start;
    let screen_axis = vec2<f32>(dot(axis, camera.right.xyz), dot(axis, camera.up.xyz));
    var perpendicular = camera.right.xyz;
    if (length(screen_axis) > 0.000001) {
        perpendicular = normalize(
            camera.right.xyz * (-screen_axis.y) + camera.up.xyz * screen_axis.x
        );
    }
    let t = (input.corner.y + 1.0) * 0.5;
    let world = mix(start, finish, t) + perpendicular * input.corner.x * outer_radius;
    var output: BondVsOut;
    output.position = camera.view_proj * vec4<f32>(world, 1.0);
    output.color = mix(input.color, input.end_color, t);
    output.local_x = input.corner.x * (outer_radius / radius);
    output.edge_ratio = outer_radius / radius;
    output.edge_mode = input.edge_mode.x;
    output.edge_color = input.edge_color;
    output.perpendicular = perpendicular;
    return output;
}

@fragment
fn bond_fs(input: BondVsOut) -> PrimitiveFsOut {
    if (abs(input.local_x) > input.edge_ratio) {
        discard;
    }
    var output: PrimitiveFsOut;
    output.depth = input.position.z;
    if (abs(input.local_x) > 1.0) {
        output.color = vec4<f32>(input.edge_color.rgb, input.color.a);
        if (input.edge_mode > 1.5) {
            output.depth = 0.99;
        }
        return output;
    }
    let z = sqrt(max(0.0, 1.0 - input.local_x * input.local_x));
    let front = normalize(cross(camera.right.xyz, camera.up.xyz));
    let normal = normalize(input.perpendicular * input.local_x + front * z);
    let shade = 0.36 + 0.64 * max(dot(normal, normalize(camera.light.xyz)), 0.0);
    output.color = vec4<f32>(input.color.rgb * shade, input.color.a);
    return output;
}

struct MeshVsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) cull_back_faces: f32,
};

struct MeshVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) cull_back_faces: f32,
};

@vertex
fn mesh_vs(input: MeshVsIn) -> MeshVsOut {
    var output: MeshVsOut;
    output.position = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    output.cull_back_faces = input.cull_back_faces;
    return output;
}

@fragment
fn mesh_fs(input: MeshVsOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    if (input.cull_back_faces > 0.5 && !front_facing) {
        discard;
    }
    if (input.cull_back_faces > 1.5) {
        return input.color;
    }
    let n = normalize(input.normal);
    let l = normalize(camera.light.xyz);
    // Two-sided lighting so tube ends and ribbon backs are not left black.
    let diff = abs(dot(n, l));
    let shade = 0.30 + 0.70 * diff;
    return vec4<f32>(input.color.rgb * shade, input.color.a);
}
"#;

fn nonempty_or(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn config_bool(state: &DesktopAppState, key: &str, fallback: bool) -> bool {
    match state.config_value(key).trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => fallback,
    }
}

fn mouse_preset_label(value: &str) -> &str {
    MOUSE_PRESETS
        .iter()
        .find_map(|(preset, label)| (*preset == value).then_some(*label))
        .unwrap_or(value)
}

fn validate_native_preferences(preferences: &NativePreferences) -> std::result::Result<(), String> {
    if preferences.atom_label_font.trim().is_empty() {
        return Err("an atom-label font is required".to_string());
    }
    if !preferences.atom_label_size.is_finite()
        || !(4.0..=144.0).contains(&preferences.atom_label_size)
    {
        return Err("atom-label size must be between 4 and 144".to_string());
    }
    if preferences.atom_label_color.trim().is_empty() {
        return Err("an atom-label color is required".to_string());
    }
    if !matches!(preferences.ui_language.as_str(), "en-US" | "ja") {
        return Err(format!(
            "unsupported UI language: {}",
            preferences.ui_language
        ));
    }
    if !MOUSE_PRESETS
        .iter()
        .any(|(preset, _)| *preset == preferences.mouse_preset)
    {
        return Err(format!(
            "unknown mouse preset: {}",
            preferences.mouse_preset
        ));
    }
    if !preferences.trackball_radius.is_finite()
        || !(0.1..=10.0).contains(&preferences.trackball_radius)
    {
        return Err("trackball radius must be between 0.1 and 10".to_string());
    }
    if !preferences.hit_precision.is_finite() || !(0.0..=100.0).contains(&preferences.hit_precision)
    {
        return Err("pick precision must be between 0 and 100".to_string());
    }
    const IDS: [&str; 2] = ["view-trans-plusx", "view-trans-minusx"];
    const KEYS: [&str; 4] = ["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"];
    if preferences.shortcuts.len() != IDS.len()
        || !IDS.iter().all(|id| {
            preferences
                .shortcuts
                .iter()
                .any(|shortcut| shortcut.id == *id)
        })
    {
        return Err("the complete reference shortcut set is required".to_string());
    }
    for shortcut in &preferences.shortcuts {
        if !KEYS.contains(&shortcut.key.as_str()) {
            return Err(format!("unsupported shortcut key: {}", shortcut.key));
        }
    }
    for (index, shortcut) in preferences.shortcuts.iter().enumerate() {
        if preferences.shortcuts.iter().skip(index + 1).any(|other| {
            shortcut.key == other.key
                && shortcut.ctrl == other.ctrl
                && shortcut.alt == other.alt
                && shortcut.shift == other.shift
        }) {
            return Err(format!("duplicate shortcut: {}", shortcut.key));
        }
    }
    Ok(())
}

fn default_config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CUEMOL2_CONFIG_FILE") {
        return Some(PathBuf::from(path));
    }
    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("CueMol2").join("config.tsv"));
    }
    if cfg!(target_os = "macos") {
        return std::env::var_os("HOME").map(PathBuf::from).map(|path| {
            path.join("Library")
                .join("Application Support")
                .join("CueMol2")
                .join("config.tsv")
        });
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|path| path.join(".config"))
        })
        .map(|path| path.join("cuemol2").join("config.tsv"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_render_helpers_define_frame_sequence_and_output_names() {
        assert!((1..=8).contains(&default_animation_render_concurrency()));
        let objects = vec![
            AnimObject {
                kind: AnimKind::Spin,
                duration: 1.25,
            },
            AnimObject {
                kind: AnimKind::Pause,
                duration: 0.75,
            },
        ];
        assert_eq!(animation_render_frame_count(&objects, 30, false), 60);
        assert_eq!(animation_render_frame_count(&objects, 30, true), 61);
        assert_eq!(
            sanitize_animation_base_name("  demo frame/1  "),
            "demo_frame_1"
        );
        assert_eq!(sanitize_animation_base_name("   "), "output");
        assert_eq!(animation_movie_extension("mov_h265"), "mov");
        assert_eq!(animation_movie_extension("wmv2"), "wmv");
        assert_eq!(animation_movie_extension("gifanim"), "gif");
        assert_eq!(animation_movie_extension("mp4_h264"), "mp4");

        let settings = AnimationRenderSettings {
            output_dir: "frames".to_string(),
            base_name: "demo".to_string(),
            ..Default::default()
        };
        assert_eq!(
            animation_frame_path(&settings, 7, "png"),
            PathBuf::from("frames/demo_0007.png")
        );

        let pov_settings = AnimationRenderSettings {
            width: 800,
            height: 600,
            povray_include_dir: "/tmp/pov include".to_string(),
            pov_shadow: true,
            pov_radiosity: 7,
            ..Default::default()
        };
        let args = animation_povray_args(
            &pov_settings,
            Path::new("frames/demo.pov"),
            Path::new("frames/demo.png"),
        );
        for expected in [
            "+Iframes/demo.pov",
            "+Oframes/demo.png",
            "+W800",
            "+H600",
            "+FN8",
            "+WT1",
            "Quality=11",
            "Antialias_Depth=3",
            "Declare=_shadow=1",
            "+L/tmp/pov include",
            "Declare=_radiosity=7",
        ] {
            assert!(
                args.iter().any(|argument| argument == expected),
                "{expected}"
            );
        }
    }

    #[test]
    fn animation_render_cancel_and_restore_are_scoped_to_animation_state() {
        let mut app = GuiApplication::new();
        let initial_camera = CameraController {
            target: Vec3::new(1.0, 2.0, 3.0),
            ..Default::default()
        };
        let mut camera = initial_camera.clone();
        app.animation_elapsed = 1.25;
        let restore = app.capture_animation_render_restore(&camera);

        app.background = SceneBackground::Black;
        app.animation_elapsed = 9.0;
        camera.rotate_by(20.0, -10.0);
        let job = AnimationRenderJob {
            settings: AnimationRenderSettings::default(),
            restore,
            total_frames: 1,
            next_frame: 1,
            completed_frames: 1,
            frame_paths: Vec::new(),
            frame_receivers: Vec::new(),
            movie_receiver: None,
            progress: Arc::new(AtomicU32::new(0)),
            cancel: Arc::new(AtomicBool::new(false)),
            encoding_started: false,
        };
        app.finish_animation_render(job, &mut camera, "done");
        assert_eq!(app.background, SceneBackground::Black);
        assert_eq!(app.animation_elapsed, 1.25);
        assert!(camera.target.abs_diff_eq(initial_camera.target, 1.0e-6));
        assert!(camera
            .orientation
            .abs_diff_eq(initial_camera.orientation, 1.0e-6));

        let cancel = Arc::new(AtomicBool::new(true));
        let task = AnimationFrameTask {
            settings: AnimationRenderSettings {
                backend: "Umbreon".to_string(),
                ..Default::default()
            },
            index: 0,
            total_frames: 1,
            snapshot: app.snapshot.clone(),
            camera: camera.to_qsys_camera("cancelled"),
            labels: Vec::new(),
            background: app.background.rgba_f32(),
            progress: Arc::new(AtomicU32::new(0)),
            cancel,
            denoise: UmbreonDenoiseMode::None,
        };
        assert_eq!(render_animation_frame(&task).unwrap_err(), "cancelled");
    }

    #[test]
    fn animation_render_umbreon_writes_a_png_frame() {
        let mut app = GuiApplication::new();
        let base_name = format!("cuemol-animation-frame-test-{}", std::process::id());
        let settings = AnimationRenderSettings {
            output_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            base_name,
            width: 16,
            height: 16,
            backend: "Umbreon".to_string(),
            ..Default::default()
        };
        let path = animation_frame_path(&settings, 0, "png");
        let task = AnimationFrameTask {
            settings,
            index: 0,
            total_frames: 1,
            snapshot: app.snapshot.clone(),
            camera: CameraController::default().to_qsys_camera("test"),
            labels: Vec::new(),
            background: SceneBackground::White.rgba_f32(),
            progress: Arc::new(AtomicU32::new(0)),
            cancel: Arc::new(AtomicBool::new(false)),
            denoise: UmbreonDenoiseMode::None,
        };
        let output = render_animation_frame(&task).expect("render frame");
        assert!(matches!(
            output,
            AnimationTaskOutput::Frame { index: 0, .. }
        ));
        let png = fs::read(&path).expect("read rendered PNG");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let ctx = egui::Context::default();
        app.load_animation_preview_texture(&ctx, &path)
            .expect("load animation preview");
        let preview = app
            .animation_preview_texture
            .as_ref()
            .expect("preview texture");
        assert_eq!(preview.path, path);
        assert_eq!(preview.size, egui::vec2(16.0, 16.0));
        fs::remove_file(path).expect("remove rendered PNG");
    }

    #[test]
    fn animation_render_label_overlay_rasterizes_styled_text() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-animation-label-test-{}.png",
            std::process::id()
        ));
        image::RgbaImage::new(160, 80)
            .save(&path)
            .expect("write label target");
        let label = GuiAtomLabel {
            x: 10.0,
            y: 60.0,
            text: "CueMol".to_string(),
            color: "#ff0000".to_string(),
            font_size: 24.0,
            font_name: "sans-serif".to_string(),
            font_style: "italic".to_string(),
            font_weight: "bold".to_string(),
        };

        overlay_animation_labels(&path, &[label]).expect("overlay label");

        let rendered = image::open(&path).expect("read labelled frame").to_rgba8();
        let red_pixels = rendered
            .pixels()
            .filter(|pixel| pixel[0] > 0 && pixel[1] == 0 && pixel[2] == 0 && pixel[3] > 0)
            .count();
        assert!(red_pixels > 30, "expected rasterized red label pixels");
        fs::remove_file(path).expect("remove labelled frame");
    }

    #[test]
    fn animation_render_schedules_bounded_parallel_workers() {
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        let base_name = format!("cuemol-animation-parallel-test-{}", std::process::id());
        app.animation_objects = vec![AnimObject {
            kind: AnimKind::Spin,
            duration: 1.0,
        }];
        app.animation_render_settings = AnimationRenderSettings {
            output_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            base_name: base_name.clone(),
            width: 100,
            height: 100,
            fps: 10,
            concurrency: 3,
            backend: "Umbreon".to_string(),
            ..Default::default()
        };

        app.start_animation_render(&camera);
        app.poll_animation_render(&mut camera);
        let job = app.animation_render_job.as_ref().expect("render job");
        assert_eq!(job.next_frame, 3);
        assert_eq!(job.frame_receivers.len(), 3);
        job.cancel.store(true, Ordering::Relaxed);

        let deadline = Instant::now() + Duration::from_secs(5);
        while app.animation_render_job.is_some() && Instant::now() < deadline {
            app.poll_animation_render(&mut camera);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(app.animation_render_job.is_none(), "workers did not cancel");
        for index in 0..10 {
            let path = std::env::temp_dir().join(format!("{base_name}_{index:04}.png"));
            if path.is_file() {
                fs::remove_file(path).expect("remove rendered PNG");
            }
        }
    }

    #[test]
    fn native_primitive_instances_preserve_edge_and_radius_data() {
        let edge = cuemol_render::RenderEdgeStyle {
            mode: cuemol_render::RenderEdgeMode::Silhouette,
            width: 0.07,
            color: [0.1, 0.2, 0.3, 0.4],
        };
        let atom = RenderAtom::new(Vector4::new3(1.0, 2.0, 3.0), 0.45, None).with_edge(Some(edge));
        let atom_gpu = atom_instance(&atom);
        assert_eq!(atom_gpu.center_radius, [1.0, 2.0, 3.0, 0.45]);
        assert_eq!(atom_gpu.edge, [0.07, 2.0, 0.0, 0.0]);
        assert_eq!(atom_gpu.edge_color, edge.color);

        let bond = RenderBond::new(
            Vector4::new3(0.0, 1.0, 2.0),
            Vector4::new3(3.0, 4.0, 5.0),
            0.25,
            Some(cuemol_gfx::parse_color("#112233").unwrap()),
        )
        .with_end_color(Some(cuemol_gfx::parse_color("#445566").unwrap()))
        .with_edge(Some(edge));
        let bond_gpu = bond_instance(&bond);
        assert_eq!(bond_gpu.from_radius, [0.0, 1.0, 2.0, 0.25]);
        assert_eq!(bond_gpu.to_edge, [3.0, 4.0, 5.0, 0.07]);
        assert_eq!(bond_gpu.edge_mode, [2.0, 0.0, 0.0, 0.0]);
        assert_ne!(bond_gpu.color, bond_gpu.end_color);
    }

    #[test]
    fn scene_upload_revision_and_capacity_avoid_per_frame_reallocation() {
        let mut app = GuiApplication::new();
        let revision = app.snapshot_revision;
        assert!(snapshot_upload_required(None, revision));
        assert!(!snapshot_upload_required(Some(revision), revision));
        app.rebuild_snapshot();
        assert_eq!(app.snapshot_revision, revision.wrapping_add(1));
        assert!(snapshot_upload_required(
            Some(revision),
            app.snapshot_revision
        ));

        assert_eq!(scene_buffer_capacity(0), MIN_SCENE_BUFFER_CAPACITY);
        assert_eq!(scene_buffer_capacity(256), 256);
        assert_eq!(scene_buffer_capacity(257), 512);
        assert_eq!(scene_buffer_capacity(513), 1024);
    }

    #[test]
    fn native_molecule_shader_and_primitive_pipelines_validate() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
        else {
            return;
        };
        let Ok((device, queue)) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("molecule-shader-test-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            }))
        else {
            return;
        };
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("molecule-shader-test-camera-layout"),
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
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("molecule-shader-test"),
            source: wgpu::ShaderSource::Wgsl(MOLECULE_SHADER.into()),
        });
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let _atom = create_atom_pipeline(&device, &shader, &camera_layout, format);
        let _bond = create_bond_pipeline(&device, &shader, &camera_layout, format);
        let _mesh = create_mesh_pipeline(&device, &shader, &camera_layout, format);
        let error = pollster::block_on(device.pop_error_scope());
        assert!(
            error.is_none(),
            "molecule shader validation failed: {error:?}"
        );

        let mut buffer = ReusableGpuBuffer::new(
            &device,
            "scene-buffer-reuse-test",
            wgpu::BufferUsages::VERTEX,
        );
        buffer.upload(&device, &queue, &[0_u32; 4]);
        assert_eq!(buffer.capacity, 256);
        assert_eq!(buffer.buffer.size(), 256);
        buffer.upload(&device, &queue, &[0_u32; 100]);
        assert_eq!(buffer.capacity, 512);
        assert_eq!(buffer.buffer.size(), 512);
        buffer.upload(&device, &queue, &[1_u32; 8]);
        assert_eq!(buffer.capacity, 512);
        assert_eq!(buffer.buffer.size(), 512);
    }

    #[test]
    fn native_gui_tracks_legacy_menu_surfaces() {
        assert!(LEGACY_MENU_SURFACES.len() >= 50);
        assert!(LEGACY_MENU_SURFACES.contains(&"Open File..."));
        assert!(LEGACY_MENU_SURFACES.contains(&"Mol surface generation..."));
        assert!(LEGACY_MENU_SURFACES.contains(&"Restore default panel location"));
        assert!(LEGACY_MENU_SURFACES.contains(&"Hardware stereo"));
        assert!(LEGACY_MENU_SURFACES.contains(&"About CueMol2"));
        assert!(!LEGACY_MENU_SURFACES.contains(&"About CueMol3-tritium"));
        assert!(RENDERER_TYPES.contains(&"ballstick"));
        assert!(RENDERER_TYPES.contains(&"disorder"));
        assert!(!RENDERER_TYPES.contains(&"molsurf"));
        assert_eq!(renderer_type_label("cartoon"), "cartoon");
    }

    #[test]
    fn native_window_panel_menu_restores_reference_defaults() {
        let mut app = GuiApplication::new();
        assert!(app.left_panel_visible.iter().all(|visible| *visible));
        app.left_panel_visible[LeftPanel::Workspace.index()] = false;
        app.reset_left_panel_layout = false;
        app.show_workspace = false;

        app.restore_panels();

        assert!(app.show_workspace);
        assert!(app.left_panel_visible.iter().all(|visible| *visible));
        assert!(app.reset_left_panel_layout);
        assert!(!app.show_inspector);
    }

    #[test]
    fn native_dialogs_and_shortcuts_match_reference_commands() {
        let open = native_file_dialog_spec(NativeFileDialog::OpenObject);
        assert!(!open.save);
        assert!(open
            .filters
            .iter()
            .flat_map(|filter| filter.extensions)
            .any(|extension| *extension == "mmcif"));

        let save_scene = native_file_dialog_spec(NativeFileDialog::SaveScene);
        assert!(save_scene.save);
        assert_eq!(save_scene.default_extension, Some("qsc"));
        assert_eq!(
            apply_default_extension(PathBuf::from("scene"), save_scene),
            PathBuf::from("scene.qsc")
        );
        assert_eq!(
            apply_default_extension(PathBuf::from("scene.qsl"), save_scene),
            PathBuf::from("scene.qsl")
        );
        let open_camera = native_file_dialog_spec(NativeFileDialog::OpenCamera);
        assert!(!open_camera.save);
        assert_eq!(open_camera.filters[0].extensions, &["cam"]);
        let save_camera = native_file_dialog_spec(NativeFileDialog::SaveCamera);
        assert!(save_camera.save);
        assert_eq!(save_camera.default_extension, Some("cam"));
        let open_style = native_file_dialog_spec(NativeFileDialog::OpenStyle);
        assert!(!open_style.save);
        assert_eq!(open_style.filters[0].extensions, &["xml"]);
        let save_style = native_file_dialog_spec(NativeFileDialog::SaveStyle);
        assert!(save_style.save);
        assert_eq!(save_style.default_extension, Some("xml"));

        assert_eq!(
            MainShortcutCommand::OpenFile.shortcut(),
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O)
        );
        let mut shifted_command = egui::Modifiers::COMMAND;
        shifted_command.shift = true;
        assert_eq!(
            MainShortcutCommand::OpenScene.shortcut(),
            egui::KeyboardShortcut::new(shifted_command, egui::Key::O)
        );
        assert_eq!(
            MainShortcutCommand::Performance.shortcut(),
            egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F2)
        );
    }

    #[test]
    fn native_options_apply_runtime_values_and_persist_config() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-native-options-{}-{}.tsv",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let mut app = GuiApplication::new();
        app.config_path = path.to_string_lossy().into_owned();
        let mut preferences = NativePreferences::from_state(&app.state);
        preferences.atom_label_font = "monospace".to_string();
        preferences.atom_label_size = 18.0;
        preferences.atom_label_color = "#123456".to_string();
        preferences.atom_label_italic = true;
        preferences.atom_label_bold = true;
        preferences.ui_language = "ja".to_string();
        preferences.use_hidpi = true;
        preferences.mouse_preset = "TrackpadViewInConf".to_string();
        preferences.trackball_radius = 1.4;
        preferences.hit_precision = 16.0;
        preferences.momentum_scroll = true;
        preferences.multitouch_pad = true;
        preferences.emulate_right_button = true;
        preferences.shortcuts[0].key = "ArrowUp".to_string();
        preferences.shortcuts[1].key = "ArrowDown".to_string();

        app.apply_preferences(&preferences)
            .expect("apply and save native options");

        assert_eq!(
            app.state.style_value(0, "", "DefaultLabel.font_name"),
            "monospace"
        );
        assert_eq!(app.state.config_value("general.useragent.locale"), "ja");
        assert_eq!(app.state.view_input_config().style(), "TrackpadViewInConf");
        assert_eq!(app.state.view_input_config().tbrad(), 1.4);
        assert_eq!(app.state.view_input_config().hitprec(), 16.0);
        assert_eq!(
            app.state.config_value("shortcut:view-trans-plusx:key"),
            "ArrowUp"
        );
        assert_eq!(
            app.camera_drag_mode(MouseButton::Middle),
            Some(CameraDragMode::Pan)
        );
        assert_eq!(
            app.camera_drag_mode(MouseButton::Right),
            Some(CameraDragMode::Pan)
        );

        app.state
            .view_input_config_mut()
            .set_style("PymolViewInConf");
        assert_eq!(
            app.camera_drag_mode(MouseButton::Right),
            Some(CameraDragMode::Zoom)
        );

        let mut restored = GuiApplication::new();
        restored
            .state
            .load_config_file(&path)
            .expect("reload saved native options");
        restored.apply_stored_preferences();
        let roundtrip = NativePreferences::from_state(&restored.state);
        assert_eq!(roundtrip, preferences);
        fs::remove_file(path).expect("remove native options config");
    }

    #[test]
    fn native_options_validate_shortcuts_and_render_all_panes() {
        let mut app = GuiApplication::new();
        let mut invalid = NativePreferences::from_state(&app.state);
        invalid.shortcuts[1] = invalid.shortcuts[0].clone();
        invalid.shortcuts[1].id = "view-trans-minusx".to_string();
        assert_eq!(
            validate_native_preferences(&invalid),
            Err("duplicate shortcut: ArrowRight".to_string())
        );

        let ctx = egui::Context::default();
        app.open_options_dialog();
        for pane in [OptionsPane::Misc, OptionsPane::Key, OptionsPane::Mouse] {
            app.options_pane = pane;
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.options_dialog(ctx));
            assert!(app.show_options);
            assert!(app.options_draft.is_some());
        }
    }

    #[test]
    fn reload_scene_replaces_saved_contents_without_duplicating_objects() {
        let temp = std::env::temp_dir();
        let stem = format!("cuemol-native-reload-test-{}", std::process::id());
        let pdb_path = temp.join(format!("{stem}.pdb"));
        let qsc_path = temp.join(format!("{stem}.qsc"));
        fs::write(
            &pdb_path,
            "ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C  \nEND\n",
        )
        .expect("write test molecule");

        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb_path.to_string_lossy().into_owned();
        app.open_current_path(&mut camera);
        assert_eq!(app.state.active_object_summaries().len(), 1);
        app.save_current_scene_to_path(&camera, &qsc_path);

        app.open_path = pdb_path.to_string_lossy().into_owned();
        app.open_current_path(&mut camera);
        assert_eq!(app.state.active_object_summaries().len(), 2);
        app.reload_current_path(&mut camera);

        assert_eq!(app.state.active_object_summaries().len(), 1);
        assert_eq!(
            app.state.active_scene_source_path(),
            Some(qsc_path.as_path())
        );
        fs::remove_file(qsc_path).expect("remove test scene");
        fs::remove_file(pdb_path).expect("remove test molecule");
    }

    #[test]
    fn camera_controller_roundtrips_view_data_and_mutates_by_mouse_actions() {
        let mut camera = CameraController {
            name: "custom view".to_string(),
            target: Vec3::new(1.0, 2.0, 3.0),
            slab_depth: 83.0,
            trans_mms: true,
            rot_mms: true,
            stereo_mode: StereoMode::Cross,
            stereo_distance: 2.25,
            swap_stereo_eyes: true,
            center_mark: CenterMark::Axis,
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
        assert_eq!(parsed.name, camera.name);
        assert_eq!(parsed.slab_depth, camera.slab_depth);
        assert!(parsed.trans_mms);
        assert!(parsed.rot_mms);
        assert_eq!(parsed.stereo_mode, StereoMode::Cross);
        assert_eq!(parsed.stereo_distance, camera.stereo_distance);
        assert!(parsed.swap_stereo_eyes);
        assert_eq!(parsed.center_mark, CenterMark::Axis);
        let qsys_camera = parsed.to_qsys_camera("__current");
        assert_eq!(qsys_camera.name(), "__current");
        assert_eq!(qsys_camera.center().x, parsed.target.x as f64);
        assert_eq!(qsys_camera.is_perspective(), parsed.perspective);
        assert_eq!(qsys_camera.slab_depth(), parsed.slab_depth as f64);
        assert_eq!(qsys_camera.stereo_mode(), StereoMode::Cross);
        assert_eq!(qsys_camera.stereo_distance(), 2.25);
        assert_eq!(qsys_camera.center_mark(), cuemol_qsys::CenterMark::Axis);
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
            slab_depth: 74.0,
            perspective: false,
            stereo_mode: StereoMode::Parallel,
            stereo_distance: 3.5,
            center_mark: CenterMark::None,
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
        assert_eq!(back.slab_depth, cc.slab_depth, "slab_depth");
        assert_eq!(back.stereo_mode, cc.stereo_mode, "stereo_mode");
        assert_eq!(back.stereo_distance, cc.stereo_distance, "stereo_distance");
        assert_eq!(back.center_mark, cc.center_mark, "center_mark");
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
    fn native_view_property_editor_matches_reference_contract_and_applies_values() {
        let names = VIEW_PROPERTY_DEFINITIONS
            .iter()
            .map(|definition| definition.name.trim())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "name",
                "uid",
                "active",
                "zoom",
                "distance",
                "slab",
                "center",
                "center.x",
                "center.y",
                "center.z",
                "rotation",
                "rotation.x",
                "rotation.y",
                "rotation.z",
                "rotation.w",
                "perspective",
                "width",
                "height",
                "trans_mms",
                "rot_mms",
                "stereoMode",
                "hasHWStereo",
                "stereoDist",
                "swapStereoEyes",
                "up_vec",
                "up_vec.x",
                "up_vec.y",
                "up_vec.z",
                "right_vec",
                "right_vec.x",
                "right_vec.y",
                "right_vec.z",
                "forward_vec",
                "forward_vec.x",
                "forward_vec.y",
                "forward_vec.z",
                "centerMark",
            ]
        );

        let viewport = egui::Rect::from_min_size(egui::pos2(5.0, 7.0), egui::vec2(800.0, 600.0));
        let mut camera = CameraController::default();
        let mut properties = NativeViewProperties::from_camera(&camera, Some(viewport));
        assert_eq!((properties.width, properties.height), (800, 600));
        properties.name = "edited view".to_string();
        properties.zoom = 62.5;
        properties.distance = 160.0;
        properties.slab = 90.0;
        properties.center = [1.0, -2.0, 3.5];
        properties.rotation = [0.0, 0.0, 1.0, 1.0];
        properties.perspective = false;
        properties.trans_mms = true;
        properties.rot_mms = true;
        properties.stereo_mode = StereoMode::Hardware;
        properties.stereo_dist = 2.5;
        properties.swap_stereo_eyes = true;
        properties.center_mark = CenterMark::Axis;

        GuiApplication::apply_view_properties(&mut camera, &properties).unwrap();
        assert_eq!(camera.name, "edited view");
        assert!((camera.ortho_scale - 16.0).abs() < 1.0e-5);
        assert_eq!(camera.distance, 160.0);
        assert_eq!(camera.slab_depth, 90.0);
        assert_eq!(camera.target, Vec3::new(1.0, -2.0, 3.5));
        assert!(camera.orientation.is_normalized());
        assert!(!camera.perspective);
        assert!(camera.trans_mms);
        assert!(camera.rot_mms);
        assert_eq!(camera.stereo_mode, StereoMode::Hardware);
        assert_eq!(camera.stereo_distance, 2.5);
        assert!(camera.swap_stereo_eyes);
        assert_eq!(camera.center_mark, CenterMark::Axis);

        properties.rotation = [0.0; 4];
        assert_eq!(
            GuiApplication::apply_view_properties(&mut camera, &properties),
            Err("view rotation must be a non-zero finite quaternion".to_string())
        );
    }

    #[test]
    fn native_scene_property_editor_matches_reference_contract_and_persists_values() {
        let names = SCENE_PROPERTY_DEFINITIONS
            .iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "uid",
                "modified",
                "name",
                "src",
                "srctype",
                "basePath",
                "bgcolor",
                "aoEnabled",
                "aoRadius",
                "aoIntensity",
                "aoSlices",
                "aoSteps",
                "aoHalfRes",
                "aa_method",
                "aaJitterLevel",
                "onload",
                "obj_uids",
                "activeObjID",
                "activeRendID",
                "view_uids",
                "activeViewID",
                "use_colproof",
                "icc_filename",
                "icc_intent",
            ]
        );

        let mut app = GuiApplication::new();
        let mut properties = NativeSceneProperties::from_state(&app.state).unwrap();
        properties.background = "#123456".to_string();
        properties.ao_enabled = true;
        properties.ao_radius = 7.5;
        properties.ao_intensity = 3.25;
        properties.ao_slices = 16;
        properties.ao_steps = 6;
        properties.ao_half_res = true;
        properties.aa_method = "smaa".to_string();
        properties.aa_jitter_level = 4;
        properties.onload = "sceneReady();".to_string();
        properties.use_color_proof = true;
        properties.icc_filename = "proof.icc".to_string();
        properties.icc_intent = "relative_colorimetric".to_string();

        app.apply_scene_properties(&properties).unwrap();
        assert_eq!(
            app.background,
            SceneBackground::Custom(Rgba8 {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0xff,
            })
        );
        assert!(app.color_proofing);
        assert_eq!(app.undo_stack.len(), 1);
        let scene = app
            .state
            .scene_manager()
            .scene(cuemol_core::Uid::new(properties.uid))
            .unwrap();
        for (name, expected) in [
            ("bgcolor", "#123456"),
            ("aoEnabled", "true"),
            ("aoRadius", "7.5"),
            ("aoIntensity", "3.25"),
            ("aoSlices", "16"),
            ("aoSteps", "6"),
            ("aoHalfRes", "true"),
            ("aa_method", "smaa"),
            ("aaJitterLevel", "4"),
            ("onload", "sceneReady();"),
            ("use_colproof", "true"),
            ("icc_filename", "proof.icc"),
            ("icc_intent", "relative_colorimetric"),
        ] {
            assert_eq!(scene.scene_attribute(name), Some(expected), "{name}");
        }

        let mut camera = CameraController::default();
        app.undo(&mut camera);
        assert_eq!(app.background, SceneBackground::Black);
        assert!(!app.color_proofing);
        assert!(app.modified_scenes.is_empty());
        app.redo(&mut camera);
        assert_eq!(
            app.background,
            SceneBackground::Custom(Rgba8 {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0xff,
            })
        );
        assert!(app.color_proofing);
        assert!(app
            .modified_scenes
            .contains(&cuemol_core::Uid::new(properties.uid)));

        let path = std::env::temp_dir().join(format!(
            "cuemol-native-scene-properties-{}.qsc",
            std::process::id()
        ));
        app.save_current_scene_to_path(&camera, &path);
        let qsc = fs::read_to_string(&path).unwrap();
        assert!(qsc.contains("bgcolor=\"#123456\""));
        assert!(qsc.contains("aoEnabled=\"true\""));
        assert!(qsc.contains("icc_intent=\"relative_colorimetric\""));
        assert!(app.undo_stack.is_empty());
        assert!(app.modified_scenes.is_empty());
        fs::remove_file(path).unwrap();

        properties.ao_radius = 0.0;
        assert_eq!(
            app.apply_scene_properties(&properties),
            Err("AO radius must be a positive finite number".to_string())
        );
    }

    #[test]
    fn native_object_property_dialog_applies_undoes_and_persists_common_fields() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let molecule = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("original", molecule);
        let mut properties = NativeObjectProperties::from_state(&app.state, object_uid).unwrap();
        assert_eq!(properties.linked, "(embeded)");
        properties.name = "renamed molecule".to_string();
        properties.visible = false;
        properties.locked = true;

        app.apply_object_properties(&properties).unwrap();
        let object = app.state.scene_manager().object(object_uid).unwrap();
        assert_eq!(object.name(), "renamed molecule");
        assert!(!object.is_visible());
        assert!(object.is_ui_locked());
        assert!(app.modified_scenes.contains(&scene_uid));

        let mut camera = CameraController::default();
        app.undo(&mut camera);
        let object = app.state.scene_manager().object(object_uid).unwrap();
        assert_eq!(object.name(), "original");
        assert!(object.is_visible());
        assert!(!object.is_ui_locked());
        app.redo(&mut camera);

        let path = std::env::temp_dir().join(format!(
            "cuemol-native-object-properties-{}.qsc",
            std::process::id()
        ));
        app.save_current_scene_to_path(&camera, &path);
        let qsc = fs::read_to_string(&path).unwrap();
        assert!(qsc.contains("name=\"renamed molecule\""));
        assert!(qsc.contains("visible=\"false\""));
        assert!(qsc.contains("locked=\"true\""));
        fs::remove_file(path).unwrap();

        properties.name = "bad:name".to_string();
        assert_eq!(
            app.apply_object_properties(&properties),
            Err("object name must be non-empty and cannot contain ':'".to_string())
        );
    }

    #[test]
    fn native_object_property_dialog_keeps_an_isolated_draft() {
        let ctx = egui::Context::default();
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("object", cuemol_molstr::MolCoord::new());
        app.open_object_properties(object_uid);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.object_props_dialog(ctx)
        });
        assert!(app.object_properties_draft.is_some());
        assert_eq!(
            app.state.scene_manager().object(object_uid).unwrap().name(),
            "object"
        );
    }

    #[test]
    fn native_renderer_property_dialog_applies_undoes_and_persists_common_fields() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let molecule = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", molecule);
        let renderer_uid = app
            .state
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("ballstick", "sticks");
        let mut properties =
            NativeRendererProperties::from_state(&app.state, object_uid, renderer_uid).unwrap();
        properties.name = "detailed sticks".to_string();
        properties.selection = "name CA".to_string();
        properties.visible = false;
        properties.locked = true;
        properties.style = "ThickSimple,DefaultCPKColoring".to_string();
        properties.material = "default".to_string();
        properties.alpha = 0.6;
        properties.edge_type = "silhouette".to_string();
        properties.edge_width = 0.2;
        properties.edge_color = "#123456".to_string();
        properties.detail = 8;
        properties.bond_width = 0.42;
        properties.sphere_radius = 0.55;
        properties.show_ring = true;
        properties.ring_thickness = 0.31;
        properties.ring_color = "#abcdef".to_string();

        app.apply_renderer_properties(&properties).unwrap();
        let renderer = app
            .state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert_eq!(renderer.name(), "detailed sticks");
        assert_eq!(renderer.qsc_attr("sel"), Some("name CA"));
        assert!(!renderer.is_visible());
        assert_eq!(renderer.qsc_attr("locked"), Some("true"));
        assert_eq!(renderer.style(), Some("ThickSimple,DefaultCPKColoring"));
        assert_eq!(renderer.qsc_attr("alpha"), Some("0.6"));
        assert_eq!(renderer.qsc_attr("bondw"), Some("0.42"));
        assert_eq!(renderer.qsc_attr("ringcolor"), Some("#abcdef"));
        assert!(app.modified_scenes.contains(&scene_uid));

        let mut camera = CameraController::default();
        app.undo(&mut camera);
        let renderer = app
            .state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert_eq!(renderer.name(), "sticks");
        assert!(renderer.is_visible());
        assert_eq!(renderer.style(), None);
        assert_eq!(renderer.qsc_attr("bondw"), None);
        app.redo(&mut camera);

        let path = std::env::temp_dir().join(format!(
            "cuemol-native-renderer-properties-{}.qsc",
            std::process::id()
        ));
        app.save_current_scene_to_path(&camera, &path);
        let qsc = fs::read_to_string(&path).unwrap();
        assert!(qsc.contains("name=\"detailed sticks\""));
        assert!(qsc.contains("style=\"ThickSimple,DefaultCPKColoring\""));
        assert!(qsc.contains("bondw=\"0.42\""));
        assert!(qsc.contains("egtype=\"silhouette\""));
        fs::remove_file(path).unwrap();

        let undo_len = app.undo_stack.len();
        properties.edge_color = "not-a-color".to_string();
        assert_eq!(
            app.apply_renderer_properties(&properties),
            Err("invalid edge color: not-a-color".to_string())
        );
        assert_eq!(app.undo_stack.len(), undo_len);
    }

    #[test]
    fn native_renderer_property_pages_roundtrip_child_and_nucleic_fields() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        let (ribbon_uid, nucl_uid) = {
            let object = app
                .state
                .scene_manager_mut()
                .object_mut(object_uid)
                .unwrap();
            (
                object.add_renderer("ribbon", "ribbon"),
                object.add_renderer("nucl", "nucleic acid"),
            )
        };

        let mut ribbon =
            NativeRendererProperties::from_state(&app.state, object_uid, ribbon_uid).unwrap();
        assert_eq!(ribbon.tabs(), &["Common", "Helix", "Sheet", "Coil"]);
        ribbon
            .specific
            .insert("helix.type".to_string(), "fancy1".to_string());
        ribbon
            .specific
            .insert("helix.width".to_string(), "0.72".to_string());
        ribbon
            .specific
            .insert("axialdetail".to_string(), "10".to_string());
        ribbon
            .specific
            .insert("helix_usebackcol".to_string(), "true".to_string());
        ribbon
            .specific
            .insert("helix_backcol".to_string(), "#123456".to_string());
        app.apply_renderer_properties(&ribbon).unwrap();

        let renderer = app
            .state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(ribbon_uid)
            .unwrap();
        assert_eq!(
            renderer.qsc_child_attr("helix", "type"),
            Some("fancy1".to_string())
        );
        assert_eq!(
            renderer.qsc_child_attr("helix", "width"),
            Some("0.72".to_string())
        );
        assert_eq!(renderer.qsc_attr("axialdetail"), Some("10"));

        let mut nucl =
            NativeRendererProperties::from_state(&app.state, object_uid, nucl_uid).unwrap();
        assert_eq!(nucl.tabs(), &["Common", "Nucleic acid", "Tube"]);
        nucl.specific
            .insert("show_basepair".to_string(), "false".to_string());
        nucl.specific
            .insert("base_type".to_string(), "detail2".to_string());
        nucl.specific
            .insert("base_size".to_string(), "1.2".to_string());
        nucl.specific
            .insert("section.width".to_string(), "0.62".to_string());
        app.apply_renderer_properties(&nucl).unwrap();

        let mut camera = CameraController::default();
        app.undo(&mut camera);
        let renderer = app
            .state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(nucl_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("show_basepair"), None);
        app.redo(&mut camera);

        let path = std::env::temp_dir().join(format!(
            "cuemol-native-renderer-pages-{}.qsc",
            std::process::id()
        ));
        app.save_current_scene_to_path(&camera, &path);
        let qsc = fs::read_to_string(&path).unwrap();
        assert!(qsc.contains("axialdetail=\"10\""));
        assert!(qsc.contains("show_basepair=\"false\""));
        assert!(qsc.contains("base_type=\"detail2\""));
        assert!(qsc.contains("width=\"0.72\""));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_renderer_map_and_surface_pages_validate_and_persist() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        let (map_uid, surface_uid) = {
            let object = app
                .state
                .scene_manager_mut()
                .object_mut(object_uid)
                .unwrap();
            (
                object.add_renderer("isosurf", "map surface"),
                object.add_renderer("dsurface", "molecular surface"),
            )
        };

        let mut map =
            NativeRendererProperties::from_state(&app.state, object_uid, map_uid).unwrap();
        assert_eq!(map.tabs(), &["Common", "Map"]);
        map.specific
            .insert("drawmode".to_string(), "line".to_string());
        map.specific.insert("width".to_string(), "2.4".to_string());
        map.specific
            .insert("max_grids".to_string(), "500".to_string());
        map.specific
            .insert("cullface".to_string(), "true".to_string());
        app.apply_renderer_properties(&map).unwrap();
        let renderer = app
            .state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(map_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("drawmode"), Some("line"));
        assert_eq!(renderer.qsc_attr("max_grids"), Some("500"));

        let mut surface =
            NativeRendererProperties::from_state(&app.state, object_uid, surface_uid).unwrap();
        assert_eq!(surface.tabs(), &["Common", "MolSurf"]);
        surface
            .specific
            .insert("colormode".to_string(), "potential".to_string());
        surface
            .specific
            .insert("surftype".to_string(), "sas".to_string());
        surface
            .specific
            .insert("detail".to_string(), "9".to_string());
        surface
            .specific
            .insert("width".to_string(), "2".to_string());
        app.apply_renderer_properties(&surface).unwrap();
        let renderer = app
            .state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(surface_uid)
            .unwrap();
        assert_eq!(renderer.qsc_attr("colormode"), Some("potential"));
        assert_eq!(renderer.qsc_attr("surftype"), Some("sas"));
        assert_eq!(renderer.qsc_attr("detail"), Some("9"));

        surface
            .specific
            .insert("width".to_string(), "0".to_string());
        assert_eq!(
            app.apply_renderer_properties(&surface),
            Err("surface width must be between 0.1 and 10".to_string())
        );
    }

    #[test]
    fn native_workspace_camera_load_delete_and_undo() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let mut saved = cuemol_qsys::Camera::new();
        saved.set_name("camera_0");
        saved.set_zoom(80.0);
        app.state
            .save_scene_camera(scene_uid, saved, false)
            .unwrap();
        let selection = WorkspaceAuxSelection::Camera {
            scene_uid,
            camera_index: 0,
        };
        app.workspace_aux_selection = Some(selection);
        let mut current = CameraController::default().to_qsys_camera("current");
        current.set_zoom(95.0);
        let mut camera = CameraController::from_qsys_camera(&current);
        app.save_workspace_camera_from_scene(selection, &camera);
        assert!(
            (app.state
                .scene_manager()
                .scene(scene_uid)
                .unwrap()
                .camera_by_name("camera_0")
                .unwrap()
                .zoom()
                - 95.0)
                .abs()
                < 1.0e-5
        );
        app.undo(&mut camera);
        app.load_workspace_camera(selection, &mut camera);
        assert!((camera.to_qsys_camera("loaded").zoom() - 80.0).abs() < 1.0e-5);

        app.copy_workspace_camera_to_clipboard(scene_uid, "camera_0");
        assert!(app.workspace_camera_clipboard.is_some());
        app.paste_workspace_camera_clipboard(scene_uid);
        assert_eq!(app.state.camera_summaries(scene_uid).len(), 2);
        assert_eq!(
            app.state.camera_summaries(scene_uid)[1].name,
            "copy1_camera_0"
        );
        app.undo(&mut camera);
        assert_eq!(app.state.camera_summaries(scene_uid).len(), 1);
        assert!(app.workspace_camera_clipboard.is_some());

        app.delete_aux_selection(selection);
        assert!(app.state.camera_summaries(scene_uid).is_empty());
        assert_eq!(
            app.workspace_aux_selection,
            Some(WorkspaceAuxSelection::CameraRoot { scene_uid })
        );
        app.undo(&mut camera);
        assert_eq!(app.state.camera_summaries(scene_uid)[0].name, "camera_0");
    }

    #[test]
    fn native_camera_visibility_editor_and_clear_are_undoable() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let scene = app.state.scene_manager_mut().scene_mut(scene_uid).unwrap();
        let object_uid = scene.add_molecule("mol", cuemol_molstr::MolCoord::new());
        let renderer_uid = scene
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "simple");
        let mut saved = cuemol_qsys::Camera::new();
        saved.set_name("camera_0");
        app.state.save_scene_camera(scene_uid, saved, true).unwrap();
        let selection = WorkspaceAuxSelection::Camera {
            scene_uid,
            camera_index: 0,
        };

        let mut current = CameraController::default().to_qsys_camera("current");
        current.set_zoom(64.0);
        let mut camera = CameraController::from_qsys_camera(&current);
        app.save_workspace_camera_from_view(selection, &camera);
        assert_eq!(app.state.camera_summaries(scene_uid)[0].visibility_count, 2);
        app.state.set_object_visible(object_uid, false).unwrap();
        let undo_count = app.undo_stack.len();
        app.load_workspace_camera_view(selection, &mut camera);
        assert_eq!(camera.to_qsys_camera("loaded").zoom(), 64.0);
        assert!(!app.state.object_summaries(scene_uid)[0].visible);
        assert_eq!(app.undo_stack.len(), undo_count);

        app.open_camera_visibility_editor(selection);
        let draft = app.camera_visibility_draft.as_mut().unwrap();
        assert_eq!(draft.entries.len(), 2);
        for entry in &mut draft.entries {
            entry.included = entry.target_uid == renderer_uid;
            entry.visible = false;
        }
        let settings = draft
            .entries
            .iter()
            .filter(|entry| entry.included)
            .map(|entry| (entry.target_uid, entry.visible, entry.is_object))
            .collect::<Vec<_>>();
        let undo = app.capture_undo();
        app.state
            .set_scene_camera_visibility_settings(scene_uid, "camera_0", &settings)
            .unwrap();
        app.push_undo_snapshot(undo);
        assert_eq!(app.state.camera_summaries(scene_uid)[0].visibility_count, 1);

        app.clear_camera_visibility(scene_uid, "camera_0");
        assert_eq!(app.state.camera_summaries(scene_uid)[0].visibility_count, 0);
        app.undo(&mut camera);
        assert_eq!(app.state.camera_summaries(scene_uid)[0].visibility_count, 1);
        assert!(app.modified_scenes.contains(&scene_uid));
    }

    #[test]
    fn native_workspace_state_toggles_are_undoable() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let scene = app.state.scene_manager_mut().scene_mut(scene_uid).unwrap();
        let object_uid = scene.add_molecule("mol", cuemol_molstr::MolCoord::new());
        let renderer_uid = scene
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "simple");

        app.set_workspace_object_visibility(NativePropertyObject::Molecule(object_uid), false);
        app.set_workspace_object_locked(NativePropertyObject::Molecule(object_uid), true);
        app.set_renderer_visibility(object_uid, renderer_uid, false);
        app.set_workspace_renderer_locked(
            NativePropertyObject::Molecule(object_uid),
            renderer_uid,
            true,
        );
        let object = &app.state.object_summaries(scene_uid)[0];
        assert!(!object.visible);
        assert!(object.locked);
        let renderer = &app.state.renderer_summaries(object_uid)[0];
        assert!(!renderer.visible);
        assert!(renderer.locked);

        let mut camera = CameraController::default();
        app.undo(&mut camera);
        assert!(!app.state.renderer_summaries(object_uid)[0].locked);
        assert!(app.modified_scenes.contains(&scene_uid));
    }

    #[test]
    fn native_workspace_style_select_delete_and_undo() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let local_index = app.state.create_scene_style(scene_uid, "style_0").unwrap();
        let style_index = app
            .state
            .style_summaries(scene_uid)
            .iter()
            .position(|style| style.local_index == Some(local_index))
            .unwrap();
        let selection = WorkspaceAuxSelection::Style {
            scene_uid,
            style_index,
        };
        app.open_style_properties(selection);
        assert_eq!(app.style_properties_draft.as_ref().unwrap().name, "style_0");

        app.set_workspace_style_read_only(selection, true);
        assert!(app
            .state
            .style_summaries(scene_uid)
            .iter()
            .any(|style| style.name == "style_0" && style.read_only));
        let mut camera = CameraController::default();
        app.undo(&mut camera);
        assert!(app
            .state
            .style_summaries(scene_uid)
            .iter()
            .any(|style| style.name == "style_0" && !style.read_only));

        app.copy_workspace_style_to_clipboard(scene_uid, local_index);
        assert!(app.workspace_style_clipboard.is_some());
        assert!(app.undo_stack.is_empty());
        app.state
            .update_scene_style(
                scene_uid,
                local_index,
                "style_0",
                false,
                "<styles><style id=\"Narrow\" type=\"renderer\" width=\"1\"/></styles>",
            )
            .unwrap();
        app.request_paste_workspace_style(scene_uid);
        assert_eq!(app.pending_style_paste_scene, Some(scene_uid));
        app.pending_style_paste_scene = None;
        app.paste_workspace_style_clipboard(scene_uid, true);
        assert!(!app
            .state
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index]
            .data
            .contains("Narrow"));
        app.undo(&mut camera);
        assert!(app
            .state
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index]
            .data
            .contains("Narrow"));

        app.delete_aux_selection(selection);
        assert!(app
            .state
            .style_summaries(scene_uid)
            .iter()
            .all(|style| style.local_index.is_none()));
        app.undo(&mut camera);
        assert!(app
            .state
            .style_summaries(scene_uid)
            .iter()
            .any(|style| style.name == "style_0"));
    }

    #[test]
    fn native_renderer_context_style_creation_is_undoable() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let local_index = app.state.create_scene_style(scene_uid, "user").unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        let renderer_uid = app
            .state
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "lines");
        app.state
            .set_renderer_property(object_uid, renderer_uid, "width", "3.5")
            .unwrap();

        app.open_renderer_style_create(scene_uid, object_uid, renderer_uid);
        let mut draft = app.renderer_style_create_draft.take().unwrap();
        assert_eq!(draft.style_set_index, Some(local_index));
        draft.name_prefix = "Wide".to_string();
        assert_eq!(
            app.apply_renderer_style_create(&draft).unwrap(),
            "Widesimple"
        );
        let style = &app
            .state
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index];
        assert!(style.data.contains("id=\"Widesimple\""));
        assert!(style.data.contains("width=\"3.5\""));
        assert!(style.node.str_attr("modified").equals_ignore_case("true"));

        app.undo(&mut CameraController::default());
        let style = &app
            .state
            .scene_manager()
            .scene(scene_uid)
            .unwrap()
            .loaded_styles()[local_index];
        assert!(!style.data.contains("id=\"Widesimple\""));
    }

    #[test]
    fn native_workspace_density_map_properties_apply_delete_undo_and_save() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let map_uid = app
            .state
            .add_density_map(scene_uid, "density map", synthetic_gaussian_map());
        app.workspace_aux_selection = Some(WorkspaceAuxSelection::DensityMap {
            scene_uid,
            map_uid,
            renderer_uid: None,
        });
        app.open_aux_renderer_dialog(app.workspace_aux_selection.unwrap());
        app.new_renderer_type = "isosurf".to_string();
        app.new_renderer_name = "map surface".to_string();
        app.create_renderer_from_dialog();
        let renderer_uid = match app.workspace_aux_selection.unwrap() {
            WorkspaceAuxSelection::DensityMap {
                renderer_uid: Some(renderer_uid),
                ..
            } => renderer_uid,
            _ => panic!("new density renderer must be selected"),
        };

        let mut object = NativeObjectProperties::from_density_state(&app.state, map_uid).unwrap();
        object.name = "edited density".to_string();
        object.locked = true;
        app.apply_object_properties(&object).unwrap();
        let map = app.state.density_map_summary(map_uid).unwrap();
        assert_eq!(map.name, "edited density");
        assert!(map.locked);

        let mut renderer =
            NativeRendererProperties::from_density_state(&app.state, map_uid, renderer_uid)
                .unwrap();
        assert_eq!(renderer.tabs(), &["Common", "Map"]);
        renderer.name = "2 sigma contour".to_string();
        renderer.alpha = 0.6;
        renderer
            .specific
            .insert("width".to_string(), "2.5".to_string());
        renderer
            .specific
            .insert("bndry_rng".to_string(), "8".to_string());
        app.apply_renderer_properties(&renderer).unwrap();
        let rendered = app
            .state
            .density_map_summary(map_uid)
            .unwrap()
            .renderers
            .into_iter()
            .find(|candidate| candidate.uid == renderer_uid)
            .unwrap();
        assert_eq!(rendered.name, "2 sigma contour");
        assert_eq!(rendered.alpha, 0.6);
        assert_eq!(rendered.specific["width"], "2.5");

        app.delete_aux_selection(WorkspaceAuxSelection::DensityMap {
            scene_uid,
            map_uid,
            renderer_uid: Some(renderer_uid),
        });
        assert!(app
            .state
            .density_map_summary(map_uid)
            .unwrap()
            .renderers
            .is_empty());
        let mut camera = CameraController::default();
        app.undo(&mut camera);
        assert_eq!(
            app.state
                .density_map_summary(map_uid)
                .unwrap()
                .renderers
                .len(),
            1
        );

        let path = std::env::temp_dir().join(format!(
            "cuemol-native-density-workspace-{}.qsc",
            std::process::id()
        ));
        app.save_current_scene_to_path(&camera, &path);
        let qsc = fs::read_to_string(&path).unwrap();
        assert!(qsc.contains("name=\"edited density\""));
        assert!(qsc.contains("name=\"2 sigma contour\""));
        assert!(qsc.contains("width=\"2.5\""));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_workspace_surface_properties_apply_add_delete_and_undo() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let data = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let surface_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_loaded_surface(cuemol_molstr::LoadedSurface {
                uid: cuemol_core::Uid::new(0),
                name: "surface".to_string(),
                object_type: "MolSurfObj".to_string(),
                srctype: "obj".to_string(),
                data: data.to_string(),
                raw_data: data.as_bytes().to_vec(),
                qsc_node: None,
            });
        let renderer_uid = app.state.surface_summary(surface_uid).unwrap().renderers[0].uid;
        let mut renderer =
            NativeRendererProperties::from_surface_state(&app.state, surface_uid, renderer_uid)
                .unwrap();
        assert_eq!(renderer.renderer_type, "molsurf");
        assert_eq!(renderer.tabs(), &["Common", "MolSurf"]);
        renderer.name = "surface mesh".to_string();
        renderer.selection = "*".to_string();
        renderer
            .specific
            .insert("drawmode".to_string(), "line".to_string());
        renderer
            .specific
            .insert("width".to_string(), "2".to_string());
        renderer
            .specific
            .insert("colormode".to_string(), "multigrad".to_string());
        app.apply_renderer_properties(&renderer).unwrap();

        let mut object =
            NativeObjectProperties::from_surface_state(&app.state, surface_uid).unwrap();
        object.name = "edited surface".to_string();
        object.locked = true;
        app.apply_object_properties(&object).unwrap();
        let surface = app.state.surface_summary(surface_uid).unwrap();
        assert_eq!(surface.name, "edited surface");
        assert!(surface.locked);
        assert_eq!(surface.renderers[0].name, "surface mesh");
        assert_eq!(surface.renderers[0].specific["drawmode"], "line");
        assert_eq!(surface.renderers[0].specific["colormode"], "multigrad");

        app.open_aux_renderer_dialog(WorkspaceAuxSelection::Surface {
            scene_uid,
            surface_uid,
            renderer_uid: None,
        });
        assert_eq!(
            app.new_renderer_owner,
            Some(NativePropertyObject::Surface(surface_uid))
        );
        assert_eq!(app.new_renderer_type, "molsurf");
        app.new_renderer_name = "second surface".to_string();
        app.create_renderer_from_dialog();
        assert_eq!(
            app.state
                .surface_summary(surface_uid)
                .unwrap()
                .renderers
                .len(),
            2
        );
        let added_uid = match app.workspace_aux_selection.unwrap() {
            WorkspaceAuxSelection::Surface {
                renderer_uid: Some(renderer_uid),
                ..
            } => renderer_uid,
            _ => panic!("new surface renderer must be selected"),
        };
        app.delete_aux_selection(WorkspaceAuxSelection::Surface {
            scene_uid,
            surface_uid,
            renderer_uid: Some(added_uid),
        });
        assert_eq!(
            app.state
                .surface_summary(surface_uid)
                .unwrap()
                .renderers
                .len(),
            1
        );
        let mut camera = CameraController::default();
        app.undo(&mut camera);
        assert_eq!(
            app.state
                .surface_summary(surface_uid)
                .unwrap()
                .renderers
                .len(),
            2
        );

        let path = std::env::temp_dir().join(format!(
            "cuemol-native-surface-workspace-{}.qsc",
            std::process::id()
        ));
        app.save_current_scene_to_path(&camera, &path);
        let qsc = fs::read_to_string(&path).unwrap();
        assert!(qsc.contains("name=\"edited surface\""));
        assert!(qsc.contains("name=\"surface mesh\""));
        assert!(qsc.contains("colormode=\"multigrad\""));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_workspace_zoom_fits_renderer_object_and_density_map() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        app.viewport_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 400.0),
        ));
        let molecule = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      2  CA  GLY A   2      10.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      3  CA  GLY B   1     100.000   0.000   0.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", molecule);
        let renderer_uid = {
            let object = app
                .state
                .scene_manager_mut()
                .object_mut(object_uid)
                .unwrap();
            let renderer_uid = object.add_renderer("simple", "chain A");
            object
                .renderer_mut(renderer_uid)
                .unwrap()
                .set_selection(cuemol_molstr::parse_selector("chain A").unwrap());
            renderer_uid
        };

        let mut camera = CameraController::default();
        app.selected_object = Some(object_uid);
        app.selected_renderer = Some(renderer_uid);
        assert!(app.fit_workspace_selection(&mut camera).unwrap());
        assert!((camera.target.x - 5.0).abs() < 1.0e-5);
        assert!((camera.ortho_scale - 1000.0 / 7.0).abs() < 1.0e-4);

        app.selected_renderer = None;
        assert!(app.fit_workspace_selection(&mut camera).unwrap());
        assert!((camera.target.x - 50.0).abs() < 1.0e-5);
        assert!((camera.ortho_scale - 1000.0 / 70.0).abs() < 1.0e-4);

        let mut map = cuemol_xtal::DensityMap::new();
        map.set_map_byte_array(&[0, 255], [2, 1, 1], 0.0, 1.0, 0.5, 0.5);
        map.set_map_params([0, 0, 0], [2, 1, 1]);
        map.set_xtal_params(20.0, 10.0, 10.0, 90.0, 90.0, 90.0);
        let map_uid = app.state.add_density_map(scene_uid, "density", map);
        app.selected_object = None;
        app.workspace_aux_selection = Some(WorkspaceAuxSelection::DensityMap {
            scene_uid,
            map_uid,
            renderer_uid: None,
        });
        assert!(app.fit_workspace_selection(&mut camera).unwrap());
        assert!((camera.target.x - 10.0).abs() < 1.0e-5);
        assert!((camera.target.y - 5.0).abs() < 1.0e-5);
        assert!((camera.ortho_scale - 100.0).abs() < 1.0e-4);
    }

    #[test]
    fn native_workspace_renderer_groups_are_nested_undoable_and_persisted() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        let (group_uid, first_uid, second_uid, free_uid) = {
            let object = app
                .state
                .scene_manager_mut()
                .object_mut(object_uid)
                .unwrap();
            let group_uid = object.add_renderer("*group", "representations");
            let first_uid = object.add_renderer("simple", "lines");
            object
                .renderer_mut(first_uid)
                .unwrap()
                .set_qsc_attr("group", "representations");
            let second_uid = object.add_renderer("cpk", "atoms");
            object
                .renderer_mut(second_uid)
                .unwrap()
                .set_qsc_attr("group", "representations");
            let free_uid = object.add_renderer("trace", "free");
            (group_uid, first_uid, second_uid, free_uid)
        };

        let tree = native_renderer_tree_nodes(app.state.renderer_summaries(object_uid));
        assert_eq!(tree.len(), 2);
        let group = tree
            .iter()
            .find(|node| node.renderer.uid == group_uid)
            .unwrap();
        assert_eq!(group.children.len(), 2);
        assert!(group
            .children
            .iter()
            .any(|renderer| renderer.uid == first_uid));
        assert!(group
            .children
            .iter()
            .any(|renderer| renderer.uid == second_uid));
        assert!(tree.iter().any(|node| node.renderer.uid == free_uid));

        app.selected_object = Some(object_uid);
        app.selected_renderer = Some(group_uid);
        assert!(app.selected_renderer_is_group());
        let mut camera = CameraController::default();
        assert!(!app.fit_workspace_selection(&mut camera).unwrap());

        app.set_renderer_group_visibility(object_uid, group_uid, false);
        let object = app.state.scene_manager().object(object_uid).unwrap();
        assert!(!object.renderer(group_uid).unwrap().is_visible());
        assert!(!object.renderer(first_uid).unwrap().is_visible());
        assert!(!object.renderer(second_uid).unwrap().is_visible());
        assert!(object.renderer(free_uid).unwrap().is_visible());
        app.undo(&mut camera);
        assert!(app
            .state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(first_uid)
            .unwrap()
            .is_visible());

        let mut properties =
            NativeRendererProperties::from_state(&app.state, object_uid, group_uid).unwrap();
        assert_eq!(properties.title(), "Renderer group properties");
        properties.name = "renamed group".to_string();
        app.apply_renderer_properties(&properties).unwrap();
        let object = app.state.scene_manager().object(object_uid).unwrap();
        assert_eq!(
            object.renderer(first_uid).unwrap().group(),
            Some("renamed group")
        );
        assert_eq!(
            object.renderer(second_uid).unwrap().group(),
            Some("renamed group")
        );

        app.move_workspace_renderer(scene_uid, object_uid, free_uid, group_uid, "inside");
        assert_eq!(
            app.state
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .renderer(free_uid)
                .unwrap()
                .group(),
            Some("renamed group")
        );
        app.undo(&mut camera);
        assert_eq!(
            app.state
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .renderer(free_uid)
                .unwrap()
                .group(),
            None
        );

        app.open_renderer_dialog_in_group(object_uid, Some("renamed group".to_string()));
        app.new_renderer_type = "tube".to_string();
        app.new_renderer_name = "group tube".to_string();
        app.create_renderer_from_dialog();
        let added_uid = app.selected_renderer.unwrap();
        assert_eq!(
            app.state
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .renderer(added_uid)
                .unwrap()
                .group(),
            Some("renamed group")
        );

        app.selected_renderer = Some(group_uid);
        app.delete_renderer(object_uid, group_uid);
        let object = app.state.scene_manager().object(object_uid).unwrap();
        assert!(object.renderer(group_uid).is_none());
        assert!(object.renderer(first_uid).is_none());
        assert!(object.renderer(second_uid).is_none());
        assert!(object.renderer(added_uid).is_none());
        assert!(object.renderer(free_uid).is_some());
        app.undo(&mut camera);
        let object = app.state.scene_manager().object(object_uid).unwrap();
        assert!(object.renderer(group_uid).is_some());
        assert!(object.renderer(added_uid).is_some());

        let path = std::env::temp_dir().join(format!(
            "cuemol-native-renderer-groups-{}.qsc",
            std::process::id()
        ));
        app.save_current_scene_to_path(&camera, &path);
        let qsc = fs::read_to_string(&path).unwrap();
        assert!(qsc.contains("type=\"*group\""));
        assert!(qsc.contains("name=\"renamed group\""));
        assert!(qsc.contains("group=\"renamed group\""));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_workspace_object_order_is_cross_kind_and_undoable() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let molecule_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        let map_uid = app
            .state
            .add_density_map(scene_uid, "map", cuemol_xtal::DensityMap::new());

        app.move_workspace_object(
            scene_uid,
            NativePropertyObject::DensityMap(map_uid),
            NativePropertyObject::Molecule(molecule_uid),
            false,
        );
        assert_eq!(app.state.density_map_summary(map_uid).unwrap().ui_order, 1);
        assert_eq!(app.state.object_summaries(scene_uid)[0].ui_order, 2);
        assert_eq!(app.undo_stack.len(), 1);
        assert_eq!(app.status, "Move object");

        app.undo(&mut CameraController::default());
        assert!(
            app.state.object_summaries(scene_uid)[0].ui_order
                < app.state.density_map_summary(map_uid).unwrap().ui_order
        );
    }

    #[test]
    fn native_workspace_renderer_selection_preset_is_undoable() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        let renderer_uid = app
            .state
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "lines");

        app.set_workspace_renderer_selection(scene_uid, object_uid, renderer_uid, "protein");
        assert_eq!(
            app.state
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .renderer(renderer_uid)
                .unwrap()
                .qsc_attr("sel"),
            Some("protein")
        );
        assert_eq!(app.undo_stack.len(), 1);

        app.undo(&mut CameraController::default());
        assert_eq!(
            app.state
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .renderer(renderer_uid)
                .unwrap()
                .qsc_attr("sel"),
            None
        );
    }

    #[test]
    fn native_workspace_renderer_type_change_selects_replacement_and_is_undoable() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        let renderer_uid = app
            .state
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "lines");

        app.change_workspace_renderer_type(scene_uid, object_uid, renderer_uid, "cartoon");
        let replacement_uid = app.selected_renderer.unwrap();
        assert_ne!(replacement_uid, renderer_uid);
        assert_eq!(
            app.state
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .renderer(replacement_uid)
                .unwrap()
                .renderer_type(),
            "cartoon"
        );
        assert_eq!(app.undo_stack.len(), 1);

        app.undo(&mut CameraController::default());
        let object = app.state.scene_manager().object(object_uid).unwrap();
        assert!(object.renderer(renderer_uid).is_some());
        assert!(object.renderer(replacement_uid).is_none());
    }

    #[test]
    fn native_workspace_renderer_style_and_coloring_actions_are_undoable() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        let renderer_uid = app
            .state
            .new_renderer(scene_uid, object_uid, "simple", "lines")
            .unwrap()
            .result_renderer;

        app.apply_workspace_renderer_style(
            scene_uid,
            object_uid,
            renderer_uid,
            "ThickSimple",
            "shape",
        );
        assert_eq!(
            app.state.renderer_summaries(object_uid)[0].style.as_deref(),
            Some("DefaultCPKColoring,ThickSimple")
        );
        app.set_workspace_renderer_coloring_preset(scene_uid, object_uid, renderer_uid, "bfactor");
        assert_eq!(app.undo_stack.len(), 2);
        let object = app.state.scene_manager().object(object_uid).unwrap();
        assert!(object
            .renderer(renderer_uid)
            .unwrap()
            .qsc_children()
            .iter()
            .any(|child| child.type_name() == "BfacColoring"));

        app.undo(&mut CameraController::default());
        let object = app.state.scene_manager().object(object_uid).unwrap();
        assert!(!object
            .renderer(renderer_uid)
            .unwrap()
            .qsc_children()
            .iter()
            .any(|child| child.type_name() == "BfacColoring"));

        app.apply_workspace_renderer_style(
            scene_uid,
            object_uid,
            renderer_uid,
            "WoodyHSCPaint",
            "coloring",
        );
        app.state
            .set_molecule_selection_text(object_uid, "protein")
            .unwrap();
        app.insert_workspace_renderer_paint(
            scene_uid,
            object_uid,
            renderer_uid,
            "hsb(240, 1.0, 1.0)",
        );
        let renderer = app
            .state
            .scene_manager()
            .object(object_uid)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        let first_paint = renderer
            .qsc_children()
            .iter()
            .find(|child| child.tag_name() == "coloring")
            .unwrap()
            .children()
            .iter()
            .find(|child| child.tag_name() == "paint")
            .unwrap();
        assert_eq!(first_paint.str_attr("sel").as_str(), "protein");
    }

    #[test]
    fn native_workspace_renderer_clipboard_survives_source_deletion() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let (source_uid, target_uid) = {
            let scene = app.state.scene_manager_mut().scene_mut(scene_uid).unwrap();
            (
                scene.add_molecule("source", cuemol_molstr::MolCoord::new()),
                scene.add_molecule("target", cuemol_molstr::MolCoord::new()),
            )
        };
        let renderer_uid = app
            .state
            .scene_manager_mut()
            .object_mut(source_uid)
            .unwrap()
            .add_renderer("simple", "lines");

        app.copy_workspace_renderer_to_clipboard(source_uid, renderer_uid);
        assert!(app.workspace_renderer_clipboard.is_some());
        assert!(app.undo_stack.is_empty());
        app.state
            .destroy_renderer_in_object(source_uid, renderer_uid)
            .unwrap();
        app.paste_workspace_renderer_clipboard(scene_uid, target_uid, None);
        let pasted_uid = app.selected_renderer.unwrap();
        assert_eq!(
            app.state
                .scene_manager()
                .object(target_uid)
                .unwrap()
                .renderer(pasted_uid)
                .unwrap()
                .renderer_type(),
            "simple"
        );
        assert_eq!(app.undo_stack.len(), 1);

        app.undo(&mut CameraController::default());
        assert!(app
            .state
            .scene_manager()
            .object(target_uid)
            .unwrap()
            .renderer(pasted_uid)
            .is_none());
        assert!(app.workspace_renderer_clipboard.is_some());
    }

    #[test]
    fn native_workspace_object_clipboard_pastes_and_undoes() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        app.state
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "lines");

        app.copy_workspace_object_to_clipboard(object_uid);
        assert!(app.workspace_object_clipboard.is_some());
        assert!(app.undo_stack.is_empty());
        app.paste_workspace_object_clipboard(scene_uid);
        let pasted_uid = app.selected_object.unwrap();
        assert_ne!(pasted_uid, object_uid);
        assert_eq!(
            app.state.scene_manager().object(pasted_uid).unwrap().name(),
            "copy1_molecule"
        );
        assert_eq!(app.undo_stack.len(), 1);

        app.undo(&mut CameraController::default());
        assert!(app.state.scene_manager().object(pasted_uid).is_none());
        assert!(app.workspace_object_clipboard.is_some());
    }

    #[test]
    fn native_workspace_creates_renderer_group_with_undo() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());

        app.open_renderer_group_dialog(object_uid);
        assert_eq!(app.new_renderer_type, "*group");
        assert_eq!(app.new_renderer_name, "group1");
        app.create_renderer_from_dialog();
        assert!(app
            .state
            .renderer_summaries(object_uid)
            .iter()
            .any(|renderer| renderer.renderer_type == "*group" && renderer.name == "group1"));
        assert_eq!(app.undo_stack.len(), 1);

        app.undo(&mut CameraController::default());
        assert!(app.state.renderer_summaries(object_uid).is_empty());
    }

    #[test]
    fn native_renderer_property_dialog_keeps_an_isolated_draft() {
        let ctx = egui::Context::default();
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("object", cuemol_molstr::MolCoord::new());
        let renderer_uid = app
            .state
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "simple");
        app.open_renderer_properties(object_uid, renderer_uid);
        app.renderer_properties_draft.as_mut().unwrap().name = "draft name".to_string();
        let revision = app.snapshot_revision;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.renderer_props_dialog(ctx)
        });
        assert!(app.renderer_properties_draft.is_some());
        assert_eq!(app.snapshot_revision, revision);
        assert_eq!(
            app.state
                .scene_manager()
                .object(object_uid)
                .unwrap()
                .renderer(renderer_uid)
                .unwrap()
                .name(),
            "simple"
        );
    }

    #[test]
    fn native_scene_property_dialog_keeps_an_isolated_draft() {
        let ctx = egui::Context::default();
        let mut app = GuiApplication::new();
        app.open_scene_properties();
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.scene_props_dialog(ctx));
        assert!(app.show_scene_props);
        assert!(app.scene_properties_draft.is_some());
        assert_eq!(app.background, SceneBackground::Black);
    }

    #[test]
    fn native_view_property_dialog_keeps_an_isolated_draft() {
        let ctx = egui::Context::default();
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.viewport_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 640.0),
        ));
        app.open_view_properties(&camera);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.view_props_dialog(ctx, &mut camera)
        });
        assert!(app.show_view_props);
        assert!(app.view_properties_draft.is_some());
        assert_eq!(camera.name, "__current");
    }

    #[test]
    fn trackball_move_accumulates_normalized_rotation() {
        let mut camera = CameraController::default();
        // Horizontal drag across the center of the viewport.
        camera.trackball_move(0.6, 0.5, 0.4, 0.5, 0.8);
        assert!(camera.orientation.is_normalized());
        assert!(!camera.orientation.abs_diff_eq(Quat::IDENTITY, 1.0e-4));

        // A zero-length drag must leave the orientation untouched.
        let before = camera.orientation;
        camera.trackball_move(0.5, 0.5, 0.5, 0.5, 0.8);
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
            url_cache_path("https://example.test/foo/1CRN.pdb?download=1")
                .ends_with("cuemol-url-1CRN.pdb")
        );
    }

    #[test]
    fn native_gui_reuses_a_nonempty_download_cache() {
        let path = std::env::temp_dir().join(format!(
            "cuemol-gui-cache-test-{}-{}.pdb",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        fs::write(&path, b"cached PDB").unwrap();
        assert_eq!(
            download_to_cache("invalid://must-not-be-fetched", &path).unwrap(),
            path
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_gui_pdb_download_defaults_match_reference_dialog() {
        let mut app = GuiApplication::new();
        assert!(app.pdb_id.is_empty());
        assert!(app.pdb_history.is_empty());
        assert!(!app.pdb_history_loaded);
        assert!(app.pdb_fetch_coordinates);
        assert_eq!(app.pdb_coordinate_source, "RCSB_CIF");
        assert!(!app.pdb_fetch_two_fofc);
        assert!(!app.pdb_fetch_fofc);
        assert_eq!(app.pdb_map_source, "RCSB_CIF");
        let path = PathBuf::from("sample.pdb");
        app.open_renderer_setup(
            "sample.pdb",
            NativeRendererSetupAction::OpenPath(path.clone()),
        );
        let setup = app.renderer_setup_draft.as_ref().unwrap();
        assert_eq!(setup.object_name, "sample.pdb");
        assert!(matches!(
            &setup.action,
            NativeRendererSetupAction::OpenPath(value) if value == &path
        ));
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
            edge: None,
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

    #[test]
    fn left_panel_order_and_defaults_match_cuemol2() {
        let panels = LeftPanel::ALL.map(|panel| (panel.title(), panel.default_open()));
        assert_eq!(
            panels,
            [
                ("Workspace", true),
                ("MolStruct", true),
                ("Selection", true),
                ("Coloring", true),
                ("Symmetry", false),
                ("Density map", true),
                ("View", false),
                ("Animation", false),
            ]
        );
    }

    #[test]
    fn native_molecule_panels_reuse_unchanged_hierarchy_data() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        app.state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("cached", cuemol_molstr::MolCoord::new());

        let structure = app.cached_active_structure_tree();
        let sequence = app.cached_active_sequence_summaries();
        assert_eq!(structure.len(), 1);
        assert!(sequence.is_empty());
        assert!(Arc::ptr_eq(&structure, &app.cached_active_structure_tree()));
        assert!(Arc::ptr_eq(
            &sequence,
            &app.cached_active_sequence_summaries()
        ));

        app.rebuild_snapshot();
        assert!(Arc::ptr_eq(&structure, &app.cached_active_structure_tree()));

        app.state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("new", cuemol_molstr::MolCoord::new());
        assert!(!Arc::ptr_eq(
            &structure,
            &app.cached_active_structure_tree()
        ));
        assert_eq!(app.cached_active_structure_tree().len(), 2);
    }

    #[test]
    fn native_bond_editor_lists_and_removes_selected_nonstandard_bonds() {
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().into_owned();
        app.open_current_path(&mut camera);
        let object_uid = app.state.active_object_summaries()[0].uid;
        app.bond_object = Some(object_uid);
        let before = app.state.nonstandard_bond_summaries(object_uid).unwrap();

        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        app.viewport_rect = Some(viewport);
        camera.fit_snapshot(&app.snapshot);
        let atom = app.state.active_pick_atoms("").unwrap()[0].clone();
        let (screen, _) = camera
            .project_to_viewport(vec3_from_vector(atom.center), viewport)
            .expect("project bond picker atom");
        let selection_before = app.selection_text.clone();
        app.tool_dialog = Some("Mol bond editor".to_string());
        app.bond_picker_target = Some(0);
        assert!(app.pick_atom_at(&camera, screen));
        assert!(app.bond_aid1.parse::<i32>().is_ok());
        assert_eq!(app.selection_text, selection_before);

        app.bond_aid1 = "1".to_string();
        app.bond_aid2 = "327".to_string();
        app.edit_bond(true);
        assert_eq!(app.status, "Bond created");
        assert!(app.bond_aid1.is_empty());
        assert!(app.bond_picker_target.is_none());
        let added = app
            .state
            .nonstandard_bond_summaries(object_uid)
            .unwrap()
            .into_iter()
            .find(|bond| {
                [bond.atom1.serial, bond.atom2.serial]
                    .into_iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    == std::collections::BTreeSet::from([1, 327])
            })
            .expect("created bond is listed");
        app.selected_nonstandard_bonds.insert((
            added.atom1.serial.min(added.atom2.serial),
            added.atom1.serial.max(added.atom2.serial),
        ));
        app.remove_selected_nonstandard_bonds();
        assert_eq!(app.status, "Removed 1 non-standard bond(s)");
        assert_eq!(
            app.state.nonstandard_bond_summaries(object_uid).unwrap(),
            before
        );
    }

    #[test]
    fn native_interaction_tool_defines_cross_molecule_renderer() {
        let Some(pdb) = reference_pdb_path() else {
            return;
        };
        let mut app = GuiApplication::new();
        let mut camera = CameraController::default();
        app.open_path = pdb.to_string_lossy().into_owned();
        app.open_current_path(&mut camera);
        app.open_current_path(&mut camera);
        let objects = app.state.active_object_summaries();
        let first = objects[0].uid;
        let second = objects[1].uid;
        app.contact_object = Some(first);
        app.contact_selection1 = "name CA".to_string();
        app.contact_use_object2 = true;
        app.contact_object2 = Some(second);
        app.contact_use_selection2 = true;
        app.contact_selection2 = "name CA".to_string();
        app.contact_r_min = 0.0;
        app.contact_r_max = 0.1;
        app.contact_hbond = false;
        app.contact_max_count = 2;
        app.contact_renderer_name = "measure".to_string();

        app.calculate_contacts();

        assert_eq!(app.status, "Defined 2 interaction(s) in measure");
        let renderer_uid = app
            .selected_renderer
            .expect("selected interaction renderer");
        let renderer = app
            .state
            .scene_manager()
            .object(first)
            .unwrap()
            .renderer(renderer_uid)
            .unwrap();
        assert_eq!(renderer.renderer_type(), "atomintr");
        assert_eq!(renderer.atom_intr_lines().len(), 2);
        assert_eq!(renderer.atom_intr_lines()[0].positions().len(), 2);
        app.undo(&mut camera);
        assert!(app
            .state
            .scene_manager()
            .object(first)
            .unwrap()
            .renderer(renderer_uid)
            .is_none());
    }

    #[test]
    fn native_crystal_restrictions_match_reference_lattices() {
        let mut draft = NativeCrystalDraft {
            object_uid: cuemol_core::Uid::new(1),
            lattice: "CUBIC".to_string(),
            space_group_number: 195,
            restrict_by_symmetry: true,
            a: 42.0,
            b: 20.0,
            c: 30.0,
            alpha: 70.0,
            beta: 80.0,
            gamma: 100.0,
        };
        restrict_crystal_draft(&mut draft, "P 2 3");
        assert_eq!((draft.a, draft.b, draft.c), (42.0, 42.0, 42.0));
        assert_eq!((draft.alpha, draft.beta, draft.gamma), (90.0, 90.0, 90.0));
        assert_eq!(
            crystal_field_locks(&draft, "P 2 3"),
            (true, true, true, true, true)
        );

        draft.lattice = "TRIGONAL".to_string();
        draft.b = 10.0;
        draft.c = 20.0;
        draft.alpha = 75.0;
        restrict_crystal_draft(&mut draft, "R 3");
        assert_eq!((draft.b, draft.c), (42.0, 42.0));
        assert_eq!(draft.alpha, 75.0);
    }

    #[test]
    fn native_surface_cutter_separates_and_undoes_loaded_surface() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.ensure_active_scene();
        let molecule = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n",
        )
        .unwrap();
        let mut object = cuemol_surface::MolSurfObj::new();
        object.create_ses_from_mol(&molecule, cuemol_molstr::Selector::All, 1.0, 1.4);
        let raw_data = cuemol_surface::qdf::write_qdf_surface_bytes(&object);
        let mut node = cuemol_core::LDom2Node::new();
        node.set_tag_name("object");
        node.set_type_name("MolSurfObj");
        node.set_str_attr("name", "surface");
        node.set_str_attr("src", "datachunk:");
        node.set_str_attr("srctype", "qdfsurf");
        let surface_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_loaded_surface(cuemol_molstr::LoadedSurface {
                uid: cuemol_core::Uid::new(0),
                name: "surface".to_string(),
                object_type: "MolSurfObj".to_string(),
                srctype: "qdfsurf".to_string(),
                data: String::from_utf8_lossy(&raw_data).into_owned(),
                raw_data,
                qsc_node: Some(node),
            });
        app.cut_surface_target = Some(surface_uid);
        app.cut_surface_mode = "separate".to_string();
        app.cut_surface_density = 1.0;
        let mut camera = CameraController {
            target: Vec3::new(0.0, 0.0, -120.0),
            slab_depth: 240.0,
            ..CameraController::default()
        };

        app.cut_surface_by_current_plane(&camera);

        assert_eq!(app.state.active_surface_summaries().len(), 2);
        assert!(app
            .state
            .active_surface_summaries()
            .iter()
            .any(|surface| surface.name == "sect1_surface"));
        assert!(app.status.starts_with("Surface cut:"));
        app.undo(&mut camera);
        assert_eq!(app.state.active_surface_summaries().len(), 1);
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
            assert!(app.snapshot.bond_count() > 0);
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
                .state
                .active_pick_atoms(&app.selection_text)
                .expect("loaded molecule pick atoms")
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
    fn navigation_selection_expressions_match_cuemol2_operations() {
        assert_eq!(atom_id_selection(&[7, 3, 2, 1, 7, 10]), "aid 1:3,7,10");
        assert_eq!(add_selection_expression("", "aid 2"), "aid 2");
        assert_eq!(
            add_selection_expression("aid 1", "aid 2"),
            "(aid 1) or (aid 2)"
        );
        assert_eq!(invert_selection_expression(""), "*");
        assert_eq!(invert_selection_expression("aid 1"), "!(aid 1)");
        assert_eq!(invert_selection_expression("!(aid 1)"), "aid 1");
        assert_eq!(toggle_sidechain_expression("aid 1"), "bysidech aid 1");
        assert_eq!(toggle_sidechain_expression("bysidech aid 1"), "aid 1");
        assert_eq!(
            around_selection_expression("byres (aid 1) around 3", 5.0, false),
            "(aid 1) around 5"
        );
        assert_eq!(
            around_selection_expression("(aid 1) around 5", 10.0, true),
            "byres (aid 1) around 10"
        );
        for expression in [
            atom_id_selection(&[1, 2, 3, 7]),
            add_selection_expression("aid 1", "aid 2"),
            invert_selection_expression("aid 1"),
            toggle_sidechain_expression("aid 1"),
            around_selection_expression("aid 1", 5.0, true),
        ] {
            cuemol_molstr::parse_selector(&expression).expect("valid selection expression");
        }
    }

    #[test]
    fn rectangle_selection_updates_the_hit_molecule_selection() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().expect("active scene");
        let molecule = cuemol_molstr::read_pdb_str(
            "ATOM      1  CA  GLY A   1      -6.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      2  CA  GLY A   2       0.000   0.000   0.000  1.00 10.00           C  \n\
             ATOM      3  CA  GLY A   3       6.000   0.000   0.000  1.00 10.00           C  \n\
             CONECT    1    2\n\
             CONECT    2    3\n\
             END\n",
        )
        .expect("parse molecule");
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .expect("scene")
            .add_molecule("rect", molecule);
        app.state
            .scene_manager_mut()
            .object_mut(object_uid)
            .expect("object")
            .add_renderer("simple", "simple");
        app.rebuild_snapshot();

        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        app.viewport_rect = Some(viewport);
        let mut camera = CameraController::default();
        camera.fit_snapshot(&app.snapshot);
        let first = app
            .state
            .active_pick_atoms("")
            .expect("pick atoms")
            .into_iter()
            .find(|atom| atom.atom_index == Some(0))
            .expect("first atom");
        let (screen, _) = camera
            .project_to_viewport(vec3_from_vector(first.center), viewport)
            .expect("project first atom");

        let selected = app.select_atoms_in_screen_rect(
            &camera,
            egui::Rect::from_center_size(screen, egui::vec2(4.0, 4.0)),
        );

        assert_eq!(selected, 1);
        assert_eq!(
            app.state.molecule_selection_text(object_uid).unwrap(),
            "aid 1"
        );
        assert_eq!(
            app.state
                .active_pick_atoms("*")
                .unwrap()
                .iter()
                .filter(|atom| atom.selected)
                .count(),
            1
        );
        assert_eq!(app.selection_text, "aid 1");
        assert_eq!(app.selected_object, Some(object_uid));

        assert!(app.apply_molecule_selection(object_uid, "aid 2".to_string(), "test selection"));
        assert_eq!(
            app.state.molecule_selection_text(object_uid).unwrap(),
            "aid 2"
        );
        app.undo(&mut camera);
        assert_eq!(
            app.state.molecule_selection_text(object_uid).unwrap(),
            "aid 1"
        );
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
                .bonds
                .iter()
                .flat_map(|bond| [&bond.color, &bond.end_color])
                .filter_map(|color| color.as_ref().and_then(|color| color.rgba8()))
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
            edge: None,
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
        for renderer in app.state.renderer_summaries(object_uid) {
            app.state
                .set_renderer_visible(object_uid, renderer.uid, false)
                .expect("hide existing renderer");
        }
        let renderer_uid = app
            .state
            .new_renderer_in_active_scene(object_uid, "ballstick", "test")
            .expect("create renderer")
            .result_renderer;

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

        // (1b2a) Workspace tree with molecule, density-map, and loaded-surface objects.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            let scene_uid = app.state.active_scene().unwrap();
            let map_uid =
                app.state
                    .add_density_map(scene_uid, "2Fo-Fc map", synthetic_gaussian_map());
            let renderer_uid = app.state.add_density_renderer(map_uid, "contour").unwrap();
            let data = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
            app.state
                .scene_manager_mut()
                .scene_mut(scene_uid)
                .unwrap()
                .add_loaded_surface(cuemol_molstr::LoadedSurface {
                    uid: cuemol_core::Uid::new(0),
                    name: "Molecular surface".to_string(),
                    object_type: "MolSurfObj".to_string(),
                    srctype: "obj".to_string(),
                    data: data.to_string(),
                    raw_data: data.as_bytes().to_vec(),
                    qsc_node: None,
                });
            if let Some(object_uid) = app
                .state
                .active_object_summaries()
                .first()
                .map(|object| object.uid)
            {
                let group_uid = {
                    let object = app
                        .state
                        .scene_manager_mut()
                        .object_mut(object_uid)
                        .unwrap();
                    let group_uid = object.add_renderer("*group", "Representations");
                    for (renderer_type, name) in [("ribbon", "Ribbon"), ("cpk", "Atoms")] {
                        let renderer_uid = object.add_renderer(renderer_type, name);
                        object
                            .renderer_mut(renderer_uid)
                            .unwrap()
                            .set_qsc_attr("group", "Representations");
                    }
                    group_uid
                };
                app.selected_object = Some(object_uid);
                app.selected_renderer = Some(group_uid);
                app.workspace_aux_selection = None;
            } else {
                app.workspace_aux_selection = Some(WorkspaceAuxSelection::DensityMap {
                    scene_uid,
                    map_uid,
                    renderer_uid: Some(renderer_uid),
                });
            }
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1100.0, 850.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("workspace_density_surface.png"));
        }

        // (1b2b) Actual loaded-surface renderer on the MolSurf property page.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            let scene_uid = app.state.active_scene().unwrap();
            let data = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
            let surface_uid = app
                .state
                .scene_manager_mut()
                .scene_mut(scene_uid)
                .unwrap()
                .add_loaded_surface(cuemol_molstr::LoadedSurface {
                    uid: cuemol_core::Uid::new(0),
                    name: "Molecular surface".to_string(),
                    object_type: "MolSurfObj".to_string(),
                    srctype: "obj".to_string(),
                    data: data.to_string(),
                    raw_data: data.as_bytes().to_vec(),
                    qsc_node: None,
                });
            let renderer_uid = app.state.surface_summary(surface_uid).unwrap().renderers[0].uid;
            app.open_surface_renderer_properties(surface_uid, renderer_uid);
            app.renderer_properties_draft.as_mut().unwrap().selected_tab = 1;
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1000.0, 760.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(
                &mut harness,
                &out_dir.join("surface_renderer_properties.png"),
            );
        }

        // (1b3) Reference Options window with the Misc pane open.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            app.open_options_dialog();
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1000.0, 760.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("options.png"));
        }

        // (1b4) Workspace Object property dialog with the reference Common page.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            if let Some(object_uid) = app
                .state
                .active_object_summaries()
                .first()
                .map(|object| object.uid)
            {
                app.selected_object = Some(object_uid);
                app.open_object_properties(object_uid);
            }
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1000.0, 760.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("object_properties.png"));
        }

        // (1b5) Workspace Renderer property dialog with draft-only editing.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            if let Some(object_uid) = app
                .state
                .active_object_summaries()
                .first()
                .map(|object| object.uid)
            {
                if let Some(renderer_uid) = app
                    .state
                    .renderer_summaries(object_uid)
                    .first()
                    .map(|renderer| renderer.uid)
                {
                    app.selected_object = Some(object_uid);
                    app.selected_renderer = Some(renderer_uid);
                    app.open_renderer_properties(object_uid, renderer_uid);
                }
            }
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1000.0, 760.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("renderer_properties.png"));
        }

        // (1b5b) Renderer style creation dialog.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            let scene_uid = app.state.active_scene().unwrap();
            app.state.create_scene_style(scene_uid, "user").unwrap();
            if let Some(object_uid) = app
                .state
                .active_object_summaries()
                .first()
                .map(|object| object.uid)
            {
                if let Some(renderer_uid) = app
                    .state
                    .renderer_summaries(object_uid)
                    .first()
                    .map(|renderer| renderer.uid)
                {
                    app.open_renderer_style_create(scene_uid, object_uid, renderer_uid);
                    app.renderer_style_create_draft
                        .as_mut()
                        .unwrap()
                        .name_prefix = "Presentation".to_string();
                }
            }
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1000.0, 760.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("renderer_style_create.png"));
        }

        // (1b6) Ribbon renderer child-property page.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            if let Some(object_uid) = app
                .state
                .active_object_summaries()
                .first()
                .map(|object| object.uid)
            {
                let renderer_uid = app
                    .state
                    .scene_manager_mut()
                    .object_mut(object_uid)
                    .unwrap()
                    .add_renderer("ribbon", "ribbon");
                app.selected_object = Some(object_uid);
                app.selected_renderer = Some(renderer_uid);
                app.open_renderer_properties(object_uid, renderer_uid);
                app.renderer_properties_draft.as_mut().unwrap().selected_tab = 1;
            }
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1000.0, 760.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("ribbon_properties.png"));
        }

        // (1b7) Generic Scene property editor with the full Scene.qif table.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            app.open_scene_properties();
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1100.0, 820.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("scene_properties.png"));
        }

        // (1b8) Generic View property editor with the full reference property table.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            app.viewport_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 640.0),
            ));
            app.open_view_properties(&camera);
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1100.0, 820.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("view_properties.png"));
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

        // (1c2) Rectangle-selection drag overlay plus the completed atom context menu.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            load(&mut app, &mut camera);
            app.interaction_mode = InteractionMode::RectSelect;
            app.rect_select_start = Some(egui::pos2(430.0, 190.0));
            app.rect_select_current = Some(egui::pos2(730.0, 470.0));
            app.viewport_menu = app.state.active_pick_atoms("").ok().and_then(|atoms| {
                atoms.into_iter().find_map(|atom| {
                    Some(ViewportMenu {
                        pos: egui::pos2(760.0, 180.0),
                        hit: Some(PickHit {
                            object_uid: atom.object_uid?,
                            atom_index: atom.atom_index?,
                        }),
                    })
                })
            });
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1100.0, 850.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("navigation_selection.png"));
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

        // (1e) Animation-render preview dialog with a decoded PNG frame. This verifies
        // the reference frame slider, aspect-preserving image area, and file label.
        {
            let preview_path = out_dir.join("animation-preview-frame_0000.png");
            let preview_image = image::RgbaImage::from_fn(640, 360, |x, y| {
                let red = ((x as f32 / 639.0) * 180.0 + 30.0) as u8;
                let green = ((y as f32 / 359.0) * 140.0 + 40.0) as u8;
                image::Rgba([red, green, 150, 255])
            });
            preview_image
                .save(&preview_path)
                .expect("write animation preview source");

            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            app.animation_render_settings.output_dir = out_dir.to_string_lossy().into_owned();
            app.animation_render_settings.base_name = "animation-preview-frame".to_string();
            app.animation_render_settings.tab = AnimationRenderTab::Preview;
            app.animation_render_total_frames = 1;
            app.tool_dialog = Some("Animation rendering".to_string());
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1000.0, 720.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("animation_render_preview.png"));
            fs::remove_file(preview_path).expect("remove animation preview source");
        }

        // (1f) Reference-density POV-Ray options page: executable/include paths,
        // render toggles, and radiosity preset in the compact native dialog.
        {
            let mut app = GuiApplication::new();
            let mut camera = CameraController::default();
            app.animation_render_settings.tab = AnimationRenderTab::Render;
            app.animation_render_settings.povray_include_dir =
                "/usr/local/share/povray/include".to_string();
            app.tool_dialog = Some("Animation rendering".to_string());
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1000.0, 720.0))
                .build(|ctx| {
                    app.ui(ctx, &mut camera);
                });
            harness.run();
            save_harness(&mut harness, &out_dir.join("animation_render_options.png"));
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
        // displays the surface through the shared render snapshot, rendered offscreen.
        // Verifies the whole chain: loader captures the surface datachunk → the core
        // converts it → every renderer and exporter sees it.
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
                app.snapshot.mesh_count() > 0,
                "loading a .qsc with an OBJ surface must populate the render snapshot"
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
