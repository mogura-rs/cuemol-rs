//! gpui-based desktop GUI (parallel to the egui `gui` module).
//!
//! gpui (Zed's GPU UI framework) draws the panels/chrome; the 3D molecular view
//! is rendered separately with wgpu and composited as an image (see
//! `gpui-gui-direction` memory). This module is the panel shell that reuses the
//! headless [`DesktopAppState`] logic, exactly as `GuiApplication` does for egui.
//!
//! This first milestone is the panel layout + scene/object/renderer tree driven
//! by `DesktopAppState`; the wgpu 3D viewport is a placeholder pending the
//! offscreen-render compositing step.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    div, img, px, rgb, size, App, Application, Bounds, Context, Entity, ImageSource, RenderImage,
    Window, WindowBounds, WindowOptions,
};

use crate::gpui_widgets::TextInput;
use crate::offscreen::SceneRenderer;
use crate::{
    DesktopAppState, GuiLoadedPath, GuiObjectSummary, GuiRendererSummary, GuiSceneSummary,
};

/// The persistent text-input entities (always-visible fields). gpui has no
/// turnkey text widget, so each is its own `Entity<TextInput>` (see
/// [`crate::gpui_widgets`]). Created lazily by [`CueMolGpuiApp::ensure_inputs`]
/// so the headless `#[cfg(test)]` constructor (which has no gpui `App`) still works
/// — there the field stays `None` and actions take explicit string arguments.
struct Inputs {
    /// Selection expression (e.g. `name CA`); typing it re-tessellates the scene.
    selection: Entity<TextInput>,
    /// File path to open (PDB/CIF/SDF/QSC/map). Read by the toolbar "Open" button.
    open_path: Entity<TextInput>,
    /// Destination path for "Save scene" (`.qsc`).
    save_qsc: Entity<TextInput>,
    /// Destination path for "Save object" (`.pdb`/`.sdf`).
    save_obj: Entity<TextInput>,
    /// Destination path for "Export POV-Ray" (`.pov`).
    export_pov: Entity<TextInput>,
}

// Chrome sizes (logical px). Shared by BOTH the on-screen layout and the
// offscreen render-resolution math so the wgpu image matches the visible region
// (a single source of truth — change one and the other follows).
const SIDEBAR_W: f32 = 300.0;
const HEADER_H: f32 = 40.0;
const STATUS_H: f32 = 26.0;

// Offscreen resolution used before the first real window layout is known (the
// view re-renders at the true viewport size on the first `render` pass).
const DEFAULT_VIEW_W: u32 = 960;
const DEFAULT_VIEW_H: u32 = 720;

// A small dark palette (hex RGB) so the panels read as a cohesive theme.
const COL_BG: u32 = 0x1e1e28;
const COL_PANEL: u32 = 0x252530;
const COL_HEADER: u32 = 0x2d2d3a;
const COL_VIEWPORT: u32 = 0x14141c;
const COL_BORDER: u32 = 0x3a3a48;
const COL_TEXT: u32 = 0xd6d6e0;
const COL_TEXT_DIM: u32 = 0x8a8a9a;
const COL_ACCENT: u32 = 0x6db3f2;
const COL_BTN: u32 = 0x39394a;
const COL_BTN_HOVER: u32 = 0x474760;
const COL_SEL: u32 = 0x34506e; // selected tree-row background
const COL_DANGER: u32 = 0xc2554f;

const INSPECTOR_W: f32 = 320.0;
const BOTTOM_H: f32 = 180.0;
const TOOLBAR_H: f32 = 36.0;

/// Renderer types offered in the "Add renderer" dialog (id, human label).
/// Mirrors the egui front end's `RENDERER_TYPES` so both GUIs expose the same set.
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
];

/// Top-level menus: (title, dropdown left-offset px, title width px). The offsets
/// are the cumulative widths of the brand (92) + preceding titles, so each
/// dropdown opens roughly under its title. Keep widths in step with the offsets.
const MENUS: &[(&str, f32, f32)] = &[
    ("File", 92.0, 46.0),
    ("Edit", 138.0, 46.0),
    ("View", 184.0, 50.0),
    ("Tools", 234.0, 56.0),
    ("Help", 290.0, 50.0),
];

/// Viewport background, toggled from the View menu / toolbar. The slate default
/// blends with the surrounding panel chrome; Black/White match the egui options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Background {
    Slate,
    Black,
    White,
}

