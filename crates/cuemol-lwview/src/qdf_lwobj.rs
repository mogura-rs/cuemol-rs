//! QDF lightweight-object reader/writer — CueMol's native `*.qdf` container for
//! a [`LWObject`] (the lightweight-viewer render object). Ports the role of C++
//! `lwview::QdfLWObjReader` / `QdfLWObjWriter` (file type `"LWO3"`).
//!
//! SCOPE / honesty: the Rust [`LWObject`] models render data as opaque
//! `data_chunks` (section name + byte payload) plus `hit_data`, NOT the C++
//! typed vertex/pixel/hit blocks (`vatr`/`vcat`/`pixd`/...). So this round-trips
//! the Rust abstraction faithfully (name, visible, chunks, hit data) but is NOT
//! byte-compatible with a C++-written LWO3 stream — and there is no `.qdf`
//! fixture, so verification is round-trip self-consistency only.

use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_qsys::{
    InOutHandler, ObjReader, ObjWriter, QdfReader, QdfWriter, QDF_TYPE_FLOAT32, QDF_TYPE_INT32,
    QDF_TYPE_INT8, QDF_TYPE_UTF8STR,
};

use crate::LWObject;

const HEAD_RECORD: &[(&str, i8)] = &[("name", QDF_TYPE_UTF8STR), ("vis", QDF_TYPE_INT8)];
// `data` is a raw byte blob, written/read via blob() (UTF8STR is just the schema
// tag — the wire form, i32 length + bytes, is identical).
const CHNK_RECORD: &[(&str, i8)] = &[("sect", QDF_TYPE_UTF8STR), ("data", QDF_TYPE_UTF8STR)];
const HIT_RECORD: &[(&str, i8)] = &[
    ("id", QDF_TYPE_INT32),
    ("msg", QDF_TYPE_UTF8STR),
    ("posx", QDF_TYPE_FLOAT32),
    ("posy", QDF_TYPE_FLOAT32),
    ("posz", QDF_TYPE_FLOAT32),
    ("rend", QDF_TYPE_INT8),
];

/// Serialize an `LWObject` to a QDF `LWO3` byte stream.
pub fn write_qdf_lwobj_bytes(obj: &LWObject) -> Vec<u8> {
    let mut w = QdfWriter::new("LWO3");

    w.data_def("head", 1);
    w.record_def(HEAD_RECORD);
    w.string(obj.name());
    w.i8(obj.is_visible() as i8);

    w.data_def("chnk", obj.data_chunks().len());
    w.record_def(CHNK_RECORD);
    for chunk in obj.data_chunks() {
        w.string(chunk.section());
        w.blob(chunk.payload());
    }

    w.data_def("hit", obj.hit_data_len());
    w.record_def(HIT_RECORD);
    for index in 0..obj.hit_data_len() as i32 {
        let hit = obj.hit_data(index).expect("hit index in range");
        w.i32(hit.id());
        w.string(hit.message());
        let p = hit.position();
        w.f32(p.x as f32);
        w.f32(p.y as f32);
        w.f32(p.z as f32);
        w.i8(hit.is_rendered() as i8);
    }

    w.finish()
}

/// Parse a QDF `LWO3` byte stream into an `LWObject`.
pub fn read_qdf_lwobj_bytes(input: &[u8]) -> Result<LWObject> {
    let mut r = QdfReader::from_bytes(input)?;
    r.expect_file_type("LWO3")?;
    let mut obj = LWObject::new();

    if r.read_data_def("head")? != 1 {
        return Err(CueMolError::parse("QDF LWO3 head block must be 1"));
    }
    r.expect_record_def(HEAD_RECORD)?;
    obj.set_name(r.read_string()?);
    obj.set_visible(r.read_i8()? != 0);

    let nchnk = r.read_data_def("chnk")?;
    r.expect_record_def(CHNK_RECORD)?;
    for _ in 0..nchnk {
        let section = r.read_string()?;
        let payload = r.read_blob()?;
        obj.add_data_chunk(section, payload);
    }

    let nhit = r.read_data_def("hit")?;
    r.expect_record_def(HIT_RECORD)?;
    for _ in 0..nhit {
        let id = r.read_i32()?;
        let message = r.read_string()?;
        let pos = Vector4::new3(
            r.read_f32()? as f64,
            r.read_f32()? as f64,
            r.read_f32()? as f64,
        );
        let rendered = r.read_i8()? != 0;
        let index = obj.add_point_hit(id, pos, message);
        if !rendered {
            if let Some(hit) = obj.hit_data_mut(index) {
                hit.set_rendered(false);
            }
        }
    }

    Ok(obj)
}

