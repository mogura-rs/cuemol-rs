//! In-process decoding with bounded output for content sniffing.
use crate::{CueMolError, Result};
use std::io::{BufRead, BufReader, Cursor, Read, Write};
use std::path::Path;

pub fn read(path: impl AsRef<Path>, limit: usize) -> Result<Vec<u8>> {
    decode(BufReader::new(std::fs::File::open(path)?), limit)
}
pub fn decode_bytes(bytes: &[u8]) -> Result<Vec<u8>> {
    decode(Cursor::new(bytes), 0)
}
pub fn decode_prefix(bytes: &[u8], limit: usize) -> Result<Vec<u8>> {
    decode(Cursor::new(bytes), limit)
}

/// Stream decoded bytes to a writer, stopping at `limit` when nonzero.
pub fn copy_to(path: impl AsRef<Path>, output: &mut impl Write, limit: u64) -> Result<u64> {
    copy_decoded(BufReader::new(std::fs::File::open(path)?), output, limit)
}

fn copy_decoded(mut input: impl BufRead, output: &mut impl Write, limit: u64) -> Result<u64> {
    let header = input.fill_buf()?;
    let limit = if limit == 0 { u64::MAX } else { limit };
    if header.starts_with(&[0x1f, 0x8b]) {
        return Ok(std::io::copy(
            &mut flate2::read::MultiGzDecoder::new(input).take(limit),
            output,
        )?);
    }
    if header.starts_with(b"BZh") {
        return Ok(std::io::copy(
            &mut bzip2::read::MultiBzDecoder::new(input).take(limit),
            output,
        )?);
    }
    if header.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0]) {
        let mut writer = BoundedWriter {
            output,
            limit,
            written: 0,
            reached: false,
        };
        let result = lzma_rs::xz_decompress(&mut input, &mut writer);
        if !writer.reached {
            result.map_err(|e| CueMolError::parse(format!("XZ decompression failed: {e}")))?;
        }
        return Ok(writer.written);
    }
    Ok(std::io::copy(&mut input.take(limit), output)?)
}

struct BoundedWriter<'a, W> {
    output: &'a mut W,
    limit: u64,
    written: u64,
    reached: bool,
}
impl<W: Write> Write for BoundedWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let n = (bytes.len() as u64).min(self.limit - self.written) as usize;
        self.output.write_all(&bytes[..n])?;
        self.written += n as u64;
        if n < bytes.len() {
            self.reached = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "prefix complete",
            ));
        }
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.output.flush()
    }
}
fn collect(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(if limit == 0 { u64::MAX } else { limit as u64 })
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}
fn decode(mut input: impl BufRead, limit: usize) -> Result<Vec<u8>> {
    let header = input.fill_buf()?;
    if header.starts_with(&[0x1f, 0x8b]) {
        return collect(flate2::read::MultiGzDecoder::new(input), limit);
    }
    if header.starts_with(b"BZh") {
        return collect(bzip2::read::MultiBzDecoder::new(input), limit);
    }
    if header.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0]) {
        let mut output = LimitedOutput {
            bytes: Vec::new(),
            limit,
            reached: false,
        };
        let result = lzma_rs::xz_decompress(&mut input, &mut output);
        if !output.reached {
            result.map_err(|e| CueMolError::parse(format!("XZ decompression failed: {e}")))?;
        }
        return Ok(output.bytes);
    }
    collect(input, limit)
}
struct LimitedOutput {
    bytes: Vec<u8>,
    limit: usize,
    reached: bool,
}
impl Write for LimitedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.limit > 0 && bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.bytes
                .extend_from_slice(&bytes[..self.limit - self.bytes.len()]);
            self.reached = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "prefix complete",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_encodings_support_full_decoding_and_bounded_sniffing() {
        let input = b"molecular input\n".repeat(10000);
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gzip.write_all(&input).unwrap();
        let mut bzip = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        bzip.write_all(&input).unwrap();
        let mut xz = Vec::new();
        lzma_rs::xz_compress(&mut Cursor::new(&input), &mut xz).unwrap();
        for encoded in [
            input.clone(),
            gzip.finish().unwrap(),
            bzip.finish().unwrap(),
            xz,
        ] {
            assert_eq!(decode_bytes(&encoded).unwrap(), input);
            assert_eq!(decode(Cursor::new(&encoded), 17).unwrap(), input[..17]);
            let mut streamed = Vec::new();
            assert_eq!(
                copy_decoded(Cursor::new(&encoded), &mut streamed, 0).unwrap(),
                input.len() as u64
            );
            assert_eq!(streamed, input);
            streamed.clear();
            assert_eq!(
                copy_decoded(Cursor::new(&encoded), &mut streamed, 17).unwrap(),
                17
            );
            assert_eq!(streamed, input[..17]);
        }
    }
}
