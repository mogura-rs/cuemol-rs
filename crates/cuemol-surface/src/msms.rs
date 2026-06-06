use crate::{SurfaceMesh, SurfaceVertex};
use cuemol_core::{CueMolError, Result, Vector4};
use cuemol_molstr::ContentVerdict;
use std::fs;
use std::path::Path;

pub struct MsmsReader;

impl MsmsReader {
    pub fn read_files(
        face_path: impl AsRef<Path>,
        vert_path: impl AsRef<Path>,
    ) -> Result<SurfaceMesh> {
        read_msms_surface_files(face_path, vert_path)
    }

    pub fn read_str(vert_input: &str, face_input: &str) -> Result<SurfaceMesh> {
        read_msms_surface_str(vert_input, face_input)
    }
}

pub fn read_msms_surface_files(
    face_path: impl AsRef<Path>,
    vert_path: impl AsRef<Path>,
) -> Result<SurfaceMesh> {
    let vert_input = fs::read_to_string(vert_path)?;
    let face_input = fs::read_to_string(face_path)?;
    read_msms_surface_str(&vert_input, &face_input)
}

pub fn read_msms_surface_str(vert_input: &str, face_input: &str) -> Result<SurfaceMesh> {
    let mut mesh = SurfaceMesh::new();
    read_vertices(vert_input, &mut mesh)?;
    read_faces(face_input, &mut mesh)?;
    Ok(mesh)
}

pub fn sniff_msms_content(input: &str) -> ContentVerdict {
    if input.is_empty() {
        return ContentVerdict::Unknown;
    }
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("# MSMS") {
            return ContentVerdict::Yes;
        }
        if !line.starts_with('#') {
            return ContentVerdict::Unknown;
        }
    }
    ContentVerdict::Unknown
}

fn read_vertices(input: &str, mesh: &mut SurfaceMesh) -> Result<()> {
    let mut lines = input.lines();
    skip_header(&mut lines, "MSMS vertex")?;
    let info = lines
        .next()
        .ok_or_else(|| CueMolError::parse("missing MSMS vertex info line"))?;
    let expected = parse_count(info, 7, "invalid MSMS vertex count")?;

    for index in 0..expected {
        let line = lines
            .next()
            .ok_or_else(|| CueMolError::parse("MSMS vertex file is too short"))?;
        let fields = parse_vertex_fields(line)
            .ok_or_else(|| CueMolError::parse("invalid MSMS vertex line"))?;
        mesh.push_vertex(SurfaceVertex {
            position: Vector4::new3(fields[0], fields[1], fields[2]),
            normal: Vector4::new3(fields[3], fields[4], fields[5]),
        });
        if mesh.vertex_count() != index + 1 {
            return Err(CueMolError::parse("failed to append MSMS vertex"));
        }
    }

    if lines.any(|line| !line.trim().is_empty()) {
        return Err(CueMolError::parse(
            "MSMS vertex file contains too many lines",
        ));
    }
    Ok(())
}

fn read_faces(input: &str, mesh: &mut SurfaceMesh) -> Result<()> {
    let mut lines = input.lines();
    skip_header(&mut lines, "MSMS face")?;
    let info = lines
        .next()
        .ok_or_else(|| CueMolError::parse("missing MSMS face info line"))?;
    let expected = parse_count(info, 8, "invalid MSMS face count")?;

    for _ in 0..expected {
        let line = lines
            .next()
            .ok_or_else(|| CueMolError::parse("MSMS face file is too short"))?;
        let ids =
            parse_face_fields(line).ok_or_else(|| CueMolError::parse("invalid MSMS face line"))?;
        let triangle = [
            checked_face_index(ids[0], mesh.vertex_count())?,
            checked_face_index(ids[1], mesh.vertex_count())?,
            checked_face_index(ids[2], mesh.vertex_count())?,
        ];
        mesh.push_triangle(triangle);
    }

    if lines.any(|line| !line.trim().is_empty()) {
        return Err(CueMolError::parse("MSMS face file contains too many lines"));
    }
    Ok(())
}

fn skip_header<'a>(lines: &mut impl Iterator<Item = &'a str>, label: &'static str) -> Result<()> {
    lines
        .next()
        .ok_or_else(|| CueMolError::parse(format!("missing {label} header line")))?;
    lines
        .next()
        .ok_or_else(|| CueMolError::parse(format!("missing {label} comment line")))?;
    Ok(())
}

fn parse_count(line: &str, width: usize, message: &'static str) -> Result<usize> {
    let field = fixed_field(line, 0, width).unwrap_or(line).trim();
    let count = field
        .split_whitespace()
        .next()
        .unwrap_or(field)
        .parse::<isize>()
        .map_err(|_| CueMolError::parse(message))?;
    if count < 0 {
        return Err(CueMolError::parse(message));
    }
    Ok(count as usize)
}

fn parse_vertex_fields(line: &str) -> Option<[f64; 6]> {
    parse_fixed_f64_array::<6>(
        line,
        &[(0, 9), (9, 19), (19, 29), (29, 39), (39, 49), (49, 59)],
    )
    .or_else(|| parse_whitespace_f64_array(line))
}

fn parse_face_fields(line: &str) -> Option<[u32; 3]> {
    parse_fixed_u32_array::<3>(line, &[(0, 6), (6, 13), (13, 20)])
        .or_else(|| parse_whitespace_u32_array(line))
}

fn checked_face_index(value: u32, vertex_count: usize) -> Result<u32> {
    if value == 0 {
        return Err(CueMolError::parse("MSMS face index is one-based"));
    }
    let index = value - 1;
    if index as usize >= vertex_count {
        return Err(CueMolError::parse("MSMS face index exceeds vertex count"));
    }
    Ok(index)
}

fn parse_fixed_f64_array<const N: usize>(
    line: &str,
    ranges: &[(usize, usize); N],
) -> Option<[f64; N]> {
    let values = ranges
        .iter()
        .map(|(start, end)| fixed_field(line, *start, *end)?.trim().parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    values.try_into().ok()
}

fn parse_fixed_u32_array<const N: usize>(
    line: &str,
    ranges: &[(usize, usize); N],
) -> Option<[u32; N]> {
    let values = ranges
        .iter()
        .map(|(start, end)| fixed_field(line, *start, *end)?.trim().parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;
    values.try_into().ok()
}

fn parse_whitespace_f64_array<const N: usize>(line: &str) -> Option<[f64; N]> {
    let values = line
        .split_whitespace()
        .take(N)
        .map(|value| value.parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    (values.len() == N)
        .then(|| values.try_into().ok())
        .flatten()
}

fn parse_whitespace_u32_array<const N: usize>(line: &str) -> Option<[u32; N]> {
    let values = line
        .split_whitespace()
        .take(N)
        .map(|value| value.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;
    (values.len() == N)
        .then(|| values.try_into().ok())
        .flatten()
}

fn fixed_field(line: &str, start: usize, end: usize) -> Option<&str> {
    let bytes = line.as_bytes();
    if bytes.len() < end {
        return None;
    }
    std::str::from_utf8(&bytes[start..end]).ok()
}
