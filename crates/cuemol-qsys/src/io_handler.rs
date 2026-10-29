use cuemol_core::{Base64InStream, Base64OutStream, ByteArray, CueMolError, Result};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Write};
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
        let bytes = fs::read(path)?;
        if self.filters_apply(key) {
            self.decode_bytes(bytes)
        } else {
            Ok(bytes)
        }
    }

    pub fn decode_bytes(&self, mut bytes: Vec<u8>) -> Result<Vec<u8>> {
        self.validate_compression()?;
        if self.base64 {
            bytes = Base64InStream::new(bytes).into_bytes();
        }
        match self.compress_mode {
            Self::COMP_GZIP => cuemol_core::decode_gzip_bytes(&bytes),
            Self::COMP_XZIP => cuemol_core::decode_xz_bytes(&bytes),
            _ => Ok(bytes),
        }
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
        String::from_utf8(self.read_bytes_for(key)?)
            .map_err(|error| CueMolError::parse(error.to_string()))
    }

    pub fn write_bytes(&self, bytes: &[u8]) -> Result<()> {
        self.write_bytes_for("", bytes)
    }

    pub fn write_bytes_for(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let path = self.required_path(key)?;
        if !self.filters_apply(key) || (self.compress_mode == Self::COMP_NONE && !self.base64) {
            fs::write(path, bytes)?;
            return Ok(());
        }
        self.validate_compression()?;
        let mut output = match self.compress_mode {
            Self::COMP_GZIP => {
                let mut encoder =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(bytes)?;
                encoder.finish()?
            }
            Self::COMP_XZIP => {
                let mut output = Vec::new();
                lzma_rs::xz_compress(&mut Cursor::new(bytes), &mut output)
                    .map_err(|error| CueMolError::Io(error.to_string()))?;
                output
            }
            _ => bytes.to_vec(),
        };
        if self.base64 {
            let mut encoder = Base64OutStream::new();
            encoder.write(&output, 0, output.len());
            output = encoder.finish();
        }
        fs::write(path, output)?;
        Ok(())
    }

    fn filters_apply(&self, key: &str) -> bool {
        key.is_empty()
            && matches!(
                self.category,
                Self::IOH_CAT_OBJREADER | Self::IOH_CAT_OBJWRITER
            )
    }

    fn validate_compression(&self) -> Result<()> {
        if matches!(
            self.compress_mode,
            Self::COMP_NONE | Self::COMP_GZIP | Self::COMP_XZIP
        ) {
            Ok(())
        } else {
            Err(CueMolError::invalid_argument(format!(
                "unsupported object stream compression method: {}",
                self.compress_mode
            )))
        }
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

    pub fn compress(&self) -> i32 {
        self.compress_mode()
    }

    pub fn set_compress_mode(&mut self, value: i32) {
        self.compress_mode = value;
    }

    pub fn set_compress(&mut self, value: i32) {
        self.set_compress_mode(value);
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
    use std::io::Read;

    #[test]
    fn object_stream_filters_compress_before_base64_and_leave_auxiliary_streams_raw() {
        let path =
            std::env::temp_dir().join(format!("cuemol-stream-filters-{}", std::process::id()));
        let aux = path.with_extension("aux");
        let payload = b"molecular coordinates\n\x00\xff";
        let mut handler = InOutHandler::new("test", "Test", "*", InOutHandler::IOH_CAT_OBJWRITER);
        handler.set_path(path.to_string_lossy());
        handler.set_sub_path("aux", aux.to_string_lossy());
        for compression in [
            InOutHandler::COMP_NONE,
            InOutHandler::COMP_GZIP,
            InOutHandler::COMP_XZIP,
        ] {
            for base64 in [false, true] {
                handler.set_compress(compression);
                handler.set_base64(base64);
                handler.write_bytes(payload).unwrap();
                let encoded = fs::read(&path).unwrap();
                let compressed = if base64 {
                    assert!(encoded.is_ascii());
                    Base64InStream::new(encoded).into_bytes()
                } else {
                    encoded
                };
                let mut decoded = Vec::new();
                match compression {
                    InOutHandler::COMP_GZIP => {
                        assert!(compressed.starts_with(&[0x1f, 0x8b]));
                        flate2::read::GzDecoder::new(compressed.as_slice())
                            .read_to_end(&mut decoded)
                            .unwrap();
                    }
                    InOutHandler::COMP_XZIP => {
                        assert!(compressed.starts_with(b"\xfd7zXZ\0"));
                        lzma_rs::xz_decompress(&mut Cursor::new(compressed), &mut decoded).unwrap();
                    }
                    _ => decoded = compressed,
                }
                assert_eq!(decoded, payload);
                assert_eq!(handler.read_bytes().unwrap(), payload);
                handler.write_bytes_for("aux", payload).unwrap();
                assert_eq!(fs::read(&aux).unwrap(), payload);
                assert_eq!(handler.read_bytes_for("aux").unwrap(), payload);
            }
        }
        fs::remove_file(path).unwrap();
        fs::remove_file(aux).unwrap();
    }

    #[test]
    fn invalid_object_filters_fail_without_replacing_files_and_scene_streams_stay_raw() {
        let path =
            std::env::temp_dir().join(format!("cuemol-stream-invalid-{}", std::process::id()));
        fs::write(&path, b"unchanged").unwrap();
        let mut handler = InOutHandler::default();
        handler.set_path(path.to_string_lossy());
        for compression in [InOutHandler::COMP_BZIP2, 99] {
            handler.set_compress(compression);
            assert!(handler.write_string("replacement").is_err());
            assert!(handler.read_bytes().is_err());
            assert_eq!(fs::read(&path).unwrap(), b"unchanged");
        }
        for compression in [InOutHandler::COMP_GZIP, InOutHandler::COMP_XZIP] {
            handler.set_compress(compression);
            assert!(handler.read_bytes().is_err());
        }
        let mut scene = InOutHandler::new("qsc", "Scene", "*.qsc", InOutHandler::IOH_CAT_SCEWRITER);
        scene.set_path(path.to_string_lossy());
        scene.set_compress(InOutHandler::COMP_GZIP);
        scene.set_base64(true);
        scene.write_string("<scene/>").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "<scene/>");
        fs::remove_file(path).unwrap();
    }

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
        handler.set_compress(InOutHandler::COMP_GZIP);
        assert_eq!(handler.compress(), InOutHandler::COMP_GZIP);
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
