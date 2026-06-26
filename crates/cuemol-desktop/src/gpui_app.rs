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

use std::path::PathBuf;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    div, img, px, rgb, size, App, Application, Bounds, Context, ImageSource, RenderImage, Window,
    WindowBounds, WindowOptions,
};

use crate::{DesktopAppState, GuiObjectSummary, GuiRendererSummary, GuiSceneSummary};

// Resolution the wgpu 3D view is rendered at offscreen; gpui scales it to fill.
const VIEW_W: u32 = 960;
const VIEW_H: u32 = 720;

// A small dark palette (hex RGB) so the panels read as a cohesive theme.
const COL_BG: u32 = 0x1e1e28;
const COL_PANEL: u32 = 0x252530;
const COL_HEADER: u32 = 0x2d2d3a;
const COL_VIEWPORT: u32 = 0x14141c;
const COL_BORDER: u32 = 0x3a3a48;
const COL_TEXT: u32 = 0xd6d6e0;
const COL_TEXT_DIM: u32 = 0x8a8a9a;
const COL_ACCENT: u32 = 0x6db3f2;

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
    /// Last pointer position while dragging the viewport (for orbit deltas).
    dragging: Option<gpui::Point<gpui::Pixels>>,
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
        let camera = state
            .active_render_snapshot("")
            .ok()
            .map(|snap| frame_camera(&snap))
            .unwrap_or_else(cuemol_qsys::Camera::new);
        let viewport_image = render_with_camera(&state, &camera);
        Self {
            state,
            status,
            viewport_image,
            camera,
            dragging: None,
        }
    }

    /// Orbit the camera (drag): `dx`/`dy` are degrees about the Y/X axes. Re-renders.
    pub fn rotate_camera(&mut self, dx_deg: f64, dy_deg: f64) {
        if let Ok(rotation) = self
            .camera
            .rotation()
            .rotate_y(dx_deg)
            .and_then(|q| q.rotate_x(dy_deg))
        {
            self.camera.set_rotation(rotation);
        }
        self.viewport_image = render_with_camera(&self.state, &self.camera);
    }

    /// Zoom the camera (scroll): `factor` > 1 zooms out, < 1 zooms in. Re-renders.
    pub fn zoom_camera(&mut self, factor: f64) {
        let zoom = (self.camera.zoom() * factor).clamp(0.5, 1.0e6);
        self.camera.set_zoom(zoom);
        self.viewport_image = render_with_camera(&self.state, &self.camera);
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(COL_BG))
            .text_color(rgb(COL_TEXT))
            .text_sm()
            .child(self.header())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar())
                    .child(self.viewport(cx)),
            )
            .child(self.status_bar())
    }
}

impl CueMolGpuiApp {
    fn header(&self) -> impl IntoElement {
        let active = self
            .state
            .active_scene_summary()
            .map(|s| s.name)
            .unwrap_or_else(|| "(no scene)".to_string());
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .w_full()
            .h(px(40.0))
            .px_4()
            .bg(rgb(COL_HEADER))
            .border_b_1()
            .border_color(rgb(COL_BORDER))
            .child(div().text_color(rgb(COL_ACCENT)).child("CueMol2-rs"))
            .child(div().text_color(rgb(COL_TEXT_DIM)).child("·"))
            .child(div().child(format!("scene: {active}")))
    }

