#![cfg(target_arch = "wasm32")]
mod panels;
mod trajectory;
use base64::Engine;
use cuemol_core::{Uid, Vector4};
use cuemol_desktop_core::{viewport::ViewportRenderer, DesktopAppState, GuiLoadedPath};
use cuemol_qsys::Camera;
use cuemol_render::RenderSceneSnapshot;
use eframe::{egui, egui_wgpu};
use serde_json::{json, Value};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use trajectory::BrowserTrajectory;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};

thread_local! {
    static EVENTS: RefCell<VecDeque<Event>> = RefCell::new(VecDeque::new());
    static CONTEXT: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
    static STATUS: RefCell<String> = RefCell::new(String::new());
}
enum Event {
    Source(web_sys::File),
    Data(String, Vec<u8>),
    Error(String),
    Trajectory(Uid, BrowserTrajectory),
    Frame(Uid, usize, cuemol_mdtools::TrajectoryFrame),
    Command(String, Value),
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
struct App {
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
    history: Vec<(DesktopAppState, Camera)>,
    future: Vec<(DesktopAppState, Camera)>,
    trajectory: Option<(Uid, Rc<BrowserTrajectory>)>,
    frame: usize,
    busy: bool,
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
    render_size: [u32; 2],
    render_transparent: bool,
    show_timeline: bool,
    settings_text: String,
    startup_text: String,
    tool_filter: String,
    form_tool: String,
    form_values: std::collections::BTreeMap<String, String>,
    properties: Option<(String, Option<u64>, Vec<(String, String)>)>,
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
        Ok(Self {
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
            render_size: [1200, 900],
            render_transparent: false,
            show_timeline: false,
            settings_text: String::new(),
            startup_text,
            tool_filter: String::new(),
            form_tool: String::new(),
            form_values: Default::default(),
            properties: None,
        })
    }
    fn checkpoint(&mut self) {
        self.history.push((self.state.clone(), self.camera.clone()));
        if self.history.len() > 12 {
            self.history.remove(0);
        }
        self.future.clear();
    }
    fn call(&mut self, name: &str, args: Value) {
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
            let (source, target) = if name == "undo" {
                (&mut self.history, &mut self.future)
            } else {
                (&mut self.future, &mut self.history)
            };
            if let Some((state, camera)) = source.pop() {
                target.push((
                    std::mem::replace(&mut self.state, state),
                    std::mem::replace(&mut self.camera, camera),
                ));
                self.dirty = true;
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
                    if let Some((state, camera)) = self.history.pop() {
                        self.state = state;
                        self.camera = camera;
                    }
                }
                self.message = e.to_string();
            }
        }
    }
    fn fit(&mut self, selected: bool) {
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
                            let (state, camera) = self.history.pop().unwrap();
                            self.state = state;
                            self.camera = camera;
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
    fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("CueMol3");
                ui.separator();
                if ui.button("Open…").clicked() {
                    choose_file();
                }
                if ui.button("Save scene").clicked() {
                    let result = self
                        .state
                        .set_active_view_camera(&self.camera)
                        .and_then(|_| self.state.active_scene_bytes());
                    match result {
                        Ok(bytes) => {
                            if let Err(e) =
                                download("scene.qsc", &bytes, "application/octet-stream")
                            {
                                self.message = js_error(e);
                            }
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
                if ui.button("PNG").clicked() {
                    self.export_png();
                }
                if ui.button("Render").clicked() {
                    if let Ok(settings) = cuemol_command::render_settings(&self.state) {
                        self.render_hatching =
                            settings.get("backend").is_some_and(|v| v == "umbreon_npr");
                        self.render_transparent =
                            settings.get("transparentBg").is_some_and(|v| v == true);
                        if let Ok(hatch) = cuemol_command::hatch_params(&settings, "umbreon_npr") {
                            self.render_hatch = hatch;
                        }
                    }
                    self.show_render = true;
                }
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.pdb_id)
                        .hint_text("PDB ID")
                        .desired_width(65.0),
                );
                if ui.button("Fetch").clicked() {
                    self.fetch();
                }
                if ui.button("Fit").clicked() {
                    self.fit(false);
                }
                if ui.button("Undo").clicked() {
                    self.call("undo", json!({}));
                }
                if ui.button("Redo").clicked() {
                    self.call("redo", json!({}));
                }
                ui.toggle_value(&mut self.show_console, "Commands");
                ui.toggle_value(&mut self.show_timeline, "Animation");
                if ui.button("Settings").clicked() {
                    self.settings_text = self.state.config_text();
                    self.show_settings = true;
                }
                ui.toggle_value(&mut self.show_help, "Help");
                if ui.checkbox(&mut self.dark, "Dark").changed() {
                    ctx.set_theme(if self.dark {
                        egui::ThemePreference::Dark
                    } else {
                        egui::ThemePreference::Light
                    });
                    ctx.set_visuals(if self.dark {
                        egui::Visuals::dark()
                    } else {
                        egui::Visuals::light()
                    });
                    if let Some(s) =
                        web_sys::window().and_then(|w| w.local_storage().ok().flatten())
                    {
                        let _ =
                            s.set_item("cuemol.theme", if self.dark { "dark" } else { "light" });
                    }
                }
            });
        });
    }
    fn workspace(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("workspace").default_width(280.0).show(ctx,|ui|{
            ui.heading("Workspace");
            egui::ScrollArea::vertical().id_salt("objects").max_height(ui.available_height()*0.52).show(ui,|ui|{
                for scene in self.state.scene_summaries(){
                    if ui.selectable_label(scene.active,&scene.name).clicked(){self.call("switch_scene",json!({"sceneId":scene.uid.get()}));}
                }
                if let Some(scene)=self.state.active_scene(){
                    for object in self.state.object_summaries(scene){
                        ui.horizontal(|ui|{let mut visible=object.visible;if ui.checkbox(&mut visible,"").changed(){self.call("set_visible",json!({"nodeId":object.uid.get(),"nodeType":"object","visible":visible}));}
                            if ui.selectable_label(self.selected==Some(object.uid),format!("{} · {} atoms",object.name,object.atom_count)).clicked(){self.selected=Some(object.uid);}
                            if ui.small_button("…").clicked(){self.open_properties("object",Some(object.uid.get()));}
                        });
                        ui.indent(object.uid.get(),|ui|{for renderer in self.state.renderer_summaries(object.uid).into_iter().filter(|r|r.renderer_type!="*selection"){
                            ui.horizontal(|ui|{let mut visible=renderer.visible;if ui.checkbox(&mut visible,&renderer.name).changed(){self.call("set_visible",json!({"nodeId":renderer.uid.get(),"nodeType":"renderer","visible":visible}));}
                                ui.label(&renderer.renderer_type);if ui.small_button("…").clicked(){self.open_properties("renderer",Some(renderer.uid.get()));}if ui.small_button("×").clicked(){self.call("delete_node",json!({"nodeId":renderer.uid.get(),"nodeType":"renderer"}));}
                            });
                        }});
                    }
                    let data=cuemol_command::call_tool(&mut self.state,&mut self.camera,"get_scene_state",&json!({})).unwrap_or(Value::Null);
                    for object in data["objects"].as_array().into_iter().flatten().filter(|o|o["className"]!="MolCoord"){
                        let id=object["id"].as_u64().unwrap();let name=object["name"].as_str().unwrap_or("object");
                        ui.horizontal(|ui|{let mut visible=object["visible"]==true;if ui.checkbox(&mut visible,name).changed(){self.call("set_visible",json!({"nodeId":id,"nodeType":"object","visible":visible}));}
                            if ui.small_button("Fit").clicked(){self.call("focus_node",json!({"nodeId":id,"nodeType":"object"}));}
                            if ui.small_button("…").clicked(){self.open_properties("object",Some(id));}
                        });
                        ui.indent(id,|ui|{for renderer in object["renderers"].as_array().into_iter().flatten(){let id=renderer["id"].as_u64().unwrap();ui.horizontal(|ui|{
                            let mut visible=renderer["visible"]==true;if ui.checkbox(&mut visible,renderer["name"].as_str().unwrap_or("renderer")).changed(){self.call("set_visible",json!({"nodeId":id,"nodeType":"renderer","visible":visible}));}
                            if ui.small_button("…").clicked(){self.open_properties("renderer",Some(id));}
                        });}});
                    }
                }
            });
            ui.horizontal(|ui|{if ui.button("New scene").clicked(){self.call("create_scene",json!({}));}if ui.button("Scene properties").clicked(){self.open_properties("scene",None);}});
            ui.separator();ui.label("Selection");
            if ui.add(egui::TextEdit::singleline(&mut self.selection).hint_text("chain A and resi 10:30")).changed(){self.dirty=true;}
            ui.horizontal(|ui|{if ui.button("Center").clicked(){if let Some(id)=self.selected{self.call("center_view",json!({"objId":id.get(),"selection":if self.selection.is_empty(){"*"}else{&self.selection},"zoom":false}));}}
                if ui.button("Fit selection").clicked(){self.fit(true);}if ui.button("Clear").clicked(){self.selection.clear();self.dirty=true;}});
            egui::ComboBox::from_label("Representation").selected_text(&self.representation).show_ui(ui,|ui|{for kind in cuemol_desktop_core::MOLECULE_RENDERER_TYPES{ui.selectable_value(&mut self.representation,kind.to_string(),*kind);}});
            if ui.button("Add representation").clicked(){if let Some(id)=self.selected{self.call("create_renderer",json!({"objId":id.get(),"rendererType":self.representation,"selection":if self.selection.is_empty(){"*"}else{&self.selection}}));}}
            egui::ComboBox::from_label("Color").selected_text(&self.coloring).show_ui(ui,|ui|{for kind in ["cpk","rainbow","bfac"]{ui.selectable_value(&mut self.coloring,kind.into(),kind);}});
            if ui.button("Apply color").clicked(){if let Some(id)=self.selected{self.call("set_renderer_coloring",json!({"nodeId":id.get(),"nodeType":"object","mode":format!("paint-type-{}",self.coloring)}));}}
            ui.separator();
            let mut perspective=self.camera.is_perspective();if ui.checkbox(&mut perspective,"Perspective").changed(){self.camera.set_perspective(perspective);}
            let mut slab=self.camera.slab_depth();if ui.add(egui::Slider::new(&mut slab,0.1..=self.camera.cam_dist()*2.0).text("Slab").logarithmic(true)).changed(){self.camera.set_slab_depth(slab);}
            ui.collapsing("Sequence",|ui|{for row in self.state.active_sequence_summaries(){ui.horizontal_wrapped(|ui|{if ui.button(format!("{} / {}",row.object_name,row.chain_id)).clicked(){self.selected=Some(row.object_uid);self.selection=format!("chain {}",row.chain_id);self.dirty=true;}ui.monospace(row.sequence);});}});
            ui.collapsing("Cameras",|ui|{let cameras=cuemol_command::call_tool(&mut self.state,&mut self.camera,"list_cameras",&json!({})).unwrap_or(Value::Null);for camera in cameras.as_array().or_else(||cameras["cameras"].as_array()).into_iter().flatten(){if let Some(name)=camera["name"].as_str(){if ui.button(name).clicked(){self.call("apply_camera",json!({"name":name}));}}}if ui.button("Save current camera…").clicked(){self.select_tool("save_camera");}});

        });
    }
    fn export_png(&mut self) {
        match cuemol_command::call_tool(
            &mut self.state,
            &mut self.camera,
            "get_screenshot",
            &json!({"width":self.render_size[0],"height":self.render_size[1]}),
        ) {
            Ok(value) => {
                let data = value["_image"]["data"]
                    .as_str()
                    .or_else(|| value["_image"].as_str())
                    .unwrap_or("");
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(data) {
                    Ok(bytes) => {
                        if let Err(e) = download("cuemol.png", &bytes, "image/png") {
                            self.message = js_error(e);
                        }
                    }
                    Err(e) => self.message = e.to_string(),
                }
            }
            Err(e) => self.message = e.to_string(),
        }
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
        if !(response.hovered() || response.dragged()) {
            return;
        }
        let (delta, modifiers, scrolls) = ui.input(|input| {
            let scrolls = input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::MouseWheel {
                        delta, modifiers, ..
                    } => Some((if delta.y != 0.0 { delta.y } else { delta.x }, *modifiers)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (input.pointer.delta(), input.modifiers, scrolls)
        });
        let q = self.camera.rotation();
        let mut rotation =
            glam::Quat::from_xyzw(q.x as f32, q.y as f32, q.z as f32, q.w as f32).normalize();
        let center = self.camera.center();
        let mut target = glam::Vec3::new(center.x as f32, center.y as f32, center.z as f32);
        if response.dragged_by(egui::PointerButton::Secondary) && modifiers.shift {
            self.camera.set_slab_depth(
                (self.camera.slab_depth() * (delta.y as f64 * 0.01).exp())
                    .clamp(0.1, self.camera.cam_dist() * 2.0),
            );
        } else if response.dragged_by(egui::PointerButton::Secondary) && modifiers.ctrl {
            target += rotation * glam::Vec3::Z * delta.y * self.camera.zoom() as f32
                / response.rect.height();
        } else if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged() && modifiers.shift)
        {
            target +=
                rotation * glam::Vec3::new(-delta.x, delta.y, 0.0) * self.camera.zoom() as f32
                    / response.rect.height();
        } else if response.dragged_by(egui::PointerButton::Primary) {
            rotation = (glam::Quat::from_rotation_x(delta.y * 0.008)
                * glam::Quat::from_rotation_y(delta.x * 0.008)
                * rotation)
                .normalize();
        }
        for (scroll, modifiers) in scrolls {
            if modifiers.shift {
                self.camera.set_slab_depth(
                    (self.camera.slab_depth() * (scroll as f64 * 0.008).exp())
                        .clamp(0.1, self.camera.cam_dist() * 2.0),
                );
            } else if modifiers.ctrl {
                target += rotation * glam::Vec3::Z * scroll * self.camera.zoom() as f32 * 0.001;
            } else {
                self.camera.set_zoom(
                    (self.camera.zoom() * (-scroll as f64 * 0.003).exp()).clamp(0.1, 100000.0),
                );
            }
        }
        self.camera.set_center(Vector4::new3(
            target.x as f64,
            target.y as f64,
            target.z as f64,
        ));
        self.camera.set_rotation(cuemol_core::Quat::new(
            rotation.x as f64,
            rotation.y as f64,
            rotation.z as f64,
            rotation.w as f64,
        ));
        if response.double_clicked() {
            if let (Some(pos), Some(snapshot)) = (response.interact_pointer_pos(), &self.snapshot) {
                let mvp =
                    glam::Mat4::from_cols_array_2d(&cuemol_desktop_core::offscreen::camera_mvp(
                        &self.camera,
                        response.rect.aspect_ratio() as f64,
                    ));
                let nearest = snapshot
                    .atoms
                    .iter()
                    .filter_map(|a| {
                        let p = mvp
                            * glam::Vec4::new(
                                a.center.x as f32,
                                a.center.y as f32,
                                a.center.z as f32,
                                1.0,
                            );
                        if p.w <= 0.0 {
                            return None;
                        }
                        let ndc = p.truncate() / p.w;
                        if !(0.0..=1.0).contains(&ndc.z) {
                            return None;
                        }
                        let screen = egui::pos2(
                            response.rect.left() + (ndc.x + 1.0) * 0.5 * response.rect.width(),
                            response.rect.top() + (1.0 - ndc.y) * 0.5 * response.rect.height(),
                        );
                        let d = screen.distance(pos);
                        (d < 18.0).then_some((d, a.center))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, position)) = nearest {
                    self.camera.set_center(position);
                }
            }
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
        if cuemol_command::advance_animation(&mut self.state, &mut self.camera).unwrap_or(false) {
            self.dirty = true;
            ctx.request_repaint();
        }
        self.toolbar(ctx);
        self.workspace(ctx);
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            if let Some((_, trajectory)) = &self.trajectory {
                let frames = trajectory.frames;
                ui.horizontal(|ui| {
                    let label = if self.playing { "Pause" } else { "Play" };
                    ui.toggle_value(&mut self.playing, label);
                    let mut frame = self.frame;
                    if ui
                        .add(egui::Slider::new(&mut frame, 0..=frames - 1).text("Frame"))
                        .changed()
                    {
                        self.request_frame(frame);
                    }
                    ui.add(
                        egui::DragValue::new(&mut self.fps)
                            .range(1.0..=120.0)
                            .suffix(" fps"),
                    );
                    if self.busy {
                        ui.spinner();
                    }
                });
            }
            egui::ScrollArea::vertical()
                .max_height(if self.show_console { 150.0 } else { 45.0 })
                .show(ui, |ui| {
                    ui.monospace(&self.message);
                });
        });
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
        self.properties_ui(ctx);
        self.timeline_ui(ctx);
        self.rebuild();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| self.viewport(ui));
        egui::Window::new("Mouse and keyboard").open(&mut self.show_help).show(ctx,|ui|{ui.label("Drag: rotate\nRight or middle drag: pan\nWheel: zoom\nShift + wheel or right drag: slab\nCtrl + wheel or right drag: view depth\nDouble-click atom: center\nHome: fit scene · F: fit selection · C: center selection\nDrop structure files onto the window. Load a topology before a trajectory.\nCommands use the same tool names and JSON arguments as cuemol exec and MCP.");});
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
        STATUS.with(|status|*status.borrow_mut()=json!({"scene":state,"view":cuemol_command::view_json(&self.camera),"message":self.message,"frame":self.frame,"busy":self.busy}).to_string());
    }
}
