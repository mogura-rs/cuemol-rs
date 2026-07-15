use crate::{CueMolError, LString, Result};
use std::collections::VecDeque;

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_LINE_INPUT: usize = 60;

/// In-memory counterpart of CueMol's `Base64OutStream` filter.
///
/// The original filter emits one 80-character line for every 60 input bytes.
/// Closing the stream writes the final padded line and its trailing newline.
#[derive(Debug, Clone, Default)]
pub struct Base64OutStream {
    pending: Vec<u8>,
    encoded: Vec<u8>,
    closed: bool,
}

impl Base64OutStream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_byte(&mut self, value: u8) {
        if self.closed {
            return;
        }
        self.pending.push(value);
        if self.pending.len() >= BASE64_LINE_INPUT {
            self.write_pending_line();
        }
    }

    pub fn write(&mut self, value: impl AsRef<[u8]>, off: usize, len: usize) -> usize {
        if self.closed {
            return 0;
        }
        let value = value.as_ref();
        if off >= value.len() {
            return 0;
        }
        let len = len.min(value.len() - off);
        for &byte in &value[off..off + len] {
            self.write_byte(byte);
        }
        len
    }

    /// Kept as a no-op for parity with `Base64OutFilterImpl::flush`.
    pub fn flush(&mut self) {}

    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.write_pending_line();
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.encoded
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.close();
        self.encoded
    }

    fn write_pending_line(&mut self) {
        for chunk in self.pending.chunks(3) {
            let a = chunk[0];
            let b = chunk.get(1).copied().unwrap_or(0);
            let c = chunk.get(2).copied().unwrap_or(0);
            self.encoded.push(BASE64_ALPHABET[(a >> 2) as usize]);
            self.encoded
                .push(BASE64_ALPHABET[(((a & 0x03) << 4) | (b >> 4)) as usize]);
            self.encoded.push(if chunk.len() >= 2 {
                BASE64_ALPHABET[(((b & 0x0f) << 2) | (c >> 6)) as usize]
            } else {
                b'='
            });
            self.encoded.push(if chunk.len() >= 3 {
                BASE64_ALPHABET[(c & 0x3f) as usize]
            } else {
                b'='
            });
        }
        self.pending.clear();
        self.encoded.push(b'\n');
    }
}

/// In-memory counterpart of CueMol's lenient `Base64InStream` filter.
/// Non-base64 bytes are ignored, as in the original C++ implementation.
#[derive(Debug, Clone)]
pub struct Base64InStream {
    decoded: Vec<u8>,
    offset: usize,
}

impl Base64InStream {
    pub fn new(input: impl AsRef<[u8]>) -> Self {
        Self {
            decoded: decode_base64_lenient(input.as_ref()),
            offset: 0,
        }
    }

    pub fn ready(&self) -> bool {
        self.offset < self.decoded.len()
    }

    pub fn read_byte(&mut self) -> i32 {
        if !self.ready() {
            return -1;
        }
        let value = self.decoded[self.offset];
        self.offset += 1;
        i32::from(value)
    }

    pub fn read(&mut self, buf: &mut [u8], off: usize, len: usize) -> isize {
        if !self.ready() || off >= buf.len() {
            return 0;
        }
        let len = len
            .min(buf.len() - off)
            .min(self.decoded.len() - self.offset);
        buf[off..off + len].copy_from_slice(&self.decoded[self.offset..self.offset + len]);
        self.offset += len;
        len as isize
    }

    /// The reference filter does not support skipping and always returns zero.
    pub fn skip(&mut self, _len: usize) -> usize {
        0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.decoded
    }
}

