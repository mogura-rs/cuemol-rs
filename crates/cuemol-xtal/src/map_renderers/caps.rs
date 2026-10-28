use super::{BoundaryFilter, GridRange};
use cuemol_core::Vector4;
use cuemol_qsys::ScalarObject;
use cuemol_surface::{SurfaceMesh, SurfaceVertex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapCapMode {
    #[default]
    Auto,
    On,
    Off,
}

impl MapCapMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" | "0" => Some(Self::Auto),
            "on" | "1" => Some(Self::On),
            "off" | "2" => Some(Self::Off),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }

    pub fn enabled(self, full_region: bool, generated_surface: bool) -> bool {
        match self {
            Self::Auto => full_region || generated_surface,
            Self::On => true,
            Self::Off => false,
        }
    }
}

// CueMol2 MapSurfRenderer_consts.hpp: bits mark density <= level.
// The reference deliberately leaves the two saddle cases (5 and 10) open.
const FACE_TRIANGLES: [[i8; 9]; 16] = [
    [0, 1, 3, 1, 2, 3, -1, -1, -1],
    [2, 7, 4, 2, 4, 1, 2, 3, 7],
    [3, 4, 5, 3, 5, 2, 3, 0, 4],
    [2, 3, 5, 3, 7, 5, -1, -1, -1],
    [0, 5, 6, 0, 6, 3, 0, 1, 5],
    [-1; 9],
    [3, 0, 6, 0, 4, 6, -1, -1, -1],
    [3, 7, 6, -1, -1, -1, -1, -1, -1],
    [1, 6, 7, 1, 7, 0, 1, 2, 6],
    [1, 2, 4, 2, 6, 4, -1, -1, -1],
    [-1; 9],
    [2, 6, 5, -1, -1, -1, -1, -1, -1],
    [0, 1, 7, 1, 5, 7, -1, -1, -1],
    [1, 5, 4, -1, -1, -1, -1, -1, -1],
    [0, 4, 7, -1, -1, -1, -1, -1, -1],
    [-1; 9],
];

pub(super) fn append_caps<S: ScalarObject>(
    mesh: &mut SurfaceMesh,
    map: &S,
    level: f64,
    indices: &[Vec<isize>; 3],
    range: GridRange,
    boundary: &BoundaryFilter<'_>,
) {
    let center = map.conv_to_orth(Vector4::new3(
        (indices[0][0] + indices[0][indices[0].len() - 1]) as f64 * 0.5,
        (indices[1][0] + indices[1][indices[1].len() - 1]) as f64 * 0.5,
        (indices[2][0] + indices[2][indices[2].len() - 1]) as f64 * 0.5,
    ));
    for axis in 0..3 {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        for upper in [false, true] {
            let fixed = if upper { indices[axis].len() - 1 } else { 0 };
            for j in 0..indices[v].len() - 1 {
                for i in 0..indices[u].len() - 1 {
                    let corners = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)].map(|(i, j)| {
                        let mut p = [0; 3];
                        p[axis] = indices[axis][fixed];
                        p[u] = indices[u][i];
                        p[v] = indices[v][j];
                        p
                    });
                    let points = corners.map(|p| {
                        map.conv_to_orth(Vector4::new3(p[0] as f64, p[1] as f64, p[2] as f64))
                    });
                    if !points.iter().all(|p| boundary.contains(*p)) {
                        continue;
                    }
                    let values = corners.map(|p| {
                        let [x, y, z] = range.map_indices(map, p);
                        map.at_float(x, y, z)
                    });
                    let Some(mut normal) = (points[1] - points[0])
                        .cross(points[3] - points[0])
                        .normalize3d()
                        .ok()
                    else {
                        continue;
                    };
                    if normal.dot3d(points[0] - center) < 0.0 {
                        normal = normal.scale(-1.0);
                    }
                    append_face(mesh, points, values, level, normal);
                }
            }
        }
    }
}

fn append_face(
    mesh: &mut SurfaceMesh,
    corners: [Vector4; 4],
    values: [f64; 4],
    level: f64,
    normal: Vector4,
) {
    let mut mask = 0;
    let mut points = [Vector4::ZERO; 8];
    for i in 0..4 {
        points[i] = corners[i];
        if values[i] <= level {
            mask |= 1 << i;
        }
        let j = (i + 1) % 4;
        let denominator = values[j] - values[i];
        let t = if denominator == 0.0 {
            0.5
        } else {
            (level - values[i]) / denominator
        };
        points[i + 4] = corners[i] + (corners[j] - corners[i]).scale(t);
    }
    let mut vertex_ids = [None; 8];
    for triangle in FACE_TRIANGLES[mask].chunks_exact(3) {
        if triangle[0] < 0 {
            break;
        }
        let mut corners = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        let cross = (points[corners[1]] - points[corners[0]])
            .cross(points[corners[2]] - points[corners[0]]);
        if cross.sqlen3d() == 0.0 {
            continue;
        }
        if cross.dot3d(normal) < 0.0 {
            corners.swap(1, 2);
        }
        let triangle = corners.map(|corner| {
            *vertex_ids[corner].get_or_insert_with(|| {
                let id = mesh.vertex_count() as u32;
                mesh.push_vertex(SurfaceVertex::new(points[corner], normal));
                id
            })
        });
        mesh.push_triangle(triangle);
    }
}