impl Background {
    fn clear(self) -> [f64; 4] {
        match self {
            Background::Slate => [0.08, 0.08, 0.11, 1.0],
            Background::Black => [0.0, 0.0, 0.0, 1.0],
            Background::White => [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// Center-mark overlay drawn over the 3D viewport (the camera origin indicator).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CenterMarkKind {
    None,
    Cross,
    Axis,
}

/// Which tab the bottom panel shows. Mirrors egui's four tabs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Output,
    Sequence,
    Animation,
    Render,
}

/// The currently open modal dialog (only one at a time). Dialog-specific text
/// fields live in [`DialogInputs`], created when the dialog opens and dropped on
/// close so we don't keep ~30 persistent `TextInput` entities alive.
#[derive(Clone, PartialEq)]
enum Dialog {
    None,
    About,
    NewRenderer {
        target: cuemol_core::Uid,
        ty: String,
    },
    ChainId {
        target: cuemol_core::Uid,
    },
    ResidueNum {
        target: cuemol_core::Uid,
    },
    DeleteAtoms {
        target: cuemol_core::Uid,
    },
    GetPdb,
    /// Superpose `moving` onto `reference` (reference = the selected object).
    Superpose {
        reference: cuemol_core::Uid,
        moving: Option<cuemol_core::Uid>,
    },
    /// Atom-contact / interaction calculation on `target`.
    Contacts {
        target: cuemol_core::Uid,
        hbond: bool,
    },
    /// Reassign secondary structure on `target`.
    Secondary {
        target: cuemol_core::Uid,
        ignore_breaks: bool,
    },
    /// Molecular-surface OBJ generation for `target`.
    Surface {
        target: cuemol_core::Uid,
    },
    /// Morph-trajectory keyframe append / apply for `target`.
    Morph {
        target: cuemol_core::Uid,
        source: Option<cuemol_core::Uid>,
        frame: f64,
    },
    /// Configuration key/value editor.
    Config,
}

/// A point-in-time copy of everything Undo/Redo restores. `DesktopAppState` is
/// `Clone`, so a snapshot is just a deep copy — no command pattern needed (the
/// same approach the egui front end uses).
#[derive(Clone)]
struct UndoSnapshot {
    state: DesktopAppState,
    selected_object: Option<cuemol_core::Uid>,
    selected_renderer: Option<cuemol_core::Uid>,
    applied_selection: String,
}

/// Lazily-created text fields for whichever modal is open. Each is `Some` only
/// while its dialog is on screen.
#[derive(Default)]
struct DialogInputs {
    renderer_name: Option<Entity<TextInput>>,
    chain_id: Option<Entity<TextInput>>,
    residue_start: Option<Entity<TextInput>>,
    delete_sel: Option<Entity<TextInput>>,
    pdb_id: Option<Entity<TextInput>>,
    // Tool-dialog fields.
    ref_sel: Option<Entity<TextInput>>,
    mov_sel: Option<Entity<TextInput>>,
    contact_min: Option<Entity<TextInput>>,
    contact_max: Option<Entity<TextInput>>,
    secondary_angle: Option<Entity<TextInput>>,
    surf_density: Option<Entity<TextInput>>,
    surf_probe: Option<Entity<TextInput>>,
    surf_path: Option<Entity<TextInput>>,
    morph_name: Option<Entity<TextInput>>,
    config_key: Option<Entity<TextInput>>,
    config_value: Option<Entity<TextInput>>,
    config_path: Option<Entity<TextInput>>,
}

/// Root gpui view: owns the application state and renders the whole window.
pub struct CueMolGpuiApp {
    state: DesktopAppState,
    /// A short status line shown in the header (last action / loaded file).
    status: String,
    /// The wgpu-rendered 3D molecular view, composited into the gpui viewport as
    /// an image (None if there was no GPU adapter / empty scene).
    viewport_image: Option<Arc<RenderImage>>,
    /// The current view camera (auto-framed at load; mutated by drag/scroll).
    camera: cuemol_qsys::Camera,
    /// Persistent wgpu renderer (device + pipeline reused across frames). `None`
    /// when no GPU adapter is available, in which case the viewport shows the
    /// placeholder. Built once so a drag doesn't recreate the device per frame.
    renderer: Option<SceneRenderer>,
    /// The active scene tessellated once into GPU geometry; reused for every
    /// camera move (rotate/zoom/pan don't change geometry, only the MVP).
    mesh: Option<cuemol_render::RenderMesh>,
    /// Offscreen render resolution = the on-screen 3D viewport's pixel size; kept
    /// in sync with the window so resizing re-renders crisply (not stretched).
    view_size: (u32, u32),
    /// Active drag gesture: which button started it + the last pointer position.
    /// Left orbits, right pans; `None` when no button is held.
    drag: Option<(gpui::MouseButton, gpui::Point<gpui::Pixels>)>,
    /// Window position where the left button was pressed (to tell a click — which
    /// picks an atom — from a drag, which orbits). `None` when not pressed.
    press_start: Option<gpui::Point<gpui::Pixels>>,
    /// Display DPI scale, captured from the window so the pick math can convert
    /// logical mouse coordinates to the offscreen image's physical pixels.
    scale_factor: f32,
    /// Persistent text-input entities; `None` until [`Self::ensure_inputs`] runs
    /// (headless tests never create them — see [`Inputs`]).
    inputs: Option<Inputs>,
    /// The selection expression the current `mesh` was tessellated with. `render`
    /// compares it against the live selection input and re-tessellates on change,
    /// translating egui's per-frame rebuild into gpui's retained-mode model.
    applied_selection: String,
    /// Tree selection: the highlighted object and renderer (drive the inspector).
    selected_object: Option<cuemol_core::Uid>,
    selected_renderer: Option<cuemol_core::Uid>,
    /// View settings.
    background: Background,
    center_mark: CenterMarkKind,
    /// Bottom-panel state.
    bottom_tab: BottomTab,
    show_inspector: bool,
    show_bottom: bool,
    /// Symmetry panel toggles (CueMol2 "Unit cell" / "Symm mol").
    show_unit_cell: bool,
    show_symm_mates: bool,
    /// Density-map chickenwire contour toggle + its sigma level.
    show_density_contour: bool,
    density_level: f64,
    /// Filled density isosurface (solid mode) + a cached marching-cubes mesh.
    show_density_solid: bool,
    density_solid_mesh: Option<cuemol_render::RenderMesh>,
    /// Molecular-surface (SES) display + a cached mesh (the SES is expensive).
    show_surface: bool,
    surface_mesh: Option<cuemol_render::RenderMesh>,
    /// Console / status log (newest last), capped to avoid unbounded growth.
    log_lines: Vec<String>,
    /// Most-recently-opened file paths (newest first, capped at 10).
    recent_paths: Vec<PathBuf>,
    /// Undo / redo stacks (snapshots of the whole app state).
    undo_stack: Vec<UndoSnapshot>,
    redo_stack: Vec<UndoSnapshot>,
    /// Currently open modal dialog + its lazily-created text fields.
    dialog: Dialog,
    dialog_inputs: DialogInputs,
    /// Which top-level menu's dropdown is open (`None` = closed). Rendered as a
    /// root-level overlay so it paints above the panels (z-order).
    open_menu: Option<&'static str>,
    /// Animation tab: keyframe count + play toggle (kept light, mirroring egui).
    animation_frames: usize,
    animation_playing: bool,
    /// Atom / bond counts of the last tessellated snapshot (for the status bar and
    /// the inspector's "selected atoms" readout). Cached so we don't recompute a
    /// snapshot just to display a number.
    rendered_atom_count: usize,
    rendered_bond_count: usize,
}

impl CueMolGpuiApp {
    /// Build the root view, loading any molecule paths given on the command line
    /// into a fresh scene so the tree has content to show.
    pub fn new(open_paths: Vec<PathBuf>) -> Self {
        let mut state = DesktopAppState::new();
        state.ensure_active_scene();
        let mut status = String::from("Ready");
        let mut loaded = 0usize;
        for path in &open_paths {
            match state.open_path(path) {
                Ok(_) => loaded += 1,
                Err(error) => status = format!("Load error: {error}"),
            }
        }
        if loaded > 0 {
            status = format!("Loaded {loaded} file(s)");
        }
        // Auto-frame the camera and tessellate the scene ONCE; both are reused for
        // every interactive re-render (only the MVP changes as the camera moves).
        let snapshot = state.active_render_snapshot("").ok();
        // Restore a loaded .qsc's saved view; auto-frame a plain molecule.
        let camera = state
            .active_scene_saved_camera()
            .or_else(|| snapshot.as_ref().map(frame_camera))
            .unwrap_or_default();
        let mesh = snapshot.as_ref().map(cuemol_render::tessellate_snapshot);
        let mut app = Self {
            state,
            status,
            viewport_image: None,
            camera,
            renderer: SceneRenderer::new(),
            mesh,
            view_size: (DEFAULT_VIEW_W, DEFAULT_VIEW_H),
            drag: None,
            press_start: None,
            scale_factor: 1.0,
            inputs: None,
            applied_selection: String::new(),
            selected_object: None,
            selected_renderer: None,
            background: Background::Slate,
            center_mark: CenterMarkKind::Cross,
            bottom_tab: BottomTab::Output,
            // CueMol2 has no right dock; properties open on demand via the toolbar toggle.
            show_inspector: false,
            show_bottom: true,
            show_unit_cell: false,
            show_symm_mates: false,
            show_density_contour: false,
            density_level: 1.5,
            show_density_solid: false,
            density_solid_mesh: None,
            show_surface: false,
            surface_mesh: None,
            log_lines: Vec::new(),
            recent_paths: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dialog: Dialog::None,
            dialog_inputs: DialogInputs::default(),
            open_menu: None,
            animation_frames: 0,
            animation_playing: false,
            rendered_atom_count: snapshot.as_ref().map(|s| s.atoms.len()).unwrap_or(0),
            rendered_bond_count: snapshot.as_ref().map(|s| s.bonds.len()).unwrap_or(0),
        };
        app.log_lines.push(app.status.clone());
        app.rerender();
        app
    }

    /// Create the persistent text-input entities on first use. Idempotent and
    /// cheap to call every `render`. Separated from `new` so the headless tests
    /// (no gpui `App`) can still construct the app without any entities.
    fn ensure_inputs(&mut self, cx: &mut Context<Self>) {
        if self.inputs.is_none() {
            let initial = self.applied_selection.clone();
            let inputs = Inputs {
                selection: cx.new(|cx| TextInput::new(cx, "name CA", &initial)),
                open_path: cx.new(|cx| {
                    TextInput::new(cx, "path/to/file.pdb", "../cuemol2/tests/test_data/1CRN.pdb")
                }),
                save_qsc: cx.new(|cx| TextInput::new(cx, "scene.qsc", "scene.qsc")),
                save_obj: cx.new(|cx| TextInput::new(cx, "object.pdb", "object.pdb")),
                export_pov: cx.new(|cx| TextInput::new(cx, "scene.pov", "scene.pov")),
            };
            self.inputs = Some(inputs);
        }
    }

    /// Rebuild the render snapshot + GPU mesh for the active scene at the current
    /// applied selection, optionally re-framing the camera (after a load/delete).
    /// This is the retained-mode equivalent of egui rebuilding its snapshot every
    /// frame: call it after any scene mutation or selection change.
    /// The render snapshot for the applied selection, with the Symmetry panel's crystal
    /// geometry (unit-cell box / mates) appended via the shared core. Used by the render
    /// path — NOT by picking, which hit-tests the bare molecule.
    fn current_render_snapshot(&self) -> Option<cuemol_render::RenderSceneSnapshot> {
        let mut snapshot = self
            .state
            .active_render_snapshot(&self.applied_selection)
            .ok()?;
        self.state.append_crystal_geometry(
            &mut snapshot,
            self.show_unit_cell,
            self.show_symm_mates,
        );
        self.state.append_density_geometry(
            &mut snapshot,
            self.density_level,
            self.show_density_contour,
        );
        if self.show_density_solid {
            if let Some(mesh) = &self.density_solid_mesh {
                snapshot.meshes.push(mesh.clone());
            }
        }
        if self.show_surface {
            if let Some(mesh) = &self.surface_mesh {
                snapshot.meshes.push(mesh.clone());
            }
        }
        Some(snapshot)
    }

    fn refresh_scene(&mut self, reframe: bool) {
        let snapshot = self.current_render_snapshot();
        if reframe {
            if let Some(snapshot) = &snapshot {
                self.camera = frame_camera(snapshot);
            }
        }
        self.rendered_atom_count = snapshot.as_ref().map(|s| s.atoms.len()).unwrap_or(0);
        self.rendered_bond_count = snapshot.as_ref().map(|s| s.bonds.len()).unwrap_or(0);
        self.mesh = snapshot.as_ref().map(cuemol_render::tessellate_snapshot);
        self.rerender();
    }

    /// Apply an object-level coloring scheme to every molecule (CueMol2 Coloring
    /// panel), then re-tessellate. Shares the core `set_object_coloring` path with the
    /// egui/dioxus front ends, so the snapshot recolours each carbon ($molcol cascade).
    fn apply_coloring(&mut self, coloring_type: Option<&str>, attrs: BTreeMap<String, String>) {
        let uids: Vec<_> = self
            .state
            .active_object_summaries()
            .iter()
            .map(|object| object.uid)
            .collect();
        for uid in uids {
            let _ = self
                .state
                .set_object_coloring(uid, coloring_type.map(String::from), attrs.clone());
        }
        self.refresh_scene(false);
    }

    fn color_element(&mut self) {
        self.apply_coloring(None, BTreeMap::new());
    }

    fn color_rainbow(&mut self) {
        let mut attrs = BTreeMap::new();
        attrs.insert("mode".into(), "mol".into());
        attrs.insert("start_hue".into(), "240".into());
        attrs.insert("end_hue".into(), "0".into());
        attrs.insert("sat".into(), "1".into());
        attrs.insert("bri".into(), "1".into());
        self.apply_coloring(Some("RainbowColoring"), attrs);
    }

    fn color_bfactor(&mut self) {
        self.apply_coloring(Some("BfacColoring"), BTreeMap::new());
    }

    /// Symmetry panel toggles (CueMol2 "Unit cell" / "Symm mol"): re-tessellate so the
    /// shared crystal geometry appears/disappears.
    fn toggle_unit_cell(&mut self) {
        self.show_unit_cell = !self.show_unit_cell;
        self.refresh_scene(false);
    }

    fn toggle_symm_mates(&mut self) {
        self.show_symm_mates = !self.show_symm_mates;
        self.refresh_scene(false);
    }

    /// Density map "chickenwire" contour toggle (CueMol2 Density map panel).
    fn toggle_density_contour(&mut self) {
        self.show_density_contour = !self.show_density_contour;
        self.refresh_scene(false);
    }

    /// Filled density isosurface (solid mode) toggle. Marches the map once on enable.
    fn toggle_density_solid(&mut self) {
        self.show_density_solid = !self.show_density_solid;
        if self.show_density_solid && self.density_solid_mesh.is_none() {
            self.density_solid_mesh = self.state.build_density_surface_mesh(self.density_level);
            if self.density_solid_mesh.is_none() {
                self.show_density_solid = false;
                self.set_status("No density map for a solid surface");
            }
        }
        self.refresh_scene(false);
    }

    /// Molecular-surface (SES) display toggle. Computes the SES mesh once on enable
    /// (it is expensive) via the shared `build_surface_mesh` and caches it.
    fn toggle_surface(&mut self) {
        self.show_surface = !self.show_surface;
        if self.show_surface && self.surface_mesh.is_none() {
            if let Some(uid) = self
                .state
                .active_object_summaries()
                .first()
                .map(|object| object.uid)
            {
                match self
                    .state
                    .build_surface_mesh(uid, &self.applied_selection, 1.0, 1.4)
                {
                    Ok(mesh) => self.surface_mesh = Some(mesh),
                    Err(error) => {
                        self.show_surface = false;
                        self.set_status(format!("Surface failed: {error}"));
                    }
                }
            }
        }
        self.refresh_scene(false);
    }

    /// Apply a selection expression (CueMol2 Selection quick-selects), keeping the
    /// selection input box in sync, then re-tessellate so the highlight updates.
    fn set_selection(&mut self, expr: &str, cx: &mut Context<Self>) {
        let expr = expr.to_string();
        self.applied_selection = expr.clone();
        if let Some(input) = self.inputs.as_ref().map(|inputs| inputs.selection.clone()) {
            input.update(cx, move |input, cx| input.set_text(expr, cx));
        }
        self.refresh_scene(false);
    }

    /// Pull the live selection text from the input entity; if it differs from the
    /// applied one, adopt it and re-tessellate (no camera re-frame — selection only
    /// changes highlighting/visibility, not the molecule's extent).
    fn sync_selection(&mut self, cx: &mut Context<Self>) {
        let live = self
            .inputs
            .as_ref()
            .map(|inputs| inputs.selection.read(cx).text());
        if let Some(live) = live {
            if live != self.applied_selection {
                self.applied_selection = live;
                self.refresh_scene(false);
            }
        }
    }

    // ---- status / logging ------------------------------------------------

    /// Set the one-line status and append it to the console log (capped).
    fn set_status(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        self.log_lines.push(msg.clone());
        if self.log_lines.len() > 200 {
            let overflow = self.log_lines.len() - 200;
            self.log_lines.drain(0..overflow);
        }
        self.status = msg;
    }

    // ---- undo / redo -----------------------------------------------------

    /// Capture the current state for Undo and clear the redo stack. Call this
    /// immediately BEFORE any scene-mutating action (the egui front end's policy).
    fn snapshot_undo(&mut self) {
        self.undo_stack.push(UndoSnapshot {
            state: self.state.clone(),
            selected_object: self.selected_object,
            selected_renderer: self.selected_renderer,
            applied_selection: self.applied_selection.clone(),
        });
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn restore(&mut self, snap: UndoSnapshot) {
        self.state = snap.state;
        self.selected_object = snap.selected_object;
        self.selected_renderer = snap.selected_renderer;
        self.applied_selection = snap.applied_selection;
        self.refresh_scene(true);
    }

    fn undo(&mut self) {
        let Some(snap) = self.undo_stack.pop() else {
            self.set_status("Nothing to undo");
            return;
        };
        self.redo_stack.push(UndoSnapshot {
            state: self.state.clone(),
            selected_object: self.selected_object,
            selected_renderer: self.selected_renderer,
            applied_selection: self.applied_selection.clone(),
        });
        self.restore(snap);
        self.set_status("Undo");
    }

    fn redo(&mut self) {
        let Some(snap) = self.redo_stack.pop() else {
            self.set_status("Nothing to redo");
            return;
        };
        self.undo_stack.push(UndoSnapshot {
            state: self.state.clone(),
            selected_object: self.selected_object,
            selected_renderer: self.selected_renderer,
            applied_selection: self.applied_selection.clone(),
        });
        self.restore(snap);
        self.set_status("Redo");
    }

    fn add_recent(&mut self, path: PathBuf) {
        self.recent_paths.retain(|p| p != &path);
        self.recent_paths.insert(0, path);
        self.recent_paths.truncate(10);
    }

    // ---- file actions ----------------------------------------------------

    /// Open a molecule / scene / map file by path (toolbar "Open" + recent files).
    fn open_file(&mut self, path_str: &str) {
        let trimmed = path_str.trim();
        if trimmed.is_empty() {
            self.set_status("Open: empty path");
            return;
        }
        let path = PathBuf::from(trimmed);
        self.snapshot_undo();
        match self.state.open_path(&path) {
            Ok(loaded) => {
                match loaded {
                    GuiLoadedPath::Object {
                        scene_uid,
                        object_uid,
                    } => {
                        self.state.set_active_scene(Some(scene_uid));
                        self.selected_object = Some(object_uid);
                        self.selected_renderer = None;
                    }
                    GuiLoadedPath::Scene { scene_uid }
                    | GuiLoadedPath::Map { scene_uid, .. } => {
                        self.state.set_active_scene(Some(scene_uid));
                    }
                }
                self.add_recent(path.clone());
                self.refresh_scene(true);
                self.set_status(format!("Loaded {}", path.display()));
            }
            Err(error) => {
                self.undo_stack.pop(); // nothing changed; discard the snapshot
                self.set_status(format!("Load error: {error}"));
            }
        }
    }

    fn save_scene_qsc(&mut self, path_str: &str) {
        let path = path_str.trim();
        match self.state.save_active_scene_qsc(path) {
            Ok(()) => self.set_status(format!("Saved scene to {path}")),
            Err(error) => self.set_status(format!("Save scene error: {error}")),
        }
    }

    fn save_object(&mut self, path_str: &str) {
        let Some(object_uid) = self.selected_object else {
            self.set_status("Save object: no object selected");
            return;
        };
        let path = path_str.trim();
        match self.state.save_object_as(object_uid, path) {
            Ok(()) => self.set_status(format!("Saved object to {path}")),
            Err(error) => self.set_status(format!("Save object error: {error}")),
        }
    }

    fn export_pov(&mut self, path_str: &str) {
        let path = path_str.trim();
        match self.state.export_active_scene_pov(path, &self.applied_selection) {
            Ok(count) => self.set_status(format!("Exported {count} atoms to {path}")),
            Err(error) => self.set_status(format!("Export error: {error}")),
        }
    }

    // ---- renderer actions ------------------------------------------------

    fn create_renderer(&mut self, object_uid: cuemol_core::Uid, ty: &str, name: &str) {
        self.snapshot_undo();
        let name = if name.trim().is_empty() { ty } else { name.trim() };
        match self
            .state
            .new_renderer_in_active_scene(object_uid, ty, name)
        {
            Ok(result) => {
                self.selected_object = Some(object_uid);
                self.selected_renderer = Some(result.result_renderer);
                self.refresh_scene(false);
                self.set_status(format!("Added {ty} renderer"));
            }
            Err(error) => {
                self.undo_stack.pop();
                self.set_status(format!("Add renderer error: {error}"));
            }
        }
    }

    fn toggle_renderer_visible(
        &mut self,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
    ) {
        let visible = self
            .renderers(object_uid)
            .into_iter()
            .find(|r| r.uid == renderer_uid)
            .map(|r| r.visible)
            .unwrap_or(true);
        self.snapshot_undo();
        match self
            .state
            .set_renderer_visible(object_uid, renderer_uid, !visible)
        {
            Ok(()) => {
                self.refresh_scene(false);
                self.set_status(if visible { "Renderer hidden" } else { "Renderer shown" });
            }
            Err(error) => {
                self.undo_stack.pop();
                self.set_status(format!("Visibility error: {error}"));
            }
        }
    }

    fn delete_renderer(&mut self, object_uid: cuemol_core::Uid, renderer_uid: cuemol_core::Uid) {
        self.snapshot_undo();
        match self
            .state
            .destroy_renderer_in_object(object_uid, renderer_uid)
        {
            Ok(true) => {
                if self.selected_renderer == Some(renderer_uid) {
                    self.selected_renderer = None;
                }
                self.refresh_scene(false);
                self.set_status("Deleted renderer");
            }
            Ok(false) => {
                self.undo_stack.pop();
                self.set_status("Renderer not found");
            }
            Err(error) => {
                self.undo_stack.pop();
                self.set_status(format!("Delete error: {error}"));
            }
        }
    }

    // ---- atom-editing actions --------------------------------------------

    fn change_chain(&mut self, object_uid: cuemol_core::Uid, selection: &str, chain: &str) {
        self.snapshot_undo();
        match self
            .state
            .change_chain_in_object(object_uid, selection, chain)
        {
            Ok(count) => {
                self.refresh_scene(false);
                self.set_status(format!("Changed chain on {count} atoms"));
            }
            Err(error) => {
                self.undo_stack.pop();
                self.set_status(format!("Chain change error: {error}"));
            }
        }
    }

    fn renumber_residues(&mut self, object_uid: cuemol_core::Uid, selection: &str, start: i32) {
        self.snapshot_undo();
        match self
            .state
            .renumber_residues_in_object(object_uid, selection, start)
        {
            Ok(count) => {
                self.refresh_scene(false);
                self.set_status(format!("Renumbered {count} residues"));
            }
            Err(error) => {
                self.undo_stack.pop();
                self.set_status(format!("Renumber error: {error}"));
            }
        }
    }

    fn delete_atoms(&mut self, object_uid: cuemol_core::Uid, selection: &str) {
        self.snapshot_undo();
        match self.state.delete_atoms_in_object(object_uid, selection) {
            Ok(count) => {
                self.refresh_scene(true);
                self.set_status(format!("Deleted {count} atoms"));
            }
            Err(error) => {
                self.undo_stack.pop();
                self.set_status(format!("Delete atoms error: {error}"));
            }
        }
    }

    // ---- view actions ----------------------------------------------------

    /// Re-frame the camera to the current scene bounds, preserving orientation.
    fn fit_view(&mut self) {
        if let Ok(snapshot) = self.state.active_render_snapshot(&self.applied_selection) {
            let rotation = self.camera.rotation();
            self.camera = frame_camera(&snapshot);
            self.camera.set_rotation(rotation);
            self.rerender();
            self.set_status("Fit view");
        }
    }

    fn set_background(&mut self, background: Background) {
        self.background = background;
        self.rerender();
    }

    fn toggle_projection(&mut self) {
        let perspective = !self.camera.is_perspective();
        self.camera.set_perspective(perspective);
        self.rerender();
        self.set_status(if perspective {
            "Perspective projection"
        } else {
            "Orthographic projection"
        });
    }

    /// Cycle the center-mark overlay None -> Cross -> Axis -> None (View menu).
    fn cycle_center_mark(&mut self) {
        self.center_mark = match self.center_mark {
            CenterMarkKind::None => CenterMarkKind::Cross,
            CenterMarkKind::Cross => CenterMarkKind::Axis,
            CenterMarkKind::Axis => CenterMarkKind::None,
        };
        self.set_status(match self.center_mark {
            CenterMarkKind::None => "Center mark: none",
            CenterMarkKind::Cross => "Center mark: cross",
            CenterMarkKind::Axis => "Center mark: axis",
        });
    }

    // ---- analysis / tool actions -----------------------------------------

    fn run_superpose(
        &mut self,
        reference: cuemol_core::Uid,
        reference_sel: &str,
        moving: cuemol_core::Uid,
        moving_sel: &str,
    ) {
        self.snapshot_undo();
        match self
            .state
            .superpose_objects(reference, reference_sel, moving, moving_sel)
        {
            Ok(_script) => {
                self.refresh_scene(true);
                self.set_status("Superposed object");
            }
            Err(error) => {
                self.undo_stack.pop();
                self.set_status(format!("Superpose error: {error}"));
            }
        }
    }

    fn run_contacts(
        &mut self,
        target: cuemol_core::Uid,
        selection: &str,
        r_min: f64,
        r_max: f64,
        hbond: bool,
    ) {
        match self
            .state
            .calc_atom_contacts_in_object(target, selection, r_min, r_max, hbond, 200)
        {
            Ok(json) => {
                self.log_lines.push(json);
                self.set_status("Contacts computed (see Output tab)");
            }
            Err(error) => self.set_status(format!("Contacts error: {error}")),
        }
    }

    fn run_secondary(&mut self, target: cuemol_core::Uid, ignore_breaks: bool, angle: f64) {
        self.snapshot_undo();
        match self
            .state
            .assign_secondary_structure_in_object(target, ignore_breaks, angle)
        {
            Ok(count) => {
                self.refresh_scene(false);
                self.set_status(format!("Reassigned secondary structure ({count} atoms)"));
            }
            Err(error) => {
                self.undo_stack.pop();
                self.set_status(format!("Secondary structure error: {error}"));
            }
        }
    }

    fn run_surface(
        &mut self,
        target: cuemol_core::Uid,
        selection: &str,
        density: f64,
        probe: f64,
        path: &str,
    ) {
        match self
            .state
            .write_surface_obj(target, selection, density, probe, path)
        {
            Ok(summary) => self.set_status(format!(
                "Surface: {} verts / {} tris -> {path}",
                summary.vertex_count, summary.triangle_count
            )),
            Err(error) => self.set_status(format!("Surface error: {error}")),
        }
    }

    fn run_morph_append(
        &mut self,
        target: cuemol_core::Uid,
        source: cuemol_core::Uid,
        name: &str,
    ) {
        self.snapshot_undo();
        match self
            .state
            .append_morph_frame_from_object(target, source, name)
        {
            Ok(summary) => {
                self.set_status(format!("Appended morph frame ({} frames)", summary.frame_count))
            }
            Err(error) => {
                self.undo_stack.pop();
                self.set_status(format!("Morph error: {error}"));
            }
        }
    }

    fn run_morph_apply(&mut self, target: cuemol_core::Uid, frame: f64) {
        match self.state.apply_morph_frame(target, frame) {
            Ok(_) => {
                self.refresh_scene(false);
                self.set_status(format!("Applied morph frame {frame:.2}"));
            }
            Err(error) => self.set_status(format!("Morph apply error: {error}")),
        }
    }

    fn config_set(&mut self, key: &str, value: &str) {
        self.state.put_config_value(key, value);
        self.set_status(format!("Set {key} = {value}"));
    }

    fn config_get(&mut self, key: &str) {
        let value = self.state.config_value(key);
        self.set_status(format!("{key} = {value}"));
    }

    fn config_load(&mut self, path: &str) {
        match self.state.load_config_file(path) {
            Ok(count) => self.set_status(format!("Loaded {count} config entries from {path}")),
            Err(error) => self.set_status(format!("Config load error: {error}")),
        }
    }

    fn config_save(&mut self, path: &str) {
        match self.state.save_config_file(path) {
            Ok(()) => self.set_status(format!("Saved config to {path}")),
            Err(error) => self.set_status(format!("Config save error: {error}")),
        }
    }

    // ---- tree selection --------------------------------------------------

    fn select_scene(&mut self, scene_uid: cuemol_core::Uid) {
        self.state.set_active_scene(Some(scene_uid));
        self.selected_object = None;
        self.selected_renderer = None;
        self.refresh_scene(true);
    }

    fn select_object(&mut self, object_uid: cuemol_core::Uid) {
        self.selected_object = Some(object_uid);
        self.selected_renderer = None;
    }

    fn select_renderer(&mut self, object_uid: cuemol_core::Uid, renderer_uid: cuemol_core::Uid) {
        self.selected_object = Some(object_uid);
        self.selected_renderer = Some(renderer_uid);
    }

    /// The summary for the currently selected object, if any.
    fn selected_object_summary(&self) -> Option<GuiObjectSummary> {
        let object_uid = self.selected_object?;
        let scene_uid = self.state.active_scene()?;
        self.objects(scene_uid)
            .into_iter()
            .find(|o| o.uid == object_uid)
    }

    /// The summary for the currently selected renderer, if any.
    fn selected_renderer_summary(&self) -> Option<GuiRendererSummary> {
        let object_uid = self.selected_object?;
        let renderer_uid = self.selected_renderer?;
        self.renderers(object_uid)
            .into_iter()
            .find(|r| r.uid == renderer_uid)
    }

    // ---- dialog open / close ---------------------------------------------

    fn close_dialog(&mut self) {
        self.dialog = Dialog::None;
        self.dialog_inputs = DialogInputs::default();
    }

    fn open_new_renderer_dialog(&mut self, target: cuemol_core::Uid, cx: &mut Context<Self>) {
        self.dialog = Dialog::NewRenderer {
            target,
            ty: "simple".to_string(),
        };
        self.dialog_inputs.renderer_name = Some(cx.new(|cx| TextInput::new(cx, "name", "simple")));
    }

    fn open_chain_dialog(&mut self, target: cuemol_core::Uid, cx: &mut Context<Self>) {
        self.dialog = Dialog::ChainId { target };
        self.dialog_inputs.chain_id = Some(cx.new(|cx| TextInput::new(cx, "A", "A")));
    }

    fn open_residue_dialog(&mut self, target: cuemol_core::Uid, cx: &mut Context<Self>) {
        self.dialog = Dialog::ResidueNum { target };
        self.dialog_inputs.residue_start = Some(cx.new(|cx| TextInput::new(cx, "1", "1")));
    }

    fn open_delete_atoms_dialog(&mut self, target: cuemol_core::Uid, cx: &mut Context<Self>) {
        self.dialog = Dialog::DeleteAtoms { target };
        let initial = self.applied_selection.clone();
        self.dialog_inputs.delete_sel = Some(cx.new(|cx| TextInput::new(cx, "hetero", &initial)));
    }

    fn open_get_pdb_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = Dialog::GetPdb;
        self.dialog_inputs.pdb_id = Some(cx.new(|cx| TextInput::new(cx, "1CRN", "1CRN")));
    }

    fn open_superpose_dialog(&mut self, reference: cuemol_core::Uid, cx: &mut Context<Self>) {
        self.dialog = Dialog::Superpose {
            reference,
            moving: None,
        };
        self.dialog_inputs.ref_sel = Some(cx.new(|cx| TextInput::new(cx, "name CA", "name CA")));
        self.dialog_inputs.mov_sel = Some(cx.new(|cx| TextInput::new(cx, "name CA", "name CA")));
    }

    fn open_contacts_dialog(&mut self, target: cuemol_core::Uid, cx: &mut Context<Self>) {
        self.dialog = Dialog::Contacts {
            target,
            hbond: false,
        };
        self.dialog_inputs.contact_min = Some(cx.new(|cx| TextInput::new(cx, "2.5", "2.5")));
        self.dialog_inputs.contact_max = Some(cx.new(|cx| TextInput::new(cx, "4.0", "4.0")));
    }

    fn open_secondary_dialog(&mut self, target: cuemol_core::Uid, cx: &mut Context<Self>) {
        self.dialog = Dialog::Secondary {
            target,
            ignore_breaks: false,
        };
        self.dialog_inputs.secondary_angle = Some(cx.new(|cx| TextInput::new(cx, "90.0", "90.0")));
    }

    fn open_surface_dialog(&mut self, target: cuemol_core::Uid, cx: &mut Context<Self>) {
        self.dialog = Dialog::Surface { target };
        self.dialog_inputs.surf_density = Some(cx.new(|cx| TextInput::new(cx, "5.0", "5.0")));
        self.dialog_inputs.surf_probe = Some(cx.new(|cx| TextInput::new(cx, "1.4", "1.4")));
        self.dialog_inputs.surf_path =
            Some(cx.new(|cx| TextInput::new(cx, "surface.obj", "surface.obj")));
    }

    fn open_morph_dialog(&mut self, target: cuemol_core::Uid, cx: &mut Context<Self>) {
        self.dialog = Dialog::Morph {
            target,
            source: None,
            frame: 0.0,
        };
        self.dialog_inputs.morph_name = Some(cx.new(|cx| TextInput::new(cx, "frame", "frame1")));
    }

    fn open_config_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = Dialog::Config;
        self.dialog_inputs.config_key = Some(cx.new(|cx| TextInput::new(cx, "key", "")));
        self.dialog_inputs.config_value = Some(cx.new(|cx| TextInput::new(cx, "value", "")));
        self.dialog_inputs.config_path =
            Some(cx.new(|cx| TextInput::new(cx, "config.txt", "config.txt")));
    }

    /// Orbit the camera (left-drag): `dx`/`dy` are degrees about the Y/X axes.
    pub fn rotate_camera(&mut self, dx_deg: f64, dy_deg: f64) {
        if let Ok(rotation) = self
            .camera
            .rotation()
            .rotate_y(dx_deg)
            .and_then(|q| q.rotate_x(dy_deg))
        {
            self.camera.set_rotation(rotation);
        }
        self.rerender();
    }

    /// Zoom the camera (scroll): `factor` > 1 zooms out, < 1 zooms in.
    pub fn zoom_camera(&mut self, factor: f64) {
        let zoom = (self.camera.zoom() * factor).clamp(0.5, 1.0e6);
        self.camera.set_zoom(zoom);
        self.rerender();
    }

    /// Pan the camera (right-drag): translate the look-at centre so the molecule
    /// tracks the cursor. `dx`/`dy` are pixel deltas; `vp_h` is the viewport
    /// height in pixels (sets the world-units-per-pixel scale).
    pub fn pan_camera(&mut self, dx_pix: f64, dy_pix: f64, vp_h: u32) {
        self.camera
            .set_center(pan_center(&self.camera, dx_pix, dy_pix, vp_h));
        self.rerender();
    }

    /// Re-render the offscreen 3D image from the persistent renderer + cached
    /// mesh at the current camera and viewport size. Cheap relative to the old
    /// path: no device creation, no re-tessellation — just an MVP + a draw.
    fn rerender(&mut self) {
        self.viewport_image = self.render_image();
    }

    fn render_image(&self) -> Option<Arc<RenderImage>> {
        let (w, h) = self.view_size;
        let rgba = self.render_rgba()?;
        let buffer = image::RgbaImage::from_raw(w, h, rgba)?;
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
    }

    /// Render the current camera view to a raw RGBA8 buffer (the bytes that back
    /// the composited image). Returns `None` for an empty scene or no GPU adapter.
    /// Split out so the pan/zoom pixel path is testable without gpui image types.
    fn render_rgba(&self) -> Option<Vec<u8>> {
        let renderer = self.renderer.as_ref()?;
        let mesh = self.mesh.as_ref()?;
        if mesh.indices.is_empty() {
            return None; // empty scene -> placeholder
        }
        let (w, h) = self.view_size;
        let mvp = crate::offscreen::camera_mvp(&self.camera, w as f64 / h as f64);
        renderer.render(std::slice::from_ref(mesh), mvp, w, h, self.background.clear())
    }

    /// Update the offscreen resolution to match the live window and re-render if
    /// it changed (resize-follow). Gated on a real change so it converges.
    fn sync_view_size(&mut self, window: &Window) {
        // The on-screen 3D region is the window minus the live chrome: sidebar
        // (+ inspector, when shown) horizontally; header + toolbar + status
        // (+ bottom panel, when shown) vertically. Keeping this in step with the
        // panel-visibility flags means the offscreen render matches the visible
        // region exactly instead of stretching.
        let chrome_w = SIDEBAR_W + if self.show_inspector { INSPECTOR_W } else { 0.0 };
        let chrome_h =
            HEADER_H + TOOLBAR_H + STATUS_H + if self.show_bottom { BOTTOM_H } else { 0.0 };
        self.scale_factor = window.scale_factor();
        let size = viewport_pixels(window, chrome_w, chrome_h);
        if size != self.view_size {
            self.view_size = size;
            self.rerender();
        }
    }

    /// Pick the atom nearest a click in the 3D viewport and select it. `window_pos`
    /// is the click position in window-logical coordinates. The offscreen image is
    /// rendered at exactly the viewport's pixel size (no letterbox), so mapping
    /// window -> viewport -> image pixels is a simple offset + DPI scale.
    fn try_pick(&mut self, window_pos: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        let local_x = f32::from(window_pos.x) - SIDEBAR_W;
        let local_y = f32::from(window_pos.y) - HEADER_H - TOOLBAR_H;
        if local_x < 0.0 || local_y < 0.0 {
            return; // click landed on the chrome, not the viewport
        }
        let (vp_w, vp_h) = self.view_size;
        let cursor_px = (
            (local_x * self.scale_factor) as f64,
            (local_y * self.scale_factor) as f64,
        );
        if cursor_px.0 > vp_w as f64 || cursor_px.1 > vp_h as f64 {
            return;
        }
        let Ok(snapshot) = self.state.active_render_snapshot(&self.applied_selection) else {
            return;
        };
        let mvp = crate::offscreen::camera_mvp(&self.camera, vp_w as f64 / vp_h as f64);
        let Some((object_u64, atom_index)) = pick_atom(
            &snapshot.atoms,
            mvp,
            cursor_px,
            (vp_w as f64, vp_h as f64),
            12.0,
        ) else {
            return;
        };
        let object_uid = cuemol_core::Uid::new(object_u64);
        let selection = self
            .state
            .atom_summary(object_uid, atom_index)
            .map(|atom| atom.selection_text)
            .unwrap_or_else(|_| format!("aid {atom_index}"));
        self.select_object(object_uid);
        if let Some(inputs) = &self.inputs {
            let input = inputs.selection.clone();
            let sel = selection.clone();
            input.update(cx, |ti, cx| ti.set_text(sel, cx));
        }
        self.applied_selection = selection.clone();
        self.refresh_scene(false);
        self.set_status(format!("Picked atom: {selection}"));
    }

    fn scenes(&self) -> Vec<GuiSceneSummary> {
        self.state.scene_summaries()
    }

    fn objects(&self, scene_uid: cuemol_core::Uid) -> Vec<GuiObjectSummary> {
        self.state.object_summaries(scene_uid)
    }

    fn renderers(&self, object_uid: cuemol_core::Uid) -> Vec<GuiRendererSummary> {
        self.state.renderer_summaries(object_uid)
    }
}

impl Render for CueMolGpuiApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_inputs(cx);
        // Adopt any edited selection text and re-tessellate before laying out.
        self.sync_selection(cx);
        // Keep the offscreen 3D resolution matched to the live window size so a
        // resize re-renders crisply instead of stretching the old image.
        self.sync_view_size(window);

