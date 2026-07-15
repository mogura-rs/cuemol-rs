use crate::{
    parse_ldom2_xml, write_ldom2_xml, CueMolError, InStream, LDataSrcContainer, LDom2Tree, LString,
    OutStream, Result, StrInStream,
};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub const END_OF_XML_MARKER: &str = "========== End of XML ==========\n";
const CHUNK_START_PREFIX: &str = "\n==========START ";
const CHUNK_START_BODY: &str = "==========START ";
const CHUNK_END_PREFIX: &str = "\n==========END ";
const CHUNK_MARKER_SUFFIX: &str = "==========\n";
const CHUNK_ID_PREFIX: &str = "datachunk:";
const CHUNK_ID_LEN: usize = 15;

/// QSC/XML output stream corresponding to `qlib::LDom2OutStream`.
pub struct LDom2OutStream {
    indent: LString,
    qdf_version: i32,
    qdf_encoding: LString,
    chunks: Vec<(LString, Rc<dyn LDataSrcContainer>)>,
}

impl fmt::Debug for LDom2OutStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LDom2OutStream")
            .field("indent", &self.indent)
            .field("qdf_version", &self.qdf_version)
            .field("qdf_encoding", &self.qdf_encoding)
            .field("chunk_count", &self.chunks.len())
            .finish()
    }
}

impl Default for LDom2OutStream {
    fn default() -> Self {
        Self::new()
    }
}

impl LDom2OutStream {
    pub fn new() -> Self {
        Self {
            indent: "\t".into(),
            qdf_version: 0,
            qdf_encoding: LString::new(),
            chunks: Vec::new(),
        }
    }

    pub fn set_indent_string(&mut self, value: impl Into<LString>) {
        self.indent = value.into();
    }

    pub fn qdf_version(&self) -> i32 {
        self.qdf_version
    }

    pub fn set_qdf_version(&mut self, value: i32) {
        self.qdf_version = value;
    }

    pub fn qdf_encoding(&self) -> &LString {
        &self.qdf_encoding
    }

    pub fn set_qdf_encoding(&mut self, value: impl Into<LString>) {
        self.qdf_encoding = value.into();
    }

    pub fn prepare_data_chunk(&mut self, container: Rc<dyn LDataSrcContainer>) -> LString {
        let chunk_id = format_chunk_id(self.chunks.len());
        self.chunks.push((chunk_id.clone(), container));
        chunk_id
    }

    pub fn write_bytes(&self, tree: &LDom2Tree) -> Result<Vec<u8>> {
        let mut output = write_ldom2_xml(tree).into_bytes();
        let mut wrote_marker = false;

        for (chunk_id, container) in &self.chunks {
            if !container.is_data_src_writable() {
                continue;
            }
            if !wrote_marker {
                output.push(b'\n');
                output.extend_from_slice(END_OF_XML_MARKER.as_bytes());
                wrote_marker = true;
            }
            output.extend_from_slice(CHUNK_START_PREFIX.as_bytes());
            output.extend_from_slice(chunk_id.as_str().as_bytes());
            output.extend_from_slice(CHUNK_MARKER_SUFFIX.as_bytes());
            output.extend_from_slice(&container.write_data_chunk()?);
            output.extend_from_slice(CHUNK_END_PREFIX.as_bytes());
            output.extend_from_slice(chunk_id.as_str().as_bytes());
            output.extend_from_slice(CHUNK_MARKER_SUFFIX.as_bytes());
        }
        Ok(output)
    }

    pub fn write_to(&self, tree: &LDom2Tree, output: &mut dyn OutStream) -> Result<()> {
        let data = self.write_bytes(tree)?;
        write_all(output, &data)
    }
}

/// Parsed QSC/XML stream with indexed binary chunks.
#[derive(Debug, Clone)]
pub struct LDom2InStream {
    tree: LDom2Tree,
    xml_source: String,
    chunks: BTreeMap<LString, Vec<u8>>,
    chunk_order: Vec<LString>,
}

