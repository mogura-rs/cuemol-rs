//! Wavefront OBJ (`.obj`) surface-mesh reader — a port of C++
//! `surface::ObjReader` (the mesh-import side of the OBJ round trip whose write
//! side is [`SurfaceMesh::to_obj_string`](crate::SurfaceMesh::to_obj_string)).
//!
//! Parses `v x y z` positions, `vn x y z` normals and `f` faces. Each face
//! vertex reference is `p`, `p/t`, `p//n` or `p/t/n`; only the position index
//! `p` drives the triangle topology. Indices are 1-based, or negative to count
//! back from the vertices seen so far. Faces with more than three vertices are
//! fan-triangulated. Per-vertex normals are taken from the `vn` list when it is
//! index-aligned with the vertices (the shape this crate writes), otherwise they
//! are computed from the incident face normals.

use crate::{SurfaceMesh, SurfaceVertex};
use cuemol_core::{CueMolError, Result, Vector4};
use std::path::Path;

/// Read a Wavefront OBJ mesh from a file.
pub fn read_obj_file(path: impl AsRef<Path>) -> Result<SurfaceMesh> {
    let input = std::fs::read_to_string(path)?;
    read_obj_str(&input)
}

/// Read a Wavefront OBJ mesh from a string.
pub fn read_obj_str(input: &str) -> Result<SurfaceMesh> {
    let mut positions: Vec<Vector4> = Vec::new();
    let mut file_normals: Vec<Vector4> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();

    // Single pass so a negative (relative) face index resolves against the number
    // of vertices seen *up to that line*, per the OBJ spec.
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut toks = line.split_whitespace();
        match toks.next() {
            Some("v") => positions.push(parse_vec3(toks)?),
            Some("vn") => file_normals.push(parse_vec3(toks)?),
            Some("f") => {
                let refs: Vec<usize> = toks
                    .filter_map(|token| resolve_index(token, positions.len()))
                    .collect();
                // Fan-triangulate (v0, vk, vk+1); a degenerate face (<3 refs) is skipped.
                for k in 1..refs.len().saturating_sub(1) {
                    triangles.push([refs[0] as u32, refs[k] as u32, refs[k + 1] as u32]);
                }
            }
            // v-texture, groups, smoothing, material lines etc. carry no geometry.
            _ => {}
        }
    }

    if positions.is_empty() {
        return Err(CueMolError::parse("OBJ file has no vertices"));
    }
    if triangles.is_empty() {
        return Err(CueMolError::parse("OBJ file has no triangular faces"));
    }

    // Use the file's normals only when there is exactly one per vertex (how this
    // crate writes them); otherwise derive them from the faces.
    let normals = if file_normals.len() == positions.len() {
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

/// Parse the three floats after a `v` / `vn` tag; missing components default to 0.
fn parse_vec3<'a>(mut toks: impl Iterator<Item = &'a str>) -> Result<Vector4> {
    let mut next = || -> f64 {
        toks.next().and_then(|value| value.parse::<f64>().ok()).unwrap_or(0.0)
    };
    Ok(Vector4::new3(next(), next(), next()))
}

/// Resolve one face vertex reference (`p`, `p/t`, `p//n`, `p/t/n`) to a 0-based
/// position index, given the number of vertices seen so far. Returns `None` for
/// an out-of-range or unparseable reference so the caller can drop it.
fn resolve_index(token: &str, count: usize) -> Option<usize> {
    let raw: i64 = token.split('/').next()?.parse().ok()?;
    let index = if raw < 0 {
        count as i64 + raw // -1 == the most recent vertex
    } else {
        raw - 1 // OBJ indices are 1-based
    };
    (index >= 0 && (index as usize) < count).then_some(index as usize)
}

