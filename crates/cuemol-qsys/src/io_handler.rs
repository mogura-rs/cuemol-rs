use cuemol_core::{ByteArray, CueMolError, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InOutHandler {
    name: String,
    type_descr: String,
    file_ext: String,
    category: i32,
    paths: BTreeMap<String, String>,
    compress_mode: i32,
    base64: bool,
}

impl Default for InOutHandler {
    fn default() -> Self {
        Self::new("", "", "", Self::IOH_CAT_OBJREADER)
    }
}

impl InOutHandler {
    pub const IOH_CAT_OBJREADER: i32 = 0;
    pub const IOH_CAT_OBJWRITER: i32 = 1;
    pub const IOH_CAT_RENDTOFILE: i32 = 2;
    pub const IOH_CAT_SCEREADER: i32 = 3;
    pub const IOH_CAT_SCEWRITER: i32 = 4;

    pub const COMP_NONE: i32 = 0;
    pub const COMP_GZIP: i32 = 1;
    pub const COMP_BZIP2: i32 = 2;
    pub const COMP_XZIP: i32 = 3;

    pub fn new(
        name: impl Into<String>,
        type_descr: impl Into<String>,
        file_ext: impl Into<String>,
        category: i32,
    ) -> Self {
        Self {
            name: name.into(),
            type_descr: type_descr.into(),
            file_ext: file_ext.into(),
            category,
            paths: BTreeMap::new(),
            compress_mode: Self::COMP_NONE,
            base64: false,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_name(&self) -> &str {
        self.name()
    }

    pub fn type_descr(&self) -> &str {
        &self.type_descr
    }

    pub fn get_type_descr(&self) -> &str {
        self.type_descr()
    }

    pub fn typedescr(&self) -> &str {
        self.type_descr()
    }

    pub fn file_ext(&self) -> &str {
        &self.file_ext
    }

    pub fn get_file_ext(&self) -> &str {
        self.file_ext()
    }

    pub fn fileext(&self) -> &str {
        self.file_ext()
    }

    pub fn category(&self) -> i32 {
        self.category
    }

    pub fn path(&self) -> &str {
        self.path_for("")
    }

    pub fn set_path(&mut self, path: impl Into<String>) {
        self.set_path_for("", path);
    }

    pub fn path_for(&self, key: &str) -> &str {
        self.paths.get(key).map_or("", String::as_str)
    }

    pub fn set_path_for(&mut self, key: impl Into<String>, path: impl Into<String>) {
        self.paths.insert(key.into(), path.into());
    }

    pub fn set_sub_path(&mut self, key: impl Into<String>, path: impl Into<String>) {
        self.set_path_for(key, path);
    }

    pub fn sub_path(&self, key: &str) -> &str {
        self.path_for(key)
    }

    pub fn clear_paths(&mut self) {
        self.paths.clear();
    }

    pub fn read_bytes(&self) -> Result<Vec<u8>> {
        self.read_bytes_for("")
    }

    pub fn read_bytes_for(&self, key: &str) -> Result<Vec<u8>> {
        let path = self.required_path(key)?;
        Ok(fs::read(path)?)
    }

    pub fn read_byte_array(&self) -> Result<ByteArray> {
        self.read_byte_array_for("")
    }

    pub fn read_byte_array_for(&self, key: &str) -> Result<ByteArray> {
        Ok(ByteArray::copy_from_u8(&self.read_bytes_for(key)?))
    }

    pub fn read_string(&self) -> Result<String> {
        self.read_string_for("")
    }

    pub fn read_string_for(&self, key: &str) -> Result<String> {
        let path = self.required_path(key)?;
        Ok(fs::read_to_string(path)?)
    }

    pub fn write_bytes(&self, bytes: &[u8]) -> Result<()> {
        self.write_bytes_for("", bytes)
    }

    pub fn write_bytes_for(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let path = self.required_path(key)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    pub fn write_string(&self, text: &str) -> Result<()> {
        self.write_string_for("", text)
    }

    pub fn write_string_for(&self, key: &str, text: &str) -> Result<()> {
        self.write_bytes_for(key, text.as_bytes())
    }

    pub fn compress_mode(&self) -> i32 {
        self.compress_mode
    }

    pub fn get_compress_mode(&self) -> i32 {
        self.compress_mode()
    }

    pub fn set_compress_mode(&mut self, value: i32) {
        self.compress_mode = value;
    }

    pub fn base64_flag(&self) -> bool {
        self.base64
    }

    pub fn get_base64_flag(&self) -> bool {
        self.base64_flag()
    }

    pub fn base64(&self) -> bool {
        self.base64_flag()
    }

    pub fn base_64(&self) -> bool {
        self.base64_flag()
    }

    pub fn set_base64_flag(&mut self, value: bool) {
        self.base64 = value;
    }

    pub fn set_base64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    pub fn set_base_64(&mut self, value: bool) {
        self.set_base64_flag(value);
    }

    fn required_path(&self, key: &str) -> Result<&Path> {
        let path = self.path_for(key);
        if path.is_empty() {
            return Err(CueMolError::invalid_argument(format!(
                "path is empty for stream key '{}'",
                key
            )));
        }
        Ok(Path::new(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inout_handler_paths_and_constants_match_reference() {
        let mut handler = InOutHandler::new(
            "minimal",
            "Minimal handler",
            "*.min",
            InOutHandler::IOH_CAT_OBJREADER,
        );
        assert_eq!(handler.path(), "");
        handler.set_path("/tmp/test.pdb");
        assert_eq!(handler.path(), "/tmp/test.pdb");
        handler.set_path_for("sub", "/tmp/sub.dat");
        assert_eq!(handler.path_for("sub"), "/tmp/sub.dat");
        handler.set_path("/main.dat");
        handler.set_path_for("aux", "/aux.dat");
        assert_eq!(handler.path(), "/main.dat");
        assert_eq!(handler.path_for("aux"), "/aux.dat");
        assert_eq!(handler.sub_path("aux"), "/aux.dat");

        assert_eq!(handler.compress_mode(), InOutHandler::COMP_NONE);
        assert!(!handler.base64_flag());
        assert_eq!(InOutHandler::COMP_NONE, 0);
        assert_ne!(InOutHandler::COMP_GZIP, InOutHandler::COMP_NONE);
        assert_ne!(InOutHandler::COMP_BZIP2, InOutHandler::COMP_GZIP);
        assert_ne!(InOutHandler::COMP_XZIP, InOutHandler::COMP_BZIP2);
        assert_eq!(InOutHandler::IOH_CAT_OBJREADER, 0);
        assert_eq!(InOutHandler::IOH_CAT_OBJWRITER, 1);
        assert_eq!(InOutHandler::IOH_CAT_RENDTOFILE, 2);
        assert_eq!(InOutHandler::IOH_CAT_SCEREADER, 3);
        assert_eq!(InOutHandler::IOH_CAT_SCEWRITER, 4);
    }

    #[test]
    fn inout_handler_reads_and_writes_configured_stream_paths() {
        let path =
            std::env::temp_dir().join(format!("cuemol-qsys-inout-{}.txt", std::process::id()));
        let aux_path = path.with_extension("aux");

        let mut handler = InOutHandler::new(
            "minimal",
            "Minimal handler",
            "*.min",
            InOutHandler::IOH_CAT_OBJWRITER,
        );
        assert!(handler.read_string().is_err());
        handler.set_path(path.to_str().unwrap());
        handler.set_sub_path("aux", aux_path.to_str().unwrap());

        handler.write_string("main").unwrap();
        handler.write_bytes_for("aux", b"aux").unwrap();
        assert_eq!(handler.read_string().unwrap(), "main");
        assert_eq!(
            handler.read_byte_array_for("aux").unwrap().as_bytes(),
            b"aux"
        );

        handler.clear_paths();
        assert_eq!(handler.path(), "");
        assert!(handler.write_string("missing").is_err());

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(aux_path);
    }
}
