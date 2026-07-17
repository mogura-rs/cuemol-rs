use cuemol_core::{ByteArray, LDom2Node, Result, Uid};

use crate::{Camera, InOutHandler, Scene};

pub trait SceneWriterOptions {
    fn qsc_options_node(&self) -> Option<&LDom2Node>;
}

impl SceneWriterOptions for LDom2Node {
    fn qsc_options_node(&self) -> Option<&LDom2Node> {
        Some(self)
    }
}

impl<T> SceneWriterOptions for Scene<T> {
    fn qsc_options_node(&self) -> Option<&LDom2Node> {
        self.qsc_options()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjReader {
    handler: InOutHandler,
    target_object: Option<Uid>,
    read_count: usize,
}

impl ObjReader {
    pub fn new(
        name: impl Into<String>,
        type_descr: impl Into<String>,
        file_ext: impl Into<String>,
    ) -> Self {
        Self {
            handler: InOutHandler::new(name, type_descr, file_ext, InOutHandler::IOH_CAT_OBJREADER),
            target_object: None,
            read_count: 0,
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        &self.handler
    }

    pub fn handler_mut(&mut self) -> &mut InOutHandler {
        &mut self.handler
    }

    pub fn path(&self) -> &str {
        self.handler.path()
    }

    pub fn set_path(&mut self, path: impl Into<String>) {
        self.handler.set_path(path);
    }

    pub fn path_for(&self, key: &str) -> &str {
        self.handler.path_for(key)
    }

    pub fn set_sub_path(&mut self, key: impl Into<String>, path: impl Into<String>) {
        self.handler.set_sub_path(key, path);
    }

    pub fn compress_mode(&self) -> i32 {
        self.handler.compress_mode()
    }

    pub fn set_compress_mode(&mut self, value: i32) {
        self.handler.set_compress_mode(value);
    }

    pub fn base64_flag(&self) -> bool {
        self.handler.base64_flag()
    }

    pub fn base64(&self) -> bool {
        self.base64_flag()
    }

    pub fn base_64(&self) -> bool {
        self.base64_flag()
    }

    pub fn set_base64_flag(&mut self, value: bool) {
        self.handler.set_base64_flag(value);
    }

    pub fn set_base64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn set_base_64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn category(&self) -> i32 {
        InOutHandler::IOH_CAT_OBJREADER
    }

    pub fn attach_object(&mut self, object_uid: Uid) {
        self.target_object = Some(object_uid);
    }

    pub fn attach(&mut self, object_uid: Uid) {
        self.attach_object(object_uid);
    }

    pub fn detach_object(&mut self) -> Option<Uid> {
        self.target_object.take()
    }

    pub fn detach(&mut self) -> Option<Uid> {
        self.detach_object()
    }

    pub fn target_object(&self) -> Option<Uid> {
        self.target_object
    }

    pub fn create_default_object(&self) -> Option<Uid> {
        self.target_object
            .or_else(|| Some(stable_uid_from_name(self.handler.name())))
    }

    pub fn create_default_obj(&self) -> Option<Uid> {
        self.create_default_object()
    }

    pub fn read(&mut self) {
        self.read_count += 1;
    }

    pub fn read_bytes(&mut self) -> Result<Vec<u8>> {
        let bytes = self.handler.read_bytes()?;
        self.read_count += 1;
        Ok(bytes)
    }

    pub fn read_string(&mut self) -> Result<String> {
        let text = self.handler.read_string()?;
        self.read_count += 1;
        Ok(text)
    }

    pub fn read_count(&self) -> usize {
        self.read_count
    }
}

impl Default for ObjReader {
    fn default() -> Self {
        Self::new("", "", "")
    }
}

fn stable_uid_from_name(name: &str) -> Uid {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Uid::new(hash.max(1))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjWriter {
    handler: InOutHandler,
    target_object: Option<Uid>,
    conv_to_link: bool,
    write_count: usize,
}

impl ObjWriter {
    pub fn new(
        name: impl Into<String>,
        type_descr: impl Into<String>,
        file_ext: impl Into<String>,
    ) -> Self {
        Self {
            handler: InOutHandler::new(name, type_descr, file_ext, InOutHandler::IOH_CAT_OBJWRITER),
            target_object: None,
            conv_to_link: false,
            write_count: 0,
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        &self.handler
    }

    pub fn handler_mut(&mut self) -> &mut InOutHandler {
        &mut self.handler
    }

    pub fn path(&self) -> &str {
        self.handler.path()
    }

    pub fn set_path(&mut self, path: impl Into<String>) {
        self.handler.set_path(path);
    }

    pub fn path_for(&self, key: &str) -> &str {
        self.handler.path_for(key)
    }

    pub fn set_sub_path(&mut self, key: impl Into<String>, path: impl Into<String>) {
        self.handler.set_sub_path(key, path);
    }

    pub fn compress_mode(&self) -> i32 {
        self.handler.compress_mode()
    }

    pub fn set_compress_mode(&mut self, value: i32) {
        self.handler.set_compress_mode(value);
    }

    pub fn base64_flag(&self) -> bool {
        self.handler.base64_flag()
    }

    pub fn base64(&self) -> bool {
        self.base64_flag()
    }

    pub fn base_64(&self) -> bool {
        self.base64_flag()
    }

    pub fn set_base64_flag(&mut self, value: bool) {
        self.handler.set_base64_flag(value);
    }

    pub fn set_base64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn set_base_64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn is_conv_to_link(&self) -> bool {
        self.conv_to_link
    }

    pub fn conv_to_link(&self) -> bool {
        self.is_conv_to_link()
    }

    pub fn set_conv_to_link(&mut self, value: bool) {
        self.conv_to_link = value;
    }

    pub fn category(&self) -> i32 {
        InOutHandler::IOH_CAT_OBJWRITER
    }

    pub fn attach_object(&mut self, object_uid: Uid) {
        self.target_object = Some(object_uid);
    }

    pub fn attach(&mut self, object_uid: Uid) {
        self.attach_object(object_uid);
    }

    pub fn detach_object(&mut self) -> Option<Uid> {
        self.target_object.take()
    }

    pub fn detach(&mut self) -> Option<Uid> {
        self.detach_object()
    }

    pub fn target_object(&self) -> Option<Uid> {
        self.target_object
    }

    pub fn write(&mut self) {
        self.write_count += 1;
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.handler.write_bytes(bytes)?;
        self.write_count += 1;
        Ok(())
    }

    pub fn write_string(&mut self, text: &str) -> Result<()> {
        self.write_bytes(text.as_bytes())
    }

    pub fn write_count(&self) -> usize {
        self.write_count
    }
}

impl Default for ObjWriter {
    fn default() -> Self {
        Self::new("", "", "")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneExporter {
    handler: InOutHandler,
    client_scene: Option<Uid>,
    camera_name: String,
    width: i32,
    height: i32,
    write_count: usize,
}

impl SceneExporter {
    pub fn new(
        name: impl Into<String>,
        type_descr: impl Into<String>,
        file_ext: impl Into<String>,
    ) -> Self {
        Self {
            handler: InOutHandler::new(
                name,
                type_descr,
                file_ext,
                InOutHandler::IOH_CAT_RENDTOFILE,
            ),
            client_scene: None,
            camera_name: String::new(),
            width: 0,
            height: 0,
            write_count: 0,
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        &self.handler
    }

    pub fn handler_mut(&mut self) -> &mut InOutHandler {
        &mut self.handler
    }

    pub fn path(&self) -> &str {
        self.handler.path()
    }

    pub fn set_path(&mut self, path: impl Into<String>) {
        self.handler.set_path(path);
    }

    pub fn path_for(&self, key: &str) -> &str {
        self.handler.path_for(key)
    }

    pub fn set_sub_path(&mut self, key: impl Into<String>, path: impl Into<String>) {
        self.handler.set_sub_path(key, path);
    }

    pub fn attach_scene(&mut self, scene_uid: Uid) {
        self.client_scene = Some(scene_uid);
    }

    pub fn attach(&mut self, scene_uid: Uid) {
        self.attach_scene(scene_uid);
    }

    pub fn detach_scene(&mut self) -> Option<Uid> {
        self.client_scene.take()
    }

    pub fn detach(&mut self) -> Option<Uid> {
        self.detach_scene()
    }

    pub fn client_scene(&self) -> Option<Uid> {
        self.client_scene
    }

    pub fn camera_name(&self) -> &str {
        &self.camera_name
    }

    pub fn get_camera_name(&self) -> &str {
        self.camera_name()
    }

    pub fn camera(&self) -> &str {
        self.camera_name()
    }

    pub fn set_camera_name(&mut self, value: impl Into<String>) {
        self.camera_name = value.into();
    }

    pub fn set_camera(&mut self, value: impl Into<String>) {
        self.set_camera_name(value);
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn get_width(&self) -> i32 {
        self.width()
    }

    pub fn set_width(&mut self, value: i32) {
        self.width = value;
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn get_height(&self) -> i32 {
        self.height()
    }

    pub fn set_height(&mut self, value: i32) {
        self.height = value;
    }

    pub fn base64_flag(&self) -> bool {
        self.handler.base64_flag()
    }

    pub fn base64(&self) -> bool {
        self.base64_flag()
    }

    pub fn base_64(&self) -> bool {
        self.base64_flag()
    }

    pub fn set_base64_flag(&mut self, value: bool) {
        self.handler.set_base64_flag(value);
    }

    pub fn set_base64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn set_base_64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn category(&self) -> i32 {
        InOutHandler::IOH_CAT_RENDTOFILE
    }

    pub fn write(&mut self) {
        self.write_count += 1;
    }

    pub fn write_count(&self) -> usize {
        self.write_count
    }
}

impl Default for SceneExporter {
    fn default() -> Self {
        Self::new("", "", "")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneXMLReader {
    handler: InOutHandler,
    client_scene: Option<Uid>,
    error_message: String,
    buffer_size: usize,
}

impl Default for SceneXMLReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneXMLReader {
    pub fn new() -> Self {
        Self {
            handler: InOutHandler::new(
                "qsc_xml",
                "CueMol Scene (*.qsc;*.qsl)",
                "*.qsc; *.qsl",
                InOutHandler::IOH_CAT_SCEREADER,
            ),
            client_scene: None,
            error_message: String::new(),
            buffer_size: 1024 * 1024,
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        &self.handler
    }

    pub fn handler_mut(&mut self) -> &mut InOutHandler {
        &mut self.handler
    }

    pub fn path(&self) -> &str {
        self.handler.path()
    }

    pub fn set_path(&mut self, path: impl Into<String>) {
        self.handler.set_path(path);
    }

    pub fn path_for(&self, key: &str) -> &str {
        self.handler.path_for(key)
    }

    pub fn set_sub_path(&mut self, key: impl Into<String>, path: impl Into<String>) {
        self.handler.set_sub_path(key, path);
    }

    pub fn name(&self) -> &str {
        self.handler.name()
    }

    pub fn type_descr(&self) -> &str {
        self.handler.type_descr()
    }

    pub fn file_ext(&self) -> &str {
        self.handler.file_ext()
    }

    pub fn base64_flag(&self) -> bool {
        self.handler.base64_flag()
    }

    pub fn base64(&self) -> bool {
        self.base64_flag()
    }

    pub fn base_64(&self) -> bool {
        self.base64_flag()
    }

    pub fn set_base64_flag(&mut self, value: bool) {
        self.handler.set_base64_flag(value);
    }

    pub fn set_base64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn set_base_64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn category(&self) -> i32 {
        InOutHandler::IOH_CAT_SCEREADER
    }

    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    pub fn get_buf_size(&self) -> usize {
        self.buffer_size()
    }

    pub fn bufsize(&self) -> usize {
        self.buffer_size()
    }

    pub fn set_buffer_size(&mut self, value: usize) {
        self.buffer_size = value;
    }

    pub fn set_buf_size(&mut self, value: usize) {
        self.set_buffer_size(value);
    }

    pub fn set_bufsize(&mut self, value: usize) {
        self.set_buffer_size(value);
    }

    pub fn attach_scene(&mut self, scene_uid: Uid) {
        self.client_scene = Some(scene_uid);
    }

    pub fn attach(&mut self, scene_uid: Uid) {
        self.attach_scene(scene_uid);
    }

    pub fn detach_scene(&mut self) -> Option<Uid> {
        self.client_scene.take()
    }

    pub fn detach(&mut self) -> Option<Uid> {
        self.detach_scene()
    }

    pub fn client_scene(&self) -> Option<Uid> {
        self.client_scene
    }

    pub fn error_message(&self) -> &str {
        &self.error_message
    }

    pub fn get_err_msg(&self) -> &str {
        self.error_message()
    }

    pub fn error(&self) -> &str {
        self.error_message()
    }

    pub fn errormsg(&self) -> &str {
        self.error_message()
    }

    pub fn from_byte_array(&mut self, data: &ByteArray) -> Option<String> {
        match String::from_utf8(data.as_bytes().to_vec()) {
            Ok(value) => {
                self.error_message.clear();
                Some(value)
            }
            Err(error) => {
                self.error_message = error.to_string();
                None
            }
        }
    }

    pub fn camera_from_byte_array(&mut self, data: &ByteArray) -> Result<Camera> {
        let text = std::str::from_utf8(data.as_bytes()).map_err(|error| {
            cuemol_core::CueMolError::parse(format!("camera XML is not UTF-8: {error}"))
        })?;
        match Camera::from_xml_string(text) {
            Ok(camera) => {
                self.error_message.clear();
                Ok(camera)
            }
            Err(error) => {
                self.error_message = error.to_string();
                Err(error)
            }
        }
    }

    pub fn read_string(&mut self) -> Result<String> {
        match self.handler.read_string() {
            Ok(value) => {
                self.error_message.clear();
                Ok(value)
            }
            Err(error) => {
                self.error_message = error.to_string();
                Err(error)
            }
        }
    }

    pub fn read(&mut self) -> Result<String> {
        self.read_string()
    }

    pub fn read_byte_array(&mut self) -> Result<ByteArray> {
        match self.handler.read_byte_array() {
            Ok(value) => {
                self.error_message.clear();
                Ok(value)
            }
            Err(error) => {
                self.error_message = error.to_string();
                Err(error)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneXMLWriter {
    handler: InOutHandler,
    client_scene: Option<Uid>,
    embed_all: bool,
    num_version: i32,
    document: String,
}

impl Default for SceneXMLWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneXMLWriter {
    pub fn new() -> Self {
        Self {
            handler: InOutHandler::new(
                "qsc_xml",
                "CueMol Scene (*.qsc)",
                "*.qsc",
                InOutHandler::IOH_CAT_SCEWRITER,
            ),
            client_scene: None,
            embed_all: false,
            num_version: 0,
            document: String::new(),
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        &self.handler
    }

    pub fn handler_mut(&mut self) -> &mut InOutHandler {
        &mut self.handler
    }

    pub fn path(&self) -> &str {
        self.handler.path()
    }

    pub fn set_path(&mut self, path: impl Into<String>) {
        self.handler.set_path(path);
    }

    pub fn path_for(&self, key: &str) -> &str {
        self.handler.path_for(key)
    }

    pub fn set_sub_path(&mut self, key: impl Into<String>, path: impl Into<String>) {
        self.handler.set_sub_path(key, path);
    }

    pub fn name(&self) -> &str {
        self.handler.name()
    }

    pub fn type_descr(&self) -> &str {
        self.handler.type_descr()
    }

    pub fn file_ext(&self) -> &str {
        self.handler.file_ext()
    }

    pub fn category(&self) -> i32 {
        InOutHandler::IOH_CAT_SCEWRITER
    }

    pub fn compress_mode(&self) -> i32 {
        self.handler.compress_mode()
    }

    pub fn set_compress_mode(&mut self, value: i32) {
        self.handler.set_compress_mode(value);
    }

    pub fn base64_flag(&self) -> bool {
        self.handler.base64_flag()
    }

    pub fn base64(&self) -> bool {
        self.base64_flag()
    }

    pub fn base_64(&self) -> bool {
        self.base64_flag()
    }

    pub fn set_base64_flag(&mut self, value: bool) {
        self.handler.set_base64_flag(value);
    }

    pub fn set_base64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn set_base_64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn embed_all(&self) -> bool {
        self.embed_all
    }

    pub fn get_embed_all(&self) -> bool {
        self.embed_all()
    }

    pub fn set_embed_all(&mut self, value: bool) {
        self.embed_all = value;
    }

    pub fn num_version(&self) -> i32 {
        self.num_version
    }

    pub fn set_num_version(&mut self, value: i32) {
        self.num_version = value;
    }

    pub fn str_version(&self) -> String {
        format!("QDF{}", self.num_version)
    }

    pub fn get_str_version(&self) -> String {
        self.str_version()
    }

    pub fn set_str_version(&mut self, value: &str) {
        self.num_version = value
            .strip_prefix("QDF")
            .and_then(|suffix| suffix.parse().ok())
            .unwrap_or(0);
    }

    pub fn version(&self) -> String {
        self.str_version()
    }

    pub fn set_version(&mut self, value: &str) {
        self.set_str_version(value);
    }

    pub fn set_default_opts<T: SceneWriterOptions>(&mut self, scene: &T) {
        let Some(options) = scene.qsc_options_node() else {
            return;
        };
        let version = options.str_attr("version").to_string();
        if !version.is_empty() {
            self.set_str_version(&version);
        }
        let compression = options.str_attr("compress").to_string();
        if !compression.is_empty() {
            self.set_compress_mode(match compression.as_str() {
                "gzip" => InOutHandler::COMP_GZIP,
                "bzip2" => InOutHandler::COMP_BZIP2,
                "xzip" | "xz" => InOutHandler::COMP_XZIP,
                _ => InOutHandler::COMP_NONE,
            });
        }
        let base64 = options.str_attr("base64").to_string();
        if !base64.is_empty() {
            self.set_base64_flag(
                base64.eq_ignore_ascii_case("true")
                    || base64.eq_ignore_ascii_case("yes")
                    || base64 == "1",
            );
        }
    }

    pub fn attach_scene(&mut self, scene_uid: Uid) {
        self.client_scene = Some(scene_uid);
    }

    pub fn attach(&mut self, scene_uid: Uid) {
        self.attach_scene(scene_uid);
    }

    pub fn detach_scene(&mut self) -> Option<Uid> {
        self.client_scene.take()
    }

    pub fn detach(&mut self) -> Option<Uid> {
        self.detach_scene()
    }

    pub fn client_scene(&self) -> Option<Uid> {
        self.client_scene
    }

    pub fn write_string(&self, text: &str) -> Result<()> {
        self.handler.write_string(text)
    }

    pub fn document(&self) -> &str {
        &self.document
    }

    pub fn set_document(&mut self, value: impl Into<String>) {
        self.document = value.into();
    }

    pub fn write(&self) -> Result<()> {
        self.handler.write_string(&self.document)
    }

    pub fn write_byte_array(&self, data: &ByteArray) -> Result<()> {
        self.handler.write_bytes(data.as_bytes())
    }

    pub fn camera_to_byte_array(&self, camera: &Camera) -> ByteArray {
        ByteArray::copy_from_u8(camera.to_xml_string().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_reader_writer_state_matches_reference() {
        let mut reader = ObjReader::new("minimal", "Minimal reader", "*.min");
        assert_eq!(reader.compress_mode(), InOutHandler::COMP_NONE);
        reader.set_compress_mode(InOutHandler::COMP_GZIP);
        assert_eq!(reader.compress_mode(), InOutHandler::COMP_GZIP);
        assert!(!reader.base64_flag());
        reader.set_base64_flag(true);
        assert!(reader.base64_flag());
        assert_eq!(reader.category(), InOutHandler::IOH_CAT_OBJREADER);
        assert_eq!(reader.target_object(), None);
        reader.attach_object(Uid::new(10));
        assert_eq!(reader.target_object(), Some(Uid::new(10)));
        assert_eq!(reader.create_default_object(), Some(Uid::new(10)));
        assert_eq!(reader.detach_object(), Some(Uid::new(10)));
        assert_eq!(reader.target_object(), None);
        assert_eq!(
            reader.create_default_object(),
            Some(stable_uid_from_name("minimal"))
        );
        reader.read();
        assert_eq!(reader.read_count(), 1);

        let mut writer = ObjWriter::new("minimal", "Minimal writer", "*.min");
        assert_eq!(writer.compress_mode(), InOutHandler::COMP_NONE);
        writer.set_compress_mode(InOutHandler::COMP_GZIP);
        assert_eq!(writer.compress_mode(), InOutHandler::COMP_GZIP);
        assert!(!writer.base64_flag());
        writer.set_base64_flag(true);
        assert!(writer.base64_flag());
        assert!(!writer.is_conv_to_link());
        writer.set_conv_to_link(true);
        assert!(writer.is_conv_to_link());
        assert_eq!(writer.category(), InOutHandler::IOH_CAT_OBJWRITER);
        assert_eq!(writer.target_object(), None);
        writer.attach_object(Uid::new(20));
        assert_eq!(writer.target_object(), Some(Uid::new(20)));
        assert_eq!(writer.detach_object(), Some(Uid::new(20)));
        assert_eq!(writer.target_object(), None);
        writer.write();
        assert_eq!(writer.write_count(), 1);

        let path = std::env::temp_dir().join(format!("cuemol-qsys-obj-{}.txt", std::process::id()));
        writer.set_path(path.to_str().unwrap());
        writer.write_string("object-stream").unwrap();
        assert_eq!(writer.write_count(), 2);
        reader.set_path(path.to_str().unwrap());
        assert_eq!(reader.read_string().unwrap(), "object-stream");
        assert_eq!(reader.read_count(), 2);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn scene_xml_reader_writer_and_exporter_state_matches_reference() {
        let uid = Uid::new(42);

        let mut reader = SceneXMLReader::new();
        assert_eq!(reader.name(), "qsc_xml");
        assert!(!reader.type_descr().is_empty());
        assert!(!reader.file_ext().is_empty());
        assert_eq!(reader.category(), InOutHandler::IOH_CAT_SCEREADER);
        assert_eq!(reader.buffer_size(), 1024 * 1024);
        assert_eq!(reader.bufsize(), 1024 * 1024);
        reader.set_bufsize(512 * 1024);
        assert_eq!(reader.buffer_size(), 512 * 1024);
        reader.attach_scene(uid);
        assert_eq!(reader.client_scene(), Some(uid));
        assert_eq!(reader.detach_scene(), Some(uid));
        assert_eq!(reader.client_scene(), None);

        let mut writer = SceneXMLWriter::new();
        assert_eq!(writer.name(), "qsc_xml");
        assert!(!writer.type_descr().is_empty());
        assert!(!writer.file_ext().is_empty());
        assert_eq!(writer.category(), InOutHandler::IOH_CAT_SCEWRITER);
        assert_eq!(writer.compress_mode(), InOutHandler::COMP_NONE);
        writer.set_compress_mode(InOutHandler::COMP_GZIP);
        assert_eq!(writer.compress_mode(), InOutHandler::COMP_GZIP);
        assert!(!writer.base64_flag());
        writer.set_base64_flag(true);
        assert!(writer.base64_flag());
        assert!(!writer.embed_all());
        writer.set_embed_all(true);
        assert!(writer.embed_all());
        assert_eq!(writer.num_version(), 0);
        writer.set_num_version(2);
        assert_eq!(writer.str_version(), "QDF2");
        writer.set_str_version("QDF3");
        assert_eq!(writer.num_version(), 3);
        let mut options = LDom2Node::new();
        options.set_tag_name("qsc_opts");
        options.set_str_attr("version", "QDF1");
        options.set_str_attr("compress", "xzip");
        options.set_str_attr("base64", "true");
        writer.set_embed_all(false);
        writer.set_default_opts(&options);
        assert_eq!(writer.num_version(), 1);
        assert_eq!(writer.compress_mode(), InOutHandler::COMP_XZIP);
        assert!(writer.base64_flag());
        assert!(!writer.embed_all());
        writer.attach_scene(uid);
        assert_eq!(writer.detach_scene(), Some(uid));

        let path =
            std::env::temp_dir().join(format!("cuemol-qsys-scene-{}.qsc", std::process::id()));
        writer.set_path(path.to_str().unwrap());
        writer.write_string("<scene/>\n").unwrap();
        reader.set_path(path.to_str().unwrap());
        assert_eq!(reader.read_string().unwrap(), "<scene/>\n");
        assert_eq!(reader.error_message(), "");
        assert_eq!(reader.read_byte_array().unwrap().as_bytes(), b"<scene/>\n");
        writer
            .write_byte_array(&ByteArray::copy_from_u8(b"<scene name=\"byte\"/>\n"))
            .unwrap();
        assert_eq!(reader.read_string().unwrap(), "<scene name=\"byte\"/>\n");
        reader.set_path(path.with_extension("missing").to_str().unwrap());
        assert!(reader.read_string().is_err());
        assert!(!reader.error_message().is_empty());
        let _ = std::fs::remove_file(path);

        let mut exporter = SceneExporter::new("minimal", "Minimal exporter", "*.min");
        assert_eq!(exporter.width(), 0);
        exporter.set_width(800);
        assert_eq!(exporter.width(), 800);
        assert_eq!(exporter.height(), 0);
        exporter.set_height(600);
        assert_eq!(exporter.height(), 600);
        assert_eq!(exporter.camera_name(), "");
        exporter.set_camera("main");
        assert_eq!(exporter.camera(), "main");
        assert_eq!(exporter.category(), InOutHandler::IOH_CAT_RENDTOFILE);
    }
}
