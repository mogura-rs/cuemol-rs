use crate::{type_consts, CueMolError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Int8,
    Int16,
    Int32,
    Int64,
    Float8,
    Float16,
    Float32,
    Float64,
    Float128,
}

impl ElementType {
    pub const UINT8: i32 = 1;
    pub const UINT16: i32 = 2;
    pub const UINT32: i32 = 3;
    pub const UINT64: i32 = 4;
    pub const INT8: i32 = 11;
    pub const INT16: i32 = 12;
    pub const INT32: i32 = 13;
    pub const INT64: i32 = 14;
    pub const FLOAT8: i32 = 19;
    pub const FLOAT16: i32 = 20;
    pub const FLOAT32: i32 = 21;
    pub const FLOAT64: i32 = 22;
    pub const FLOAT128: i32 = 23;

    pub fn from_code(code: i32) -> Result<Self> {
        match code {
            Self::UINT8 => Ok(Self::Uint8),
            Self::UINT16 => Ok(Self::Uint16),
            Self::UINT32 => Ok(Self::Uint32),
            Self::UINT64 => Ok(Self::Uint64),
            Self::INT8 => Ok(Self::Int8),
            Self::INT16 => Ok(Self::Int16),
            Self::INT32 => Ok(Self::Int32),
            Self::INT64 => Ok(Self::Int64),
            Self::FLOAT8 => Ok(Self::Float8),
            Self::FLOAT16 => Ok(Self::Float16),
            Self::FLOAT32 => Ok(Self::Float32),
            Self::FLOAT64 => Ok(Self::Float64),
            Self::FLOAT128 => Ok(Self::Float128),
            _ => Err(CueMolError::invalid_argument(
                "unknown byte array element type",
            )),
        }
    }

    pub fn code(self) -> i32 {
        match self {
            Self::Uint8 => Self::UINT8,
            Self::Uint16 => Self::UINT16,
            Self::Uint32 => Self::UINT32,
            Self::Uint64 => Self::UINT64,
            Self::Int8 => Self::INT8,
            Self::Int16 => Self::INT16,
            Self::Int32 => Self::INT32,
            Self::Int64 => Self::INT64,
            Self::Float8 => Self::FLOAT8,
            Self::Float16 => Self::FLOAT16,
            Self::Float32 => Self::FLOAT32,
            Self::Float64 => Self::FLOAT64,
            Self::Float128 => Self::FLOAT128,
        }
    }

    pub fn size(self) -> usize {
        match self {
            Self::Uint8 | Self::Int8 | Self::Float8 => 1,
            Self::Uint16 | Self::Int16 | Self::Float16 => 2,
            Self::Uint32 | Self::Int32 | Self::Float32 => 4,
            Self::Uint64 | Self::Int64 | Self::Float64 => 8,
            Self::Float128 => 16,
        }
    }

    pub fn is_integer(self) -> bool {
        matches!(
            self,
            Self::Uint8
                | Self::Uint16
                | Self::Uint32
                | Self::Uint64
                | Self::Int8
                | Self::Int16
                | Self::Int32
                | Self::Int64
        )
    }

