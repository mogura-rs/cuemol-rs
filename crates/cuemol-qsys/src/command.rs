use crate::{InOutHandler, RendererFactory, RendererInstance, SceneManager};
use cuemol_core::{CueMolError, Result, Uid};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum CommandValue {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Uid(Uid),
    None,
}

impl CommandValue {
    fn as_bool(&self, name: &str) -> Result<bool> {
        match self {
            Self::Bool(value) => Ok(*value),
            _ => Err(CueMolError::invalid_argument(format!(
                "{name} expects a boolean"
            ))),
        }
    }

    fn as_u64(&self, name: &str) -> Result<u64> {
        match self {
            Self::UInt(value) => Ok(*value),
            Self::Int(value) if *value >= 0 => Ok(*value as u64),
            Self::Uid(value) => Ok(value.get()),
            _ => Err(CueMolError::invalid_argument(format!(
                "{name} expects an unsigned integer"
            ))),
        }
    }

    fn as_uid(&self, name: &str) -> Result<Uid> {
        match self {
            Self::Uid(value) => Ok(*value),
            Self::UInt(value) => Ok(Uid::new(*value)),
            Self::Int(value) if *value >= 0 => Ok(Uid::new(*value as u64)),
            _ => Err(CueMolError::invalid_argument(format!(
                "{name} expects a uid"
            ))),
        }
    }

    fn as_string(&self, name: &str) -> Result<String> {
        match self {
            Self::String(value) => Ok(value.clone()),
            _ => Err(CueMolError::invalid_argument(format!(
                "{name} expects a string"
            ))),
        }
    }
}

impl From<bool> for CommandValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for CommandValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<u64> for CommandValue {
    fn from(value: u64) -> Self {
        Self::UInt(value)
    }
}

