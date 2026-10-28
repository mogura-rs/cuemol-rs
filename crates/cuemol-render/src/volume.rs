use cuemol_core::{CueMolError, Matrix4, Result, Vector4};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct RenderVolume {
    pub counts: [usize; 3],
    pub values: Arc<Vec<u8>>,
    pub transfer: [[f32; 4]; 256],
    pub alpha: f32,
    texture_to_world: Matrix4,
    world_to_texture: Matrix4,
    sample_step: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct VolumeSlices {
    pub lower: [f64; 3],
    pub upper: [f64; 3],
    pub count: u32,
    pub thickness: f64,
}

impl RenderVolume {
    pub fn new(
        counts: [usize; 3],
        values: Arc<Vec<u8>>,
        texture_to_world: Matrix4,
        transfer: [[u8; 4]; 256],
    ) -> Result<Self> {
        if counts.contains(&0)
            || counts.iter().try_fold(1usize, |n, &v| n.checked_mul(v)) != Some(values.len())
            || texture_to_world
                .values
                .iter()
                .flatten()
                .any(|v| !v.is_finite())
        {
            return Err(CueMolError::invalid_argument("invalid volume grid"));
        }
        let mut volume = Self {
            counts,
            values,
            transfer: transfer.map(|rgba| rgba.map(|v| f32::from(v) / 255.)),
            alpha: 1.,
            texture_to_world,
            world_to_texture: texture_to_world.invert()?,
            sample_step: 0.,
        };
        volume.update_step();
        Ok(volume)
    }

    pub fn texture_to_world(&self) -> Matrix4 {
        self.texture_to_world
    }

    pub fn world_to_texture(&self) -> Matrix4 {
        self.world_to_texture
    }

    fn update_step(&mut self) {
        self.sample_step = (0..3)
            .map(|axis| {
                let length = (0..3)
                    .map(|row| self.texture_to_world.values[row][axis].powi(2))
                    .sum::<f64>()
                    .sqrt();
                length / self.counts[axis] as f64 * 0.5
            })
            .fold(f64::INFINITY, f64::min);
    }

    pub fn apply_xform(&mut self, transform: &Matrix4) {
        self.texture_to_world = transform.mul_mat(self.texture_to_world);
        if let Ok(inverse) = self.texture_to_world.invert() {
            self.world_to_texture = inverse;
            self.update_step();
        } else {
            self.sample_step = 0.;
        }
    }

    pub fn corners(&self) -> [Vector4; 8] {
        std::array::from_fn(|i| {
            self.texture_to_world.transform_point(Vector4::new4(
                (i & 1) as f64,
                ((i >> 1) & 1) as f64,
                ((i >> 2) & 1) as f64,
                1.,
            ))
        })
    }

    pub fn slices(
        &self,
        eye: Vector4,
        right: Vector4,
        up: Vector4,
        front: Vector4,
    ) -> Option<VolumeSlices> {
        if !self.sample_step.is_finite() || self.sample_step <= 0. || self.alpha <= 0. {
            return None;
        }
        let mut lower = [f64::INFINITY; 3];
        let mut upper = [f64::NEG_INFINITY; 3];
        for corner in self.corners() {
            let delta = corner - eye;
            for (axis, basis) in [right, up, front].into_iter().enumerate() {
                let value = delta.dot3d(basis);
                lower[axis] = lower[axis].min(value);
                upper[axis] = upper[axis].max(value);
            }
        }
        let depth = upper[2] - lower[2];
        if !depth.is_finite() || depth <= 0. {
            return None;
        }
        let count = (depth / self.sample_step).ceil().clamp(1., 4096.) as u32;
        Some(VolumeSlices {
            lower,
            upper,
            count,
            thickness: depth / f64::from(count),
        })
    }

    pub fn sample(&self, coordinate: Vector4) -> [f64; 4] {
        let coordinates = [coordinate.x, coordinate.y, coordinate.z];
        if coordinates
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return [0.; 4];
        }
        let grid = std::array::from_fn::<_, 3, _>(|axis| {
            (coordinates[axis] * self.counts[axis] as f64 - 0.5)
                .clamp(0., (self.counts[axis] - 1) as f64)
        });
        let lower = grid.map(|v| v.floor() as usize);
        let fraction = std::array::from_fn::<_, 3, _>(|i| grid[i] - lower[i] as f64);
        let mut density = 0.;
        for corner in 0..8 {
            let mut index = [0; 3];
            let mut weight = 1.;
            for axis in 0..3 {
                let upper = (corner >> axis) & 1;
                index[axis] = (lower[axis] + upper).min(self.counts[axis] - 1);
                weight *= if upper == 0 {
                    1. - fraction[axis]
                } else {
                    fraction[axis]
                };
            }
            density += f64::from(
                self.values[index[0] + self.counts[0] * (index[1] + self.counts[1] * index[2])],
            ) * weight;
        }
        let position = (density / 255. * 256. - 0.5).clamp(0., 255.);
        let first = position.floor() as usize;
        let next = (first + 1).min(255);
        let fraction = position - first as f64;
        std::array::from_fn(|i| {
            f64::from(self.transfer[first][i]) * (1. - fraction)
                + f64::from(self.transfer[next][i]) * fraction
        })
    }

    pub(crate) fn ray_samples(
        &self,
        origin: Vector4,
        direction: Vector4,
        eye: Vector4,
        front: Vector4,
        far_limit: f64,
    ) -> Option<VolumeRaySamples<'_>> {
        let tex_origin = self
            .world_to_texture
            .transform_point(Vector4::new4(origin.x, origin.y, origin.z, 1.));
        let tex_direction = self.world_to_texture.transform_point(Vector4::new4(
            direction.x,
            direction.y,
            direction.z,
            0.,
        ));
        let mut near: f64 = 0.;
        let mut far = far_limit;
        for (position, velocity) in [tex_origin.x, tex_origin.y, tex_origin.z].into_iter().zip([
            tex_direction.x,
            tex_direction.y,
            tex_direction.z,
        ]) {
            if velocity.abs() <= 1e-15 {
                if !(0. ..=1.).contains(&position) {
                    return None;
                }
            } else {
                let a = -position / velocity;
                let b = (1. - position) / velocity;
                near = near.max(a.min(b));
                far = far.min(a.max(b));
            }
        }
        if near > far {
            return None;
        }
        let slices = self.slices(eye, Vector4::ZERO, Vector4::ZERO, front)?;
        let z0 = (origin - eye).dot3d(front);
        let dz = direction.dot3d(front);
        if dz.abs() <= 1e-15 {
            return None;
        }
        let at_near = (z0 + dz * near - slices.lower[2]) / slices.thickness;
        let at_far = (z0 + dz * far - slices.lower[2]) / slices.thickness;
        let (first, last, stride) = if dz < 0. {
            (
                at_near.floor().min(f64::from(slices.count) - 1.) as i32,
                at_far.ceil().max(0.) as i32,
                -1,
            )
        } else {
            (
                at_near.ceil().max(0.) as i32,
                at_far.floor().min(f64::from(slices.count) - 1.) as i32,
                1,
            )
        };
        if (last - first) * stride < 0 {
            return None;
        }
        Some(VolumeRaySamples {
            volume: self,
            tex_origin,
            tex_direction,
            distance: (slices.lower[2] + f64::from(first) * slices.thickness - z0) / dz,
            increment: slices.thickness * f64::from(stride) / dz,
            remaining: (last - first).unsigned_abs() + 1,
            thickness: slices.thickness,
        })
    }
}

