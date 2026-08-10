use crate::{CueMolError, InStream, LString, OutStream, Result};
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fmt;
use std::io::{Cursor, Read, Write};

pub struct FilterInStream {
    inner: Box<dyn InStream>,
}

impl FilterInStream {
    pub fn new<S: InStream + 'static>(inner: S) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }

    pub fn inner(&self) -> &dyn InStream {
        self.inner.as_ref()
    }
}

impl fmt::Debug for FilterInStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FilterInStream")
            .field("uri", &self.inner.uri())
            .finish()
    }
}

impl InStream for FilterInStream {
    fn ready(&mut self) -> Result<bool> {
        self.inner.ready()
    }
    fn read_byte(&mut self) -> Result<i32> {
        self.inner.read_byte()
    }
    fn read(&mut self, buffer: &mut [u8], offset: usize, len: usize) -> Result<usize> {
        self.inner.read(buffer, offset, len)
    }
    fn skip(&mut self, len: usize) -> Result<usize> {
        self.inner.skip(len)
    }
    fn close(&mut self) -> Result<()> {
        self.inner.close()
    }
    fn uri(&self) -> LString {
        self.inner.uri()
    }
}

pub struct FilterOutStream {
    inner: Box<dyn OutStream>,
}

impl FilterOutStream {
    pub fn new<S: OutStream + 'static>(inner: S) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }

    pub fn inner(&self) -> &dyn OutStream {
        self.inner.as_ref()
    }
}

impl fmt::Debug for FilterOutStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FilterOutStream")
            .field("uri", &self.inner.uri())
            .finish()
    }
}

impl OutStream for FilterOutStream {
    fn write(&mut self, buffer: &[u8], offset: usize, len: usize) -> Result<usize> {
        self.inner.write(buffer, offset, len)
    }
    fn write_byte(&mut self, value: u8) -> Result<()> {
        self.inner.write_byte(value)
    }
    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
    fn close(&mut self) -> Result<()> {
        self.inner.close()
    }
    fn uri(&self) -> LString {
        self.inner.uri()
    }
}

pub struct ChunkDelimitedInStream {
    source: Option<Box<dyn InStream>>,
    data: Vec<u8>,
    position: usize,
    mark: Vec<u8>,
    chunk_end: Option<usize>,
    closed: bool,
    uri: LString,
}

impl fmt::Debug for ChunkDelimitedInStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChunkDelimitedInStream")
            .field("position", &self.position)
            .field("chunk_end", &self.chunk_end)
            .field("mark", &String::from_utf8_lossy(&self.mark))
            .finish()
    }
}

impl ChunkDelimitedInStream {
    pub fn new<S: InStream + 'static>(source: S) -> Result<Self> {
        let uri = source.uri();
        let mut source: Box<dyn InStream> = Box::new(source);
        let data = read_all(source.as_mut())?;
        Ok(Self {
            source: Some(source),
            data,
            position: 0,
            mark: Vec::new(),
            chunk_end: None,
            closed: false,
            uri,
        })
    }

    pub fn set_mark(&mut self, mark: impl AsRef<[u8]>) -> Result<bool> {
        if mark.as_ref().is_empty() {
            return Err(CueMolError::invalid_argument("chunk delimiter is empty"));
        }
        self.mark = mark.as_ref().to_vec();
        self.locate_end();
        Ok(true)
    }

    pub fn reset(&mut self) -> bool {
        let Some(end) = self.chunk_end else {
            return false;
        };
        self.position = end + self.mark.len();
        self.locate_end();
        true
    }

    pub fn read_start_marker(&mut self, buffer: &mut [u8]) -> usize {
        let count = buffer
            .len()
            .min(self.data.len().saturating_sub(self.position));
        buffer[..count].copy_from_slice(&self.data[self.position..self.position + count]);
        self.position += count;
        self.locate_end();
        count
    }

    fn locate_end(&mut self) {
        self.chunk_end = if self.mark.is_empty() {
            None
        } else {
            locate(&self.data[self.position..], &self.mark).map(|index| self.position + index)
        };
    }

    fn available_end(&self) -> usize {
        self.chunk_end.unwrap_or(self.data.len())
    }
}

impl InStream for ChunkDelimitedInStream {
    fn ready(&mut self) -> Result<bool> {
        Ok(!self.closed && self.position < self.available_end())
    }

    fn read_byte(&mut self) -> Result<i32> {
        if !self.ready()? {
            return Ok(-1);
        }
        let value = self.data[self.position];
        self.position += 1;
        Ok(i32::from(value))
    }

