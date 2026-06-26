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

use cuemol_core::{CueMolError, Result};

/// QDF field type tags (the on-disk discriminators for record columns).
pub const QDF_TYPE_BOOL: i8 = 0;
pub const QDF_TYPE_UINT8: i8 = 1;
pub const QDF_TYPE_UINT32: i8 = 3;
/// UID fields are serialized as UINT32 (C++ `QDF_TYPE_UID = QDF_TYPE_UINT32`).
pub const QDF_TYPE_UID: i8 = QDF_TYPE_UINT32;
pub const QDF_TYPE_INT8: i8 = 11;
pub const QDF_TYPE_INT32: i8 = 13;
pub const QDF_TYPE_FLOAT32: i8 = 21;
pub const QDF_TYPE_UTF8STR: i8 = 41;
/// Fixed-length string, length stored as a `u8` in the record schema.
pub const QDF_TYPE_FIXSTR8: i8 = 42;

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
    bytes: &'a [u8],
    offset: usize,
    endian: Endian,
    file_type: String,
}

impl<'a> QdfReader<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < 4 {
            return Err(CueMolError::parse("QDF input is too short"));
        }

        let version = parse_qdf_version(&bytes[0..4])?;
        let mut offset = if version == 0 {
            4
        } else {
            if bytes.len() < 6 {
                return Err(CueMolError::parse("QDF encoded header is too short"));
            }
            if &bytes[4..6] != b"00" {
                return Err(CueMolError::parse(
                    "encoded or compressed QDF streams are not supported",
                ));
            }
            6
        };

        let int_bo = read_u8_at(bytes, &mut offset)?;
        let endian = match int_bo {
            0 => Endian::Little,
            1 => Endian::Big,
            _ => return Err(CueMolError::parse("invalid QDF integer byte order")),
        };
        let int_marker = read_i32_at(bytes, &mut offset, endian)?;
        if int_marker != 12_345_678 {
            return Err(CueMolError::parse("invalid QDF integer byte-order marker"));
        }

        let float_bo = read_u8_at(bytes, &mut offset)?;
        if float_bo != int_bo {
            return Err(CueMolError::parse(
                "mixed integer/float byte order is not supported",
            ));
        }
        let float_marker = read_f32_at(bytes, &mut offset, endian)?;
        if (float_marker - 1.234_567_8).abs() > 1.0e-6 {
            return Err(CueMolError::parse("invalid QDF float byte-order marker"));
        }

        let _dummy = read_i32_at(bytes, &mut offset, endian)?;
        let file_type = read_string_at(bytes, &mut offset, endian)?;
        Ok(Self {
            bytes,
            offset,
            endian,
            file_type,
        })
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

    /// Read a data-block header (`name`, record count); errors on a name mismatch.
    pub fn read_data_def(&mut self, expected_name: &str) -> Result<usize> {
        let name = self.read_string()?;
        if name != expected_name {
            return Err(CueMolError::parse(format!(
                "QDF data name mismatch: expected {expected_name}, got {name}"
            )));
        }
        Ok(self.read_nonnegative_i32()? as usize)
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
        read_string_at(self.bytes, &mut self.offset, self.endian)
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(read_u8_at(self.bytes, &mut self.offset)? as i8)
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
        read_i32_at(self.bytes, &mut self.offset, self.endian)
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        Ok(read_i32_at(self.bytes, &mut self.offset, self.endian)? as u32)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        read_u8_at(self.bytes, &mut self.offset)
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        read_f32_at(self.bytes, &mut self.offset, self.endian)
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
            let fixed_len = if type_tag == QDF_TYPE_FIXSTR8 {
                self.read_u8()? as usize
            } else {
                0
            };
            defs.push(QdfFieldDef {
                name,
                type_tag,
                fixed_len,
            });
        }
        Ok(defs)
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
    /// `FIXSTR8` columns; the length is emitted after the type tag, matching C++
    /// `QdfOutStream::startData`). Non-FIXSTR fields ignore `fixed_len`.
    pub fn record_def_ext(&mut self, records: &[(&str, i8, usize)]) {
        self.i32(records.len() as i32);
        for &(name, record_type, fixed_len) in records {
            self.string(name);
            self.i8(record_type);
            if record_type == QDF_TYPE_FIXSTR8 {
                self.u8(fixed_len as u8);
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

    pub fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
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
}

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
