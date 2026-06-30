//! Dioxus-based desktop GUI — a third front end parallel to the egui (`gui`) and
//! gpui (`gpui_app`) modules in `cuemol-desktop`.
//!
//! Same division of labour as the gpui shell: Dioxus/RSX draws the panel chrome
//! (header, scene-tree sidebar, status bar) over the headless [`DesktopAppState`]
//! logic, while the 3D molecular view is rendered separately with wgpu
//! ([`cuemol_desktop_core::offscreen`]) and composited into the webview as an
//! `<img>`. Because a webview can't take a raw GPU texture, the rendered RGBA is
//! PNG-encoded and embedded as a base64 `data:` URI; it is regenerated whenever
//! the camera moves, the viewport resizes, or the selection changes. (A wry
//! custom-protocol handler serving raw PNG bytes would avoid the base64 inflation
//! — a worthwhile later optimisation.)
//!
//! The offscreen image is rendered at the live viewport's CSS-pixel size (tracked
//! via the `onresize` observer), so a click's `element_coordinates()` (also CSS
//! px) map 1:1 onto image pixels and atom picking needs no DPI math.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dioxus::html::input_data::MouseButton;
use dioxus::html::{InteractionElementOffset, Key, PointerInteraction};
use dioxus::prelude::*;

use cuemol_desktop_core::offscreen::{camera_mvp, encode_rgba_png, SceneRenderer};
use cuemol_desktop_core::{pick_atom_screen, DesktopAppState};

// Initial offscreen render resolution. Updated to the live viewport size by the
// `onresize` observer (CSS-px basis), so the image matches the element 1:1.
const INIT_VIEW_W: u32 = 960;
const INIT_VIEW_H: u32 = 720;
// Upper bound on the offscreen texture so a maximised window can't request an
// absurd allocation; the image stays sharp well past typical viewport sizes.
const MAX_VIEW_DIM: u32 = 4096;
// A left press+release that moved less than this (CSS px) is treated as a pick,
// not an orbit — mirrors the egui `drag_total` / gpui click discrimination.
const CLICK_MAX_MOVE: f64 = 4.0;
// Screen-pixel pick tolerance (radius around the click, CSS px).
const PICK_RADIUS_PX: f64 = 12.0;

// Background of the 3D image (matches --viewport so letterboxed margins blend in).
const VIEW_CLEAR: [f64; 4] = [0.08, 0.08, 0.11, 1.0];

/// Renderer representation types offered by the Render panel, mirroring the egui
/// (`gui.rs`) and gpui (`gpui_app.rs`) `RENDERER_TYPES` so all three front ends expose
/// the same set (CueMol2 Render menu). `(internal_id, display_label)`.
const RENDERER_TYPES: &[(&str, &str)] = &[
    ("cpk", "CPK"),
    ("ballstick", "Ball & Stick"),
    ("trace", "Trace"),
    ("ribbon", "Ribbon"),
    ("cartoon", "Cartoon"),
    ("tube", "Tube"),
];

// Dark palette shared by the panels (CSS hex), mirroring the gpui shell.
// Colors reference CSS custom properties (defined on the root via `theme_vars`), so a
// light/dark toggle just swaps the root variable values — every baked `{COL_*}` style
// follows automatically (CSS variables inherit), no per-style refactor needed.
const COL_BG: &str = "var(--col-bg)";
const COL_PANEL: &str = "var(--col-panel)";
const COL_HEADER: &str = "var(--col-header)";
const COL_VIEWPORT: &str = "var(--col-viewport)";
const COL_BORDER: &str = "var(--col-border)";
const COL_TEXT: &str = "var(--col-text)";
const COL_TEXT_DIM: &str = "var(--col-text-dim)";
const COL_ACCENT: &str = "var(--col-accent)";

/// The CSS custom-property block defining the palette for the dark or light theme,
/// prepended to the root element's style (children inherit the variables).
fn theme_vars(light: bool) -> &'static str {
    if light {
        "--col-bg:#f4f4f6;--col-panel:#eaeaef;--col-header:#deded8;\
         --col-viewport:#fafafc;--col-border:#c4c4cc;--col-text:#1e1e28;\
         --col-text-dim:#6a6a78;--col-accent:#1f6fd0;"
    } else {
        "--col-bg:#1e1e28;--col-panel:#252530;--col-header:#2d2d3a;\
         --col-viewport:#14141c;--col-border:#3a3a48;--col-text:#d6d6e0;\
         --col-text-dim:#8a8a9a;--col-accent:#6db3f2;"
    }
}

/// Command-line molecule paths, handed to the root component through Dioxus
/// context (the component is a plain `fn`, so it can't capture them directly).
#[derive(Clone)]
struct InitialPaths(Vec<PathBuf>);

/// Which camera gesture an in-progress drag performs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DragKind {
    Orbit,
    Pan,
}

/// An in-progress pointer drag: gesture kind, last pointer position (element
/// coords, CSS px), and the accumulated movement so a near-stationary left
/// release can be told apart from an orbit and treated as a pick.
#[derive(Clone, Copy)]
struct DragState {
    kind: DragKind,
    last_x: f64,
    last_y: f64,
    moved: f64,
}

/// One rendered row of the scene/object/renderer tree (owned, so it outlives the
/// signal read guard when handed to RSX). `PartialEq` lets it be a memoized
/// component prop.
#[derive(Clone, PartialEq)]
struct TreeRow {
    depth: usize,
    mark: String,
    name: String,
    detail: String,
    accent: bool,
}

/// One stacked left-dock panel below the Workspace tree (CueMol2 side panels),
/// rendered as a titled list of read-only lines.
#[derive(Clone, PartialEq)]
struct PanelSection {
    title: String,
    lines: Vec<String>,
}

/// Mutable application model, held behind a `Signal`. Not `Clone` (it owns the
/// wgpu device); the reactive runtime stores it by value and re-renders on write.
struct Model {
    state: DesktopAppState,
    status: String,
    camera: cuemol_qsys::Camera,
    /// Persistent wgpu renderer (device + pipeline reused across frames). `None`
    /// when no GPU adapter is available, in which case the viewport shows the
    /// placeholder instead of an image.
    renderer: Option<SceneRenderer>,
    /// Scene tessellated into GPU geometry; reused for every camera move, rebuilt
    /// when the selection changes (so the highlight shows) or a file is loaded.
    mesh: Option<cuemol_render::RenderMesh>,
    /// `data:image/png;base64,…` of the latest offscreen render (`None` = empty
    /// scene / no adapter).
    image_uri: Option<String>,
    /// Active drag gesture, or `None`.
    drag: Option<DragState>,
    /// Offscreen render resolution = the live viewport's CSS-pixel size.
    view_size: (u32, u32),
    /// The applied selection expression (drives atom highlighting); set by picking.
    selection_text: String,
    /// Symmetry panel toggles (CueMol2 "Unit cell" / "Symm mol"): draw the cell box
    /// and the home-cell symmetry-mate copies.
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
    /// Hydrogen-bond dashed lines.
    show_hbonds: bool,
    /// Atom/residue labels overlaid on the selected atoms.
    show_labels: bool,
    /// Which top menu's dropdown is open (index into the menu bar), or `None`.
    menu_open: Option<usize>,
    /// Animation playback (CueMol2 SimpleSpin): a continuous camera spin while set.
    anim_playing: bool,
    /// Animation timeline objects + elapsed seconds (shared-core anim_spin_direction).
    animation_objects: Vec<cuemol_desktop_core::AnimObject>,
    animation_elapsed: f64,
    /// Light vs dark UI theme (CueMol2's native look is light; dark is the default).
    theme_light: bool,
    /// Viewport background clear colour (CueMol2 Scene → Background): cycles
    /// dark / black / white. RGBA in 0..1 for the offscreen renderer.
    bg_clear: [f64; 4],
    /// Undo / redo stacks (CueMol2 Edit → Undo/Redo). Each entry is a full clone of the
    /// scene state taken before a mutating edit, mirroring the egui/gpui front ends.
    undo_stack: Vec<DioxusUndo>,
    redo_stack: Vec<DioxusUndo>,
}

/// A point-in-time scene snapshot for undo/redo: the cloned app state plus the
/// selection expression (the only model field that participates in an edit).
struct DioxusUndo {
    state: DesktopAppState,
    selection_text: String,
}

impl Model {
    /// Build the model, loading any command-line molecules into a fresh scene,
    /// restoring a saved `.qsc` view (else auto-framing), and tessellating once.
    /// Mirrors gpui's `CueMolGpuiApp::new`.
    fn new(open_paths: Vec<PathBuf>) -> Self {
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
        let mut model = Self {
            state,
            status,
            camera: cuemol_qsys::Camera::new(),
            renderer: SceneRenderer::new(),
            mesh: None,
            image_uri: None,
            drag: None,
            view_size: (INIT_VIEW_W, INIT_VIEW_H),
            selection_text: String::new(),
            show_unit_cell: false,
            show_symm_mates: false,
            show_density_contour: false,
            density_level: 1.5,
            show_density_solid: false,
            density_solid_mesh: None,
            show_surface: false,
            surface_mesh: None,
            show_hbonds: false,
            show_labels: false,
            menu_open: None,
            anim_playing: false,
            animation_objects: Vec::new(),
            animation_elapsed: 0.0,
            theme_light: false,
            bg_clear: VIEW_CLEAR,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        model.rebuild_after_load();
        model
    }

    /// The render snapshot for the current selection, with the Symmetry panel's crystal
    /// geometry (unit-cell box / symmetry mates) appended via the shared core. Used by
    /// the render paths — NOT by picking, which must hit-test the bare molecule.
    fn current_render_snapshot(&self) -> Option<cuemol_render::RenderSceneSnapshot> {
        let mut snapshot = self
            .state
            .active_render_snapshot(&self.selection_text)
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
        self.state
            .append_hbond_geometry(&mut snapshot, self.show_hbonds);
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

    /// Recompute snapshot → camera → mesh → image after the scene's molecule set
    /// changes. Restores a loaded `.qsc`'s saved view; auto-frames a plain
    /// molecule from its bounding box.
    fn rebuild_after_load(&mut self) {
        let snapshot = self.current_render_snapshot();
        self.camera = self
            .state
            .active_scene_saved_camera()
            .or_else(|| snapshot.as_ref().map(frame_camera))
            .unwrap_or_default();
        self.mesh = snapshot.as_ref().map(cuemol_render::tessellate_snapshot);
        self.rerender();
    }

    /// Re-tessellate the current scene at the active selection (so the highlight
    /// reflects `selection_text`) and re-render. Used after a pick.
    fn rebuild_selection(&mut self) {
        self.mesh = self
            .current_render_snapshot()
            .as_ref()
            .map(cuemol_render::tessellate_snapshot);
        self.rerender();
    }

    /// Re-render the offscreen 3D image at the current camera into `image_uri`.
    /// Cheap relative to first build: reuses the persistent device + cached mesh.
    fn rerender(&mut self) {
        self.image_uri = self.render_uri();
    }

    /// Apply an object-level coloring scheme to every molecule in the active scene
    /// (CueMol2 Coloring panel), then re-tessellate. Uses the shared-core
    /// `set_object_coloring`, so the snapshot's object-coloring cascade recolours each
    /// carbon (and the ribbon) — identical to the egui front end.
    fn apply_coloring(&mut self, coloring_type: Option<&str>, attrs: BTreeMap<String, String>) {
        self.push_undo();
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
        self.status = match coloring_type {
            Some(name) => format!("Coloring: {name}"),
            None => "Coloring: Element".into(),
        };
        self.rebuild_selection();
    }

    /// Coloring presets wired to the sidebar buttons.
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

    /// Apply a selection expression (CueMol2 Selection panel quick-selects) and
    /// re-tessellate so the highlight updates. Shares the molstr selection grammar.
    fn set_selection(&mut self, expr: &str) {
        self.selection_text = expr.to_string();
        self.status = format!("Selection: {expr}");
        self.rebuild_selection();
    }

    /// Symmetry panel toggles (CueMol2 "Unit cell" / "Symm mol"). Re-tessellate so the
    /// shared crystal geometry appears/disappears in the 3D view.
    fn toggle_unit_cell(&mut self) {
        self.show_unit_cell = !self.show_unit_cell;
        self.rebuild_selection();
    }

    fn toggle_symm_mates(&mut self) {
        self.show_symm_mates = !self.show_symm_mates;
        self.rebuild_selection();
    }

    /// Density map "chickenwire" contour toggle (CueMol2 Density map panel).
    fn toggle_density_contour(&mut self) {
        self.show_density_contour = !self.show_density_contour;
        self.rebuild_selection();
    }

    /// Hydrogen-bond dashed-line display toggle.
    fn toggle_hbonds(&mut self) {
        self.show_hbonds = !self.show_hbonds;
        self.rebuild_selection();
    }

    /// Atom-label display toggle (overlaid on the composited image; no re-tessellation).
    fn toggle_labels(&mut self) {
        self.show_labels = !self.show_labels;
    }

    /// Animation playback toggle (CueMol2 SimpleSpin); the use_future loop drives it.
    fn toggle_animation(&mut self) {
        self.anim_playing = !self.anim_playing;
    }

    /// Set every renderer's visibility for a Show/Hide animation object, re-tessellating
    /// only if something changed.
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
            self.rebuild_selection();
        }
    }

