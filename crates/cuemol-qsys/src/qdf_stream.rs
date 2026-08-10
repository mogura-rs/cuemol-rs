//! QDF stream primitives — the tagged binary container CueMol uses for its
//! native object files (`*.qdf`). Ports the framing of C++ `qsys::QdfInStream` /
//! `qsys::QdfOutStream`: a `QDF<version>` magic, integer/float byte-order markers,
//! a file-type tag, then a sequence of named *data* blocks. Each data block
//! declares a record schema (`name`, type-tag pairs) and a record count, followed
//! by that many fixed records.
//!
//! This lives in `qsys` (matching the C++ module layout) so every object reader/
//! writer — molecular surfaces, electrostatic potentials, density maps — shares
//! one implementation of the container instead of duplicating the byte format.

use cuemol_core::{Base64InStream, Base64OutStream, CueMolError, Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use std::borrow::Cow;
use std::io::{Cursor, Read, Write};

/// QDF field type tags (the on-disk discriminators for record columns).
pub const QDF_TYPE_BOOL: i8 = 0;
pub const QDF_TYPE_UINT8: i8 = 1;
pub const QDF_TYPE_UINT16: i8 = 2;
pub const QDF_TYPE_UINT32: i8 = 3;
pub const QDF_TYPE_UINT64: i8 = 4;
/// UID fields are serialized as UINT32 (C++ `QDF_TYPE_UID = QDF_TYPE_UINT32`).
pub const QDF_TYPE_UID: i8 = QDF_TYPE_UINT32;
pub const QDF_TYPE_INT8: i8 = 11;
pub const QDF_TYPE_INT16: i8 = 12;
pub const QDF_TYPE_INT32: i8 = 13;
pub const QDF_TYPE_INT64: i8 = 14;
pub const QDF_TYPE_FLOAT32: i8 = 21;
pub const QDF_TYPE_FLOAT64: i8 = 22;
pub const QDF_TYPE_FLOAT128: i8 = 23;
pub const QDF_TYPE_FLOAT16: i8 = 24;
pub const QDF_TYPE_UTF8STR: i8 = 41;
/// Fixed-length string, length stored as a `u8` in the record schema.
pub const QDF_TYPE_FIXSTR8: i8 = 42;
/// Fixed-length string, length stored as a `u16` in the record schema.
pub const QDF_TYPE_FIXSTR16: i8 = 43;
/// Fixed-length string, length stored as a `u32` in the record schema.
pub const QDF_TYPE_FIXSTR32: i8 = 44;
pub const QDF_TYPE_VEC3: i8 = 51;
pub const QDF_TYPE_VEC4: i8 = 52;
pub const QDF_TYPE_RGB: i8 = 53;
pub const QDF_TYPE_RGBA: i8 = 54;

/// Namespace for persistent QDF type tags and version helpers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QdfDataType;

impl QdfDataType {
    pub const QDF_TYPE_BOOL: i8 = QDF_TYPE_BOOL;
    pub const QDF_TYPE_UINT8: i8 = QDF_TYPE_UINT8;
    pub const QDF_TYPE_UINT16: i8 = QDF_TYPE_UINT16;
    pub const QDF_TYPE_UINT32: i8 = QDF_TYPE_UINT32;
    pub const QDF_TYPE_UINT64: i8 = QDF_TYPE_UINT64;
    pub const QDF_TYPE_UID: i8 = QDF_TYPE_UID;
    pub const QDF_TYPE_INT8: i8 = QDF_TYPE_INT8;
    pub const QDF_TYPE_INT16: i8 = QDF_TYPE_INT16;
    pub const QDF_TYPE_INT32: i8 = QDF_TYPE_INT32;
    pub const QDF_TYPE_INT64: i8 = QDF_TYPE_INT64;
    pub const QDF_TYPE_FLOAT16: i8 = QDF_TYPE_FLOAT16;
    pub const QDF_TYPE_FLOAT32: i8 = QDF_TYPE_FLOAT32;
    pub const QDF_TYPE_FLOAT64: i8 = QDF_TYPE_FLOAT64;
    pub const QDF_TYPE_FLOAT128: i8 = QDF_TYPE_FLOAT128;
    pub const QDF_TYPE_UTF8STR: i8 = QDF_TYPE_UTF8STR;
    pub const QDF_TYPE_FIXSTR8: i8 = QDF_TYPE_FIXSTR8;
    pub const QDF_TYPE_FIXSTR16: i8 = QDF_TYPE_FIXSTR16;
    pub const QDF_TYPE_FIXSTR32: i8 = QDF_TYPE_FIXSTR32;
    pub const QDF_TYPE_VEC3: i8 = QDF_TYPE_VEC3;
    pub const QDF_TYPE_VEC4: i8 = QDF_TYPE_VEC4;
    pub const QDF_TYPE_RGB: i8 = QDF_TYPE_RGB;
    pub const QDF_TYPE_RGBA: i8 = QDF_TYPE_RGBA;

    pub fn get_size(type_tag: i8, fixed: bool) -> Result<usize> {
        if type_tag == QDF_TYPE_UTF8STR && fixed {
            return Ok(std::mem::size_of::<i32>());
        }
        qdf_type_size(type_tag).ok_or_else(|| {
            CueMolError::invalid_argument(format!(
                "QDF type {type_tag} has no context-free fixed size"
            ))
        })
    }

    pub fn create_version_string(version: u16) -> Result<String> {
        qdf_version_string(version)
    }

    pub fn parse_version_string(version: &str) -> Result<u16> {
        if version.len() != 4 {
            return Err(CueMolError::parse(
                "QDF version signature must be four bytes",
            ));
        }
        parse_qdf_version(version.as_bytes())
    }
}

/// Return the fixed byte width of a QDF value. Variable-length UTF-8 strings
/// only have a fixed four-byte length prefix and therefore return `None`.
pub fn qdf_type_size(type_tag: i8) -> Option<usize> {
    match type_tag {
        QDF_TYPE_BOOL | QDF_TYPE_UINT8 | QDF_TYPE_INT8 => Some(1),
        QDF_TYPE_UINT16 | QDF_TYPE_INT16 | QDF_TYPE_FLOAT16 => Some(2),
        QDF_TYPE_UINT32 | QDF_TYPE_INT32 | QDF_TYPE_FLOAT32 => Some(4),
        QDF_TYPE_UINT64 | QDF_TYPE_INT64 | QDF_TYPE_FLOAT64 => Some(8),
        QDF_TYPE_FLOAT128 => Some(16),
        QDF_TYPE_VEC3 | QDF_TYPE_RGB => Some(if type_tag == QDF_TYPE_VEC3 { 12 } else { 3 }),
        QDF_TYPE_VEC4 | QDF_TYPE_RGBA => Some(if type_tag == QDF_TYPE_VEC4 { 16 } else { 4 }),
        QDF_TYPE_UTF8STR | QDF_TYPE_FIXSTR8 | QDF_TYPE_FIXSTR16 | QDF_TYPE_FIXSTR32 => None,
        _ => None,
    }
}

/// A parsed record-schema field: its name, type tag, and (for `FIXSTR8`) the
/// fixed byte length declared in the schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdfFieldDef {
    pub name: String,
    pub type_tag: i8,
    pub fixed_len: usize,
}