impl LDom2InStream {
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let marker = END_OF_XML_MARKER.as_bytes();
        let (xml, chunk_data) = match locate(data, marker) {
            Some(index) => (&data[..index], Some(&data[index + marker.len()..])),
            None => (data, None),
        };
        let xml = std::str::from_utf8(xml)
            .map_err(|error| CueMolError::parse(format!("XML is not UTF-8: {error}")))?;
        let xml_source = xml.trim_end().to_string();
        let tree = parse_ldom2_xml(&xml_source)?;
        let mut result = Self {
            tree,
            xml_source,
            chunks: BTreeMap::new(),
            chunk_order: Vec::new(),
        };
        if let Some(chunk_data) = chunk_data {
            result.parse_chunks(chunk_data)?;
        }
        Ok(result)
    }

    pub fn read_from(input: &mut dyn InStream) -> Result<Self> {
        let mut data = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let size = buffer.len();
            let count = input.read(&mut buffer, 0, size)?;
            if count == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..count]);
        }
        Self::from_bytes(&data)
    }

    pub fn tree(&self) -> &LDom2Tree {
        &self.tree
    }

    pub fn xml_source(&self) -> &str {
        &self.xml_source
    }

    pub fn into_tree(self) -> LDom2Tree {
        self.tree
    }

    pub fn chunk_ids(&self) -> &[LString] {
        &self.chunk_order
    }

    pub fn chunk(&self, chunk_id: impl AsRef<str>) -> Result<&[u8]> {
        self.chunks
            .get(&LString::from(chunk_id.as_ref()))
            .map(Vec::as_slice)
            .ok_or_else(|| {
                CueMolError::not_found(format!(
                    "chunk (ID: \"{}\") is not found",
                    chunk_id.as_ref()
                ))
            })
    }

    pub fn read_chunk_into(
        &self,
        chunk_id: impl AsRef<str>,
        container: &mut dyn LDataSrcContainer,
    ) -> Result<()> {
        let mut input = StrInStream::new(self.chunk(chunk_id)?);
        container.read_from_stream(&mut input)
    }

    fn parse_chunks(&mut self, data: &[u8]) -> Result<()> {
        let mut cursor = 0;
        while cursor < data.len() {
            while data
                .get(cursor)
                .is_some_and(|byte| byte.is_ascii_whitespace())
            {
                cursor += 1;
            }
            if cursor == data.len() {
                break;
            }
            let remaining = &data[cursor..];
            if !remaining.starts_with(CHUNK_START_BODY.as_bytes()) {
                return Err(corrupt_chunk());
            }
            cursor += CHUNK_START_BODY.len();
            if data.len() < cursor + CHUNK_ID_LEN + CHUNK_MARKER_SUFFIX.len() {
                return Err(corrupt_chunk());
            }
            let id_bytes = &data[cursor..cursor + CHUNK_ID_LEN];
            let chunk_id = validate_chunk_id(id_bytes)?;
            cursor += CHUNK_ID_LEN;
            if !data[cursor..].starts_with(CHUNK_MARKER_SUFFIX.as_bytes()) {
                return Err(corrupt_chunk());
            }
            cursor += CHUNK_MARKER_SUFFIX.len();

            let end_marker = format!(
                "{CHUNK_END_PREFIX}{}{CHUNK_MARKER_SUFFIX}",
                chunk_id.as_str()
            );
            let payload_end =
                locate(&data[cursor..], end_marker.as_bytes()).ok_or_else(corrupt_chunk)?;
            let payload = data[cursor..cursor + payload_end].to_vec();
            if self.chunks.insert(chunk_id.clone(), payload).is_some() {
                return Err(CueMolError::parse(format!(
                    "duplicate data chunk ID: {chunk_id}"
                )));
            }
            self.chunk_order.push(chunk_id);
            cursor += payload_end + end_marker.len();
        }
        Ok(())
    }
}

pub type LObjInStream3 = LDom2InStream;

fn format_chunk_id(index: usize) -> LString {
    LString::from(format!("{CHUNK_ID_PREFIX}{index:05}"))
}

fn validate_chunk_id(bytes: &[u8]) -> Result<LString> {
    let value = std::str::from_utf8(bytes).map_err(|_| corrupt_chunk())?;
    let Some(number) = value.strip_prefix(CHUNK_ID_PREFIX) else {
        return Err(corrupt_chunk());
    };
    if number.len() != 5 || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(corrupt_chunk());
    }
    Ok(value.into())
}

