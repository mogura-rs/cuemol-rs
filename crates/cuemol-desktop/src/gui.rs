use crate::{DesktopAppState, GuiLoadedPath};
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
    view_save_path: String,
    export_path: String,
    log_save_path: String,
    pdb_id: String,
    script_path: String,
    console_input: String,
    selection_text: String,
    selected_object: Option<cuemol_core::Uid>,
    selected_renderer: Option<cuemol_core::Uid>,
    snapshot: RenderSceneSnapshot,
    background: SceneBackground,
    center_mark: CenterMark,
    color_proofing: bool,
    bottom_tab: BottomTab,
    render_preset: String,
    render_backend: String,
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
    perf_measure: bool,
    animation_frames: usize,
    animation_playing: bool,
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

impl GuiApplication {
    fn new() -> Self {
        let mut state = DesktopAppState::new();
        state.new_scene("Untitled", true);
        let mut app = Self {
            state,
            open_path: "../cuemol2/tests/test_data/1CRN.pdb".to_string(),
            save_path: "scene.qsc".to_string(),
            object_save_path: "object.pdb".to_string(),
            view_save_path: "current_view.cam".to_string(),
            export_path: "scene.pov".to_string(),
            log_save_path: "cuemol.log".to_string(),
            pdb_id: "1CRN".to_string(),
            script_path: "script.js".to_string(),
            console_input: String::new(),
            selection_text: String::new(),
            selected_object: None,
            selected_renderer: None,
            snapshot: RenderSceneSnapshot::empty(),
            background: SceneBackground::Black,
            center_mark: CenterMark::Cross,
            color_proofing: false,
            bottom_tab: BottomTab::Output,
            render_preset: RENDER_PRESETS[1].0.to_string(),
            render_backend: "POV-Ray".to_string(),
            show_toolbar: true,
            show_workspace: true,
            show_inspector: true,
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
            perf_measure: false,
            animation_frames: 0,
            animation_playing: false,
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
        };
        app.rebuild_snapshot();
        app
    }

