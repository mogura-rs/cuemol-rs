use cuemol_core::Matrix4;
use cuemol_qsys::Camera;

/// A row-major translation matrix.
fn translation(tx: f64, ty: f64, tz: f64) -> Matrix4 {
    let mut m = Matrix4 {
        values: [[0.0; 4]; 4],
    };
    for i in 0..4 {
        m.values[i][i] = 1.0;
    }
    m.values[0][3] = tx;
    m.values[1][3] = ty;
    m.values[2][3] = tz;
    m
}

/// Build the row-major projection matrix for a [`Camera`] — orthographic, or
/// perspective when the camera's `perspective` flag is set. The focal plane at
/// `cam_dist` has half-height `zoom/2` (width scaled by `aspect`) in both modes;
/// the display clip planes map to wgpu depth `[0, 1]`.
pub(crate) fn projection_matrix(camera: &Camera, aspect: f64) -> Matrix4 {
    projection_matrix_with_range(camera, aspect, camera_depth_range(camera))
}

fn projection_matrix_with_range(camera: &Camera, aspect: f64, range: (f64, f64)) -> Matrix4 {
    let cam_dist = camera.cam_dist();
    let half_h = (camera.zoom() / 2.0).max(1.0e-6);
    let half_w = (half_h * aspect).max(1.0e-6);
    let (near_dist, far_dist) = range;
    let mut p = Matrix4 {
        values: [[0.0; 4]; 4],
    };
    if camera.is_perspective() {
        let f = cam_dist / half_h; // 1 / tan(half_fov_y)
        p.values[0][0] = f / aspect;
        p.values[1][1] = f;
        p.values[2][2] = far_dist / (near_dist - far_dist);
        p.values[2][3] = near_dist * far_dist / (near_dist - far_dist);
        p.values[3][2] = -1.0; // perspective divide by -z_view
    } else {
        p.values[0][0] = 1.0 / half_w;
        p.values[1][1] = 1.0 / half_h;
        p.values[2][2] = -1.0 / (far_dist - near_dist);
        p.values[2][3] = -near_dist / (far_dist - near_dist);
        p.values[3][3] = 1.0;
    }
    p
}

/// Build the column-major MVP matrix WGSL expects from a scene [`Camera`].
///
/// Mirrors the CueMol camera model: view = `T(0,0,-cam_dist) · R(rotation) ·
/// T(-center)`, then the [`projection_matrix`] (orthographic or perspective).
pub fn camera_mvp(camera: &Camera, aspect: f64) -> [[f32; 4]; 4] {
    camera_mvp_with_projection(camera, projection_matrix(camera, aspect))
}

/// Picking stops at the fog end; fully fogged geometry remains in display depth.
pub fn camera_pick_mvp(camera: &Camera, aspect: f64) -> [[f32; 4]; 4] {
    let (near, _) = camera_depth_range(camera);
    let far = (camera.cam_dist() + camera.slab_depth() * 0.5).max(near + 0.1);
    camera_mvp_with_projection(
        camera,
        projection_matrix_with_range(camera, aspect, (near, far)),
    )
}

fn camera_mvp_with_projection(camera: &Camera, projection: Matrix4) -> [[f32; 4]; 4] {
    let center = camera.center();
    let rotation = camera.rotation().to_matrix();
    let cam_dist = camera.cam_dist();
    let view = translation(0.0, 0.0, -cam_dist)
        .mul_mat(rotation)
        .mul_mat(translation(-center.x, -center.y, -center.z));

    let mvp = projection.mul_mat(view);
    // Row-major Matrix4 -> column-major array for WGSL (out[col][row]).
    let mut out = [[0.0f32; 4]; 4];
    for (col, out_col) in out.iter_mut().enumerate() {
        for (row, slot) in out_col.iter_mut().enumerate() {
            *slot = mvp.values[row][col] as f32;
        }
    }
    out
}

/// Near and far distances used by [`camera_mvp`].
pub fn camera_depth_range(camera: &Camera) -> (f64, f64) {
    let slab = camera.slab_depth().max(0.1);
    (
        (camera.cam_dist() - slab / 2.0).max(0.1),
        camera.cam_dist() + slab,
    )
}
