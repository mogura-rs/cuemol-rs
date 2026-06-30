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

// Dark palette shared by the panels (CSS hex), mirroring the gpui shell.
const COL_BG: &str = "#1e1e28";
const COL_PANEL: &str = "#252530";
const COL_HEADER: &str = "#2d2d3a";
const COL_VIEWPORT: &str = "#14141c";
const COL_BORDER: &str = "#3a3a48";
const COL_TEXT: &str = "#d6d6e0";
const COL_TEXT_DIM: &str = "#8a8a9a";
const COL_ACCENT: &str = "#6db3f2";

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
        };
        model.rebuild_after_load();
        model
    }

    /// Recompute snapshot → camera → mesh → image after the scene's molecule set
    /// changes. Restores a loaded `.qsc`'s saved view; auto-frames a plain
    /// molecule from its bounding box.
    fn rebuild_after_load(&mut self) {
        let snapshot = self.state.active_render_snapshot(&self.selection_text).ok();
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
            .state
            .active_render_snapshot(&self.selection_text)
            .ok()
            .as_ref()
            .map(cuemol_render::tessellate_snapshot);
        self.rerender();
    }

    /// Re-render the offscreen 3D image at the current camera into `image_uri`.
    /// Cheap relative to first build: reuses the persistent device + cached mesh.
    fn rerender(&mut self) {
        self.image_uri = self.render_uri();
    }

    fn render_uri(&self) -> Option<String> {
        let renderer = self.renderer.as_ref()?;
        let mesh = self.mesh.as_ref()?;
        if mesh.indices.is_empty() {
            return None; // empty scene -> placeholder
        }
        let (w, h) = self.view_size;
        let mvp = camera_mvp(&self.camera, w as f64 / h as f64);
        let rgba = renderer.render(std::slice::from_ref(mesh), mvp, w, h, VIEW_CLEAR)?;
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

        let mut lines = Vec::new();
        for seq in self.state.active_sequence_summaries() {
            let chain = if seq.chain_id.is_empty() {
                "_".to_string()
            } else {
                seq.chain_id.clone()
            };
            lines.push(format!("chain {chain} — {} residues", seq.residue_count));
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
fn app() -> Element {
    let paths = use_context::<InitialPaths>();
    let mut model = use_signal(move || Model::new(paths.0.clone()));
    // The Open field's text, held separately from the model so per-keystroke edits
    // don't churn the wgpu-owning model signal.
    let mut open_text = use_signal(String::new);

    // Derive view data under a short read borrow, then release it before RSX so
    // the event handlers (which write the signal) never alias the read guard.
    let (status, scene_name, image_uri, rows, sections) = {
        let m = model.read();
        (
            m.status.clone(),
            m.active_scene_name(),
            m.image_uri.clone(),
            m.tree_rows(),
            m.panel_sections(),
        )
    };

    let root_style = format!(
        "display:flex;flex-direction:column;height:100vh;width:100vw;margin:0;\
         background:{COL_BG};color:{COL_TEXT};font:13px -apple-system,system-ui,sans-serif;\
         overflow:hidden;user-select:none;"
    );
    let header_style = format!(
        "display:flex;align-items:center;gap:12px;height:40px;padding:0 16px;\
         background:{COL_HEADER};border-bottom:1px solid {COL_BORDER};flex:0 0 auto;"
    );
    let open_input_style = format!(
        "margin-left:auto;width:280px;height:24px;padding:0 8px;border-radius:4px;\
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
        "display:flex;flex:1;min-width:0;align-items:center;justify-content:center;\
         background:{COL_VIEWPORT};overflow:hidden;cursor:grab;"
    );
    let status_style = format!(
        "display:flex;align-items:center;height:26px;padding:0 16px;background:{COL_HEADER};\
         border-top:1px solid {COL_BORDER};font-size:11px;color:{COL_TEXT_DIM};flex:0 0 auto;"
    );
    let menubar_style = format!(
        "display:flex;align-items:center;gap:4px;height:28px;padding:0 12px;background:{COL_PANEL};\
         border-bottom:1px solid {COL_BORDER};flex:0 0 auto;font-size:12px;"
    );
    let menu_item_style =
        format!("padding:2px 8px;border-radius:4px;color:{COL_TEXT};cursor:default;");
    let section_header_style = format!(
        "padding:6px 12px 2px;color:{COL_ACCENT};font-size:11px;\
         border-bottom:1px solid {COL_BORDER};margin-top:6px;"
    );
    let section_line_style = format!("padding:2px 12px;color:{COL_TEXT};font-size:11px;");

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
                span { style: "color:{COL_TEXT_DIM};margin-left:12px;", "Dioxus" }
            }
            // Menu bar (CueMol2 cuemol2-menus.xul). Visual structure; functional
            // dropdowns are a later pass — file open works via the header field.
            div { style: "{menubar_style}",
                for menu in ["File", "Edit", "Render", "Scene", "View", "Tools", "Window", "Help"] {
                    span { key: "{menu}", style: "{menu_item_style}", "{menu}" }
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
