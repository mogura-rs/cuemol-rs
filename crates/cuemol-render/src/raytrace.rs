use std::time::Instant;

use cuemol_core::Vector4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UmbreonSmokeResult {
    pub width: i32,
    pub height: i32,
    pub render_seconds: f64,
    pub non_background_pixels: usize,
}

pub fn render_umbreon_smoke() -> UmbreonSmokeResult {
    const WIDTH: i32 = 64;
    const HEIGHT: i32 = 64;
    const CAMERA_HEIGHT: f64 = 2.5;
    let corners = [
        Vector4::new3(-1.0, -1.0, 0.0),
        Vector4::new3(1.0, -1.0, 0.0),
        Vector4::new3(1.0, 1.0, 0.0),
        Vector4::new3(-1.0, -1.0, 0.0),
        Vector4::new3(1.0, 1.0, 0.0),
        Vector4::new3(-1.0, 1.0, 0.0),
    ];
    let started = Instant::now();
    let mut non_background_pixels = 0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let world_x = ((f64::from(x) + 0.5) / f64::from(WIDTH) - 0.5) * CAMERA_HEIGHT;
            let world_y = (0.5 - (f64::from(y) + 0.5) / f64::from(HEIGHT)) * CAMERA_HEIGHT;
            let origin = Vector4::new3(world_x, world_y, 5.0);
            let direction = Vector4::new3(0.0, 0.0, -1.0);
            if corners
                .chunks_exact(3)
                .any(|triangle| ray_triangle_hit(origin, direction, triangle))
            {
                non_background_pixels += 1;
            }
        }
    }

    UmbreonSmokeResult {
        width: WIDTH,
        height: HEIGHT,
        render_seconds: started.elapsed().as_secs_f64(),
        non_background_pixels,
    }
}

fn ray_triangle_hit(origin: Vector4, direction: Vector4, triangle: &[Vector4]) -> bool {
    let edge1 = triangle[1] - triangle[0];
    let edge2 = triangle[2] - triangle[0];
    let p = cross(direction, edge2);
    let determinant = edge1.dot3d(p);
    if determinant.abs() < 1.0e-12 {
        return false;
    }
    let inverse = determinant.recip();
    let translated = origin - triangle[0];
    let u = translated.dot3d(p) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = cross(translated, edge1);
    let v = direction.dot3d(q) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    edge2.dot3d(q) * inverse >= 0.0
}

fn cross(left: Vector4, right: Vector4) -> Vector4 {
    Vector4::new3(
        left.y * right.z - left.z * right.y,
        left.z * right.x - left.x * right.z,
        left.x * right.y - left.y * right.x,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn umbreon_smoke_renders_a_non_empty_frame() {
        let result = render_umbreon_smoke();
        assert_eq!(result.width, 64);
        assert_eq!(result.height, 64);
        assert!(result.render_seconds >= 0.0);
        assert!(result.non_background_pixels > 1_000);
    }

    #[test]
    fn triangle_intersection_rejects_misses_and_backward_rays() {
        let triangle = [
            Vector4::new3(-1.0, -1.0, 0.0),
            Vector4::new3(1.0, -1.0, 0.0),
            Vector4::new3(0.0, 1.0, 0.0),
        ];
        assert!(ray_triangle_hit(
            Vector4::new3(0.0, 0.0, 1.0),
            Vector4::new3(0.0, 0.0, -1.0),
            &triangle
        ));
        assert!(!ray_triangle_hit(
            Vector4::new3(2.0, 0.0, 1.0),
            Vector4::new3(0.0, 0.0, -1.0),
            &triangle
        ));
        assert!(!ray_triangle_hit(
            Vector4::new3(0.0, 0.0, 1.0),
            Vector4::new3(0.0, 0.0, 1.0),
            &triangle
        ));
    }
}