    /// Load an AMBER mdcrd trajectory onto the active object (MolAnim source).
    fn load_trajectory(&mut self, path: &str) {
        if let Some(uid) = self
            .state
            .active_object_summaries()
            .first()
            .map(|object| object.uid)
        {
            match self
                .state
                .load_amber_trajectory(uid, std::path::Path::new(path))
            {
                Ok(frames) => self.status = format!("Trajectory: {frames} frames"),
                Err(error) => self.status = format!("Trajectory load failed: {error}"),
            }
        }
    }

    /// Append an animation object to the timeline (CueMol2 anim-panel Add).
    fn add_anim(&mut self, kind: cuemol_desktop_core::AnimKind) {
        self.animation_objects.push(cuemol_desktop_core::AnimObject {
            kind,
            duration: 3.0,
        });
    }

    /// Remove the animation object at `index`.
    fn remove_anim(&mut self, index: usize) {
        if index < self.animation_objects.len() {
            self.animation_objects.remove(index);
            self.animation_elapsed = 0.0;
        }
    }

    /// Move the animation object at `index` one slot toward the front (`delta`=-1) or
    /// back (`delta`=+1) — CueMol2 anim-panel Up/Down reorder.
    fn move_anim(&mut self, index: usize, delta: i32) {
        let target = index as i32 + delta;
        if index < self.animation_objects.len() && target >= 0 && (target as usize) < self.animation_objects.len() {
            self.animation_objects.swap(index, target as usize);
            self.animation_elapsed = 0.0;
        }
    }

    /// Cycle the viewport background: dark → black → white → dark (CueMol2 Scene →
    /// Background). Re-renders so the new clear colour shows immediately.
    fn cycle_background(&mut self) {
        const DARK: [f64; 4] = VIEW_CLEAR;
        const BLACK: [f64; 4] = [0.0, 0.0, 0.0, 1.0];
        const WHITE: [f64; 4] = [1.0, 1.0, 1.0, 1.0];
        let (next, label) = if self.bg_clear == DARK {
            (BLACK, "black")
        } else if self.bg_clear == BLACK {
            (WHITE, "white")
        } else {
            (DARK, "dark")
        };
        self.bg_clear = next;
        self.status = format!("Background: {label}");
        self.rerender();
    }

    /// Discard all undo/redo history (CueMol2 Edit → Clear undo data).
    fn clear_undo_data(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.status = "Undo data cleared".into();
    }

    /// Nudge a numeric renderer property (bond/atom radius) by a clamped delta and
    /// re-tessellate (CueMol2 renderer property dialog), via the shared core.
    fn adjust_renderer_property(
        &mut self,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        key: &str,
        delta: f64,
    ) {
        self.push_undo();
        let (default, min, max) = match key {
            "sphr" => (0.30, 0.05, 3.0),
            _ => (0.18, 0.02, 2.0),
        };
        let current = self
            .state
            .renderer_property(object_uid, renderer_uid, key)
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(default);
        let next = (current + delta).clamp(min, max);
        let _ =
            self.state
                .set_renderer_property(object_uid, renderer_uid, key, format!("{next:.3}"));
        self.rebuild_selection();
    }

    /// Create a renderer of representation `ty` on `object_uid` (CueMol2 Render menu).
    /// The name is suffixed with the current renderer count so repeated adds stay unique,
    /// mirroring egui's `open_renderer_dialog` naming. Re-tessellates so the new
    /// representation appears in the 3D view immediately.
    fn add_renderer(&mut self, object_uid: cuemol_core::Uid, ty: &str) {
        self.push_undo();
        let index = self.state.renderer_summaries(object_uid).len();
        let name = format!("{ty}{index}");
        match self.state.new_renderer_in_active_scene(object_uid, ty, name) {
            Ok(_) => self.status = format!("Added {ty} renderer"),
            Err(error) => self.status = format!("Add renderer failed: {error}"),
        }
        self.rebuild_selection();
    }

    /// Remove a renderer from its object (CueMol2 workspace-tree → Delete).
    fn delete_renderer(&mut self, object_uid: cuemol_core::Uid, renderer_uid: cuemol_core::Uid) {
        self.push_undo();
        match self
            .state
            .destroy_renderer_in_object(object_uid, renderer_uid)
        {
            Ok(_) => self.status = "Renderer deleted".to_string(),
            Err(error) => self.status = format!("Delete renderer failed: {error}"),
        }
        self.rebuild_selection();
    }

    /// Flip a renderer's visibility (CueMol2 workspace-tree checkbox). `visible` is the
    /// renderer's CURRENT state; the new state is its negation.
    fn toggle_renderer_visible(
        &mut self,
        object_uid: cuemol_core::Uid,
        renderer_uid: cuemol_core::Uid,
        visible: bool,
    ) {
        self.push_undo();
        let _ = self
            .state
            .set_renderer_visible(object_uid, renderer_uid, !visible);
        self.rebuild_selection();
    }

