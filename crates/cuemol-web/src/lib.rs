#![cfg(target_arch = "wasm32")]
mod input;
mod panels;
mod trajectory;
mod tritium;
use base64::Engine;
use cuemol_core::Uid;
use cuemol_desktop_core::{viewport::ViewportRenderer, DesktopAppState, GuiLoadedPath};
use cuemol_qsys::Camera;
use cuemol_render::RenderSceneSnapshot;
use eframe::{egui, egui_wgpu};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use trajectory::BrowserTrajectory;
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace=cuemolOidn,js_name=cancel)]
    fn cancel_oidn();
}
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};

thread_local! {
    static EVENTS: RefCell<VecDeque<Event>> = RefCell::new(VecDeque::new());
    static CONTEXT: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
    static STATUS: RefCell<String> = RefCell::new(r#"{"busy":true,"scene":{"sceneId":0,"objects":[]}}"#.to_string());
}
enum Event {
    Input(Value),
    AgentStart(String, cuemol_agent::Settings, String),
    AgentCancel,
    AgentFrame(
        Uid,
        usize,
        Result<cuemol_mdtools::TrajectoryFrame, String>,
        cuemol_agent::ToolReply,
        u64,
    ),
    Source(web_sys::File),
    Data(String, Vec<u8>),
    Error(String),
    Trajectory(Uid, BrowserTrajectory),
    Frame(Uid, usize, cuemol_mdtools::TrajectoryFrame),
    Command(String, Value),
    Rendered(cuemol_core::Result<Value>, Option<String>),
}
fn enqueue(event: Event) {
    EVENTS.with(|events| events.borrow_mut().push_back(event));
    CONTEXT.with(|ctx| {
        if let Some(ctx) = ctx.borrow().as_ref() {
            ctx.request_repaint();
        }
    });
}
fn js_error(error: JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
#[wasm_bindgen]
pub fn open_file(file: web_sys::File) {
    enqueue(Event::Source(file));
}
#[wasm_bindgen]
pub fn command(name: String, arguments: String) -> Result<(), JsValue> {
    let args = serde_json::from_str(&arguments).map_err(|e| JsValue::from_str(&e.to_string()))?;
    enqueue(Event::Command(name, args));
    Ok(())
}
#[wasm_bindgen]
pub fn agent_turn(prompt: String, settings: String, api_key: String) -> Result<(), JsValue> {
    let settings = serde_json::from_str(&settings)
        .map_err(|e| JsValue::from_str(&format!("Invalid agent settings: {e}")))?;
    enqueue(Event::AgentStart(prompt, settings, api_key));
    Ok(())
}
#[wasm_bindgen]
pub fn agent_cancel() {
    enqueue(Event::AgentCancel);
}
#[wasm_bindgen]
pub fn view_input(event: String) -> Result<(), JsValue> {
    let event = serde_json::from_str(&event).map_err(|e| JsValue::from_str(&e.to_string()))?;
    enqueue(Event::Input(event));
    Ok(())
}
#[wasm_bindgen]
pub fn status() -> String {
    STATUS.with(|status| status.borrow().clone())
}
#[wasm_bindgen]
pub async fn start(canvas: web_sys::HtmlCanvasElement, force_webgl: bool) -> Result<(), JsValue> {
    let runner = eframe::WebRunner::new();
    let mut options = eframe::WebOptions::default();
    if force_webgl {
        if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
            setup.instance_descriptor.backends = wgpu::Backends::GL;
        }
    }
    runner
        .start(canvas, options, Box::new(|cc| Ok(Box::new(App::new(cc)?))))
        .await?;
    // The browser owns the event loop for the lifetime of the document.
    std::mem::forget(runner);
    Ok(())
}
fn download(name: &str, bytes: &[u8], mime: &str) -> Result<(), JsValue> {
    let array = js_sys::Array::new();
    array.push(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&array, &options)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let document = web_sys::window().unwrap().document().unwrap();
    let link = document
        .create_element("a")?
        .dyn_into::<web_sys::HtmlAnchorElement>()?;
    link.set_href(&url);
    link.set_download(name);
    link.click();
    web_sys::Url::revoke_object_url(&url)?;
    Ok(())
}
fn choose_file() {
    if let Some(input) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("files"))
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
    {
        input.click();
    }
}
struct Target {
    view: wgpu::TextureView,
    depth: wgpu::TextureView,
    id: egui::TextureId,
    size: [u32; 2],
}
struct HistoryEntry {
    selection: String,
    state: DesktopAppState,
    camera: Camera,
    trajectory: Option<(Uid, Rc<BrowserTrajectory>)>,
    frame: usize,
    selected: Option<Uid>,
}
struct App {
    input: cuemol_qsys::view_input::ViewInputController,
    input_device: cuemol_qsys::view_input::InputDeviceDetector,
    input_events: VecDeque<Value>,
    agent: cuemol_ui::AgentPanel,
    activity: Option<cuemol_ui::shell::Activity>,
    inspector: cuemol_ui::Inspector,
    rectangle_select: bool,
    rectangle_start: Option<egui::Pos2>,
    inspector_target: Option<cuemol_ui::InspectorTarget>,
    bottom_tab: String,
    bottom_open: bool,
    console_source: String,
    logs: Vec<String>,
    last_log: String,
    last_scene: Option<(String, Vec<u8>)>,
    show_get_pdb: bool,
    ui_rects: std::collections::BTreeMap<String, [f32; 4]>,
    revision: u64,
    state: DesktopAppState,
    camera: Camera,
    gpu: egui_wgpu::RenderState,
    renderer: ViewportRenderer,
    target: Option<Target>,
    snapshot: Option<RenderSceneSnapshot>,
    dirty: bool,
    selected: Option<Uid>,
    selection: String,
    representation: String,
    coloring: String,
    pdb_id: String,
    console_tool: String,
    console_args: String,
    message: String,
    history: Vec<HistoryEntry>,
    future: Vec<HistoryEntry>,
    trajectory: Option<(Uid, Rc<BrowserTrajectory>)>,
    frame: usize,
    busy: bool,
    render_cancel: Option<Arc<AtomicBool>>,
    playing: bool,
    fps: f32,
    next_frame: f64,
    show_console: bool,
    show_help: bool,
    dark: bool,
    show_settings: bool,
    show_render: bool,
    render_hatching: bool,
    render_hatch: cuemol_render::HatchRenderParams,
    render_lighting: cuemol_render::UmbreonRenderParams,
    render_size: [u32; 2],
    render_transparent: bool,
    show_timeline: bool,
    settings_text: String,
    startup_text: String,
    tool_filter: String,
    form_tool: String,
    form_values: std::collections::BTreeMap<String, String>,
}
impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let gpu = cc.wgpu_render_state.clone().ok_or("wgpu is unavailable")?;
        let mut state = DesktopAppState::new();
        state.new_scene("Untitled", true);
        let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten());
        if let Some(config) = storage
            .as_ref()
            .and_then(|s| s.get_item("cuemol.config").ok().flatten())
        {
            let _ = state.load_config_text(&config);
        }
        if let Some(storage) = &storage {
            for i in 0..storage.length().unwrap_or(0) {
                let Some(key) = storage.key(i).ok().flatten() else {
                    continue;
                };
                if let Some(name) = key.strip_prefix("cuemol.icc.") {
                    if let Some(bytes) = storage.get_item(&key).ok().flatten().and_then(|value| {
                        base64::engine::general_purpose::STANDARD.decode(value).ok()
                    }) {
                        let _ = cuemol_gfx::cms::register_icc_profile(name, &bytes);
                    }
                }
            }
        }
        let dark = storage
            .as_ref()
            .and_then(|s| s.get_item("cuemol.theme").ok().flatten())
            .as_deref()
            != Some("light");
        cc.egui_ctx.set_theme(if dark {
            egui::ThemePreference::Dark
        } else {
            egui::ThemePreference::Light
        });
        cc.egui_ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        let startup_text = storage
            .as_ref()
            .and_then(|s| s.get_item("cuemol.startup").ok().flatten())
            .unwrap_or_default();
        let mut message = "Open a structure or enter a PDB ID.".to_string();
        if !startup_text.trim().is_empty() {
            match state.run_script_js(&startup_text) {
                Ok((log, _)) => message = log,
                Err(e) => message = format!("Startup script: {e}"),
            }
        }
        let camera = state.active_view_camera().unwrap_or_default();
        CONTEXT.with(|ctx| *ctx.borrow_mut() = Some(cc.egui_ctx.clone()));
        let renderer = ViewportRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm);
        let mut agent = cuemol_ui::AgentPanel::default();
        if let Some(text) = storage
            .as_ref()
            .and_then(|s| s.get_item("cuemol.agent").ok().flatten())
        {
            agent.load_settings(&text);
        }
        Ok(Self {
            agent,
            activity: storage
                .as_ref()
                .and_then(|s| s.get_item("cuemol.activity").ok().flatten())
                .map(|s| cuemol_ui::shell::Activity::parse(&s))
                .unwrap_or(Some(cuemol_ui::shell::Activity::Explorer)),
            inspector: Default::default(),
            rectangle_select: false,
            rectangle_start: None,
            inspector_target: None,
            bottom_tab: "Console".into(),
            bottom_open: true,
            console_source: String::new(),
            logs: Vec::new(),
            last_log: String::new(),
            last_scene: None,
            show_get_pdb: false,
            ui_rects: Default::default(),
            revision: 0,
            state,
            camera,
            gpu,
            renderer,
            target: None,
            snapshot: None,
            dirty: true,
            selected: None,
            selection: String::new(),
            representation: "ribbon".into(),
            coloring: "rainbow".into(),
            pdb_id: String::new(),
            console_tool: "get_scene_state".into(),
            console_args: "{}".into(),
            message,
            history: Vec::new(),
            future: Vec::new(),
            trajectory: None,
            frame: 0,
            busy: false,
            render_cancel: None,
            playing: false,
            fps: 24.0,
            next_frame: 0.0,
            show_console: false,
            show_help: false,
            dark,
            show_settings: false,
            show_render: false,
            render_hatching: false,
            render_hatch: Default::default(),
            render_lighting: cuemol_command::rendering_params(&serde_json::Map::new(), "umbreon")
                .unwrap(),
            render_size: [1200, 900],
            render_transparent: false,
            show_timeline: false,
            settings_text: String::new(),
            input: Default::default(),
            input_device: Default::default(),
            input_events: Default::default(),
            startup_text,
            tool_filter: String::new(),
            form_tool: String::new(),
            form_values: Default::default(),
        })
    }
    fn capture_history(&self) -> HistoryEntry {
        HistoryEntry {
            selection: self.selection.clone(),
            state: self.state.clone(),
            camera: self.camera.clone(),
            trajectory: self.trajectory.clone(),
            frame: self.frame,
            selected: self.selected,
        }
    }
    fn restore_history(&mut self, snapshot: HistoryEntry) {
        self.selection = snapshot.selection;
        self.state = snapshot.state;
        self.camera = snapshot.camera;
        self.trajectory = snapshot.trajectory;
        self.frame = snapshot.frame;
        self.selected = snapshot.selected;
        self.playing = false;
        self.dirty = true;
    }
    fn checkpoint(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.history.push(self.capture_history());
        if self.history.len() > 12 {
            self.history.remove(0);
        }
        self.future.clear();
    }
    fn call(&mut self, name: &str, args: Value) {
        if name == "cancel_render" {
            if let Some(cancel) = &self.render_cancel {
                cancel.store(true, Ordering::Relaxed);
                cancel_oidn();
            }
            return;
        }
        if matches!(name, "get_screenshot" | "capture_view" | "render_image") {
            let download = if name == "render_image" {
                Some(
                    args["fileName"]
                        .as_str()
                        .unwrap_or("cuemol.png")
                        .to_string(),
                )
            } else {
                None
            };
            self.render_async(name, &args, download);
            return;
        }
        if let Some((id, trajectory)) = &self.trajectory {
            if name == "set_frame" && args["objId"].as_u64() == Some(id.get()) {
                if let Some(index) = args["frame"]
                    .as_u64()
                    .filter(|i| *i < trajectory.frames as u64)
                {
                    self.request_frame(index as usize);
                } else {
                    self.message = "Frame out of range".into();
                }
                return;
            }
            if name == "list_trajectories" {
                self.message = json!({"trajectories":[{"objectId":id.get(),"frames":trajectory.frames,"frame":self.frame}]}).to_string();
                return;
            }
        }
        if name == "undo" || name == "redo" {
            self.revision = self.revision.wrapping_add(1);
            let snapshot = if name == "undo" {
                self.history.pop()
            } else {
                self.future.pop()
            };
            if let Some(snapshot) = snapshot {
                let current = self.capture_history();
                if name == "undo" {
                    self.future.push(current);
                } else {
                    self.history.push(current);
                }
                self.restore_history(snapshot);
            }
            return;
        }
        if name == "clear_undo" {
            self.history.clear();
            self.future.clear();
            self.message = "Undo history cleared.".into();
            return;
        }
        let mutating = cuemol_command::tool_is_mutating(name);
        let record_history = cuemol_command::tool_records_history(name);
        if record_history {
            self.checkpoint();
        }
        match cuemol_command::call_tool(&mut self.state, &mut self.camera, name, &args) {
            Ok(value) => {
                self.message = serde_json::to_string_pretty(&value).unwrap();
                self.dirty |= mutating;
            }
            Err(e) => {
                if record_history {
                    if let Some(snapshot) = self.history.pop() {
                        self.restore_history(snapshot);
                    }
                }
                self.message = e.to_string();
            }
        }
    }
    fn fit(&mut self, selected: bool) {
        self.input.cancel();
        if let Some(id) = self.selected.filter(|_| selected) {
            self.call("center_view",json!({"objId":id.get(),"selection":if self.selection.is_empty(){"*"}else{&self.selection},"zoom":true}));
        } else if let Some((min, max)) = self.snapshot.as_ref().and_then(|s| s.bounds) {
            self.camera.set_center((min + max) * 0.5);
            let extent = (max - min).length().max(2.0);
            self.camera.set_zoom(extent * 1.15);
            self.camera.set_cam_dist(extent * 2.5);
            self.camera.set_slab_depth(extent * 2.0);
        }
    }
    fn events(&mut self) {
        let events = EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()));
        for event in events {
            match event {
                Event::Input(event) => self.input_events.push_back(event),
                Event::AgentCancel => self.agent.cancel("Cancelled; completed edits are kept."),
                Event::AgentFrame(id, index, result, reply, revision) => {
                    if reply.is_closed() {
                        continue;
                    }
                    if revision != self.revision {
                        let _ =
                            reply.try_send(Err("Scene changed before the frame was loaded".into()));
                        continue;
                    }
                    let undo = (!self.agent.changed_scene).then(|| self.capture_history());
                    let result = result.and_then(|frame| {
                        self.state
                            .apply_coordinate_frame(id, &frame)
                            .map_err(|e| e.to_string())
                    });
                    if result.is_ok() {
                        if let Some(undo) = undo {
                            self.history.push(undo);
                            self.future.clear();
                            if self.history.len() > 12 {
                                self.history.remove(0);
                            }
                        }
                        self.frame = index;
                        self.dirty = true;
                        self.agent.changed_scene = true;
                        self.revision = self.revision.wrapping_add(1);
                        self.agent.revision = self.revision;
                    }
                    let _ =
                        reply.try_send(result.map(|()| json!({"objectId":id.get(),"frame":index})));
                }
                Event::AgentStart(prompt, settings, key) => {
                    if self.agent.running() {
                        self.message = "An agent turn is already running.".into();
                        continue;
                    }
                    self.agent.settings = settings;
                    self.agent.set_session_key(key);
                    self.agent.visible = true;
                    CONTEXT.with(|ctx| {
                        if let Some(ctx) = ctx.borrow().as_ref() {
                            self.agent.start(
                                &mut self.state,
                                &mut self.camera,
                                prompt,
                                ctx,
                                self.revision,
                            );
                        }
                    });
                }
                Event::Source(file) => {
                    let name = file.name();
                    let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                    if matches!(
                        extension.as_str(),
                        "dcd" | "xtc" | "trr" | "nc" | "netcdf" | "mdcrd" | "crd"
                    ) {
                        let Some(id) = self.selected else {
                            self.message = "Load and select a topology first.".into();
                            continue;
                        };
                        let atoms = self
                            .state
                            .scene_manager()
                            .object(id)
                            .map(|o| o.mol().atom_count());
                        self.busy = true;
                        self.message = "Indexing trajectory…".into();
                        spawn_local(async move {
                            match BrowserTrajectory::open(file, atoms).await {
                                Ok(t) => enqueue(Event::Trajectory(id, t)),
                                Err(e) => enqueue(Event::Error(js_error(e))),
                            }
                        });
                    } else {
                        spawn_local(async move {
                            match JsFuture::from(file.array_buffer()).await {
                                Ok(bytes) => enqueue(Event::Data(
                                    name,
                                    js_sys::Uint8Array::new(&bytes).to_vec(),
                                )),
                                Err(e) => enqueue(Event::Error(js_error(e))),
                            }
                        });
                    }
                }
                Event::Data(name, bytes) => {
                    if name.to_lowercase().ends_with(".qsc") {
                        self.last_scene = Some((name.clone(), bytes.clone()));
                    }
                    if name.rsplit('.').next().is_some_and(|ext| {
                        matches!(ext.to_ascii_lowercase().as_str(), "icc" | "icm")
                    }) {
                        match cuemol_gfx::cms::register_icc_profile(&name, &bytes) {
                            Ok(()) => {
                                self.checkpoint();
                                self.state.put_config_value("color:icc_profile", &name);
                                if let Some(scene) = self
                                    .state
                                    .active_scene()
                                    .and_then(|id| self.state.scene_manager_mut().scene_mut(id))
                                {
                                    scene.set_scene_attribute("use_colproof", "true");
                                    scene.set_scene_attribute("icc_filename", &name);
                                }
                                self.dirty = true;
                                self.message = format!("Loaded ICC profile {name}");
                                if let Some(storage) =
                                    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
                                {
                                    let result = storage
                                        .set_item(
                                            &format!("cuemol.icc.{name}"),
                                            &base64::engine::general_purpose::STANDARD
                                                .encode(&bytes),
                                        )
                                        .and_then(|_| {
                                            storage.set_item(
                                                "cuemol.config",
                                                &self.state.config_text(),
                                            )
                                        });
                                    if result.is_err() {
                                        self.message.push_str(" (browser storage is full)");
                                    }
                                }
                            }
                            Err(error) => self.message = error.to_string(),
                        }
                        continue;
                    }
                    self.checkpoint();
                    match self.state.open_bytes(&name, &bytes) {
                        Ok(loaded) => {
                            self.selected = match loaded {
                                GuiLoadedPath::Object { object_uid, .. } => Some(object_uid),
                                _ => None,
                            };
                            self.dirty = true;
                            self.rebuild();
                            if matches!(loaded, GuiLoadedPath::Scene { .. }) {
                                self.camera = self.state.active_view_camera().unwrap_or_default();
                            } else {
                                self.fit(false);
                            }
                            self.message = format!("Loaded {name}");
                        }
                        Err(e) => {
                            let snapshot = self.history.pop().unwrap();
                            self.restore_history(snapshot);
                            self.message = e.to_string();
                        }
                    }
                }
                Event::Error(error) => {
                    self.message = error;
                    self.busy = false;
                    self.playing = false;
                }
                Event::Trajectory(id, trajectory) => {
                    let atoms = self
                        .state
                        .scene_manager()
                        .object(id)
                        .map(|o| o.mol().atom_count());
                    if atoms != Some(trajectory.atoms) {
                        self.message =
                            "Trajectory atom count does not match the selected topology.".into();
                        self.busy = false;
                        continue;
                    }
                    self.message = format!(
                        "Indexed {} frames; coordinates are read one frame at a time.",
                        trajectory.frames
                    );
                    self.trajectory = Some((id, Rc::new(trajectory)));
                    self.busy = false;
                    self.request_frame(0);
                }
                Event::Frame(id, index, frame) => {
                    self.revision = self.revision.wrapping_add(1);
                    self.busy = false;
                    match self.state.apply_coordinate_frame(id, &frame) {
                        Ok(()) => {
                            self.frame = index;
                            self.dirty = true;
                        }
                        Err(e) => {
                            self.message = e.to_string();
                            self.playing = false;
                        }
                    }
                }
                Event::Command(name, args) => self.call(&name, args),
                Event::Rendered(result, name) => {
                    self.busy = false;
                    self.render_cancel = None;
                    match result {
                        Ok(value) => {
                            if let Some(name) = name {
                                match base64::engine::general_purpose::STANDARD
                                    .decode(value["_image"].as_str().unwrap_or(""))
                                {
                                    Ok(bytes) => match download(&name, &bytes, "image/png") {
                                        Ok(()) => self.message = format!("Saved {name}"),
                                        Err(e) => self.message = js_error(e),
                                    },
                                    Err(e) => self.message = e.to_string(),
                                }
                            } else {
                                self.message = serde_json::to_string_pretty(&value).unwrap();
                            }
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
            }
        }
    }
    fn request_frame(&mut self, index: usize) {
        if self.busy {
            return;
        }
        let Some((id, trajectory)) = self.trajectory.as_ref() else {
            return;
        };
        let (id, trajectory) = (*id, trajectory.clone());
        self.busy = true;
        spawn_local(async move {
            match trajectory.frame(index).await {
                Ok(frame) => enqueue(Event::Frame(id, index, frame)),
                Err(e) => enqueue(Event::Error(js_error(e))),
            }
        });
    }
    fn rebuild(&mut self) {
        if !self.dirty {
            return;
        }
        match self.state.active_render_snapshot(&self.selection) {
            Ok(mut snapshot) => {
                if let Err(error) = self.state.apply_scene_color_proof(&mut snapshot) {
                    self.message = error.to_string();
                }
                self.renderer
                    .upload(&self.gpu.device, &self.gpu.queue, &snapshot);
                self.snapshot = Some(snapshot);
            }
            Err(e) => self.message = e.to_string(),
        }
        self.dirty = false;
    }
    fn fetch(&mut self) {
        let id = self.pdb_id.trim().to_uppercase();
        if id.len() != 4 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            self.message = "Enter a four-character PDB ID.".into();
            return;
        }
        self.message = format!("Downloading {id}…");
        spawn_local(async move {
            let result = async {
                let response = JsFuture::from(
                    web_sys::window()
                        .unwrap()
                        .fetch_with_str(&format!("https://files.rcsb.org/download/{id}.cif")),
                )
                .await?
                .dyn_into::<web_sys::Response>()?;
                if !response.ok() {
                    return Err(JsValue::from_str(&format!("HTTP {}", response.status())));
                }
                let bytes = JsFuture::from(response.array_buffer()?).await?;
                Ok(js_sys::Uint8Array::new(&bytes).to_vec())
            }
            .await;
            match result {
                Ok(bytes) => enqueue(Event::Data(format!("{id}.cif"), bytes)),
                Err(e) => enqueue(Event::Error(js_error(e))),
            }
        });
    }
    fn render_async(&mut self, name: &str, args: &Value, download: Option<String>) {
        if self.busy {
            self.message = "Wait for the current operation to finish.".into();
            return;
        }
        match cuemol_command::prepare_render(&self.state, &self.camera, name, args) {
            Ok(render) => {
                self.busy = true;
                self.message = "Rendering image…".into();
                let cancel = Arc::new(AtomicBool::new(false));
                self.render_cancel = Some(cancel.clone());
                spawn_local(async move {
                    enqueue(Event::Rendered(
                        render.run_async_with_cancel(Some(&cancel)).await,
                        download,
                    ));
                });
            }
            Err(e) => self.message = e.to_string(),
        }
    }
    fn export_png(&mut self) {
        self.render_async(
            "get_screenshot",
            &json!({"width":self.render_size[0],"height":self.render_size[1]}),
            Some("cuemol.png".into()),
        );
    }
    fn viewport(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let pixels = ui.ctx().pixels_per_point();
        let resolution = [
            (size.x * pixels).max(1.0) as u32,
            (size.y * pixels).max(1.0) as u32,
        ];
        if self.target.as_ref().is_none_or(|t| t.size != resolution) {
            let view = self
                .gpu
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("molecular-view"),
                    size: wgpu::Extent3d {
                        width: resolution[0],
                        height: resolution[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            let depth = cuemol_desktop_core::viewport::create_depth_view(
                &self.gpu.device,
                resolution[0],
                resolution[1],
            );
            let id = if let Some(target) = &self.target {
                self.gpu
                    .renderer
                    .write()
                    .update_egui_texture_from_wgpu_texture(
                        &self.gpu.device,
                        &view,
                        wgpu::FilterMode::Linear,
                        target.id,
                    );
                target.id
            } else {
                self.gpu.renderer.write().register_native_texture(
                    &self.gpu.device,
                    &view,
                    wgpu::FilterMode::Linear,
                )
            };
            self.target = Some(Target {
                view,
                depth,
                id,
                size: resolution,
            });
        }
        let target = self.target.as_ref().unwrap();
        let response =
            ui.add(egui::Image::new((target.id, size)).sense(egui::Sense::click_and_drag()));
        self.ui_rects.insert(
            "Viewport".into(),
            [
                response.rect.min.x,
                response.rect.min.y,
                response.rect.width(),
                response.rect.height(),
            ],
        );
        self.mouse(ui, &response);
        let target = self.target.as_ref().unwrap();
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        let rgba = self
            .state
            .active_background_color()
            .unwrap_or([0., 0., 0., 1.]);
        let background = wgpu::Color {
            r: rgba[0] as f64,
            g: rgba[1] as f64,
            b: rgba[2] as f64,
            a: rgba[3] as f64,
        };
        self.renderer.render(
            &mut encoder,
            &self.gpu.queue,
            &target.view,
            &target.depth,
            &self.camera,
            resolution,
            background,
        );
        self.gpu.queue.submit([encoder.finish()]);
    }
    fn mouse(&mut self, ui: &egui::Ui, response: &egui::Response) {
        let clicks = self.pointer_input(ui, response);
        if !(response.hovered() || self.input.is_pressed() || !clicks.is_empty()) {
            return;
        }
        if !ui.ctx().wants_keyboard_input() {
            if ui.input(|i| i.key_pressed(egui::Key::B)) {
                self.rectangle_select = true;
            }
            if ui.input(|i| i.key_pressed(egui::Key::N)) {
                self.rectangle_select = false;
            }
        }
        if self.rectangle_select && response.drag_started_by(egui::PointerButton::Primary) {
            self.rectangle_start = ui.input(|i| i.pointer.press_origin());
        }
        if let Some(start) = self.rectangle_start {
            if let Some(current) = ui.input(|i| i.pointer.interact_pos()) {
                let rect = egui::Rect::from_two_pos(start, current).intersect(response.rect);
                ui.painter().rect_stroke(
                    rect,
                    0.,
                    egui::Stroke::new(1., egui::Color32::LIGHT_BLUE),
                    egui::StrokeKind::Inside,
                );
                if response.drag_stopped_by(egui::PointerButton::Primary) {
                    self.rectangle_start = None;
                    self.select_screen_rect(response.rect, rect);
                }
            }
            return;
        }
        for (position, double) in clicks {
            if double {
                self.center_screen_atom(response.rect, position);
            } else {
                self.pick_screen_atom(response.rect, position);
            }
        }
        if ui.ctx().wants_keyboard_input() {
            return;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Home)) {
            self.fit(false);
        }
        if ui.input(|i| i.key_pressed(egui::Key::F)) {
            self.fit(true);
        }
        if ui.input(|i| i.key_pressed(egui::Key::C)) {
            if let Some(id) = self.selected {
                self.call("center_view",json!({"objId":id.get(),"selection":if self.selection.is_empty(){"*"}else{&self.selection},"zoom":false}));
            }
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.events();
        self.ui_rects.clear();
        self.agent_ui(ctx);
        if cuemol_command::advance_animation(&mut self.state, &mut self.camera).unwrap_or(false) {
            self.dirty = true;
            ctx.request_repaint();
        }
        self.shell(ctx);
        if self.playing && !self.busy {
            let now = ctx.input(|i| i.time);
            if now >= self.next_frame {
                if let Some((_, trajectory)) = &self.trajectory {
                    self.request_frame((self.frame + 1) % trajectory.frames);
                    self.next_frame = now + 1.0 / self.fps as f64;
                }
            }
        }
        if self.playing {
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(1.0 / self.fps));
        }
        self.settings_ui(ctx);
        self.render_ui(ctx);
        self.tools_ui(ctx);
        self.timeline_ui(ctx);
        self.rebuild();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| self.viewport(ui));
        self.viewport_palette(ctx);
        egui::Window::new("Mouse and keyboard").open(&mut self.show_help).show(ctx,|ui|{ui.label("Default preset: left drag rotates, right drag pans\nWheel: zoom\nShift + left drag: horizontal slab, vertical zoom\nLeft + right drag: horizontal depth, vertical roll\nTrackpad: two-finger pan, pinch zoom\nChoose Default, Coot, PyMOL or Trackpad in Settings\nDouble-click atom: center\nHome: fit scene · F: fit selection · C: center selection\nDrop structure files onto the window. Load a topology before a trajectory.\nCommands use the same tool names and JSON arguments as cuemol exec and MCP.");});
        for file in ctx.input(|i| i.raw.dropped_files.clone()) {
            if let Some(bytes) = file.bytes {
                enqueue(Event::Data(file.name, bytes.to_vec()));
            }
        }
        let state = cuemol_command::call_tool(
            &mut self.state,
            &mut self.camera,
            "get_scene_state",
            &json!({}),
        )
        .unwrap_or(Value::Null);
        let layers = ctx.memory(|memory| memory.areas().visible_layer_ids());
        let overlays: Vec<[f32; 4]> = layers
            .into_iter()
            .filter(|layer| layer.order != egui::Order::Background)
            .filter_map(|layer| egui::AreaState::load(ctx, layer.id))
            .filter(|area| area.interactable)
            .map(|area| {
                let rect = area.rect();
                [rect.min.x, rect.min.y, rect.width(), rect.height()]
            })
            .collect();
        STATUS.with(|status|*status.borrow_mut()=json!({"scene":state,"selection":self.selection,"view":cuemol_command::view_json(&self.camera),"message":self.message,"frame":self.frame,"busy":self.busy,"agent":self.agent.status(),"ui":{"overlays":overlays,"rects":self.ui_rects,"activity":self.activity.map(cuemol_ui::shell::Activity::key),"bottom":self.bottom_tab,"inspector":self.inspector.rects}}).to_string());
    }
}