    fn read(&mut self, buffer: &mut [u8], offset: usize, len: usize) -> Result<usize> {
        validate_range(buffer.len(), offset, len)?;
        if self.closed {
            return Ok(0);
        }
        let count = len.min(self.available_end().saturating_sub(self.position));
        buffer[offset..offset + count]
            .copy_from_slice(&self.data[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }

    fn skip(&mut self, _len: usize) -> Result<usize> {
        Err(CueMolError::invalid_argument(
            "skip is not supported by chunk streams",
        ))
    }

    fn close(&mut self) -> Result<()> {
        self.closed = true;
        if let Some(mut source) = self.source.take() {
            source.close()?;
        }
        Ok(())
    }

    fn uri(&self) -> LString {
        self.uri.clone()
    }
}

macro_rules! compressed_input {
    ($name:ident, $decode:expr) => {
        #[derive(Debug)]
        pub struct $name {
            data: Vec<u8>,
            position: usize,
            closed: bool,
            uri: LString,
        }

        impl $name {
            pub fn new<S: InStream + 'static>(mut source: S) -> Result<Self> {
                let uri = source.uri();
                let encoded = read_all(&mut source)?;
                source.close()?;
                let data: Vec<u8> = ($decode)(&encoded)?;
                Ok(Self {
                    data,
                    position: 0,
                    closed: false,
                    uri,
                })
            }
        }

        impl InStream for $name {
            fn ready(&mut self) -> Result<bool> {
                Ok(!self.closed && self.position < self.data.len())
            }
            fn read_byte(&mut self) -> Result<i32> {
                if !self.ready()? {
                    return Ok(-1);
                }
                let value = self.data[self.position];
                self.position += 1;
                Ok(i32::from(value))
            }
            fn read(&mut self, buffer: &mut [u8], offset: usize, len: usize) -> Result<usize> {
                validate_range(buffer.len(), offset, len)?;
                if self.closed {
                    return Ok(0);
                }
                let count = len.min(self.data.len().saturating_sub(self.position));
                buffer[offset..offset + count]
                    .copy_from_slice(&self.data[self.position..self.position + count]);
                self.position += count;
                Ok(count)
            }
            fn skip(&mut self, len: usize) -> Result<usize> {
                let count = len.min(self.data.len().saturating_sub(self.position));
                self.position += count;
                Ok(count)
            }
            fn close(&mut self) -> Result<()> {
                self.closed = true;
                Ok(())
            }
            fn uri(&self) -> LString {
                self.uri.clone()
            }
        }
    };
}

compressed_input!(GzipInStream, decode_gzip_bytes);
compressed_input!(XzInStream, decode_xz_bytes);

#[derive(Debug, Clone, Copy)]
enum CompressionKind {
    Gzip,
    Xz,
}

pub struct CompressedOutStream {
    output: Option<Box<dyn OutStream>>,
    data: Vec<u8>,
    kind: CompressionKind,
    uri: LString,
    closed: bool,
}

impl fmt::Debug for CompressedOutStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompressedOutStream")
            .field("kind", &self.kind)
            .finish()
    }
}

impl CompressedOutStream {
    fn new<S: OutStream + 'static>(output: S, kind: CompressionKind) -> Self {
        let uri = output.uri();
        Self {
            output: Some(Box::new(output)),
            data: Vec::new(),
            kind,
            uri,
            closed: false,
        }
    }
}

impl OutStream for CompressedOutStream {
    fn write(&mut self, buffer: &[u8], offset: usize, len: usize) -> Result<usize> {
        validate_range(buffer.len(), offset, len)?;
        if self.closed {
            return Ok(0);
        }
        self.data.extend_from_slice(&buffer[offset..offset + len]);
        Ok(len)
    }
    fn write_byte(&mut self, value: u8) -> Result<()> {
        if self.closed {
            return Err(CueMolError::Io("output stream is closed".into()));
        }
        self.data.push(value);
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        self.output.as_mut().map_or(Ok(()), |output| output.flush())
    }
    fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        let encoded = match self.kind {
            CompressionKind::Gzip => encode_gzip(&self.data)?,
            CompressionKind::Xz => encode_xz(&self.data)?,
        };
        let mut output = self
            .output
            .take()
            .ok_or_else(|| CueMolError::Io("output stream is closed".into()))?;
        write_all(output.as_mut(), &encoded)?;
        output.flush()?;
        output.close()?;
        self.closed = true;
        Ok(())
    }
    fn uri(&self) -> LString {
        self.uri.clone()
    }
}

pub struct GzipOutStream(CompressedOutStream);
pub struct XzOutStream(CompressedOutStream);