        // Pre-evaluate each panel into an owned element so the assembling `div`
        // expression below holds no `self`/`cx` borrows (see module note): every
        // `self.panel(cx)` call borrows independently and finishes before the next.
        let header = self.header(cx).into_any_element();
        let toolbar = self.toolbar(cx).into_any_element();
        let sidebar = self.sidebar(cx).into_any_element();
        let viewport = self.viewport(cx).into_any_element();
        let bottom = self
            .show_bottom
            .then(|| self.bottom_panel(cx).into_any_element());
        let inspector = self
            .show_inspector
            .then(|| self.inspector(cx).into_any_element());
        let status = self.status_bar().into_any_element();
        let menu = self
            .open_menu
            .map(|_| self.menu_overlay(cx).into_any_element());
        let dialog = (self.dialog != Dialog::None)
            .then(|| self.dialog_overlay(cx).into_any_element());

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(COL_BG))
            .text_color(rgb(COL_TEXT))
            .text_sm()
            .child(header)
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(sidebar)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(viewport)
                            .children(bottom),
                    )
                    .children(inspector),
            )
            .child(status)
            .children(menu)
            .children(dialog)
    }
}

impl CueMolGpuiApp {
    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self
            .state
            .active_scene_summary()
            .map(|s| s.name)
            .unwrap_or_else(|| "(no scene)".to_string());
        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(HEADER_H))
            .bg(rgb(COL_HEADER))
            .border_b_1()
            .border_color(rgb(COL_BORDER))
            .child(
                div()
                    .w(px(92.0))
                    .px_3()
                    .text_color(rgb(COL_ACCENT))
                    .child("CueMol2-rs"),
            );
        // Top-level menu titles; clicking toggles that menu's dropdown overlay.
        for (name, _left, width) in MENUS {
            let name = *name;
            let open = self.open_menu == Some(name);
            bar = bar.child(
                div()
                    .id(eid(format!("menu-{name}")))
                    .w(px(*width))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(rgb(if open { COL_BTN } else { COL_HEADER }))
                    .hover(|s| s.bg(rgb(COL_BTN)))
                    .child(name)
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        this.open_menu = if this.open_menu == Some(name) {
                            None
                        } else {
                            Some(name)
                        };
                        cx.notify();
                    })),
            );
        }
        bar.child(div().flex_1().px_3().text_color(rgb(COL_TEXT_DIM)).child(format!("scene: {active}")))
            // Right-aligned panel toggles (mirrors egui's Options/Window menu).
            .child(
                toggle_button("tgl-inspector", "Inspector", self.show_inspector).on_click(
                    cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                        this.show_inspector = !this.show_inspector;
                        cx.notify();
                    }),
                ),
            )
            .child(
                toggle_button("tgl-bottom", "Console", self.show_bottom)
                    .mr_2()
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                        this.show_bottom = !this.show_bottom;
                        cx.notify();
                    })),
            )
    }

    /// The open menu's dropdown, rendered as a root-level overlay (so it paints
    /// above the panels). A full-screen scrim behind it closes the menu on an
    /// outside click; each item runs its action and closes the menu.
    fn menu_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.open_menu.unwrap_or("");
        let left = MENUS
            .iter()
            .find(|(name, _, _)| *name == menu)
            .map(|(_, left, _)| *left)
            .unwrap_or(0.0);
        let items = self.menu_items(cx, menu);
        div()
            .absolute()
            .inset_0()
            // Scrim: an outside click closes the menu. It starts BELOW the menu bar
            // so the titles stay clickable — switching menus is a single click.
            .child(
                div()
                    .id("menu-scrim")
                    .absolute()
                    .top(px(HEADER_H))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                        this.open_menu = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .absolute()
                    .top(px(HEADER_H))
                    .left(px(left))
                    .flex()
                    .flex_col()
                    .py_1()
                    .min_w(px(200.0))
                    .rounded_md()
                    .bg(rgb(COL_PANEL))
                    .border_1()
                    .border_color(rgb(COL_BORDER))
                    .children(items),
            )
    }

    /// Build the dropdown rows for the named menu.
    fn menu_items(&self, cx: &mut Context<Self>, menu: &str) -> Vec<gpui::AnyElement> {
        let mut items: Vec<gpui::AnyElement> = Vec::new();
        match menu {
            "File" => {
                items.push(
                    menu_item("mi-open", "Open (toolbar path)")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            let path = this
                                .inputs
                                .as_ref()
                                .map(|i| i.open_path.read(cx).text())
                                .unwrap_or_default();
                            this.open_file(&path);
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-getpdb", "Get PDB…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.open_get_pdb_dialog(cx);
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-savescene", "Save scene")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            let path = this
                                .inputs
                                .as_ref()
                                .map(|i| i.save_qsc.read(cx).text())
                                .unwrap_or_default();
                            this.save_scene_qsc(&path);
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-saveobj", "Save object")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            let path = this
                                .inputs
                                .as_ref()
                                .map(|i| i.save_obj.read(cx).text())
                                .unwrap_or_default();
                            this.save_object(&path);
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-exportpov", "Export POV-Ray")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            let path = this
                                .inputs
                                .as_ref()
                                .map(|i| i.export_pov.read(cx).text())
                                .unwrap_or_default();
                            this.export_pov(&path);
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(menu_separator().into_any_element());
                if self.recent_paths.is_empty() {
                    items.push(
                        div()
                            .px_3()
                            .py_1()
                            .text_xs()
                            .text_color(rgb(COL_TEXT_DIM))
                            .child("(no recent files)")
                            .into_any_element(),
                    );
                } else {
                    for (idx, path) in self.recent_paths.iter().enumerate() {
                        let path_str = path.to_string_lossy().to_string();
                        let label = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path_str.clone());
                        items.push(
                            menu_item(eid(format!("mi-recent-{idx}")), label)
                                .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                                    let path = path_str.clone();
                                    this.open_file(&path);
                                    this.open_menu = None;
                                    cx.notify();
                                }))
                                .into_any_element(),
                        );
                    }
                }
            }
            "Edit" => {
                items.push(
                    menu_item("mi-undo", "Undo")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.undo();
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-redo", "Redo")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.redo();
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(menu_separator().into_any_element());
                items.push(
                    menu_item("mi-chain", "Change chain ID…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.with_selected_object(cx, |this, obj, cx| {
                                this.open_chain_dialog(obj, cx)
                            });
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-renum", "Renumber residues…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.with_selected_object(cx, |this, obj, cx| {
                                this.open_residue_dialog(obj, cx)
                            });
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-delatoms", "Delete atoms…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.with_selected_object(cx, |this, obj, cx| {
                                this.open_delete_atoms_dialog(obj, cx)
                            });
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
            "View" => {
                items.push(
                    menu_item("mi-fit", "Fit")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.fit_view();
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-proj", "Toggle projection")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.toggle_projection();
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-mark", "Cycle center mark")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.cycle_center_mark();
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(menu_separator().into_any_element());
                for (label, bg) in [
                    ("Background: Slate", Background::Slate),
                    ("Background: Black", Background::Black),
                    ("Background: White", Background::White),
                ] {
                    items.push(
                        menu_item(eid(format!("mi-bg-{label}")), label)
                            .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                                this.set_background(bg);
                                this.open_menu = None;
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
            }
            "Tools" => {
                items.push(
                    menu_item("mi-superpose", "Molecular superposition…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.with_selected_object(cx, |this, obj, cx| {
                                this.open_superpose_dialog(obj, cx)
                            });
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-contacts", "Interaction / contacts…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.with_selected_object(cx, |this, obj, cx| {
                                this.open_contacts_dialog(obj, cx)
                            });
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-secondary", "Reassign secondary str…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.with_selected_object(cx, |this, obj, cx| {
                                this.open_secondary_dialog(obj, cx)
                            });
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-surface", "Surface generation…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.with_selected_object(cx, |this, obj, cx| {
                                this.open_surface_dialog(obj, cx)
                            });
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(
                    menu_item("mi-morph", "Morphing animation…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.with_selected_object(cx, |this, obj, cx| {
                                this.open_morph_dialog(obj, cx)
                            });
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                items.push(menu_separator().into_any_element());
                items.push(
                    menu_item("mi-config", "Configuration…")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.open_config_dialog(cx);
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
            "Help" => {
                items.push(
                    menu_item("mi-about", "About CueMol2-rs")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.dialog = Dialog::About;
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
            _ => {}
        }
        items
    }

    /// Run `f` with the selected object's uid, or set a status hint if none is
    /// selected. Centralises the "needs a selected object" guard for menu items.
    fn with_selected_object(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Self, cuemol_core::Uid, &mut Context<Self>),
    ) {
        match self.selected_object {
            Some(obj) => f(self, obj, cx),
            None => self.set_status("Select an object in the tree first"),
        }
    }

    /// The action toolbar below the header: the high-frequency commands as direct
    /// buttons (the cheap, low-risk parity surface; menus come on top of this).
    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .w_full()
            .h(px(TOOLBAR_H))
            .px_3()
            .bg(rgb(COL_HEADER))
            .border_b_1()
            .border_color(rgb(COL_BORDER))
            .child(button("tb-open", "Open").on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                let path = this
                    .inputs
                    .as_ref()
                    .map(|i| i.open_path.read(cx).text())
                    .unwrap_or_default();
                this.open_file(&path);
                cx.notify();
            })))
            .child(button("tb-savescene", "Save scene").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    let path = this
                        .inputs
                        .as_ref()
                        .map(|i| i.save_qsc.read(cx).text())
                        .unwrap_or_default();
                    this.save_scene_qsc(&path);
                    cx.notify();
                },
            )))
            .child(button("tb-saveobj", "Save obj").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    let path = this
                        .inputs
                        .as_ref()
                        .map(|i| i.save_obj.read(cx).text())
                        .unwrap_or_default();
                    this.save_object(&path);
                    cx.notify();
                },
            )))
            .child(button("tb-fit", "Fit").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.fit_view();
                    cx.notify();
                },
            )))
            .child(button("tb-bg", "BG").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    let next = match this.background {
                        Background::Slate => Background::Black,
                        Background::Black => Background::White,
                        Background::White => Background::Slate,
                    };
                    this.set_background(next);
                    cx.notify();
                },
            )))
            .child(button("tb-proj", "Proj").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.toggle_projection();
                    cx.notify();
                },
            )))
            .child(button("tb-mark", "Mark").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.cycle_center_mark();
                    cx.notify();
                },
            )))
            .child(button("tb-pdb", "PDB").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.open_get_pdb_dialog(cx);
                    cx.notify();
                },
            )))
            .child(div().flex_1())
            .child(button("tb-undo", "Undo").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.undo();
                    cx.notify();
                },
            )))
            .child(button("tb-redo", "Redo").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.redo();
                    cx.notify();
                },
            )))
            .child(button("tb-about", "About").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.dialog = Dialog::About;
                    cx.notify();
                },
            )))
    }

    /// The left scene tree: clickable scene / object / renderer rows that drive the
    /// selection, with a "+ Renderer" button per object. The active scene, selected
    /// object and selected renderer are highlighted.
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        let active_scene = self.state.active_scene();
        for scene in self.scenes() {
            let scene_uid = scene.uid;
            let selected = active_scene == Some(scene_uid);
            rows.push(
                tree_row_el(
                    eid(format!("scene-{}", scene_uid.get())),
                    0,
                    "▸",
                    &scene.name,
                    &format!("{} obj", scene.object_count),
                    selected,
                    true,
                )
                .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                    this.select_scene(scene_uid);
                    cx.notify();
                }))
                .into_any_element(),
            );
            for object in self.objects(scene_uid) {
                let object_uid = object.uid;
                let osel = self.selected_object == Some(object_uid);
                rows.push(
                    tree_row_el(
                        eid(format!("obj-{}", object_uid.get())),
                        1,
                        "◆",
                        &object.name,
                        &format!("{} atoms", object.atom_count),
                        osel && self.selected_renderer.is_none(),
                        false,
                    )
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        this.select_object(object_uid);
                        cx.notify();
                    }))
                    .into_any_element(),
                );
                for renderer in self.renderers(object_uid) {
                    let renderer_uid = renderer.uid;
                    let mark = if renderer.visible { "●" } else { "○" };
                    let rsel = self.selected_renderer == Some(renderer_uid);
                    rows.push(
                        tree_row_el(
                            eid(format!("rend-{}", renderer_uid.get())),
                            2,
                            mark,
                            &renderer.name,
                            &renderer.renderer_type,
                            rsel,
                            false,
                        )
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.select_renderer(object_uid, renderer_uid);
                            cx.notify();
                        }))
                        .into_any_element(),
                    );
                }
                rows.push(
                    button(eid(format!("addr-{}", object_uid.get())), "+ Renderer")
                        .ml(px(44.0))
                        .mr(px(8.0))
                        .my_1()
                        .text_xs()
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.open_new_renderer_dialog(object_uid, cx);
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
        }
        if rows.is_empty() {
            rows.push(
                div()
                    .p_3()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("No objects loaded. Use Open (toolbar) or pass a file path.")
                    .into_any_element(),
            );
        }
        // CueMol2's left dock stacks several panels below the Workspace tree.
        rows.extend(self.cuemol_extra_panels());

        // Coloring panel (CueMol2 coloring-panel.xul): object-level scheme buttons,
        // sharing the core set_object_coloring path with egui/dioxus.
        rows.push(section_label("Coloring").into_any_element());
        rows.push(
            div()
                .flex()
                .flex_row()
                .gap_1()
                .px_2()
                .py_1()
                .child(color_btn("col-element", "Element").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.color_element();
                        cx.notify();
                    },
                )))
                .child(color_btn("col-rainbow", "Rainbow").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.color_rainbow();
                        cx.notify();
                    },
                )))
                .child(color_btn("col-bfac", "B-factor").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.color_bfactor();
                        cx.notify();
                    },
                )))
                .into_any_element(),
        );

        // Symmetry actions (CueMol2 symmetry-panel.xul): cell box + symmetry mates.
        rows.push(section_label("Symmetry actions").into_any_element());
        rows.push(
            div()
                .flex()
                .flex_row()
                .gap_1()
                .px_2()
                .py_1()
                .child(color_btn("sym-cell", "Unit cell").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.toggle_unit_cell();
                        cx.notify();
                    },
                )))
                .child(color_btn("sym-mates", "Symm mol").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.toggle_symm_mates();
                        cx.notify();
                    },
                )))
                .into_any_element(),
        );

        // Selection quick-selects (CueMol2 selection-panel.xul Editor tab).
        rows.push(section_label("Selection").into_any_element());
        rows.push(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .px_2()
                .py_1()
                .child(color_btn("sel-all", "All").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.set_selection("*", cx);
                        cx.notify();
                    },
                )))
                .child(color_btn("sel-none", "None").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.set_selection("none", cx);
                        cx.notify();
                    },
                )))
                .child(color_btn("sel-bb", "Backbone").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.set_selection("bymainch *", cx);
                        cx.notify();
                    },
                )))
                .child(color_btn("sel-sc", "Sidechain").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.set_selection("bysidech *", cx);
                        cx.notify();
                    },
                )))
                .into_any_element(),
        );

        // Density map chickenwire contour (CueMol2 densitymap-panel.xul).
        rows.push(section_label("Density map").into_any_element());
        rows.push(
            div()
                .flex()
                .flex_row()
                .gap_1()
                .px_2()
                .py_1()
                .child(color_btn("den-contour", "Show contour").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.toggle_density_contour();
                        cx.notify();
                    },
                )))
                .child(color_btn("den-solid", "Solid").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.toggle_density_solid();
                        cx.notify();
                    },
                )))
                .child(color_btn("surf-show", "Surface").on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.toggle_surface();
                        cx.notify();
                    },
                )))
                .into_any_element(),
        );

        div()
            .flex()
            .flex_col()
            .w(px(SIDEBAR_W))
            .h_full()
            .bg(rgb(COL_PANEL))
            .border_r_1()
            .border_color(rgb(COL_BORDER))
            .child(panel_title("Workspace"))
            .child(
                // A scrollable region must be stateful (an `id`'d element), since
                // `overflow_y_scroll` lives on `StatefulInteractiveElement`.
                div()
                    .id("scene-tree")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows),
            )
    }

    /// CueMol2 left-dock panels beyond the Workspace tree — MolStruct (per-chain
    /// structure), Symmetry (crystal cell), Density map — as read-only sections.
    fn cuemol_extra_panels(&self) -> Vec<gpui::AnyElement> {
        let mut out: Vec<gpui::AnyElement> = Vec::new();

        // --- MolStruct: chain → residue hierarchy (CueMol2 `molStructTree`), shared
        // with the egui/dioxus front ends via `active_structure_tree`. Residue rows are
        // indented with an em-space. ---
        out.push(section_label("MolStruct").into_any_element());
        let tree = self.state.active_structure_tree();
        if tree.is_empty() {
            out.push(info_row("No molecule loaded.".into()).into_any_element());
        } else {
            for object in tree {
                for chain in object.chains {
                    let chain_id = if chain.chain_id.is_empty() {
                        "_".to_string()
                    } else {
                        chain.chain_id.clone()
                    };
                    out.push(
                        info_row(format!(
                            "Chain {chain_id} — {} residues",
                            chain.residues.len()
                        ))
                        .into_any_element(),
                    );
                    for residue in chain.residues {
                        let ins = residue.ins_code.map(String::from).unwrap_or_default();
                        out.push(
                            info_row(format!(
                                "\u{2003}{} {}{} · {} atoms",
                                residue.res_name.trim(),
                                residue.res_id,
                                ins,
                                residue.atoms.len()
                            ))
                            .into_any_element(),
                        );
                    }
                }
            }
        }

        // --- Symmetry ---
        out.push(section_label("Symmetry").into_any_element());
        let crystals = self.state.active_crystal_summaries();
        if crystals.is_empty() {
            out.push(info_row("No crystal cell.".into()).into_any_element());
        } else {
            for cell in crystals {
                let sg = cell.space_group_name.trim();
                out.push(
                    info_row(format!("SG {}", if sg.is_empty() { "(none)" } else { sg }))
                        .into_any_element(),
                );
                out.push(
                    info_row(format!("a/b/c {:.2} {:.2} {:.2}", cell.a, cell.b, cell.c))
                        .into_any_element(),
                );
                out.push(
                    info_row(format!(
                        "α/β/γ {:.1} {:.1} {:.1}",
                        cell.alpha, cell.beta, cell.gamma
                    ))
                    .into_any_element(),
                );
            }
        }

        // --- Density map ---
        out.push(section_label("Density map").into_any_element());
        let maps = self
            .state
            .active_scene()
            .map(|scene| self.state.density_map_summaries(scene))
            .unwrap_or_default();
        if maps.is_empty() {
            out.push(info_row("No density map.".into()).into_any_element());
        } else {
            for map in maps {
                out.push(
                    info_row(format!(
                        "{} {}×{}×{}",
                        map.name, map.counts[0], map.counts[1], map.counts[2]
                    ))
                    .into_any_element(),
                );
            }
        }

        out
    }

    /// The right inspector: selection editor, scene/object/renderer details, and
    /// the edit/save/export actions. Mirrors the egui inspector's section layout.
    fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut items: Vec<gpui::AnyElement> = Vec::new();

        // --- Selection ---
        items.push(section_label("Selection").into_any_element());
        if let Some(inputs) = &self.inputs {
            items.push(inputs.selection.clone().into_any_element());
        }
        items.push(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(COL_TEXT_DIM))
                        .child(format!("Selected atoms: {}", self.rendered_atom_count)),
                )
                .child(button("ins-clearsel", "Clear").text_xs().on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        if let Some(inputs) = &this.inputs {
                            let sel = inputs.selection.clone();
                            sel.update(cx, |ti, cx| ti.set_text("", cx));
                        }
                        cx.notify();
                    },
                )))
                .into_any_element(),
        );

        // --- Scene ---
        if let Some(scene) = self.state.active_scene_summary() {
            items.push(section_label("Scene").into_any_element());
            items.push(kv("name", scene.name).into_any_element());
            items.push(kv("objects", scene.object_count.to_string()).into_any_element());
            items.push(kv("cameras", scene.camera_count.to_string()).into_any_element());
        }

        // --- Object ---
        if let Some(object) = self.selected_object_summary() {
            let object_uid = object.uid;
            items.push(section_label("Object").into_any_element());
            items.push(kv("name", object.name).into_any_element());
            items.push(kv("atoms", object.atom_count.to_string()).into_any_element());
            items.push(kv("residues", object.residue_count.to_string()).into_any_element());
            items.push(kv("bonds", object.bond_count.to_string()).into_any_element());
            items.push(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(button("ins-addr", "Add renderer").text_xs().on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            this.open_new_renderer_dialog(object_uid, cx);
                            cx.notify();
                        },
                    )))
                    .child(button("ins-chain", "Chain ID").text_xs().on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            this.open_chain_dialog(object_uid, cx);
                            cx.notify();
                        },
                    )))
                    .child(button("ins-renum", "Renumber").text_xs().on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            this.open_residue_dialog(object_uid, cx);
                            cx.notify();
                        },
                    )))
                    .child(danger_button("ins-delatoms", "Delete atoms").text_xs().on_click(
                        cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.open_delete_atoms_dialog(object_uid, cx);
                            cx.notify();
                        }),
                    ))
                    .into_any_element(),
            );
            if let Some(inputs) = &self.inputs {
                items.push(inputs.save_obj.clone().into_any_element());
            }
            items.push(
                button("ins-saveobj", "Save object")
                    .text_xs()
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                        let path = this
                            .inputs
                            .as_ref()
                            .map(|i| i.save_obj.read(cx).text())
                            .unwrap_or_default();
                        this.save_object(&path);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        // --- Renderer ---
        if let Some(renderer) = self.selected_renderer_summary() {
            let object_uid = self.selected_object.unwrap();
            let renderer_uid = renderer.uid;
            items.push(section_label("Renderer").into_any_element());
            items.push(kv("name", renderer.name).into_any_element());
            items.push(kv("type", renderer.renderer_type).into_any_element());
            if let Some(group) = renderer.group {
                items.push(kv("group", group).into_any_element());
            }
            if let Some(style) = renderer.style {
                items.push(kv("style", style).into_any_element());
            }
            items.push(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(checkbox("ins-vis", renderer.visible, "Visible").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            this.toggle_renderer_visible(object_uid, renderer_uid);
                            cx.notify();
                        },
                    )))
                    .child(danger_button("ins-delrend", "Delete").text_xs().on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            this.delete_renderer(object_uid, renderer_uid);
                            cx.notify();
                        },
                    )))
                    .into_any_element(),
            );
        }

        // --- Save scene / Export ---
        items.push(section_label("Scene / export").into_any_element());
        if let Some(inputs) = &self.inputs {
            items.push(inputs.save_qsc.clone().into_any_element());
        }
        items.push(
            button("ins-saveqsc", "Save scene (.qsc)")
                .text_xs()
                .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    let path = this
                        .inputs
                        .as_ref()
                        .map(|i| i.save_qsc.read(cx).text())
                        .unwrap_or_default();
                    this.save_scene_qsc(&path);
                    cx.notify();
                }))
                .into_any_element(),
        );
        if let Some(inputs) = &self.inputs {
            items.push(inputs.export_pov.clone().into_any_element());
        }
        items.push(
            button("ins-exportpov", "Export POV-Ray")
                .text_xs()
                .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    let path = this
                        .inputs
                        .as_ref()
                        .map(|i| i.export_pov.read(cx).text())
                        .unwrap_or_default();
                    this.export_pov(&path);
                    cx.notify();
                }))
                .into_any_element(),
        );

        div()
            .flex()
            .flex_col()
            .w(px(INSPECTOR_W))
            .h_full()
            .bg(rgb(COL_PANEL))
            .border_l_1()
            .border_color(rgb(COL_BORDER))
            .child(panel_title("Inspector"))
            .child(
                div()
                    .id("inspector-scroll")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .min_h_0()
                    .p_2()
                    .overflow_y_scroll()
                    .children(items),
            )
    }

    /// The bottom panel with Output / Sequence / Animation / Render tabs.
    fn bottom_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .w_full()
            .px_2()
            .py_1()
            .bg(rgb(COL_HEADER))
            .border_b_1()
            .border_color(rgb(COL_BORDER))
            .child(self.tab_button(cx, "Output", BottomTab::Output))
            .child(self.tab_button(cx, "Sequence", BottomTab::Sequence))
            .child(self.tab_button(cx, "Animation", BottomTab::Animation))
            .child(self.tab_button(cx, "Render", BottomTab::Render));

        let content = match self.bottom_tab {
            BottomTab::Output => self.output_tab(cx).into_any_element(),
            BottomTab::Sequence => self.sequence_tab(cx).into_any_element(),
            BottomTab::Animation => self.animation_tab(cx).into_any_element(),
            BottomTab::Render => self.render_tab(cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .w_full()
            .h(px(BOTTOM_H))
            .bg(rgb(COL_PANEL))
            .border_t_1()
            .border_color(rgb(COL_BORDER))
            .child(tabs)
            .child(content)
    }

    fn tab_button(&self, cx: &mut Context<Self>, label: &str, tab: BottomTab) -> impl IntoElement {
        let active = self.bottom_tab == tab;
        div()
            .id(eid(format!("tab-{label}")))
            .px_3()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .text_color(rgb(if active { COL_TEXT } else { COL_TEXT_DIM }))
            .bg(rgb(if active { COL_SEL } else { COL_PANEL }))
            .hover(|s| s.bg(rgb(COL_BTN)))
            .child(label.to_string())
            .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                this.bottom_tab = tab;
                cx.notify();
            }))
    }

    fn output_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let lines = self.log_lines.clone();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .child(button("log-clear", "Clear").text_xs().on_click(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            this.log_lines.clear();
                            cx.notify();
                        },
                    ))),
            )
            .child(
                div()
                    .id("output-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .px_2()
                    .overflow_y_scroll()
                    .font_family("monospace")
                    .text_xs()
                    .children(
                        lines
                            .into_iter()
                            .map(|line| div().text_color(rgb(COL_TEXT_DIM)).child(line)),
                    ),
            )
    }

    fn sequence_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for row in self.state.active_sequence_summaries() {
            let object_uid = row.object_uid;
            let chain_id = row.chain_id.clone();
            let sel_expr = format!("chain {chain_id}");
            rows.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_family("monospace")
                            .text_color(rgb(COL_ACCENT))
                            .child(format!("{}:{}", row.object_name, row.chain_id)),
                    )
                    .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child(format!(
                        "{} res / {} atoms",
                        row.residue_count, row.atom_count
                    )))
                    .child(button(eid(format!("seq-sel-{}-{}", object_uid.get(), chain_id)), "Select")
                        .text_xs()
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.select_object(object_uid);
                            if let Some(inputs) = &this.inputs {
                                let sel = inputs.selection.clone();
                                let expr = sel_expr.clone();
                                sel.update(cx, |ti, cx| ti.set_text(expr, cx));
                            }
                            this.set_status(format!("Selected chain {chain_id}"));
                            cx.notify();
                        })))
                    .into_any_element(),
            );
            rows.push(
                div()
                    .font_family("monospace")
                    .text_xs()
                    .text_color(rgb(COL_TEXT_DIM))
                    .pb_1()
                    .child(row.sequence)
                    .into_any_element(),
            );
        }
        if rows.is_empty() {
            rows.push(
                div()
                    .p_2()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("No sequence (load a molecule).")
                    .into_any_element(),
            );
        }
        div()
            .id("sequence-scroll")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .min_h_0()
            .p_2()
            .overflow_y_scroll()
            .children(rows)
    }

    fn animation_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let playing = self.animation_playing;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .p_3()
            .child(div().child(format!("Frames: {}", self.animation_frames)))
            .child(button("anim-add", "Add Camera").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.animation_frames += 1;
                    this.set_status("Added animation frame");
                    cx.notify();
                },
            )))
            .child(
                button("anim-play", if playing { "Stop" } else { "Play" }).on_click(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx| {
                        this.animation_playing = !this.animation_playing;
                        cx.notify();
                    },
                )),
            )
    }

    fn render_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .p_3()
            .child(div().text_color(rgb(COL_TEXT_DIM)).child("Backend: POV-Ray"))
            .child(button("render-go", "Render").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    let path = this
                        .inputs
                        .as_ref()
                        .map(|i| i.export_pov.read(cx).text())
                        .unwrap_or_default();
                    this.export_pov(&path);
                    cx.notify();
                },
            )))
    }

    fn viewport(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let base = div()
            .id("viewport")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .min_h_0()
            .items_center()
            .justify_center()
            .bg(rgb(COL_VIEWPORT))
            // Scroll to zoom, left-drag to orbit, right-drag to pan — each mutates
            // the camera and re-renders the offscreen wgpu image.
            .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _window, cx| {
                let dy = match event.delta {
                    gpui::ScrollDelta::Lines(p) => p.y as f64,
                    gpui::ScrollDelta::Pixels(p) => f32::from(p.y) as f64 / 40.0,
                };
                if dy != 0.0 {
                    this.zoom_camera(if dy > 0.0 { 0.9 } else { 1.1 });
                    cx.notify();
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _window, _cx| {
                    this.drag = Some((gpui::MouseButton::Left, event.position));
                    this.press_start = Some(event.position);
                }),
            )
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|this, event: &gpui::MouseDownEvent, _window, _cx| {
                    this.drag = Some((gpui::MouseButton::Right, event.position));
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                let Some((button, last)) = this.drag else {
                    return;
                };
                // The starting button must still be held; otherwise a mouse-up was
                // missed (released off the viewport) — cancel the gesture.
                if event.pressed_button != Some(button) {
                    this.drag = None;
                    return;
                }
                let dx = f32::from(event.position.x - last.x) as f64;
                let dy = f32::from(event.position.y - last.y) as f64;
                this.drag = Some((button, event.position));
                match button {
                    gpui::MouseButton::Left => this.rotate_camera(dx * 0.5, dy * 0.5),
                    gpui::MouseButton::Right => {
                        let vp_h = this.view_size.1;
                        this.pan_camera(dx, dy, vp_h);
                    }
                    _ => {}
                }
                cx.notify();
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _window, cx| {
                    // A left press that barely moved is a pick, not an orbit.
                    if let Some(start) = this.press_start.take() {
                        let dx = f32::from(event.position.x - start.x);
                        let dy = f32::from(event.position.y - start.y);
                        if (dx * dx + dy * dy).sqrt() < 5.0 {
                            this.try_pick(event.position, cx);
                            cx.notify();
                        }
                    }
                    this.drag = None;
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Right,
                cx.listener(|this, _event: &gpui::MouseUpEvent, _window, _cx| {
                    this.drag = None;
                }),
            );
        match &self.viewport_image {
            // The wgpu-rendered 3D molecular view, composited as a gpui image, with
            // the center-mark overlay painted on top (the small marks don't block
            // the surrounding drag region, so orbit/pan still work).
            Some(image) => base
                .child(
                    img(ImageSource::Render(image.clone()))
                        .size_full()
                        .object_fit(gpui::ObjectFit::Contain),
                )
                .children(center_mark_overlay(self.center_mark)),
            None => base.gap_2().child(div().text_color(rgb(COL_TEXT_DIM)).child(
                "3D viewport (wgpu) — load a molecule (or no GPU adapter available)",
            )),
        }
    }

    /// The bottom status bar: last action + live atom/bond counts + view modes.
    fn status_bar(&self) -> impl IntoElement {
        let projection = if self.camera.is_perspective() {
            "persp"
        } else {
            "ortho"
        };
        let background = match self.background {
            Background::Slate => "slate",
            Background::Black => "black",
            Background::White => "white",
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .w_full()
            .h(px(STATUS_H))
            .px_4()
            .bg(rgb(COL_HEADER))
            .border_t_1()
            .border_color(rgb(COL_BORDER))
            .text_xs()
            .text_color(rgb(COL_TEXT_DIM))
            .child(div().flex_1().child(self.status.clone()))
            .child(div().child(format!(
                "{} atoms / {} bonds",
                self.rendered_atom_count, self.rendered_bond_count
            )))
            .child(div().child(format!("{projection} · {background}")))
    }

    /// The modal dialog overlay (a dimmed backdrop + centered box). Only rendered
    /// when `self.dialog != Dialog::None`. Each dialog has explicit buttons; the
    /// backdrop is inert (no click-to-close), keeping the gpui event model simple.
    fn dialog_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.dialog {
            Dialog::None => div().into_any_element(),
            Dialog::About => self.about_dialog(cx).into_any_element(),
            Dialog::NewRenderer { target, ty } => {
                self.new_renderer_dialog(cx, *target, ty.clone()).into_any_element()
            }
            Dialog::ChainId { target } => self.chain_dialog(cx, *target).into_any_element(),
            Dialog::ResidueNum { target } => self.residue_dialog(cx, *target).into_any_element(),
            Dialog::DeleteAtoms { target } => self.delete_atoms_dialog(cx, *target).into_any_element(),
            Dialog::GetPdb => self.get_pdb_dialog(cx).into_any_element(),
            Dialog::Superpose { reference, moving } => {
                self.superpose_dialog(cx, *reference, *moving).into_any_element()
            }
            Dialog::Contacts { target, hbond } => {
                self.contacts_dialog(cx, *target, *hbond).into_any_element()
            }
            Dialog::Secondary { target, ignore_breaks } => {
                self.secondary_dialog(cx, *target, *ignore_breaks).into_any_element()
            }
            Dialog::Surface { target } => self.surface_dialog(cx, *target).into_any_element(),
            Dialog::Morph { target, source, frame } => {
                self.morph_dialog(cx, *target, *source, *frame).into_any_element()
            }
            Dialog::Config => self.config_dialog(cx).into_any_element(),
        };
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000099))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .min_w(px(360.0))
                    .max_w(px(520.0))
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(COL_PANEL))
                    .border_1()
                    .border_color(rgb(COL_BORDER))
                    .child(body),
            )
    }

    fn about_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("CueMol2-rs"))
            .child(
                div()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("gpui desktop front end (Zed's GPU UI framework)"),
            )
            .child(
                div()
                    .text_color(rgb(COL_TEXT_DIM))
                    .text_xs()
                    .child("Panels in gpui · 3D view rendered offscreen with wgpu"),
            )
            .child(dialog_buttons_close(cx, "OK"))
    }

    fn new_renderer_dialog(
        &self,
        cx: &mut Context<Self>,
        target: cuemol_core::Uid,
        selected_ty: String,
    ) -> impl IntoElement {
        let mut type_rows: Vec<gpui::AnyElement> = Vec::new();
        for (id, label) in RENDERER_TYPES {
            let id_s = id.to_string();
            let chosen = *id == selected_ty;
            type_rows.push(
                div()
                    .id(eid(format!("nr-{id}")))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(rgb(if chosen { COL_SEL } else { COL_PANEL }))
                    .hover(|s| s.bg(rgb(COL_BTN)))
                    .child(label.to_string())
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        if let Dialog::NewRenderer { ty, .. } = &mut this.dialog {
                            *ty = id_s.clone();
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        let name_input = self.dialog_inputs.renderer_name.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Add renderer"))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child(format!("on object uid {}", target.get())),
            )
            .child(
                div()
                    .id("nr-typelist")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .h(px(180.0))
                    .overflow_y_scroll()
                    .children(type_rows),
            )
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Name"))
            .children(name_input.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(button("nr-create", "Create").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let (ty, name) = match &this.dialog {
                                Dialog::NewRenderer { ty, .. } => (
                                    ty.clone(),
                                    this.dialog_inputs
                                        .renderer_name
                                        .as_ref()
                                        .map(|i| i.read(cx).text())
                                        .unwrap_or_default(),
                                ),
                                _ => return,
                            };
                            this.create_renderer(target, &ty, &name);
                            this.close_dialog();
                            cx.notify();
                        },
                    )))
                    .child(dialog_cancel(cx)),
            )
    }

    fn chain_dialog(&self, cx: &mut Context<Self>, target: cuemol_core::Uid) -> impl IntoElement {
        let input = self.dialog_inputs.chain_id.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Change chain ID"))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("New chain ID for the current selection"),
            )
            .children(input.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(button("chain-apply", "Apply").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let chain = this
                                .dialog_inputs
                                .chain_id
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            let selection = this.applied_selection.clone();
                            this.change_chain(target, &selection, &chain);
                            this.close_dialog();
                            cx.notify();
                        },
                    )))
                    .child(dialog_cancel(cx)),
            )
    }

    fn residue_dialog(&self, cx: &mut Context<Self>, target: cuemol_core::Uid) -> impl IntoElement {
        let input = self.dialog_inputs.residue_start.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Renumber residues"))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("Start number for the current selection"),
            )
            .children(input.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(button("res-apply", "Apply").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let start = this
                                .dialog_inputs
                                .residue_start
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default()
                                .trim()
                                .parse::<i32>()
                                .unwrap_or(1);
                            let selection = this.applied_selection.clone();
                            this.renumber_residues(target, &selection, start);
                            this.close_dialog();
                            cx.notify();
                        },
                    )))
                    .child(dialog_cancel(cx)),
            )
    }

    fn delete_atoms_dialog(
        &self,
        cx: &mut Context<Self>,
        target: cuemol_core::Uid,
    ) -> impl IntoElement {
        let input = self.dialog_inputs.delete_sel.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_DANGER)).child("Delete atoms"))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("Selection of atoms to delete"),
            )
            .children(input.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(danger_button("del-apply", "Delete").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let selection = this
                                .dialog_inputs
                                .delete_sel
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            this.delete_atoms(target, &selection);
                            this.close_dialog();
                            cx.notify();
                        },
                    )))
                    .child(dialog_cancel(cx)),
            )
    }

    fn get_pdb_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let input = self.dialog_inputs.pdb_id.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Get PDB"))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("Set the Open path to a local PDB file by ID"),
            )
            .children(input.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(button("pdb-use", "Use as Open path").on_click(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            let id = this
                                .dialog_inputs
                                .pdb_id
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            let path = format!("{}.pdb", id.trim());
                            if let Some(inputs) = &this.inputs {
                                let open = inputs.open_path.clone();
                                open.update(cx, |ti, cx| ti.set_text(path, cx));
                            }
                            this.set_status(format!("Open path set to {}.pdb", id.trim()));
                            this.close_dialog();
                            cx.notify();
                        },
                    )))
                    .child(dialog_cancel(cx)),
            )
    }

    fn superpose_dialog(
        &self,
        cx: &mut Context<Self>,
        reference: cuemol_core::Uid,
        moving: Option<cuemol_core::Uid>,
    ) -> impl IntoElement {
        let mut obj_rows: Vec<gpui::AnyElement> = Vec::new();
        for object in self.state.active_object_summaries() {
            if object.uid == reference {
                continue;
            }
            let uid = object.uid;
            let chosen = moving == Some(uid);
            obj_rows.push(
                div()
                    .id(eid(format!("sp-{}", uid.get())))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(rgb(if chosen { COL_SEL } else { COL_PANEL }))
                    .hover(|s| s.bg(rgb(COL_BTN)))
                    .child(object.name)
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        if let Dialog::Superpose { moving, .. } = &mut this.dialog {
                            *moving = Some(uid);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        if obj_rows.is_empty() {
            obj_rows.push(
                div()
                    .text_xs()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("(no other object to move)")
                    .into_any_element(),
            );
        }
        let ref_sel = self.dialog_inputs.ref_sel.clone();
        let mov_sel = self.dialog_inputs.mov_sel.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Molecular superposition"))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child(format!(
                "Reference: object {}  ·  pick a moving object:",
                reference.get()
            )))
            .child(
                div()
                    .id("sp-list")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .max_h(px(120.0))
                    .overflow_y_scroll()
                    .children(obj_rows),
            )
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Reference selection"))
            .children(ref_sel.map(|i| i.into_any_element()))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Moving selection"))
            .children(mov_sel.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(button("sp-go", "Superpose").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let moving = match &this.dialog {
                                Dialog::Superpose { moving, .. } => *moving,
                                _ => None,
                            };
                            let Some(moving) = moving else {
                                this.set_status("Superpose: pick a moving object");
                                cx.notify();
                                return;
                            };
                            let rsel = this
                                .dialog_inputs
                                .ref_sel
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            let msel = this
                                .dialog_inputs
                                .mov_sel
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            this.run_superpose(reference, &rsel, moving, &msel);
                            this.close_dialog();
                            cx.notify();
                        },
                    )))
                    .child(dialog_cancel(cx)),
            )
    }

    fn contacts_dialog(
        &self,
        cx: &mut Context<Self>,
        target: cuemol_core::Uid,
        hbond: bool,
    ) -> impl IntoElement {
        let cmin = self.dialog_inputs.contact_min.clone();
        let cmax = self.dialog_inputs.contact_max.clone();
        let sel_label = if self.applied_selection.is_empty() {
            "(all atoms)".to_string()
        } else {
            self.applied_selection.clone()
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Interaction / contacts"))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child(format!(
                "Object {}  ·  selection: {sel_label}",
                target.get()
            )))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Distance min / max (Å)"))
            .children(cmin.map(|i| i.into_any_element()))
            .children(cmax.map(|i| i.into_any_element()))
            .child(checkbox("ct-hbond", hbond, "H-bond candidates only").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    if let Dialog::Contacts { hbond, .. } = &mut this.dialog {
                        *hbond = !*hbond;
                    }
                    cx.notify();
                },
            )))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(button("ct-go", "Calculate").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let hbond = matches!(this.dialog, Dialog::Contacts { hbond: true, .. });
                            let rmin = parse_input(&this.dialog_inputs.contact_min, cx, 2.5);
                            let rmax = parse_input(&this.dialog_inputs.contact_max, cx, 4.0);
                            let sel = this.applied_selection.clone();
                            this.run_contacts(target, &sel, rmin, rmax, hbond);
                            this.bottom_tab = BottomTab::Output;
                            this.close_dialog();
                            cx.notify();
                        },
                    )))
                    .child(dialog_cancel(cx)),
            )
    }

    fn secondary_dialog(
        &self,
        cx: &mut Context<Self>,
        target: cuemol_core::Uid,
        ignore_breaks: bool,
    ) -> impl IntoElement {
        let angle = self.dialog_inputs.secondary_angle.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Reassign secondary structure"))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child(format!("Object {}", target.get())))
            .child(checkbox("ss-breaks", ignore_breaks, "Ignore chain breaks").on_click(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    if let Dialog::Secondary { ignore_breaks, .. } = &mut this.dialog {
                        *ignore_breaks = !*ignore_breaks;
                    }
                    cx.notify();
                },
            )))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Helix angle threshold (deg)"))
            .children(angle.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(button("ss-go", "Assign").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let ignore = matches!(this.dialog, Dialog::Secondary { ignore_breaks: true, .. });
                            let angle = parse_input(&this.dialog_inputs.secondary_angle, cx, 90.0);
                            this.run_secondary(target, ignore, angle);
                            this.close_dialog();
                            cx.notify();
                        },
                    )))
                    .child(dialog_cancel(cx)),
            )
    }

    fn surface_dialog(&self, cx: &mut Context<Self>, target: cuemol_core::Uid) -> impl IntoElement {
        let density = self.dialog_inputs.surf_density.clone();
        let probe = self.dialog_inputs.surf_probe.clone();
        let path = self.dialog_inputs.surf_path.clone();
        let sel_label = if self.applied_selection.is_empty() {
            "(all atoms)".to_string()
        } else {
            self.applied_selection.clone()
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Molecular surface (OBJ)"))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child(format!(
                "Object {}  ·  selection: {sel_label}",
                target.get()
            )))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Density / probe radius"))
            .children(density.map(|i| i.into_any_element()))
            .children(probe.map(|i| i.into_any_element()))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Output OBJ path"))
            .children(path.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(button("surf-go", "Generate OBJ").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let density = parse_input(&this.dialog_inputs.surf_density, cx, 5.0);
                            let probe = parse_input(&this.dialog_inputs.surf_probe, cx, 1.4);
                            let path = this
                                .dialog_inputs
                                .surf_path
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            let sel = this.applied_selection.clone();
                            this.run_surface(target, &sel, density, probe, &path);
                            this.close_dialog();
                            cx.notify();
                        },
                    )))
                    .child(dialog_cancel(cx)),
            )
    }

    fn morph_dialog(
        &self,
        cx: &mut Context<Self>,
        target: cuemol_core::Uid,
        source: Option<cuemol_core::Uid>,
        frame: f64,
    ) -> impl IntoElement {
        let mut obj_rows: Vec<gpui::AnyElement> = Vec::new();
        for object in self.state.active_object_summaries() {
            if object.uid == target {
                continue;
            }
            let uid = object.uid;
            let chosen = source == Some(uid);
            obj_rows.push(
                div()
                    .id(eid(format!("mo-{}", uid.get())))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(rgb(if chosen { COL_SEL } else { COL_PANEL }))
                    .hover(|s| s.bg(rgb(COL_BTN)))
                    .child(object.name)
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        if let Dialog::Morph { source, .. } = &mut this.dialog {
                            *source = Some(uid);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        if obj_rows.is_empty() {
            obj_rows.push(
                div()
                    .text_xs()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("(no source object)")
                    .into_any_element(),
            );
        }
        let name = self.dialog_inputs.morph_name.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Morphing animation"))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child(format!(
                "Target object {}  ·  source frame object:",
                target.get()
            )))
            .child(
                div()
                    .id("mo-list")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .max_h(px(110.0))
                    .overflow_y_scroll()
                    .children(obj_rows),
            )
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Frame name"))
            .children(name.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(button("mo-append", "Append frame").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let source = match &this.dialog {
                                Dialog::Morph { source, .. } => *source,
                                _ => None,
                            };
                            let Some(source) = source else {
                                this.set_status("Morph: pick a source object");
                                cx.notify();
                                return;
                            };
                            let name = this
                                .dialog_inputs
                                .morph_name
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            this.run_morph_append(target, source, &name);
                            cx.notify();
                        },
                    )))
                    .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child(format!("frame {frame:.2}")))
                    .child(button("mo-dec", "-").on_click(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            if let Dialog::Morph { frame, .. } = &mut this.dialog {
                                *frame = (*frame - 0.25).max(0.0);
                            }
                            cx.notify();
                        },
                    )))
                    .child(button("mo-inc", "+").on_click(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            if let Dialog::Morph { frame, .. } = &mut this.dialog {
                                *frame += 0.25;
                            }
                            cx.notify();
                        },
                    )))
                    .child(button("mo-apply", "Apply").on_click(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let frame = match &this.dialog {
                                Dialog::Morph { frame, .. } => *frame,
                                _ => 0.0,
                            };
                            this.run_morph_apply(target, frame);
                            cx.notify();
                        },
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .child(dialog_cancel(cx)),
            )
    }

    fn config_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let key = self.dialog_inputs.config_key.clone();
        let value = self.dialog_inputs.config_value.clone();
        let path = self.dialog_inputs.config_path.clone();
        let entries = self.state.config_entries();
        let entry_rows = entries
            .into_iter()
            .take(200)
            .map(|e| kv(&e.key, e.value).into_any_element());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(rgb(COL_ACCENT)).child("Configuration"))
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Key / value"))
            .children(key.map(|i| i.into_any_element()))
            .children(value.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(button("cfg-get", "Get").text_xs().on_click(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            let k = this
                                .dialog_inputs
                                .config_key
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            this.config_get(&k);
                            if let Some(input) = &this.dialog_inputs.config_value {
                                let v = this.state.config_value(&k);
                                let input = input.clone();
                                input.update(cx, |ti, cx| ti.set_text(v, cx));
                            }
                            cx.notify();
                        },
                    )))
                    .child(button("cfg-set", "Set").text_xs().on_click(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            let k = this
                                .dialog_inputs
                                .config_key
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            let v = this
                                .dialog_inputs
                                .config_value
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            this.config_set(&k, &v);
                            cx.notify();
                        },
                    ))),
            )
            .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child("Config file path"))
            .children(path.map(|i| i.into_any_element()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(button("cfg-load", "Load").text_xs().on_click(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            let p = this
                                .dialog_inputs
                                .config_path
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            this.config_load(&p);
                            cx.notify();
                        },
                    )))
                    .child(button("cfg-save", "Save").text_xs().on_click(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            let p = this
                                .dialog_inputs
                                .config_path
                                .as_ref()
                                .map(|i| i.read(cx).text())
                                .unwrap_or_default();
                            this.config_save(&p);
                            cx.notify();
                        },
                    ))),
            )
            .child(
                div()
                    .id("cfg-entries")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .max_h(px(130.0))
                    .mt_1()
                    .p_1()
                    .rounded_sm()
                    .bg(rgb(COL_VIEWPORT))
                    .overflow_y_scroll()
                    .children(entry_rows),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .child(dialog_cancel(cx)),
            )
    }
}

