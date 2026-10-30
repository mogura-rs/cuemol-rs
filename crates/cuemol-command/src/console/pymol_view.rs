use super::{pymol::tool, Action};
use cuemol_core::{Matrix3, Quat, Vector4};
use cuemol_qsys::Camera;
use serde_json::json;

pub(super) fn values(camera: &Camera) -> [f64; 18] {
    let rotation = camera.rotation().to_rot_matrix().values;
    let mut distance = camera.cam_dist();
    let mut fov = (2. * (camera.zoom() / (2. * distance)).atan()).to_degrees();
    if fov <= 1. {
        fov = 20.;
        distance = camera.zoom() / (2. * 10_f64.to_radians().tan());
    }
    let center = camera.center();
    [
        rotation[0][0],
        rotation[1][0],
        rotation[2][0],
        rotation[0][1],
        rotation[1][1],
        rotation[2][1],
        rotation[0][2],
        rotation[1][2],
        rotation[2][2],
        0.,
        0.,
        -distance,
        center.x,
        center.y,
        center.z,
        (distance - camera.slab_depth() / 2.).max(0.1),
        distance + camera.slab_depth() / 2.,
        if camera.is_perspective() { -fov } else { fov },
    ]
}

pub(super) fn format(camera: &Camera) -> String {
    let values = values(camera);
    format!(
        "set_view ({})",
        values
            .iter()
            .map(|value| format!("{value:.9}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(super) fn parse(raw: &str, prior: &Camera) -> Result<Camera, String> {
    let text = raw.trim().trim_matches(['(', ')', '[', ']']);
    let values = text
        .split(',')
        .map(|value| value.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "set_view needs eighteen finite numbers")?;
    if values.len() != 18 || values.iter().any(|value| !value.is_finite()) {
        return Err("set_view needs eighteen finite numbers".into());
    }
    let rotation = [
        [values[0], values[3], values[6]],
        [values[1], values[4], values[7]],
        [values[2], values[5], values[8]],
    ];
    let distance = -values[11];
    let slab = 2. * (distance - values[15]).max(values[16] - distance);
    let fov = if values[17].abs() > 1. {
        values[17].abs()
    } else {
        20.
    };
    if distance <= 0. || slab <= 0. || fov >= 180. {
        return Err(
            "set_view requires positive camera distances and a field of view below 180 degrees"
                .into(),
        );
    }
    let mut camera = prior.clone();
    camera.set_rotation(
        Quat::from_rot_matrix(Matrix3::new(rotation)).map_err(|error| error.to_string())?,
    );
    camera.set_center(Vector4::new3(
        values[12] - (rotation[0][0] * values[9] + rotation[1][0] * values[10]),
        values[13] - (rotation[0][1] * values[9] + rotation[1][1] * values[10]),
        values[14] - (rotation[0][2] * values[9] + rotation[1][2] * values[10]),
    ));
    camera.set_cam_dist(distance);
    camera.set_slab_depth(slab);
    camera.set_zoom(2. * distance * (fov.to_radians() / 2.).tan());
    camera.set_perspective(values[17] <= 0.5);
    Ok(camera)
}

pub(super) fn actions(camera: &Camera) -> Result<Vec<Action>, String> {
    [
        ("rotation", camera.rotation().to_string()),
        ("center", camera.center().to_string()),
        ("distance", camera.cam_dist().to_string()),
        ("slab", camera.slab_depth().to_string()),
        ("zoom", camera.zoom().to_string()),
        ("perspective", camera.is_perspective().to_string()),
    ]
    .into_iter()
    .map(|(name, value)| {
        tool(
            "set_node_prop",
            json!({"nodeType":"view","prop":name,"value":value}),
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_reference_view_convention_keeps_rotation_center_and_projection() {
        let mut camera = Camera::new();
        camera.set_rotation(Quat::from_axis_angle(Vector4::new3(0., 1., 0.), 0.7).unwrap());
        camera.set_center(Vector4::new3(1., 2., 3.));
        camera.set_cam_dist(80.);
        camera.set_slab_depth(50.);
        camera.set_zoom(30.);
        for perspective in [false, true] {
            camera.set_perspective(perspective);
            let line = format(&camera);
            let restored = parse(line.strip_prefix("set_view ").unwrap(), &Camera::new()).unwrap();
            assert!((restored.center() - camera.center()).length3d() < 1e-8);
            assert!((restored.zoom() - camera.zoom()).abs() < 1e-7);
            assert!((restored.rotation().y - camera.rotation().y).abs() < 1e-8);
            assert_eq!(restored.is_perspective(), perspective);
        }
        let offset = parse("(1,0,0,0,1,0,0,0,1,2,3,-50,10,20,30,40,60,20)", &camera).unwrap();
        assert_eq!(offset.center(), Vector4::new3(8., 17., 30.));
        assert_eq!(offset.slab_depth(), 20.);
        assert!(parse("(1,2,3)", &camera).is_err());
    }
}