fn locate(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn write_all(output: &mut dyn OutStream, data: &[u8]) -> Result<()> {
    let mut offset = 0;
    while offset < data.len() {
        let count = output.write(data, offset, data.len() - offset)?;
        if count == 0 {
            return Err(CueMolError::Io("output stream is closed".to_string()));
        }
        offset += count;
    }
    Ok(())
}

fn corrupt_chunk() -> CueMolError {
    CueMolError::parse("data chunk is corrupted")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StrOutStream;
    use std::cell::RefCell;

    #[derive(Debug)]
    struct MemoryContainer {
        writable: bool,
        output: Vec<u8>,
        input: RefCell<Vec<u8>>,
    }

    impl MemoryContainer {
        fn writable(output: impl AsRef<[u8]>) -> Self {
            Self {
                writable: true,
                output: output.as_ref().to_vec(),
                input: RefCell::new(Vec::new()),
            }
        }
    }

    impl LDataSrcContainer for MemoryContainer {
        fn is_data_src_writable(&self) -> bool {
            self.writable
        }

        fn write_data_chunk(&self) -> Result<Vec<u8>> {
            Ok(self.output.clone())
        }

        fn read_from_stream(&mut self, input: &mut dyn InStream) -> Result<()> {
            let mut data = Vec::new();
            let mut buffer = [0_u8; 8];
            loop {
                let size = buffer.len();
                let count = input.read(&mut buffer, 0, size)?;
                if count == 0 {
                    break;
                }
                data.extend_from_slice(&buffer[..count]);
            }
            *self.input.borrow_mut() = data;
            Ok(())
        }

        fn update_src_path(&mut self, _path: &LString) {}
    }

    #[test]
    fn qsc_stream_roundtrips_xml_and_multiple_binary_chunks() {
        let mut tree = LDom2Tree::new("scene");
        tree.top_mut().set_str_attr("name", "main");
        let mut output = LDom2OutStream::new();
        let first = output.prepare_data_chunk(Rc::new(MemoryContainer::writable([0, 1, 2, 0xff])));
        let second = output.prepare_data_chunk(Rc::new(MemoryContainer::writable(b"payload")));

        assert_eq!(first.as_str(), "datachunk:00000");
        assert_eq!(second.as_str(), "datachunk:00001");
        let bytes = output.write_bytes(&tree).unwrap();
        let parsed = LDom2InStream::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.tree().top().tag_name(), "scene");
        assert!(parsed.tree().top().str_attr("name").equals("main"));
        assert_eq!(
            parsed.chunk_ids(),
            &[
                LString::from("datachunk:00000"),
                LString::from("datachunk:00001")
            ]
        );
        assert_eq!(parsed.chunk("datachunk:00000").unwrap(), [0, 1, 2, 0xff]);
        assert_eq!(parsed.chunk("datachunk:00001").unwrap(), b"payload");
    }

    #[test]
    fn qsc_stream_reads_and_writes_through_stream_interfaces() {
        let tree = LDom2Tree::new("root");
        let output = LDom2OutStream::new();
        let mut stream = StrOutStream::new();
        output.write_to(&tree, &mut stream).unwrap();

        let mut input = StrInStream::new(stream.get_data());
        let parsed = LDom2InStream::read_from(&mut input).unwrap();
        assert_eq!(parsed.tree().top().tag_name(), "root");
        assert!(parsed.chunk_ids().is_empty());
    }

    #[test]
    fn chunk_payload_can_be_loaded_into_a_data_source_container() {
        let tree = LDom2Tree::new("root");
        let mut output = LDom2OutStream::new();
        output.prepare_data_chunk(Rc::new(MemoryContainer::writable(b"content")));
        let parsed = LDom2InStream::from_bytes(&output.write_bytes(&tree).unwrap()).unwrap();
        let mut target = MemoryContainer::writable([]);

        parsed
            .read_chunk_into("datachunk:00000", &mut target)
            .unwrap();
        assert_eq!(*target.input.borrow(), b"content");
    }

    #[test]
    fn qsc_stream_rejects_malformed_chunk_markers_and_duplicate_ids() {
        let xml = write_ldom2_xml(&LDom2Tree::new("root"));
        let malformed = format!("{xml}\n{END_OF_XML_MARKER}\n==========START invalid========\n");
        assert!(LDom2InStream::from_bytes(malformed.as_bytes()).is_err());

        let chunk = "\n==========START datachunk:00000==========\na\n==========END datachunk:00000==========\n";
        let duplicate = format!("{xml}\n{END_OF_XML_MARKER}{chunk}{chunk}");
        assert!(LDom2InStream::from_bytes(duplicate.as_bytes()).is_err());
    }

    #[test]
    fn readonly_dom_containers_emit_only_when_descendants_are_modified() {
        let mut tree = LDom2Tree::new("root");
        let container = tree.top_mut().append_child("container");
        container.set_read_only(true);
        let leaf = container.append_child("leaf");
        leaf.set_default_flag(true);
        assert!(!write_ldom2_xml(&tree).contains("container"));

        tree.top_mut()
            .find_child_mut("container")
            .unwrap()
            .find_child_mut("leaf")
            .unwrap()
            .set_default_flag(false);
        assert!(write_ldom2_xml(&tree).contains("container"));
    }
}