/// Parse a numeric [`TextInput`]'s content as `f64`, falling back to `default`
/// for empty/garbage input (used by the tool dialogs' numeric fields).
fn parse_input(
    input: &Option<Entity<TextInput>>,
    cx: &mut Context<CueMolGpuiApp>,
    default: f64,
) -> f64 {
    input
        .as_ref()
        .map(|i| i.read(cx).text())
        .and_then(|t| t.trim().parse::<f64>().ok())
        .unwrap_or(default)
}

/// Build an auto-framed camera for a scene from its bounding box.
fn frame_camera(snapshot: &cuemol_render::RenderSceneSnapshot) -> cuemol_qsys::Camera {
    let mut camera = cuemol_qsys::Camera::new();
    if let Some((min, max)) = snapshot.bounds {
        let size = ((max.x - min.x).powi(2) + (max.y - min.y).powi(2) + (max.z - min.z).powi(2))
            .sqrt()
            .max(1.0);
        camera.set_center(cuemol_core::Vector4::new3(
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

/// Compute the panned camera centre for a screen drag of `(dx, dy)` pixels over a
/// `vp_h`-pixel-tall viewport.
///
/// The view transform is `R·(p − centre)`, so shifting the molecule on screen by
/// a camera-space delta `Δ` means `centre_new = centre − Rᵀ·Δ`. `R` is a rotation
/// (orthonormal), so `Rᵀ·e_x` / `Rᵀ·e_y` are just rows 0 / 1 of `R` — the world
/// axes that map to screen right / up. Orthographic half-height is `zoom/2` over
/// `vp_h/2` pixels, i.e. `zoom/vp_h` world units per pixel (isotropic). Screen-y
/// points down, hence the `dy` negation. Same convention as the orbit, so the two
/// gestures stay consistent.
fn pan_center(
    camera: &cuemol_qsys::Camera,
    dx_pix: f64,
    dy_pix: f64,
    vp_h: u32,
) -> cuemol_core::Vector4 {
    let scale = camera.zoom() / (vp_h.max(1) as f64);
    let r = camera.rotation().to_matrix();
    let (right_x, right_y, right_z) = (r.values[0][0], r.values[0][1], r.values[0][2]);
    let (up_x, up_y, up_z) = (r.values[1][0], r.values[1][1], r.values[1][2]);
    let dcam_x = dx_pix * scale;
    let dcam_y = -dy_pix * scale;
    let c = camera.center();
    cuemol_core::Vector4::new3(
        c.x - dcam_x * right_x - dcam_y * up_x,
        c.y - dcam_x * right_y - dcam_y * up_y,
        c.z - dcam_x * right_z - dcam_y * up_z,
    )
}

/// Find the atom whose projected screen position is nearest `cursor_px` (within
/// `radius_px`), or `None`. `mvp` is the same column-major matrix the offscreen
/// renderer uses, so the projection matches what's drawn; the readback does not
/// flip vertically, so image row 0 (NDC y = +1) is the top — hence the `1 - y`.
/// Returns the atom's `(object_uid, atom_index)` for `atom_summary` lookup.
fn pick_atom(
    atoms: &[cuemol_render::RenderAtom],
    mvp: [[f32; 4]; 4],
    cursor_px: (f64, f64),
    vp_px: (f64, f64),
    radius_px: f64,
) -> Option<(u64, usize)> {
    let mut best: Option<(f64, u64, usize)> = None;
    for atom in atoms {
        let (Some(object_uid), Some(atom_index)) = (atom.object_uid, atom.atom_index) else {
            continue;
        };
        let c = &atom.center;
        let v = [c.x as f32, c.y as f32, c.z as f32, 1.0];
        // column-major mvp * column vector
        let mut clip = [0.0f32; 4];
        for i in 0..4 {
            for col in 0..4 {
                clip[i] += mvp[col][i] * v[col];
            }
        }
        if clip[3].abs() < 1.0e-6 {
            continue;
        }
        let ndc_x = (clip[0] / clip[3]) as f64;
        let ndc_y = (clip[1] / clip[3]) as f64;
        let ndc_z = (clip[2] / clip[3]) as f64;
        if !(-1.0..=1.0).contains(&ndc_z) {
            continue; // outside the depth slab (behind camera / clipped)
        }
        let sx = (ndc_x + 1.0) / 2.0 * vp_px.0;
        let sy = (1.0 - ndc_y) / 2.0 * vp_px.1;
        let dx = sx - cursor_px.0;
        let dy = sy - cursor_px.1;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= radius_px && best.is_none_or(|(b, _, _)| dist < b) {
            best = Some((dist, object_uid, atom_index));
        }
    }
    best.map(|(_, object_uid, atom_index)| (object_uid, atom_index))
}

/// The on-screen 3D viewport's pixel size from a live gpui [`Window`]: the window
/// minus the given chrome, scaled by the display DPI so the offscreen image is
/// crisp on retina. `chrome_w`/`chrome_h` are the total non-viewport extents the
/// caller computes from the live panel-visibility flags.
fn viewport_pixels(window: &Window, chrome_w: f32, chrome_h: f32) -> (u32, u32) {
    let vs = window.viewport_size();
    viewport_pixels_from(
        f32::from(vs.width),
        f32::from(vs.height),
        window.scale_factor(),
        chrome_w,
        chrome_h,
    )
}

/// Pure core of [`viewport_pixels`] (no `Window`, so it is unit-testable): logical
/// window size + DPI scale - chrome -> physical viewport pixels, clamped to >= 1×1.
fn viewport_pixels_from(win_w: f32, win_h: f32, scale: f32, chrome_w: f32, chrome_h: f32) -> (u32, u32) {
    let w = ((win_w - chrome_w).max(1.0) * scale).round() as u32;
    let h = ((win_h - chrome_h).max(1.0) * scale).round() as u32;
    (w.max(1), h.max(1))
}

fn panel_title(text: &str) -> impl IntoElement {
    div()
        .w_full()
        .px_3()
        .py_2()
        .bg(rgb(COL_HEADER))
        .border_b_1()
        .border_color(rgb(COL_BORDER))
        .text_color(rgb(COL_TEXT_DIM))
        .text_xs()
        .child(text.to_string())
}

/// Wrap a `String` into an [`gpui::ElementId`] (gpui needs stable, unique ids on
/// every interactive element; we derive them from scene/object/renderer uids).
fn eid(s: String) -> gpui::ElementId {
    gpui::ElementId::from(gpui::SharedString::from(s))
}

/// A standard clickable toolbar/inspector button.
fn button(id: impl Into<gpui::ElementId>, label: impl Into<String>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(rgb(COL_BTN))
        .text_color(rgb(COL_TEXT))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(COL_BTN_HOVER)))
        .child(label.into())
}

/// A destructive-action button (delete), tinted to read as dangerous.
fn danger_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<String>,
) -> gpui::Stateful<gpui::Div> {
    button(id, label).bg(rgb(COL_DANGER)).hover(|s| s.bg(rgb(0xd6695f)))
}

/// A header pill that reads as on/off (panel-visibility toggles).
fn toggle_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<String>,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .text_xs()
        .bg(rgb(if active { COL_SEL } else { COL_BTN }))
        .text_color(rgb(if active { COL_TEXT } else { COL_TEXT_DIM }))
        .hover(|s| s.bg(rgb(COL_BTN_HOVER)))
        .child(label.into())
}