    /// Push the current scene state onto the undo stack before a mutating edit
    /// (capped at 50, clears the redo stack). Call at the START of each edit op.
    fn push_undo(&mut self) {
        self.undo_stack.push(DioxusUndo {
            state: self.state.clone(),
            selection_text: self.selection_text.clone(),
        });
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Replace the live scene with a saved snapshot, invalidating cached meshes and
    /// re-tessellating (shared by undo and redo).
    fn restore_undo(&mut self, snapshot: DioxusUndo) {
        self.state = snapshot.state;
        self.selection_text = snapshot.selection_text;
        self.surface_mesh = None;
        self.density_solid_mesh = None;
        self.rebuild_selection();
    }

    /// Undo the last edit (CueMol2 Edit → Undo). Saves the current state to the redo
    /// stack first so redo can replay it.
    fn undo(&mut self) {
        let Some(snapshot) = self.undo_stack.pop() else {
            self.status = "Nothing to undo".into();
            return;
        };
        self.redo_stack.push(DioxusUndo {
            state: self.state.clone(),
            selection_text: self.selection_text.clone(),
        });
        self.restore_undo(snapshot);
        self.status = "Undo".into();
    }

    /// Redo the last undone edit (CueMol2 Edit → Redo).
    fn redo(&mut self) {
        let Some(snapshot) = self.redo_stack.pop() else {
            self.status = "Nothing to redo".into();
            return;
        };
        self.undo_stack.push(DioxusUndo {
            state: self.state.clone(),
            selection_text: self.selection_text.clone(),
        });
        self.restore_undo(snapshot);
        self.status = "Redo".into();
    }

    /// Download a structure from the RCSB PDB by 4-character ID and load it (CueMol2
    /// File → Get PDB). Uses `curl` (a subprocess, no HTTP crate), mirroring the egui
    /// front end's downloader.
    fn get_pdb(&mut self, pdb_id: &str) {
        let id = pdb_id.trim();
        if id.is_empty() {
            self.status = "Get PDB: enter a 4-character ID".into();
            return;
        }
        let url = rcsb_pdb_url(id);
        let path = std::env::temp_dir().join(format!("cuemol-pdb-{}.pdb", sanitize_pdb_id(id)));
        match std::process::Command::new("curl")
            .arg("-fsSL")
            .arg("-o")
            .arg(&path)
            .arg(&url)
            .status()
        {
            Ok(status) if status.success() => self.open_path(path),
            Ok(status) => self.status = format!("Get PDB failed: curl status {status}"),
            Err(error) => self.status = format!("Get PDB failed: {error}"),
        }
    }

    /// Save the active scene to a `.qsc` file (CueMol2 File → Save scene). Read-only
    /// on the model; the shared-core writer serializes the scene + its qdfpdb data.
    fn save_scene(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() {
            self.status = "Save scene: enter a path".into();
            return;
        }
        match self.state.save_active_scene_qsc(path) {
            Ok(()) => self.status = format!("Saved scene: {path}"),
            Err(error) => self.status = format!("Save scene failed: {error}"),
        }
    }

    /// Save the first molecule to a `.pdb`/`.sdf` file (CueMol2 File → Save object).
    /// The extension selects the writer; no extension defaults to PDB.
    fn save_object(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() {
            self.status = "Save object: enter a path".into();
            return;
        }
        let Some(uid) = self
            .state
            .active_object_summaries()
            .first()
            .map(|object| object.uid)
        else {
            self.status = "Save object: no molecule loaded".into();
            return;
        };
        match self.state.save_object_as(uid, path) {
            Ok(()) => self.status = format!("Saved object: {path}"),
            Err(error) => self.status = format!("Save object failed: {error}"),
        }
    }

    /// Export the active scene's atoms to a POV-Ray `.pov` file (CueMol2 File →
    /// Export scene). Exports the current selection (empty = whole scene).
    fn export_scene(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() {
            self.status = "Export scene: enter a path".into();
            return;
        }
        match self.state.export_active_scene_pov(path, &self.selection_text) {
            Ok(atoms) => self.status = format!("Exported {atoms} atoms → {path}"),
            Err(error) => self.status = format!("Export scene failed: {error}"),
        }
    }

    /// The active scene's first molecule, or `None` (with a status message) — the
    /// target object for the single-molecule editing ops below.
    fn first_object_uid(&mut self) -> Option<cuemol_core::Uid> {
        match self.state.active_object_summaries().first() {
            Some(object) => Some(object.uid),
            None => {
                self.status = "No molecule loaded".into();
                None
            }
        }
    }

    /// Delete the atoms matching the current selection (CueMol2 Edit → Delete atoms).
    /// Requires a non-empty selection so an empty box can't silently delete everything
    /// (the core treats an empty selector as "all").
    fn delete_selected_atoms(&mut self) {
        if self.selection_text.trim().is_empty() {
            self.status = "Delete atoms: make a selection first".into();
            return;
        }
        let Some(uid) = self.first_object_uid() else {
            return;
        };
        let selection = self.selection_text.clone();
        self.push_undo();
        match self.state.delete_atoms_in_object(uid, &selection) {
            Ok(count) => self.status = format!("Deleted {count} atoms"),
            Err(error) => self.status = format!("Delete atoms failed: {error}"),
        }
        self.rebuild_selection();
    }

    /// Rename the chain of the selected atoms (CueMol2 Edit → Change chain name). An
    /// empty selection applies to the whole molecule (core default).
    fn change_chain(&mut self, chain: &str) {
        let chain = chain.trim().to_string();
        if chain.is_empty() {
            self.status = "Change chain: enter a chain name".into();
            return;
        }
        let Some(uid) = self.first_object_uid() else {
            return;
        };
        let selection = self.selection_text.clone();
        self.push_undo();
        match self.state.change_chain_in_object(uid, &selection, &chain) {
            Ok(count) => self.status = format!("Chain → {chain} on {count} atoms"),
            Err(error) => self.status = format!("Change chain failed: {error}"),
        }
        self.rebuild_selection();
    }

    /// Recompute secondary structure (DSSP) on the first molecule (CueMol2 Tools →
    /// Reassign secondary structure). Updates the `secondary` residue prop the
    /// cartoon/ribbon renderers + SS coloring read. Defaults match the gpui dialog
    /// (don't ignore chain breaks, 90° helix-angle threshold).
    fn reassign_secondary(&mut self) {
        let Some(uid) = self.first_object_uid() else {
            return;
        };
        self.push_undo();
        match self
            .state
            .assign_secondary_structure_in_object(uid, false, 90.0)
        {
            Ok(_) => self.status = "Secondary structure reassigned".into(),
            Err(error) => self.status = format!("Reassign SS failed: {error}"),
        }
        self.rebuild_selection();
    }

    /// Renumber the residues of the selected atoms starting at `start` (CueMol2
    /// Edit → Renumber residues). An empty selection applies to the whole molecule.
    fn renumber_residues(&mut self, start: i32) {
        let Some(uid) = self.first_object_uid() else {
            return;
        };
        let selection = self.selection_text.clone();
        self.push_undo();
        match self.state.renumber_residues_in_object(uid, &selection, start) {
            Ok(count) => self.status = format!("Renumbered {count} atoms from {start}"),
            Err(error) => self.status = format!("Renumber failed: {error}"),
        }
        self.rebuild_selection();
    }

    /// Run a menu-bar dropdown action and close the menu.
    fn run_menu_action(&mut self, action: &str) {
        match action {
            "new_scene" => self.new_scene_tab(),
            "undo" => self.undo(),
            "redo" => self.redo(),
            "fit_view" => self.fit_view(),
            "toggle_projection" => self.toggle_projection(),
            "reassign_ss" => self.reassign_secondary(),
            "background" => self.cycle_background(),
            "clear_undo" => self.clear_undo_data(),
            "sel_none" => self.set_selection("none"),
            "sel_all" => self.set_selection("*"),
            "col_element" => self.color_element(),
            "col_rainbow" => self.color_rainbow(),
            "col_bfactor" => self.color_bfactor(),
            "surface" => self.toggle_surface(),
            "den_contour" => self.toggle_density_contour(),
            "den_solid" => self.toggle_density_solid(),
            "hbonds" => self.toggle_hbonds(),
            "labels" => self.toggle_labels(),
            "toggle_theme" => self.theme_light = !self.theme_light,
            "about" => self.status = "CueMol2-rs — Dioxus front end".into(),
            _ => {}
        }
        self.menu_open = None;
    }

    /// Switch to another scene tab (CueMol2 multi-scene `tabmolview`).
    fn switch_scene(&mut self, uid: cuemol_core::Uid) {
        self.state.set_active_scene(Some(uid));
        self.surface_mesh = None;
        self.density_solid_mesh = None;
        self.rebuild_after_load();
    }

    /// Create a new (empty) scene tab and switch to it.
    fn new_scene_tab(&mut self) {
        let _ = self.state.new_scene("Scene", true);
        self.surface_mesh = None;
        self.density_solid_mesh = None;
        self.rebuild_after_load();
    }

    /// Projected atom labels for the selected atoms at the current camera, in image
    /// pixels (== this viewport's CSS px, so they overlay 1:1). Empty unless shown.
    fn atom_labels(&self) -> Vec<cuemol_desktop_core::GuiAtomLabel> {
        if !self.show_labels {
            return Vec::new();
        }
        let (w, h) = self.view_size;
        let mvp = camera_mvp(&self.camera, w as f64 / h as f64);
        self.state
            .active_atom_labels(&self.selection_text, mvp, w as f32, h as f32, 80)
    }

    /// Filled density isosurface (solid mode) toggle. Marches the map once on enable.
    fn toggle_density_solid(&mut self) {
        self.show_density_solid = !self.show_density_solid;
        if self.show_density_solid && self.density_solid_mesh.is_none() {
            self.density_solid_mesh = self.state.build_density_surface_mesh(self.density_level);
            if self.density_solid_mesh.is_none() {
                self.show_density_solid = false;
                self.status = "No density map for a solid surface".into();
            }
        }
        self.rebuild_selection();
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
                    .build_surface_mesh(uid, &self.selection_text, 1.0, 1.4)
                {
                    Ok(mesh) => self.surface_mesh = Some(mesh),
                    Err(error) => {
                        self.show_surface = false;
                        self.status = format!("Surface failed: {error}");
                    }
                }
            }
        }
        self.rebuild_selection();
    }

    fn render_uri(&self) -> Option<String> {
        let renderer = self.renderer.as_ref()?;
        let mesh = self.mesh.as_ref()?;
        if mesh.indices.is_empty() {
            return None; // empty scene -> placeholder
        }
        let (w, h) = self.view_size;
        let mvp = camera_mvp(&self.camera, w as f64 / h as f64);
        let rgba = renderer.render(std::slice::from_ref(mesh), mvp, w, h, self.bg_clear)?;
        let png = encode_rgba_png(&rgba, w, h);
        Some(format!("data:image/png;base64,{}", base64_encode(&png)))
    }

    /// Resize the offscreen render to the live viewport size (CSS px). Clamped so
    /// a degenerate or maximised layout can't request a 0-px or oversized texture.
    /// Re-renders only when the size actually changed (the `onresize` observer can
    /// fire redundantly).
    fn set_view_size(&mut self, w: u32, h: u32) {
        let size = (w.clamp(64, MAX_VIEW_DIM), h.clamp(64, MAX_VIEW_DIM));
        if size != self.view_size {
            self.view_size = size;
            self.rerender();
        }
    }

    /// Load a molecule/scene file from the UI, reframing the camera and clearing
    /// any stale selection. Mirrors `DesktopAppState::open_path` + the post-load
    /// rebuild that `Model::new` performs.
    fn open_path(&mut self, path: PathBuf) {
        match self.state.open_path(&path) {
            Ok(_) => {
                self.selection_text.clear();
                self.status = format!("Loaded {}", path.display());
                self.rebuild_after_load();
            }
            Err(error) => {
                self.status = format!("Load error: {error}");
            }
        }
    }

    /// Pick the atom under a viewport click (CSS-px element coords) and select it.
    /// Projects every atom with the same `camera_mvp` the offscreen image used, so
    /// the pick matches what's drawn; on a hit, sets `selection_text` and rebuilds
    /// the mesh so the highlight shows. A miss leaves the selection unchanged.
    fn pick_at(&mut self, px: f64, py: f64) {
        let Ok(snapshot) = self.state.active_render_snapshot(&self.selection_text) else {
            return;
        };
        let (w, h) = self.view_size;
        let mvp = camera_mvp(&self.camera, w as f64 / h as f64);
        let Some((object_u64, atom_index)) = pick_atom_screen(
            &snapshot.atoms,
            mvp,
            (px, py),
            (w as f64, h as f64),
            PICK_RADIUS_PX,
        ) else {
            return;
        };
        let object_uid = cuemol_core::Uid::new(object_u64);
        let selection = self
            .state
            .atom_summary(object_uid, atom_index)
            .map(|atom| atom.selection_text)
            .unwrap_or_else(|_| format!("aid {atom_index}"));
        self.status = format!("Picked: {selection}");
        self.selection_text = selection;
        self.rebuild_selection();
    }

    /// Orbit the camera (left-drag): `dx`/`dy` are degrees about the Y/X axes.
    fn rotate_camera(&mut self, dx_deg: f64, dy_deg: f64) {
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

    /// Zoom the camera (wheel): `factor` > 1 zooms out, < 1 zooms in.
    fn zoom_camera(&mut self, factor: f64) {
        let zoom = (self.camera.zoom() * factor).clamp(0.5, 1.0e6);
        self.camera.set_zoom(zoom);
        self.rerender();
    }

    /// Pan the camera (right-drag): `dx`/`dy` are pixel deltas over the offscreen
    /// view height.
    fn pan_camera(&mut self, dx_pix: f64, dy_pix: f64) {
        self.camera
            .set_center(pan_center(&self.camera, dx_pix, dy_pix, self.view_size.1));
        self.rerender();
    }

    /// Re-frame the camera on the current scene's bounding box (CueMol2 View → Fit).
    fn fit_view(&mut self) {
        if let Some(snapshot) = self.current_render_snapshot() {
            self.camera = frame_camera(&snapshot);
            self.status = "View fitted".into();
            self.rerender();
        }
    }

    /// Toggle perspective vs orthographic projection (CueMol2 View → Projection).
    fn toggle_projection(&mut self) {
        let perspective = !self.camera.is_perspective();
        self.camera.set_perspective(perspective);
        self.status = if perspective {
            "Projection: perspective".into()
        } else {
            "Projection: orthographic".into()
        };
        self.rerender();
    }

    /// Flatten the scene/object/renderer hierarchy into display rows.
    fn tree_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        for scene in self.state.scene_summaries() {
            rows.push(TreeRow {
                depth: 0,
                mark: "▸".into(),
                name: scene.name.clone(),
                detail: format!("{} obj", scene.object_count),
                accent: true,
            });
            for object in self.state.object_summaries(scene.uid) {
                rows.push(TreeRow {
                    depth: 1,
                    mark: "◆".into(),
                    name: object.name.clone(),
                    detail: format!("{} atoms", object.atom_count),
                    accent: false,
                });
                for renderer in self.state.renderer_summaries(object.uid) {
                    rows.push(TreeRow {
                        depth: 2,
                        mark: if renderer.visible { "●" } else { "○" }.into(),
                        name: renderer.name.clone(),
                        detail: renderer.renderer_type.clone(),
                        accent: false,
                    });
                }
            }
        }
        rows
    }

    fn active_scene_name(&self) -> String {
        self.state
            .active_scene_summary()
            .map(|s| s.name)
            .unwrap_or_else(|| "(no scene)".to_string())
    }

    /// CueMol2 left-dock panels below the Workspace tree (MolStruct / Symmetry /
    /// Density map), as read-only titled line lists.
    fn panel_sections(&self) -> Vec<PanelSection> {
        let mut sections = Vec::new();

        // MolStruct: the chain → residue hierarchy (CueMol2 `molStructTree`), shared
        // with the egui/gpui front ends via `active_structure_tree`. Residue rows are
        // indented with an em-space (ASCII leading spaces collapse in HTML).
        let mut lines = Vec::new();
        for object in self.state.active_structure_tree() {
            for chain in &object.chains {
                let chain_id = if chain.chain_id.is_empty() {
                    "_"
                } else {
                    chain.chain_id.as_str()
                };
                lines.push(format!("Chain {chain_id} — {} residues", chain.residues.len()));
                for residue in &chain.residues {
                    let ins = residue.ins_code.map(String::from).unwrap_or_default();
                    lines.push(format!(
                        "\u{2003}{} {}{} · {} atoms",
                        residue.res_name.trim(),
                        residue.res_id,
                        ins,
                        residue.atoms.len()
                    ));
                }
            }
        }
        if lines.is_empty() {
            lines.push("No molecule loaded.".into());
        }
        sections.push(PanelSection {
            title: "MolStruct".into(),
            lines,
        });

        let mut lines = Vec::new();
        for cell in self.state.active_crystal_summaries() {
            let sg = cell.space_group_name.trim();
            lines.push(format!("SG {}", if sg.is_empty() { "(none)" } else { sg }));
            lines.push(format!("a/b/c {:.2} {:.2} {:.2}", cell.a, cell.b, cell.c));
            lines.push(format!(
                "α/β/γ {:.1} {:.1} {:.1}",
                cell.alpha, cell.beta, cell.gamma
            ));
        }
        if lines.is_empty() {
            lines.push("No crystal cell.".into());
        }
        sections.push(PanelSection {
            title: "Symmetry".into(),
            lines,
        });

        let mut lines = Vec::new();
        let maps = self
            .state
            .active_scene()
            .map(|scene| self.state.density_map_summaries(scene))
            .unwrap_or_default();
        for map in maps {
            lines.push(format!(
                "{} {}×{}×{}",
                map.name, map.counts[0], map.counts[1], map.counts[2]
            ));
        }
        if lines.is_empty() {
            lines.push("No density map.".into());
        }
        sections.push(PanelSection {
            title: "Density map".into(),
            lines,
        });

        sections
    }
}

