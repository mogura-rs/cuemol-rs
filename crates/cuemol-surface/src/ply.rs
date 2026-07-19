//! Stanford Polygon (`.ply`) surface-mesh reader — a port of C++
//! `surface::PLYFileReader`.
//!
//! The header declares `element vertex N` with `x`/`y`/`z` (and optional
//! `nx`/`ny`/`nz`) properties and `element face M` with a vertex-index list;
//! faces with more than three vertices are fan-triangulated. Per-vertex normals
//! are taken from the file when present, otherwise computed from the incident
//! face normals.

use crate::{SurfaceMesh, SurfaceVertex};
use cuemol_core::{CueMolError, Result, Vector4};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlyFormat {
    Ascii,
    BinaryLittle,
    BinaryBig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlyType {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlyProperty {
    name: String,
    scalar_type: PlyType,
    count_type: Option<PlyType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlyElement {
    name: String,
    count: usize,
    properties: Vec<PlyProperty>,
}

#[derive(Debug, Clone, PartialEq)]
enum PlyValue {
    Scalar(f64),
    List(Vec<i64>),
}

/// Read an ASCII or binary PLY mesh from a file.
pub fn read_ply_file(path: impl AsRef<Path>) -> Result<SurfaceMesh> {
    let input = std::fs::read(path)?;
    read_ply_bytes(&input)
}

/// Read an ASCII PLY mesh from a string.
pub fn read_ply_str(input: &str) -> Result<SurfaceMesh> {
    read_ply_bytes(input.as_bytes())
}

/// Read an ASCII or binary PLY mesh from bytes.
pub fn read_ply_bytes(input: &[u8]) -> Result<SurfaceMesh> {
    let (format, elements, body_offset) = parse_header(input)?;
    let vertex_index = elements
        .iter()
        .position(|element| element.name == "vertex")
        .ok_or_else(|| CueMolError::parse("PLY has no vertex element"))?;
    let vertex_count = elements[vertex_index].count;
    if vertex_count == 0 {
        return Err(CueMolError::parse("PLY has no vertices"));
    }
    for coord in ["x", "y", "z"] {
        if !elements[vertex_index]
            .properties
            .iter()
            .any(|property| property.name == coord && property.count_type.is_none())
        {
            return Err(CueMolError::parse(format!(
                "PLY vertex has no {coord} property"
            )));
        }
    }
    let face_index = elements.iter().position(|element| element.name == "face");
    let has_normals = ["nx", "ny", "nz"].iter().all(|name| {
        elements[vertex_index]
            .properties
            .iter()
            .any(|property| property.name == *name && property.count_type.is_none())
    });

    let mut positions = Vec::with_capacity(vertex_count);
    let mut file_normals = Vec::with_capacity(vertex_count);
    let mut triangles = Vec::new();
    match format {
        PlyFormat::Ascii => {
            let body = std::str::from_utf8(&input[body_offset..])
                .map_err(|_| CueMolError::parse("PLY ASCII body is not UTF-8"))?;
            let mut lines = body.lines().filter(|line| !line.trim().is_empty());
            for (element_index, element) in elements.iter().enumerate() {
                for _ in 0..element.count {
                    let line = lines
                        .next()
                        .ok_or_else(|| CueMolError::parse("PLY body is truncated"))?;
                    let values = parse_ascii_record(element, line)?;
                    capture_record(
                        element_index,
                        vertex_index,
                        face_index,
                        element,
                        &values,
                        vertex_count,
                        &mut positions,
                        &mut file_normals,
                        &mut triangles,
                    )?;
                }
            }
        }
        PlyFormat::BinaryLittle | PlyFormat::BinaryBig => {
            let little_endian = format == PlyFormat::BinaryLittle;
            let mut offset = body_offset;
            for (element_index, element) in elements.iter().enumerate() {
                for _ in 0..element.count {
                    let values = parse_binary_record(input, &mut offset, element, little_endian)?;
                    capture_record(
                        element_index,
                        vertex_index,
                        face_index,
                        element,
                        &values,
                        vertex_count,
                        &mut positions,
                        &mut file_normals,
                        &mut triangles,
                    )?;
                }
            }
        }
    }
    if positions.len() != vertex_count {
        return Err(CueMolError::parse("PLY vertex data is incomplete"));
    }
    if triangles.is_empty() {
        return Err(CueMolError::parse("PLY has no faces"));
    }

    let normals = if has_normals {
        file_normals
    } else {
        compute_vertex_normals(&positions, &triangles)
    };

    let mut mesh = SurfaceMesh::new();
    for (position, normal) in positions.into_iter().zip(normals) {
        mesh.push_vertex(SurfaceVertex::new(position, normal));
    }
    for triangle in triangles {
        mesh.push_triangle(triangle);
    }
    Ok(mesh)
}

fn parse_header(input: &[u8]) -> Result<(PlyFormat, Vec<PlyElement>, usize)> {
    let mut offset = 0usize;
    let magic = next_header_line(input, &mut offset)?;
    if magic.trim() != "ply" {
        return Err(CueMolError::parse("not a PLY file (missing 'ply' magic)"));
    }
    let mut format = None;
    let mut elements: Vec<PlyElement> = Vec::new();
    loop {
        let line = next_header_line(input, &mut offset)?;
        let tokens: Vec<&str> = line.split_whitespace().collect();
        match tokens.as_slice() {
            [] | ["comment", ..] | ["obj_info", ..] => {}
            ["format", name, ..] => {
                format = Some(match *name {
                    "ascii" => PlyFormat::Ascii,
                    "binary_little_endian" => PlyFormat::BinaryLittle,
                    "binary_big_endian" => PlyFormat::BinaryBig,
                    _ => {
                        return Err(CueMolError::parse(format!(
                            "unsupported PLY format: {name}"
                        )))
                    }
                });
            }
            ["element", name, count] => elements.push(PlyElement {
                name: (*name).to_string(),
                count: count
                    .parse::<usize>()
                    .map_err(|_| CueMolError::parse("invalid PLY element count"))?,
                properties: Vec::new(),
            }),
            ["property", "list", count_type, scalar_type, name] => {
                let element = elements
                    .last_mut()
                    .ok_or_else(|| CueMolError::parse("PLY property without element"))?;
                element.properties.push(PlyProperty {
                    name: (*name).to_string(),
                    scalar_type: parse_type(scalar_type)?,
                    count_type: Some(parse_type(count_type)?),
                });
            }
            ["property", scalar_type, name] => {
                let element = elements
                    .last_mut()
                    .ok_or_else(|| CueMolError::parse("PLY property without element"))?;
                element.properties.push(PlyProperty {
                    name: (*name).to_string(),
                    scalar_type: parse_type(scalar_type)?,
                    count_type: None,
                });
            }
            ["end_header"] => break,
            _ => {}
        }
    }
    let format = format.ok_or_else(|| CueMolError::parse("PLY header has no format"))?;
    Ok((format, elements, offset))
}

fn next_header_line<'a>(input: &'a [u8], offset: &mut usize) -> Result<&'a str> {
    if *offset >= input.len() {
        return Err(CueMolError::parse("PLY header is truncated"));
    }
    let start = *offset;
    let relative_end = input[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or_else(|| CueMolError::parse("PLY header line is unterminated"))?;
    *offset = start + relative_end + 1;
    let mut end = start + relative_end;
    if end > start && input[end - 1] == b'\r' {
        end -= 1;
    }
    std::str::from_utf8(&input[start..end])
        .map_err(|_| CueMolError::parse("PLY header is not ASCII"))
}

fn parse_type(value: &str) -> Result<PlyType> {
    match value {
        "char" | "int8" => Ok(PlyType::I8),
        "uchar" | "uint8" => Ok(PlyType::U8),
        "short" | "int16" => Ok(PlyType::I16),
        "ushort" | "uint16" => Ok(PlyType::U16),
        "int" | "int32" => Ok(PlyType::I32),
        "uint" | "uint32" => Ok(PlyType::U32),
        "float" | "float32" => Ok(PlyType::F32),
        "double" | "float64" => Ok(PlyType::F64),
        _ => Err(CueMolError::parse(format!(
            "unsupported PLY property type: {value}"
        ))),
    }
}

fn parse_ascii_record(element: &PlyElement, line: &str) -> Result<Vec<PlyValue>> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut cursor = 0usize;
    let mut values = Vec::with_capacity(element.properties.len());
    for property in &element.properties {
        if property.count_type.is_some() {
            let count = tokens
                .get(cursor)
                .ok_or_else(|| CueMolError::parse("short PLY list property"))?
                .parse::<usize>()
                .map_err(|_| CueMolError::parse("invalid PLY list count"))?;
            cursor += 1;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                let value = tokens
                    .get(cursor)
                    .ok_or_else(|| CueMolError::parse("short PLY list property"))?
                    .parse::<i64>()
                    .map_err(|_| CueMolError::parse("invalid PLY list value"))?;
                cursor += 1;
                list.push(value);
            }
            values.push(PlyValue::List(list));
        } else {
            let value = tokens
                .get(cursor)
                .ok_or_else(|| CueMolError::parse("short PLY scalar property"))?
                .parse::<f64>()
                .map_err(|_| CueMolError::parse("invalid PLY scalar value"))?;
            cursor += 1;
            values.push(PlyValue::Scalar(value));
        }
    }
    Ok(values)
}

