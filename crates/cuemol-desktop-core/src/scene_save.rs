use cuemol_core::{CueMolError, Result};
use std::path::{Path, PathBuf};

pub(crate) fn write_scene_files(
    path: &Path,
    document: &[u8],
    styles: &[(PathBuf, String)],
) -> Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let files = styles
            .iter()
            .map(|(path, xml)| (path.as_path(), xml.as_bytes()))
            .chain(std::iter::once((path, document)));
        native::commit(native::prepare(files)?)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (path, document, styles);
        Err(CueMolError::invalid_argument(
            "Use a browser download to save scenes",
        ))
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::{self, Write};
    use tempfile::{NamedTempFile, TempPath};

    pub(super) struct PreparedWrite {
        target: PathBuf,
        data: Option<TempPath>,
        backup: Option<TempPath>,
    }

    fn destination(path: &Path) -> io::Result<PathBuf> {
        let mut path = path.to_path_buf();
        for _ in 0..40 {
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    let link = fs::read_link(&path)?;
                    path = if link.is_absolute() {
                        link
                    } else {
                        path.parent().unwrap_or(Path::new(".")).join(link)
                    };
                }
                Ok(metadata) if metadata.is_file() => return fs::canonicalize(path),
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "save destination is not a regular file",
                    ))
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let parent = path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or(Path::new("."));
                    let name = path.file_name().ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "save destination has no filename",
                        )
                    })?;
                    return Ok(fs::canonicalize(parent)?.join(name));
                }
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many save destination symlinks",
        ))
    }

    pub(super) fn prepare<'a>(
        files: impl IntoIterator<Item = (&'a Path, &'a [u8])>,
    ) -> Result<Vec<PreparedWrite>> {
        let mut destinations = BTreeMap::new();
        let mut unique = Vec::new();
        for (path, bytes) in files {
            let target = destination(path)?;
            if let Some(previous) = destinations.insert(target.clone(), bytes) {
                if previous != bytes {
                    return Err(CueMolError::invalid_argument(format!(
                        "Conflicting save contents for {}",
                        path.display()
                    )));
                }
                continue;
            }
            unique.push((target, bytes));
        }
        let total = unique.len();
        let mut prepared = Vec::with_capacity(total);
        for (index, (target, bytes)) in unique.into_iter().enumerate() {
            let parent = target.parent().expect("resolved destination has a parent");
            let mut data = NamedTempFile::new_in(parent)?;
            data.write_all(bytes)?;
            data.as_file().sync_all()?;
            let backup = match fs::metadata(&target) {
                Ok(metadata) => {
                    fs::OpenOptions::new().write(true).open(&target)?;
                    data.as_file().set_permissions(metadata.permissions())?;
                    // The final rename cannot leave a later replacement to roll back.
                    if index + 1 == total {
                        None
                    } else {
                        let backup = NamedTempFile::new_in(parent)?.into_temp_path();
                        fs::remove_file(&backup)?;
                        if fs::hard_link(&target, &backup).is_err() {
                            let mut output = fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(&backup)?;
                            io::copy(&mut fs::File::open(&target)?, &mut output)?;
                            output.set_permissions(metadata.permissions())?;
                            output.sync_all()?;
                        }
                        Some(backup)
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };
            prepared.push(PreparedWrite {
                target,
                data: Some(data.into_temp_path()),
                backup,
            });
        }
        Ok(prepared)
    }

    pub(super) fn commit(mut files: Vec<PreparedWrite>) -> Result<()> {
        for index in 0..files.len() {
            let data = files[index]
                .data
                .take()
                .expect("each staged file is committed once");
            if let Err(error) = data.persist(&files[index].target) {
                let mut message = format!(
                    "Cannot save {}: {}",
                    files[index].target.display(),
                    error.error
                );
                for previous in files[..index].iter_mut().rev() {
                    let result = if let Some(backup) = &previous.backup {
                        fs::rename(backup, &previous.target)
                    } else {
                        fs::remove_file(&previous.target)
                    };
                    if let Err(rollback) = result {
                        message.push_str(&format!(
                            "; cannot restore {}: {rollback}",
                            previous.target.display()
                        ));
                        if let Some(backup) = previous.backup.take() {
                            let path = backup.to_path_buf();
                            match backup.keep() {
                                Ok(_) => message.push_str(&format!(
                                    "; original retained at {}",
                                    path.display()
                                )),
                                Err(error) => {
                                    message.push_str(&format!("; cannot retain backup: {error}"))
                                }
                            }
                        }
                    }
                }
                return Err(CueMolError::Io(message));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn failed_commit_restores_already_replaced_files() {
            let directory = tempfile::tempdir().unwrap();
            let first = directory.path().join("first");
            let second = directory.path().join("second");
            fs::write(&first, "original").unwrap();
            fs::write(&second, "second").unwrap();
            let files = prepare([
                (first.as_path(), b"changed".as_slice()),
                (second.as_path(), b"changed".as_slice()),
            ])
            .unwrap();
            fs::remove_file(&second).unwrap();
            fs::create_dir(&second).unwrap();
            assert!(commit(files).is_err());
            assert_eq!(fs::read_to_string(&first).unwrap(), "original");
            assert!(second.is_dir());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
        }
    }
}