    fn ui(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        self.pixels_per_point = ctx.pixels_per_point();
        self.top_bar(ctx, camera);
        if self.show_workspace {
            self.workspace_panel(ctx, camera);
        }
        if self.show_inspector {
            self.inspector_panel(ctx, camera);
        }
        if self.show_bottom_panel {
            self.bottom_panel(ctx);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                self.viewport_rect = Some(rect);
                self.draw_center_mark(ui, rect);
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
        self.dialogs(ctx, camera);
    }

    fn top_bar(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
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
                        self.set_status(format!("Color proofing: {}", self.color_proofing));
                    }
                    if ui.button("Properties...").clicked() {
                        self.show_scene_props = true;
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
            if self.show_toolbar {
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("New Tab").clicked() {
                        self.new_scene(camera);
                    }
                    if ui.button("Open File").clicked() {
                        self.open_current_path(camera);
                    }
                    if ui.button("Save").clicked() {
                        self.save_current_scene(camera);
                    }
                    if ui.button("Save As").clicked() {
                        self.save_current_scene(camera);
                    }
                    ui.separator();
                    if ui.button("Open Scene").clicked() {
                        self.open_current_path(camera);
                    }
                    if ui.button("Reload Scene").clicked() {
                        self.reload_current_path(camera);
                    }
                    if ui.button("Save Scene").clicked() {
                        self.save_current_scene(camera);
                    }
                    ui.separator();
                    if ui.button("Get PDB").clicked() {
                        self.show_get_pdb = true;
                    }
                    if ui.button("Render").clicked() {
                        self.bottom_tab = BottomTab::Render;
                    }
                    ui.separator();
                    if ui.button("Undo").clicked() {
                        self.undo(camera);
                    }
                    if ui.button("Redo").clicked() {
                        self.redo(camera);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Open");
                    let open = ui.add_sized(
                        [360.0, 22.0],
                        egui::TextEdit::singleline(&mut self.open_path),
                    );
                    if open.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                        self.open_current_path(camera);
                    }
                    if ui.button("Open").clicked() {
                        self.open_current_path(camera);
                    }
                    if ui.button("Fit").clicked() {
                        camera.fit_snapshot(&self.snapshot);
                    }
                });
            }
        });
    }

    fn workspace_panel(&mut self, ctx: &egui::Context, camera: &mut CameraController) {
        egui::SidePanel::left("workspace")
            .resizable(true)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.heading("Workspace");
                ui.separator();
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
                                    let label =
                                        format!("{} ({})", renderer.name, renderer.renderer_type);
                                    if ui.selectable_label(selected, label).clicked() {
                                        self.selected_object = Some(object.uid);
                                        self.selected_renderer = Some(renderer.uid);
                                    }
                                }
                                if ui.small_button("+ Renderer").clicked() {
                                    self.open_renderer_dialog(
                                        object.uid,
                                        object.renderer_count + 1,
                                    );
                                }
                            });
                        }
                    });
                }
            });
    }

    fn inspector_panel(&mut self, ctx: &egui::Context, camera: &CameraController) {
        egui::SidePanel::right("inspector")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Inspector");
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
                    ui.separator();
                    ui.label(&self.status);
                    ui.separator();
                    ui.label(format!(
                        "{} atoms / {} bonds",
                        self.snapshot.atom_count(),
                        self.snapshot.bond_count()
                    ));
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

    fn sequence_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for row in self.state.active_sequence_summaries() {
                ui.horizontal(|ui| {
                    ui.monospace(format!("{}:{}", row.object_name, row.chain_id));
                    ui.label(format!(
                        "{} residues / {} atoms",
                        row.residue_count, row.atom_count
                    ));
                    if ui.button("Select").clicked() {
                        self.selected_object = Some(row.object_uid);
                        self.selection_text = format!("chain {}", row.chain_id);
                        self.rebuild_snapshot();
                    }
                    if ui.button("Center").clicked() {
                        self.selected_object = Some(row.object_uid);
                        self.selection_text = format!("chain {}", row.chain_id);
                        self.rebuild_snapshot();
                        self.set_status(format!("Selected chain {}", row.chain_id));
                    }
                });
                ui.monospace(row.sequence);
                ui.separator();
            }
        });
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
                self.set_status(format!("Opened scene {}", scene_uid.get()));
                self.rebuild_snapshot();
                camera.fit_snapshot(&self.snapshot);
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
        match self
            .state
            .export_active_scene_pov(self.export_path.trim(), &self.selection_text)
        {
            Ok(count) => self.set_status(format!(
                "Exported {count} atoms to {}",
                self.export_path.trim()
            )),
            Err(error) => self.set_error(error),
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
        match fs::read_to_string(self.script_path.trim()) {
            Ok(script) => {
                self.log_lines
                    .push(format!("Loaded script: {}", self.script_path.trim()));
                for line in script.lines().take(20) {
                    self.log_lines.push(format!("  {line}"));
                }
                if script.lines().count() > 20 {
                    self.log_lines.push("  ...".to_string());
                }
                self.set_status("Script loaded into log");
            }
            Err(error) => self.set_status(format!("Error: {error}")),
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
                ui.checkbox(&mut self.color_proofing, "Color proofing");
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
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed if self.pointer_in_viewport(camera.last_cursor) => {
                    camera.drag_mode = CameraDragMode::from_button(*button);
                    camera.last_drag_cursor = camera.last_cursor;
                    camera.drag_total = 0.0;
                    camera.drag_mode.is_some()
                }
                ElementState::Released => {
                    let was_dragging = camera.drag_mode.is_some();
                    let should_pick = *button == MouseButton::Left
                        && camera.drag_mode == Some(CameraDragMode::Rotate)
                        && camera.drag_total < 4.0
                        && self.pointer_in_viewport(camera.last_cursor);
                    if should_pick {
                        if let Some(position) = camera.last_cursor {
                            self.pick_atom_at(camera, position);
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
    ) -> std::result::Result<(), wgpu::SurfaceError> {
        self.prepare_snapshot(snapshot);
        let camera_uniform = self.camera.uniform(self.size);
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
        });
        assert_eq!(
            app.pick_hit(&camera, egui::pos2(400.0, 300.0)),
            Some(PickHit {
                object_uid: 7,
                atom_index: 3,
            })
        );
    }
}