/// A labelled checkbox (a small filled/empty box + caption).
fn checkbox(
    id: impl Into<gpui::ElementId>,
    checked: bool,
    label: impl Into<String>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .rounded_sm()
        .cursor_pointer()
        .bg(rgb(COL_BTN))
        .hover(|s| s.bg(rgb(COL_BTN_HOVER)))
        .child(
            div()
                .w(px(12.0))
                .h(px(12.0))
                .rounded_sm()
                .border_1()
                .border_color(rgb(COL_TEXT_DIM))
                .bg(rgb(if checked { COL_ACCENT } else { COL_PANEL })),
        )
        .child(label.into())
}

/// A scene-tree row (stateful so it can take a click + hover). The caller chains
/// `.on_click(...)` to select the corresponding scene / object / renderer.
fn tree_row_el(
    id: impl Into<gpui::ElementId>,
    depth: usize,
    mark: &str,
    name: &str,
    detail: &str,
    selected: bool,
    accent: bool,
) -> gpui::Stateful<gpui::Div> {
    let name_color = if accent { COL_ACCENT } else { COL_TEXT };
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .w_full()
        .px_3()
        .py_1()
        .pl(px(12.0 + depth as f32 * 16.0))
        .bg(rgb(if selected { COL_SEL } else { COL_PANEL }))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(if selected { COL_SEL } else { COL_BTN })))
        .child(div().text_color(rgb(COL_TEXT_DIM)).child(mark.to_string()))
        .child(div().flex_1().text_color(rgb(name_color)).child(name.to_string()))
        .child(
            div()
                .text_color(rgb(COL_TEXT_DIM))
                .text_xs()
                .child(detail.to_string()),
        )
}