impl QdfFieldDef {
    /// Whether a field with this name exists in a parsed schema (C++ `isDefined`).
    pub fn is_defined(defs: &[QdfFieldDef], name: &str) -> bool {
        defs.iter().any(|d| d.name == name)
    }

    /// The declared fixed length of a named `FIXSTR8` field, if present.
    pub fn fixed_len_of(defs: &[QdfFieldDef], name: &str) -> Option<usize> {
        defs.iter().find(|d| d.name == name).map(|d| d.fixed_len)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

/// Sequential reader over a QDF byte stream. Parses the header on construction
/// and then exposes the data-block / record cursor used by the object readers.
pub struct QdfReader<'a> {
    bytes: Cow<'a, [u8]>,
    offset: usize,
    endian: Endian,
    file_type: String,
    version: u16,
}

impl<'a> QdfReader<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < 4 {
            return Err(CueMolError::parse("QDF input is too short"));
        }

        let version = parse_qdf_version(&bytes[0..4])?;
        let bytes = if version == 0 {
            Cow::Borrowed(&bytes[4..])
        } else {
            if bytes.len() < 6 {
                return Err(CueMolError::parse("QDF encoded header is too short"));
            }
            let base64 = bytes[4];
            let compression = bytes[5];
            let decoded = match base64 {
                b'0' => Cow::Borrowed(&bytes[6..]),
                b'1' => Cow::Owned(Base64InStream::new(&bytes[6..]).into_bytes()),
                _ => return Err(CueMolError::parse("invalid QDF base64 encoding flag")),
            };
            match compression {
                b'0' => decoded,
                b'1' => Cow::Owned(decode_gzip(decoded.as_ref())?),
                b'3' => Cow::Owned(decode_xz(decoded.as_ref())?),
                _ => return Err(CueMolError::parse("unsupported QDF compression method")),
            }
        };
        let mut offset = 0;
        let payload = bytes.as_ref();

        let int_bo = read_u8_at(payload, &mut offset)?;
        let endian = match int_bo {
            0 => Endian::Little,
            1 => Endian::Big,
            _ => return Err(CueMolError::parse("invalid QDF integer byte order")),
        };
        let int_marker = read_i32_at(payload, &mut offset, endian)?;
        if int_marker != 12_345_678 {
            return Err(CueMolError::parse("invalid QDF integer byte-order marker"));
        }

        let float_bo = read_u8_at(payload, &mut offset)?;
        if float_bo != int_bo {
            return Err(CueMolError::parse(
                "mixed integer/float byte order is not supported",
            ));
        }
        let float_marker = read_f32_at(payload, &mut offset, endian)?;
        if (float_marker - 1.234_567_8).abs() > 1.0e-6 {
            return Err(CueMolError::parse("invalid QDF float byte-order marker"));
        }

        let _dummy = read_i32_at(payload, &mut offset, endian)?;
        let file_type = read_string_at(payload, &mut offset, endian)?;
        Ok(Self {
            bytes,
            offset,
            endian,
            file_type,
            version,
        })
    }

    pub fn version(&self) -> u16 {
        self.version
    }

    /// The file-type tag declared in the header (e.g. `"MAP1"`, `"SRF1"`).
    pub fn file_type(&self) -> &str {
        &self.file_type
    }

    pub fn expect_file_type(&self, expected: &str) -> Result<()> {
        if self.file_type == expected {
            Ok(())
        } else {
            Err(CueMolError::parse(format!(
                "QDF file type mismatch: expected {expected}, got {}",
                self.file_type
            )))
        }
    }

    /// Read a data-block header (`name`, record count). Unknown preceding blocks
    /// are skipped, matching the default behavior of C++ `readDataDef`.
    pub fn read_data_def(&mut self, expected_name: &str) -> Result<usize> {
        self.read_data_def_with_skip(expected_name, true)
    }

    /// Read a data-block header, optionally rejecting an unknown block instead
    /// of skipping it.
    pub fn read_data_def_with_skip(
        &mut self,
        expected_name: &str,
        skip_unknown: bool,
    ) -> Result<usize> {
        loop {
            let name = self.read_string()?;
            let count = self.read_nonnegative_i32()? as usize;
            if name == expected_name {
                return Ok(count);
            }
            if !skip_unknown {
                return Err(CueMolError::parse(format!(
                    "QDF data name mismatch: expected {expected_name}, got {name}"
                )));
            }
            self.skip_records(count)?;
        }
    }

    /// Read and validate a record schema against the expected `(name, type)` list.
    pub fn expect_record_def(&mut self, expected: &[(&str, i8)]) -> Result<()> {
        let count = self.read_nonnegative_i32()? as usize;
        if count != expected.len() {
            return Err(CueMolError::parse(format!(
                "QDF record count mismatch: expected {}, got {count}",
                expected.len()
            )));
        }
        for &(expected_name, expected_type) in expected {
            let name = self.read_string()?;
            let record_type = self.read_i8()?;
            if name != expected_name || record_type != expected_type {
                return Err(CueMolError::parse(format!(
                    "QDF record mismatch: expected {expected_name}/{expected_type}, got {name}/{record_type}"
                )));
            }
        }
        Ok(())
    }

    pub fn read_string(&mut self) -> Result<String> {
        read_string_at(self.bytes.as_ref(), &mut self.offset, self.endian)
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(read_u8_at(self.bytes.as_ref(), &mut self.offset)? as i8)
    }

    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_u8()? != 0)
    }

    pub fn read_nonnegative_i32(&mut self) -> Result<i32> {
        let value = self.read_i32()?;
        if value < 0 {
            return Err(CueMolError::parse("QDF negative integer is invalid here"));
        }
        Ok(value)
    }

    /// Read a signed 32-bit integer (e.g. a map start index, which is often
    /// negative).
    pub fn read_i32(&mut self) -> Result<i32> {
        read_i32_at(self.bytes.as_ref(), &mut self.offset, self.endian)
    }

    pub fn read_i16(&mut self) -> Result<i16> {
        let field = read_array_at::<2>(self.bytes.as_ref(), &mut self.offset)?;
        Ok(match self.endian {
            Endian::Little => i16::from_le_bytes(field),
            Endian::Big => i16::from_be_bytes(field),
        })
    }

    pub fn read_i64(&mut self) -> Result<i64> {
        let field = read_array_at::<8>(self.bytes.as_ref(), &mut self.offset)?;
        Ok(match self.endian {
            Endian::Little => i64::from_le_bytes(field),
            Endian::Big => i64::from_be_bytes(field),
        })
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        Ok(read_i32_at(self.bytes.as_ref(), &mut self.offset, self.endian)? as u32)
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        Ok(self.read_i16()? as u16)
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        Ok(self.read_i64()? as u64)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        read_u8_at(self.bytes.as_ref(), &mut self.offset)
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        read_f32_at(self.bytes.as_ref(), &mut self.offset, self.endian)
    }

    pub fn read_f64(&mut self) -> Result<f64> {
        let field = read_array_at::<8>(self.bytes.as_ref(), &mut self.offset)?;
        Ok(match self.endian {
            Endian::Little => f64::from_le_bytes(field),
            Endian::Big => f64::from_be_bytes(field),
        })
    }

    pub fn read_vec3(&mut self) -> Result<[f32; 3]> {
        Ok([self.read_f32()?, self.read_f32()?, self.read_f32()?])
    }

    pub fn read_vec4(&mut self) -> Result<[f32; 4]> {
        Ok([
            self.read_f32()?,
            self.read_f32()?,
            self.read_f32()?,
            self.read_f32()?,
        ])
    }

    pub fn read_rgb(&mut self) -> Result<[u8; 3]> {
        Ok([self.read_u8()?, self.read_u8()?, self.read_u8()?])
    }

    pub fn read_rgba(&mut self) -> Result<[u8; 4]> {
        Ok([
            self.read_u8()?,
            self.read_u8()?,
            self.read_u8()?,
            self.read_u8()?,
        ])
    }

    /// Read a fixed-length string: exactly `len` bytes, truncated at the first
    /// NUL (C++ `QdfInStream::readStr` for FIXSTR types).
    /// Read a length-prefixed raw byte blob (`i32` length + bytes) — like a
    /// UTF8STR field but without the UTF-8 validation, for opaque payloads.
    pub fn read_blob(&mut self) -> Result<Vec<u8>> {
        let len = self.read_nonnegative_i32()? as usize;
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| CueMolError::parse("QDF blob length overflow"))?;
        let Some(field) = self.bytes.get(self.offset..end) else {
            return Err(CueMolError::parse("unexpected end of QDF blob"));
        };
        self.offset = end;
        Ok(field.to_vec())
    }

    pub fn read_fixed_str(&mut self, len: usize) -> Result<String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| CueMolError::parse("QDF fixed string length overflow"))?;
        let Some(field) = self.bytes.get(self.offset..end) else {
            return Err(CueMolError::parse("unexpected end of QDF fixed string"));
        };
        self.offset = end;
        let trimmed: Vec<u8> = field.iter().copied().take_while(|&b| b != 0).collect();
        Ok(String::from_utf8_lossy(&trimmed).into_owned())
    }

    /// Parse a record schema (`startData` block): a count followed by each
    /// field's `(name, type[, fixstr length])`. Returns the field list so the
    /// caller can validate names/types and look up FIXSTR lengths and optional
    /// fields (C++ reads the schema then dispatches per field).
    pub fn read_record_def(&mut self) -> Result<Vec<QdfFieldDef>> {
        let count = self.read_nonnegative_i32()? as usize;
        let mut defs = Vec::with_capacity(count);
        for _ in 0..count {
            let name = self.read_string()?;
            let type_tag = self.read_i8()?;
            let fixed_len = match type_tag {
                QDF_TYPE_FIXSTR8 => self.read_u8()? as usize,
                QDF_TYPE_FIXSTR16 => self.read_u16()? as usize,
                QDF_TYPE_FIXSTR32 => usize::try_from(self.read_u32()?)
                    .map_err(|_| CueMolError::parse("QDF fixed string length overflow"))?,
                _ => 0,
            };
            defs.push(QdfFieldDef {
                name,
                type_tag,
                fixed_len,
            });
        }
        Ok(defs)
    }

    /// Skip a complete data block when its schema is not needed by this reader.
    pub fn skip_all_records(&mut self) -> Result<()> {
        let _name = self.read_string()?;
        let count = self.read_nonnegative_i32()? as usize;
        self.skip_records(count)
    }

    fn skip_records(&mut self, count: usize) -> Result<()> {
        let defs = self.read_record_def()?;
        for _ in 0..count {
            for def in &defs {
                self.skip_field(def)?;
            }
        }
        Ok(())
    }

    fn skip_field(&mut self, def: &QdfFieldDef) -> Result<()> {
        let len = match def.type_tag {
            QDF_TYPE_UTF8STR => self.read_nonnegative_i32()? as usize,
            QDF_TYPE_FIXSTR8 | QDF_TYPE_FIXSTR16 | QDF_TYPE_FIXSTR32 => def.fixed_len,
            type_tag => qdf_type_size(type_tag).ok_or_else(|| {
                CueMolError::parse(format!("unsupported QDF record type {type_tag}"))
            })?,
        };
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| CueMolError::parse("QDF field length overflow"))?;
        if end > self.bytes.len() {
            return Err(CueMolError::parse("unexpected end of QDF field"));
        }
        self.offset = end;
        Ok(())
    }
}

