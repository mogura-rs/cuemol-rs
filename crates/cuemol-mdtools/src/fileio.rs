use cuemol_core::{CueMolError, Result};
use std::fs::File;
use std::io::Read;
use std::io::{BufReader, Write};
use std::path::Path;

pub(crate) fn read_text_maybe_compressed(path: impl AsRef<Path>) -> Result<String> {
    let bytes = read_bytes_maybe_compressed(path)?;
    String::from_utf8(bytes).map_err(|err| CueMolError::parse(format!("invalid UTF-8 text: {err}")))
}

pub(crate) fn read_bytes_maybe_compressed(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    let mut output = Vec::new();
    decompress_to(path, &mut output)?;
    Ok(output)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompressionEncoding {
    Raw,
    Gzip,
    Bzip2,
    Xz,
}

pub(crate) fn detect_compression(path: &Path) -> Result<CompressionEncoding> {
    let mut file = File::open(path)?;
    let mut magic = [0_u8; 6];
    let nread = file.read(&mut magic)?;
    if nread >= 2 && magic[0] == 0x1f && magic[1] == 0x8b {
        return Ok(CompressionEncoding::Gzip);
    }
    if nread >= 3 && magic[..3] == *b"BZh" {
        return Ok(CompressionEncoding::Bzip2);
    }
    if nread >= 6 && magic == [0xfd, b'7', b'z', b'X', b'Z', 0x00] {
        return Ok(CompressionEncoding::Xz);
    }
    Ok(CompressionEncoding::Raw)
}

pub(crate) fn decompress_to(path: &Path, output: &mut impl Write) -> Result<()> {
    let file = File::open(path)?;
    match detect_compression(path)? {
        CompressionEncoding::Raw => {
            std::io::copy(&mut BufReader::new(file), output)?;
        }
        CompressionEncoding::Gzip => {
            std::io::copy(&mut flate2::read::MultiGzDecoder::new(file), output)?;
        }
        CompressionEncoding::Bzip2 => {
            std::io::copy(&mut bzip2::read::MultiBzDecoder::new(file), output)?;
        }
        CompressionEncoding::Xz => {
            lzma_rs::xz_decompress(&mut BufReader::new(file), output)
                .map_err(|error| CueMolError::parse(format!("XZ decompression failed: {error}")))?;
        }
    }
    Ok(())
}
