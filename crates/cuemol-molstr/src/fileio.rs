use cuemol_core::{CueMolError, Result};
#[cfg(test)]
use std::fs;
use std::path::Path;

pub fn read_text_maybe_compressed(path: impl AsRef<Path>) -> Result<String> {
    let bytes = read_bytes_maybe_compressed(path)?;
    String::from_utf8(bytes).map_err(|err| CueMolError::parse(format!("invalid UTF-8 text: {err}")))
}

pub fn read_bytes_maybe_compressed(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    cuemol_core::compressed::read(path, 0)
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
