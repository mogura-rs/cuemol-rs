use std::fs;
use std::path::Path;

const MOVIE_FILE_EXTENSIONS: [&str; 4] = ["mov", "mp4", "wmv", "gif"];

fn validate_base_name(base_name: &str) -> Result<&str, String> {
    let base_name = base_name.trim();
    if base_name.is_empty()
        || base_name.contains('/')
        || base_name.contains('\\')
        || matches!(base_name, "." | "..")
    {
        return Err("output base name must be a simple file name".to_string());
    }
    Ok(base_name)
}

pub(crate) fn animation_frame_count(output_dir: &Path, base_name: &str) -> Result<u32, String> {
    let base_name = validate_base_name(base_name)?;
    let mut count = 0_u32;
    loop {
        let path = output_dir.join(format!("{base_name}_frm_{count:04}.png"));
        if !path.is_file() {
            return Ok(count);
        }
        count = count
            .checked_add(1)
            .ok_or_else(|| "animation frame count exceeds the supported range".to_string())?;
    }
}

pub(crate) fn cleanup_animation_outputs(
    output_dir: &Path,
    base_name: &str,
) -> Result<usize, String> {
    let base_name = validate_base_name(base_name)?;
    let frame_prefix = format!("{base_name}_frm_");
    let movie_names = MOVIE_FILE_EXTENSIONS.map(|extension| format!("{base_name}.{extension}"));
    let entries = fs::read_dir(output_dir).map_err(|error| {
        format!(
            "failed to read animation output directory {}: {error}",
            output_dir.display()
        )
    })?;
    let mut deleted = 0;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read animation output: {error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let frame_index = name
            .strip_prefix(&frame_prefix)
            .and_then(|value| value.strip_suffix(".png"));
        let is_frame = frame_index.is_some_and(|value| {
            !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
        });
        if !is_frame && !movie_names.iter().any(|movie| movie == name) {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect animation output: {error}"))?;
        if (file_type.is_file() || file_type.is_symlink()) && fs::remove_file(entry.path()).is_ok()
        {
            deleted += 1;
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_outputs_count_contiguous_frames_and_cleanup_all_known_outputs() {
        let output_dir =
            std::env::temp_dir().join(format!("cuemol-animation-cleanup-{}", std::process::id()));
        fs::create_dir_all(&output_dir).unwrap();
        for name in [
            "movie_frm_0000.png",
            "movie_frm_0001.png",
            "movie_frm_0003.png",
            "movie_frm_12.png",
            "movie.mov",
            "movie.mp4",
            "movie.wmv",
            "movie.gif",
            "movie.pov",
            "other_frm_0000.png",
        ] {
            fs::write(output_dir.join(name), name).unwrap();
        }
        fs::create_dir(output_dir.join("movie_frm_9999.png")).unwrap();

        assert_eq!(animation_frame_count(&output_dir, "movie").unwrap(), 2);
        assert_eq!(cleanup_animation_outputs(&output_dir, "movie").unwrap(), 8);
        assert!(output_dir.join("movie.pov").is_file());
        assert!(output_dir.join("other_frm_0000.png").is_file());
        assert!(output_dir.join("movie_frm_9999.png").is_dir());

        fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn animation_outputs_reject_unsafe_base_names() {
        for base_name in ["", ".", "..", "../movie", "dir/movie", "dir\\movie"] {
            assert!(animation_frame_count(Path::new("."), base_name).is_err());
            assert!(cleanup_animation_outputs(Path::new("."), base_name).is_err());
        }
    }
}