/// A key/value display row for the inspector.
fn kv(key: &str, value: impl Into<String>) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .gap_2()
        .child(div().text_xs().text_color(rgb(COL_TEXT_DIM)).child(key.to_string()))
        .child(div().text_xs().child(value.into()))
}

/// A dropdown menu row (full-width, hover-highlighted).
fn menu_item(id: impl Into<gpui::ElementId>, label: impl Into<String>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w_full()
        .px_3()
        .py_1()
        .cursor_pointer()
        .text_sm()
        .text_color(rgb(COL_TEXT))
        .hover(|s| s.bg(rgb(COL_BTN)))
        .child(label.into())
}

/// A compact bordered button for the side-panel control rows (Coloring etc.).
fn color_btn(id: impl Into<gpui::ElementId>, label: impl Into<String>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .cursor_pointer()
        .text_xs()
        .text_color(rgb(COL_TEXT))
        .border_1()
        .border_color(rgb(COL_BORDER))
        .rounded_sm()
        .hover(|s| s.bg(rgb(COL_BTN)))
        .child(label.into())
}

/// A thin separator line for dropdown menus.
fn menu_separator() -> gpui::Div {
    div().w_full().my_1().h(px(1.0)).bg(rgb(COL_BORDER))
}

/// An inspector section header (accent text + underline).
fn section_label(text: &str) -> gpui::Div {
    div()
        .w_full()
        .mt_2()
        .pb_1()
        .border_b_1()
        .border_color(rgb(COL_BORDER))
        .text_color(rgb(COL_ACCENT))
        .text_xs()
        .child(text.to_string())
}