fn decode_base64_lenient(input: &[u8]) -> Vec<u8> {
    let filtered = input
        .iter()
        .copied()
        .filter(|&byte| byte == b'=' || base64_value(byte).is_some())
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(filtered.len() / 4 * 3);
    let mut offset = 0;
    while filtered.len().saturating_sub(offset) >= 2 {
        let a = match base64_value(filtered[offset]) {
            Some(value) => value,
            None => break,
        };
        let b = match base64_value(filtered[offset + 1]) {
            Some(value) => value,
            None => break,
        };
        output.push((a << 2) | (b >> 4));

        let remaining = filtered.len() - offset;
        if remaining == 2 || filtered[offset + 2] == b'=' {
            break;
        }
        let c = match base64_value(filtered[offset + 2]) {
            Some(value) => value,
            None => break,
        };
        output.push((b << 4) | (c >> 2));

        if remaining == 3 || filtered[offset + 3] == b'=' {
            break;
        }
        let d = match base64_value(filtered[offset + 3]) {
            Some(value) => value,
            None => break,
        };
        output.push((c << 6) | d);
        offset += 4;
    }
    output
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[derive(Debug, Clone, Default)]
pub struct PipeStreamImpl {
    bytes: VecDeque<u8>,
    closed: bool,
}

impl PipeStreamImpl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_byte(&mut self, value: u8) {
        self.bytes.push_back(value);
    }

    pub fn write(&mut self, value: impl AsRef<[u8]>, off: usize, len: usize) -> usize {
        let value = value.as_ref();
        if off >= value.len() {
            return 0;
        }
        let len = len.min(value.len() - off);
        self.bytes.extend(value[off..off + len].iter().copied());
        len
    }

    pub fn close_write(&mut self) {
        self.closed = true;
    }

    pub fn read_byte(&mut self) -> i32 {
        self.bytes.pop_front().map(i32::from).unwrap_or(-1)
    }

    pub fn read(&mut self, buf: &mut [u8], off: usize, len: usize) -> isize {
        if self.bytes.is_empty() {
            return -1;
        }
        if off >= buf.len() {
            return 0;
        }
        let len = len.min(buf.len() - off).min(self.bytes.len());
        for slot in &mut buf[off..off + len] {
            *slot = self.bytes.pop_front().unwrap();
        }
        len as isize
    }

    pub fn ready(&self) -> bool {
        !self.bytes.is_empty() || !self.closed
    }

    pub fn skip(&mut self, len: usize) -> usize {
        let count = len.min(self.bytes.len());
        self.bytes.drain(..count);
        count
    }
}

#[derive(Debug, Clone, Default)]
pub struct BinOutStream {
    bytes: Vec<u8>,
}

impl BinOutStream {
    pub const INTBO_LE: i32 = 0;
    pub const INTBO_BE: i32 = 1;
    pub const FLTBO_LE: i32 = 0;
    pub const FLTBO_BE: i32 = 1;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn write_int8(&mut self, value: i8) {
        self.bytes.push(value as u8);
    }

    pub fn write_int16(&mut self, value: i16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_int32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_float32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_str(&mut self, value: impl AsRef<str>) {
        let value = value.as_ref();
        self.write_int32(value.len() as i32);
        self.bytes.extend_from_slice(value.as_bytes());
    }

    pub fn write_fixed_str(&mut self, value: impl AsRef<str>, max_len: usize) {
        let bytes = value.as_ref().as_bytes();
        let write_len = bytes.len().min(max_len);
        self.bytes.extend_from_slice(&bytes[..write_len]);
        self.bytes.resize(self.bytes.len() + max_len - write_len, 0);
    }

    pub fn get_int_byte_order() -> i32 {
        Self::INTBO_LE
    }

    pub fn get_float_byte_order() -> i32 {
        Self::FLTBO_LE
    }
}

#[derive(Debug, Clone)]
pub struct BinInStream {
    bytes: Vec<u8>,
    offset: usize,
    swap_mode: i32,
}

impl BinInStream {
    pub const MODE_NORM_LE: i32 = 0;
    pub const MODE_NORM_BE: i32 = 1;
    pub const MODE_SWAP: i32 = 2;
    pub const MODE_NOOP: i32 = 3;

    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            offset: 0,
            swap_mode: Self::MODE_NOOP,
        }
    }

    pub fn get_swap_mode(&self) -> i32 {
        self.swap_mode
    }

    pub fn set_swap_mode(&mut self, mode: i32) {
        self.swap_mode = mode;
    }

    pub fn read_int8(&mut self) -> Result<i8> {
        Ok(self.read_exact::<1>()?[0] as i8)
    }

    pub fn read_int16(&mut self) -> Result<i16> {
        let mut bytes = self.read_exact::<2>()?;
        self.apply_swap(&mut bytes);
        Ok(i16::from_le_bytes(bytes))
    }

    pub fn read_int32(&mut self) -> Result<i32> {
        let mut bytes = self.read_exact::<4>()?;
        self.apply_swap(&mut bytes);
        Ok(i32::from_le_bytes(bytes))
    }

    pub fn read_float32(&mut self) -> Result<f32> {
        let mut bytes = self.read_exact::<4>()?;
        self.apply_swap(&mut bytes);
        Ok(f32::from_le_bytes(bytes))
    }

    pub fn read_str(&mut self) -> Result<LString> {
        let len = self.read_int32()?;
        if len <= 0 {
            return Ok(LString::new());
        }
        let bytes = self.read_bytes(len as usize)?;
        String::from_utf8(bytes)
            .map(LString::from)
            .map_err(|_| CueMolError::invalid_argument("invalid utf-8 string"))
    }

    pub fn assert_i32(&mut self, expected: i32) -> Result<bool> {
        Ok(self.read_int32()? == expected)
    }

    fn read_exact<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.read_bytes(N)?;
        bytes
            .try_into()
            .map_err(|_| CueMolError::invalid_argument("unexpected end of stream"))
    }

    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        if self.offset + len > self.bytes.len() {
            return Err(CueMolError::invalid_argument("unexpected end of stream"));
        }
        let bytes = self.bytes[self.offset..self.offset + len].to_vec();
        self.offset += len;
        Ok(bytes)
    }

    fn apply_swap<const N: usize>(&self, bytes: &mut [u8; N]) {
        if self.swap_mode == Self::MODE_SWAP {
            bytes.reverse();
        }
    }
}

