//! Dioxus-based desktop GUI — a third front end parallel to the egui (`gui`) and
//! gpui (`gpui_app`) modules in `cuemol-desktop`.
//!
//! Same division of labour as the gpui shell: Dioxus/RSX draws the panel chrome
//! (header, scene-tree sidebar, status bar) over the headless [`DesktopAppState`]
//! logic, while the 3D molecular view is rendered separately with wgpu
//! ([`cuemol_desktop_core::offscreen`]) and composited into the webview as an
//! `<img>`. Because a webview can't take a raw GPU texture, the rendered RGBA is
//! PNG-encoded and embedded as a base64 `data:` URI; it is regenerated whenever
//! the camera moves. (A wry custom-protocol handler serving raw PNG bytes would
//! avoid the base64 inflation — a worthwhile later optimisation.)

use std::path::PathBuf;

use dioxus::html::input_data::MouseButton;
use dioxus::html::{InteractionElementOffset, PointerInteraction};
use dioxus::prelude::*;

use cuemol_desktop_core::offscreen::{camera_mvp, encode_rgba_png, SceneRenderer};
use cuemol_desktop_core::DesktopAppState;

// Offscreen render resolution. The webview scales the PNG to the viewport with
// `object-fit: contain`, so this is a quality/cost knob, not a layout size.
const VIEW_W: u32 = 960;
const VIEW_H: u32 = 720;

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
    /// Scene tessellated once into GPU geometry; reused for every camera move.
    mesh: Option<cuemol_render::RenderMesh>,
    /// `data:image/png;base64,…` of the latest offscreen render (`None` = empty
    /// scene / no adapter).
    image_uri: Option<String>,
    /// Active drag: gesture kind + last pointer position (element coords, px).
    drag: Option<(DragKind, f64, f64)>,
}

impl Model {
    /// Build the model, loading any command-line molecules into a fresh scene,
    /// auto-framing the camera, and tessellating once. Mirrors gpui's
    /// `CueMolGpuiApp::new`.
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
        let snapshot = state.active_render_snapshot("").ok();
        // Restore a loaded .qsc's saved view; auto-frame a plain molecule.
        let camera = state
            .active_scene_saved_camera()
            .or_else(|| snapshot.as_ref().map(frame_camera))
            .unwrap_or_default();
        let mesh = snapshot.as_ref().map(cuemol_render::tessellate_snapshot);
        let mut model = Self {
            state,
            status,
            camera,
            renderer: SceneRenderer::new(),
            mesh,
            image_uri: None,
            drag: None,
        };
        model.rerender();
        model
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
        let mvp = camera_mvp(&self.camera, VIEW_W as f64 / VIEW_H as f64);
        let rgba = renderer.render(std::slice::from_ref(mesh), mvp, VIEW_W, VIEW_H, VIEW_CLEAR)?;
        let png = encode_rgba_png(&rgba, VIEW_W, VIEW_H);
        Some(format!("data:image/png;base64,{}", base64_encode(&png)))
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
            .set_center(pan_center(&self.camera, dx_pix, dy_pix, VIEW_H));
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

    // Derive view data under a short read borrow, then release it before RSX so
    // the event handlers (which write the signal) never alias the read guard.
    let (status, scene_name, image_uri, rows) = {
        let m = model.read();
        (
            m.status.clone(),
            m.active_scene_name(),
            m.image_uri.clone(),
            m.tree_rows(),
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

    rsx! {
        div { style: "{root_style}",
            // Header.
            div { style: "{header_style}",
                span { style: "color:{COL_ACCENT};", "CueMol2-rs" }
                span { style: "color:{COL_TEXT_DIM};", "·" }
                span { "scene: {scene_name}" }
                span { style: "color:{COL_TEXT_DIM};margin-left:auto;", "Dioxus" }
            }
            // Body: sidebar + viewport.
            div { style: "display:flex;flex:1;min-height:0;",
                div { style: "{sidebar_style}",
                    div { style: "{panel_title_style}", "Scene tree" }
                    div { style: "flex:1;min-height:0;overflow-y:auto;",
                        if rows.is_empty() {
                            div { style: "padding:12px;color:{COL_TEXT_DIM};",
                                "No objects loaded. Pass a file path on the command line."
                            }
                        }
                        for (i, row) in rows.iter().enumerate() {
                            TreeRowView { key: "{i}", row: row.clone() }
                        }
                    }
                }
                // 3D viewport: scroll to zoom, left-drag to orbit, right-drag to pan.
                div {
                    style: "{viewport_style}",
                    onmousedown: move |e: MouseEvent| {
                        let kind = match e.trigger_button() {
                            Some(MouseButton::Secondary) => DragKind::Pan,
                            _ => DragKind::Orbit,
                        };
                        let p = e.element_coordinates();
                        model.write().drag = Some((kind, p.x, p.y));
                    },
                    onmousemove: move |e: MouseEvent| {
                        let Some((kind, lx, ly)) = model.read().drag else { return };
                        // The triggering button must still be held; otherwise a
                        // mouseup was missed off-viewport — cancel the gesture.
                        let still_held = match kind {
                            DragKind::Orbit => e.held_buttons().contains(MouseButton::Primary),
                            DragKind::Pan => e.held_buttons().contains(MouseButton::Secondary),
                        };
                        if !still_held {
                            model.write().drag = None;
                            return;
                        }
                        let p = e.element_coordinates();
                        let (dx, dy) = (p.x - lx, p.y - ly);
                        let mut m = model.write();
                        m.drag = Some((kind, p.x, p.y));
                        match kind {
                            DragKind::Orbit => m.rotate_camera(dx * 0.5, dy * 0.5),
                            DragKind::Pan => m.pan_camera(dx, dy),
                        }
                    },
                    onmouseup: move |_| { model.write().drag = None; },
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

    // The non-rendering logic (model + summaries + camera) is GPU-free testable,
    // exactly like the gpui shell's headless tests; only `app()`/`run()` need a
    // webview. (`Model::new` may build a wgpu renderer if an adapter exists, but
    // `rerender` self-skips to `None` on an empty scene, so this runs in CI.)

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

    /// Resolve the 1CRN reference fixture from `CUEMOL2_REF_DIR` or the default
    /// sibling `../cuemol2` tree (relative to the repo root, NOT the test CWD —
    /// `cargo test` runs from the crate dir). `None` when the tree is absent, so
    /// the test self-skips exactly like the workspace's `*_compat` tests.
    fn ref_1crn() -> Option<PathBuf> {
        let base = std::env::var_os("CUEMOL2_REF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2")
            });
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
        assert!(model.status.starts_with("Loaded"), "status: {}", model.status);
        let tris = model.mesh.as_ref().map(|m| m.indices.len() / 3).unwrap_or(0);
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
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}