/// A plain read-only text row inside a left-dock panel section.
fn info_row(text: String) -> gpui::Div {
    div()
        .px_3()
        .py_1()
        .text_xs()
        .text_color(rgb(COL_TEXT))
        .child(text)
}

/// The dialog "Cancel" button (closes the modal).
fn dialog_cancel(cx: &mut Context<CueMolGpuiApp>) -> gpui::Stateful<gpui::Div> {
    button("dlg-cancel", "Cancel").on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
        this.close_dialog();
        cx.notify();
    }))
}

/// A right-aligned single close button row (for the About dialog).
fn dialog_buttons_close(cx: &mut Context<CueMolGpuiApp>, label: &str) -> impl IntoElement {
    div().flex().flex_row().justify_end().child(
        button("dlg-ok", label.to_string()).on_click(cx.listener(
            |this, _: &gpui::ClickEvent, _, cx| {
                this.close_dialog();
                cx.notify();
            },
        )),
    )
}

/// The viewport center-mark overlay (`None` when disabled). Cross = two dim bars;
/// Axis = red X / green Y, matching the egui front end.
fn center_mark_overlay(kind: CenterMarkKind) -> Option<gpui::AnyElement> {
    let (h_color, v_color) = match kind {
        CenterMarkKind::None => return None,
        CenterMarkKind::Cross => (COL_TEXT_DIM, COL_TEXT_DIM),
        CenterMarkKind::Axis => (0xdc4444u32, 0x50b460u32),
    };
    Some(
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .relative()
                    .w(px(22.0))
                    .h(px(22.0))
                    .child(
                        div()
                            .absolute()
                            .top(px(10.0))
                            .left(px(0.0))
                            .w(px(22.0))
                            .h(px(2.0))
                            .bg(rgb(h_color)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(10.0))
                            .top(px(0.0))
                            .w(px(2.0))
                            .h(px(22.0))
                            .bg(rgb(v_color)),
                    ),
            )
            .into_any_element(),
    )
}