/// The RCSB download URL for a PDB ID (lower-cased, like the C++/egui path).
fn rcsb_pdb_url(pdb_id: &str) -> String {
    format!(
        "https://files.rcsb.org/download/{}.pdb",
        pdb_id.trim().to_ascii_lowercase()
    )
}

/// Keep only filename-safe characters from a PDB ID for the temp path.
fn sanitize_pdb_id(pdb_id: &str) -> String {
    pdb_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

/// Build an auto-framed camera for a scene from its bounding box. (Local copy of
/// the gpui shell's helper; kept here so this crate stays self-contained and the
/// gpui WIP file is untouched.)
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

/// Panned camera centre for a screen drag of `(dx, dy)` px over a `vp_h`-px-tall
/// viewport. Same derivation as the gpui shell (orthographic, isotropic, screen-y
/// down).
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

/// Minimal standard base64 (RFC 4648, padded) — avoids pulling a crate just to
/// embed the PNG as a `data:` URI.
fn base64_encode(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(A[((n >> 18) & 63) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            A[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            A[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Root Dioxus component: owns the [`Model`] signal and renders the whole window.
/// Dropdown items `(label, action)` for the menu-bar entry at `index` (matches the
/// menu-name list in the RSX). Empty menus have no dropdown.
fn menu_items(index: usize) -> &'static [(&'static str, &'static str)] {
    match index {
        0 => &[("New scene tab", "new_scene")], // File
        1 => &[
            ("Undo", "undo"),
            ("Redo", "redo"),
            ("Clear undo data", "clear_undo"),
            ("Select all", "sel_all"),
            ("Clear selection", "sel_none"),
        ], // Edit
        2 => &[
            ("Element coloring", "col_element"),
            ("Rainbow coloring", "col_rainbow"),
            ("B-factor coloring", "col_bfactor"),
        ], // Render
        3 => &[("New tab", "new_scene"), ("Background (cycle)", "background")], // Scene
        4 => &[
            ("Fit view", "fit_view"),
            ("Toggle projection", "toggle_projection"),
            ("Toggle light/dark theme", "toggle_theme"),
            ("Surface", "surface"),
            ("Density contour", "den_contour"),
            ("Density solid", "den_solid"),
            ("H-bonds", "hbonds"),
            ("Labels", "labels"),
        ], // View
        5 => &[
            ("Generate surface", "surface"),
            ("Reassign secondary structure", "reassign_ss"),
        ], // Tools
        7 => &[("About", "about")],              // Help
        _ => &[],
    }
}

fn app() -> Element {
    let paths = use_context::<InitialPaths>();
    let mut model = use_signal(move || Model::new(paths.0.clone()));
    // The Open field's text, held separately from the model so per-keystroke edits
    // don't churn the wgpu-owning model signal.
    let mut open_text = use_signal(String::new);
    let mut traj_text = use_signal(String::new);
    // Save/export path field, held separately from the model like `open_text`.
    let mut save_text = use_signal(String::new);
    // Molecule-editing input fields (chain name, residue start), held outside the model.
    let mut edit_chain_text = use_signal(String::new);
    let mut edit_renum_text = use_signal(String::new);
    // Free-text selection expression (the full selection grammar, beyond quick-selects).
    let mut sel_input_text = use_signal(String::new);
    // PDB ID for the Get-PDB (RCSB download) field.
    let mut pdb_id_text = use_signal(String::new);

    // Animation playback loop (CueMol2 SimpleSpin): an executor-agnostic timer ticks
    // ~60 fps; while `anim_playing` is set, each tick spins the camera + re-renders.
    use_future(move || async move {
        loop {
            if model.read().anim_playing {
                let mut m = model.write();
                let frame = cuemol_desktop_core::anim_step(
                    &m.animation_objects,
                    m.animation_elapsed,
                    0.016,
                );
                m.animation_elapsed = frame.elapsed;
                if frame.rotate_dir != 0.0 {
                    m.rotate_camera(frame.rotate_dir * 2.0, 0.0);
                }
                if frame.slide_dir != 0.0 {
                    m.zoom_camera(1.0 + frame.slide_dir * 0.02);
                }
                if let Some(target) = frame.set_visible {
                    m.apply_anim_visibility(target);
                }
                if let Some(fraction) = frame.frame_fraction {
                    if let Some(uid) = m.state.active_object_summaries().first().map(|o| o.uid) {
                        if m.state.object_has_trajectory(uid) {
                            m.state.apply_trajectory_fraction(uid, fraction);
                            m.rebuild_selection();
                        }
                    }
                }
            }
            futures_timer::Delay::new(std::time::Duration::from_millis(16)).await;
        }
    });

    // Derive view data under a short read borrow, then release it before RSX so
    // the event handlers (which write the signal) never alias the read guard.
    let (status, scene_name, image_uri, rows, sections, seq_chains, atom_labels, scenes, active_scene) = {
        let m = model.read();
        (
            m.status.clone(),
            m.active_scene_name(),
            m.image_uri.clone(),
            m.tree_rows(),
            m.panel_sections(),
            m.state.active_sequence_view(&m.selection_text),
            m.atom_labels(),
            m.state.scene_summaries(),
            m.state.active_scene(),
        )
    };
    let menu_open = model.read().menu_open;
    let anim_playing = model.read().anim_playing;
    let theme_light = model.read().theme_light;
    let anim_objects: Vec<(String, f64)> = model
        .read()
        .animation_objects
        .iter()
        .map(|object| (object.kind.label().to_string(), object.duration))
        .collect();
    let measure = {
        let m = model.read();
        m.state.active_measure(&m.selection_text)
    };
    // Active scene's objects (target list for "Add renderer", per-object so a
    // multi-molecule scene never silently attaches to the wrong one).
    let objects: Vec<(cuemol_core::Uid, String)> = model
        .read()
        .state
        .active_object_summaries()
        .iter()
        .map(|object| (object.uid, object.name.clone()))
        .collect();
    // Renderers of the active scene's objects + their editable bond/atom radii + the
    // live visibility flag (drives the per-row show/hide toggle).
    let renderers: Vec<(cuemol_core::Uid, cuemol_core::Uid, String, f64, f64, bool)> = {
        let m = model.read();
        let mut out = Vec::new();
        for object in m.state.active_object_summaries() {
            for renderer in m.state.renderer_summaries(object.uid) {
                let bondw = m
                    .state
                    .renderer_property(object.uid, renderer.uid, "bondw")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.18);
                let sphr = m
                    .state
                    .renderer_property(object.uid, renderer.uid, "sphr")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.30);
                out.push((
                    object.uid,
                    renderer.uid,
                    renderer.name,
                    bondw,
                    sphr,
                    renderer.visible,
                ));
            }
        }
        out
    };

    let root_style = format!(
        "{}display:flex;flex-direction:column;height:100vh;width:100vw;margin:0;\
         background:{COL_BG};color:{COL_TEXT};font:13px -apple-system,system-ui,sans-serif;\
         overflow:hidden;user-select:none;",
        theme_vars(theme_light)
    );
    let header_style = format!(
        // flex-wrap + min-height so the file/fetch controls wrap to a second row on a
        // narrow window instead of pushing Get-PDB off-screen (no fixed height clip).
        "display:flex;align-items:center;flex-wrap:wrap;gap:8px 12px;min-height:40px;\
         padding:4px 16px;background:{COL_HEADER};border-bottom:1px solid {COL_BORDER};\
         flex:0 0 auto;"
    );
    let open_input_style = format!(
        "margin-left:auto;width:220px;height:24px;padding:0 8px;border-radius:4px;\
         border:1px solid {COL_BORDER};background:{COL_BG};color:{COL_TEXT};\
         font:12px -apple-system,system-ui,sans-serif;outline:none;"
    );
    let open_btn_style = format!(
        "height:26px;padding:0 12px;border-radius:4px;border:1px solid {COL_BORDER};\
         background:{COL_PANEL};color:{COL_TEXT};cursor:pointer;font-size:12px;"
    );
    let sidebar_style = format!(
        "display:flex;flex-direction:column;width:300px;flex:0 0 auto;\
         background:{COL_PANEL};border-right:1px solid {COL_BORDER};"
    );
    let panel_title_style = format!(
        "padding:8px 12px;background:{COL_HEADER};border-bottom:1px solid {COL_BORDER};\
         color:{COL_TEXT_DIM};font-size:11px;flex:0 0 auto;"
    );
    let viewport_style = format!(
        "position:relative;display:flex;flex:1;min-width:0;align-items:center;\
         justify-content:center;background:{COL_VIEWPORT};overflow:hidden;cursor:grab;"
    );
    let status_style = format!(
        "display:flex;align-items:center;height:26px;padding:0 16px;background:{COL_HEADER};\
         border-top:1px solid {COL_BORDER};font-size:11px;color:{COL_TEXT_DIM};flex:0 0 auto;"
    );
    let menubar_style = format!(
        "display:flex;align-items:center;gap:4px;height:28px;padding:0 12px;background:{COL_PANEL};\
         border-bottom:1px solid {COL_BORDER};flex:0 0 auto;font-size:12px;"
    );
    let section_header_style = format!(
        "padding:6px 12px 2px;color:{COL_ACCENT};font-size:11px;\
         border-bottom:1px solid {COL_BORDER};margin-top:6px;"
    );
    let section_line_style = format!("padding:2px 12px;color:{COL_TEXT};font-size:11px;");
    let color_btn_style = format!(
        "height:22px;padding:0 8px;border-radius:4px;border:1px solid {COL_BORDER};\
         background:{COL_BG};color:{COL_TEXT};cursor:pointer;font-size:11px;"
    );
    let seq_cell_style = format!(
        "display:inline-block;width:13px;text-align:center;font-family:monospace;\
         font-size:12px;cursor:pointer;color:{COL_TEXT};"
    );
    let edit_input_style = format!(
        "height:20px;padding:0 6px;border-radius:4px;border:1px solid {COL_BORDER};\
         background:{COL_BG};color:{COL_TEXT};font-size:11px;outline:none;"
    );
    let seq_cell_sel_style = format!(
        "display:inline-block;width:13px;text-align:center;font-family:monospace;\
         font-size:12px;cursor:pointer;color:#fff;background:{COL_ACCENT};"
    );

    rsx! {
        div { style: "{root_style}",
            // Header.
            div { style: "{header_style}",
                span { style: "color:{COL_ACCENT};", "CueMol2-rs" }
                span { style: "color:{COL_TEXT_DIM};", "·" }
                span { "scene: {scene_name}" }
                input {
                    style: "{open_input_style}",
                    placeholder: "open file: path to .pdb / .cif / .qsc …",
                    oninput: move |e| open_text.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter {
                            let path = open_text.peek().trim().to_string();
                            if !path.is_empty() {
                                model.write().open_path(PathBuf::from(path));
                            }
                        }
                    },
                }
                button {
                    style: "{open_btn_style}",
                    onclick: move |_| {
                        let path = open_text.peek().trim().to_string();
                        if !path.is_empty() {
                            model.write().open_path(PathBuf::from(path));
                        }
                    },
                    "Open"
                }
                input {
                    style: "{open_input_style}",
                    placeholder: "save path: .qsc / .pdb / .sdf / .pov …",
                    oninput: move |e| save_text.set(e.value()),
                }
                button {
                    style: "{open_btn_style}",
                    title: "Save the active scene as .qsc",
                    onclick: move |_| {
                        let path = save_text.peek().trim().to_string();
                        model.write().save_scene(&path);
                    },
                    "Save scene"
                }
                button {
                    style: "{open_btn_style}",
                    title: "Save the first molecule as .pdb / .sdf",
                    onclick: move |_| {
                        let path = save_text.peek().trim().to_string();
                        model.write().save_object(&path);
                    },
                    "Save obj"
                }
                button {
                    style: "{open_btn_style}",
                    title: "Export the scene (current selection) to POV-Ray .pov",
                    onclick: move |_| {
                        let path = save_text.peek().trim().to_string();
                        model.write().export_scene(&path);
                    },
                    "Export"
                }
                input {
                    style: "width:64px;{open_btn_style}",
                    placeholder: "PDB ID",
                    oninput: move |e| pdb_id_text.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter {
                            let id = pdb_id_text.peek().trim().to_string();
                            model.write().get_pdb(&id);
                        }
                    },
                }
                button {
                    style: "{open_btn_style}",
                    title: "Download a structure from RCSB by PDB ID",
                    onclick: move |_| {
                        let id = pdb_id_text.peek().trim().to_string();
                        model.write().get_pdb(&id);
                    },
                    "Get PDB"
                }
                span { style: "color:{COL_TEXT_DIM};margin-left:12px;", "Dioxus" }
            }
            // Menu bar (CueMol2 cuemol2-menus.xul) — functional dropdowns.
            div { style: "position:relative;{menubar_style}",
                for (mi , menu) in ["File", "Edit", "Render", "Scene", "View", "Tools", "Window", "Help"].iter().enumerate() {
                    div { key: "menu-{mi}", style: "position:relative;",
                        span {
                            style: if menu_open == Some(mi) {
                                format!("padding:2px 8px;border-radius:4px;cursor:pointer;background:{COL_PANEL};color:{COL_TEXT};")
                            } else {
                                format!("padding:2px 8px;border-radius:4px;cursor:pointer;color:{COL_TEXT};")
                            },
                            onclick: move |_| {
                                let mut m = model.write();
                                m.menu_open = if m.menu_open == Some(mi) { None } else { Some(mi) };
                            },
                            "{menu}"
                        }
                        if menu_open == Some(mi) && !menu_items(mi).is_empty() {
                            div {
                                style: "position:absolute;top:100%;left:0;z-index:20;min-width:160px;\
                                        background:{COL_PANEL};border:1px solid {COL_BORDER};border-radius:4px;\
                                        box-shadow:0 4px 12px rgba(0,0,0,0.4);padding:4px 0;",
                                for (ii , item) in menu_items(mi).iter().enumerate() {
                                    div {
                                        key: "mi-{mi}-{ii}",
                                        style: "padding:4px 14px;cursor:pointer;font-size:12px;color:{COL_TEXT};white-space:nowrap;",
                                        onclick: {
                                            let action = item.1;
                                            move |_| model.write().run_menu_action(action)
                                        },
                                        "{item.0}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Scene tabs (CueMol2 multi-scene tabmolview): one tab per scene + "+".
            div {
                style: "display:flex;align-items:center;gap:4px;height:26px;padding:0 12px;\
                        background:{COL_HEADER};border-bottom:1px solid {COL_BORDER};flex:0 0 auto;",
                for scene in scenes.iter() {
                    span {
                        key: "scenetab-{scene.uid.get()}",
                        style: if Some(scene.uid) == active_scene {
                            format!("padding:2px 8px;border-radius:4px;cursor:pointer;font-size:12px;background:{COL_PANEL};color:{COL_ACCENT};")
                        } else {
                            format!("padding:2px 8px;border-radius:4px;cursor:pointer;font-size:12px;color:{COL_TEXT_DIM};")
                        },
                        onclick: {
                            let uid = scene.uid;
                            move |_| model.write().switch_scene(uid)
                        },
                        "{scene.name}"
                    }
                }
                span {
                    style: "padding:2px 8px;border-radius:4px;cursor:pointer;font-size:12px;color:{COL_TEXT};",
                    onclick: move |_| model.write().new_scene_tab(),
                    "+"
                }
            }
            // Body: sidebar + viewport.
            div { style: "display:flex;flex:1;min-height:0;",
                div { style: "{sidebar_style}",
                    div { style: "{panel_title_style}", "Workspace" }
                    div { style: "flex:1;min-height:0;overflow-y:auto;",
                        if rows.is_empty() {
                            div { style: "padding:12px;color:{COL_TEXT_DIM};",
                                "No objects loaded. Type a path above and press Open."
                            }
                        }
                        for (i, row) in rows.iter().enumerate() {
                            TreeRowView { key: "{i}", row: row.clone() }
                        }
                        // CueMol2 left-dock panels below the Workspace tree.
                        for (si, sec) in sections.iter().enumerate() {
                            div { key: "sec-{si}", style: "{section_header_style}", "{sec.title}" }
                            for (li, line) in sec.lines.iter().enumerate() {
                                div { key: "sec-{si}-l{li}", style: "{section_line_style}", "{line}" }
                            }
                        }
                        // Coloring panel (CueMol2 coloring-panel.xul): object-level scheme.
                        div { style: "{section_header_style}", "Coloring" }
                        div { style: "display:flex;gap:4px;padding:4px 12px;flex-wrap:wrap;",
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().color_element(),
                                "Element"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().color_rainbow(),
                                "Rainbow"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().color_bfactor(),
                                "B-factor"
                            }
                        }
                        // Symmetry actions (CueMol2 symmetry-panel.xul): cell box + mates.
                        div { style: "{section_header_style}", "Symmetry" }
                        div { style: "display:flex;gap:4px;padding:4px 12px;flex-wrap:wrap;",
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().toggle_unit_cell(),
                                "Unit cell"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().toggle_symm_mates(),
                                "Symm mol"
                            }
                        }
                        // Selection: a free-text expression (full grammar — the Command
                        // tab) + the quick-selects (the Editor tab) (CueMol2 selection-panel.xul).
                        div { style: "{section_header_style}", "Selection" }
                        div { style: "display:flex;gap:4px;align-items:center;padding:4px 12px;flex-wrap:wrap;",
                            input {
                                style: "flex:1;min-width:120px;{edit_input_style}",
                                placeholder: "e.g. resid 1:10 and bymainch *",
                                value: "{sel_input_text}",
                                oninput: move |e| sel_input_text.set(e.value()),
                                onkeydown: move |e| {
                                    if e.key() == Key::Enter {
                                        let expr = sel_input_text.peek().trim().to_string();
                                        if !expr.is_empty() {
                                            model.write().set_selection(&expr);
                                        }
                                    }
                                },
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| {
                                    let expr = sel_input_text.peek().trim().to_string();
                                    if !expr.is_empty() {
                                        model.write().set_selection(&expr);
                                    }
                                },
                                "Apply"
                            }
                        }
                        div { style: "display:flex;gap:4px;padding:4px 12px;flex-wrap:wrap;",
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().set_selection("*"),
                                "All"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().set_selection("none"),
                                "None"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().set_selection("bymainch *"),
                                "Backbone"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().set_selection("bysidech *"),
                                "Sidechain"
                            }
                        }
                        // Molecule editing (CueMol2 Edit menu): acts on the current selection
                        // of the first molecule (change chain / renumber / delete atoms).
                        div { style: "{section_header_style}", "Edit" }
                        div { style: "display:flex;gap:4px;padding:4px 12px;flex-wrap:wrap;",
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().undo(),
                                "Undo"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().redo(),
                                "Redo"
                            }
                        }
                        div { style: "display:flex;gap:4px;align-items:center;padding:4px 12px;flex-wrap:wrap;",
                            input {
                                style: "width:52px;{edit_input_style}",
                                placeholder: "chain",
                                oninput: move |e| edit_chain_text.set(e.value()),
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| {
                                    let chain = edit_chain_text.peek().trim().to_string();
                                    model.write().change_chain(&chain);
                                },
                                "Set chain"
                            }
                        }
                        div { style: "display:flex;gap:4px;align-items:center;padding:4px 12px;flex-wrap:wrap;",
                            input {
                                style: "width:52px;{edit_input_style}",
                                placeholder: "start#",
                                oninput: move |e| edit_renum_text.set(e.value()),
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| {
                                    match edit_renum_text.peek().trim().parse::<i32>() {
                                        Ok(start) => model.write().renumber_residues(start),
                                        Err(_) => {
                                            model.write().status = "Renumber: enter an integer".into()
                                        }
                                    }
                                },
                                "Renumber"
                            }
                            button {
                                style: "{color_btn_style}",
                                title: "Delete the atoms in the current selection",
                                onclick: move |_| model.write().delete_selected_atoms(),
                                "Delete sel"
                            }
                        }
                        div { style: "display:flex;gap:4px;padding:4px 12px;flex-wrap:wrap;",
                            button {
                                style: "{color_btn_style}",
                                title: "Recompute secondary structure (DSSP)",
                                onclick: move |_| model.write().reassign_secondary(),
                                "Reassign SS"
                            }
                        }
                        // Measure (CueMol2 AtomIntr): distance/angle/torsion of selection.
                        div { style: "{section_header_style}", "Measure" }
                        div { style: "{section_line_style}",
                            {measure.clone().unwrap_or_else(|| "Select 2 / 3 / 4 atoms".to_string())}
                        }
                        // Renderer management: create-by-type / visibility / delete /
                        // geometry editing (CueMol2 Render menu + renderer property dialog).
                        if !objects.is_empty() {
                            div { style: "{section_header_style}", "Renderer" }
                            // Add a renderer of a chosen representation, PER OBJECT so a
                            // multi-molecule scene attaches to the intended one.
                            for (oi , (obj_uid , obj_name)) in objects.iter().enumerate() {
                                div { key: "rend-add-{oi}",
                                    div { style: "{section_line_style}", "Add to {obj_name}:" }
                                    div { style: "display:flex;gap:4px;padding:2px 12px;flex-wrap:wrap;",
                                        for (ty , label) in RENDERER_TYPES.iter() {
                                            button {
                                                key: "{ty}",
                                                style: "{color_btn_style}",
                                                onclick: { let (o, t) = (*obj_uid, *ty); move |_| model.write().add_renderer(o, t) },
                                                "{label}"
                                            }
                                        }
                                    }
                                }
                            }
                            for (ri , (obj , rend , name , bondw , sphr , visible)) in renderers.iter().enumerate() {
                                div { key: "rend-{ri}",
                                    div { style: "display:flex;gap:4px;align-items:center;padding:2px 12px;",
                                        span { style: "flex:1;", "{name}" }
                                        button {
                                            style: "{color_btn_style}",
                                            title: "Toggle visibility",
                                            onclick: { let (o, r, v) = (*obj, *rend, *visible); move |_| model.write().toggle_renderer_visible(o, r, v) },
                                            {if *visible { "●" } else { "○" }}
                                        }
                                        button {
                                            style: "{color_btn_style}",
                                            title: "Delete renderer",
                                            onclick: { let (o, r) = (*obj, *rend); move |_| model.write().delete_renderer(o, r) },
                                            "✕"
                                        }
                                    }
                                    div { style: "display:flex;gap:4px;align-items:center;padding:2px 12px;",
                                        span { style: "font-size:11px;color:{COL_TEXT_DIM};width:70px;", "Bond {bondw:.2}" }
                                        button {
                                            style: "{color_btn_style}",
                                            onclick: { let (o, r) = (*obj, *rend); move |_| model.write().adjust_renderer_property(o, r, "bondw", -0.02) },
                                            "−"
                                        }
                                        button {
                                            style: "{color_btn_style}",
                                            onclick: { let (o, r) = (*obj, *rend); move |_| model.write().adjust_renderer_property(o, r, "bondw", 0.02) },
                                            "+"
                                        }
                                    }
                                    div { style: "display:flex;gap:4px;align-items:center;padding:2px 12px;",
                                        span { style: "font-size:11px;color:{COL_TEXT_DIM};width:70px;", "Atom {sphr:.2}" }
                                        button {
                                            style: "{color_btn_style}",
                                            onclick: { let (o, r) = (*obj, *rend); move |_| model.write().adjust_renderer_property(o, r, "sphr", -0.05) },
                                            "−"
                                        }
                                        button {
                                            style: "{color_btn_style}",
                                            onclick: { let (o, r) = (*obj, *rend); move |_| model.write().adjust_renderer_property(o, r, "sphr", 0.05) },
                                            "+"
                                        }
                                    }
                                }
                            }
                        }
                        // Density map chickenwire contour (CueMol2 densitymap-panel.xul).
                        div { style: "{section_header_style}", "Density map" }
                        div { style: "display:flex;gap:4px;padding:4px 12px;flex-wrap:wrap;",
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().toggle_density_contour(),
                                "Show contour"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().toggle_density_solid(),
                                "Solid"
                            }
                        }
                        // Molecular surface (CueMol2 molsurf renderer display).
                        div { style: "{section_header_style}", "Surface" }
                        div { style: "display:flex;gap:4px;padding:4px 12px;flex-wrap:wrap;",
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().toggle_surface(),
                                "Show surface"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().toggle_hbonds(),
                                "H-bonds"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().toggle_labels(),
                                "Labels"
                            }
                        }
                        // Animation timeline (CueMol2 anim-panel): Add Spin/Rock/Pause + Play.
                        div { style: "{section_header_style}", "Animation" }
                        div { style: "display:flex;gap:4px;padding:4px 12px;flex-wrap:wrap;",
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().add_anim(cuemol_desktop_core::AnimKind::Spin),
                                "+ Spin"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().add_anim(cuemol_desktop_core::AnimKind::Rock),
                                "+ Rock"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().add_anim(cuemol_desktop_core::AnimKind::Pause),
                                "+ Pause"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().add_anim(cuemol_desktop_core::AnimKind::SlideIn),
                                "+ SlideIn"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().add_anim(cuemol_desktop_core::AnimKind::SlideOut),
                                "+ SlideOut"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().add_anim(cuemol_desktop_core::AnimKind::ShowAnim),
                                "+ Show"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().add_anim(cuemol_desktop_core::AnimKind::HideAnim),
                                "+ Hide"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().add_anim(cuemol_desktop_core::AnimKind::MolAnim),
                                "+ MolAnim"
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| model.write().toggle_animation(),
                                if anim_playing { "⏸ Stop" } else { "▶ Play" }
                            }
                        }
                        // MolAnim source: load an AMBER mdcrd trajectory onto the active object.
                        div { style: "display:flex;gap:4px;padding:4px 12px;align-items:center;",
                            input {
                                style: "flex:1;min-width:0;background:{COL_BG};color:{COL_TEXT};border:1px solid {COL_BORDER};border-radius:4px;padding:2px 6px;font-size:11px;",
                                placeholder: "trajectory.mdcrd",
                                value: "{traj_text}",
                                oninput: move |e| traj_text.set(e.value()),
                            }
                            button {
                                style: "{color_btn_style}",
                                onclick: move |_| {
                                    let path = traj_text.peek().trim().to_string();
                                    if !path.is_empty() {
                                        model.write().load_trajectory(&path);
                                    }
                                },
                                "Load traj"
                            }
                        }
                        for (ai , (label , duration)) in anim_objects.iter().enumerate() {
                            div {
                                key: "anim-{ai}",
                                style: "display:flex;gap:6px;align-items:center;padding:2px 12px;font-size:11px;",
                                span { style: "color:{COL_TEXT};width:120px;", "{ai + 1}. {label} ({duration:.1}s)" }
                                button {
                                    style: "{color_btn_style}",
                                    title: "Move up",
                                    onclick: { let i = ai; move |_| model.write().move_anim(i, -1) },
                                    "▲"
                                }
                                button {
                                    style: "{color_btn_style}",
                                    title: "Move down",
                                    onclick: { let i = ai; move |_| model.write().move_anim(i, 1) },
                                    "▼"
                                }
                                button {
                                    style: "{color_btn_style}",
                                    onclick: { let i = ai; move |_| model.write().remove_anim(i) },
                                    "Delete"
                                }
                            }
                        }
                        // Interactive sequence viewer (CueMol2 seqpanel): clickable
                        // 1-letter cells, selected residues highlighted.
                        if !seq_chains.is_empty() {
                            div { style: "{section_header_style}", "Sequence" }
                            for (ci , chain) in seq_chains.iter().enumerate() {
                                div { key: "seqc-{ci}",
                                    div {
                                        style: "padding:2px 12px 0;color:{COL_TEXT_DIM};font-size:10px;",
                                        "chain {chain.chain_id} ({chain.residues.len()})"
                                    }
                                    div { style: "padding:0 12px 4px;line-height:1.5;",
                                        for (ri , residue) in chain.residues.iter().enumerate() {
                                            span {
                                                key: "seq-{ci}-{ri}",
                                                style: if residue.selected { "{seq_cell_sel_style}" } else { "{seq_cell_style}" },
                                                title: "{residue.res_name} {residue.res_id}",
                                                onclick: {
                                                    let sel = residue.selection.clone();
                                                    move |_| model.write().set_selection(&sel)
                                                },
                                                "{residue.letter}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // 3D viewport: scroll to zoom, left-drag to orbit, right-drag to
                // pan, left-click (no drag) to pick an atom. The offscreen image
                // tracks this element's size via `onresize`.
                div {
                    style: "{viewport_style}",
                    onmounted: move |e| {
                        // Fetch the initial viewport size explicitly. `onresize`
                        // (ResizeObserver) *should* also fire on mount, but the
                        // first rendered frame AND the first atom pick depend on
                        // `view_size` being the real element size — not the
                        // 960x720 default — so don't bet the first frame on the
                        // observer firing in time.
                        spawn(async move {
                            if let Ok(rect) = e.get_client_rect().await {
                                model
                                    .write()
                                    .set_view_size(rect.size.width as u32, rect.size.height as u32);
                            }
                        });
                    },
                    onresize: move |e| {
                        if let Ok(size) = e.get_border_box_size() {
                            model.write().set_view_size(size.width as u32, size.height as u32);
                        }
                    },
                    onmousedown: move |e: MouseEvent| {
                        let kind = match e.trigger_button() {
                            Some(MouseButton::Secondary) => DragKind::Pan,
                            _ => DragKind::Orbit,
                        };
                        let p = e.element_coordinates();
                        model.write().drag = Some(DragState {
                            kind,
                            last_x: p.x,
                            last_y: p.y,
                            moved: 0.0,
                        });
                    },
                    onmousemove: move |e: MouseEvent| {
                        let Some(state) = model.read().drag else { return };
                        // The triggering button must still be held; otherwise a
                        // mouseup was missed off-viewport — cancel the gesture.
                        let still_held = match state.kind {
                            DragKind::Orbit => e.held_buttons().contains(MouseButton::Primary),
                            DragKind::Pan => e.held_buttons().contains(MouseButton::Secondary),
                        };
                        if !still_held {
                            model.write().drag = None;
                            return;
                        }
                        let p = e.element_coordinates();
                        let (dx, dy) = (p.x - state.last_x, p.y - state.last_y);
                        let mut m = model.write();
                        m.drag = Some(DragState {
                            kind: state.kind,
                            last_x: p.x,
                            last_y: p.y,
                            moved: state.moved + dx.hypot(dy),
                        });
                        match state.kind {
                            DragKind::Orbit => m.rotate_camera(dx * 0.5, dy * 0.5),
                            DragKind::Pan => m.pan_camera(dx, dy),
                        }
                    },
                    onmouseup: move |e: MouseEvent| {
                        let finished = model.write().drag.take();
                        if let Some(state) = finished {
                            // A left release that barely moved is a pick, not an orbit.
                            if state.kind == DragKind::Orbit && state.moved < CLICK_MAX_MOVE {
                                let p = e.element_coordinates();
                                model.write().pick_at(p.x, p.y);
                            }
                        }
                    },
                    onwheel: move |e: WheelEvent| {
                        let dy = e.delta().strip_units().y;
                        if dy != 0.0 {
                            model.write().zoom_camera(if dy < 0.0 { 0.9 } else { 1.1 });
                        }
                    },
                    if let Some(uri) = image_uri {
                        img {
                            src: "{uri}",
                            style: "width:100%;height:100%;object-fit:contain;",
                            draggable: "false",
                        }
                    } else {
                        div { style: "color:{COL_TEXT_DIM};padding:8px;text-align:center;",
                            "3D viewport (wgpu) — load a molecule (or no GPU adapter available)"
                        }
                    }
                    // Atom labels overlaid at projected positions (CueMol2 atom labels).
                    for (li , label) in atom_labels.iter().enumerate() {
                        div {
                            key: "lbl-{li}",
                            style: "position:absolute;left:{label.x}px;top:{label.y}px;\
                                    transform:translateY(-100%);font-size:11px;color:#eaf0f8;\
                                    pointer-events:none;white-space:nowrap;text-shadow:0 0 2px #000;",
                            "{label.text}"
                        }
                    }
                }
            }
            // Status bar.
            div { style: "{status_style}", "{status}" }
        }
    }
}

/// One indented tree row (its own component so the `key` + per-row styling stay
/// tidy in the parent RSX).
#[component]
fn TreeRowView(row: TreeRow) -> Element {
    let name_color = if row.accent { COL_ACCENT } else { COL_TEXT };
    let pad_left = 12 + row.depth * 16;
    let row_style = format!(
        "display:flex;align-items:center;gap:8px;padding:4px 12px 4px {pad_left}px;white-space:nowrap;"
    );
    rsx! {
        div { style: "{row_style}",
            span { style: "color:{COL_TEXT_DIM};", "{row.mark}" }
            span { style: "color:{name_color};", "{row.name}" }
            span { style: "color:{COL_TEXT_DIM};font-size:11px;", "{row.detail}" }
        }
    }
}

/// Launch the Dioxus desktop GUI. `open_paths` are molecule files to load on
/// start; they reach the root component through Dioxus context.
pub fn run(open_paths: Vec<PathBuf>) {
    LaunchBuilder::desktop()
        .with_context(InitialPaths(open_paths))
        .launch(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    // The non-rendering logic (model + summaries + camera + picking) is GPU-free
    // testable, exactly like the gpui shell's headless tests; only `app()`/`run()`
    // need a webview. (`Model::new` may build a wgpu renderer if an adapter
    // exists, but `rerender` self-skips to `None` on an empty scene, so this runs
    // in CI.)

    /// Project an atom centre to a viewport pixel with the same column-major mvp
    /// the offscreen renderer + `pick_atom_screen` use (image row 0 = top).
    fn project_px(center: &cuemol_core::Vector4, mvp: [[f32; 4]; 4], vp: (f64, f64)) -> (f64, f64) {
        let v = [center.x as f32, center.y as f32, center.z as f32, 1.0];
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
    fn model_builds_with_one_active_scene() {
        let model = Model::new(Vec::new());
        assert_eq!(model.status, "Ready");
        let scenes = model.state.scene_summaries();
        assert_eq!(scenes.len(), 1, "ensure_active_scene should make one scene");
        assert_eq!(model.state.object_summaries(scenes[0].uid).len(), 0);
        // Empty scene -> no composited image, just the placeholder.
        assert!(model.image_uri.is_none());
        assert!(!model.tree_rows().is_empty());
        assert_eq!(model.view_size, (INIT_VIEW_W, INIT_VIEW_H));
    }

    #[test]
    fn camera_controls_mutate_the_view_camera() {
        let mut model = Model::new(Vec::new());
        let zoom0 = model.camera.zoom();
        model.zoom_camera(2.0);
        assert!((model.camera.zoom() - zoom0 * 2.0).abs() < 1.0e-6);

        let rot0 = model.camera.rotation();
        model.rotate_camera(20.0, 10.0);
        let rot1 = model.camera.rotation();
        let delta = (rot1.x - rot0.x).abs()
            + (rot1.y - rot0.y).abs()
            + (rot1.z - rot0.z).abs()
            + (rot1.w - rot0.w).abs();
        assert!(delta > 1.0e-3, "rotation should change: delta={delta}");
    }

    #[test]
    fn set_view_size_updates_and_clamps_the_offscreen_resolution() {
        let mut model = Model::new(Vec::new());
        model.set_view_size(1280, 400);
        assert_eq!(model.view_size, (1280, 400));
        // Degenerate / oversized layout requests are clamped, never panic.
        model.set_view_size(0, 999_999);
        assert_eq!(model.view_size, (64, MAX_VIEW_DIM));
    }

    /// Resolve the 1CRN reference fixture from `CUEMOL2_REF_DIR` or the default
    /// sibling `../cuemol2` tree (relative to the repo root, NOT the test CWD —
    /// `cargo test` runs from the crate dir). `None` when the tree is absent, so
    /// the test self-skips exactly like the workspace's `*_compat` tests.
    fn ref_1crn() -> Option<PathBuf> {
        let base = std::env::var_os("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let pdb = base.join("tests/test_data/1CRN.pdb");
        pdb.exists().then_some(pdb)
    }

    #[test]
    fn loading_1crn_tessellates_to_triangles_and_composites_an_image() {
        // Full headless exercise of the viewport pipeline: load a real molecule,
        // tessellate to triangles, render offscreen, and PNG-encode into a data
        // URI. GPU-free up to the render; the image step self-skips with no
        // adapter (CI), exactly like the core offscreen tests.
        let Some(pdb) = ref_1crn() else { return };
        let model = Model::new(vec![pdb]);
        assert!(
            model.status.starts_with("Loaded"),
            "status: {}",
            model.status
        );
        let tris = model
            .mesh
            .as_ref()
            .map(|m| m.indices.len() / 3)
            .unwrap_or(0);
        assert!(tris > 0, "1CRN should tessellate to triangles, got {tris}");
        // With a GPU adapter the offscreen render must yield a composited image;
        // without one (headless CI) both are legitimately absent.
        if model.renderer.is_some() {
            let uri = model.image_uri.as_deref().unwrap_or("");
            assert!(
                uri.starts_with("data:image/png;base64,"),
                "expected a PNG data URI, got {} chars",
                uri.len()
            );
        }
    }

    #[test]
    fn molstruct_section_shows_the_chain_residue_hierarchy() {
        // The MolStruct panel descends chain → residue (shared `active_structure_tree`),
        // not just a per-chain summary, so it carries per-residue rows.
        let Some(pdb) = ref_1crn() else { return };
        let model = Model::new(vec![pdb]);
        let sections = model.panel_sections();
        let molstruct = sections
            .iter()
            .find(|section| section.title == "MolStruct")
            .expect("MolStruct section present");
        assert!(
            molstruct.lines.len() > 40,
            "expected per-residue rows under the chain, got {} lines",
            molstruct.lines.len()
        );
        assert!(
            molstruct
                .lines
                .iter()
                .any(|line| line.contains("THR 1") && line.contains("atoms")),
            "expected a THR 1 residue row (1CRN starts with THR 1)"
        );
    }

    #[test]
    fn renderer_create_toggle_delete_round_trips_in_dioxus() {
        // Render-menu parity with egui/gpui: the dioxus front end can create a
        // representation by type, flip its visibility, and delete it — all through the
        // shared-core API (`new_renderer_in_active_scene`/`set_renderer_visible`/
        // `destroy_renderer_in_object`). Mirrors gpui's
        // `create_then_delete_renderer_round_trips`. GPU-free (rebuild_selection's
        // offscreen render self-skips with no adapter).
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        let object = model
            .state
            .active_object_summaries()
            .first()
            .expect("1CRN loads one object")
            .uid;
        let count = |model: &Model| model.state.renderer_summaries(object).len();
        let cartoons = |model: &Model| {
            model
                .state
                .renderer_summaries(object)
                .into_iter()
                .filter(|renderer| renderer.renderer_type == "cartoon")
                .count()
        };
        let before = count(&model);
        assert_eq!(cartoons(&model), 0, "no cartoon renderer before add");

        // Create a cartoon representation.
        model.add_renderer(object, "cartoon");
        assert_eq!(count(&model), before + 1, "add_renderer adds one renderer");
        assert_eq!(cartoons(&model), 1, "the new renderer is a cartoon");

        // Toggle visibility (created visible → hidden).
        let renderer = model
            .state
            .renderer_summaries(object)
            .into_iter()
            .find(|renderer| renderer.renderer_type == "cartoon")
            .expect("cartoon renderer present");
        assert!(renderer.visible, "a freshly added renderer is visible");
        model.toggle_renderer_visible(object, renderer.uid, renderer.visible);
        let hidden = model
            .state
            .renderer_summaries(object)
            .into_iter()
            .find(|candidate| candidate.uid == renderer.uid)
            .expect("renderer still present after toggle");
        assert!(!hidden.visible, "toggle hides the renderer");

        // Delete it.
        model.delete_renderer(object, renderer.uid);
        assert_eq!(count(&model), before, "delete_renderer removes it");
        assert_eq!(cartoons(&model), 0, "the cartoon renderer is gone");
    }

    #[test]
    fn save_scene_object_and_export_write_files_in_dioxus() {
        // File-persistence parity with egui/gpui: save scene (.qsc), save molecule
        // (.pdb), and export POV (.pov) each write a real file through the shared core.
        // Verified by INDEPENDENT file-content invariants (not a self-round-trip): the
        // .pdb must carry 1CRN's 327 ATOM records, the .qsc a <scene> element, the .pov
        // a non-empty body.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        let dir = std::env::temp_dir().join(format!("cuemol_dioxus_save_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");

        let qsc = dir.join("scene.qsc");
        model.save_scene(qsc.to_str().unwrap());
        assert!(qsc.exists(), "save_scene wrote a file ({})", model.status);
        let qsc_body = std::fs::read_to_string(&qsc).unwrap();
        assert!(
            qsc_body.contains("<scene"),
            "the .qsc carries a scene element"
        );

        let out_pdb = dir.join("out.pdb");
        model.save_object(out_pdb.to_str().unwrap());
        assert!(out_pdb.exists(), "save_object wrote a file ({})", model.status);
        let atom_lines = std::fs::read_to_string(&out_pdb)
            .unwrap()
            .lines()
            .filter(|line| line.starts_with("ATOM  ") || line.starts_with("HETATM"))
            .count();
        assert_eq!(atom_lines, 327, "1CRN round-trips its 327 atom records");

        let pov = dir.join("scene.pov");
        model.export_scene(pov.to_str().unwrap());
        assert!(pov.exists(), "export_scene wrote a file ({})", model.status);
        assert!(
            std::fs::metadata(&pov).unwrap().len() > 0,
            "the .pov export is non-empty"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn molecule_editing_changes_chain_renumbers_and_deletes_in_dioxus() {
        // Edit-menu parity with egui/gpui: change chain / renumber residues / delete
        // atoms act on the current selection of the first molecule via the shared core.
        // Verified by re-evaluating selections against the EDITED molecule (relational
        // invariants, not hardcoded counts).
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        // `active_render_snapshot(sel)` draws every atom (default simple renderer) and
        // flags the `sel`-matching ones `.selected`. So `total` = whole-molecule atom
        // count (shrinks on delete) and `sel_count` = atoms a selection matches against
        // the EDITED molecule (drops to 0 once those atoms move chain / renumber away).
        let total = |model: &Model| -> usize {
            model
                .state
                .active_render_snapshot("")
                .map(|snapshot| snapshot.atoms.len())
                .unwrap_or(0)
        };
        let sel_count = |model: &Model, sel: &str| -> usize {
            model
                .state
                .active_render_snapshot(sel)
                .map(|snapshot| snapshot.atoms.iter().filter(|atom| atom.selected).count())
                .unwrap_or(0)
        };
        let atoms = total(&model);
        assert!(atoms > 300, "1CRN loads its atoms (got {atoms})");
        assert_eq!(sel_count(&model, "chain A"), atoms, "1CRN is all chain A");

        // Change chain: rename the whole molecule's chain A → Z.
        model.set_selection("chain A");
        model.change_chain("Z");
        assert_eq!(sel_count(&model, "chain A"), 0, "no atom remains on chain A");
        assert_eq!(sel_count(&model, "chain Z"), atoms, "every atom moved to chain Z");

        // Renumber chain Z residues starting at 100 — the first residue becomes 100.
        model.set_selection("chain Z");
        model.renumber_residues(100);
        assert_eq!(sel_count(&model, "resid 1"), 0, "residue 1 renumbered away");
        assert!(sel_count(&model, "resid 100") > 0, "first residue is now 100");

        // Delete the (renumbered) first residue.
        model.set_selection("resid 100");
        let residue_atoms = sel_count(&model, "resid 100");
        let before = total(&model);
        model.delete_selected_atoms();
        assert_eq!(
            total(&model),
            before - residue_atoms,
            "deletion removed exactly the selected residue's atoms"
        );
        assert_eq!(sel_count(&model, "resid 100"), 0, "the deleted residue is gone");
    }

    #[test]
    fn undo_redo_restores_and_replays_an_edit_in_dioxus() {
        // Undo/redo parity with egui/gpui: each mutating edit snapshots the scene, so
        // Undo restores the prior state and Redo replays the edit. Verified on a delete.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        let total = |model: &Model| -> usize {
            model
                .state
                .active_render_snapshot("")
                .map(|snapshot| snapshot.atoms.len())
                .unwrap_or(0)
        };
        let before = total(&model);
        assert!(before > 300, "1CRN loads its atoms (got {before})");

        // Delete the first residue.
        model.set_selection("resid 1");
        model.delete_selected_atoms();
        let after_delete = total(&model);
        assert!(after_delete < before, "delete shrank the molecule");

        // Undo restores the deleted atoms; redo replays the delete.
        model.undo();
        assert_eq!(total(&model), before, "undo restored the deleted atoms");
        model.redo();
        assert_eq!(total(&model), after_delete, "redo replayed the delete");

        // Undo once more empties the stack; a further undo is a no-op with a message.
        model.undo();
        assert_eq!(total(&model), before, "second undo restored again");
        model.undo();
        assert_eq!(model.status, "Nothing to undo", "undo stack is now empty");
    }

    #[test]
    fn reassign_secondary_structure_wires_dssp_in_dioxus() {
        // GUI-wiring test for the Tools → Reassign-secondary-structure button: DSSP runs,
        // preserves the atom set, and is undoable. (The DSSP algorithm itself is verified
        // by cuemol-molanl's prot2ndry_compat tests, not re-checked here.)
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        let atoms = |model: &Model| {
            model
                .state
                .active_render_snapshot("")
                .map(|snapshot| snapshot.atoms.len())
                .unwrap_or(0)
        };
        let before = atoms(&model);
        let undo_before = model.undo_stack.len();
        model.reassign_secondary();
        assert_eq!(model.status, "Secondary structure reassigned");
        assert_eq!(atoms(&model), before, "DSSP does not add/remove atoms");
        assert_eq!(
            model.undo_stack.len(),
            undo_before + 1,
            "reassign is undoable"
        );
    }

    #[test]
    fn background_clear_undo_and_anim_reorder_in_dioxus() {
        // Parity for Scene → Background cycle, Edit → Clear undo data, and anim Up/Down.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);

        // Background cycles dark → black → white → dark (3 steps return to start).
        let dark = model.bg_clear;
        model.cycle_background();
        assert_ne!(model.bg_clear, dark, "background changed");
        model.cycle_background();
        model.cycle_background();
        assert_eq!(model.bg_clear, dark, "background cycles back after 3 steps");

        // Clear undo data empties both stacks.
        model.set_selection("resid 1");
        model.delete_selected_atoms();
        assert!(!model.undo_stack.is_empty(), "edit pushed an undo entry");
        model.clear_undo_data();
        assert!(
            model.undo_stack.is_empty() && model.redo_stack.is_empty(),
            "clear_undo_data empties both stacks"
        );

        // Animation reorder swaps adjacent objects.
        model.add_anim(cuemol_desktop_core::AnimKind::Spin);
        model.add_anim(cuemol_desktop_core::AnimKind::Rock);
        let first = model.animation_objects[0].kind.label();
        model.move_anim(0, 1);
        assert_ne!(
            model.animation_objects[0].kind.label(),
            first,
            "first anim moved down"
        );
        assert_eq!(
            model.animation_objects[1].kind.label(),
            first,
            "it landed in slot 2"
        );
    }

    #[test]
    fn get_pdb_builds_the_rcsb_url_and_guards_empty_input() {
        // The download URL + filename sanitization are pure (network-free); the empty-id
        // guard short-circuits before any curl call. The actual fetch mirrors the egui
        // front end's shipped downloader, so it isn't re-tested over the network here.
        assert_eq!(
            rcsb_pdb_url("1CRN"),
            "https://files.rcsb.org/download/1crn.pdb"
        );
        assert_eq!(sanitize_pdb_id("1c/r n"), "1crn");
        let mut model = Model::new(Vec::new());
        model.get_pdb("   ");
        assert!(
            model.status.contains("Get PDB"),
            "empty id is guarded ({})",
            model.status
        );
    }

    #[test]
    fn view_and_selection_controls_in_dioxus() {
        // Parity for the free-text selection box (full grammar, not just quick-selects),
        // perspective/ortho toggle, and View-fit.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);

        // A free-text expression drives the full selection grammar.
        model.set_selection("resid 1:10");
        assert_eq!(model.selection_text, "resid 1:10");
        let selected = model
            .state
            .active_render_snapshot("resid 1:10")
            .map(|snapshot| snapshot.atoms.iter().filter(|atom| atom.selected).count())
            .unwrap_or(0);
        assert!(selected > 0, "free-text selection matches atoms");

        // Projection toggle flips perspective/orthographic.
        let before = model.camera.is_perspective();
        model.toggle_projection();
        assert_eq!(
            model.camera.is_perspective(),
            !before,
            "projection toggled"
        );

        // Fit re-frames the camera without panicking, yielding a valid (positive) zoom.
        model.fit_view();
        assert!(model.camera.zoom() > 0.0, "fit produced a valid camera");
        assert_eq!(model.status, "View fitted");
    }

    #[test]
    fn coloring_recolors_the_dioxus_snapshot() {
        // Applying Rainbow through the shared core must spread the carbon atoms across a
        // hue gradient (one colour per residue) where Element gives a single carbon
        // colour — the same object-coloring cascade the egui front end uses.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        let distinct = |model: &Model| -> usize {
            model
                .state
                .active_render_snapshot("")
                .map(|snapshot| {
                    snapshot
                        .atoms
                        .iter()
                        .filter_map(|atom| atom.color.as_ref())
                        .map(|color| format!("{color:?}"))
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                })
                .unwrap_or(0)
        };
        model.color_element();
        let element = distinct(&model);
        model.color_rainbow();
        let rainbow = distinct(&model);
        assert!(
            rainbow > element + 10,
            "rainbow must spread carbon colours: element={element} rainbow={rainbow}"
        );
    }

    #[test]
    fn surface_toggle_builds_and_caches_an_ses_mesh() {
        // The dioxus Surface toggle builds the SES via the shared build_surface_mesh and
        // caches a real triangle mesh. A small selection keeps the SES grid cheap.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        model.selection_text = "resid 1:3".to_string();
        assert!(!model.show_surface);
        model.toggle_surface();
        assert!(model.show_surface, "surface toggled on");
        assert!(
            model
                .surface_mesh
                .as_ref()
                .map(|mesh| mesh.indices.len() >= 30)
                .unwrap_or(false),
            "a real SES triangle mesh is cached"
        );
    }

    #[test]
    fn density_contour_toggle_flips_and_keeps_the_render_path_sound() {
        // The dioxus density toggle flips the flag and routes through the same shared
        // append_density_geometry the egui front end's test proves adds a contour; here
        // (no map loaded) it must simply leave the render path panic-free.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        assert!(!model.show_density_contour);
        model.toggle_density_contour();
        assert!(model.show_density_contour);
        assert!(
            model.current_render_snapshot().is_some(),
            "the snapshot path stays sound with the contour toggled on"
        );
    }

    #[test]
    fn selection_quick_selects_drive_the_dioxus_selection() {
        // The sidebar quick-selects set the shared selection grammar and re-tessellate;
        // `*` selects everything, `bymainch *` a strict non-empty backbone subset.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        let selected = |model: &Model| -> usize {
            model
                .state
                .active_render_snapshot(&model.selection_text)
                .map(|snapshot| snapshot.atoms.iter().filter(|atom| atom.selected).count())
                .unwrap_or(0)
        };
        let total = model
            .state
            .active_render_snapshot("")
            .map(|snapshot| snapshot.atoms.len())
            .unwrap_or(0);
        assert!(total > 0);

        model.set_selection("*");
        assert_eq!(selected(&model), total, "* selects all atoms");

        model.set_selection("bymainch *");
        let backbone = selected(&model);
        assert!(
            backbone > 0 && backbone < total,
            "backbone is a strict non-empty subset, got {backbone}/{total}"
        );
    }

    #[test]
    fn symmetry_toggles_add_crystal_geometry_to_the_dioxus_snapshot() {
        // Unit cell adds the 12 box edges; Symm mol adds whole-molecule copies — the
        // shared append_crystal_geometry path, exercised through the dioxus Model.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        let base = model.current_render_snapshot().expect("base snapshot");
        let (base_atoms, base_bonds) = (base.atoms.len(), base.bonds.len());

        model.show_unit_cell = true;
        let with_cell = model.current_render_snapshot().expect("cell snapshot");
        assert_eq!(
            with_cell.bonds.len() - base_bonds,
            12,
            "the unit cell box has 12 edges"
        );
        model.show_unit_cell = false;

        model.show_symm_mates = true;
        let with_mates = model.current_render_snapshot().expect("mates snapshot");
        assert!(
            with_mates.atoms.len() > base_atoms,
            "symmetry mates add whole-molecule copies"
        );
    }

    #[test]
    fn opening_a_file_loads_it_into_the_scene() {
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(Vec::new());
        let scene_uid = model.state.scene_summaries()[0].uid;
        assert_eq!(model.state.object_summaries(scene_uid).len(), 0);
        model.open_path(pdb);
        assert!(
            !model.state.object_summaries(scene_uid).is_empty(),
            "open_path should add an object to the scene"
        );
        assert!(
            model.status.starts_with("Loaded"),
            "status: {}",
            model.status
        );
        assert!(model.tree_rows().len() > 1);
    }

    #[test]
    fn clicking_an_atom_pixel_selects_an_atom_and_missing_selects_nothing() {
        // End-to-end pick: snapshot -> camera_mvp -> pick_atom_screen -> atom
        // summary -> selection. Pure geometry (no GPU); self-skips without 1CRN.
        let Some(pdb) = ref_1crn() else { return };
        let mut model = Model::new(vec![pdb]);
        let snapshot = model.state.active_render_snapshot("").expect("snapshot");
        let Some(atom) = snapshot
            .atoms
            .iter()
            .find(|a| a.object_uid.is_some() && a.atom_index.is_some())
        else {
            return;
        };
        let (w, h) = model.view_size;
        let mvp = camera_mvp(&model.camera, w as f64 / h as f64);
        let (px, py) = project_px(&atom.center, mvp, (w as f64, h as f64));

        model.pick_at(px, py);
        assert!(
            !model.selection_text.is_empty(),
            "clicking an atom's pixel should set a selection"
        );
        assert!(
            model.status.starts_with("Picked"),
            "status: {}",
            model.status
        );

        // A click far outside any projected atom leaves the selection unchanged.
        let kept = model.selection_text.clone();
        model.pick_at(w as f64 * 100.0, h as f64 * 100.0);
        assert_eq!(
            model.selection_text, kept,
            "a miss must not change selection"
        );
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}