    pub fn is_float(self) -> bool {
        matches!(
            self,
            Self::Float8 | Self::Float16 | Self::Float32 | Self::Float64 | Self::Float128
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ByteArray {
    elem_type: ElementType,
    elem_count: usize,
    data: Vec<u8>,
}

impl Default for ByteArray {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteArray {
    pub const BOOL: i32 = type_consts::QTC_BOOL;
    pub const UINT8: i32 = ElementType::UINT8;
    pub const UINT16: i32 = type_consts::QTC_UINT16;
    pub const UINT32: i32 = type_consts::QTC_UINT32;
    pub const UINT64: i32 = type_consts::QTC_UINT64;
    pub const INT8: i32 = type_consts::QTC_INT8;
    pub const INT16: i32 = type_consts::QTC_INT16;
    pub const INT32: i32 = ElementType::INT32;
    pub const INT64: i32 = type_consts::QTC_INT64;
    pub const FLOAT8: i32 = type_consts::QTC_FLOAT8;
    pub const FLOAT16: i32 = type_consts::QTC_FLOAT16;
    pub const FLOAT32: i32 = ElementType::FLOAT32;
    pub const FLOAT64: i32 = ElementType::FLOAT64;
    pub const FLOAT128: i32 = type_consts::QTC_FLOAT128;
    pub const UTF8STR: i32 = type_consts::QTC_UTF8STR;

    pub fn new() -> Self {
        Self {
            elem_type: ElementType::Uint8,
            elem_count: 0,
            data: Vec::new(),
        }
    }

    pub fn init(&mut self, elem_type: ElementType, elem_count: usize) {
        self.elem_type = elem_type;
        self.elem_count = elem_count;
        self.data = vec![0; elem_type.size() * elem_count];
    }

    pub fn init_code(&mut self, elem_type: i32, elem_count: usize) -> Result<()> {
        self.init(ElementType::from_code(elem_type)?, elem_count);
        Ok(())
    }

    pub fn length(&self) -> usize {
        self.byte_len()
    }

    pub fn get_size(&self) -> usize {
        self.byte_len()
    }

    pub fn get_elem_count(&self) -> usize {
        self.elem_count
    }

    pub fn elem_count(&self) -> usize {
        self.get_elem_count()
    }

    pub fn elem_type(&self) -> ElementType {
        self.elem_type
    }

    pub fn elem_type_code(&self) -> i32 {
        self.elem_type.code()
    }

    pub fn r#type(&self) -> i32 {
        self.elem_type_code()
    }

    pub fn get_elem_type(&self) -> i32 {
        self.elem_type_code()
    }

    pub fn bool(&self) -> i32 {
        Self::BOOL
    }

    pub fn uint8(&self) -> i32 {
        Self::UINT8
    }

    pub fn uint16(&self) -> i32 {
        Self::UINT16
    }

    pub fn uint32(&self) -> i32 {
        Self::UINT32
    }

    pub fn uint64(&self) -> i32 {
        Self::UINT64
    }

    pub fn int8(&self) -> i32 {
        Self::INT8
    }

    pub fn int16(&self) -> i32 {
        Self::INT16
    }

    pub fn int32(&self) -> i32 {
        Self::INT32
    }

    pub fn int64(&self) -> i32 {
        Self::INT64
    }

    pub fn float8(&self) -> i32 {
        Self::FLOAT8
    }

    pub fn float16(&self) -> i32 {
        Self::FLOAT16
    }

    pub fn float32(&self) -> i32 {
        Self::FLOAT32
    }

    pub fn float64(&self) -> i32 {
        Self::FLOAT64
    }

    pub fn float128(&self) -> i32 {
        Self::FLOAT128
    }

    pub fn utf8_str(&self) -> i32 {
        Self::UTF8STR
    }

    pub fn byte_len(&self) -> usize {
        self.data.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn copy_from_u8(values: &[u8]) -> Self {
        let mut result = Self::new();
        result.init(ElementType::Uint8, values.len());
        result.data.copy_from_slice(values);
        result
    }

    pub fn copy_from_i32(values: &[i32]) -> Self {
        let mut result = Self::new();
        result.init(ElementType::Int32, values.len());
        for (chunk, value) in result.data.chunks_exact_mut(4).zip(values) {
            chunk.copy_from_slice(&value.to_ne_bytes());
        }
        result
    }

    pub fn copy_from_f32(values: &[f32]) -> Self {
        let mut result = Self::new();
        result.init(ElementType::Float32, values.len());
        for (chunk, value) in result.data.chunks_exact_mut(4).zip(values) {
            chunk.copy_from_slice(&value.to_ne_bytes());
        }
        result
    }

    pub fn copy_from_f64(values: &[f64]) -> Self {
        let mut result = Self::new();
        result.init(ElementType::Float64, values.len());
        for (chunk, value) in result.data.chunks_exact_mut(8).zip(values) {
            chunk.copy_from_slice(&value.to_ne_bytes());
        }
        result
    }

    pub fn copy_to_u8(&self) -> Result<Vec<u8>> {
        self.require_elem_type(ElementType::Uint8)?;
        Ok(self.data.clone())
    }

    pub fn copy_to_i32(&self) -> Result<Vec<i32>> {
        self.require_elem_type(ElementType::Int32)?;
        Ok(self
            .data
            .chunks_exact(4)
            .map(|chunk| i32::from_ne_bytes(chunk.try_into().unwrap()))
            .collect())
    }

    pub fn copy_to_f32(&self) -> Result<Vec<f32>> {
        self.require_elem_type(ElementType::Float32)?;
        Ok(self
            .data
            .chunks_exact(4)
            .map(|chunk| f32::from_ne_bytes(chunk.try_into().unwrap()))
            .collect())
    }

    pub fn copy_to_f64(&self) -> Result<Vec<f64>> {
        self.require_elem_type(ElementType::Float64)?;
        Ok(self
            .data
            .chunks_exact(8)
            .map(|chunk| f64::from_ne_bytes(chunk.try_into().unwrap()))
            .collect())
    }

    pub fn get_value(&self, index: usize) -> Result<u8> {
        self.require_integer_type()?;
        self.data
            .get(index)
            .copied()
            .ok_or_else(|| CueMolError::invalid_argument("byte array index out of bounds"))
    }

    pub fn set_value(&mut self, index: usize, value: u8) -> Result<()> {
        self.require_integer_type()?;
        let target = self
            .data
            .get_mut(index)
            .ok_or_else(|| CueMolError::invalid_argument("byte array index out of bounds"))?;
        *target = value;
        Ok(())
    }

    pub fn get_at(&self, index: usize) -> Result<i32> {
        self.require_integer_type()?;
        self.check_elem_index(index)?;
        let offset = index * self.elem_type.size();
        match self.elem_type {
            ElementType::Uint8 => Ok(self.data[offset] as i32),
            ElementType::Int8 => Ok(i8::from_ne_bytes([self.data[offset]]) as i32),
            ElementType::Uint16 => {
                Ok(u16::from_ne_bytes(self.data[offset..offset + 2].try_into().unwrap()) as i32)
            }
            ElementType::Int16 => {
                Ok(i16::from_ne_bytes(self.data[offset..offset + 2].try_into().unwrap()) as i32)
            }
            ElementType::Uint32 => {
                let value = u32::from_ne_bytes(self.data[offset..offset + 4].try_into().unwrap());
                i32::try_from(value)
                    .map_err(|_| CueMolError::invalid_argument("byte array value overflow"))
            }
            ElementType::Int32 => Ok(i32::from_ne_bytes(
                self.data[offset..offset + 4].try_into().unwrap(),
            )),
            ElementType::Uint64 | ElementType::Int64 => Err(CueMolError::invalid_argument(
                "unsupported integer element type",
            )),
            ElementType::Float8
            | ElementType::Float16
            | ElementType::Float32
            | ElementType::Float64
            | ElementType::Float128 => unreachable!(),
        }
    }

    pub fn set_at(&mut self, index: usize, value: i32) -> Result<()> {
        self.require_integer_type()?;
        self.check_elem_index(index)?;
        let offset = index * self.elem_type.size();
        match self.elem_type {
            ElementType::Uint8 => self.data[offset] = value as u8,
            ElementType::Int8 => self.data[offset] = (value as i8).to_ne_bytes()[0],
            ElementType::Uint16 => {
                self.data[offset..offset + 2].copy_from_slice(&(value as u16).to_ne_bytes())
            }
            ElementType::Int16 => {
                self.data[offset..offset + 2].copy_from_slice(&(value as i16).to_ne_bytes())
            }
            ElementType::Uint32 => {
                self.data[offset..offset + 4].copy_from_slice(&(value as u32).to_ne_bytes())
            }
            ElementType::Int32 => {
                self.data[offset..offset + 4].copy_from_slice(&value.to_ne_bytes())
            }
            ElementType::Uint64 | ElementType::Int64 => {
                return Err(CueMolError::invalid_argument(
                    "unsupported integer element type",
                ));
            }
            ElementType::Float8
            | ElementType::Float16
            | ElementType::Float32
            | ElementType::Float64
            | ElementType::Float128 => unreachable!(),
        }
        Ok(())
    }

    pub fn get_at_f(&self, index: usize) -> Result<f64> {
        self.require_float_type()?;
        self.check_elem_index(index)?;
        let offset = index * self.elem_type.size();
        match self.elem_type {
            ElementType::Uint8
            | ElementType::Uint16
            | ElementType::Uint32
            | ElementType::Uint64
            | ElementType::Int8
            | ElementType::Int16
            | ElementType::Int32
            | ElementType::Int64 => unreachable!(),
            ElementType::Float32 => Ok(f64::from(f32::from_ne_bytes(
                self.data[offset..offset + 4].try_into().unwrap(),
            ))),
            ElementType::Float64 => Ok(f64::from_ne_bytes(
                self.data[offset..offset + 8].try_into().unwrap(),
            )),
            ElementType::Float8 | ElementType::Float16 | ElementType::Float128 => Err(
                CueMolError::invalid_argument("unsupported floating-point element type"),
            ),
        }
    }

    pub fn set_at_f(&mut self, index: usize, value: f64) -> Result<()> {
        self.require_float_type()?;
        self.check_elem_index(index)?;
        let offset = index * self.elem_type.size();
        match self.elem_type {
            ElementType::Uint8
            | ElementType::Uint16
            | ElementType::Uint32
            | ElementType::Uint64
            | ElementType::Int8
            | ElementType::Int16
            | ElementType::Int32
            | ElementType::Int64 => unreachable!(),
            ElementType::Float32 => {
                self.data[offset..offset + 4].copy_from_slice(&(value as f32).to_ne_bytes())
            }
            ElementType::Float64 => {
                self.data[offset..offset + 8].copy_from_slice(&value.to_ne_bytes())
            }
            ElementType::Float8 | ElementType::Float16 | ElementType::Float128 => {
                return Err(CueMolError::invalid_argument(
                    "unsupported floating-point element type",
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        format!(
            "ByteArray(type={}, nelem={})",
            self.elem_type.code(),
            self.elem_count
        )
    }

    fn require_elem_type(&self, elem_type: ElementType) -> Result<()> {
        if self.elem_type == elem_type {
            Ok(())
        } else {
            Err(CueMolError::invalid_argument(format!(
                "byte array element type mismatch: expected {}, got {}",
                elem_type.code(),
                self.elem_type.code()
            )))
        }
    }

    fn require_integer_type(&self) -> Result<()> {
        if self.elem_type.is_integer() {
            Ok(())
        } else {
            Err(CueMolError::invalid_argument(format!(
                "byte array element type mismatch: expected integer, got {}",
                self.elem_type.code()
            )))
        }
    }

    fn require_float_type(&self) -> Result<()> {
        if self.elem_type.is_float() {
            Ok(())
        } else {
            Err(CueMolError::invalid_argument(format!(
                "byte array element type mismatch: expected float, got {}",
                self.elem_type.code()
            )))
        }
    }

    fn check_elem_index(&self, index: usize) -> Result<()> {
        if index < self.elem_count {
            Ok(())
        } else {
            Err(CueMolError::invalid_argument(
                "byte array index out of bounds",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_array_representative_operations() {
        let mut bytes = ByteArray::new();
        assert_eq!(bytes.length(), 0);
        bytes.init(ElementType::Uint8, 100);
        bytes.set_value(10, 77).unwrap();
        assert_eq!(bytes.get_value(10).unwrap(), 77);

        bytes.init(ElementType::Int32, 100);
        bytes.set_at(10, -123).unwrap();
        assert_eq!(bytes.get_at(10).unwrap(), -123);

        bytes.init(ElementType::Float32, 100);
        bytes.set_at_f(10, 12.34).unwrap();
        assert!((bytes.get_at_f(10).unwrap() - 12.34).abs() < 1.0e-5);

        bytes.init(ElementType::Float64, 100);
        bytes.set_at_f(10, 12.34).unwrap();
        assert!((bytes.get_at_f(10).unwrap() - 12.34).abs() < 1.0e-12);
    }

    #[test]
    fn byte_array_constants_match_cuemol2() {
        assert_eq!(ByteArray::UINT8, 1);
        assert_eq!(ByteArray::INT32, 13);
        assert_eq!(ByteArray::FLOAT32, 21);
        assert_eq!(ByteArray::FLOAT64, 22);
    }

    #[test]
    fn byte_array_initializes_every_reference_storage_type() {
        let cases = [
            (ByteArray::UINT8, 1),
            (ByteArray::UINT16, 2),
            (ByteArray::UINT32, 4),
            (ByteArray::UINT64, 8),
            (ByteArray::INT8, 1),
            (ByteArray::INT16, 2),
            (ByteArray::INT32, 4),
            (ByteArray::INT64, 8),
            (ByteArray::FLOAT8, 1),
            (ByteArray::FLOAT16, 2),
            (ByteArray::FLOAT32, 4),
            (ByteArray::FLOAT64, 8),
            (ByteArray::FLOAT128, 16),
        ];
        for (code, size) in cases {
            let mut bytes = ByteArray::new();
            bytes.init_code(code, 3).unwrap();
            assert_eq!(bytes.length(), size * 3);
            assert_eq!(bytes.elem_count(), 3);
            assert_eq!(
                bytes.to_string(),
                format!("ByteArray(type={code}, nelem=3)")
            );
        }
        let mut bytes = ByteArray::new();
        assert!(bytes.init_code(ByteArray::BOOL, 1).is_err());
        assert!(bytes.init_code(ByteArray::UTF8STR, 1).is_err());
    }

    #[test]
    fn byte_array_integer_access_matches_supported_reference_widths() {
        let mut bytes = ByteArray::new();
        for code in [
            ByteArray::UINT8,
            ByteArray::UINT16,
            ByteArray::UINT32,
            ByteArray::INT8,
            ByteArray::INT16,
            ByteArray::INT32,
        ] {
            bytes.init_code(code, 1).unwrap();
            bytes.set_at(0, 123).unwrap();
            assert_eq!(bytes.get_at(0).unwrap(), 123);
        }
        bytes.init_code(ByteArray::INT8, 1).unwrap();
        bytes.set_at(0, -12).unwrap();
        assert_eq!(bytes.get_at(0).unwrap(), -12);

        bytes.init_code(ByteArray::UINT32, 1).unwrap();
        bytes.set_at(0, -1).unwrap();
        assert!(bytes.get_at(0).is_err());
        bytes.init_code(ByteArray::INT64, 1).unwrap();
        assert!(bytes.get_at(0).is_err());
        bytes.init_code(ByteArray::FLOAT16, 1).unwrap();
        assert!(bytes.get_at_f(0).is_err());
    }

    #[test]
    fn byte_array_typed_copy_roundtrips() {
        let ints = ByteArray::copy_from_i32(&[-1, 0, 7]);
        assert_eq!(ints.copy_to_i32().unwrap(), [-1, 0, 7]);

        let floats = ByteArray::copy_from_f64(&[1.25, -3.5]);
        assert_eq!(floats.copy_to_f64().unwrap(), [1.25, -3.5]);
    }
}