pub(crate) struct VolumeRaySamples<'a> {
    volume: &'a RenderVolume,
    tex_origin: Vector4,
    tex_direction: Vector4,
    distance: f64,
    increment: f64,
    remaining: u32,
    thickness: f64,
}

impl Iterator for VolumeRaySamples<'_> {
    type Item = (f64, [f64; 4]);
    fn next(&mut self) -> Option<Self::Item> {
        while self.remaining > 0 {
            self.remaining -= 1;
            let distance = self.distance;
            self.distance += self.increment;
            let mut color = self
                .volume
                .sample(self.tex_origin + self.tex_direction * distance);
            color[3] = (color[3] * self.thickness * f64::from(self.volume.alpha)).clamp(0., 1.);
            if color[3] > 0. {
                return Some((distance, color));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn volume_ray_samples_follow_texture_centers_and_world_transforms() {
        let palette = std::array::from_fn(|i| [i as u8, 0, 0, 255]);
        let mut volume = RenderVolume::new(
            [2, 1, 1],
            Arc::new(vec![0, 255]),
            Matrix4::IDENTITY,
            palette,
        )
        .unwrap();
        assert_eq!(volume.sample(Vector4::new3(0.25, 0.5, 0.5))[0], 0.);
        assert_eq!(volume.sample(Vector4::new3(0.75, 0.5, 0.5))[0], 1.);
        assert!((volume.sample(Vector4::new3(0.5, 0.5, 0.5))[0] - 0.5).abs() < 1e-7);
        let mut transform = Matrix4::IDENTITY;
        transform.values[0][3] = 2.;
        transform.values[2][2] = 3.;
        volume.apply_xform(&transform);
        let eye = Vector4::new3(2.5, 0.5, 5.);
        let samples = volume
            .ray_samples(
                eye,
                Vector4::new3(0., 0., -1.),
                eye,
                Vector4::new3(0., 0., 1.),
                4.,
            )
            .unwrap()
            .collect::<Vec<_>>();
        assert!(!samples.is_empty());
        assert!(samples.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(samples
            .iter()
            .all(|(depth, color)| (2. ..=4.).contains(depth) && (color[0] - 0.5).abs() < 1e-7));
    }
}