impl From<f64> for CommandValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<String> for CommandValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for CommandValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<Uid> for CommandValue {
    fn from(value: Uid) -> Self {
        Self::Uid(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedRenderer {
    uid: Uid,
    target_object: Uid,
    type_name: String,
    name: String,
    default_style_name: String,
}

impl CreatedRenderer {
    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn target_object(&self) -> Uid {
        self.target_object
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn default_style_name(&self) -> &str {
        &self.default_style_name
    }
}

#[derive(Debug, Clone)]
pub struct CommandContext<T> {
    scene_manager: SceneManager<T>,
    renderer_factory: RendererFactory,
    handlers: Vec<InOutHandler>,
    renderers: BTreeMap<Uid, CreatedRenderer>,
}

impl<T> Default for CommandContext<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> CommandContext<T> {
    pub fn new() -> Self {
        Self {
            scene_manager: SceneManager::new(),
            renderer_factory: RendererFactory::new(),
            handlers: Vec::new(),
            renderers: BTreeMap::new(),
        }
    }

    pub fn scene_manager(&self) -> &SceneManager<T> {
        &self.scene_manager
    }

    pub fn scene_manager_mut(&mut self) -> &mut SceneManager<T> {
        &mut self.scene_manager
    }

    pub fn renderer_factory(&self) -> &RendererFactory {
        &self.renderer_factory
    }

    pub fn renderer_factory_mut(&mut self) -> &mut RendererFactory {
        &mut self.renderer_factory
    }

    pub fn register_handler(&mut self, handler: InOutHandler) -> bool {
        if self.handlers.iter().any(|existing| {
            existing.name() == handler.name() && existing.category() == handler.category()
        }) {
            return false;
        }
        self.handlers.push(handler);
        true
    }

    pub fn handlers(&self) -> &[InOutHandler] {
        &self.handlers
    }

    pub fn create_handler(&self, name: &str, category: i32) -> Option<InOutHandler> {
        self.handlers
            .iter()
            .find(|handler| handler.name() == name && handler.category() == category)
            .cloned()
    }

    pub fn renderer(&self, uid: Uid) -> Option<&CreatedRenderer> {
        self.renderers.get(&uid)
    }

    pub fn renderer_count(&self) -> usize {
        self.renderers.len()
    }

    fn object_exists(&self, object_uid: Uid) -> bool {
        self.scene_manager
            .scene_uids()
            .into_iter()
            .any(|scene_uid| {
                self.scene_manager
                    .scene(scene_uid)
                    .is_some_and(|scene| scene.object(object_uid).is_some())
            })
    }

    fn add_renderer(
        &mut self,
        instance: RendererInstance,
        target_object: Uid,
        name: String,
        default_style_name: String,
    ) -> Uid {
        let uid = instance.uid();
        self.renderers.insert(
            uid,
            CreatedRenderer {
                uid,
                target_object,
                type_name: instance.type_name().to_string(),
                name,
                default_style_name,
            },
        );
        uid
    }

    fn guess_format_by_extension(
        &self,
        path: &str,
        category: i32,
        candidates: Option<&str>,
    ) -> String {
        let wanted = candidates
            .unwrap_or("")
            .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        let filtered = !wanted.is_empty();
        let path_lower = path.to_ascii_lowercase();
        self.handlers
            .iter()
            .filter(|handler| handler.category() == category)
            .filter(|handler| !filtered || wanted.iter().any(|name| *name == handler.name()))
            .find(|handler| extension_matches(&path_lower, handler.file_ext()))
            .map(|handler| handler.name().to_string())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSceneCommand {
    uid: Uid,
    scene_name: String,
    create_view: bool,
    activate: bool,
    result_scene: Option<Uid>,
    result_view: Option<Uid>,
}

impl Default for NewSceneCommand {
    fn default() -> Self {
        Self {
            uid: Uid::new(1),
            scene_name: String::new(),
            create_view: true,
            activate: false,
            result_scene: None,
            result_view: None,
        }
    }
}

impl NewSceneCommand {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(&self) -> &'static str {
        "new_scene"
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn scene_name(&self) -> &str {
        &self.scene_name
    }

    pub fn set_scene_name(&mut self, value: impl Into<String>) {
        self.scene_name = value.into();
    }

    pub fn create_view(&self) -> bool {
        self.create_view
    }

    pub fn set_create_view(&mut self, value: bool) {
        self.create_view = value;
    }

    pub fn activate(&self) -> bool {
        self.activate
    }

    pub fn set_activate(&mut self, value: bool) {
        self.activate = value;
    }

    pub fn result_scene(&self) -> Option<Uid> {
        self.result_scene
    }

    pub fn result_view(&self) -> Option<Uid> {
        self.result_view
    }

    fn reset_outputs(&mut self) {
        self.result_scene = None;
        self.result_view = None;
    }

    fn run<T>(&mut self, context: &mut CommandContext<T>) -> Result<()> {
        let scene_name = if self.scene_name.is_empty() {
            generate_new_scene_name(context.scene_manager())
        } else {
            self.scene_name.clone()
        };
        let scene_uid = context.scene_manager_mut().create_scene(scene_name);
        self.result_scene = Some(scene_uid);

        if self.create_view {
            let scene = context
                .scene_manager_mut()
                .scene_mut(scene_uid)
                .ok_or_else(|| CueMolError::NotFound("created scene was not registered".into()))?;
            let view_uid = scene.create_view();
            if let Some(view) = scene.view_mut(view_uid) {
                view.set_name("0");
            }
            self.result_view = Some(view_uid);
            if self.activate {
                scene.set_active_view_uid(Some(view_uid));
            }
        }

        if self.activate {
            context
                .scene_manager_mut()
                .set_active_scene_uid(Some(scene_uid));
        }
        Ok(())
    }

    fn set_arg(&mut self, name: &str, value: CommandValue) -> Result<()> {
        match name {
            "scene_name" => self.scene_name = value.as_string(name)?,
            "create_view" => self.create_view = value.as_bool(name)?,
            "activate" => self.activate = value.as_bool(name)?,
            _ => return unknown_argument(name),
        }
        Ok(())
    }

    fn outputs(&self) -> BTreeMap<String, CommandValue> {
        let mut output = BTreeMap::new();
        output.insert(
            "result_scene".to_string(),
            self.result_scene
                .map_or(CommandValue::None, CommandValue::Uid),
        );
        output.insert(
            "result_view".to_string(),
            self.result_view
                .map_or(CommandValue::None, CommandValue::Uid),
        );
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRendererCommand {
    uid: Uid,
    target_object: Option<Uid>,
    renderer_type: String,
    renderer_name: String,
    recenter_view: bool,
    default_style_name: String,
    result_renderer: Option<Uid>,
}

impl Default for NewRendererCommand {
    fn default() -> Self {
        Self {
            uid: Uid::new(2),
            target_object: None,
            renderer_type: String::new(),
            renderer_name: String::new(),
            recenter_view: false,
            default_style_name: String::new(),
            result_renderer: None,
        }
    }
}

impl NewRendererCommand {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(&self) -> &'static str {
        "new_renderer"
    }

    pub fn target_object(&self) -> Option<Uid> {
        self.target_object
    }

    pub fn set_target_object(&mut self, value: Uid) {
        self.target_object = Some(value);
    }

    pub fn renderer_type(&self) -> &str {
        &self.renderer_type
    }

    pub fn set_renderer_type(&mut self, value: impl Into<String>) {
        self.renderer_type = value.into();
    }

    pub fn renderer_name(&self) -> &str {
        &self.renderer_name
    }

    pub fn set_renderer_name(&mut self, value: impl Into<String>) {
        self.renderer_name = value.into();
    }

    pub fn recenter_view(&self) -> bool {
        self.recenter_view
    }

    pub fn set_recenter_view(&mut self, value: bool) {
        self.recenter_view = value;
    }

    pub fn default_style_name(&self) -> &str {
        &self.default_style_name
    }

    pub fn set_default_style_name(&mut self, value: impl Into<String>) {
        self.default_style_name = value.into();
    }

    pub fn result_renderer(&self) -> Option<Uid> {
        self.result_renderer
    }

    fn reset_outputs(&mut self) {
        self.result_renderer = None;
    }

    fn run<T>(&mut self, context: &mut CommandContext<T>) -> Result<()> {
        let target_object = self
            .target_object
            .ok_or_else(|| CueMolError::invalid_argument("target_object is required"))?;
        if !context.object_exists(target_object) {
            return Err(CueMolError::NotFound(format!(
                "target object {} was not found",
                target_object.get()
            )));
        }
        let instance = context.renderer_factory_mut().create(&self.renderer_type)?;
        let uid = context.add_renderer(
            instance,
            target_object,
            self.renderer_name.clone(),
            self.default_style_name.clone(),
        );
        self.result_renderer = Some(uid);
        Ok(())
    }

    fn set_arg(&mut self, name: &str, value: CommandValue) -> Result<()> {
        match name {
            "target_object" => self.target_object = Some(value.as_uid(name)?),
            "renderer_type" => self.renderer_type = value.as_string(name)?,
            "renderer_name" => self.renderer_name = value.as_string(name)?,
            "recenter_view" => self.recenter_view = value.as_bool(name)?,
            "default_style_name" => self.default_style_name = value.as_string(name)?,
            _ => return unknown_argument(name),
        }
        Ok(())
    }

    fn outputs(&self) -> BTreeMap<String, CommandValue> {
        let mut output = BTreeMap::new();
        output.insert(
            "result_renderer".to_string(),
            self.result_renderer
                .map_or(CommandValue::None, CommandValue::Uid),
        );
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadObjectCommand {
    uid: Uid,
    target_scene: Option<Uid>,
    file_path: String,
    object_name: String,
    file_format: String,
    content_first: bool,
    max_sniff_bytes: u64,
    result_object: Option<Uid>,
}

impl Default for LoadObjectCommand {
    fn default() -> Self {
        Self {
            uid: Uid::new(3),
            target_scene: None,
            file_path: String::new(),
            object_name: String::new(),
            file_format: String::new(),
            content_first: false,
            max_sniff_bytes: 0,
            result_object: None,
        }
    }
}

impl LoadObjectCommand {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(&self) -> &'static str {
        "load_object"
    }

    pub fn set_target_scene(&mut self, value: Uid) {
        self.target_scene = Some(value);
    }

    pub fn target_scene(&self) -> Option<Uid> {
        self.target_scene
    }

    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    pub fn set_file_path(&mut self, value: impl Into<String>) {
        self.file_path = value.into();
    }

    pub fn object_name(&self) -> &str {
        &self.object_name
    }

    pub fn set_object_name(&mut self, value: impl Into<String>) {
        self.object_name = value.into();
    }

    pub fn file_format(&self) -> &str {
        &self.file_format
    }

    pub fn set_file_format(&mut self, value: impl Into<String>) {
        self.file_format = value.into();
    }

    pub fn content_first(&self) -> bool {
        self.content_first
    }

    pub fn set_content_first(&mut self, value: bool) {
        self.content_first = value;
    }

    pub fn max_sniff_bytes(&self) -> u64 {
        self.max_sniff_bytes
    }

    pub fn set_max_sniff_bytes(&mut self, value: u64) {
        self.max_sniff_bytes = value;
    }

    pub fn result_object(&self) -> Option<Uid> {
        self.result_object
    }

    fn reset_outputs(&mut self) {
        self.result_object = None;
    }

    fn run<T: Default>(&mut self, context: &mut CommandContext<T>) -> Result<()> {
        let scene_uid = self
            .target_scene
            .ok_or_else(|| CueMolError::invalid_argument("target_scene is required"))?;

        if self.file_format.is_empty() {
            self.file_format = context.guess_format_by_extension(
                &self.file_path,
                InOutHandler::IOH_CAT_OBJREADER,
                None,
            );
            if self.file_format.is_empty() {
                return Err(CueMolError::invalid_argument("cannot guess file type"));
            }
        }
        let object_name = if self.object_name.is_empty() {
            default_object_name(&self.file_path)
        } else {
            self.object_name.clone()
        };

        let scene = context
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .ok_or_else(|| CueMolError::NotFound("target scene was not found".into()))?;
        let object_uid = scene.add_object(object_name, T::default());
        if let Some(object) = scene.object_mut(object_uid) {
            object.set_source(self.file_path.clone());
            object.set_source_type(self.file_format.clone());
        }
        self.result_object = Some(object_uid);
        Ok(())
    }

    fn set_arg(&mut self, name: &str, value: CommandValue) -> Result<()> {
        match name {
            "target_scene" => self.target_scene = Some(value.as_uid(name)?),
            "file_path" => self.file_path = value.as_string(name)?,
            "object_name" => self.object_name = value.as_string(name)?,
            "file_format" => self.file_format = value.as_string(name)?,
            "content_first" => self.content_first = value.as_bool(name)?,
            "max_sniff_bytes" => self.max_sniff_bytes = value.as_u64(name)?,
            _ => return unknown_argument(name),
        }
        Ok(())
    }

    fn outputs(&self) -> BTreeMap<String, CommandValue> {
        let mut output = BTreeMap::new();
        output.insert(
            "result_object".to_string(),
            self.result_object
                .map_or(CommandValue::None, CommandValue::Uid),
        );
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadSceneCommand {
    uid: Uid,
    file_path: String,
    scene_name: String,
    file_format: String,
    target_scene: Option<Uid>,
    set_camera: bool,
    result_scene: Option<Uid>,
}

impl Default for LoadSceneCommand {
    fn default() -> Self {
        Self {
            uid: Uid::new(4),
            file_path: String::new(),
            scene_name: String::new(),
            file_format: String::new(),
            target_scene: None,
            set_camera: true,
            result_scene: None,
        }
    }
}

impl LoadSceneCommand {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(&self) -> &'static str {
        "load_scene"
    }

    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    pub fn set_file_path(&mut self, value: impl Into<String>) {
        self.file_path = value.into();
    }

    pub fn scene_name(&self) -> &str {
        &self.scene_name
    }

    pub fn set_scene_name(&mut self, value: impl Into<String>) {
        self.scene_name = value.into();
    }

    pub fn file_format(&self) -> &str {
        &self.file_format
    }

    pub fn set_file_format(&mut self, value: impl Into<String>) {
        self.file_format = value.into();
    }

    pub fn target_scene(&self) -> Option<Uid> {
        self.target_scene
    }

    pub fn set_target_scene(&mut self, value: Uid) {
        self.target_scene = Some(value);
    }

    pub fn set_camera(&self) -> bool {
        self.set_camera
    }

    pub fn set_set_camera(&mut self, value: bool) {
        self.set_camera = value;
    }

    pub fn result_scene(&self) -> Option<Uid> {
        self.result_scene
    }

    fn reset_outputs(&mut self) {
        self.result_scene = None;
    }

    fn run<T>(&mut self, context: &mut CommandContext<T>) -> Result<()> {
        if self.file_format.is_empty() {
            self.file_format = context.guess_format_by_extension(
                &self.file_path,
                InOutHandler::IOH_CAT_SCEREADER,
                None,
            );
            if self.file_format.is_empty() {
                return Err(CueMolError::invalid_argument("cannot guess file type"));
            }
        }

        let scene_uid = match self.target_scene {
            Some(uid) => {
                if context.scene_manager().scene(uid).is_none() {
                    return Err(CueMolError::NotFound("target scene was not found".into()));
                }
                uid
            }
            None => {
                let name = if self.scene_name.is_empty() {
                    default_object_name(&self.file_path)
                } else {
                    self.scene_name.clone()
                };
                context.scene_manager_mut().create_scene(name)
            }
        };

        let scene = context
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .ok_or_else(|| CueMolError::NotFound("result scene was not found".into()))?;
        if !self.scene_name.is_empty() {
            scene.set_name(self.scene_name.clone());
        }
        scene.set_source(self.file_path.clone());
        scene.set_source_type(self.file_format.clone());
        scene.set_base_path(default_base_path(&self.file_path));
        if self.set_camera {
            for view_uid in scene.view_uids() {
                scene.load_view_from_camera(view_uid, "__current");
            }
        }
        self.result_scene = Some(scene_uid);
        Ok(())
    }

    fn set_arg(&mut self, name: &str, value: CommandValue) -> Result<()> {
        match name {
            "file_path" => self.file_path = value.as_string(name)?,
            "scene_name" => self.scene_name = value.as_string(name)?,
            "file_format" => self.file_format = value.as_string(name)?,
            "target_scene" => self.target_scene = Some(value.as_uid(name)?),
            "set_camera" => self.set_camera = value.as_bool(name)?,
            _ => return unknown_argument(name),
        }
        Ok(())
    }

    fn outputs(&self) -> BTreeMap<String, CommandValue> {
        let mut output = BTreeMap::new();
        output.insert(
            "result_scene".to_string(),
            self.result_scene
                .map_or(CommandValue::None, CommandValue::Uid),
        );
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandInstance {
    NewScene(NewSceneCommand),
    NewRenderer(NewRendererCommand),
    LoadObject(LoadObjectCommand),
    LoadScene(LoadSceneCommand),
}

impl CommandInstance {
    pub fn name(&self) -> &'static str {
        match self {
            Self::NewScene(command) => command.name(),
            Self::NewRenderer(command) => command.name(),
            Self::LoadObject(command) => command.name(),
            Self::LoadScene(command) => command.name(),
        }
    }

    pub fn uid(&self) -> Uid {
        match self {
            Self::NewScene(command) => command.uid,
            Self::NewRenderer(command) => command.uid,
            Self::LoadObject(command) => command.uid,
            Self::LoadScene(command) => command.uid,
        }
    }

    pub fn to_command_string(&self) -> String {
        self.name().to_string()
    }

    pub fn reset_outputs(&mut self) {
        match self {
            Self::NewScene(command) => command.reset_outputs(),
            Self::NewRenderer(command) => command.reset_outputs(),
            Self::LoadObject(command) => command.reset_outputs(),
            Self::LoadScene(command) => command.reset_outputs(),
        }
    }

    pub fn set_arg(&mut self, name: &str, value: CommandValue) -> Result<()> {
        match self {
            Self::NewScene(command) => command.set_arg(name, value),
            Self::NewRenderer(command) => command.set_arg(name, value),
            Self::LoadObject(command) => command.set_arg(name, value),
            Self::LoadScene(command) => command.set_arg(name, value),
        }
    }

    pub fn run<T: Default>(&mut self, context: &mut CommandContext<T>) -> Result<()> {
        self.reset_outputs();
        match self {
            Self::NewScene(command) => command.run(context),
            Self::NewRenderer(command) => command.run(context),
            Self::LoadObject(command) => command.run(context),
            Self::LoadScene(command) => command.run(context),
        }
    }

    pub fn outputs(&self) -> BTreeMap<String, CommandValue> {
        match self {
            Self::NewScene(command) => command.outputs(),
            Self::NewRenderer(command) => command.outputs(),
            Self::LoadObject(command) => command.outputs(),
            Self::LoadScene(command) => command.outputs(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmdMgr {
    commands: BTreeMap<String, CommandInstance>,
}

impl Default for CmdMgr {
    fn default() -> Self {
        let mut manager = Self {
            commands: BTreeMap::new(),
        };
        manager.register(CommandInstance::NewScene(NewSceneCommand::new()));
        manager.register(CommandInstance::NewRenderer(NewRendererCommand::new()));
        manager.register(CommandInstance::LoadObject(LoadObjectCommand::new()));
        manager.register(CommandInstance::LoadScene(LoadSceneCommand::new()));
        manager
    }
}

impl CmdMgr {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, command: CommandInstance) -> bool {
        self.commands
            .insert(command.name().to_string(), command)
            .is_none()
    }

    pub fn unregister(&mut self, name: &str) -> bool {
        self.commands.remove(name).is_some()
    }

    pub fn get_cmd(&self, name: &str) -> Option<CommandInstance> {
        self.commands.get(name).cloned()
    }

    pub fn run_cmd<T: Default>(
        &self,
        name: &str,
        context: &mut CommandContext<T>,
    ) -> Result<CommandInstance> {
        let mut command = self
            .get_cmd(name)
            .ok_or_else(|| CueMolError::NotFound(format!("command not found: {name}")))?;
        command.run(context)?;
        Ok(command)
    }

    pub fn run_cmd_args<T: Default>(
        &self,
        name: &str,
        args: BTreeMap<String, CommandValue>,
        context: &mut CommandContext<T>,
    ) -> Result<BTreeMap<String, CommandValue>> {
        let mut command = self
            .get_cmd(name)
            .ok_or_else(|| CueMolError::NotFound(format!("command not found: {name}")))?;
        for (key, value) in args {
            command.set_arg(&key, value)?;
        }
        command.run(context)?;
        Ok(command.outputs())
    }

    pub fn command_names(&self) -> Vec<&str> {
        self.commands.keys().map(String::as_str).collect()
    }
}

fn generate_new_scene_name<T>(manager: &SceneManager<T>) -> String {
    for index in 1.. {
        let name = format!("Untitled {index}");
        if manager.scene_by_name(&name).is_none() {
            return name;
        }
    }
    unreachable!()
}

fn default_object_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn default_base_path(path: &str) -> String {
    Path::new(path)
        .parent()
        .and_then(|parent| parent.to_str())
        .unwrap_or("")
        .to_string()
}

fn extension_matches(path_lower: &str, file_ext: &str) -> bool {
    file_ext
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .any(|part| {
            let suffix = part.strip_prefix('*').unwrap_or(part).to_ascii_lowercase();
            !suffix.is_empty() && path_lower.ends_with(&suffix)
        })
}

fn unknown_argument<T>(name: &str) -> Result<T> {
    Err(CueMolError::invalid_argument(format!(
        "unknown command argument: {name}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler(name: &str, descr: &str, ext: &str, category: i32) -> InOutHandler {
        InOutHandler::new(name, descr, ext, category)
    }

    #[test]
    fn command_manager_registers_reference_commands() {
        let manager = CmdMgr::new();
        assert!(manager.get_cmd("new_scene").is_some());
        assert!(manager.get_cmd("new_renderer").is_some());
        assert!(manager.get_cmd("load_object").is_some());
        assert!(manager.get_cmd("load_scene").is_some());
        assert!(manager.get_cmd("__missing__").is_none());
    }

    #[test]
    fn new_scene_command_matches_reference_defaults_and_results() {
        let manager = CmdMgr::new();
        let mut context = CommandContext::<()>::new();
        let command = manager.run_cmd("new_scene", &mut context).unwrap();
        assert_eq!(command.name(), "new_scene");
        assert_eq!(context.scene_manager().scene_count(), 1);
        let scene_uid = match command.outputs().get("result_scene") {
            Some(CommandValue::Uid(uid)) => *uid,
            other => panic!("unexpected result_scene: {other:?}"),
        };
        let scene = context.scene_manager().scene(scene_uid).unwrap();
        assert_eq!(scene.name(), "Untitled 1");
        assert_eq!(scene.view_count(), 1);
        assert_eq!(scene.view_by_name("0").unwrap().name(), "0");
    }

    #[test]
    fn load_object_command_guesses_format_and_adds_object() {
        let mut context = CommandContext::<()>::new();
        context.register_handler(handler(
            "pdb",
            "PDB Coordinates (*.pdb;*.ent;*.pdb.gz)",
            "*.pdb; *.ent; *.pdb.gz",
            InOutHandler::IOH_CAT_OBJREADER,
        ));
        let scene_uid = context.scene_manager_mut().create_scene("target");
        let manager = CmdMgr::new();
        let mut args = BTreeMap::new();
        args.insert("target_scene".to_string(), CommandValue::Uid(scene_uid));
        args.insert(
            "file_path".to_string(),
            CommandValue::String("/tmp/1CRN.pdb.gz".to_string()),
        );
        let output = manager
            .run_cmd_args("load_object", args, &mut context)
            .unwrap();
        let object_uid = match output.get("result_object") {
            Some(CommandValue::Uid(uid)) => *uid,
            other => panic!("unexpected result_object: {other:?}"),
        };
        let scene = context.scene_manager().scene(scene_uid).unwrap();
        let object = scene.object(object_uid).unwrap();
        assert_eq!(object.name(), "1CRN.pdb");
        assert_eq!(object.source(), "/tmp/1CRN.pdb.gz");
        assert_eq!(object.source_type(), "pdb");
    }

    #[test]
    fn load_scene_command_sets_source_metadata() {
        let mut context = CommandContext::<()>::new();
        context.register_handler(handler(
            "qsc_xml",
            "CueMol Scene (*.qsc;*.qsl)",
            "*.qsc; *.qsl",
            InOutHandler::IOH_CAT_SCEREADER,
        ));
        let manager = CmdMgr::new();
        let mut args = BTreeMap::new();
        args.insert(
            "file_path".to_string(),
            CommandValue::String("/tmp/test.qsc".to_string()),
        );
        args.insert(
            "scene_name".to_string(),
            CommandValue::String("loaded".to_string()),
        );
        let output = manager
            .run_cmd_args("load_scene", args, &mut context)
            .unwrap();
        let scene_uid = match output.get("result_scene") {
            Some(CommandValue::Uid(uid)) => *uid,
            other => panic!("unexpected result_scene: {other:?}"),
        };
        let scene = context.scene_manager().scene(scene_uid).unwrap();
        assert_eq!(scene.name(), "loaded");
        assert_eq!(scene.source(), "/tmp/test.qsc");
        assert_eq!(scene.source_type(), "qsc_xml");
        assert_eq!(scene.base_path(), "/tmp");
    }

    #[test]
    fn new_renderer_command_uses_factory_and_records_result() {
        let mut context = CommandContext::<()>::new();
        context.renderer_factory_mut().register("ballstick", false);
        let scene_uid = context.scene_manager_mut().create_scene("target");
        let object_uid = context
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_object("obj", ());
        let manager = CmdMgr::new();
        let mut args = BTreeMap::new();
        args.insert("target_object".to_string(), CommandValue::Uid(object_uid));
        args.insert("renderer_type".to_string(), "ballstick".into());
        args.insert("renderer_name".to_string(), "rend1".into());
        args.insert("default_style_name".to_string(), "default".into());
        let output = manager
            .run_cmd_args("new_renderer", args, &mut context)
            .unwrap();
        let renderer_uid = match output.get("result_renderer") {
            Some(CommandValue::Uid(uid)) => *uid,
            other => panic!("unexpected result_renderer: {other:?}"),
        };
        let renderer = context.renderer(renderer_uid).unwrap();
        assert_eq!(renderer.target_object(), object_uid);
        assert_eq!(renderer.type_name(), "ballstick");
        assert_eq!(renderer.name(), "rend1");
        assert_eq!(renderer.default_style_name(), "default");
    }
}