fn parse_binary_record(
    input: &[u8],
    offset: &mut usize,
    element: &PlyElement,
    little_endian: bool,
) -> Result<Vec<PlyValue>> {
    let mut values = Vec::with_capacity(element.properties.len());
    for property in &element.properties {
        if let Some(count_type) = property.count_type {
            let count = read_binary_scalar(input, offset, count_type, little_endian)? as i64;
            let count = usize::try_from(count)
                .map_err(|_| CueMolError::parse("negative PLY list count"))?;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                list.push(
                    read_binary_scalar(input, offset, property.scalar_type, little_endian)? as i64,
                );
            }
            values.push(PlyValue::List(list));
        } else {
            values.push(PlyValue::Scalar(read_binary_scalar(
                input,
                offset,
                property.scalar_type,
                little_endian,
            )?));
        }
    }
    Ok(values)
}

#[allow(clippy::too_many_arguments)]
fn capture_record(
    element_index: usize,
    vertex_index: usize,
    face_index: Option<usize>,
    element: &PlyElement,
    values: &[PlyValue],
    vertex_count: usize,
    positions: &mut Vec<Vector4>,
    normals: &mut Vec<Vector4>,
    triangles: &mut Vec<[u32; 3]>,
) -> Result<()> {
    if element_index == vertex_index {
        let mut position = Vector4::ZERO;
        let mut normal = Vector4::ZERO;
        for (property, value) in element.properties.iter().zip(values) {
            let PlyValue::Scalar(value) = value else {
                continue;
            };
            match property.name.as_str() {
                "x" => position.x = *value,
                "y" => position.y = *value,
                "z" => position.z = *value,
                "nx" => normal.x = *value,
                "ny" => normal.y = *value,
                "nz" => normal.z = *value,
                _ => {}
            }
        }
        positions.push(position);
        normals.push(normal);
    } else if Some(element_index) == face_index {
        let preferred = element
            .properties
            .iter()
            .position(|property| {
                property.count_type.is_some()
                    && matches!(property.name.as_str(), "vertex_indices" | "vertex_index")
            })
            .or_else(|| {
                element
                    .properties
                    .iter()
                    .position(|property| property.count_type.is_some())
            });
        if let Some(index) = preferred {
            if let PlyValue::List(indices) = &values[index] {
                triangulate(indices, vertex_count, triangles)?;
            }
        }
    }
    Ok(())
}

