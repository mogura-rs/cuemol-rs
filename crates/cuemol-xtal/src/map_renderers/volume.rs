use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct MapVolumeData {
    pub counts: [usize; 3],
    pub values: Arc<Vec<u8>>,
    pub texture_to_world: Matrix4,
    pub transfer: [[u8; 4]; 256],
}

impl GlslMapVolRenderer {
    pub fn transfer_function(&self, scalar: &impl ScalarObject) -> [[u8; 4]; 256] {
        let level = self.map.level_from(scalar);
        let threshold = ((level - scalar.level_base()) / scalar.level_step())
            .floor()
            .clamp(0., 255.) as usize;
        std::array::from_fn(|i| {
            let density = (i as f64 / 255.).powf(1.5);
            let hue = density.rem_euclid(1.) * 6.;
            let fraction = hue.fract();
            let rgb = match hue.floor() as u8 {
                0 => [1., fraction, 0.],
                1 => [1. - fraction, 1., 0.],
                2 => [0., 1., fraction],
                3 => [0., 1. - fraction, 1.],
                4 => [fraction, 0., 1.],
                _ => [1., 0., 1. - fraction],
            };
            let alpha = density * if i > threshold { 2. } else { 0.1 };
            [rgb[0], rgb[1], rgb[2], alpha].map(|v| (v * 255.).clamp(0., 255.) as u8)
        })
    }

    pub fn volume_data(&self, map: &DensityMap) -> Option<MapVolumeData> {
        let region = self
            .map
            .sampling_region(map, None, self.map.extent(), None, 1, false)?;
        let range = region.range();
        let original = range.node_counts();
        let mut counts = region.sampled_counts();
        while counts.iter().try_fold(1usize, |n, &v| n.checked_mul(v))? > 16 * 1024 * 1024 {
            counts = counts.map(|n| n.div_ceil(2));
        }
        let values = if counts == original && counts == map.counts && range.start == [0; 3] {
            map.bytes.clone()
        } else {
            let mut values = Vec::with_capacity(counts.iter().product());
            for z in 0..counts[2] {
                for y in 0..counts[1] {
                    for x in 0..counts[0] {
                        let indices = std::array::from_fn(|axis| {
                            range.start[axis]
                                + ((([x, y, z][axis] as f64 + 0.5) * original[axis] as f64
                                    / counts[axis] as f64)
                                    - 0.5)
                                    .round() as isize
                        });
                        let [x, y, z] = range.map_indices(map, indices);
                        values.push(map.at_byte(x, y, z));
                    }
                }
            }
            Arc::new(values)
        };
        let start = Vector4::new3(
            range.start[0] as f64,
            range.start[1] as f64,
            range.start[2] as f64,
        );
        let origin = map.conv_to_orth(start);
        let axes = std::array::from_fn::<_, 3, _>(|axis| {
            let mut offset = [0.; 3];
            offset[axis] = original[axis] as f64;
            map.conv_to_orth(start + Vector4::new3(offset[0], offset[1], offset[2])) - origin
        });
        Some(MapVolumeData {
            counts,
            values,
            texture_to_world: Matrix4 {
                values: [
                    [axes[0].x, axes[1].x, axes[2].x, origin.x],
                    [axes[0].y, axes[1].y, axes[2].y, origin.y],
                    [axes[0].z, axes[1].z, axes[2].z, origin.z],
                    [0., 0., 0., 1.],
                ],
            },
            transfer: self.transfer_function(map),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_data_shares_full_grids_and_samples_periodic_regions() {
        let mut map = DensityMap::new();
        map.set_map_byte_array(&(0..64).collect::<Vec<_>>(), [4; 3], 0., 256., 31.5, 1.);
        map.set_map_params([2, 3, 4], [4; 3]);
        map.set_xtal_params(4., 4., 4., 90., 90., 60.);
        let mut renderer = GlslMapVolRenderer::new();
        renderer.set_use_pbc(false);
        renderer.set_extent(100.);
        let volume = renderer.volume_data(&map).unwrap();
        assert!(Arc::ptr_eq(&map.bytes, &volume.values));
        assert!(
            (volume
                .texture_to_world
                .transform_point(Vector4::new4(0., 0., 0., 1.))
                - map.conv_to_orth(Vector4::ZERO))
            .length3d()
                < 1e-10
        );
        let expected = map.conv_to_orth(Vector4::new3(4., 4., 4.));
        assert!(
            (volume
                .texture_to_world
                .transform_point(Vector4::new4(1., 1., 1., 1.))
                - expected)
                .length3d()
                < 1e-10
        );
        renderer.set_use_pbc(true);
        renderer.set_center(map.conv_to_orth(Vector4::ZERO));
        renderer.set_extent(1.);
        let periodic = renderer.volume_data(&map).unwrap();
        assert!(periodic.values.iter().all(|value| *value < 64));
        assert!(!Arc::ptr_eq(&map.bytes, &periodic.values));
    }

    #[test]
    fn transfer_table_uses_reference_rainbow_and_threshold_opacity() {
        let mut map = DensityMap::new();
        map.set_map_byte_array(&[0, 64, 128, 255], [4, 1, 1], 0., 256., 0., 1.);
        let mut renderer = GlslMapVolRenderer::new();
        renderer.set_siglevel(128.);
        let table = renderer.transfer_function(&map);
        assert_eq!(table[0], [255, 0, 0, 0]);
        assert_eq!(table[255], [255, 0, 0, 255]);
        assert!(table[128][3] < 10);
        assert!(table[129][3] > 180);
        for kind in [
            GlslMapVolRenderer::GLMV_MULTIGRAD,
            GlslMapVolRenderer::GLMV_AUTO2,
        ] {
            renderer.set_xferfn_type(kind);
            assert_eq!(renderer.transfer_function(&map), table);
        }
    }
}