/// Area-weighted vertex normals from incident triangle face normals (used when
/// the OBJ supplies no usable per-vertex normals).
fn compute_vertex_normals(positions: &[Vector4], triangles: &[[u32; 3]]) -> Vec<Vector4> {
    let mut acc = vec![Vector4::ZERO; positions.len()];
    for &[a, b, c] in triangles {
        let (a, b, c) = (a as usize, b as usize, c as usize);
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
    use crate::SurfaceMesh;

    #[test]
    fn reads_obj_triangle_with_normals() {
        // Hand-authored OBJ (the independent oracle): 3 vertices + 1 triangle,
        // faces in the `p//n` form this crate writes.
        let obj = "\
# test
o tri
v 0 0 0
v 1 0 0
v 0 1 0
vn 0 0 1
vn 0 0 1
vn 0 0 1
f 1//1 2//2 3//3
";
        let mesh = read_obj_str(obj).unwrap();
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.triangles()[0], [0, 1, 2]);
        assert_eq!(mesh.vertices()[1].position, Vector4::new3(1.0, 0.0, 0.0));
        assert!((mesh.vertices()[0].normal.z - 1.0).abs() < 1.0e-9);
    }

    #[test]
    fn fan_triangulates_a_quad_and_reads_plain_and_relative_indices() {
        // A quad face (4 refs) fans to 2 triangles; `f` uses plain indices, and a
        // second triangle uses negative (relative) indices (-4..-1 back-reference).
        let obj = "\
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
f 1 2 3 4
v 2 0 0
v 3 0 0
v 2 1 0
f -3 -2 -1
";
        let mesh = read_obj_str(obj).unwrap();
        assert_eq!(mesh.vertex_count(), 7);
        // quad -> 2 tris, plus the relative-index tri = 3.
        assert_eq!(mesh.triangle_count(), 3);
        assert_eq!(mesh.triangles()[0], [0, 1, 2]);
        assert_eq!(mesh.triangles()[1], [0, 2, 3]);
        // -3,-2,-1 against the 7 vertices seen so far -> 4,5,6.
        assert_eq!(mesh.triangles()[2], [4, 5, 6]);
        // No usable per-vertex normals (vn count 0 != 7) -> computed, all ±z.
        assert!(mesh.vertices().iter().all(|vertex| vertex.normal.z.abs() > 0.99));
    }

    #[test]
    fn rejects_empty_or_geometryless_input() {
        assert!(read_obj_str("").is_err());
        assert!(read_obj_str("# just a comment\no thing\n").is_err());
        assert!(read_obj_str("v 0 0 0\nv 1 0 0\n").is_err(), "no faces");
    }

    #[test]
    fn round_trips_the_surface_writer() {
        // The decisive oracle: a mesh written by this crate's OBJ writer must read
        // back identical (positions, normals index-aligned, triangle topology).
        // Independent because the writer and reader share no code.
        let mut mesh = SurfaceMesh::new();
        for (position, normal) in [
            (Vector4::new3(0.0, 0.0, 0.0), Vector4::new3(0.0, 0.0, 1.0)),
            (Vector4::new3(1.5, 0.0, 0.0), Vector4::new3(1.0, 0.0, 0.0)),
            (Vector4::new3(0.0, 2.5, 0.0), Vector4::new3(0.0, 1.0, 0.0)),
            (Vector4::new3(0.0, 0.0, 3.5), Vector4::new3(0.0, 0.0, -1.0)),
        ] {
            mesh.push_vertex(SurfaceVertex { position, normal });
        }
        mesh.push_triangle([0, 1, 2]);
        mesh.push_triangle([0, 2, 3]);

        let text = mesh.to_obj_string("round trip");
        let read = read_obj_str(&text).unwrap();

        assert_eq!(read.vertex_count(), mesh.vertex_count());
        assert_eq!(read.triangle_count(), mesh.triangle_count());
        assert_eq!(read.triangles(), mesh.triangles());
        for (before, after) in mesh.vertices().iter().zip(read.vertices()) {
            // The writer prints 6 decimals, so compare with a matching tolerance.
            assert!((before.position - after.position).length() < 1.0e-5);
            assert!((before.normal - after.normal).length() < 1.0e-5);
        }
    }
}
