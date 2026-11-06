use cuemol_core::{CueMolError, Result, Uid, Vector4};
use cuemol_qsys::{Camera, CenterMark, View};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewTransform {
    Rotate([f64; 3]),
    Translate([f64; 3]),
    Center([f64; 3]),
    Zoom(f64),
    Slab(f64),
    Distance(f64),
    Perspective(bool),
    CenterMark(CenterMark),
}

impl ViewTransform {
    pub fn apply(self, camera: &mut Camera, viewport: [u32; 2]) -> Result<()> {
        let finite = match self {
            Self::Rotate(values) | Self::Translate(values) | Self::Center(values) => {
                values.into_iter().all(f64::is_finite)
            }
            Self::Zoom(value) | Self::Slab(value) | Self::Distance(value) => value.is_finite(),
            _ => true,
        };
        if !finite {
            return Err(CueMolError::InvalidArgument(
                "view transform must be finite".into(),
            ));
        }
        let mut view = View::new(Uid::new(0), Uid::new(0));
        view.set_camera(camera);
        view.set_view_size(viewport[0].max(1), viewport[1].max(1));
        match self {
            Self::Rotate([x, y, z]) => {
                view.rotate_view(x, y, z);
                camera.set_rotation(view.rotation());
            }
            Self::Translate([x, y, z]) => {
                view.translate_view(x, y, z);
                camera.set_center(view.center());
            }
            Self::Center([x, y, z]) => camera.set_center(Vector4::new3(x, y, z)),
            Self::Zoom(value) => {
                view.set_zoom(value);
                camera.set_zoom(view.zoom());
            }
            Self::Slab(value) => {
                view.set_slab(value);
                camera.set_slab_depth(view.slab());
            }
            Self::Distance(value) => {
                view.set_distance(value);
                camera.set_cam_dist(view.distance());
            }
            Self::Perspective(value) => camera.set_perspective(value),
            Self::CenterMark(value) => camera.set_center_mark(value),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_preserve_camera_metadata_and_use_view_clamps() {
        let mut camera = Camera::new();
        camera.set_name("detail");
        camera.set_source("view.cam");
        camera.set_slab_depth(300.);
        ViewTransform::Distance(10.)
            .apply(&mut camera, [800, 200])
            .unwrap();
        assert_eq!(camera.slab_depth(), 300.);
        ViewTransform::Slab(300.)
            .apply(&mut camera, [800, 200])
            .unwrap();
        assert_eq!(camera.slab_depth(), 20.);
        ViewTransform::Zoom(0.)
            .apply(&mut camera, [800, 200])
            .unwrap();
        assert_eq!(camera.zoom(), 0.0001);
        ViewTransform::Zoom(50.)
            .apply(&mut camera, [800, 200])
            .unwrap();
        ViewTransform::Rotate([30., 45., 90.])
            .apply(&mut camera, [800, 200])
            .unwrap();
        ViewTransform::Translate([4., 0., 0.])
            .apply(&mut camera, [800, 200])
            .unwrap();
        let center = camera.center();
        assert!(center.x.abs() < 1e-9);
        assert!((center.y + 0.8660254037844387).abs() < 1e-9);
        assert!((center.z + 0.5).abs() < 1e-9);
        assert_eq!(camera.name(), "detail");
        assert_eq!(camera.source(), "view.cam");
        for transform in [
            ViewTransform::Distance(f64::NAN),
            ViewTransform::Translate([f64::INFINITY, 0., 0.]),
        ] {
            let before = camera.clone();
            assert!(transform.apply(&mut camera, [800, 200]).is_err());
            assert_eq!(camera, before);
        }
    }
}