    fn sidebar(&self) -> impl IntoElement {
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for scene in self.scenes() {
            rows.push(
                tree_row(
                    0,
                    "▸",
                    &scene.name,
                    &format!("{} obj", scene.object_count),
                    true,
                )
                .into_any_element(),
            );
            for object in self.objects(scene.uid) {
                rows.push(
                    tree_row(
                        1,
                        "◆",
                        &object.name,
                        &format!("{} atoms", object.atom_count),
                        false,
                    )
                    .into_any_element(),
                );
                for renderer in self.renderers(object.uid) {
                    let mark = if renderer.visible { "●" } else { "○" };
                    rows.push(
                        tree_row(2, mark, &renderer.name, &renderer.renderer_type, false)
                            .into_any_element(),
                    );
                }
            }
        }
        if rows.is_empty() {
            rows.push(
                div()
                    .p_3()
                    .text_color(rgb(COL_TEXT_DIM))
                    .child("No objects loaded. Pass a file path on the command line.")
                    .into_any_element(),
            );
        }

        div()
            .flex()
            .flex_col()
            .w(px(300.0))
            .h_full()
            .bg(rgb(COL_PANEL))
            .border_r_1()
            .border_color(rgb(COL_BORDER))
            .child(panel_title("Scene tree"))
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

    fn viewport(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let base = div()
            .id("viewport")
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .items_center()
            .justify_center()
            .bg(rgb(COL_VIEWPORT))
            // Scroll to zoom, left-drag to orbit — each mutates the camera and
            // re-renders the offscreen wgpu image.
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
                    this.dragging = Some(event.position);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                if let Some(last) = this.dragging {
                    let dx = f32::from(event.position.x - last.x) as f64;
                    let dy = f32::from(event.position.y - last.y) as f64;
                    this.dragging = Some(event.position);
                    this.rotate_camera(dx * 0.5, dy * 0.5);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _event: &gpui::MouseUpEvent, _window, _cx| {
                    this.dragging = None;
                }),
            );
        match &self.viewport_image {
            // The wgpu-rendered 3D molecular view, composited as a gpui image.
            Some(image) => base.child(
                img(ImageSource::Render(image.clone()))
                    .size_full()
                    .object_fit(gpui::ObjectFit::Contain),
            ),
            None => base.gap_2().child(div().text_color(rgb(COL_TEXT_DIM)).child(
                "3D viewport (wgpu) — load a molecule (or no GPU adapter available)",
            )),
        }
    }

    fn status_bar(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(26.0))
            .px_4()
            .bg(rgb(COL_HEADER))
            .border_t_1()
            .border_color(rgb(COL_BORDER))
            .text_xs()
            .text_color(rgb(COL_TEXT_DIM))
            .child(self.status.clone())
    }
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

/// Render the active scene's 3D molecular view offscreen (wgpu) into a gpui
/// `RenderImage` using `camera`. Returns `None` when the scene is empty or no GPU
/// adapter is present.
fn render_with_camera(
    state: &DesktopAppState,
    camera: &cuemol_qsys::Camera,
) -> Option<Arc<RenderImage>> {
    let snapshot = state.active_render_snapshot("").ok()?;
    if snapshot.atoms.is_empty() && snapshot.bonds.is_empty() && snapshot.meshes.is_empty() {
        return None;
    }
    let mvp = crate::offscreen::camera_mvp(camera, VIEW_W as f64 / VIEW_H as f64);
    let rgba = crate::offscreen::render_snapshot_offscreen(
        &snapshot,
        mvp,
        VIEW_W,
        VIEW_H,
        [0.08, 0.08, 0.11, 1.0],
    )?;
    let buffer = image::RgbaImage::from_raw(VIEW_W, VIEW_H, rgba)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
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

fn tree_row(depth: usize, mark: &str, name: &str, detail: &str, accent: bool) -> impl IntoElement {
    let name_color = if accent { COL_ACCENT } else { COL_TEXT };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .w_full()
        .px_3()
        .py_1()
        .pl(px(12.0 + depth as f32 * 16.0))
        .child(div().text_color(rgb(COL_TEXT_DIM)).child(mark.to_string()))
        .child(div().text_color(rgb(name_color)).child(name.to_string()))
        .child(
            div()
                .text_color(rgb(COL_TEXT_DIM))
                .text_xs()
                .child(detail.to_string()),
        )
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
}

/// Launch the gpui desktop GUI. `open_paths` are molecule files to load on start.
pub fn run(open_paths: Vec<PathBuf>) {
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
        let open_paths = open_paths.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| CueMolGpuiApp::new(open_paths)),
        )
        .expect("failed to open gpui window");
        cx.activate(true);
    });
}
