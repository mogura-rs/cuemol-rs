use cuemol_core::{CueMolError, Result};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;

pub fn read_text_maybe_compressed(path: impl AsRef<Path>) -> Result<String> {
    let bytes = read_bytes_maybe_compressed(path)?;
    String::from_utf8(bytes).map_err(|err| CueMolError::parse(format!("invalid UTF-8 text: {err}")))
}

pub fn read_bytes_maybe_compressed(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    match detect_compression(path)? {
        CompressionEncoding::Raw => Ok(fs::read(path)?),
        CompressionEncoding::Gzip => decompress_with_command("gzip", path),
        CompressionEncoding::Bzip2 => decompress_with_command("bzip2", path),
        CompressionEncoding::Xz => decompress_with_command("xz", path),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressionEncoding {
    Raw,
    Gzip,
    Bzip2,
    Xz,
}

fn detect_compression(path: &Path) -> Result<CompressionEncoding> {
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

fn decompress_with_command(program: &str, path: &Path) -> Result<Vec<u8>> {
    let output = Command::new(program).arg("-cd").arg(path).output()?;
    if !output.status.success() {
        return Err(CueMolError::invalid_argument(format!(
            "failed to decompress {} with {program}",
            path.display()
        )));
    }
    Ok(output.stdout)
}
