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
        CompressionEncoding::Gzip => cuemol_core::decode_gzip_bytes(&fs::read(path)?),
        CompressionEncoding::Bzip2 => decompress_with_command("bzip2", path),
        CompressionEncoding::Xz => cuemol_core::decode_xz_bytes(&fs::read(path)?),
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

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::{GzipOutStream, OutStream, StrOutStream, XzOutStream};

    fn compressed_bytes(gzip: bool, input: &[u8]) -> Vec<u8> {
        let sink = StrOutStream::new();
        let shared = sink.clone();
        let mut output: Box<dyn OutStream> = if gzip {
            Box::new(GzipOutStream::new(sink))
        } else {
            Box::new(XzOutStream::new(sink))
        };
        output.write(input, 0, input.len()).unwrap();
        output.close().unwrap();
        shared.get_data()
    }

    #[test]
    fn compressed_files_decode_without_external_processes() {
        for (extension, gzip) in [("gz", true), ("xz", false)] {
            let path = std::env::temp_dir().join(format!(
                "cuemol-in-process-decode-{}-{extension}",
                std::process::id()
            ));
            fs::write(&path, compressed_bytes(gzip, b"CueMol compressed input")).unwrap();
            assert_eq!(
                read_bytes_maybe_compressed(&path).unwrap(),
                b"CueMol compressed input"
            );
            fs::remove_file(path).unwrap();
        }
    }
}
