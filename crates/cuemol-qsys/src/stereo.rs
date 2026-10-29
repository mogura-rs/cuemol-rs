use crate::{Camera, StereoMode};
use cuemol_core::{Quat, Vector4};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StereoEye {
    Mono,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StereoViewport {
    pub camera: Camera,
    pub rect: [f64; 4],
    pub eye: StereoEye,
}

/// Preserve a view's eye order in an export that carries only a camera.
pub fn with_eye_order(camera: &Camera, swap_eyes: bool) -> Camera {
    let mut camera = camera.clone();
    if swap_eyes {
        camera.set_stereo_mode(match camera.stereo_mode() {
            StereoMode::Parallel => StereoMode::Cross,
            StereoMode::Cross => StereoMode::Parallel,
            mode => mode,
        });
    }
    camera
}

impl StereoViewport {
    pub fn aspect(&self) -> f64 {
        self.rect[2] / self.rect[3]
    }

    pub fn local_point(&self, point: [f64; 2]) -> Option<[f64; 2]> {
        let [x, y, width, height] = self.rect;
        (point.iter().all(|value| value.is_finite())
            && point[0] >= x
            && point[0] < x + width
            && point[1] >= y
            && point[1] < y + height)
            .then_some([point[0] - x, point[1] - y])
    }
}

/// Stereo separation is an angle in degrees about the camera's vertical axis.
/// Each eye rotates around the view center, matching GUIView::setUpModelMat.
pub fn viewports(camera: &Camera, size: [f64; 2], swap_eyes: bool) -> Vec<StereoViewport> {
    let [width, height] = size.map(|value| if value.is_finite() { value.max(1.) } else { 1. });
    let split = width >= 2.
        && matches!(
            camera.stereo_mode(),
            StereoMode::Parallel | StereoMode::Cross
        );
    let swapped = swap_eyes ^ (camera.stereo_mode() == StereoMode::Cross);
    (0..if split { 2 } else { 1 })
        .map(|index| {
            let eye = if !split {
                StereoEye::Mono
            } else if (index == 0) ^ swapped {
                StereoEye::Left
            } else {
                StereoEye::Right
            };
            let angle = match eye {
                StereoEye::Mono => 0.,
                StereoEye::Left => camera.stereo_distance() * 0.5,
                StereoEye::Right => -camera.stereo_distance() * 0.5,
            };
            let rotation = Quat::from_axis_angle(
                Vector4::new3(0., 1., 0.),
                if angle.is_finite() {
                    angle.to_radians()
                } else {
                    0.
                },
            )
            .unwrap_or(Quat::IDENTITY);
            let mut adjusted = camera.clone();
            adjusted.set_rotation(camera.rotation().mul_quat(rotation));
            adjusted.set_stereo_mode(StereoMode::None);
            let left_width = (width * 0.5).floor();
            let eye_width = if !split {
                width
            } else if index == 0 {
                left_width
            } else {
                width - left_width
            };
            StereoViewport {
                camera: adjusted,
                rect: [index as f64 * left_width, 0., eye_width, height],
                eye,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eyes_follow_reference_rotation_order_and_center() {
        let mut camera = Camera::new();
        camera.set_center(Vector4::new3(2., 3., 4.));
        camera.set_rotation(Quat::from_axis_angle(Vector4::new3(1., 0., 0.), 0.7).unwrap());
        camera.set_stereo_mode(StereoMode::Parallel);
        camera.set_stereo_distance(12.);
        let eyes = viewports(&camera, [800., 600.], false);
        assert_eq!(eyes[0].rect, [0., 0., 400., 600.]);
        assert_eq!(eyes[1].rect, [400., 0., 400., 600.]);
        for (eye, angle) in eyes.iter().zip([6_f64, -6.]) {
            let expected = Quat::from_axis_angle(Vector4::new3(0., 1., 0.), angle.to_radians())
                .unwrap()
                .to_matrix()
                .mul_mat(camera.rotation().to_matrix());
            let actual = eye.camera.rotation().to_matrix();
            for row in 0..4 {
                for col in 0..4 {
                    assert!((actual.values[row][col] - expected.values[row][col]).abs() < 1.0e-12);
                }
            }
            assert_eq!(eye.camera.center(), camera.center());
            assert_eq!(eye.camera.zoom(), camera.zoom());
            assert_eq!(eye.camera.stereo_mode(), StereoMode::None);
        }
        assert_eq!(eyes[0].local_point([399., 300.]), Some([399., 300.]));
        assert_eq!(eyes[0].local_point([400., 300.]), None);
        assert_eq!(eyes[1].local_point([400., 300.]), Some([0., 300.]));
        assert_eq!(eyes[1].local_point([f64::NAN, 300.]), None);
    }

    #[test]
    fn crossing_and_swapping_reverse_eye_order_without_changing_framing() {
        let mut camera = Camera::new();
        camera.set_stereo_mode(StereoMode::Parallel);
        let parallel = viewports(&camera, [801., 601.], false);
        let swapped = viewports(&camera, [801., 601.], true);
        camera.set_stereo_mode(StereoMode::Cross);
        assert_eq!(viewports(&camera, [801., 601.], false), swapped);
        assert_eq!(viewports(&camera, [801., 601.], true), parallel);
        assert_eq!(parallel[0].camera, swapped[1].camera);
        assert_eq!(parallel[1].camera, swapped[0].camera);
        assert_eq!(parallel[1].rect[0] + parallel[1].rect[2], 801.);
        camera.set_stereo_mode(StereoMode::Hardware);
        assert_eq!(viewports(&camera, [801., 601.], false).len(), 1);
    }
}