/// `ObjReader` facade for QDF lightweight objects (handler name `qdflwobj`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfLWObjReader {
    reader: ObjReader,
}

impl QdfLWObjReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("qdflwobj", "CueMol lightweight object (*.qdf)", "*.qdf"),
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn create_default_obj(&self) -> LWObject {
        LWObject::new()
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<LWObject> {
        read_qdf_lwobj_bytes(input)
    }
}

impl Default for QdfLWObjReader {
    fn default() -> Self {
        Self::new()
    }
}

/// `ObjWriter` facade for QDF lightweight objects (handler name `qdflwobj`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfLWObjWriter {
    writer: ObjWriter,
}

impl QdfLWObjWriter {
    pub fn new() -> Self {
        Self {
            writer: ObjWriter::new("qdflwobj", "CueMol lightweight object (*.qdf)", "*.qdf"),
        }
    }

    pub fn handler(&self) -> &InOutHandler {
        self.writer.handler()
    }

    pub fn can_handle(&self, _object: &LWObject) -> bool {
        true
    }

    pub fn write_bytes(&self, object: &LWObject) -> Vec<u8> {
        write_qdf_lwobj_bytes(object)
    }
}

impl Default for QdfLWObjWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_qsys::qdf_file_type;

    fn sample() -> LWObject {
        let mut obj = LWObject::new();
        obj.set_name("mesh1");
        obj.set_visible(false);
        obj.add_data_chunk("vatr", vec![1u8, 2, 3, 250, 0, 128]);
        obj.add_data_chunk("vnci", vec![9u8, 8, 7]);
        obj.add_point_hit(42, Vector4::new3(1.0, 2.0, 3.0), "atom:CA");
        let idx = obj.add_point_hit(7, Vector4::new3(-4.0, 5.0, 6.0), "bond");
        obj.hit_data_mut(idx).unwrap().set_rendered(false);
        obj
    }

    #[test]
    fn qdf_lwobj_roundtrip_preserves_chunks_and_hits() {
        let src = sample();
        let bytes = write_qdf_lwobj_bytes(&src);
        assert_eq!(qdf_file_type(&bytes).as_deref(), Some("LWO3"));

        let dst = read_qdf_lwobj_bytes(&bytes).expect("read back LWO3");

        assert_eq!(dst.name(), "mesh1");
        assert!(!dst.is_visible());

        assert_eq!(dst.data_chunks().len(), 2);
        assert_eq!(dst.data_chunks()[0].section(), "vatr");
        assert_eq!(dst.data_chunks()[0].payload(), &[1, 2, 3, 250, 0, 128]);
        assert_eq!(dst.data_chunks()[1].section(), "vnci");
        assert_eq!(dst.data_chunks()[1].payload(), &[9, 8, 7]);

        assert_eq!(dst.hit_data_len(), 2);
        let h0 = dst.hit_data(0).unwrap();
        assert_eq!(h0.id(), 42);
        assert_eq!(h0.message(), "atom:CA");
        assert!((h0.position().x - 1.0).abs() < 1.0e-5);
        assert!(h0.is_rendered());
        let h1 = dst.hit_data(1).unwrap();
        assert_eq!(h1.id(), 7);
        assert!(!h1.is_rendered());
        assert!((h1.position().y - 5.0).abs() < 1.0e-5);
    }

    #[test]
    fn read_rejects_non_lwo_stream() {
        let bytes = QdfWriter::new("MOL2").finish();
        assert!(read_qdf_lwobj_bytes(&bytes).is_err());
    }

    #[test]
    fn handler_metadata_matches_reference() {
        let reader = QdfLWObjReader::new();
        assert_eq!(reader.handler().name(), "qdflwobj");
        assert_eq!(reader.handler().file_ext(), "*.qdf");
        let writer = QdfLWObjWriter::new();
        assert!(writer.can_handle(&LWObject::new()));
    }
}