fn triangulate(indices: &[i64], vertex_count: usize, triangles: &mut Vec<[u32; 3]>) -> Result<()> {
    for index in indices {
        if *index < 0 || *index as usize >= vertex_count {
            return Err(CueMolError::parse("PLY face vertex index is out of range"));
        }
    }
    for index in 1..indices.len().saturating_sub(1) {
        triangles.push([
            indices[0] as u32,
            indices[index] as u32,
            indices[index + 1] as u32,
        ]);
    }
    Ok(())
}

fn read_binary_scalar(
    input: &[u8],
    offset: &mut usize,
    scalar_type: PlyType,
    little_endian: bool,
) -> Result<f64> {
    let size = match scalar_type {
        PlyType::I8 | PlyType::U8 => 1,
        PlyType::I16 | PlyType::U16 => 2,
        PlyType::I32 | PlyType::U32 | PlyType::F32 => 4,
        PlyType::F64 => 8,
    };
    let bytes = input
        .get(*offset..*offset + size)
        .ok_or_else(|| CueMolError::parse("PLY binary body is truncated"))?;
    *offset += size;
    Ok(match scalar_type {
        PlyType::I8 => i8::from_ne_bytes([bytes[0]]) as f64,
        PlyType::U8 => bytes[0] as f64,
        PlyType::I16 => endian_i16(bytes, little_endian) as f64,
        PlyType::U16 => endian_u16(bytes, little_endian) as f64,
        PlyType::I32 => endian_i32(bytes, little_endian) as f64,
        PlyType::U32 => endian_u32(bytes, little_endian) as f64,
        PlyType::F32 => f32::from_bits(endian_u32(bytes, little_endian)) as f64,
        PlyType::F64 => f64::from_bits(endian_u64(bytes, little_endian)),
    })
}