/// Sequential writer producing a little-endian QDF byte stream.
pub struct QdfWriter {
    bytes: Vec<u8>,
}

impl QdfWriter {
    pub fn new(file_type: &str) -> Self {
        let mut writer = Self { bytes: Vec::new() };
        writer.bytes.extend_from_slice(b"QDF0");
        writer.i8(0);
        writer.i32(12_345_678);
        writer.i8(0);
        writer.f32(1.234_567_8);
        writer.i32(2);
        writer.string(file_type);
        writer
    }

    pub fn data_def(&mut self, name: &str, count: usize) {
        self.string(name);
        self.i32(count as i32);
    }

    pub fn record_def(&mut self, records: &[(&str, i8)]) {
        self.i32(records.len() as i32);
        for &(name, record_type) in records {
            self.string(name);
            self.i8(record_type);
        }
    }

    /// Write a record schema whose fields carry a fixed length (used by
    /// `FIXSTR` columns; the length is emitted after the type tag, matching C++
    /// `QdfOutStream::startData`). Non-FIXSTR fields ignore `fixed_len`.
    pub fn record_def_ext(&mut self, records: &[(&str, i8, usize)]) {
        self.i32(records.len() as i32);
        for &(name, record_type, fixed_len) in records {
            self.string(name);
            self.i8(record_type);
            match record_type {
                QDF_TYPE_FIXSTR8 => self.u8(fixed_len as u8),
                QDF_TYPE_FIXSTR16 => self.u16(fixed_len as u16),
                QDF_TYPE_FIXSTR32 => self.u32(fixed_len as u32),
                _ => {}
            }
        }
    }