/// Launch the gpui desktop GUI. `open_paths` are molecule files to load on start.
pub fn run(open_paths: Vec<PathBuf>) {
    Application::new().run(move |cx: &mut App| {
        // Register the text-editing key bindings the TextInput fields listen for;
        // without this, fields render but ignore Backspace / arrows / Cmd-V.
        crate::gpui_widgets::bind_keys(cx);
        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
        let open_paths = open_paths.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| {
                    let mut app = CueMolGpuiApp::new(open_paths);
                    app.ensure_inputs(cx);
                    app
                })
            },
        )
        .expect("failed to open gpui window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpui_app_builds_and_binds_the_scene_tree() {
        // The non-rendering logic (state + summaries) is testable headlessly; only
        // `render()` needs a gpui window. A fresh app has one active scene.
        let app = CueMolGpuiApp::new(Vec::new());
        assert_eq!(app.status, "Ready");
        let scenes = app.scenes();
        assert_eq!(scenes.len(), 1, "ensure_active_scene should make one scene");
        // The tree accessors return cleanly for the active scene (no objects yet).
        assert_eq!(app.objects(scenes[0].uid).len(), 0);
    }

    #[test]
    fn camera_controls_mutate_the_view_camera() {
        // The interactive re-render logic is GPU-free verifiable at the camera
        // layer: zoom scales the camera, rotate changes its orientation. (The
        // camera->image step is covered by the offscreen renderer's tests.)
        let mut app = CueMolGpuiApp::new(Vec::new());
        let zoom0 = app.camera.zoom();
        app.zoom_camera(2.0);
        assert!((app.camera.zoom() - zoom0 * 2.0).abs() < 1.0e-6);

        let rot0 = app.camera.rotation();
        app.rotate_camera(20.0, 10.0);
        let rot1 = app.camera.rotation();
        // The orientation actually changed.
        let delta = (rot1.x - rot0.x).abs()
            + (rot1.y - rot0.y).abs()
            + (rot1.z - rot0.z).abs()
            + (rot1.w - rot0.w).abs();
        assert!(delta > 1.0e-3, "rotation should change: delta={delta}");
    }

    // Column-major MVP * column vector -> clip space; divide by w for NDC.
    fn project_ndc(mvp: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 3] {
        let mut c = [0.0f32; 4];
        for i in 0..4 {
            for col in 0..4 {
                c[i] += mvp[col][i] * v[col];
            }
        }
        [c[0] / c[3], c[1] / c[3], c[2] / c[3]]
    }

    #[test]
    fn pan_center_moves_the_view_along_the_drag() {
        // GPU-free oracle: panning right then re-projecting a fixed world point
        // must shift its screen-x the SAME way the orbit's convention implies — a
        // rightward drag moves the molecule right (NDC +x). Uses camera_mvp so the
        // pan is verified against the exact transform the renderer uses, not a
        // hand-rolled one (so the default-rotation y-flip can't fool it).
        let mut camera = cuemol_qsys::Camera::new();
        camera.set_zoom(50.0);
        let aspect = 800.0 / 600.0;
        let world = [7.0, 3.0, -2.0, 1.0]; // an arbitrary off-centre point

        let before = project_ndc(crate::offscreen::camera_mvp(&camera, aspect), world);
        // Drag right by 120 px and down by 60 px over a 600-px-tall viewport.
        camera.set_center(pan_center(&camera, 120.0, 60.0, 600));
        let after = project_ndc(crate::offscreen::camera_mvp(&camera, aspect), world);

        // Rightward drag -> the point's screen-x increases (moves right with it).
        assert!(
            after[0] > before[0] + 0.05,
            "pan did not move the point right: {before:?} -> {after:?}"
        );
        // Magnitude, derived from the same inputs (so it can't drift): a 120 px
        // drag moves the centre `120 * zoom/vp_h` world units; ortho NDC-x divides
        // by the half-WIDTH `(zoom/2) * aspect`. zoom is unchanged by a pan.
        let dx_ndc = after[0] - before[0];
        let world_shift = 120.0 * camera.zoom() / 600.0;
        let expected_dx = (world_shift / ((camera.zoom() / 2.0) * aspect)) as f32;
        assert!(
            (dx_ndc - expected_dx).abs() < 0.02,
            "pan x magnitude off: dx_ndc={dx_ndc} expected={expected_dx}"
        );

        // A PURELY vertical drag must move the point in y and NOT bleed into x
        // (the two pan axes are independent). Magnitude-only on y, because the
        // default camera rotation flips the y sign (same reason the projection
        // tests assert |y|, not y) — so this is robust to that flip while still
        // proving pan-y is wired and orthogonal to pan-x.
        let mut vcam = cuemol_qsys::Camera::new();
        vcam.set_zoom(50.0);
        let v_before = project_ndc(crate::offscreen::camera_mvp(&vcam, aspect), world);
        vcam.set_center(pan_center(&vcam, 0.0, 80.0, 600));
        let v_after = project_ndc(crate::offscreen::camera_mvp(&vcam, aspect), world);
        assert!(
            (v_after[1] - v_before[1]).abs() > 0.05,
            "vertical pan didn't move y: {v_before:?} -> {v_after:?}"
        );
        assert!(
            (v_after[0] - v_before[0]).abs() < 1.0e-4,
            "vertical pan leaked into x: {v_before:?} -> {v_after:?}"
        );
    }

    #[test]
    fn viewport_pixels_track_window_size_minus_chrome() {
        // The offscreen resolution is the window minus the supplied chrome, ×DPI.
        let cw = SIDEBAR_W + INSPECTOR_W;
        let ch = HEADER_H + TOOLBAR_H + STATUS_H + BOTTOM_H;
        let (w, h) = viewport_pixels_from(1280.0, 820.0, 1.0, cw, ch);
        assert_eq!(w, (1280.0 - cw) as u32);
        assert_eq!(h, (820.0 - ch) as u32);
        // Retina doubles the physical resolution.
        let (w2, h2) = viewport_pixels_from(1280.0, 820.0, 2.0, cw, ch);
        assert_eq!((w2, h2), (w * 2, h * 2));
        // A tiny window never yields a zero dimension (clamped to >= 1).
        let (wmin, hmin) = viewport_pixels_from(10.0, 10.0, 1.0, cw, ch);
        assert!(wmin >= 1 && hmin >= 1);
    }

    #[test]
    fn panning_changes_the_rendered_image() {
        // End-to-end (this Mac has Metal, so it runs): load a molecule, render,
        // pan, render again — a meaningful fraction of pixels must change. Proves
        // the whole interactive pan chain (handler math -> camera -> reused
        // renderer -> RGBA) actually moves the picture.
        let mut app = CueMolGpuiApp::new(vec![reference_pdb()]);
        let Some(before) = app.render_rgba() else {
            eprintln!("no GPU adapter / no molecule; skipping pan render test");
            return;
        };
        app.pan_camera(60.0, 40.0, app.view_size.1);
        let after = app.render_rgba().expect("post-pan image");
        let changed = before
            .iter()
            .zip(&after)
            .filter(|(x, y)| (**x as i32 - **y as i32).abs() > 16)
            .count();
        assert!(changed > 200, "pan barely changed the image: {changed}");
    }

    #[test]
    fn resizing_re_renders_at_the_new_resolution() {
        // Resize-follow: changing view_size and re-rendering yields a buffer of the
        // new pixel count (w*h*4), proving the offscreen render tracks the window.
        let mut app = CueMolGpuiApp::new(vec![reference_pdb()]);
        app.view_size = (200, 150);
        let Some(small) = app.render_rgba() else {
            return;
        };
        assert_eq!(small.len(), 200 * 150 * 4);
        app.view_size = (320, 240);
        let big = app.render_rgba().expect("post-resize image");
        assert_eq!(big.len(), 320 * 240 * 4);
    }

    // --- Action-layer logic (headless: no gpui window needed) ----------------

    /// Open a reference molecule into a fresh app, returning the app + the loaded
    /// object uid, or `None` if the reference tree is absent (self-skip).
    fn app_with_molecule() -> Option<(CueMolGpuiApp, cuemol_core::Uid)> {
        let path = reference_pdb();
        if !path.exists() {
            return None;
        }
        let mut app = CueMolGpuiApp::new(Vec::new());
        app.open_file(path.to_str().unwrap());
        let object = app.selected_object?;
        Some((app, object))
    }

    #[test]
    fn open_file_loads_selects_and_counts_atoms() {
        let Some((app, _object)) = app_with_molecule() else {
            return;
        };
        assert!(app.selected_object.is_some(), "load should select the object");
        assert!(app.rendered_atom_count > 0, "1CRN has atoms");
        assert_eq!(app.recent_paths.len(), 1, "open records a recent path");
        assert!(app.status.starts_with("Loaded"), "status: {}", app.status);
    }

    #[test]
    fn create_then_delete_renderer_round_trips() {
        let Some((mut app, object)) = app_with_molecule() else {
            return;
        };
        let before = app.renderers(object).len();
        app.create_renderer(object, "cpk", "cpk");
        assert_eq!(app.renderers(object).len(), before + 1);
        let renderer = app.selected_renderer.expect("new renderer is selected");
        app.delete_renderer(object, renderer);
        assert_eq!(app.renderers(object).len(), before, "delete undoes the add");
        assert_eq!(app.selected_renderer, None);
    }

    #[test]
    fn toggle_visibility_flips_the_flag() {
        let Some((mut app, object)) = app_with_molecule() else {
            return;
        };
        app.create_renderer(object, "cpk", "cpk");
        let renderer = app.selected_renderer.unwrap();
        let vis0 = app
            .renderers(object)
            .into_iter()
            .find(|r| r.uid == renderer)
            .unwrap()
            .visible;
        app.toggle_renderer_visible(object, renderer);
        let vis1 = app
            .renderers(object)
            .into_iter()
            .find(|r| r.uid == renderer)
            .unwrap()
            .visible;
        assert_ne!(vis0, vis1, "visibility should toggle");
    }

    #[test]
    fn undo_redo_restores_renderer_count() {
        let Some((mut app, object)) = app_with_molecule() else {
            return;
        };
        let before = app.renderers(object).len();
        app.create_renderer(object, "ballstick", "bs");
        assert_eq!(app.renderers(object).len(), before + 1);
        app.undo();
        assert_eq!(app.renderers(object).len(), before, "undo removes the renderer");
        app.redo();
        assert_eq!(app.renderers(object).len(), before + 1, "redo re-adds it");
    }

    #[test]
    fn delete_atoms_then_undo_restores_count() {
        let Some((mut app, object)) = app_with_molecule() else {
            return;
        };
        let before = app.selected_object_summary().unwrap().atom_count;
        app.delete_atoms(object, "name CA");
        let after = app.selected_object_summary().unwrap().atom_count;
        assert!(after < before, "deleting CA atoms reduces the count ({after} < {before})");
        app.undo();
        assert_eq!(
            app.selected_object_summary().unwrap().atom_count,
            before,
            "undo restores the deleted atoms"
        );
    }

    #[test]
    fn secondary_structure_assignment_runs() {
        let Some((mut app, object)) = app_with_molecule() else {
            return;
        };
        let atoms_before = app.selected_object_summary().unwrap().atom_count;
        app.run_secondary(object, false, 90.0);
        // Assert the operation SUCCEEDED (status begins "Reassigned…"), not merely
        // that some status was set — an error path also sets a non-empty status and
        // leaves the atom count untouched, so the weaker check proves nothing.
        assert!(
            app.status.starts_with("Reassigned"),
            "secondary structure did not succeed: {}",
            app.status
        );
        assert_eq!(app.selected_object_summary().unwrap().atom_count, atoms_before);
    }

    #[test]
    fn contacts_calculation_logs_output() {
        let Some((mut app, object)) = app_with_molecule() else {
            return;
        };
        let before = app.log_lines.len();
        app.run_contacts(object, "", 2.5, 4.0, false);
        // Success pushes the JSON result line THEN the status; assert it actually
        // ran (status is the success message) rather than just that the log grew
        // (an error also appends a status line).
        assert!(app.log_lines.len() > before, "contacts should log a result line");
        assert!(
            app.status.contains("computed"),
            "contacts did not succeed: {}",
            app.status
        );
        // The result line is a JSON array of contacts.
        let result = &app.log_lines[before];
        assert!(
            result.trim_start().starts_with('['),
            "expected a JSON array, got: {result}"
        );
    }

    #[test]
    fn config_set_get_round_trips() {
        let mut app = CueMolGpuiApp::new(Vec::new());
        app.config_set("gui:test_key", "42");
        assert_eq!(app.state.config_value("gui:test_key"), "42");
    }

    #[test]
    fn projection_and_background_toggle() {
        let mut app = CueMolGpuiApp::new(Vec::new());
        let persp0 = app.camera.is_perspective();
        app.toggle_projection();
        assert_ne!(persp0, app.camera.is_perspective());
        app.set_background(Background::White);
        assert_eq!(app.background, Background::White);
        app.cycle_center_mark();
        // Cross -> Axis after one cycle from the default Cross.
        assert_eq!(app.center_mark, CenterMarkKind::Axis);
    }

    fn mk_atom(obj: u64, idx: usize, x: f64, y: f64, z: f64) -> cuemol_render::RenderAtom {
        cuemol_render::RenderAtom {
            center: cuemol_core::Vector4::new3(x, y, z),
            radius: 1.0,
            color: None,
            object_uid: Some(obj),
            atom_index: Some(idx),
            selected: false,
            detail: None,
        }
    }

    // Project an atom centre to viewport pixels with the SAME convention pick_atom
    // uses, so the test exercises the nearest-atom selection, not the mapping.
    fn project_px(
        atom: &cuemol_render::RenderAtom,
        mvp: [[f32; 4]; 4],
        vp: (f64, f64),
    ) -> (f64, f64) {
        let c = &atom.center;
        let v = [c.x as f32, c.y as f32, c.z as f32, 1.0];
        let mut clip = [0.0f32; 4];
        for i in 0..4 {
            for col in 0..4 {
                clip[i] += mvp[col][i] * v[col];
            }
        }
        let ndc_x = (clip[0] / clip[3]) as f64;
        let ndc_y = (clip[1] / clip[3]) as f64;
        ((ndc_x + 1.0) / 2.0 * vp.0, (1.0 - ndc_y) / 2.0 * vp.1)
    }

    #[test]
    fn pick_atom_selects_the_nearest_projected_atom() {
        let mut camera = cuemol_qsys::Camera::new();
        camera.set_zoom(50.0);
        camera.set_cam_dist(200.0);
        camera.set_slab_depth(200.0);
        let vp = (800.0_f64, 600.0_f64);
        let mvp = crate::offscreen::camera_mvp(&camera, vp.0 / vp.1);
        let atoms = vec![
            mk_atom(7, 0, 0.0, 0.0, 0.0),
            mk_atom(7, 1, 12.0, 0.0, 0.0),
            mk_atom(7, 2, -12.0, 0.0, 0.0),
        ];
        // Clicking exactly on atom 1's projected position picks atom 1.
        let p1 = project_px(&atoms[1], mvp, vp);
        assert_eq!(pick_atom(&atoms, mvp, p1, vp, 12.0), Some((7, 1)));
        // Clicking on the centre atom picks atom 0.
        let p0 = project_px(&atoms[0], mvp, vp);
        assert_eq!(pick_atom(&atoms, mvp, p0, vp, 12.0), Some((7, 0)));
        // A click far from any projected atom hits nothing.
        assert_eq!(pick_atom(&atoms, mvp, (p1.0 + 250.0, p1.1), vp, 12.0), None);
    }

    // Path to a small reference molecule; self-skip cleanly if the tree is absent.
    // Anchored on CARGO_MANIFEST_DIR (NOT the cwd, which is the package dir under
    // `cargo test`) so the GPU pixel tests actually run instead of silently
    // skipping — matches the workspace compat-test convention. Override with
    // CUEMOL2_REF_DIR at build time.
    fn reference_pdb() -> PathBuf {
        option_env!("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"))
            .join("tests/test_data/1CRN.pdb")
    }
}
