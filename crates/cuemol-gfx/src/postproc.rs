use crate::{
    DataTexture, RenderTarget, RT_TU_COLOR, RT_TU_DEPTH, RT_TU_NORMAL, RT_TU_SMAA_AREA,
    RT_TU_SMAA_SEARCH,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AoConstants {
    pub is_ortho: i32,
    pub depth_linearize_mul: f32,
    pub depth_linearize_add: f32,
    pub ndc_to_view_mul: [f32; 2],
    pub ndc_to_view_add: [f32; 2],
    pub viewport_pixel_size: [f32; 2],
    pub ao_texel_size: [f32; 2],
    pub effect_radius: f32,
    pub final_value_power: f32,
    pub max_screenspace_radius: f32,
    pub fog_end: f32,
    pub fog_scale: f32,
    pub slice_count: i32,
    pub steps_per_slice: i32,
    pub ao_noise_offset: f32,
    pub smaa_threshold: f32,
}

impl Default for AoConstants {
    fn default() -> Self {
        Self {
            is_ortho: 0,
            depth_linearize_mul: 0.0,
            depth_linearize_add: 0.0,
            ndc_to_view_mul: [0.0; 2],
            ndc_to_view_add: [0.0; 2],
            viewport_pixel_size: [0.0; 2],
            ao_texel_size: [0.0; 2],
            effect_radius: 1.0,
            final_value_power: 2.2,
            max_screenspace_radius: 256.0,
            fog_end: 0.0,
            fog_scale: 0.0,
            slice_count: 9,
            steps_per_slice: 3,
            ao_noise_offset: 0.0,
            smaa_threshold: 0.05,
        }
    }
}

impl AoConstants {
    #[allow(clippy::too_many_arguments)]
    pub fn from_camera(
        camera_distance: f64,
        zoom: f64,
        slab_depth: f64,
        aspect: f64,
        backing_width: i32,
        backing_height: i32,
        perspective: bool,
    ) -> Self {
        let slab_depth = slab_depth.max(0.1);
        let slab_near = (camera_distance - slab_depth / 2.0).max(0.1);
        let slab_far = camera_distance + slab_depth;
        let half_width = zoom / 2.0;
        let mut constants = Self {
            is_ortho: i32::from(!perspective),
            ..Self::default()
        };
        if perspective {
            constants.depth_linearize_mul = (slab_far * slab_near / (slab_far - slab_near)) as f32;
            constants.depth_linearize_add = (slab_far / (slab_far - slab_near)) as f32;
            let tan_half_fov_y = half_width / camera_distance;
            let tan_half_fov_x = tan_half_fov_y * aspect;
            constants.ndc_to_view_mul =
                [(2.0 * tan_half_fov_x) as f32, (2.0 * tan_half_fov_y) as f32];
            constants.ndc_to_view_add = [-(tan_half_fov_x as f32), -(tan_half_fov_y as f32)];
        } else {
            constants.depth_linearize_mul = slab_near as f32;
            constants.depth_linearize_add = (slab_far - slab_near) as f32;
            constants.ndc_to_view_mul = [
                (2.0 * half_width * aspect) as f32,
                (2.0 * half_width) as f32,
            ];
            constants.ndc_to_view_add = [-(half_width * aspect) as f32, -half_width as f32];
        }
        constants.viewport_pixel_size = [
            if backing_width > 0 {
                1.0 / backing_width as f32
            } else {
                0.0
            },
            if backing_height > 0 {
                1.0 / backing_height as f32
            } else {
                0.0
            },
        ];
        constants
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PostProcessPass {
    DepthVisualize {
        near: f32,
        far: f32,
    },
    Composite {
        has_ao: bool,
        constants: AoConstants,
    },
    Gtao {
        has_normal: bool,
        debug_mode: i32,
        constants: AoConstants,
    },
    Denoise {
        constants: AoConstants,
    },
    Fxaa {
        constants: AoConstants,
    },
    SmaaEdges {
        constants: AoConstants,
    },
    SmaaWeights {
        constants: AoConstants,
    },
    SmaaBlend {
        constants: AoConstants,
    },
    JitterCompose {
        weight: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostProcGpuPrim {
    initialized: bool,
    fullscreen_triangle: [[f32; 2]; 3],
    smaa_area_texture: Option<DataTexture>,
    smaa_search_texture: Option<DataTexture>,
    passes: Vec<PostProcessPass>,
}

impl Default for PostProcGpuPrim {
    fn default() -> Self {
        Self {
            initialized: false,
            fullscreen_triangle: [[-1.0, -1.0], [3.0, -1.0], [-1.0, 3.0]],
            smaa_area_texture: None,
            smaa_search_texture: None,
            passes: Vec::new(),
        }
    }
}

impl PostProcGpuPrim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self) -> bool {
        self.initialized = true;
        true
    }

    pub fn is_valid(&self) -> bool {
        self.initialized
    }

    pub fn invalidate(&mut self) {
        self.initialized = false;
        self.smaa_area_texture = None;
        self.smaa_search_texture = None;
        self.passes.clear();
    }

    pub fn fullscreen_triangle(&self) -> &[[f32; 2]; 3] {
        &self.fullscreen_triangle
    }

    pub fn passes(&self) -> &[PostProcessPass] {
        &self.passes
    }

    pub fn set_smaa_textures(&mut self, area: DataTexture, search: DataTexture) {
        self.smaa_area_texture = Some(area);
        self.smaa_search_texture = Some(search);
    }

    pub fn ensure_smaa_textures(&self) -> bool {
        self.smaa_area_texture.is_some() && self.smaa_search_texture.is_some()
    }

    pub fn draw_depth_visualization(
        &mut self,
        target: &mut RenderTarget,
        near: f32,
        far: f32,
    ) -> bool {
        if !self.ensure_initialized() || target.bind_depth_texture(RT_TU_DEPTH).is_err() {
            return false;
        }
        self.passes
            .push(PostProcessPass::DepthVisualize { near, far });
        target.unbind_textures();
        true
    }

    pub fn draw_composite(
        &mut self,
        scene: &mut RenderTarget,
        mut ao: Option<&mut RenderTarget>,
        constants: AoConstants,
    ) -> bool {
        if !self.ensure_initialized() || scene.bind_color_texture(0, RT_TU_COLOR).is_err() {
            return false;
        }
        if let Some(target) = ao.as_deref_mut() {
            if target.bind_color_texture(0, RT_TU_NORMAL).is_err()
                || scene.bind_depth_texture(RT_TU_DEPTH).is_err()
            {
                scene.unbind_textures();
                target.unbind_textures();
                return false;
            }
        }
        self.passes.push(PostProcessPass::Composite {
            has_ao: ao.is_some(),
            constants,
        });
        scene.unbind_textures();
        if let Some(target) = ao {
            target.unbind_textures();
        }
        true
    }

    pub fn draw_gtao(
        &mut self,
        scene: &mut RenderTarget,
        constants: AoConstants,
        debug_mode: i32,
    ) -> bool {
        if !self.ensure_initialized() || scene.bind_depth_texture(RT_TU_DEPTH).is_err() {
            return false;
        }
        let has_normal = scene.has_normal();
        if has_normal && scene.bind_color_texture(1, RT_TU_NORMAL).is_err() {
            scene.unbind_textures();
            return false;
        }
        self.passes.push(PostProcessPass::Gtao {
            has_normal,
            debug_mode,
            constants,
        });
        scene.unbind_textures();
        true
    }

    pub fn draw_denoise(&mut self, ao: &mut RenderTarget, constants: AoConstants) -> bool {
        self.draw_single_color_pass(ao, PostProcessPass::Denoise { constants })
    }

    pub fn draw_fxaa(&mut self, source: &mut RenderTarget, constants: AoConstants) -> bool {
        self.draw_single_color_pass(source, PostProcessPass::Fxaa { constants })
    }

    pub fn draw_smaa_edges(&mut self, source: &mut RenderTarget, constants: AoConstants) -> bool {
        self.draw_single_color_pass(source, PostProcessPass::SmaaEdges { constants })
    }

    pub fn draw_smaa_weights(&mut self, edges: &mut RenderTarget, constants: AoConstants) -> bool {
        if !self.ensure_smaa_textures() || !self.ensure_initialized() {
            return false;
        }
        if edges.bind_color_texture(0, RT_TU_COLOR).is_err() {
            return false;
        }
        self.smaa_area_texture
            .as_mut()
            .expect("SMAA area texture is missing")
            .bind(RT_TU_SMAA_AREA);
        self.smaa_search_texture
            .as_mut()
            .expect("SMAA search texture is missing")
            .bind(RT_TU_SMAA_SEARCH);
        self.passes.push(PostProcessPass::SmaaWeights { constants });
        edges.unbind_textures();
        self.smaa_area_texture.as_mut().unwrap().unbind();
        self.smaa_search_texture.as_mut().unwrap().unbind();
        true
    }

    pub fn draw_smaa_blend(
        &mut self,
        source: &mut RenderTarget,
        weights: &mut RenderTarget,
        constants: AoConstants,
    ) -> bool {
        if !self.ensure_initialized()
            || source.bind_color_texture(0, RT_TU_COLOR).is_err()
            || weights.bind_color_texture(0, RT_TU_NORMAL).is_err()
        {
            source.unbind_textures();
            weights.unbind_textures();
            return false;
        }
        self.passes.push(PostProcessPass::SmaaBlend { constants });
        source.unbind_textures();
        weights.unbind_textures();
        true
    }

    pub fn draw_jitter_compose(&mut self, source: &mut RenderTarget, weight: f32) -> bool {
        self.draw_single_color_pass(source, PostProcessPass::JitterCompose { weight })
    }

    fn draw_single_color_pass(&mut self, source: &mut RenderTarget, pass: PostProcessPass) -> bool {
        if !self.ensure_initialized() || source.bind_color_texture(0, RT_TU_COLOR).is_err() {
            return false;
        }
        self.passes.push(pass);
        source.unbind_textures();
        true
    }

    fn ensure_initialized(&mut self) -> bool {
        if !self.initialized {
            self.init();
        }
        self.initialized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RT_COLOR_RGBA8, RT_DEPTH_TEX, RT_NORMAL_RGBA16F};

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1.0e-5, "{actual} != {expected}");
    }

    #[test]
    fn camera_constants_match_perspective_and_orthographic_reference_math() {
        let perspective = AoConstants::from_camera(10.0, 4.0, 6.0, 2.0, 800, 400, true);
        assert_eq!(perspective.is_ortho, 0);
        assert_close(perspective.depth_linearize_mul, 112.0 / 9.0);
        assert_close(perspective.depth_linearize_add, 16.0 / 9.0);
        assert_eq!(perspective.ndc_to_view_mul, [0.8, 0.4]);
        assert_eq!(perspective.ndc_to_view_add, [-0.4, -0.2]);
        assert_eq!(perspective.viewport_pixel_size, [0.00125, 0.0025]);

        let orthographic = AoConstants::from_camera(10.0, 4.0, 6.0, 2.0, 0, 0, false);
        assert_eq!(orthographic.is_ortho, 1);
        assert_eq!(orthographic.depth_linearize_mul, 7.0);
        assert_eq!(orthographic.depth_linearize_add, 9.0);
        assert_eq!(orthographic.ndc_to_view_mul, [8.0, 4.0]);
        assert_eq!(orthographic.ndc_to_view_add, [-4.0, -2.0]);
        assert_eq!(orthographic.viewport_pixel_size, [0.0, 0.0]);
    }

    #[test]
    fn post_processing_tracks_passes_and_releases_texture_bindings() {
        let mut scene =
            RenderTarget::new(4, 4, RT_COLOR_RGBA8 | RT_DEPTH_TEX | RT_NORMAL_RGBA16F).unwrap();
        let mut ao = RenderTarget::new(2, 2, RT_COLOR_RGBA8).unwrap();
        let mut primitive = PostProcGpuPrim::new();
        let constants = AoConstants::default();
        assert!(primitive.draw_gtao(&mut scene, constants, 0));
        assert!(primitive.draw_composite(&mut scene, Some(&mut ao), constants));
        assert_eq!(primitive.fullscreen_triangle()[1], [3.0, -1.0]);
        assert_eq!(primitive.passes().len(), 2);
        assert!(scene.bound_color_textures().is_empty());
        assert!(ao.bound_color_textures().is_empty());
        assert!(matches!(
            primitive.passes()[0],
            PostProcessPass::Gtao {
                has_normal: true,
                ..
            }
        ));
    }

    #[test]
    fn smaa_weights_require_both_lookup_textures() {
        let mut edges = RenderTarget::new(2, 2, RT_COLOR_RGBA8).unwrap();
        let mut primitive = PostProcGpuPrim::new();
        assert!(!primitive.draw_smaa_weights(&mut edges, AoConstants::default()));
        primitive.set_smaa_textures(
            DataTexture::new(1, 1, 2, true, vec![0, 0]).unwrap(),
            DataTexture::new(1, 1, 1, false, vec![0]).unwrap(),
        );
        assert!(primitive.draw_smaa_weights(&mut edges, AoConstants::default()));
        assert!(edges.bound_color_textures().is_empty());
    }
}
