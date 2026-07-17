use std::collections::BTreeMap;

use cuemol_core::{Matrix3, Matrix4, Result};

use crate::DisplayContext;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FogBlock {
    pub fog_end: f32,
    pub fog_scale: f32,
    pub padding1: f32,
    pub padding2: f32,
    pub fog_color: [f32; 3],
    pub padding3: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatricesBlock {
    pub model_view: [f32; 16],
    pub projection: [f32; 16],
    pub normal: [f32; 16],
}

impl Default for MatricesBlock {
    fn default() -> Self {
        Self {
            model_view: [0.0; 16],
            projection: [0.0; 16],
            normal: [0.0; 16],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UniformValue {
    Integers(Vec<i32>),
    Floats(Vec<f32>),
    Matrix3([f32; 9]),
    Matrix4([f32; 16]),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShaderObject {
    name: String,
    shader_paths: BTreeMap<String, String>,
    enabled: bool,
    uniforms: BTreeMap<String, UniformValue>,
    attribute_locations: BTreeMap<String, i32>,
    fog_block: Option<FogBlock>,
    matrices_block: Option<MatricesBlock>,
    draw_params_size: usize,
    draw_params: Vec<u8>,
}

impl ShaderObject {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_shaders(
        &mut self,
        shaders: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> bool {
        self.shader_paths = shaders
            .into_iter()
            .map(|(name, path)| (name.into(), path.into()))
            .collect();
        !self.shader_paths.is_empty()
    }

    pub fn shader_paths(&self) -> &BTreeMap<String, String> {
        &self.shader_paths
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_uniform(&mut self, name: impl Into<String>, values: &[i32]) {
        assert!((1..=4).contains(&values.len()));
        self.uniforms
            .insert(name.into(), UniformValue::Integers(values.to_vec()));
    }

    pub fn set_uniform_f(&mut self, name: impl Into<String>, values: &[f32]) {
        assert!((1..=4).contains(&values.len()));
        self.uniforms
            .insert(name.into(), UniformValue::Floats(values.to_vec()));
    }

    pub fn set_matrix4(&mut self, name: impl Into<String>, matrix: Matrix4) {
        self.uniforms.insert(
            name.into(),
            UniformValue::Matrix4(matrix_to_column_major(matrix)),
        );
    }

    pub fn set_matrix3(&mut self, name: impl Into<String>, matrix: Matrix3) {
        let mut values = [0.0; 9];
        for column in 0..3 {
            for row in 0..3 {
                values[column * 3 + row] = matrix.values[row][column] as f32;
            }
        }
        self.uniforms
            .insert(name.into(), UniformValue::Matrix3(values));
    }

    pub fn uniforms(&self) -> &BTreeMap<String, UniformValue> {
        &self.uniforms
    }

    pub fn set_attribute_location(&mut self, name: impl Into<String>, location: i32) {
        self.attribute_locations.insert(name.into(), location);
    }

    pub fn attribute_location(&self, name: &str) -> i32 {
        self.attribute_locations.get(name).copied().unwrap_or(-1)
    }

    pub fn setup_fog(&mut self, context: &DisplayContext) {
        let fog_scale = 1.0 / (context.fog_end() - context.fog_start());
        let color = context.fog_color();
        self.fog_block = Some(FogBlock {
            fog_end: context.fog_end(),
            fog_scale,
            padding1: 0.0,
            padding2: 0.0,
            fog_color: [color.fr() as f32, color.fg() as f32, color.fb() as f32],
            padding3: 0.0,
        });
    }

    pub fn setup_matrices(&mut self, context: &DisplayContext) -> Result<()> {
        let model_view = context.model_view_matrix();
        let projection = context.projection_matrix();
        let linear = Matrix3::new([
            [
                model_view.values[0][0],
                model_view.values[0][1],
                model_view.values[0][2],
            ],
            [
                model_view.values[1][0],
                model_view.values[1][1],
                model_view.values[1][2],
            ],
            [
                model_view.values[2][0],
                model_view.values[2][1],
                model_view.values[2][2],
            ],
        ]);
        let inverse = linear.invert()?;
        let mut normal = [0.0; 16];
        for column in 0..3 {
            for row in 0..3 {
                normal[column * 4 + row] = inverse.values[column][row] as f32;
            }
        }
        normal[15] = 1.0;
        self.matrices_block = Some(MatricesBlock {
            model_view: matrix_to_column_major(model_view),
            projection: matrix_to_column_major(projection),
            normal,
        });
        Ok(())
    }

    pub fn fog_block(&self) -> Option<&FogBlock> {
        self.fog_block.as_ref()
    }

    pub fn matrices_block(&self) -> Option<&MatricesBlock> {
        self.matrices_block.as_ref()
    }

    pub fn init_draw_params_ubo(&mut self, size: usize) {
        self.draw_params_size = size;
        self.draw_params = vec![0; size];
    }

    pub fn update_draw_params_ubo(&mut self, data: &[u8]) -> bool {
        if data.len() != self.draw_params_size {
            return false;
        }
        self.draw_params.copy_from_slice(data);
        true
    }

    pub fn draw_params(&self) -> &[u8] {
        &self.draw_params
    }
}

fn matrix_to_column_major(matrix: Matrix4) -> [f32; 16] {
    let mut values = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            values[column * 4 + row] = matrix.values[row][column] as f32;
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::Vector4;

    #[test]
    fn uniform_and_ubo_state_matches_shader_contract() {
        assert_eq!(std::mem::size_of::<FogBlock>(), 32);
        assert_eq!(std::mem::size_of::<MatricesBlock>(), 192);
        let mut shader = ShaderObject::new();
        shader.set_name("main");
        assert!(shader.load_shaders([("vertex", "v.glsl"), ("fragment", "f.glsl")]));
        shader.set_uniform("mode", &[2]);
        shader.set_uniform_f("range", &[1.0, 3.0]);
        assert_eq!(
            shader.uniforms().get("mode"),
            Some(&UniformValue::Integers(vec![2]))
        );
        shader.init_draw_params_ubo(4);
        assert!(!shader.update_draw_params_ubo(&[1, 2]));
        assert!(shader.update_draw_params_ubo(&[1, 2, 3, 4]));
        shader.enable();
        assert!(shader.is_enabled());
        shader.disable();
        assert!(!shader.is_enabled());
    }

    #[test]
    fn fog_and_matrix_blocks_use_reference_std140_layout() {
        let mut context = DisplayContext::new();
        context.set_fog_start(2.0);
        context.set_fog_end(6.0);
        context.set_fog_color(crate::SolidColor::from_rgb(0.25, 0.5, 0.75));
        context.scale(Vector4::new3(2.0, 4.0, 5.0));
        context.translate(Vector4::new3(1.0, 2.0, 3.0));
        let mut shader = ShaderObject::new();
        shader.setup_fog(&context);
        shader.setup_matrices(&context).unwrap();
        let fog = shader.fog_block().unwrap();
        assert_eq!(fog.fog_end, 6.0);
        assert_eq!(fog.fog_scale, 0.25);
        let matrices = shader.matrices_block().unwrap();
        assert_eq!(matrices.model_view[0], 2.0);
        assert_eq!(matrices.model_view[5], 4.0);
        assert_eq!(matrices.model_view[10], 5.0);
        assert_eq!(matrices.normal[0], 0.5);
        assert_eq!(matrices.normal[5], 0.25);
        assert_eq!(matrices.normal[10], 0.2);
        assert_eq!(matrices.normal[15], 1.0);
    }
}