#[derive(Debug, Clone)]
pub struct LimitedInStream {
    bytes: Vec<u8>,
    offset: usize,
    remaining: usize,
}

impl LimitedInStream {
    pub fn new(bytes: impl AsRef<[u8]>, max_bytes: usize) -> Self {
        Self {
            bytes: bytes.as_ref().to_vec(),
            offset: 0,
            remaining: max_bytes,
        }
    }

    pub fn ready(&self) -> bool {
        self.remaining > 0 && self.offset < self.bytes.len()
    }

    pub fn read_byte(&mut self) -> i32 {
        if !self.ready() {
            return -1;
        }
        let value = self.bytes[self.offset];
        self.offset += 1;
        self.remaining -= 1;
        i32::from(value)
    }

    pub fn read(&mut self, buf: &mut [u8], off: usize, len: usize) -> isize {
        if self.remaining == 0 || self.offset >= self.bytes.len() || off >= buf.len() {
            return -1;
        }
        let requested = len
            .min(self.remaining)
            .min(self.bytes.len() - self.offset)
            .min(buf.len() - off);
        if requested == 0 {
            return -1;
        }
        buf[off..off + requested]
            .copy_from_slice(&self.bytes[self.offset..self.offset + requested]);
        self.offset += requested;
        self.remaining -= requested;
        requested as isize
    }

    pub fn skip(&mut self, len: usize) -> usize {
        let skipped = len.min(self.remaining).min(self.bytes.len() - self.offset);
        self.offset += skipped;
        self.remaining -= skipped;
        skipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_streams_match_reference_filter_behavior() {
        let mut output = Base64OutStream::new();
        assert_eq!(output.write(b"Man", 0, 3), 3);
        assert_eq!(output.finish(), b"TWFu\n");

        let mut input = Base64InStream::new(b" T W\nFu!\r\n");
        let mut decoded = [0; 8];
        assert_eq!(input.read(&mut decoded, 2, 6), 3);
        assert_eq!(&decoded[2..5], b"Man");
        assert_eq!(input.read_byte(), -1);
        assert_eq!(input.skip(10), 0);
    }

    #[test]
    fn base64_stream_handles_padding_and_unpadded_tail() {
        assert_eq!(Base64InStream::new(b"TQ==").into_bytes(), b"M");
        assert_eq!(Base64InStream::new(b"TWE=").into_bytes(), b"Ma");
        assert_eq!(Base64InStream::new(b"TQ").into_bytes(), b"M");
        assert_eq!(Base64InStream::new(b"TWE").into_bytes(), b"Ma");
    }

    #[test]
    fn base64_output_wraps_at_reference_line_width() {
        let source = (0..61).collect::<Vec<u8>>();
        let mut output = Base64OutStream::new();
        assert_eq!(output.write(&source, 0, source.len()), source.len());
        let encoded = output.finish();
        let lines = encoded.split(|&byte| byte == b'\n').collect::<Vec<_>>();
        assert_eq!(lines[0].len(), 80);
        assert_eq!(lines[1].len(), 4);
        assert_eq!(Base64InStream::new(encoded).into_bytes(), source);
    }

    #[test]
    fn pipe_stream_matches_reference_memory_pipe_cases() {
        let mut pipe = PipeStreamImpl::new();
        pipe.write_byte(42);
        assert!(pipe.ready());
        pipe.close_write();
        assert_eq!(pipe.read_byte(), 42);
        assert_eq!(pipe.read_byte(), -1);

        let mut pipe = PipeStreamImpl::new();
        pipe.write("hello", 0, 5);
        pipe.close_write();
        let mut dst = [0; 10];
        assert_eq!(pipe.read(&mut dst, 0, 10), 5);
        assert_eq!(&dst[..5], b"hello");

        let mut pipe = PipeStreamImpl::new();
        assert!(pipe.ready());
        pipe.close_write();
        assert!(!pipe.ready());
        assert_eq!(pipe.read(&mut dst, 0, 4), -1);

        let mut pipe = PipeStreamImpl::new();
        pipe.write("hello", 0, 5);
        pipe.close_write();
        assert_eq!(pipe.skip(3), 3);
        assert_eq!(pipe.skip(10), 2);
    }
}