macro_rules! compressed_output {
    ($name:ident, $kind:expr) => {
        impl $name {
            pub fn new<S: OutStream + 'static>(output: S) -> Self {
                Self(CompressedOutStream::new(output, $kind))
            }
        }
        impl OutStream for $name {
            fn write(&mut self, b: &[u8], o: usize, l: usize) -> Result<usize> {
                self.0.write(b, o, l)
            }
            fn write_byte(&mut self, v: u8) -> Result<()> {
                self.0.write_byte(v)
            }
            fn flush(&mut self) -> Result<()> {
                self.0.flush()
            }
            fn close(&mut self) -> Result<()> {
                self.0.close()
            }
            fn uri(&self) -> LString {
                self.0.uri()
            }
        }
    };
}
compressed_output!(GzipOutStream, CompressionKind::Gzip);
compressed_output!(XzOutStream, CompressionKind::Xz);

pub fn decode_gzip_bytes(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    MultiGzDecoder::new(data)
        .read_to_end(&mut output)
        .map_err(CueMolError::from)?;
    Ok(output)
}
fn encode_gzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).map_err(CueMolError::from)?;
    encoder.finish().map_err(CueMolError::from)
}
pub fn decode_xz_bytes(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    lzma_rs::xz_decompress(&mut Cursor::new(data), &mut output)
        .map_err(|error| CueMolError::parse(error.to_string()))?;
    Ok(output)
}
fn encode_xz(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    lzma_rs::xz_compress(&mut Cursor::new(data), &mut output)
        .map_err(|error| CueMolError::Io(error.to_string()))?;
    Ok(output)
}
fn read_all(input: &mut dyn InStream) -> Result<Vec<u8>> {
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
    Ok(data)
}
fn write_all(output: &mut dyn OutStream, data: &[u8]) -> Result<()> {
    let mut offset = 0;
    while offset < data.len() {
        let count = output.write(data, offset, data.len() - offset)?;
        if count == 0 {
            return Err(CueMolError::Io("output stream is closed".into()));
        }
        offset += count;
    }
    Ok(())
}
fn validate_range(size: usize, offset: usize, len: usize) -> Result<()> {
    if offset > size || len > size - offset {
        Err(CueMolError::invalid_argument("buffer range is invalid"))
    } else {
        Ok(())
    }
}
fn locate(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StrInStream, StrOutStream};

    #[test]
    fn chunk_filter_hides_delimiter_and_resets_to_next_chunk() {
        let mut stream =
            ChunkDelimitedInStream::new(StrInStream::new(b"abc<END>def<END>tail")).unwrap();
        stream.set_mark(b"<END>").unwrap();
        let mut buffer = [0_u8; 16];
        assert_eq!(stream.read(&mut buffer, 0, 16).unwrap(), 3);
        assert_eq!(&buffer[..3], b"abc");
        assert!(!stream.ready().unwrap());
        assert!(stream.reset());
        assert_eq!(stream.read(&mut buffer, 0, 16).unwrap(), 3);
        assert_eq!(&buffer[..3], b"def");
        assert!(stream.reset());
        assert_eq!(stream.read(&mut buffer, 0, 16).unwrap(), 4);
        assert_eq!(&buffer[..4], b"tail");
        assert!(!stream.reset());
    }

    #[test]
    fn gzip_and_xz_streams_roundtrip_partial_reads_and_skips() {
        for kind in [CompressionKind::Gzip, CompressionKind::Xz] {
            let sink = StrOutStream::new();
            let shared = sink.clone();
            let mut output = CompressedOutStream::new(sink, kind);
            output.write(b"0123456789", 0, 10).unwrap();
            output.close().unwrap();
            let compressed = shared.get_data();
            let mut input: Box<dyn InStream> = match kind {
                CompressionKind::Gzip => {
                    Box::new(GzipInStream::new(StrInStream::new(compressed)).unwrap())
                }
                CompressionKind::Xz => {
                    Box::new(XzInStream::new(StrInStream::new(compressed)).unwrap())
                }
            };
            assert_eq!(input.skip(3).unwrap(), 3);
            let mut data = [0_u8; 4];
            assert_eq!(input.read(&mut data, 0, 4).unwrap(), 4);
            assert_eq!(&data, b"3456");
        }
    }

    #[test]
    fn compressed_inputs_reject_corrupt_payloads() {
        assert!(GzipInStream::new(StrInStream::new(b"not gzip")).is_err());
        assert!(XzInStream::new(StrInStream::new(b"not xz")).is_err());
        assert!(decode_gzip_bytes(b"not gzip").is_err());
        assert!(decode_xz_bytes(b"not xz").is_err());
    }
}