fn endian_u16(bytes: &[u8], little: bool) -> u16 {
    let bytes = [bytes[0], bytes[1]];
    if little {
        u16::from_le_bytes(bytes)
    } else {
        u16::from_be_bytes(bytes)
    }
}

fn endian_i16(bytes: &[u8], little: bool) -> i16 {
    endian_u16(bytes, little) as i16
}

fn endian_u32(bytes: &[u8], little: bool) -> u32 {
    let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
    if little {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    }
}

fn endian_i32(bytes: &[u8], little: bool) -> i32 {
    endian_u32(bytes, little) as i32
}

fn endian_u64(bytes: &[u8], little: bool) -> u64 {
    let bytes = [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ];
    if little {
        u64::from_le_bytes(bytes)
    } else {
        u64::from_be_bytes(bytes)
    }
}

/// Area-weighted vertex normals from incident triangle face normals (used when
/// the PLY supplies no per-vertex normals).
fn compute_vertex_normals(positions: &[Vector4], triangles: &[[u32; 3]]) -> Vec<Vector4> {
    let mut acc = vec![Vector4::ZERO; positions.len()];
    for &[a, b, c] in triangles {
        let (a, b, c) = (a as usize, b as usize, c as usize);
        // Cross of two edges = twice the area-weighted face normal.
        let n = (positions[b] - positions[a]).cross(positions[c] - positions[a]);
        acc[a] = acc[a] + n;
        acc[b] = acc[b] + n;
        acc[c] = acc[c] + n;
    }
    acc.into_iter()
        .map(|n| n.normalize3d().unwrap_or(Vector4::new3(0.0, 0.0, 1.0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ascii_ply_with_normals_and_quad_face() {
        // Hand-authored PLY (the independent oracle): 4 vertices + 1 quad face,
        // with explicit per-vertex normals. The quad fan-triangulates to 2 tris.
        let ply = "\
ply
format ascii 1.0
comment hand-authored test fixture
element vertex 4
property float x
property float y
property float z
property float nx
property float ny
property float nz
element face 1
property list uchar int vertex_indices
end_header
0 0 0 0 0 1
1 0 0 0 0 1
1 1 0 0 0 1
0 1 0 0 0 1
4 0 1 2 3
";
        let mesh = read_ply_str(ply).unwrap();
        assert_eq!(mesh.vertex_count(), 4);
        assert_eq!(mesh.triangle_count(), 2, "quad -> 2 triangles");
        assert_eq!(mesh.triangles()[0], [0, 1, 2]);
        assert_eq!(mesh.triangles()[1], [0, 2, 3]);
        // Explicit +z normals are read verbatim.
        assert_eq!(mesh.vertices()[2].position, Vector4::new3(1.0, 1.0, 0.0));
        assert!((mesh.vertices()[0].normal.z - 1.0).abs() < 1.0e-9);
    }

    #[test]
    fn computes_normals_when_absent() {
        // No normal properties -> normals computed from the (z=0) face: ±z.
        let ply = "\
ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
element face 1
property list uchar int vertex_indices
end_header
0 0 0
1 0 0
0 1 0
3 0 1 2
";
        let mesh = read_ply_str(ply).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert!(mesh.vertices().iter().all(|v| v.normal.z.abs() > 0.99));
    }

    #[test]
    fn reads_little_and_big_endian_binary() {
        fn fixture(little: bool) -> Vec<u8> {
            let format = if little {
                "binary_little_endian"
            } else {
                "binary_big_endian"
            };
            let mut bytes = format!(
                "ply\nformat {format} 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n"
            )
            .into_bytes();
            for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
                bytes.extend(if little {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                });
            }
            bytes.push(3);
            for value in [0_i32, 1, 2] {
                bytes.extend(if little {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                });
            }
            bytes
        }

        for little in [true, false] {
            let mesh = read_ply_bytes(&fixture(little)).unwrap();
            assert_eq!(mesh.vertex_count(), 3);
            assert_eq!(mesh.triangles(), &[[0, 1, 2]]);
            assert!(mesh.vertices().iter().all(|vertex| vertex.normal.z > 0.99));
        }
    }

    #[test]
    fn rejects_non_ply_and_truncated_binary() {
        assert!(read_ply_str("not a ply").is_err());
        assert!(read_ply_bytes(
            b"ply\nformat binary_little_endian 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n"
        )
        .is_err());
    }
}
