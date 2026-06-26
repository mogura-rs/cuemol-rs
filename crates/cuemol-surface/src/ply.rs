//! Stanford Polygon (`.ply`) surface-mesh reader — a port of C++
//! `surface::PLYFileReader` (ASCII subset).
//!
//! The header declares `element vertex N` with `x`/`y`/`z` (and optional
//! `nx`/`ny`/`nz`) properties and `element face M` with a vertex-index list;
//! faces with more than three vertices are fan-triangulated. Per-vertex normals
//! are taken from the file when present, otherwise computed from the incident
//! face normals.

use crate::{SurfaceMesh, SurfaceVertex};
use cuemol_core::{CueMolError, Result, Vector4};
use std::path::Path;

/// Read an ASCII PLY mesh from a file.
pub fn read_ply_file(path: impl AsRef<Path>) -> Result<SurfaceMesh> {
    let input = std::fs::read_to_string(path)?;
    read_ply_str(&input)
}

/// Read an ASCII PLY mesh from a string.
pub fn read_ply_str(input: &str) -> Result<SurfaceMesh> {
    let mut lines = input.lines();
    if lines.next().map(str::trim) != Some("ply") {
        return Err(CueMolError::parse("not a PLY file (missing 'ply' magic)"));
    }

    let mut nverts = 0usize;
    let mut nfaces = 0usize;
    let mut vprops: Vec<String> = Vec::new();
    let mut cur_element = String::new();
    let mut format_ascii = true;
    for line in lines.by_ref() {
        let line = line.trim();
        if line == "end_header" {
            break;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        match toks.as_slice() {
            ["format", fmt, ..] => format_ascii = fmt.starts_with("ascii"),
            ["element", "vertex", n] => {
                nverts = parse_count(n)?;
                cur_element = "vertex".to_string();
            }
            ["element", "face", n] => {
                nfaces = parse_count(n)?;
                cur_element = "face".to_string();
            }
            ["element", ..] => cur_element = "other".to_string(),
            ["property", _ty, name] if cur_element == "vertex" => vprops.push(name.to_string()),
            _ => {}
        }
    }
    if !format_ascii {
        return Err(CueMolError::parse("only ASCII PLY is supported"));
    }

    let col = |name: &str| vprops.iter().position(|p| p == name);
    let cx = col("x").ok_or_else(|| CueMolError::parse("PLY vertex has no x"))?;
    let cy = col("y").ok_or_else(|| CueMolError::parse("PLY vertex has no y"))?;
    let cz = col("z").ok_or_else(|| CueMolError::parse("PLY vertex has no z"))?;
    let normal_cols = match (col("nx"), col("ny"), col("nz")) {
        (Some(a), Some(b), Some(c)) => Some((a, b, c)),
        _ => None,
    };

    // Collect positions / file-normals / triangles first, so vertex normals can
    // be computed before the mesh is built (SurfaceMesh has no normal setter).
    let mut positions: Vec<Vector4> = Vec::with_capacity(nverts);
    let mut file_normals: Vec<Vector4> = Vec::with_capacity(nverts);
    for _ in 0..nverts {
        let line = lines
            .next()
            .ok_or_else(|| CueMolError::parse("PLY truncated in vertex data"))?;
        let f: Vec<f64> = line
            .split_whitespace()
            .map(|t| t.parse::<f64>().unwrap_or(0.0))
            .collect();
        let at = |i: usize| f.get(i).copied().unwrap_or(0.0);
        positions.push(Vector4::new3(at(cx), at(cy), at(cz)));
        file_normals.push(match normal_cols {
            Some((a, b, c)) => Vector4::new3(at(a), at(b), at(c)),
            None => Vector4::ZERO,
        });
    }

    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for _ in 0..nfaces {
        let line = lines
            .next()
            .ok_or_else(|| CueMolError::parse("PLY truncated in face data"))?;
        let idx: Vec<usize> = line
            .split_whitespace()
            .filter_map(|t| t.parse::<usize>().ok())
            .collect();
        if idx.is_empty() {
            continue;
        }
        let count = idx[0].min(idx.len() - 1);
        let verts = &idx[1..=count];
        // Fan-triangulate (v0, vk, vk+1), keeping only in-range index triples.
        for k in 1..verts.len().saturating_sub(1) {
            let (a, b, c) = (verts[0], verts[k], verts[k + 1]);
            if a.max(b).max(c) < positions.len() {
                triangles.push([a as u32, b as u32, c as u32]);
            }
        }
    }

    let normals = if normal_cols.is_some() {
        file_normals
    } else {
        compute_vertex_normals(&positions, &triangles)
    };

    let mut mesh = SurfaceMesh::new();
    for (position, normal) in positions.into_iter().zip(normals) {
        mesh.push_vertex(SurfaceVertex { position, normal });
    }
    for triangle in triangles {
        mesh.push_triangle(triangle);
    }
    Ok(mesh)
}

fn parse_count(value: &str) -> Result<usize> {
    value
        .parse::<usize>()
        .map_err(|_| CueMolError::parse(format!("invalid PLY element count: {value}")))
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
    fn rejects_non_ply_and_binary() {
        assert!(read_ply_str("not a ply").is_err());
        assert!(read_ply_str("ply\nformat binary_little_endian 1.0\nend_header\n").is_err());
    }
}