    pub fn string(&mut self, value: &str) {
        self.i32(value.len() as i32);
        self.bytes.extend_from_slice(value.as_bytes());
    }

    /// Write a length-prefixed raw byte blob (`i32` length + bytes), for opaque
    /// payloads that are not valid UTF-8 (pairs with [`QdfReader::read_blob`]).
    pub fn blob(&mut self, value: &[u8]) {
        self.i32(value.len() as i32);
        self.bytes.extend_from_slice(value);
    }

    pub fn i8(&mut self, value: i8) {
        self.bytes.push(value as u8);
    }

    pub fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub fn i16(&mut self, value: i16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn vec3(&mut self, value: [f32; 3]) {
        for component in value {
            self.f32(component);
        }
    }

    pub fn vec4(&mut self, value: [f32; 4]) {
        for component in value {
            self.f32(component);
        }
    }

    pub fn rgb(&mut self, value: [u8; 3]) {
        self.bytes.extend_from_slice(&value);
    }

    pub fn rgba(&mut self, value: [u8; 4]) {
        self.bytes.extend_from_slice(&value);
    }

    /// Write a fixed-length string field: the value's bytes padded with NUL (or
    /// truncated) to exactly `maxlen` bytes (C++ FIXSTR data has no length
    /// prefix — the length lives in the schema).
    pub fn fixed_str(&mut self, value: &str, maxlen: usize) {
        let b = value.as_bytes();
        let n = b.len().min(maxlen);
        self.bytes.extend_from_slice(&b[..n]);
        self.bytes.resize(self.bytes.len() + (maxlen - n), 0);
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    /// Finish as a versioned QDF stream using the original two-character
    /// encoding selector: optional base64 followed by no compression (`0`),
    /// gzip (`1`), or XZ (`3`).
    pub fn finish_with_encoding(self, version: u16, encoding: &str) -> Result<Vec<u8>> {
        encode_qdf0_bytes(&self.bytes, version, encoding)
    }
}

pub type QdfAbsReader<'a> = QdfReader<'a>;
pub type QdfInStream<'a> = QdfReader<'a>;
pub type QdfAbsWriter = QdfWriter;
pub type QdfOutStream = QdfWriter;

/// The file-type tag of a QDF stream, or `None` if the header is unparseable.
pub fn qdf_file_type(input: &[u8]) -> Option<String> {
    QdfReader::from_bytes(input)
        .ok()
        .map(|reader| reader.file_type)
}

fn parse_qdf_version(header: &[u8]) -> Result<u16> {
    let text = std::str::from_utf8(header)
        .map_err(|_| CueMolError::parse("QDF version header is not UTF-8"))?;
    let digits = if let Some(rest) = text.strip_prefix("QDF") {
        rest
    } else if let Some(rest) = text.strip_prefix("QD") {
        rest
    } else if let Some(rest) = text.strip_prefix('Q') {
        rest
    } else {
        return Err(CueMolError::parse("invalid QDF version header"));
    };
    u16::from_str_radix(digits, 16).map_err(|_| CueMolError::parse("invalid QDF version number"))
}

/// Create the compact four-byte QDF version signature used by CueMol.
pub fn qdf_version_string(version: u16) -> Result<String> {
    match version {
        0x000..=0x00f => Ok(format!("QDF{version:x}")),
        0x010..=0x0ff => Ok(format!("QD{version:02x}")),
        0x100..=0xfff => Ok(format!("Q{version:03x}")),
        _ => Err(CueMolError::invalid_argument(
            "QDF version number is too large",
        )),
    }
}

/// Re-encode an existing QDF0 byte stream using CueMol's versioned QDF
/// envelope. This is used when an object writer's binary output is embedded in
/// a text `.qsc` data chunk.
pub fn encode_qdf0_bytes(input: &[u8], version: u16, encoding: &str) -> Result<Vec<u8>> {
    if !input.starts_with(b"QDF0") {
        return Err(CueMolError::parse("QDF0 input signature is missing"));
    }
    if version == 0 {
        if encoding != "00" {
            return Err(CueMolError::invalid_argument(
                "QDF0 does not carry an encoding selector",
            ));
        }
        return Ok(input.to_vec());
    }
    let flags = encoding.as_bytes();
    if flags.len() != 2
        || !matches!(flags[0], b'0' | b'1')
        || !matches!(flags[1], b'0' | b'1' | b'3')
    {
        return Err(CueMolError::invalid_argument(
            "QDF encoding must be a two-character base64/compression selector",
        ));
    }

    let mut output = qdf_version_string(version)?.into_bytes();
    output.extend_from_slice(flags);
    let raw_payload = &input[4..];
    let payload = match flags[1] {
        b'0' => raw_payload.to_vec(),
        b'1' => encode_gzip(raw_payload)?,
        b'3' => encode_xz(raw_payload)?,
        _ => unreachable!(),
    };
    if flags[0] == b'1' {
        let mut encoder = Base64OutStream::new();
        encoder.write(&payload, 0, payload.len());
        output.extend_from_slice(&encoder.finish());
    } else {
        output.extend_from_slice(&payload);
    }
    Ok(output)
}

/// Decode any supported versioned QDF stream back to the unencoded QDF0 form.
pub fn decode_qdf_to_qdf0_bytes(input: &[u8]) -> Result<Vec<u8>> {
    if input.len() < 4 {
        return Err(CueMolError::parse("QDF input is too short"));
    }
    let version = parse_qdf_version(&input[..4])?;
    if version == 0 {
        return Ok(input.to_vec());
    }
    if input.len() < 6 {
        return Err(CueMolError::parse("QDF encoded header is too short"));
    }
    let decoded = match input[4] {
        b'0' => input[6..].to_vec(),
        b'1' => Base64InStream::new(&input[6..]).into_bytes(),
        _ => return Err(CueMolError::parse("invalid QDF base64 encoding flag")),
    };
    let payload = match input[5] {
        b'0' => decoded,
        b'1' => decode_gzip(&decoded)?,
        b'3' => decode_xz(&decoded)?,
        _ => return Err(CueMolError::parse("unsupported QDF compression method")),
    };
    let mut output = b"QDF0".to_vec();
    output.extend_from_slice(&payload);
    Ok(output)
}

fn decode_gzip(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(input);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|err| CueMolError::parse(format!("invalid gzip QDF payload: {err}")))?;
    Ok(output)
}

fn encode_gzip(input: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(input)
        .map_err(|err| CueMolError::Io(err.to_string()))?;
    encoder
        .finish()
        .map_err(|err| CueMolError::Io(err.to_string()))
}

fn decode_xz(input: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    lzma_rs::xz_decompress(&mut Cursor::new(input), &mut output)
        .map_err(|err| CueMolError::parse(format!("invalid XZ QDF payload: {err}")))?;
    Ok(output)
}

fn encode_xz(input: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    lzma_rs::xz_compress(&mut Cursor::new(input), &mut output)
        .map_err(|err| CueMolError::Io(err.to_string()))?;
    Ok(output)
}

fn read_u8_at(bytes: &[u8], offset: &mut usize) -> Result<u8> {
    let Some(value) = bytes.get(*offset).copied() else {
        return Err(CueMolError::parse("unexpected end of QDF data"));
    };
    *offset += 1;
    Ok(value)
}

fn read_i32_at(bytes: &[u8], offset: &mut usize, endian: Endian) -> Result<i32> {
    let field = read_array_at::<4>(bytes, offset)?;
    Ok(match endian {
        Endian::Little => i32::from_le_bytes(field),
        Endian::Big => i32::from_be_bytes(field),
    })
}

fn read_f32_at(bytes: &[u8], offset: &mut usize, endian: Endian) -> Result<f32> {
    let field = read_array_at::<4>(bytes, offset)?;
    Ok(match endian {
        Endian::Little => f32::from_le_bytes(field),
        Endian::Big => f32::from_be_bytes(field),
    })
}

fn read_string_at(bytes: &[u8], offset: &mut usize, endian: Endian) -> Result<String> {
    let len = read_i32_at(bytes, offset, endian)?;
    if len < 0 {
        return Err(CueMolError::parse("QDF string length is negative"));
    }
    let len = len as usize;
    let end = offset
        .checked_add(len)
        .ok_or_else(|| CueMolError::parse("QDF string length overflow"))?;
    let Some(field) = bytes.get(*offset..end) else {
        return Err(CueMolError::parse("unexpected end of QDF string"));
    };
    *offset = end;
    std::str::from_utf8(field)
        .map(str::to_string)
        .map_err(|_| CueMolError::parse("QDF string is not UTF-8"))
}

fn read_array_at<const N: usize>(bytes: &[u8], offset: &mut usize) -> Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or_else(|| CueMolError::parse("QDF offset overflow"))?;
    let Some(field) = bytes.get(*offset..end) else {
        return Err(CueMolError::parse("unexpected end of QDF data"));
    };
    *offset = end;
    field
        .try_into()
        .map_err(|_| CueMolError::parse("invalid QDF field length"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qdf_version_strings_match_reference_ranges() {
        assert_eq!(qdf_version_string(1).unwrap(), "QDF1");
        assert_eq!(qdf_version_string(0x12).unwrap(), "QD12");
        assert_eq!(qdf_version_string(0x123).unwrap(), "Q123");
        assert!(qdf_version_string(0x1000).is_err());
        assert_eq!(QdfDataType::parse_version_string("QD12").unwrap(), 0x12);
        assert_eq!(QdfDataType::get_size(QDF_TYPE_UTF8STR, true).unwrap(), 4);
        assert!(QdfDataType::get_size(QDF_TYPE_UTF8STR, false).is_err());
    }

    #[test]
    fn encoded_qdf1_roundtrips_through_reader() {
        for encoding in ["00", "10", "01", "11", "03", "13"] {
            assert_qdf_roundtrip(encoding);
        }
    }

    fn assert_qdf_roundtrip(encoding: &str) {
        let mut writer = QdfWriter::new("TEST");
        writer.data_def("values", 1);
        writer.record_def(&[("value", QDF_TYPE_INT32)]);
        writer.i32(42);
        let raw = writer.finish();
        let encoded = encode_qdf0_bytes(&raw, 1, encoding).unwrap();
        assert_eq!(&encoded[..4], b"QDF1");
        assert_eq!(&encoded[4..6], encoding.as_bytes());
        assert_eq!(decode_qdf_to_qdf0_bytes(&encoded).unwrap(), raw);

        let mut reader = QdfReader::from_bytes(&encoded).unwrap();
        assert_eq!(reader.version(), 1);
        assert_eq!(reader.file_type(), "TEST");
        assert_eq!(reader.read_data_def("values").unwrap(), 1);
        reader
            .expect_record_def(&[("value", QDF_TYPE_INT32)])
            .unwrap();
        assert_eq!(reader.read_i32().unwrap(), 42);
    }

    #[test]
    fn plain_qdf1_roundtrips_and_invalid_flags_fail() {
        let encoded = QdfWriter::new("TEST")
            .finish_with_encoding(2, "00")
            .unwrap();
        let reader = QdfReader::from_bytes(&encoded).unwrap();
        assert_eq!(reader.version(), 2);
        assert_eq!(reader.file_type(), "TEST");

        let mut invalid = encoded;
        invalid[4] = b'2';
        assert!(QdfReader::from_bytes(&invalid).is_err());
        assert!(QdfWriter::new("TEST")
            .finish_with_encoding(1, "12")
            .is_err());
    }

    #[test]
    fn extended_record_types_roundtrip() {
        let mut writer = QdfWriter::new("TEST");
        writer.data_def("extended", 1);
        writer.record_def_ext(&[
            ("bool", QDF_TYPE_BOOL, 0),
            ("u16", QDF_TYPE_UINT16, 0),
            ("u64", QDF_TYPE_UINT64, 0),
            ("i16", QDF_TYPE_INT16, 0),
            ("i64", QDF_TYPE_INT64, 0),
            ("f64", QDF_TYPE_FLOAT64, 0),
            ("vec3", QDF_TYPE_VEC3, 0),
            ("vec4", QDF_TYPE_VEC4, 0),
            ("rgb", QDF_TYPE_RGB, 0),
            ("rgba", QDF_TYPE_RGBA, 0),
            ("long_name", QDF_TYPE_FIXSTR16, 300),
        ]);
        writer.bool(true);
        writer.u16(65_000);
        writer.u64(u64::MAX - 7);
        writer.i16(-12_345);
        writer.i64(-9_876_543_210);
        writer.f64(std::f64::consts::PI);
        writer.vec3([1.0, -2.0, 3.5]);
        writer.vec4([4.0, 5.0, 6.0, 7.0]);
        writer.rgb([1, 2, 3]);
        writer.rgba([4, 5, 6, 7]);
        writer.fixed_str("a long fixed field", 300);

        let bytes = writer.finish();
        let mut reader = QdfReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.read_data_def("extended").unwrap(), 1);
        let defs = reader.read_record_def().unwrap();
        assert_eq!(QdfFieldDef::fixed_len_of(&defs, "long_name"), Some(300));
        assert!(reader.read_bool().unwrap());
        assert_eq!(reader.read_u16().unwrap(), 65_000);
        assert_eq!(reader.read_u64().unwrap(), u64::MAX - 7);
        assert_eq!(reader.read_i16().unwrap(), -12_345);
        assert_eq!(reader.read_i64().unwrap(), -9_876_543_210);
        assert_eq!(reader.read_f64().unwrap(), std::f64::consts::PI);
        assert_eq!(reader.read_vec3().unwrap(), [1.0, -2.0, 3.5]);
        assert_eq!(reader.read_vec4().unwrap(), [4.0, 5.0, 6.0, 7.0]);
        assert_eq!(reader.read_rgb().unwrap(), [1, 2, 3]);
        assert_eq!(reader.read_rgba().unwrap(), [4, 5, 6, 7]);
        assert_eq!(reader.read_fixed_str(300).unwrap(), "a long fixed field");
    }

    #[test]
    fn data_def_skips_unknown_blocks_using_their_schema() {
        let mut writer = QdfWriter::new("TEST");
        writer.data_def("future", 2);
        writer.record_def_ext(&[
            ("id", QDF_TYPE_UINT64, 0),
            ("position", QDF_TYPE_VEC3, 0),
            ("label", QDF_TYPE_UTF8STR, 0),
            ("fixed", QDF_TYPE_FIXSTR16, 260),
        ]);
        for id in [11, 12] {
            writer.u64(id);
            writer.vec3([id as f32, 2.0, 3.0]);
            writer.string("ignored");
            writer.fixed_str("also ignored", 260);
        }
        writer.data_def("wanted", 1);
        writer.record_def(&[("value", QDF_TYPE_INT32)]);
        writer.i32(42);

        let bytes = writer.finish();
        let mut reader = QdfReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.read_data_def("wanted").unwrap(), 1);
        reader
            .expect_record_def(&[("value", QDF_TYPE_INT32)])
            .unwrap();
        assert_eq!(reader.read_i32().unwrap(), 42);

        let mut strict = QdfReader::from_bytes(&bytes).unwrap();
        assert!(strict.read_data_def_with_skip("wanted", false).is_err());
    }

    #[test]
    fn qdf_type_sizes_cover_reference_type_constants() {
        assert_eq!(qdf_type_size(QDF_TYPE_BOOL), Some(1));
        assert_eq!(qdf_type_size(QDF_TYPE_UINT16), Some(2));
        assert_eq!(qdf_type_size(QDF_TYPE_FLOAT64), Some(8));
        assert_eq!(qdf_type_size(QDF_TYPE_VEC3), Some(12));
        assert_eq!(qdf_type_size(QDF_TYPE_VEC4), Some(16));
        assert_eq!(qdf_type_size(QDF_TYPE_RGB), Some(3));
        assert_eq!(qdf_type_size(QDF_TYPE_RGBA), Some(4));
        assert_eq!(qdf_type_size(QDF_TYPE_UTF8STR), None);
        assert_eq!(qdf_type_size(127), None);
    }
}
